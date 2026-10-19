use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProjectStatus {
    pub path: String,
    pub branch: Option<String>,
    pub changed_files: Option<usize>,
    pub remote_url: Option<String>,
    pub error: Option<String>,
    pub detached: bool,
}
fn command_with(exe: &str, args: &[&str], cwd: &Path) -> Result<Vec<u8>, String> {
    let mut c = Command::new(exe);
    c.args(args)
        .current_dir(cwd)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    let mut child = c.spawn().map_err(|e| format!("Git 不可用：{e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let err_tx = tx.clone();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.read_to_end(&mut bytes);
        let _ = tx.send((true, result.map(|_| bytes)));
    });
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.read_to_end(&mut bytes);
        let _ = err_tx.send((false, result.map(|_| bytes)));
    });
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Git 查询超时".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    for _ in 0..2 {
        let (is_out, result) = rx
            .recv_timeout(timeout.saturating_sub(start.elapsed()))
            .map_err(|_| "Git 输出读取超时")?;
        let bytes = result.map_err(|e| e.to_string())?;
        if is_out {
            out = bytes
        } else {
            err = bytes
        }
    }
    if !status.success() {
        return Err(String::from_utf8_lossy(&err).trim().to_string());
    }
    Ok(out)
}
fn command(args: &[&str], cwd: &Path) -> Result<Vec<u8>, String> {
    command_with("git", args, cwd)
}
fn text(args: &[&str], cwd: &Path) -> Result<String, String> {
    command(args, cwd).map(|v| String::from_utf8_lossy(&v).trim().to_string())
}
fn directory(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err("路径不存在".into());
    }
    std::fs::canonicalize(if p.is_file() {
        p.parent().ok_or("文件没有所在目录")?
    } else {
        p
    })
    .map_err(|e| e.to_string())
}
fn repo(dir: &Path) -> Result<PathBuf, String> {
    let top = text(&["rev-parse", "--show-toplevel"], dir)?;
    std::fs::canonicalize(top).map_err(|e| e.to_string())
}
fn remote_url(s: &str) -> Option<String> {
    let s = s.trim();
    let url = if let Some(rest) = s.strip_prefix("ssh://") {
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?.split(':').next()?;
        format!("https://{host}/{path}")
    } else if s.starts_with("https://") || s.starts_with("http://") {
        s.to_string()
    } else {
        let (host, path) = s.split_once(':')?;
        let host = host.rsplit('@').next()?;
        if host.len() <= 1
            || host.contains('/')
            || (!host.contains('.') && !s.split_once(':')?.0.contains('@'))
            || path.starts_with('\\')
        {
            return None;
        }
        format!("https://{host}/{path}")
    };
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let (host, path) = rest.split_once('/')?;
    if host.is_empty()
        || path.is_empty()
        || host.contains('@')
        || url.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return None;
    }
    Some(
        url.trim_end_matches('/')
            .strip_suffix(".git")
            .unwrap_or(url.trim_end_matches('/'))
            .to_string(),
    )
}
fn changed_files(bytes: &[u8]) -> usize {
    let mut records = bytes.split(|b| *b == 0);
    let mut files = HashSet::new();
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        files.insert(record[3..].to_vec());
        if record[..2].contains(&b'R') || record[..2].contains(&b'C') {
            records.next();
        }
    }
    files.len()
}
fn blank(path: String) -> GitProjectStatus {
    GitProjectStatus {
        path,
        branch: None,
        changed_files: None,
        remote_url: None,
        error: None,
        detached: false,
    }
}
fn status(root: &Path) -> GitProjectStatus {
    let mut s = blank(String::new());
    match text(&["symbolic-ref", "--short", "-q", "HEAD"], root) {
        Ok(b) => s.branch = Some(b),
        Err(_) => s.detached = true,
    }
    match command(
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
        root,
    ) {
        Ok(v) => s.changed_files = Some(changed_files(&v)),
        Err(e) => s.error = Some(e),
    }
    if let Ok(remotes) = text(&["remote"], root) {
        let mut names: Vec<_> = remotes.lines().collect();
        names.sort_by_key(|n| if *n == "origin" { 0 } else { 1 });
        for name in names {
            if let Ok(url) = text(&["remote", "get-url", name], root) {
                if let Some(url) = remote_url(&url) {
                    s.remote_url = Some(url);
                    break;
                }
            }
        }
    }
    s
}
fn query_with(
    paths: Vec<String>,
    mut load: impl FnMut(&Path) -> GitProjectStatus,
) -> Vec<GitProjectStatus> {
    let mut roots: HashMap<PathBuf, Result<PathBuf, String>> = HashMap::new();
    let mut statuses = HashMap::new();
    paths
        .into_iter()
        .map(|path| {
            let result = directory(&path).and_then(|dir| {
                roots
                    .entry(dir.clone())
                    .or_insert_with(|| repo(&dir))
                    .clone()
            });
            match result {
                Ok(root) => {
                    let s = statuses
                        .entry(root.clone())
                        .or_insert_with(|| load(&root))
                        .clone();
                    GitProjectStatus { path, ..s }
                }
                Err(error) => {
                    let mut s = blank(path);
                    s.error = Some(if error.contains("not a git repository") {
                        "非 Git 仓库".into()
                    } else {
                        error
                    });
                    s
                }
            }
        })
        .collect()
}
pub fn query(paths: Vec<String>) -> Vec<GitProjectStatus> {
    query_with(paths, status)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        for input in [
            "git@github.com:a/b.git",
            "ssh://git@github.com:22/a/b.git",
            "https://github.com/a/b.git",
        ] {
            assert_eq!(remote_url(input).as_deref(), Some("https://github.com/a/b"));
        }
        assert_eq!(
            remote_url("http://git.local/a/b.git").as_deref(),
            Some("http://git.local/a/b")
        );
        for input in [
            "file:///a/b",
            "D:\\repo",
            "https://user:secret@host/a",
            "javascript:alert(1)",
        ] {
            assert!(remote_url(input).is_none());
        }
    }
    #[test]
    fn porcelain() {
        assert_eq!(
            changed_files(b"MM same\0?? new\0R  renamed\0original\0 M line\nname\0"),
            4
        );
        assert_eq!(changed_files(b""), 0);
    }
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[cfg(windows)]
    #[test]
    fn timeout() {
        let started = Instant::now();
        let error = command_with(
            "powershell.exe",
            &["-NoProfile", "-Command", "Start-Sleep -Seconds 20"],
            &std::env::temp_dir(),
        )
        .unwrap_err();
        assert!(error.contains("超时"));
        assert!(started.elapsed() < Duration::from_secs(9));
    }
    #[test]
    fn repositories() {
        let f =
            Fixture(std::env::temp_dir().join(format!("hub 中文 & 空格 {}", uuid::Uuid::new_v4())));
        std::fs::create_dir_all(&f.0).unwrap();
        let root = f.0.join("repo");
        std::fs::create_dir(&root).unwrap();
        text(&["init", "-b", "main"], &root).unwrap();
        let empty = query(vec![root.to_string_lossy().into()]);
        assert_eq!(empty[0].branch.as_deref(), Some("main"));
        assert_eq!(empty[0].changed_files, Some(0));
        std::fs::create_dir(root.join("sub")).unwrap();
        let sln = root.join("sub/test.sln");
        std::fs::write(&sln, "").unwrap();
        std::fs::write(root.join("same"), "initial").unwrap();
        text(&["add", "."], &root).unwrap();
        text(
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "initial",
            ],
            &root,
        )
        .unwrap();
        assert_eq!(status(&root).changed_files, Some(0));
        std::fs::write(root.join("same"), "staged").unwrap();
        text(&["add", "same"], &root).unwrap();
        std::fs::write(root.join("same"), "unstaged").unwrap();
        std::fs::write(root.join("untracked"), "").unwrap();
        std::fs::write(&sln, "changed").unwrap();
        let mut calls = 0;
        let results = query_with(
            vec![
                root.to_string_lossy().into(),
                sln.to_string_lossy().into(),
                root.join("sub").to_string_lossy().into(),
            ],
            |p| {
                calls += 1;
                status(p)
            },
        );
        assert_eq!(calls, 1);
        assert!(results.iter().all(|s| s.changed_files == Some(3)));
        let wt = f.0.join("worktree");
        text(
            &["worktree", "add", "-b", "other", wt.to_str().unwrap()],
            &root,
        )
        .unwrap();
        let results = query(vec![
            root.to_string_lossy().into(),
            wt.to_string_lossy().into(),
        ]);
        assert_eq!(results[1].branch.as_deref(), Some("other"));
        assert_eq!(results[1].changed_files, Some(0));
        assert_eq!(results[0].changed_files, Some(3));
        text(&["checkout", "--detach"], &wt).unwrap();
        assert!(status(&wt).detached);
        text(
            &[
                "remote",
                "add",
                "upstream",
                "ssh://git@gitlab.com:22/team/repo.git",
            ],
            &wt,
        )
        .unwrap();
        assert_eq!(
            status(&wt).remote_url.as_deref(),
            Some("https://gitlab.com/team/repo")
        );
        text(
            &["remote", "add", "origin", "git@github.com:team/origin.git"],
            &wt,
        )
        .unwrap();
        assert_eq!(
            status(&wt).remote_url.as_deref(),
            Some("https://github.com/team/origin")
        );
        for index in 0..700 {
            std::fs::write(wt.join(format!("{index:04}-{}", "x".repeat(100))), "").unwrap();
        }
        assert_eq!(status(&wt).changed_files, Some(700));
        assert!(query(vec![
            f.0.to_string_lossy().into(),
            f.0.join("missing").to_string_lossy().into()
        ])
        .iter()
        .all(|s| s.error.is_some()));
        assert!(
            command_with("project-hub-no-such-git-executable", &[], &root)
                .unwrap_err()
                .contains("Git 不可用")
        );
    }
}
