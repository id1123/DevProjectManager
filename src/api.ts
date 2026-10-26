import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  DashboardData,
  ModuleInput,
  ProjectInput,
  SaveIdeDefinitionInput,
  SaveIdeInput,
} from "./types";

export const api = {
  showPathMaintenance: (projectId: string) => invoke<void>("show_path_maintenance", { projectId }),
  inspectPaths: (projectId?: string) => invoke<import("./types").PathRow[]>("inspect_paths", { projectId: projectId || null }),
  prefixPaths: (projectId: string, oldPrefix: string, newPrefix: string) => invoke<import("./types").PathRow[]>("prefix_paths", { projectId: projectId || null, oldPrefix, newPrefix }),
  previewPaths: (changes: import("./types").PathChange[]) => invoke<import("./types").PathRow[]>("preview_paths", { changes }),
  applyPaths: (changes: import("./types").PathChange[]) => invoke<void>("apply_paths", { changes }),
  workGroups: () => invoke<import("./types").WorkGroup[]>("list_work_groups"),
  saveWorkGroup: (group: import("./types").WorkGroup) => invoke<import("./types").WorkGroup>("save_work_group", { group }),
  deleteWorkGroup: (id: string) => invoke<void>("delete_work_group", { id }),
  checkWorkGroup: (id: string) => invoke<import("./types").GroupResult[]>("check_work_group", { id }),
  runWorkGroup: (id: string, expectedItems: import("./types").GroupItem[], skipInvalid: boolean, indices?: number[]) => invoke<import("./types").GroupResult[]>("run_work_group", { id, expectedItems, skipInvalid, indices: indices ?? null }),
  showWorkGroup: (id: string) => invoke<void>("show_work_group", { id }),
  dashboard: () => invoke<DashboardData>("get_dashboard"),
  saveProject: (project: ProjectInput) => invoke("save_project", { input: project }),
  reorderModules: (projectId: string, moduleIds: string[]) => invoke("reorder_modules", { projectId, moduleIds }),
  deleteProject: (id: string) => invoke("delete_project", { id }),
  saveModule: (module: ModuleInput) => invoke("save_module", { input: module }),
  deleteModule: (id: string) => invoke("delete_module", { id }),
  detectIdes: () => invoke("detect_ides"),
  saveIdeDefinition: (definition: SaveIdeDefinitionInput) =>
    invoke<import("./types").IdeDefinition>("save_ide_definition", { input: definition }),
  deleteIdeDefinition: (id: string) => invoke("delete_ide_definition", { id }),
  saveIde: (installation: SaveIdeInput) =>
    invoke("save_ide_installation", { input: installation }),
  launchModule: (moduleId: string, ideId?: string) =>
    invoke("launch_module", { moduleId, ideId: ideId ?? null }),
  revealModule: (moduleId: string) => invoke("reveal_module_path", { moduleId }),
  saveSettings: (settings: AppSettings) => invoke("save_settings", { settings }),
  setGlobalShortcut: (shortcut: string) =>
    invoke("set_global_shortcut", { shortcut }),
  exportConfig: (path: string) => invoke("export_config", { path }),
  importConfig: (path: string) => invoke("import_config", { path }),
  hideLauncher: () => invoke("hide_launcher_window"),
  gitStatuses: (paths: string[]) => invoke<import("./types").GitStatus[]>("git_statuses", { paths }),
  openProjectTerminal: (moduleId: string) => invoke("open_project_terminal", { moduleId }),
};

export function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "操作失败，请稍后重试";
}
