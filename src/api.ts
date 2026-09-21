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
  dashboard: () => invoke<DashboardData>("get_dashboard"),
  saveProject: (project: ProjectInput) => invoke("save_project", { input: project }),
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
};

export function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "操作失败，请稍后重试";
}
