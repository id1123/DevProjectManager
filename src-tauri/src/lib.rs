mod database;
mod models;
mod repository;
mod services;

use database::AppState;
use models::*;
use std::str::FromStr;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent,
};
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, GlobalShortcutExt, Shortcut, ShortcutState,
};

fn with_db<T>(
    state: &State<AppState>,
    operation: impl FnOnce(&mut rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let mut db = state
        .db
        .lock()
        .map_err(|_| "本地配置数据库正被占用".to_string())?;
    operation(&mut db)
}
fn show_launcher(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("launcher") {
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        let _ = window.emit("launcher://shown", ());
    }
}
fn register_shortcut(app: &AppHandle, shortcut_text: &str) -> Result<(), String> {
    let shortcut =
        Shortcut::from_str(shortcut_text).map_err(|e| format!("全局快捷键格式无效：{e}"))?;
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|e| format!("无法更新全局快捷键：{e}"))?;
    manager
        .register(shortcut)
        .map_err(|e| format!("无法注册全局快捷键（可能已被其他程序占用）：{e}"))
}

const TRAY_ID: &str = "main-tray";
const TRAY_OPEN_MAIN: &str = "tray-open-main";
const TRAY_QUIT: &str = "tray-quit";
const TRAY_RECENT_PREFIX: &str = "tray-recent-";
const TRAY_BUSINESS_PREFIX: &str = "tray-business-";
const TRAY_PROJECT_PREFIX: &str = "tray-project-";

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn build_tray_menu(app: &AppHandle) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    let projects = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .db
                .lock()
                .ok()
                .and_then(|db| repository::list_projects(&db).ok())
        })
        .unwrap_or_default();

    let recent = {
        let mut modules = projects
            .iter()
            .flat_map(|project| {
                let project_name = project.name.clone();
                project.modules.iter().filter_map(move |module| {
                    module
                        .last_opened_at
                        .map(|opened_at| (opened_at, module, project_name.clone()))
                })
            })
            .collect::<Vec<_>>();
        modules.sort_by(|a, b| b.0.cmp(&a.0));
        modules.truncate(5);
        modules
    };

    let open_main = MenuItemBuilder::with_id(TRAY_OPEN_MAIN, "打开主窗口")
        .build(app)
        .map_err(|e| e.to_string())?;
    let quit = MenuItemBuilder::with_id(TRAY_QUIT, "退出")
        .build(app)
        .map_err(|e| e.to_string())?;
    let mut builder = MenuBuilder::new(app).item(&open_main);
    for project in projects.iter().filter(|project| project.is_pinned) {
        let mut submenu = SubmenuBuilder::with_id(
            app,
            format!("{TRAY_BUSINESS_PREFIX}{}", project.id),
            format!("📌 {}", project.name),
        );
        for module in &project.modules {
            let label = if module.name.is_empty() {
                "未命名项目"
            } else {
                &module.name
            };
            let item =
                MenuItemBuilder::with_id(format!("{TRAY_PROJECT_PREFIX}{}", module.id), label)
                    .build(app)
                    .map_err(|e| e.to_string())?;
            submenu = submenu.item(&item);
        }
        builder = builder.item(&submenu.build().map_err(|e| e.to_string())?);
    }
    if !recent.is_empty() {
        builder = builder.separator();
        for (_, module, project_name) in recent {
            let label = format!("{} · {}", module.name, project_name);
            let item =
                MenuItemBuilder::with_id(format!("{TRAY_RECENT_PREFIX}{}", module.id), label)
                    .build(app)
                    .map_err(|e| e.to_string())?;
            builder = builder.item(&item);
        }
    }

    if projects.iter().any(|project| !project.is_pinned) {
        builder = builder.separator();
        for project in projects.iter().filter(|project| !project.is_pinned) {
            let mut submenu = SubmenuBuilder::with_id(
                app,
                format!("{TRAY_BUSINESS_PREFIX}{}", project.id),
                &project.name,
            );
            for module in &project.modules {
                let label = if module.name.is_empty() {
                    "未命名项目"
                } else {
                    &module.name
                };
                let item =
                    MenuItemBuilder::with_id(format!("{TRAY_PROJECT_PREFIX}{}", module.id), label)
                        .build(app)
                        .map_err(|e| e.to_string())?;
                submenu = submenu.item(&item);
            }
            let submenu = submenu.build().map_err(|e| e.to_string())?;
            builder = builder.item(&submenu);
        }
    }

    builder
        .separator()
        .item(&quit)
        .build()
        .map_err(|e| e.to_string())
}

