use crate::{
    database::{
        launch_target_available, new_id, now, platform, put_settings, resolve_launch_target,
        validate_path,
    },
    models::*,
    repository::{list_installations, list_projects, normalize_tags, save_installation},
};
use rusqlite::{params, Connection};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "windows")]
fn configure_hidden_console(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn configure_hidden_console(_command: &mut Command) {}

pub fn detect_ides(db: &Connection) -> Result<Vec<IdeInstallation>, String> {
    let installed = detected_candidates();
    for (ide_id, executable_path, launch_kind) in installed {
        let input = IdeInstallationInput {
            id: None,
            ide_id,
            executable_path,
            launch_kind,
            detected_source: "automatic".into(),
            enabled: true,
        };
        // Keep manual configurations intact; an automatic record is still useful when an IDE was upgraded.
        let _ = save_installation(db, input)?;
    }
    refresh_installation_statuses(db)?;
    list_installations(db)
}

fn refresh_installation_statuses(db: &Connection) -> Result<(), String> {
    let mut q = db
        .prepare("SELECT id, executable_path, launch_kind FROM ide_installations")
        .map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (id, exe, launch_kind) in rows {
        db.execute(
            "UPDATE ide_installations SET status=?1 WHERE id=?2",
            params![
                if launch_target_available(&exe, &launch_kind) {
                    "available"
                } else {
                    "missing"
                },
                id
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn refresh_module_path_statuses(db: &Connection) -> Result<(), String> {
    let mut query = db
        .prepare("SELECT module_id, platform, path, path_kind FROM module_paths")
        .map_err(|e| e.to_string())?;
    let paths = query
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(query);
    for (module_id, platform_name, path, kind) in paths {
        db.execute(
            "UPDATE module_paths SET validation_status=?1,last_validated_at=?2 WHERE module_id=?3 AND platform=?4",
            params![validate_path(&path, &kind), now(), module_id, platform_name],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn detected_candidates() -> Vec<(String, String, String)> {
    #[cfg(target_os = "windows")]
    {
        return windows_candidates();
    }
    #[cfg(target_os = "macos")]
    {
        return macos_candidates();
    }
    #[allow(unreachable_code)]
    Vec::new()
}

#[cfg(target_os = "windows")]
fn windows_candidates() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let program_files = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let local_app = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let add = |id: &str,
               p: PathBuf,
               out: &mut Vec<(String, String, String)>,
               seen: &mut HashSet<String>| {
        if p.is_file() && seen.insert(p.to_string_lossy().to_string()) {
            out.push((
                id.into(),
                p.to_string_lossy().to_string(),
                "executable".into(),
            ));
        }
    };
    if launch_target_available("code", "command") {
        out.push(("vscode".into(), "code".into(), "command".into()));
    }
    if let Some(root) = program_files.as_ref() {
        add(
            "vscode",
            root.join("Microsoft VS Code").join("Code.exe"),
            &mut out,
            &mut seen,
        );
        add(
            "hbuilderx",
            root.join("HBuilderX").join("HBuilderX.exe"),
            &mut out,
            &mut seen,
        );
        scan_jetbrains(
            &root.join("JetBrains"),
            "Rider",
            "rider",
            &mut out,
            &mut seen,
        );
        scan_jetbrains(
            &root.join("JetBrains"),
            "WebStorm",
            "webstorm",
            &mut out,
            &mut seen,
        );
    }
    if let Some(root) = local_app.as_ref() {
        add(
            "vscode",
            root.join("Programs")
                .join("Microsoft VS Code")
                .join("Code.exe"),
            &mut out,
            &mut seen,
        );
        scan_jetbrains(
            &root.join("Programs"),
            "Rider",
            "rider",
            &mut out,
            &mut seen,
        );
        scan_jetbrains(
            &root.join("Programs"),
            "WebStorm",
            "webstorm",
            &mut out,
            &mut seen,
        );
        scan_toolbox(
            &root.join("JetBrains").join("Toolbox").join("apps"),
            &mut out,
            &mut seen,
        );
    }
    if let Some(vs) = find_visual_studio() {
        add("visual-studio", vs, &mut out, &mut seen);
    }
    out
}

#[cfg(target_os = "windows")]
fn scan_jetbrains(
    root: &Path,
    product: &str,
    ide_id: &str,
    out: &mut Vec<(String, String, String)>,
    seen: &mut HashSet<String>,
) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(product) {
            let exe = entry.path().join("bin").join(if ide_id == "rider" {
                "rider64.exe"
            } else {
                "webstorm64.exe"
            });
            if exe.is_file() && seen.insert(exe.to_string_lossy().to_string()) {
                out.push((
                    ide_id.into(),
                    exe.to_string_lossy().to_string(),
                    "executable".into(),
                ));
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn scan_toolbox(root: &Path, out: &mut Vec<(String, String, String)>, seen: &mut HashSet<String>) {
    fn visit(
        dir: &Path,
        depth: usize,
        out: &mut Vec<(String, String, String)>,
        seen: &mut HashSet<String>,
    ) {
        if depth > 6 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, depth + 1, out, seen);
                continue;
            }
            let file = entry.file_name().to_string_lossy().to_ascii_lowercase();
            let ide_id = match file.as_str() {
                "rider64.exe" => Some("rider"),
                "webstorm64.exe" => Some("webstorm"),
                _ => None,
            };
            if let Some(ide_id) = ide_id {
                let value = path.to_string_lossy().to_string();
                if seen.insert(value.clone()) {
                    out.push((ide_id.into(), value, "executable".into()));
                }
            }
        }
    }
    visit(root, 0, out, seen);
}

#[cfg(target_os = "windows")]
fn find_visual_studio() -> Option<PathBuf> {
    let root = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from)?;
    let vswhere = root
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    if !vswhere.is_file() {
        return None;
    }
    let mut command = Command::new(vswhere);
    configure_hidden_console(&mut command);
    let output = command
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.Component.MSBuild",
            "-find",
            "Common7\\IDE\\devenv.exe",
        ])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.lines().next()?.trim();
    let result = PathBuf::from(path);
    result.is_file().then_some(result)
}

#[cfg(target_os = "macos")]
fn macos_candidates() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    if launch_target_available("code", "command") {
        out.push(("vscode".into(), "code".into(), "command".into()));
    }
    let roots = [
        PathBuf::from("/Applications"),
        std::env::var_os("HOME")
            .map(|p| PathBuf::from(p).join("Applications"))
            .unwrap_or_default(),
    ];
    for root in roots {
        for (id, name) in [
            ("rider", "Rider.app"),
            ("webstorm", "WebStorm.app"),
            ("vscode", "Visual Studio Code.app"),
            ("hbuilderx", "HBuilderX.app"),
        ] {
            let app = root.join(name);
            if app.is_dir() {
                out.push((
                    id.into(),
                    app.to_string_lossy().to_string(),
                    "macosApp".into(),
                ));
            }
        }
    }
    out
}

fn resolve_args(template: &[String], target: &Path, name: &str) -> Vec<String> {
    let path = target.to_string_lossy();
    let directory = if target.is_dir() {
        target.to_path_buf()
    } else {
        target.parent().unwrap_or(target).to_path_buf()
    };
    let directory = directory.to_string_lossy();
    template
        .iter()
        .map(|a| {
            a.replace("{path}", &path)
                .replace("{directory}", &directory)
                .replace("{name}", name)
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn quote_windows_arg(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".into();
    }
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            result.push_str(&"\\".repeat(slashes * 2 + 1));
            result.push('"');
        } else {
            result.push_str(&"\\".repeat(slashes));
            result.push(ch);
        }
        slashes = 0;
    }
    result.push_str(&"\\".repeat(slashes * 2));
    result.push('"');
    result
}

#[cfg(target_os = "windows")]
fn windows_command_line(executable: &Path, args: &[String]) -> String {
    std::iter::once(executable.to_string_lossy().to_string())
        .chain(args.iter().cloned())
        .map(|arg| quote_windows_arg(&arg))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn launch_module(
    db: &Connection,
    module_id: &str,
    override_ide_id: Option<String>,
) -> Result<LaunchResult, String> {
    let last_started = db
        .query_row(
            "SELECT MAX(opened_at) FROM open_history WHERE module_id=?1 AND result='started'",
            [module_id],
            |r| r.get::<_, Option<i64>>(0),
        )
        .map_err(|e| e.to_string())?;
    if last_started.is_some_and(|value| now() - value < 2) {
        return Err("该项目刚刚已经提交打开，请勿重复操作。".into());
    }
    let project_and_module = list_projects(db)?
        .into_iter()
        .find_map(|p| {
            p.modules
                .iter()
                .find(|m| m.id == module_id)
                .cloned()
                .map(|m| (p, m))
        })
        .ok_or_else(|| "找不到要打开的项目".to_string())?;
    let (project, module) = project_and_module;
    let target = module
        .path
        .clone()
        .ok_or_else(|| format!("{} / {} 未配置当前平台路径", project.name, module.name))?;
    let status = validate_path(&target.path, &target.path_kind);
    if status != "valid" {
        return Err(format!(
            "无法打开 {} / {}：路径 {} 当前状态为 {}，请编辑项目路径。",
            project.name, module.name, target.path, status
        ));
    }
    let ide_id = override_ide_id
        .or(module.ide_id.clone())
        .ok_or_else(|| format!("{} / {} 未指定 IDE", project.name, module.name))?;
    let installation = list_installations(db)?
        .into_iter()
        .find(|x| {
            x.ide_id == ide_id && x.platform == platform() && x.enabled && x.status == "available"
        })
        .ok_or_else(|| {
            format!(
                "{} / {} 的 IDE 未检测到或未启用，请前往 IDE 管理重新配置。",
                project.name, module.name
            )
        })?;
    let target_path = PathBuf::from(&target.path);
    let args = resolve_args(&module.argument_template, &target_path, &module.name);
    let spawn = if installation.launch_kind == "macosApp" {
        Command::new("open")
            .arg("-a")
            .arg(&installation.executable_path)
            .arg(&target.path)
            .spawn()
    } else if installation.launch_kind == "command" {
        let resolved = resolve_launch_target(&installation.executable_path, "command")
            .ok_or_else(|| format!("找不到 PATH 命令：{}", installation.executable_path))?;
        #[cfg(target_os = "windows")]
        {
            if matches!(
                resolved.extension().and_then(|x| x.to_str()),
                Some("cmd" | "bat")
            ) {
                let mut command = Command::new("cmd.exe");
                configure_hidden_console(&mut command);
                command
                    .args(["/D", "/S", "/C"])
                    .arg(windows_command_line(&resolved, &args))
                    .spawn()
            } else {
                let mut command = Command::new(&resolved);
                configure_hidden_console(&mut command);
                command.args(&args).spawn()
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Command::new(&resolved).args(&args).spawn()
        }
    } else {
        let mut command = Command::new(&installation.executable_path);
        configure_hidden_console(&mut command);
        command.args(&args).spawn()
    };
    match spawn {
        Ok(_) => {
            record_history(db, &module.id, Some(&ide_id), "started", None)?;
            Ok(LaunchResult {
                module_id: module.id,
                project_name: project.name,
                module_name: module.name,
                ide_name: installation.ide_name.unwrap_or(installation.ide_id),
                path: target.path,
                started: true,
                message: "已提交给操作系统启动".into(),
            })
        }
        Err(e) => {
            let msg = format!(
                "无法使用 {} 打开 {}：{}",
                installation
                    .ide_name
                    .clone()
                    .unwrap_or(installation.ide_id.clone()),
                target.path,
                e
            );
            record_history(db, &module.id, Some(&ide_id), "failed", Some(&msg))?;
            Err(msg)
        }
    }
}

pub fn reveal_module_path(db: &Connection, module_id: &str) -> Result<(), String> {
    let module = list_projects(db)?
        .into_iter()
        .flat_map(|p| p.modules)
        .find(|m| m.id == module_id)
        .ok_or_else(|| "项目不存在".to_string())?;
    let target = module
        .path
        .ok_or_else(|| "项目未配置当前平台路径".to_string())?;
    let path = PathBuf::from(&target.path);
    if !path.exists() {
        return Err(format!("路径不存在：{}", target.path));
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("explorer.exe");
        configure_hidden_console(&mut cmd);
        if path.is_file() {
            cmd.arg(format!("/select,{}", target.path));
        } else {
            cmd.arg(&target.path);
        }
        cmd.spawn()
            .map_err(|e| format!("无法打开资源管理器：{e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = Command::new("open");
        if path.is_file() {
            cmd.arg("-R");
        }
        cmd.arg(&target.path);
        cmd.spawn()
            .map_err(|e| format!("无法在 Finder 中显示路径：{e}"))?;
    }
    Ok(())
}

fn record_history(
    db: &Connection,
    module_id: &str,
    ide_id: Option<&str>,
    result: &str,
    error: Option<&str>,
) -> Result<(), String> {
    db.execute("INSERT INTO open_history(id,module_id,ide_id,opened_at,result,error_message) VALUES(?1,?2,?3,?4,?5,?6)",params![new_id(),module_id,ide_id,now(),result,error]).map_err(|e|e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_args;
    use std::path::Path;

    #[test]
    fn substitutes_unicode_paths_without_shell_quoting() {
        let target = Path::new(r#"D:\项目 目录\API\示例.sln"#);
        let args = resolve_args(
            &[
                "{path}".into(),
                "--name={name}".into(),
                "--root={directory}".into(),
            ],
            target,
            "业务 API",
        );
        assert_eq!(args[0], target.to_string_lossy());
        assert_eq!(args[1], "--name=业务 API");
        assert!(args[2].contains("项目 目录"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_command_line_quotes_batch_launcher_and_arguments() {
        use super::windows_command_line;
        let line = windows_command_line(
            Path::new(r#"C:\Tools\VS Code\bin\code.cmd"#),
            &[r#"D:\项目 目录\API"#.into(), "--reuse-window".into()],
        );
        assert_eq!(
            line,
            r#""C:\Tools\VS Code\bin\code.cmd" "D:\项目 目录\API" "--reuse-window""#
        );
    }
}

pub fn export_bundle(db: &Connection) -> Result<ExportBundle, String> {
    let mut q=db.prepare("SELECT c.module_id,c.platform,mp.path,mp.path_kind,mp.validation_status,mp.last_validated_at,lp.ide_id,lp.argument_template FROM (SELECT module_id,platform FROM module_paths UNION SELECT module_id,platform FROM module_launch_profiles) c LEFT JOIN module_paths mp ON mp.module_id=c.module_id AND mp.platform=c.platform LEFT JOIN module_launch_profiles lp ON lp.module_id=c.module_id AND lp.platform=c.platform").map_err(|e|e.to_string())?;
    let configs = q
        .query_map([], |r| {
            let raw: Option<String> = r.get(7)?;
            let path: Option<String> = r.get(2)?;
            let platform: String = r.get(1)?;
            let module_path = match path {
                Some(path) => Some(ModulePath {
                    platform: platform.clone(),
                    path,
                    path_kind: r.get(3)?,
                    validation_status: r.get(4)?,
                    last_validated_at: r.get(5)?,
                }),
                None => None,
            };
            Ok(ModulePlatformConfig {
                module_id: r.get(0)?,
                platform,
                path: module_path,
                ide_id: r.get(6)?,
                argument_template: raw
                    .map(|x| serde_json::from_str(&x).unwrap_or_else(|_| vec!["{path}".into()]))
                    .unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(ExportBundle {
        schema_version: EXPORT_SCHEMA_VERSION,
        exported_at: now(),
        projects: list_projects(db)?,
        module_platform_configs: configs,
        ide_installations: list_installations(db)?,
        settings: crate::database::get_settings(db)?,
    })
}

pub fn write_export_file(db: &Connection, path: &str) -> Result<(), String> {
    let target = Path::new(path);
    if target.as_os_str().is_empty() {
        return Err("请选择配置导出位置".into());
    };
    let data = serde_json::to_string_pretty(&export_bundle(db)?)
        .map_err(|e| format!("无法序列化配置：{e}"))?;
    fs::write(target, data).map_err(|e| format!("无法写入配置文件 {}：{e}", target.display()))
}
pub fn read_import_file(db: &mut Connection, path: &str) -> Result<ImportResult, String> {
    let target = Path::new(path);
    let content = fs::read_to_string(target)
        .map_err(|e| format!("无法读取配置文件 {}：{e}", target.display()))?;
    let bundle: ExportBundle =
        serde_json::from_str(&content).map_err(|e| format!("配置文件不是有效的 JSON：{e}"))?;
    import_bundle(db, bundle)
}

pub fn import_bundle(db: &mut Connection, bundle: ExportBundle) -> Result<ImportResult, String> {
    if bundle.schema_version > EXPORT_SCHEMA_VERSION {
        return Err(format!(
            "配置版本 {} 比当前应用支持的版本新",
            bundle.schema_version
        ));
    }
    let mut result = ImportResult {
        projects_imported: 0,
        modules_imported: 0,
        installations_imported: 0,
        missing_paths: 0,
    };
    let timestamp = now();
    let mut ids = HashMap::new();
    let tx = db.transaction().map_err(|e| e.to_string())?;
    for project in &bundle.projects {
        let pid = new_id();
        let project_tags = serde_json::to_string(&normalize_tags(project.tags.clone()))
            .map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO projects(id,name,description,icon,color,tags,is_favorite,sort_order,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",params![pid,project.name,project.description,project.icon,project.color,project_tags,if project.is_favorite{1}else{0},project.sort_order,timestamp]).map_err(|e|e.to_string())?;
        result.projects_imported += 1;
        for module in &project.modules {
            let mid = new_id();
            ids.insert(module.id.clone(), mid.clone());
            let module_tags = serde_json::to_string(&normalize_tags(module.tags.clone()))
                .map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO project_modules(id,project_id,name,module_type,description,tags,is_favorite,sort_order,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)",params![mid,pid,module.name,module.module_type,module.description,module_tags,if module.is_favorite{1}else{0},module.sort_order,timestamp]).map_err(|e|e.to_string())?;
            result.modules_imported += 1;
        }
    }
    let configs = if bundle.module_platform_configs.is_empty() {
        // compatibility with v1 previews that exported only the current UI path
        bundle
            .projects
            .iter()
            .flat_map(|p| p.modules.iter())
            .map(|m| ModulePlatformConfig {
                module_id: m.id.clone(),
                platform: platform().into(),
                path: m.path.clone(),
                ide_id: m.ide_id.clone(),
                argument_template: m.argument_template.clone(),
            })
            .collect()
    } else {
        bundle.module_platform_configs.clone()
    };
    for cfg in configs {
        let Some(mid) = ids.get(&cfg.module_id) else {
            continue;
        };
        if let Some(path) = cfg.path {
            let status = validate_path(&path.path, &path.path_kind);
            if status != "valid" {
                result.missing_paths += 1
            }
            tx.execute("INSERT INTO module_paths(module_id,platform,path,path_kind,validation_status,last_validated_at) VALUES(?1,?2,?3,?4,?5,?6)",params![mid,cfg.platform,path.path,path.path_kind,status,timestamp]).map_err(|e|e.to_string())?;
        }
        if let Some(ide) = cfg.ide_id {
            let args = serde_json::to_string(&cfg.argument_template).map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO module_launch_profiles(module_id,platform,ide_id,argument_template) VALUES(?1,?2,?3,?4)",params![mid,cfg.platform,ide,args]).map_err(|e|e.to_string())?;
        }
    }
    for installation in &bundle.ide_installations {
        let status =
            if launch_target_available(&installation.executable_path, &installation.launch_kind) {
                "available"
            } else {
                "missing"
            };
        tx.execute("INSERT OR IGNORE INTO ide_installations(id,ide_id,platform,executable_path,launch_kind,detected_source,status,enabled) VALUES(?1,?2,?3,?4,?5,'import',?6,?7)",params![new_id(),installation.ide_id,installation.platform,installation.executable_path,installation.launch_kind,status,if installation.enabled{1}else{0}]).map_err(|e|e.to_string())?;
        result.installations_imported += 1;
    }
    tx.commit().map_err(|e| e.to_string())?;
    put_settings(db, &bundle.settings)?;
    Ok(result)
}
