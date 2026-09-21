use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(target_os = "windows")]
fn configure_hidden_console(command: &mut std::process::Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn configure_hidden_console(_command: &mut std::process::Command) {}

pub struct AppState {
    pub db: Mutex<Connection>,
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else {
        "windows"
    }
}
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn launch_target_available(value: &str, launch_kind: &str) -> bool {
    resolve_launch_target(value, launch_kind).is_some()
}

/// Resolves a configured executable or PATH command without invoking a shell.
/// The resolved path is also used to handle Windows `.cmd` launchers correctly.
pub fn resolve_launch_target(value: &str, launch_kind: &str) -> Option<PathBuf> {
    if launch_kind != "command" {
        return PathBuf::from(value).exists().then(|| PathBuf::from(value));
    }
    #[cfg(target_os = "windows")]
    let resolver = "where.exe";
    #[cfg(not(target_os = "windows"))]
    let resolver = "which";
    let mut command = std::process::Command::new(resolver);
    #[cfg(target_os = "windows")]
    configure_hidden_console(&mut command);
    let output = command.arg(value).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    #[cfg(target_os = "windows")]
    {
        let is_native = |path: &&PathBuf| {
            matches!(
                path.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_ascii_lowercase())
                    .as_deref(),
                Some("exe" | "com")
            )
        };
        let is_batch = |path: &&PathBuf| {
            matches!(
                path.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.to_ascii_lowercase())
                    .as_deref(),
                Some("cmd" | "bat")
            )
        };
        paths
            .iter()
            .find(is_native)
            .or_else(|| paths.iter().find(is_batch))
            .cloned()
    }
    #[cfg(not(target_os = "windows"))]
    {
        paths.into_iter().next()
    }
}

impl AppState {
    pub fn open(app: &AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("无法定位应用数据目录: {e}"))?;
        fs::create_dir_all(&dir)
            .map_err(|e| format!("无法创建应用数据目录 {}: {e}", dir.display()))?;
        let path = dir.join("dev-project-manager.sqlite3");
        let db = Connection::open(path).map_err(|e| format!("无法打开本地配置数据库: {e}"))?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        migrate(&db)?;
        seed_ides(&db)?;
        Ok(Self { db: Mutex::new(db) })
    }
}

fn migrate(db: &Connection) -> Result<(), String> {
    db.execute_batch(r#"
      PRAGMA foreign_keys = ON;
      CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS projects (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, icon TEXT, color TEXT,
        tags TEXT NOT NULL DEFAULT '[]',
        is_favorite INTEGER NOT NULL DEFAULT 0, sort_order INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS project_modules (
        id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name TEXT NOT NULL, module_type TEXT NOT NULL, description TEXT,
        tags TEXT NOT NULL DEFAULT '[]',
        is_favorite INTEGER NOT NULL DEFAULT 0, sort_order INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS module_paths (
        module_id TEXT NOT NULL REFERENCES project_modules(id) ON DELETE CASCADE,
        platform TEXT NOT NULL, path TEXT NOT NULL, path_kind TEXT NOT NULL,
        validation_status TEXT NOT NULL, last_validated_at INTEGER NOT NULL,
        PRIMARY KEY (module_id, platform)
      );
      CREATE TABLE IF NOT EXISTS module_launch_profiles (
        module_id TEXT NOT NULL REFERENCES project_modules(id) ON DELETE CASCADE,
        platform TEXT NOT NULL, ide_id TEXT NOT NULL, argument_template TEXT NOT NULL,
        PRIMARY KEY (module_id, platform)
      );
      CREATE TABLE IF NOT EXISTS ide_definitions (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, icon TEXT NOT NULL,
        supported_platforms TEXT NOT NULL, supported_path_kinds TEXT NOT NULL,
        default_argument_template TEXT NOT NULL DEFAULT '["{path}"]', built_in INTEGER NOT NULL DEFAULT 1
      );
      CREATE TABLE IF NOT EXISTS ide_installations (
        id TEXT PRIMARY KEY, ide_id TEXT NOT NULL REFERENCES ide_definitions(id) ON DELETE CASCADE,
        platform TEXT NOT NULL, executable_path TEXT NOT NULL, launch_kind TEXT NOT NULL DEFAULT 'executable',
        detected_source TEXT NOT NULL DEFAULT 'manual', status TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1,
        UNIQUE(ide_id, platform, executable_path)
      );
      CREATE TABLE IF NOT EXISTS open_history (
        id TEXT PRIMARY KEY, module_id TEXT NOT NULL REFERENCES project_modules(id) ON DELETE CASCADE,
        ide_id TEXT, opened_at INTEGER NOT NULL, result TEXT NOT NULL, error_message TEXT
      );
      CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS idx_modules_project ON project_modules(project_id, sort_order);
      CREATE INDEX IF NOT EXISTS idx_history_module ON open_history(module_id, opened_at DESC);
      INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (1, strftime('%s','now'));
    "#).map_err(|e| format!("初始化数据库失败: {e}"))?;
    let has_template: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('ide_definitions') WHERE name='default_argument_template')",
        [], |r| r.get(0)).map_err(|e| format!("检查数据库迁移失败: {e}"))?;
    if !has_template {
        db.execute("ALTER TABLE ide_definitions ADD COLUMN default_argument_template TEXT NOT NULL DEFAULT '[\"{path}\"]'", [])
            .map_err(|e| format!("升级数据库失败: {e}"))?;
    }
    db.execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (2, strftime('%s','now'))", [])
        .map_err(|e| format!("记录数据库迁移失败: {e}"))?;
    for table in ["projects", "project_modules"] {
        let has_tags: bool = db
            .query_row(
                &format!(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name='tags')"
                ),
                [],
                |r| r.get(0),
            )
            .map_err(|e| format!("检查标签字段迁移失败: {e}"))?;
        if !has_tags {
            db.execute(
                &format!("ALTER TABLE {table} ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'"),
                [],
            )
            .map_err(|e| format!("升级标签字段失败: {e}"))?;
        }
    }
    db.execute("INSERT OR IGNORE INTO schema_migrations(version, applied_at) VALUES (3, strftime('%s','now'))", [])
        .map_err(|e| format!("记录数据库迁移失败: {e}"))?;
    Ok(())
}