fn refresh_tray_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        match build_tray_menu(app) {
            Ok(menu) => {
                if let Err(error) = tray.set_menu(Some(menu)) {
                    eprintln!("托盘菜单刷新失败：{error}");
                }
            }
            Err(error) => eprintln!("托盘菜单构建失败：{error}"),
        }
    }
}

#[tauri::command]
fn get_dashboard(state: State<AppState>) -> Result<Dashboard, String> {
    with_db(&state, |db| {
        services::refresh_module_path_statuses(db)?;
        Ok(Dashboard {
            projects: repository::list_projects(db)?,
            ide_definitions: repository::list_ide_definitions(db)?,
            ide_installations: repository::list_installations(db)?,
            settings: database::get_settings(db)?,
        })
    })
}
#[tauri::command]
fn list_projects(state: State<AppState>) -> Result<Vec<Project>, String> {
    with_db(&state, |db| repository::list_projects(db))
}
#[tauri::command]
fn get_project(id: String, state: State<AppState>) -> Result<Option<Project>, String> {
    with_db(&state, |db| repository::get_project(db, &id))
}
#[tauri::command]
fn save_project(
    input: ProjectInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<Project, String> {
    let project = with_db(&state, |db| repository::save_project(db, input))?;
    refresh_tray_menu(&app);
    Ok(project)
}
#[tauri::command]
fn delete_project(id: String, app: AppHandle, state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| repository::delete_project(db, &id))?;
    refresh_tray_menu(&app);
    Ok(())
}
#[tauri::command]
fn save_module(
    input: ModuleInput,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ProjectModule, String> {
    let module = with_db(&state, |db| repository::save_module(db, input))?;
    refresh_tray_menu(&app);
    Ok(module)
}
#[tauri::command]
fn delete_module(id: String, app: AppHandle, state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| repository::delete_module(db, &id))?;
    refresh_tray_menu(&app);
    Ok(())
}
#[tauri::command]
fn list_ide_definitions(state: State<AppState>) -> Result<Vec<IdeDefinition>, String> {
    with_db(&state, |db| repository::list_ide_definitions(db))
}
#[tauri::command]
fn list_ide_installations(state: State<AppState>) -> Result<Vec<IdeInstallation>, String> {
    with_db(&state, |db| repository::list_installations(db))
}
#[tauri::command]
fn save_ide_definition(
    input: IdeDefinitionInput,
    state: State<AppState>,
) -> Result<IdeDefinition, String> {
    with_db(&state, |db| repository::save_ide_definition(db, input))
}
#[tauri::command]
fn delete_ide_definition(id: String, state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| repository::delete_ide_definition(db, &id))
}
#[tauri::command]
fn save_ide_installation(
    input: IdeInstallationInput,
    state: State<AppState>,
) -> Result<IdeInstallation, String> {
    with_db(&state, |db| repository::save_installation(db, input))
}
#[tauri::command]
fn detect_ides(state: State<AppState>) -> Result<Vec<IdeInstallation>, String> {
    with_db(&state, |db| services::detect_ides(db))
}
#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    with_db(&state, |db| database::get_settings(db))
}
#[tauri::command]
fn save_settings(settings: AppSettings, state: State<AppState>) -> Result<AppSettings, String> {
    with_db(&state, |db| {
        database::put_settings(db, &settings)?;
        Ok(settings)
    })
}
#[tauri::command]
fn set_global_shortcut(
    shortcut: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<AppSettings, String> {
    register_shortcut(&app, &shortcut)?;
    with_db(&state, |db| {
        let mut settings = database::get_settings(db)?;
        settings.global_shortcut = shortcut;
        database::put_settings(db, &settings)?;
        Ok(settings)
    })
}
#[tauri::command]
fn show_launcher_window(app: AppHandle) {
    show_launcher(&app);
}
#[tauri::command]
fn hide_launcher_window(window: WebviewWindow) -> Result<(), String> {
    window.hide().map_err(|e| e.to_string())
}
#[tauri::command]
fn launch_module(
    module_id: String,
    ide_id: Option<String>,
    app: AppHandle,
    state: State<AppState>,
) -> Result<LaunchResult, String> {
    let result = with_db(&state, |db| services::launch_module(db, &module_id, ide_id));
    if result.is_ok() {
        refresh_tray_menu(&app);
    }
    result
}
#[tauri::command]
fn reveal_module_path(module_id: String, state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| services::reveal_module_path(db, &module_id))
}
#[tauri::command]
fn export_config(path: String, state: State<AppState>) -> Result<(), String> {
    with_db(&state, |db| services::write_export_file(db, &path))
}
#[tauri::command]
fn import_config(
    path: String,
    app: AppHandle,
    state: State<AppState>,
) -> Result<ImportResult, String> {
    let result = with_db(&state, |db| services::read_import_file(db, &path))?;
    refresh_tray_menu(&app);
    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Register this first so a second process hands off to the existing
        // process before any other plugin initializes.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        show_launcher(app);
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == TRAY_OPEN_MAIN {
                show_main_window(app);
            } else if id == TRAY_QUIT {
                app.exit(0);
            } else if let Some(module_id) = id
                .strip_prefix(TRAY_RECENT_PREFIX)
                .or_else(|| id.strip_prefix(TRAY_PROJECT_PREFIX))
            {
                let result = app
                    .try_state::<AppState>()
                    .map(|state| with_db(&state, |db| services::launch_module(db, module_id, None)))
                    .unwrap_or_else(|| Err("应用状态尚未初始化".into()));
                if let Err(error) = result {
                    eprintln!("托盘项目启动失败：{error}");
                } else {
                    refresh_tray_menu(app);
                }
            }
        })
        .setup(|app| {
            let state = AppState::open(app.handle()).map_err(std::io::Error::other)?;
            let shortcut = {
                let db = state
                    .db
                    .lock()
                    .map_err(|_| std::io::Error::other("数据库锁被占用"))?;
                if let Err(error) = services::detect_ides(&db) {
                    eprintln!("IDE 自动检测失败：{error}");
                }
                if let Err(error) = services::refresh_module_path_statuses(&db) {
                    eprintln!("项目路径状态刷新失败：{error}");
                }
                database::get_settings(&db)
                    .map_err(std::io::Error::other)?
                    .global_shortcut
            };
            app.manage(state);
            let tray_menu = build_tray_menu(app.handle()).map_err(std::io::Error::other)?;
            let mut tray = TrayIconBuilder::with_id(TRAY_ID)
                .menu(&tray_menu)
                .tooltip("Project Hub")
                .show_menu_on_left_click(true);
            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }
            tray.build(app).map_err(std::io::Error::other)?;
            if let Err(error) = register_shortcut(app.handle(), &shortcut) {
                eprintln!("{error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            list_projects,
            get_project,
            save_project,
            delete_project,
            save_module,
            delete_module,
            list_ide_definitions,
            list_ide_installations,
            save_ide_definition,
            delete_ide_definition,
            save_ide_installation,
            detect_ides,
            get_settings,
            save_settings,
            set_global_shortcut,
            show_launcher_window,
            hide_launcher_window,
            launch_module,
            reveal_module_path,
            export_config,
            import_config
        ])
        .run(tauri::generate_context!())
        .expect("启动本地项目管理器失败");
}
