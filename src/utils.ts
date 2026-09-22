import Fuse from "fuse.js";
import { pinyin } from "pinyin-pro";
import type {
  IdeDefinition,
  IdeInstallation,
  LauncherItem,
  ModuleType,
  PathKind,
  Project,
} from "./types";

export const moduleTypeLabel: Record<ModuleType, string> = {
  api: "API",
  web: "Web",
  app: "App",
  other: "其他",
};

export const moduleTypeTone: Record<ModuleType, string> = {
  api: "violet",
  web: "blue",
  app: "green",
  other: "amber",
};

export const colors = ["#7065e8", "#4f8fea", "#22a06b", "#d97835", "#d05285", "#398a99"];

export function displayPath(path: string) {
  if (!path) return "尚未配置路径";
  return path;
}

export function ideForModule(
  moduleIdeId: string | null | undefined,
  definitions: IdeDefinition[],
) {
  return definitions.find((ide) => ide.id === moduleIdeId);
}

export function ideInstallation(
  ide: IdeDefinition,
  installations: IdeInstallation[],
) {
  return installations
    .filter((item) => item.ideId === ide.id)
    .sort((a, b) => Number(b.detectedSource === "manual") - Number(a.detectedSource === "manual"))[0];
}

export function supportsPath(ide: IdeDefinition, kind: PathKind) {
  return ide.supportedPathKinds.includes(kind);
}

export function relativeTime(value?: string | number | null) {
  if (!value) return "尚未打开";
  const time = typeof value === "number" ? value * 1000 : new Date(value).getTime();
  if (!Number.isFinite(time)) return "尚未打开";
  const seconds = Math.max(0, Math.round((Date.now() - time) / 1000));
  if (seconds < 60) return "刚刚";
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟前`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时前`;
  if (seconds < 604800) return `${Math.floor(seconds / 86400)} 天前`;
  return new Intl.DateTimeFormat("zh-CN", { month: "short", day: "numeric" }).format(new Date(time));
}

export function launcherItems(projects: Project[], definitions: IdeDefinition[]): LauncherItem[] {
  return projects.flatMap((project) =>
    (project.modules ?? []).map((module) => ({
      project,
      module,
      ide: ideForModule(module.ideId, definitions),
    })),
  );
}

export function searchLauncherItems(items: LauncherItem[], query: string) {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return sortLauncherItems(items);

  // A project-name match is decisive. Do not let a shared parent path or IDE
  // pull sibling projects from the same business project into the results.
  const exactNameMatches = items.filter((item) => item.module.name.trim().toLocaleLowerCase() === normalized);
  if (exactNameMatches.length) return sortLauncherItems(exactNameMatches);

  // Search module names in both Chinese and pinyin. Keep the name-match
  // boundary decisive so a matching module is not accompanied by its
  // siblings merely because they share a parent path, IDE, or tags.
  const nameMatches = items.filter((item) => {
    const name = item.module.name.toLocaleLowerCase();
    if (name.includes(normalized)) return true;
    const forms = pinyinNameForms(item.module.name);
    const pinyinQuery = compactLatinQuery(normalized);
    return pinyinQuery.length > 0 && forms.some((form) => form.includes(pinyinQuery));
  });
  if (nameMatches.length) return sortLauncherItems(nameMatches);

  const fuse = new Fuse(items, {
    keys: ["module.tags", "module.description", "module.moduleType", "ide.name", "module.path.path"],
    threshold: 0.24,
    ignoreLocation: true,
  });
  const searchableValues = (item: LauncherItem) => [item.module.description ?? "", item.module.moduleType, item.ide?.name ?? "", item.module.path?.path ?? "", ...item.module.tags].map((value) => value.toLocaleLowerCase());
  const directMatches = items.filter((item) => searchableValues(item).some((value) => value.includes(normalized)));
  return sortLauncherItems(directMatches.length ? directMatches : fuse.search(normalized).map((result) => result.item));
}

/** Return the full and initial pinyin forms used by launcher name search. */
export function pinyinNameForms(name: string) {
  const full = pinyin(name, { toneType: "none" }).replace(/\s+/g, "").toLocaleLowerCase();
  const initials = pinyin(name, { toneType: "none", pattern: "first" }).replace(/\s+/g, "").toLocaleLowerCase();
  return [...new Set([full, initials].filter(Boolean))];
}

function compactLatinQuery(query: string) {
  return query.replace(/[\s-]+/g, "");
}

function sortLauncherItems(items: LauncherItem[]) {
  return [...items].sort((a, b) => {
    const favoriteA = Number(a.module.isFavorite) * 2 + Number(a.project.isFavorite);
    const favoriteB = Number(b.module.isFavorite) * 2 + Number(b.project.isFavorite);
    if (favoriteA !== favoriteB) return favoriteB - favoriteA;
    return (
      Number(b.module.lastOpenedAt ?? b.project.lastOpenedAt ?? 0) -
      Number(a.module.lastOpenedAt ?? a.project.lastOpenedAt ?? 0)
    );
  });
}
