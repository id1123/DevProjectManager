//! Path maintenance and named launch groups for local projects.
use crate::{
    database::{new_id, now, platform},
    models::*,
    repository, services,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathChange {
    pub module_id: String,
    pub old_path: Option<String>,
    pub path_kind: String,
    pub new_path: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathRow {
    pub change: PathChange,
    pub project_id: String,
    pub project_name: String,
    pub module_name: String,
    pub status: String,
    pub message: String,
}

/// Inspect a target without treating permission and type errors as missing paths.
pub fn inspect(path: &str, kind: &str) -> (String, String) {
    if path.trim().is_empty() {
        return ("unconfigured".into(), "未配置路径".into());
    }
    if !Path::new(path).is_absolute() {
        return ("invalid".into(), "请选择绝对路径".into());
    }
    match std::fs::metadata(path) {
        Ok(m) if (kind == "file" && m.is_file()) || (kind == "directory" && m.is_dir()) => {
            ("valid".into(), "有效".into())
        }
        Ok(_) => ("wrongKind".into(), "文件或目录类型不匹配".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            ("missing".into(), "路径不存在".into())
        }
        Err(e) => ("inaccessible".into(), format!("无法访问：{e}")),
    }
}

/// List current-platform targets, optionally limited to one business project.
pub fn inspect_paths(db: &Connection, project_id: Option<&str>) -> Result<Vec<PathRow>, String> {
    let projects = repository::list_projects(db)?;
    Ok(projects
        .into_iter()
        .filter(|p| project_id.is_none_or(|id| id == p.id))
        .flat_map(|p| {
            p.modules.into_iter().map(move |m| {
                let old_path = m.path.as_ref().map(|x| x.path.clone());
                let kind = m
                    .path
                    .as_ref()
                    .map(|x| x.path_kind.clone())
                    .unwrap_or_else(|| "directory".into());
                let target = old_path.clone().unwrap_or_default();
                let (status, message) = inspect(&target, &kind);
                PathRow {
                    change: PathChange {
                        module_id: m.id,
                        old_path,
                        path_kind: kind,
                        new_path: target,
                    },
                    project_id: p.id.clone(),
                    project_name: p.name.clone(),
                    module_name: m.name,
                    status,
                    message,
                }
            })
        })
        .collect())
}

/// Replace a complete directory prefix, preserving path component boundaries.
fn replace_prefix(path: &str, old: &str, new: &str, windows: bool) -> Option<String> {
    if old.trim().is_empty() || new.trim().is_empty() {
        return None;
    }
    let normalize = |s: &str| {
        if windows {
            s.replace('\\', "/")
        } else {
            s.to_string()
        }
    };
    let p = normalize(path);
    let old = normalize(old);
    let old = old.trim_end_matches('/');
    let new = normalize(new);
    let matches = if windows {
        p.get(..old.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(old))
    } else {
        p.starts_with(old)
    };
    if !matches || !(p.len() == old.len() || p.as_bytes().get(old.len()) == Some(&b'/')) {
        return None;
    }
    let suffix = &p[old.len()..];
    let result = if suffix.is_empty() {
        // Preserve a destination root (E:/ or /) when the target equals the prefix.
        new.clone()
    } else {
        format!("{}{}", new.trim_end_matches('/'), suffix)
    };
    let result = if windows {
        result.replace('/', "\\")
    } else {
        result
    };
    (result != path).then_some(result)
}

/// Preview a prefix replacement without writing any configuration.
pub fn prefix_paths(
    db: &Connection,
    project_id: Option<&str>,
    old: &str,
    new: &str,
) -> Result<Vec<PathRow>, String> {
    if old.trim().is_empty()
        || new.trim().is_empty()
        || !Path::new(old).is_absolute()
        || !Path::new(new).is_absolute()
    {
        return Err("请填写完整的旧目录和新目录".into());
    }
    Ok(inspect_paths(db, project_id)?
        .into_iter()
        .filter_map(|mut row| {
            row.change.new_path =
                replace_prefix(row.change.old_path.as_deref()?, old, new, cfg!(windows))?;
            (row.status, row.message) = inspect(&row.change.new_path, &row.change.path_kind);
            Some(row)
        })
        .collect())
}

/// Verify a proposed repair against the current data to detect stale previews.
pub fn preview_paths(db: &Connection, changes: &[PathChange]) -> Result<Vec<PathRow>, String> {
    let current = inspect_paths(db, None)?;
    let mut seen = HashSet::new();
    changes
        .iter()
        .map(|change| {
            if !seen.insert(&change.module_id) {
                return Err("同一项目不能重复修改".into());
            }
            let mut row = current
                .iter()
                .find(|r| r.change.module_id == change.module_id)
                .cloned()
                .ok_or("项目已删除，请重新检查")?;
            if row.change.old_path != change.old_path {
                return Err("项目路径已被修改，请重新检查".into());
            }
            if !["file", "directory"].contains(&change.path_kind.as_str()) {
                return Err("路径类型无效".into());
            }
            row.change = change.clone();
            (row.status, row.message) = inspect(&change.new_path, &change.path_kind);
            Ok(row)
        })
        .collect()
}

/// Apply valid repairs atomically while preserving IDE and linked-project configuration.
pub fn apply_paths(db: &mut Connection, changes: &[PathChange]) -> Result<(), String> {
    if changes.is_empty() {
        return Err("请先选择要保存的路径".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let rows = preview_paths(&tx, changes)?;
    for row in rows {
        if row.status != "valid" {
            return Err(format!("{}：{}", row.module_name, row.message));
        }
        let c = row.change;
        tx.execute("INSERT INTO module_paths(module_id,platform,path,path_kind,validation_status,last_validated_at) VALUES(?1,?2,?3,?4,'valid',?5) ON CONFLICT(module_id,platform) DO UPDATE SET path=excluded.path,path_kind=excluded.path_kind,validation_status='valid',last_validated_at=excluded.last_validated_at", params![c.module_id,platform(),c.new_path,c.path_kind,now()]).map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE project_modules SET updated_at=?2 WHERE id=?1",
            params![c.module_id, now()],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupItem {
    pub kind: String,
    pub target: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkGroup {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub items: Vec<GroupItem>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub index: usize,
    pub label: String,
    pub status: String,
    pub message: String,
}

/// Load all named groups in stable display order.
pub fn list_groups(db: &Connection) -> Result<Vec<WorkGroup>, String> {
    let mut q = db
        .prepare("SELECT id,project_id,name,items FROM work_groups ORDER BY name,id")
        .map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.map(|r| {
        let (id, project_id, name, raw) = r.map_err(|e| e.to_string())?;
        Ok(WorkGroup {
            id,
            project_id,
            name,
            items: serde_json::from_str(&raw).map_err(|e| format!("组合数据无效：{e}"))?,
        })
    })
    .collect()
}

/// Accept only web links with a host, excluding credentials and command protocols.
fn valid_url(value: &str) -> bool {
    tauri::Url::parse(value).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https")
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
    })
}

/// Save an ordered group, allowing existing deleted references to remain visible for repair.
pub fn save_group(db: &Connection, mut group: WorkGroup) -> Result<WorkGroup, String> {
    group.name = group.name.trim().to_string();
    if group.name.is_empty() || group.items.is_empty() {
        return Err("请填写组合名称并添加至少一个启动项".into());
    }
    if group.items.len() > 50 {
        return Err("一个组合最多包含 50 个启动项".into());
    }
    let projects = repository::list_projects(db)?;
    let project = projects
        .iter()
        .find(|p| p.id == group.project_id)
        .ok_or("业务项目已删除")?;
    let groups = list_groups(db)?;
    let existing = groups.iter().find(|g| g.id == group.id);
    if !group.id.is_empty() && existing.is_none() {
        return Err("组合已删除，请重新加载".into());
    }
    if existing.is_some_and(|g| g.project_id != group.project_id) {
        return Err("不能移动组合到其他业务项目".into());
    }
    let mut seen = HashSet::new();
    for item in &mut group.items {
        item.target = item.target.trim().to_string();
        if !seen.insert((item.kind.clone(), item.target.clone())) {
            return Err("组合包含重复启动项".into());
        }
        match item.kind.as_str() {
            "module" | "terminal" => {
                let known = project.modules.iter().any(|m| m.id == item.target);
                let retained = existing.is_some_and(|g| {
                    g.items
                        .iter()
                        .any(|i| i.kind == item.kind && i.target == item.target)
                });
                if !known && !retained {
                    return Err("只能选择当前业务项目中的项目".into());
                }
            }
            "folder" if Path::new(&item.target).is_absolute() => {}
            "url" if valid_url(&item.target) => {}
            _ => return Err("启动项无效：文件夹须为绝对路径，网址仅支持 HTTP/HTTPS".into()),
        }
    }
    if group.id.is_empty() {
        group.id = new_id();
    }
    db.execute("INSERT INTO work_groups(id,project_id,name,items) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET name=excluded.name,items=excluded.items", params![group.id,group.project_id,group.name,serde_json::to_string(&group.items).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
    Ok(group)
}

/// Check one group item using current project and IDE definitions.
fn check_item(
    item: &GroupItem,
    project: &Project,
    installations: &[IdeInstallation],
    definitions: &[IdeDefinition],
) -> Result<String, String> {
    match item.kind.as_str() {
        "module" | "terminal" => {
            let m = project
                .modules
                .iter()
                .find(|m| m.id == item.target)
                .ok_or("项目已删除，请编辑组合移除失效项")?;
            let path = m.path.as_ref().ok_or("未配置项目路径")?;
            let (status, message) = inspect(&path.path, &path.path_kind);
            if status != "valid" {
                return Err(message);
            }
            if item.kind == "module" {
                let ide = m.ide_id.as_ref().ok_or("未指定 IDE")?;
                let definition = definitions
                    .iter()
                    .find(|d| &d.id == ide)
                    .ok_or("IDE 配置已删除")?;
                if !definition.supported_path_kinds.contains(&path.path_kind)
                    || !definition
                        .supported_platforms
                        .iter()
                        .any(|p| p == platform())
                {
                    return Err("IDE 不支持当前平台或路径类型".into());
                }
                let installation = installations
                    .iter()
                    .find(|i| {
                        &i.ide_id == ide
                            && i.platform == platform()
                            && i.enabled
                            && i.status == "available"
                    })
                    .ok_or("IDE 未启用或不可用，请到设置检查")?;
                if !crate::database::launch_target_available(
                    &installation.executable_path,
                    &installation.launch_kind,
                ) {
                    return Err("IDE 启动程序不存在".into());
                }
            }
            Ok(format!(
                "{} · {}",
                m.name,
                if item.kind == "terminal" {
                    "终端"
                } else {
                    "IDE"
                }
            ))
        }
        "folder" => {
            let (s, m) = inspect(&item.target, "directory");
            if s == "valid" {
                Ok(item.target.clone())
            } else {
                Err(m)
            }
        }
        "url" if valid_url(&item.target) => Ok(item.target.clone()),
        _ => Err("启动项类型或网址无效".into()),
    }
}

/// Check a saved group's current targets without launching anything.
pub fn check_group(db: &Connection, id: &str) -> Result<Vec<ItemResult>, String> {
    let group = list_groups(db)?
        .into_iter()
        .find(|g| g.id == id)
        .ok_or("组合已删除")?;
    let project = repository::list_projects(db)?
        .into_iter()
        .find(|p| p.id == group.project_id)
        .ok_or("业务项目已删除")?;
    let installations = repository::list_installations(db)?;
    let definitions = repository::list_ide_definitions(db)?;
    Ok(group
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let fallback = project
                .modules
                .iter()
                .find(|m| m.id == item.target)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| item.target.clone());
            match check_item(item, &project, &installations, &definitions) {
                Ok(label) => ItemResult {
                    index,
                    label,
                    status: "ready".into(),
                    message: "可以启动".into(),
                },
                Err(message) => ItemResult {
                    index,
                    label: fallback,
                    status: "invalid".into(),
                    message,
                },
            }
        })
        .collect())
}

/// Reject stale launch previews so retry indices never point to different items.
pub fn run_group_snapshot(
    db: &Connection,
    id: &str,
    expected: &[GroupItem],
    indices: Option<Vec<usize>>,
    skip_invalid: bool,
) -> Result<Vec<ItemResult>, String> {
    let group = list_groups(db)?
        .into_iter()
        .find(|g| g.id == id)
        .ok_or("组合已删除")?;
    if group.items != expected {
        return Err("组合清单已变更，请关闭后重新检查再启动".into());
    }
    run_group(db, id, indices, skip_invalid)
}

/// Execute selected items in order, never expanding ordinary project links.
pub fn run_group(
    db: &Connection,
    id: &str,
    indices: Option<Vec<usize>>,
    skip_invalid: bool,
) -> Result<Vec<ItemResult>, String> {
    run_group_with(
        db,
        id,
        indices,
        skip_invalid,
        |item, project, installations| match item.kind.as_str() {
            "module" => services::launch_single_module(
                db,
                project,
                project
                    .modules
                    .iter()
                    .find(|m| m.id == item.target)
                    .unwrap(),
                None,
                installations,
            )
            .map(|_| ()),
            "terminal" => services::open_project_terminal(
                &project
                    .modules
                    .iter()
                    .find(|m| m.id == item.target)
                    .unwrap()
                    .path
                    .as_ref()
                    .unwrap()
                    .path,
            ),
            "folder" => tauri_plugin_opener::open_path(&item.target, None::<&str>)
                .map_err(|e| e.to_string()),
            "url" => {
                tauri_plugin_opener::open_url(&item.target, None::<&str>).map_err(|e| e.to_string())
            }
            _ => Err("启动项无效".into()),
        },
    )
}

/// Share ordering and failure handling with side-effect-free execution tests.
fn run_group_with(
    db: &Connection,
    id: &str,
    indices: Option<Vec<usize>>,
    skip_invalid: bool,
    mut launch: impl FnMut(&GroupItem, &Project, &[IdeInstallation]) -> Result<(), String>,
) -> Result<Vec<ItemResult>, String> {
    let group = list_groups(db)?
        .into_iter()
        .find(|g| g.id == id)
        .ok_or("组合已删除")?;
    let project = repository::list_projects(db)?
        .into_iter()
        .find(|p| p.id == group.project_id)
        .ok_or("业务项目已删除")?;
    let installations = repository::list_installations(db)?;
    let definitions = repository::list_ide_definitions(db)?;
    let selected: HashSet<usize> = indices
        .unwrap_or_else(|| (0..group.items.len()).collect())
        .into_iter()
        .collect();
    if selected.is_empty() || selected.iter().any(|i| *i >= group.items.len()) {
        return Err("启动项已变更，请重新检查".into());
    }
    let checks: Vec<_> = group
        .items
        .iter()
        .enumerate()
        .filter(|(i, _)| selected.contains(i))
        .map(|(i, item)| {
            (
                i,
                item,
                check_item(item, &project, &installations, &definitions),
            )
        })
        .collect();
    if !skip_invalid && checks.iter().any(|(_, _, r)| r.is_err()) {
        return Err("检查发现异常，请重新检查后修复或选择跳过异常项".into());
    }
    Ok(checks
        .into_iter()
        .map(|(index, item, check)| {
            let label = project
                .modules
                .iter()
                .find(|m| m.id == item.target)
                .map(|m| m.name.clone())
                .unwrap_or_else(|| item.target.clone());
            let label = match check {
                Ok(label) => label,
                Err(message) => {
                    return ItemResult {
                        index,
                        label,
                        status: "skipped".into(),
                        message,
                    }
                }
            };
            let result = launch(item, &project, &installations);
            match result {
                Ok(()) => ItemResult {
                    index,
                    label,
                    status: "started".into(),
                    message: "已提交系统打开".into(),
                },
                Err(message) => ItemResult {
                    index,
                    label,
                    status: "failed".into(),
                    message,
                },
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix_respects_boundaries_and_special_characters() {
        assert_eq!(
            replace_prefix(
                r"D:\Project\中文 & test\a.sln",
                r"d:\project",
                r"E:\Code",
                true
            ),
            Some(r"E:\Code\中文 & test\a.sln".into())
        );
        assert_eq!(
            replace_prefix(r"D:\Project2\a", r"D:\Project", r"E:\Code", true),
            None
        );
        assert_eq!(replace_prefix("/Work/a", "/work", "/new", false), None);
        assert_eq!(
            replace_prefix("/work", "/work", "/", false),
            Some("/".into())
        );
        assert_eq!(
            replace_prefix(r"D:\Work", r"D:\Work", r"E:\", true),
            Some(r"E:\".into())
        );
        assert_eq!(
            replace_prefix("/a", "/", "/new", false),
            Some("/new/a".into())
        );
        assert_eq!(
            replace_prefix(r"D:\a", r"D:\", r"E:\", true),
            Some(r"E:\a".into())
        );
    }
    #[test]
    fn web_links_reject_command_protocols_and_credentials() {
        assert!(valid_url("https://example.com/docs?q=中文"));
        for url in [
            "javascript:alert(1)",
            "file:///C:/test",
            "https://",
            "https://user:password@example.com",
        ] {
            assert!(!valid_url(url));
        }
    }
    /// Create a real migrated in-memory database for configuration-level checks.
    fn database() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        crate::database::migrate(&db).unwrap();
        db.execute(
            "INSERT INTO projects(id,name,created_at,updated_at) VALUES('p','业务',0,0)",
            [],
        )
        .unwrap();
        for id in ["a", "b"] {
            db.execute("INSERT INTO project_modules(id,project_id,name,module_type,created_at,updated_at) VALUES(?1,'p',?1,'other',0,0)",[id]).unwrap();
        }
        db
    }

    #[test]
    fn path_batch_rolls_back_and_rejects_stale_or_duplicate_changes() {
        let mut db = database();
        let target = std::env::temp_dir().to_string_lossy().to_string();
        let a = PathChange {
            module_id: "a".into(),
            old_path: None,
            path_kind: "directory".into(),
            new_path: target.clone(),
        };
        let mut b = PathChange {
            module_id: "b".into(),
            ..a.clone()
        };
        b.new_path = std::env::temp_dir()
            .join(new_id())
            .to_string_lossy()
            .to_string();
        assert!(apply_paths(&mut db, &[a.clone(), b]).is_err());
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM module_paths", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(apply_paths(&mut db, &[a.clone(), a.clone()]).is_err());
        apply_paths(&mut db, &[a.clone()]).unwrap();
        assert!(apply_paths(&mut db, &[a]).unwrap_err().contains("已被修改"));
        let rows = inspect_paths(&db, Some("p")).unwrap();
        assert_eq!(
            rows.iter()
                .find(|r| r.change.module_id == "a")
                .unwrap()
                .status,
            "valid"
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.change.module_id == "b")
                .unwrap()
                .status,
            "unconfigured"
        );
    }

    #[test]
    fn file_targets_and_wrong_types_are_distinguished() {
        let path = std::env::temp_dir().join(format!("中文 & ; {}.sln", new_id()));
        std::fs::write(&path, "").unwrap();
        assert_eq!(inspect(path.to_str().unwrap(), "file").0, "valid");
        assert_eq!(inspect(path.to_str().unwrap(), "directory").0, "wrongKind");
        std::fs::remove_file(path).unwrap();
        assert_eq!(inspect("", "file").0, "unconfigured");
        assert_eq!(inspect("relative/file", "file").0, "invalid");
    }

    #[test]
    fn groups_preserve_order_and_deleted_references_and_cascade_with_parent() {
        let db = database();
        let group = save_group(
            &db,
            WorkGroup {
                id: "".into(),
                project_id: "p".into(),
                name: "联调".into(),
                items: vec![
                    GroupItem {
                        kind: "terminal".into(),
                        target: "b".into(),
                    },
                    GroupItem {
                        kind: "module".into(),
                        target: "a".into(),
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(list_groups(&db).unwrap()[0].items[0].target, "b");
        db.execute("DELETE FROM project_modules WHERE id='a'", [])
            .unwrap();
        assert!(check_group(&db, &group.id).unwrap()[1]
            .message
            .contains("已删除"));
        save_group(&db, group.clone()).unwrap();
        assert!(run_group(&db, &group.id, None, false).is_err());
        assert!(run_group_snapshot(&db, &group.id, &[], Some(vec![0]), true)
            .unwrap_err()
            .contains("清单已变更"));
        let results = run_group(&db, &group.id, Some(vec![1]), true).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "skipped");
        assert!(run_group(&db, &group.id, Some(vec![99]), true).is_err());
        db.execute("DELETE FROM projects WHERE id='p'", []).unwrap();
        assert!(list_groups(&db).unwrap().is_empty());
        crate::database::migrate(&db).unwrap();
    }

    #[test]
    fn groups_reject_cross_project_targets_duplicate_items_and_unsafe_links() {
        let db = database();
        for items in [
            vec![GroupItem {
                kind: "module".into(),
                target: "unknown".into(),
            }],
            vec![GroupItem {
                kind: "url".into(),
                target: "file:///C:/test".into(),
            }],
            vec![
                GroupItem {
                    kind: "module".into(),
                    target: "a".into(),
                },
                GroupItem {
                    kind: "module".into(),
                    target: "a".into(),
                },
            ],
        ] {
            assert!(save_group(
                &db,
                WorkGroup {
                    id: "".into(),
                    project_id: "p".into(),
                    name: "test".into(),
                    items
                }
            )
            .is_err());
        }
    }

    #[test]
    fn execution_continues_after_failure_and_retries_only_selected_indices() {
        let db = database();
        let group = save_group(
            &db,
            WorkGroup {
                id: "".into(),
                project_id: "p".into(),
                name: "sequence".into(),
                items: (0..3)
                    .map(|i| GroupItem {
                        kind: "url".into(),
                        target: format!("https://example.com/{i}"),
                    })
                    .collect(),
            },
        )
        .unwrap();
        let mut visited = Vec::new();
        let results = run_group_with(&db, &group.id, None, false, |item, _, _| {
            visited.push(item.target.clone());
            if item.target.ends_with('1') {
                Err("simulated OS launch failure".into())
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(
            visited,
            group
                .items
                .iter()
                .map(|i| i.target.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            results
                .iter()
                .map(|r| r.status.as_str())
                .collect::<Vec<_>>(),
            vec!["started", "failed", "started"]
        );
        visited.clear();
        let retries = results
            .iter()
            .filter(|r| r.status == "failed")
            .map(|r| r.index)
            .collect();
        let results = run_group_with(&db, &group.id, Some(retries), false, |item, _, _| {
            visited.push(item.target.clone());
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, vec!["https://example.com/1"]);
        assert_eq!(results[0].status, "started");
    }
}