fn seed_ides(db: &Connection) -> Result<(), String> {
    let rows = [
        (
            "rider",
            "JetBrains Rider",
            "rider",
            r#"["windows","macos"]"#,
            r#"["file","directory"]"#,
        ),
        (
            "webstorm",
            "JetBrains WebStorm",
            "webstorm",
            r#"["windows","macos"]"#,
            r#"["file","directory"]"#,
        ),
        (
            "visual-studio",
            "Microsoft Visual Studio",
            "visual-studio",
            r#"["windows"]"#,
            r#"["file","directory"]"#,
        ),
        (
            "vscode",
            "Visual Studio Code",
            "vscode",
            r#"["windows","macos"]"#,
            r#"["file","directory"]"#,
        ),
        (
            "hbuilderx",
            "HBuilderX",
            "hbuilderx",
            r#"["windows","macos"]"#,
            r#"["file","directory"]"#,
        ),
    ];
    for (id, name, icon, platforms, kinds) in rows {
        db.execute("INSERT OR IGNORE INTO ide_definitions(id,name,icon,supported_platforms,supported_path_kinds,default_argument_template,built_in) VALUES (?1,?2,?3,?4,?5,'[\"{path}\"]',1)", params![id,name,icon,platforms,kinds]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn validate_path(value: &str, kind: &str) -> String {
    let p = PathBuf::from(value);
    if value.trim().is_empty() {
        "unconfigured".into()
    } else if !p.exists() {
        "missing".into()
    } else if kind == "file" && !p.is_file() {
        "wrongKind".into()
    } else if kind == "directory" && !p.is_dir() {
        "wrongKind".into()
    } else {
        "valid".into()
    }
}

pub fn get_settings(db: &Connection) -> Result<AppSettings, String> {
    let raw: Option<String> = db
        .query_row(
            "SELECT value FROM app_settings WHERE key='settings'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    raw.map(|s| serde_json::from_str(&s).map_err(|e| format!("设置数据无效: {e}")))
        .transpose()
        .map(|x| x.unwrap_or_default())
}
pub fn put_settings(db: &Connection, settings: &AppSettings) -> Result<(), String> {
    let value = serde_json::to_string(settings).map_err(|e| e.to_string())?;
    db.execute("INSERT INTO app_settings(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{migrate, seed_ides, validate_path};
    use crate::{
        models::{IdeDefinitionInput, IdeInstallationInput, ModuleInput, PathInput, ProjectInput},
        repository, services,
    };
    use rusqlite::Connection;

    #[test]
    fn missing_paths_are_kept_as_missing() {
        assert_eq!(
            validate_path(r#"Z:\definitely-missing\项目.sln"#, "file"),
            "missing"
        );
    }

    #[test]
    fn v3_migration_adds_tags_to_legacy_tables() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            r#"
            CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
            CREATE TABLE projects (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, icon TEXT, color TEXT,
                is_favorite INTEGER NOT NULL DEFAULT 0, sort_order INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
            );
            CREATE TABLE project_modules (
                id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL, module_type TEXT NOT NULL, description TEXT,
                is_favorite INTEGER NOT NULL DEFAULT 0, sort_order INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
            );
            "#,
        )
        .unwrap();
        migrate(&db).unwrap();
        for table in ["projects", "project_modules"] {
            let has_tags: bool = db
                .query_row(
                    &format!(
                        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('{table}') WHERE name='tags')"
                    ),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(has_tags, "{table} should receive tags during v3 migration");
        }
    }

    #[test]
    fn project_module_export_and_import_round_trip() {
        let mut source = Connection::open_in_memory().unwrap();
        migrate(&source).unwrap();
        seed_ides(&source).unwrap();
        let project = repository::save_project(
            &source,
            ProjectInput {
                id: None,
                name: "业务项目".into(),
                description: Some("验收测试".into()),
                icon: None,
                color: Some("#7065e8".into()),
                tags: vec!["验收".into(), "验收".into(), " 核心 ".into()],
                is_favorite: true,
                sort_order: 0,
            },
        )
        .unwrap();
        let current_dir = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        repository::save_module(
            &mut source,
            ModuleInput {
                id: None,
                project_id: project.id,
                name: "Web".into(),
                module_type: "web".into(),
                description: None,
                tags: vec!["前端".into(), "前端".into(), " Web ".into()],
                is_favorite: true,
                sort_order: 0,
                path: Some(PathInput {
                    path: current_dir,
                    path_kind: "directory".into(),
                }),
                ide_id: Some("webstorm".into()),
                argument_template: vec!["{path}".into()],
            },
        )
        .unwrap();

        let bundle = services::export_bundle(&source).unwrap();
        let mut target = Connection::open_in_memory().unwrap();
        migrate(&target).unwrap();
        seed_ides(&target).unwrap();
        let result = services::import_bundle(&mut target, bundle).unwrap();
        assert_eq!(result.projects_imported, 1);
        assert_eq!(result.modules_imported, 1);
        let imported = repository::list_projects(&target).unwrap();
        assert_eq!(imported[0].name, "业务项目");
        assert_eq!(imported[0].modules[0].name, "Web");
        assert_eq!(imported[0].tags, vec!["验收", "核心"]);
        assert_eq!(imported[0].modules[0].tags, vec!["前端", "Web"]);
    }

    #[test]
    fn custom_ide_definition_can_be_saved_and_deleted_but_builtins_cannot() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        seed_ides(&db).unwrap();
        let custom = repository::save_ide_definition(
            &db,
            IdeDefinitionInput {
                id: None,
                name: "我的编辑器".into(),
                icon: "terminal".into(),
                supported_platforms: vec!["windows".into()],
                supported_path_kinds: vec!["directory".into()],
                default_argument_template: vec!["{path}".into(), "--reuse-window".into()],
            },
        )
        .unwrap();
        assert_eq!(
            custom.default_argument_template,
            vec!["{path}".to_string(), "--reuse-window".to_string()]
        );
        repository::save_installation(
            &db,
            IdeInstallationInput {
                id: None,
                ide_id: custom.id.clone(),
                executable_path: "missing-editor-one".into(),
                launch_kind: "command".into(),
                detected_source: "manual".into(),
                enabled: true,
            },
        )
        .unwrap();
        repository::save_installation(
            &db,
            IdeInstallationInput {
                id: None,
                ide_id: custom.id.clone(),
                executable_path: "missing-editor-two".into(),
                launch_kind: "command".into(),
                detected_source: "manual".into(),
                enabled: true,
            },
        )
        .unwrap();
        assert_eq!(
            repository::list_installations(&db)
                .unwrap()
                .into_iter()
                .filter(|item| item.ide_id == custom.id && item.detected_source == "manual")
                .count(),
            1
        );
        assert!(repository::delete_ide_definition(&db, &custom.id).is_ok());
        assert!(repository::delete_ide_definition(&db, "vscode").is_err());
    }
}
