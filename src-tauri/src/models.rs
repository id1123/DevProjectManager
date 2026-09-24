use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const EXPORT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub is_favorite: bool,
    #[serde(default)]
    pub is_pinned: bool,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub modules: Vec<ProjectModule>,
    pub last_opened_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInput {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModule {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub module_type: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub is_favorite: bool,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub path: Option<ModulePath>,
    pub ide_id: Option<String>,
    pub argument_template: Vec<String>,
    #[serde(default)]
    pub linked_module_ids: Vec<String>,
    pub last_opened_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInput {
    pub id: Option<String>,
    pub project_id: String,
    pub name: String,
    pub module_type: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub sort_order: i64,
    pub path: Option<PathInput>,
    pub ide_id: Option<String>,
    #[serde(default)]
    pub argument_template: Vec<String>,
    #[serde(default)]
    pub linked_module_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModulePath {
    pub platform: String,
    pub path: String,
    pub path_kind: String,
    pub validation_status: String,
    pub last_validated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathInput {
    pub path: String,
    pub path_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeDefinition {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub supported_platforms: Vec<String>,
    pub supported_path_kinds: Vec<String>,
    pub built_in: bool,
    pub default_argument_template: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeDefinitionInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default = "default_supported_platforms")]
    pub supported_platforms: Vec<String>,
    #[serde(default = "default_supported_path_kinds")]
    pub supported_path_kinds: Vec<String>,
    #[serde(default = "default_argument_template")]
    pub default_argument_template: Vec<String>,
}

fn default_supported_platforms() -> Vec<String> {
    vec!["windows".into()]
}
fn default_supported_path_kinds() -> Vec<String> {
    vec!["file".into(), "directory".into()]
}
fn default_argument_template() -> Vec<String> {
    vec!["{path}".into()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeInstallation {
    pub id: String,
    pub ide_id: String,
    pub platform: String,
    pub executable_path: String,
    pub launch_kind: String,
    pub detected_source: String,
    pub status: String,
    pub enabled: bool,
    pub ide_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdeInstallationInput {
    pub id: Option<String>,
    pub ide_id: String,
    pub executable_path: String,
    #[serde(default = "default_launch_kind")]
    pub launch_kind: String,
    #[serde(default = "manual_source")]
    pub detected_source: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_launch_kind() -> String {
    "executable".into()
}
fn manual_source() -> String {
    "manual".into()
}
fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub global_shortcut: String,
    pub launcher_width: i64,
    pub launcher_height: i64,
    #[serde(default)]
    pub default_ide_ids: HashMap<String, String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            global_shortcut: "CommandOrControl+Shift+P".into(),
            launcher_width: 720,
            launcher_height: 440,
            default_ide_ids: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub module_id: String,
    pub project_name: String,
    pub module_name: String,
    pub ide_name: String,
    pub path: String,
    pub started: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub projects_imported: usize,
    pub modules_imported: usize,
    pub installations_imported: usize,
    pub missing_paths: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub projects: Vec<Project>,
    pub ide_definitions: Vec<IdeDefinition>,
    pub ide_installations: Vec<IdeInstallation>,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    pub schema_version: u32,
    pub exported_at: i64,
    pub projects: Vec<Project>,
    /// All platform-specific paths and launch profiles. `ProjectModule.path` remains the current-platform convenience value for UI callers.
    #[serde(default)]
    pub module_platform_configs: Vec<ModulePlatformConfig>,
    pub ide_installations: Vec<IdeInstallation>,
    pub settings: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModulePlatformConfig {
    pub module_id: String,
    pub platform: String,
    pub path: Option<ModulePath>,
    pub ide_id: Option<String>,
    #[serde(default)]
    pub argument_template: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::AppSettings;

    #[test]
    fn old_settings_without_default_apps_still_load() {
        let settings: AppSettings = serde_json::from_str(
            r#"{"theme":"system","globalShortcut":"CommandOrControl+Shift+P","launcherWidth":720,"launcherHeight":440}"#,
        )
        .unwrap();
        assert!(settings.default_ide_ids.is_empty());
    }
}
