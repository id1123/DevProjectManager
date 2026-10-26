export type ModuleType = "api" | "web" | "app" | "other";
export type PathKind = "file" | "directory";
export type Platform = "windows" | "macos";

export interface GitStatus { path: string; branch: string | null; changedFiles: number | null; remoteUrl: string | null; error: string | null; detached: boolean }

export interface ProjectModule {
  id: string;
  projectId: string;
  name: string;
  moduleType: ModuleType;
  description?: string | null;
  tags: string[];
  path?: ModulePath | null;
  ideId?: string | null;
  argumentTemplate: string[];
  linkedModuleIds: string[];
  isFavorite: boolean;
  sortOrder: number;
  createdAt?: number;
  updatedAt?: number;
  lastOpenedAt?: number | null;
}

export interface ModulePath {
  platform: Platform;
  path: string;
  pathKind: PathKind;
  validationStatus: "valid" | "missing" | "unchecked";
  lastValidatedAt: number;
}

export interface Project {
  id: string;
  name: string;
  description?: string | null;
  tags: string[];
  color?: string | null;
  icon?: string | null;
  isFavorite: boolean;
  isPinned: boolean;
  sortOrder: number;
  createdAt?: number;
  updatedAt?: number;
  lastOpenedAt?: number | null;
  modules: ProjectModule[];
}

export interface IdeDefinition {
  id: string;
  name: string;
  icon: string;
  supportedPlatforms: Platform[];
  supportedPathKinds: PathKind[];
  defaultArgumentTemplate: string[];
  builtIn: boolean;
}

export interface IdeInstallation {
  id: string;
  ideId: string;
  platform: Platform;
  executablePath: string;
  launchKind: string;
  status: "available" | "installed" | "missing" | "invalid" | "unchecked";
  enabled: boolean;
  detectedSource: "auto" | "manual" | string;
  ideName?: string | null;
}

export interface AppSettings {
  theme: "system" | "light" | "dark";
  globalShortcut: string;
  launcherWidth: number;
  launcherHeight: number;
  defaultIdeIds: Partial<Record<"api" | "web" | "app", string>>;
}

export interface DashboardData {
  projects: Project[];
  ideDefinitions: IdeDefinition[];
  ideInstallations: IdeInstallation[];
  settings: AppSettings;
}

export interface ProjectInput {
  id?: string;
  name: string;
  description?: string;
  tags: string[];
  color?: string;
  icon?: string;
  isFavorite: boolean;
  isPinned: boolean;
  sortOrder: number;
}

export interface ModuleInput {
  id?: string;
  projectId: string;
  name: string;
  moduleType: ModuleType;
  description?: string;
  tags: string[];
  path?: { path: string; pathKind: PathKind } | null;
  ideId?: string | null;
  argumentTemplate: string[];
  linkedModuleIds: string[];
  isFavorite: boolean;
  sortOrder: number;
}

export interface SaveIdeInput {
  ideId: string;
  executablePath: string;
  launchKind: string;
  detectedSource: string;
  enabled: boolean;
}

export interface SaveIdeDefinitionInput {
  id?: string;
  name: string;
  icon: string;
  supportedPlatforms: Platform[];
  supportedPathKinds: PathKind[];
  defaultArgumentTemplate: string[];
}

export interface LauncherItem {
  groupId?: string;
  project: Project;
  module: ProjectModule;
  ide?: IdeDefinition;
}

export interface PathChange { moduleId: string; oldPath: string | null; pathKind: PathKind; newPath: string }
export interface PathRow { change: PathChange; projectId: string; projectName: string; moduleName: string; status: string; message: string }
export interface GroupItem { kind: "module" | "terminal" | "folder" | "url"; target: string }
export interface WorkGroup { id: string; projectId: string; name: string; items: GroupItem[] }
export interface GroupResult { index: number; label: string; status: "ready" | "invalid" | "started" | "failed" | "skipped"; message: string }
