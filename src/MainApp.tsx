import { useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open, save } from "@tauri-apps/plugin-dialog";
import { disable as disableAutostart, enable as enableAutostart, isEnabled as isAutostartEnabled } from "@tauri-apps/plugin-autostart";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import {
  ArchiveRestore,
  Boxes,
  ChevronLeft,
  ChevronRight,
  Clock3,
  Code2,
  Download,
  ExternalLink,
  FileCode2,
  FolderOpen,
  Import,
  Laptop,
  LayoutGrid,
  MoreHorizontal,
  Moon,
  Pencil,
  Pin,
  Plus,
  RefreshCw,
  Rows3,
  Search,
  Settings,
  Star,
  Sun,
  Tags,
  Trash2,
  X,
} from "lucide-react";
import { api, errorMessage } from "./api";
import type {
  AppSettings,
  DashboardData,
  IdeDefinition,
  ModuleInput,
  ModuleType,
  PathKind,
  Project,
  ProjectInput,
  ProjectModule,
  SaveIdeDefinitionInput,
  SaveIdeInput,
} from "./types";
import {
  colors,
  displayPath,
  ideForModule,
  ideInstallation,
  moduleTypeLabel,
  moduleTypeTone,
  relativeTime,
  supportsPath,
} from "./utils";

type View = "all" | "recent";
type SettingsTab = "general" | "ides";
type ToastState = { kind: "success" | "error"; message: string } | null;

const emptyDashboard: DashboardData = {
  projects: [],
  ideDefinitions: [],
  ideInstallations: [],
  settings: { theme: "system", globalShortcut: "CommandOrControl+Shift+P", launcherWidth: 720, launcherHeight: 440 },
};

function normalizeDashboard(value: DashboardData): DashboardData {
  return {
    ...emptyDashboard,
    ...value,
    projects: (value.projects ?? []).map((project) => ({ ...project, isPinned: project.isPinned ?? false, tags: project.tags ?? [], modules: (project.modules ?? []).map((module) => ({ ...module, tags: module.tags ?? [], linkedModuleIds: module.linkedModuleIds ?? [] })) })),
    ideDefinitions: value.ideDefinitions ?? [],
    ideInstallations: value.ideInstallations ?? [],
    settings: { ...emptyDashboard.settings, ...(value.settings ?? {}) },
  };
}

export default function MainApp() {
  const [dashboard, setDashboard] = useState<DashboardData>(emptyDashboard);
  const [view, setView] = useState<View>("all");
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [favoriteOnly, setFavoriteOnly] = useState(false);
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [compactMode, setCompactMode] = useState(() => window.localStorage.getItem("project-hub:list-density") !== "detailed");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsTab, setSettingsTab] = useState<SettingsTab>("general");
  const [loading, setLoading] = useState(true);
  const [projectEditor, setProjectEditor] = useState<Project | "new" | null>(null);
  const [moduleEditor, setModuleEditor] = useState<{ project: Project; module?: ProjectModule } | null>(null);
  const [confirm, setConfirm] = useState<{ title: string; body: string; action: () => Promise<boolean> } | null>(null);
  const [toast, setToast] = useState<ToastState>(null);
  const toastTimer = useRef<number | null>(null);

  const selectedProject = dashboard.projects.find((project) => project.id === selectedProjectId);

  async function refresh(silent = false) {
    if (!silent) setLoading(true);
    try {
      setDashboard(normalizeDashboard(await api.dashboard()));
    } catch (error) {
      notify("error", errorMessage(error));
    } finally {
      if (!silent) setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = dashboard.settings.theme;
  }, [dashboard.settings.theme]);

  useEffect(() => {
    const closeOutsideMenus = (event: PointerEvent) => {
      document.querySelectorAll<HTMLDetailsElement>("details.action-details[open]").forEach((menu) => {
        if (!menu.contains(event.target as Node)) menu.open = false;
      });
    };
    const closeMenusOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") document.querySelectorAll<HTMLDetailsElement>("details.action-details[open]").forEach((menu) => { menu.open = false; });
    };
    document.addEventListener("pointerdown", closeOutsideMenus);
    document.addEventListener("keydown", closeMenusOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOutsideMenus);
      document.removeEventListener("keydown", closeMenusOnEscape);
    };
  }, []);

  function notify(kind: "success" | "error", message: string) {
    if (toastTimer.current !== null) window.clearTimeout(toastTimer.current);
    setToast({ kind, message });
    toastTimer.current = window.setTimeout(() => setToast(null), 3600);
  }

  async function run(action: () => Promise<unknown>, success?: string) {
    try {
      await action();
      if (success) notify("success", success);
      await refresh(true);
      return true;
    } catch (error) {
      notify("error", errorMessage(error));
      return false;
    }
  }

  async function launch(module: ProjectModule, ideId?: string) {
    await run(() => api.launchModule(module.id, ideId), `正在用 ${ideForModule(ideId ?? module.ideId, dashboard.ideDefinitions)?.name ?? "指定 IDE"} 打开`);
  }

  const allTags = useMemo(() => [...new Set(dashboard.projects.flatMap((project) => [...project.tags, ...project.modules.flatMap((module) => module.tags)]))].sort((a, b) => a.localeCompare(b, "zh-CN")), [dashboard.projects]);

  const visibleProjects = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    const hasProjectFilter = favoriteOnly || !!needle || selectedTags.length > 0 || view === "recent";
    const projects = dashboard.projects.flatMap((project) => {
      let modules = [...project.modules];
      if (view === "recent") modules = modules.filter((module) => !!module.lastOpenedAt);
      if (favoriteOnly && !project.isFavorite) modules = modules.filter((module) => module.isFavorite);
      const matchesProject = needle && [project.name, project.description ?? "", ...project.tags].join(" ").toLocaleLowerCase().includes(needle);
      if (needle && !matchesProject) {
        modules = modules.filter((module) => [module.name, module.description ?? "", module.path?.path ?? "", ...module.tags].join(" ").toLocaleLowerCase().includes(needle));
      }
      if (selectedTags.length) {
        modules = modules.filter((module) => {
          const effectiveTags = new Set([...project.tags, ...module.tags]);
          return selectedTags.every((tag) => effectiveTags.has(tag));
        });
      }
      const keepEmptyFavorite = favoriteOnly && project.isFavorite && !needle && selectedTags.length === 0 && view !== "recent";
      if (!modules.length && hasProjectFilter && !keepEmptyFavorite) return [];
      return [{ ...project, modules }];
    });
    if (view === "recent") projects.sort((a, b) => recentTimestamp(b) - recentTimestamp(a));
    return projects;
  }, [dashboard.projects, favoriteOnly, query, selectedTags, view]);

  const visibleModules = useMemo(() => visibleProjects
    .flatMap((project) => project.modules.map((module) => ({ project, module })))
    .sort((a, b) => Number(b.module.lastOpenedAt ?? 0) - Number(a.module.lastOpenedAt ?? 0)), [visibleProjects]);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><Code2 size={21} /></div>
          <div><strong>Project Hub</strong><span>本地开发项目管理器</span></div>
        </div>
        <nav className="nav-list">
          <NavItem active={view === "all" && !selectedProject} icon={<LayoutGrid />} label="全部项目" onClick={() => { setSelectedProjectId(null); setView("all"); }} />
          <NavItem active={view === "recent" && !selectedProject} icon={<Clock3 />} label="最近打开" onClick={() => { setSelectedProjectId(null); setView("recent"); }} />
        </nav>
        <div className="sidebar-tools"><button title="设置" onClick={() => { setSettingsTab("general"); setSettingsOpen(true); }}><Settings size={17} /><span>设置</span></button></div>
      </aside>

      <main className="main-content">
        {loading ? (
          <div className="center-state"><span className="spinner" /><p>正在加载本地配置…</p></div>
        ) : selectedProject ? (
          <ProjectDetail
            project={selectedProject}
            dashboard={dashboard}
            onBack={() => setSelectedProjectId(null)}
            onEditProject={() => setProjectEditor(selectedProject)}
            onAddModule={() => setModuleEditor({ project: selectedProject })}
            onEditModule={(module) => setModuleEditor({ project: selectedProject, module })}
            onLaunch={launch}
            onReveal={(module) => run(() => api.revealModule(module.id))}
            onDeleteModule={(module) => setConfirm({
              title: `删除“${module.name}”配置？`,
              body: "只会删除 Project Hub 中的配置，不会删除磁盘上的代码文件。",
              action: () => run(() => api.deleteModule(module.id), "项目配置已删除"),
            })}
          />
        ) : (
          <>
            <header className="page-header">
              <div><p className="eyebrow">{view === "recent" ? "RECENT" : "WORKSPACE"}</p><h1>{view === "recent" ? "最近打开" : "全部项目"}</h1><p>{view === "recent" ? `${visibleModules.length} 个最近打开的项目` : `${dashboard.projects.length} 个业务项目 · ${dashboard.projects.reduce((sum, item) => sum + item.modules.length, 0)} 个项目`}</p></div>
              {view === "all" ? <button className="primary-button" onClick={() => setProjectEditor("new")}><Plus size={18} />新建业务项目</button> : <button className="secondary-button" onClick={() => setView("all")}><LayoutGrid size={17} />查看全部项目</button>}
            </header>
            <div className="toolbar">
              <label className="search-box"><Search size={18} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索项目、标签或路径" />{query && <button className="icon-button compact" onClick={() => setQuery("")}><X size={15} /></button>}</label>
              {view === "all" && <div className="density-toggle" role="group" aria-label="项目显示模式"><button className={!compactMode ? "active" : ""} aria-pressed={!compactMode} onClick={() => { setCompactMode(false); window.localStorage.setItem("project-hub:list-density", "detailed"); }}><LayoutGrid size={16} />详细</button><button className={compactMode ? "active" : ""} aria-pressed={compactMode} onClick={() => { setCompactMode(true); window.localStorage.setItem("project-hub:list-density", "compact"); }}><Rows3 size={16} />精简</button></div>}
              <button className="secondary-button" onClick={() => void refresh()}><RefreshCw size={17} />刷新列表</button>
            </div>
            <div className="project-filters"><button className={`favorite-filter ${favoriteOnly ? "active" : ""}`} onClick={() => setFavoriteOnly((value) => !value)}><Star size={15} fill={favoriteOnly ? "currentColor" : "none"} />收藏</button><div className="tag-filter-row"><span><Tags size={14} />标签</span>{allTags.length ? allTags.map((tag) => <button key={tag} className={selectedTags.includes(tag) ? "active" : ""} onClick={() => setSelectedTags((values) => values.includes(tag) ? values.filter((value) => value !== tag) : [...values, tag])}><i />{tag}</button>) : <small>添加项目标签后可在这里筛选</small>}</div>{(favoriteOnly || selectedTags.length > 0) && <button className="clear-filter" onClick={() => { setFavoriteOnly(false); setSelectedTags([]); }}>清除筛选</button>}</div>
            {visibleModules.length || (view === "all" && visibleProjects.length) ? (
              view === "recent" ? <RecentProjectList items={visibleModules} definitions={dashboard.ideDefinitions} installations={dashboard.ideInstallations} onLaunch={launch} onEdit={(project, module) => setModuleEditor({ project: dashboard.projects.find((item) => item.id === project.id) ?? project, module })} /> : <section className={`project-grid ${compactMode ? "compact-grid" : ""}`}>
                {visibleProjects.map((project) => {
                  const sourceProject = dashboard.projects.find((item) => item.id === project.id) ?? project;
                  return <ProjectCard key={project.id} project={project} compact={compactMode} definitions={dashboard.ideDefinitions} installations={dashboard.ideInstallations} onOpen={() => setSelectedProjectId(project.id)} onLaunch={launch} onAddModule={() => setModuleEditor({ project: sourceProject })} onEditModule={(module) => setModuleEditor({ project: sourceProject, module })} onEdit={() => setProjectEditor(sourceProject)} onPin={() => void run(() => api.saveProject({ id: sourceProject.id, name: sourceProject.name, description: sourceProject.description ?? undefined, tags: sourceProject.tags, color: sourceProject.color ?? undefined, icon: sourceProject.icon ?? undefined, isFavorite: sourceProject.isFavorite, isPinned: !sourceProject.isPinned, sortOrder: sourceProject.sortOrder }), sourceProject.isPinned ? "已取消置顶" : "已置顶业务项目")} onFavorite={() => void run(() => api.saveProject({ id: sourceProject.id, name: sourceProject.name, description: sourceProject.description ?? undefined, tags: sourceProject.tags, color: sourceProject.color ?? undefined, icon: sourceProject.icon ?? undefined, isFavorite: !sourceProject.isFavorite, isPinned: sourceProject.isPinned, sortOrder: sourceProject.sortOrder }), sourceProject.isFavorite ? "已取消收藏" : "已收藏业务项目")} onDelete={() => setConfirm({
                    title: `删除“${sourceProject.name}”？`,
                    body: `将删除该业务项目及其 ${sourceProject.modules.length} 个项目配置，但不会删除任何真实代码文件。`,
                    action: () => run(() => api.deleteProject(project.id), "业务项目配置已删除"),
                  })} />;
                })}
              </section>
            ) : (
              <div className="empty-state"><div><Boxes size={28} /></div><h2>{query || favoriteOnly || selectedTags.length ? "没有匹配结果" : view === "recent" ? "还没有最近打开的项目" : "还没有业务项目"}</h2><p>{query || favoriteOnly || selectedTags.length ? "换个关键词，或清空筛选条件。" : view === "recent" ? "从全部项目中打开项目后，会显示在这里。" : "先创建一个业务项目，再添加 API、Web 或 App 项目。"}</p>{query || favoriteOnly || selectedTags.length ? <button className="secondary-button" onClick={() => { setQuery(""); setFavoriteOnly(false); setSelectedTags([]); }}>清空筛选</button> : view === "recent" ? <button className="secondary-button" onClick={() => setView("all")}><LayoutGrid size={17} />查看全部项目</button> : <button className="primary-button" onClick={() => setProjectEditor("new")}><Plus size={18} />创建第一个业务项目</button>}</div>
            )}
          </>
        )}
      </main>

      {projectEditor && <ProjectModal project={projectEditor === "new" ? undefined : projectEditor} tagSuggestions={allTags} onClose={() => setProjectEditor(null)} onSave={(input) => run(() => api.saveProject(input), input.id ? "业务项目已更新" : "业务项目已创建").then((saved) => { if (saved) setProjectEditor(null); })} />}
      {moduleEditor && <ModuleModal project={moduleEditor.project} module={moduleEditor.module} definitions={dashboard.ideDefinitions} installations={dashboard.ideInstallations} tagSuggestions={allTags} onClose={() => setModuleEditor(null)} onSave={(input) => run(() => api.saveModule(input), input.id ? "项目已更新" : "项目已添加").then((saved) => { if (saved) setModuleEditor(null); })} />}
      {settingsOpen && <Modal title="设置" subtitle="调整外观、快捷键、IDE 和本地配置。" onClose={() => setSettingsOpen(false)} wide>
        <div className="settings-tabs" role="tablist" aria-label="设置分类"><button role="tab" aria-selected={settingsTab === "general"} className={settingsTab === "general" ? "active" : ""} onClick={() => setSettingsTab("general")}><Settings size={16} />常规设置</button><button role="tab" aria-selected={settingsTab === "ides"} className={settingsTab === "ides" ? "active" : ""} onClick={() => setSettingsTab("ides")}><Laptop size={16} />IDE 管理</button></div>
        <div style={{ display: settingsTab === "general" ? undefined : "none" }}><SettingsPage settings={dashboard.settings} onSave={(settings) => run(async () => { if (settings.globalShortcut !== dashboard.settings.globalShortcut) await api.setGlobalShortcut(settings.globalShortcut); await api.saveSettings(settings); }, "设置已保存")} onImport={async () => { const path = await open({ multiple: false, directory: false, filters: [{ name: "Project Hub 配置", extensions: ["json"] }] }); if (typeof path === "string") await run(() => api.importConfig(path), "配置已导入，失效路径已保留并标记"); }} onExport={async () => { const path = await save({ defaultPath: "project-hub-config.json", filters: [{ name: "Project Hub 配置", extensions: ["json"] }] }); if (path) await run(() => api.exportConfig(path), "配置已导出"); }} /></div>
        <div style={{ display: settingsTab === "ides" ? undefined : "none" }}><IdeManager dashboard={dashboard} onRefresh={() => run(api.detectIdes, "IDE 检测完成")} onSave={(definition, installation) => run(async () => { const saved = await api.saveIdeDefinition(definition); await api.saveIde({ ...installation, ideId: saved.id }); }, definition.id ? "IDE 配置已保存" : "自定义 IDE 已添加")} onDelete={(ide) => setConfirm({ title: `删除“${ide.name}”？`, body: "将删除这个 IDE 的启动配置，已选择它的项目会保留，但需要重新指定启动应用。", action: () => run(() => api.deleteIdeDefinition(ide.id), "自定义 IDE 已删除") })} /></div>
      </Modal>}
      {confirm && <ConfirmModal {...confirm} onClose={() => setConfirm(null)} onConfirm={() => confirm.action().then((saved) => { if (saved) setConfirm(null); })} />}
      {toast && <div className={`toast ${toast.kind}`}>{toast.message}</div>}
    </div>
  );
}

function NavItem({ active, icon, label, onClick }: { active: boolean; icon: React.ReactNode; label: string; onClick: () => void }) {
  return <button className={`nav-item ${active ? "active" : ""}`} onClick={onClick}>{icon}<span>{label}</span></button>;
}

function ProjectCard({ project, compact, definitions, installations, onOpen, onLaunch, onAddModule, onEditModule, onEdit, onPin, onFavorite, onDelete }: { project: Project; compact: boolean; definitions: IdeDefinition[]; installations: DashboardData["ideInstallations"]; onOpen: () => void; onLaunch: (module: ProjectModule) => void; onAddModule: () => void; onEditModule: (module: ProjectModule) => void; onEdit: () => void; onPin: () => void; onFavorite: () => void; onDelete: () => void }) {
  return <article className={`project-card ${compact ? "compact-project" : ""} ${project.isPinned ? "pinned-project" : project.isFavorite ? "favorite-project" : ""}`}>
    <header className="project-card-header" role="button" tabIndex={0} onClick={onOpen} onKeyDown={(event) => { if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); onOpen(); } }}>
      <div className="project-summary"><div className="project-avatar" style={{ background: project.color ?? colors[0] }}>{project.name.slice(0, 1).toUpperCase()}</div><div><div className="project-name-line"><h2>{project.name}</h2><button className={`project-status-toggle pin-status ${project.isPinned ? "active" : ""}`} title={project.isPinned ? "取消置顶" : "置顶业务项目"} aria-label={project.isPinned ? "取消置顶" : "置顶业务项目"} aria-pressed={project.isPinned} onClick={(event) => { event.stopPropagation(); onPin(); }}><Pin size={15} fill={project.isPinned ? "currentColor" : "none"} /></button><button className={`project-status-toggle favorite-status ${project.isFavorite ? "active" : ""}`} title={project.isFavorite ? "取消收藏" : "收藏业务项目"} aria-label={project.isFavorite ? "取消收藏" : "收藏业务项目"} aria-pressed={project.isFavorite} onClick={(event) => { event.stopPropagation(); onFavorite(); }}><Star size={15} fill={project.isFavorite ? "currentColor" : "none"} /></button></div><p>{project.description || "未填写业务项目说明"}</p>{project.tags.length > 0 && <div className="entity-tags">{project.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}</div></div>
      <div className="project-card-actions"><span>{project.modules.length} 个项目</span><button className="project-inline-add" title="添加项目" onClick={(event) => { event.stopPropagation(); onAddModule(); }}><Plus size={15} />添加项目</button><details className="card-action-details action-details" onClick={(event) => event.stopPropagation()}><summary aria-label="更多业务项目操作" title="更多操作"><MoreHorizontal size={17} /></summary><div className="action-menu" onClick={(event) => { event.currentTarget.closest("details")!.open = false; }}><button onClick={onEdit}>编辑业务项目</button><hr /><button className="danger-text" onClick={onDelete}>删除业务项目配置</button></div></details><ChevronRight size={19} /></div>
    </header>
    <div className={`card-module-list ${compact ? "compact-list" : ""}`}>{project.modules.map((module) => {
      const ide = ideForModule(module.ideId, definitions);
      const installation = ide ? ideInstallation(ide, installations) : undefined;
      const ready = module.path?.validationStatus === "valid" && !!ide && !!installation && ["available", "installed"].includes(installation.status);
      if (compact) return <article key={module.id} className={`compact-module-card type-${module.moduleType}`} role="button" tabIndex={0} aria-label={`打开项目 ${module.name}`} title={`打开${module.name}`} onClick={() => onLaunch(module)} onKeyDown={(event) => { if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); void onLaunch(module); } }}><strong title={module.name}>{module.name}</strong>{module.linkedModuleIds.length > 0 && <span className="linked-badge">联动 {module.linkedModuleIds.length}</span>}<button className="icon-button compact" title="编辑项目" aria-label={`编辑${module.name}`} onClick={(event) => { event.stopPropagation(); onEditModule(module); }}><Pencil size={13} /></button><ExternalLink size={15} className="compact-open-icon" aria-hidden="true" /></article>;
      return <article key={module.id} className="card-module-card" role="button" tabIndex={0} title={`用 ${ide?.name ?? "默认 IDE"} 打开`} onClick={() => onLaunch(module)} onKeyDown={(event) => { if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); void onLaunch(module); } }}>
        <div className="card-module-card-top"><span className={`module-icon ${moduleTypeTone[module.moduleType]}`}><FileCode2 size={19} /></span><span className={`type-badge ${moduleTypeTone[module.moduleType]}`}>{moduleTypeLabel[module.moduleType]}</span>{module.isFavorite && <Star size={13} fill="currentColor" />}{module.linkedModuleIds.length > 0 && <span className="linked-badge">联动 {module.linkedModuleIds.length}</span>}<button className="icon-button" title="编辑项目" onClick={(event) => { event.stopPropagation(); onEditModule(module); }}><Pencil size={14} /></button></div>
        <div className="card-module-title"><strong>{module.name}</strong></div>
        <span className="card-module-description">{module.description || "未填写项目说明"}</span>
        {module.tags.length > 0 && <div className="entity-tags compact">{module.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}
        <span className="card-module-path">{displayPath(module.path?.path ?? "")}</span>
        <div className="card-module-card-footer"><span className={ready ? "status-dot ok" : "status-dot warning"}>{ready ? "可启动" : "需检查"}</span><span><Code2 size={13} />{ide?.name ?? "未指定 IDE"}</span><span className="card-open-hint"><ExternalLink size={14} />打开</span></div>
      </article>;
    })}{!compact && project.modules.length === 0 && <div className="card-empty-modules"><FileCode2 size={22} /><span><strong>尚未添加项目</strong><small>使用标题栏中的“添加项目”开始配置。</small></span></div>}{compact && project.modules.length === 0 && <span className="compact-empty">尚未添加项目</span>}</div>
    {!compact && <footer className="card-footer"><span><Clock3 size={14} />最近打开 {relativeTime(project.lastOpenedAt)}</span></footer>}
  </article>;
}

function RecentProjectList({ items, definitions, installations, onLaunch, onEdit }: { items: { project: Project; module: ProjectModule }[]; definitions: IdeDefinition[]; installations: DashboardData["ideInstallations"]; onLaunch: (module: ProjectModule) => void; onEdit: (project: Project, module: ProjectModule) => void }) {
  return <section className="recent-project-list"><div className="recent-list-header" aria-hidden="true"><span /><span>项目信息</span><span>所属业务项目 / 时间</span><span>打开方式</span><span>操作</span></div>{items.map(({ project, module }) => {
    const ide = ideForModule(module.ideId, definitions);
    const installation = ide ? ideInstallation(ide, installations) : undefined;
    const ready = module.path?.validationStatus === "valid" && !!ide && !!installation && ["available", "installed"].includes(installation.status);
    return <article key={module.id} className="recent-project-row" role="button" tabIndex={0} onClick={() => onLaunch(module)} onKeyDown={(event) => { if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); onLaunch(module); } }}>
      <span className={`module-icon ${moduleTypeTone[module.moduleType]}`}><FileCode2 size={21} /></span>
      <div className="recent-project-main"><div className="module-name"><h3>{module.name}</h3><span className={`type-badge ${moduleTypeTone[module.moduleType]}`}>{moduleTypeLabel[module.moduleType]}</span>{module.isFavorite && <Star size={14} fill="currentColor" />}</div><p>{module.description || displayPath(module.path?.path ?? "")}</p>{module.tags.length > 0 && <div className="entity-tags compact">{module.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}</div>
      <div className="recent-project-context"><span>{project.name}</span><small><Clock3 size={13} />{relativeTime(module.lastOpenedAt)}</small></div>
      <span className={ready ? "status-dot ok" : "status-dot warning"}>{ready ? ide?.name : "需检查"}</span>
      <button className="icon-button" title="编辑项目" onClick={(event) => { event.stopPropagation(); onEdit(project, module); }}><Pencil size={15} /></button>
      <span className="recent-project-open"><ExternalLink size={14} />打开</span>
    </article>;
  })}</section>;
}

function ProjectDetail({ project, dashboard, onBack, onEditProject, onAddModule, onEditModule, onLaunch, onReveal, onDeleteModule }: { project: Project; dashboard: DashboardData; onBack: () => void; onEditProject: () => void; onAddModule: () => void; onEditModule: (module: ProjectModule) => void; onLaunch: (module: ProjectModule, ideId?: string) => void; onReveal: (module: ProjectModule) => void; onDeleteModule: (module: ProjectModule) => void }) {
  return <>
    <header className="detail-header"><button className="back-button" onClick={onBack}><ChevronLeft size={20} />全部项目</button><div className="detail-title"><div className="project-avatar large" style={{ background: project.color ?? colors[0] }}>{project.name.slice(0, 1)}</div><div><p className="eyebrow">BUSINESS PROJECT</p><h1>{project.name}</h1><p>{project.description || "暂无业务项目说明"}</p>{project.tags.length > 0 && <div className="entity-tags">{project.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}</div></div><div className="header-actions"><button className="secondary-button" onClick={onEditProject}><Pencil size={17} />编辑业务项目</button><button className="primary-button" onClick={onAddModule}><Plus size={18} />添加项目</button></div></header>
    <div className="section-heading"><div><h2>项目</h2><p>{project.modules.length} 个已配置项目</p></div></div>
    <section className="module-list">{project.modules.length ? project.modules.map((module) => {
      const ide = ideForModule(module.ideId, dashboard.ideDefinitions);
      const installation = ide ? ideInstallation(ide, dashboard.ideInstallations) : undefined;
      const invalid = !!module.path && module.path.validationStatus !== "valid";
      return <article className="module-row" key={module.id}>
        <div className={`module-icon ${moduleTypeTone[module.moduleType]}`}><FileCode2 size={22} /></div>
        <div className="module-main"><div className="module-name"><h3>{module.name}</h3><span className={`type-badge ${moduleTypeTone[module.moduleType]}`}>{moduleTypeLabel[module.moduleType]}</span>{module.isFavorite && <Star size={14} fill="currentColor" />}</div>{module.tags.length > 0 && <div className="entity-tags compact">{module.tags.map((tag) => <span key={tag}>{tag}</span>)}</div>}<p className={invalid ? "path invalid" : "path"}>{displayPath(module.path?.path ?? "")}{invalid && <span> · 路径不存在</span>}</p><span className="module-meta">{ide?.name ?? "未指定 IDE"} · {relativeTime(module.lastOpenedAt)}{module.linkedModuleIds.length > 0 && ` · 联动 ${module.linkedModuleIds.length} 个项目`}</span></div>
        <div className="module-actions"><button className="secondary-button small" onClick={() => onReveal(module)}><FolderOpen size={16} />打开目录</button><div className="split-action"><button className="secondary-button module-launch-button small" disabled={invalid || !ide || !installation || !["available", "installed"].includes(installation.status)} onClick={() => onLaunch(module)}><ExternalLink size={16} />用 {ide?.name ?? "IDE"} 打开</button><details className="action-details"><summary aria-label="更多项目操作" title="更多操作"><MoreHorizontal size={17} /></summary><div className="action-menu" onClick={(event) => { event.currentTarget.closest("details")!.open = false; }}><span className="action-menu-label">打开方式</span><button className="reveal-menu-action" onClick={() => onReveal(module)}>打开项目目录</button>{dashboard.ideDefinitions.filter((item) => supportsPath(item, module.path?.pathKind ?? "directory")).map((item) => <button key={item.id} onClick={() => onLaunch(module, item.id)}>临时使用 {item.name}</button>)}<hr /><span className="action-menu-label">配置</span><button onClick={() => onEditModule(module)}>编辑项目配置</button><button className="danger-text" onClick={() => onDeleteModule(module)}>删除项目配置</button></div></details></div></div>
      </article>;
    }) : <div className="empty-inline"><FileCode2 size={25} /><div><h3>尚未添加项目</h3><p>添加 API、Web、App 或其他类型的本地路径。</p></div><button className="primary-button" onClick={onAddModule}><Plus size={17} />添加项目</button></div>}</section>
  </>;
}

function IdeManager({ dashboard, onRefresh, onSave, onDelete }: { dashboard: DashboardData; onRefresh: () => void; onSave: (definition: SaveIdeDefinitionInput, installation: Omit<SaveIdeInput, "ideId">) => Promise<boolean>; onDelete: (ide: IdeDefinition) => void }) {
  const [editing, setEditing] = useState<IdeDefinition | "new" | null>(null);
  return <>
    <div className="tool-panel-header"><p>可以添加任意编辑器或开发工具，也支持直接使用 PATH 中的命令启动项目。</p><div className="header-actions"><button className="secondary-button" onClick={onRefresh}><RefreshCw size={17} />重新检测</button><button className="primary-button" onClick={() => setEditing("new")}><Plus size={18} />添加 IDE</button></div></div>
    <section className="settings-card ide-grid">{dashboard.ideDefinitions.map((ide) => {
      const item = ideInstallation(ide, dashboard.ideInstallations);
      const installed = item?.status === "available" || item?.status === "installed";
      return <div className="ide-row" key={ide.id}><div className="ide-logo"><Code2 /></div><div><div className="ide-title-line"><h3>{ide.name}</h3>{!ide.builtIn && <span>自定义</span>}</div><p>{item?.executablePath || "尚未配置启动命令"}</p><div className="support-tags">{item?.launchKind === "command" && <span>PATH 命令</span>}{ide.supportedPathKinds.includes("file") && <span>文件</span>}{ide.supportedPathKinds.includes("directory") && <span>文件夹</span>}</div></div><span className={`status-dot ${installed ? "ok" : "warning"}`}>{installed ? "可用" : "需配置"}</span><div className="ide-actions"><button className="secondary-button small" onClick={() => setEditing(ide)}>配置</button>{!ide.builtIn && <button className="icon-button danger" title="删除自定义 IDE" onClick={() => onDelete(ide)}><Trash2 size={15} /></button>}</div></div>;
    })}</section>
    {editing && <IdeEditor ide={editing === "new" ? undefined : editing} installation={editing === "new" ? undefined : ideInstallation(editing, dashboard.ideInstallations)} onClose={() => setEditing(null)} onSave={async (definition, installation) => { if (await onSave(definition, installation)) setEditing(null); }} />}
  </>;
}

function IdeEditor({ ide, installation, onClose, onSave }: { ide?: IdeDefinition; installation?: DashboardData["ideInstallations"][number]; onClose: () => void; onSave: (definition: SaveIdeDefinitionInput, installation: Omit<SaveIdeInput, "ideId">) => Promise<void> }) {
  const fallbackCommand = ide?.id === "vscode" ? "code" : "";
  const [name, setName] = useState(ide?.name ?? "");
  const [command, setCommand] = useState(installation?.executablePath ?? fallbackCommand);
  const [launchKind, setLaunchKind] = useState(installation?.launchKind ?? (ide?.id === "vscode" ? "command" : "command"));
  const [supportsFile, setSupportsFile] = useState(ide?.supportedPathKinds.includes("file") ?? true);
  const [supportsDirectory, setSupportsDirectory] = useState(ide?.supportedPathKinds.includes("directory") ?? true);
  const [argumentsText, setArgumentsText] = useState((ide?.defaultArgumentTemplate ?? ["{path}"]).join("\n"));
  const [saving, setSaving] = useState(false);
  async function pickExecutable() {
    const selected = await open({ multiple: false, directory: launchKind === "macosApp" });
    if (typeof selected === "string") setCommand(selected);
  }
  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setSaving(true);
    try {
      const defaultArgumentTemplate = argumentsText.split(/\r?\n/).map((value) => value.trim()).filter(Boolean);
      await onSave({ id: ide?.id, name: name.trim(), icon: ide?.icon ?? "code", supportedPlatforms: ide?.supportedPlatforms ?? [currentPlatform()], supportedPathKinds: [...(supportsFile ? ["file" as const] : []), ...(supportsDirectory ? ["directory" as const] : [])], defaultArgumentTemplate: defaultArgumentTemplate.length ? defaultArgumentTemplate : ["{path}"] }, { executablePath: command.trim(), launchKind, detectedSource: "manual", enabled: true });
    } finally {
      setSaving(false);
    }
  }
  return <Modal title={ide ? `配置 ${ide.name}` : "添加自定义 IDE"} subtitle="命令模式会从系统 PATH 查找程序；参数模板中的 {path} 会替换为项目路径。" onClose={onClose} wide><form className="form-stack" onSubmit={submit}><label>显示名称<input autoFocus required value={name} onChange={(event) => setName(event.target.value)} placeholder="输入 IDE 名称" /></label><label>启动方式<select value={launchKind} onChange={(event) => setLaunchKind(event.target.value)}><option value="command">系统命令（推荐）</option><option value="executable">可执行文件</option>{currentPlatform() === "macos" && <option value="macosApp">macOS 应用</option>}</select></label><label>{launchKind === "command" ? "启动命令" : "程序位置"}<div className="path-picker"><input required value={command} onChange={(event) => setCommand(event.target.value)} placeholder={launchKind === "command" ? "输入系统命令" : "选择程序文件"} />{launchKind !== "command" && <button type="button" className="secondary-button" onClick={pickExecutable}><FolderOpen size={17} />选择</button>}</div><small className="field-help">命令模式可直接填写 code、idea 等已加入 PATH 的命令，无需填写完整安装路径。</small></label><label>默认参数模板<textarea value={argumentsText} onChange={(event) => setArgumentsText(event.target.value)} placeholder="{path}" /><small className="field-help">每行一个参数，使用 {"{path}"} 表示项目路径。</small></label><div className="path-kind-options"><span>支持的路径</span><label><input type="checkbox" checked={supportsFile} onChange={(event) => setSupportsFile(event.target.checked)} />项目文件</label><label><input type="checkbox" checked={supportsDirectory} onChange={(event) => setSupportsDirectory(event.target.checked)} />项目文件夹</label></div><div className="modal-actions"><button type="button" className="secondary-button" onClick={onClose}>取消</button><button className="primary-button" disabled={saving || !name.trim() || !command.trim() || (!supportsFile && !supportsDirectory)}>{saving ? "正在保存…" : ide ? "保存配置" : "添加 IDE"}</button></div></form></Modal>;
}

function SettingsPage({ settings, onSave, onImport, onExport }: { settings: AppSettings; onSave: (settings: AppSettings) => void; onImport: () => void; onExport: () => void }) {
  const [draft, setDraft] = useState(settings);
  const [appVersion, setAppVersion] = useState("-");
  const [availableUpdate, setAvailableUpdate] = useState<Awaited<ReturnType<typeof check>>>(null);
  const [updateStatus, setUpdateStatus] = useState("点击检查是否有可用的新版本。");
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateProgress, setUpdateProgress] = useState<number | null>(null);
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [autostartBusy, setAutostartBusy] = useState(true);
  const [autostartError, setAutostartError] = useState("");
  useEffect(() => setDraft(settings), [settings]);
  useEffect(() => { void getVersion().then(setAppVersion).catch(() => setAppVersion("未知")); }, []);
  useEffect(() => {
    void isAutostartEnabled().then(setAutostartEnabled).catch((error) => setAutostartError(`读取开机自启状态失败：${errorMessage(error)}`)).finally(() => setAutostartBusy(false));
  }, []);

  async function setAutostart(enabled: boolean) {
    setAutostartBusy(true);
    setAutostartError("");
    try {
      if (enabled) await enableAutostart();
      else await disableAutostart();
      setAutostartEnabled(await isAutostartEnabled());
    } catch (error) {
      setAutostartError(`设置开机自启失败：${errorMessage(error)}`);
    } finally {
      setAutostartBusy(false);
    }
  }

  async function checkForUpdate() {
    setUpdateBusy(true);
    setUpdateProgress(null);
    setUpdateStatus("正在从 GitHub Releases 检查更新…");
    try {
      const update = await check();
      setAvailableUpdate(update);
      setUpdateStatus(update ? `发现新版本 ${update.version}${update.body ? `：${update.body}` : ""}` : "当前已经是最新版本。");
    } catch (error) {
      setAvailableUpdate(null);
      setUpdateStatus(`检查失败：${errorMessage(error)}`);
    } finally {
      setUpdateBusy(false);
    }
  }

  async function installUpdate() {
    if (!availableUpdate) return;
    setUpdateBusy(true);
    setUpdateProgress(0);
    setUpdateStatus("正在下载更新…");
    let downloaded = 0;
    let total = 0;
    try {
      await availableUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setUpdateProgress(total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : null);
        }
        if (event.event === "Finished") {
          setUpdateProgress(100);
          setUpdateStatus("更新已安装，正在重新启动…");
        }
      });
      await relaunch();
    } catch (error) {
      setUpdateStatus(`更新失败：${errorMessage(error)}`);
      setUpdateBusy(false);
    }
  }

  return <div className="settings-stack"><section className="settings-card"><div className="settings-title"><div><h2>外观</h2><p>选择适合当前工作环境的主题。</p></div></div><div className="theme-options"><ThemeOption active={draft.theme === "system"} icon={<Laptop />} label="跟随系统" onClick={() => setDraft({ ...draft, theme: "system" })} /><ThemeOption active={draft.theme === "light"} icon={<Sun />} label="浅色" onClick={() => setDraft({ ...draft, theme: "light" })} /><ThemeOption active={draft.theme === "dark"} icon={<Moon />} label="深色" onClick={() => setDraft({ ...draft, theme: "dark" })} /></div></section><section className="settings-card"><div className="settings-title"><div><h2>快速启动</h2><p>该快捷键在应用后台运行时也可以唤起搜索窗口。</p></div></div><label className="setting-field"><span>全局快捷键</span><input value={draft.globalShortcut} onChange={(event) => setDraft({ ...draft, globalShortcut: event.target.value })} placeholder="CommandOrControl+Shift+P" /></label></section><section className="settings-card"><div className="settings-title"><div><h2>开机启动</h2><p>登录系统后自动运行 Project Hub。</p></div></div><label className="autostart-option"><input type="checkbox" checked={autostartEnabled} disabled={autostartBusy} onChange={(event) => void setAutostart(event.target.checked)} /><span>开机自启</span></label>{autostartError && <p className="setting-error" role="alert">{autostartError}</p>}</section><section className="settings-card"><div className="settings-title"><div><h2>软件更新</h2><p>当前版本 {appVersion}，更新包从 GitHub Releases 获取并校验签名。</p></div></div><div className="update-row"><div><strong>{updateStatus}</strong>{updateProgress !== null && <div className="update-progress"><i style={{ width: `${updateProgress}%` }} /></div>}</div>{availableUpdate ? <button className="primary-button" disabled={updateBusy} onClick={() => void installUpdate()}><Download size={17} />{updateBusy ? `下载中${updateProgress !== null ? ` ${updateProgress}%` : "…"}` : `更新到 ${availableUpdate.version}`}</button> : <button className="secondary-button" disabled={updateBusy} onClick={() => void checkForUpdate()}><RefreshCw size={17} />{updateBusy ? "检查中…" : "检查更新"}</button>}</div></section><section className="settings-card"><div className="settings-title"><div><h2>配置备份</h2><p>JSON 文件包含项目结构和平台配置，不包含任何真实代码文件。</p></div></div><div className="backup-actions"><button className="secondary-button" onClick={onImport}><Import size={17} />导入配置</button><button className="secondary-button" onClick={onExport}><ArchiveRestore size={17} />导出配置</button></div></section><div className="save-row"><button className="primary-button" onClick={() => onSave(draft)}>保存设置</button></div></div>;
}

function ThemeOption({ active, icon, label, onClick }: { active: boolean; icon: React.ReactNode; label: string; onClick: () => void }) { return <button className={`theme-option ${active ? "active" : ""}`} onClick={onClick}>{icon}<span>{label}</span></button>; }

function TagEditor({ value, suggestions, onChange }: { value: string[]; suggestions: string[]; onChange: (value: string[]) => void }) {
  const [query, setQuery] = useState("");
  const [focused, setFocused] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const available = suggestions.filter((tag) => !value.some((selected) => selected.toLocaleLowerCase() === tag.toLocaleLowerCase()) && tag.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())).slice(0, 8);
  function addTag(raw: string) {
    const trimmed = raw.trim();
    if (!trimmed) return;
    const canonical = suggestions.find((tag) => tag.toLocaleLowerCase() === trimmed.toLocaleLowerCase()) ?? trimmed;
    if (!value.some((tag) => tag.toLocaleLowerCase() === canonical.toLocaleLowerCase())) onChange([...value, canonical]);
    setQuery("");
    inputRef.current?.focus();
  }
  return <div className="form-field"><span className="form-field-label">标签（可选）</span><div className={`tag-editor ${focused ? "focused" : ""}`} onClick={() => inputRef.current?.focus()}>{value.map((tag) => <span className="tag-token" key={tag}>{tag}<button type="button" title={`移除 ${tag}`} onClick={(event) => { event.stopPropagation(); onChange(value.filter((item) => item !== tag)); }}><X size={12} /></button></span>)}<input ref={inputRef} value={query} onFocus={() => setFocused(true)} onBlur={() => window.setTimeout(() => setFocused(false), 120)} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); addTag(query); } if (event.key === "Backspace" && !query && value.length) onChange(value.slice(0, -1)); }} placeholder={value.length ? "搜索或创建标签" : "输入搜索，回车创建标签"} /></div>{focused && (query.trim() || available.length > 0) && <div className="tag-suggestions">{available.map((tag) => <button type="button" key={tag} onMouseDown={(event) => event.preventDefault()} onClick={() => addTag(tag)}>{tag}</button>)}{query.trim() && !suggestions.some((tag) => tag.toLocaleLowerCase() === query.trim().toLocaleLowerCase()) && <button type="button" className="create-tag" onMouseDown={(event) => event.preventDefault()} onClick={() => addTag(query)}><Plus size={13} />创建“{query.trim()}”</button>}</div>}</div>;
}

function ProjectModal({ project, tagSuggestions, onClose, onSave }: { project?: Project; tagSuggestions: string[]; onClose: () => void; onSave: (value: ProjectInput) => void }) {
  const [name, setName] = useState(project?.name ?? ""); const [description, setDescription] = useState(project?.description ?? ""); const [tags, setTags] = useState(project?.tags ?? []); const [color, setColor] = useState(project?.color ?? colors[0]);
  const colorNames = ["紫色", "蓝色", "绿色", "橙色", "粉色", "青色"];
  return <Modal title={project ? "编辑业务项目" : "新建业务项目"} subtitle="业务项目用于组织多个项目，本身不对应代码目录。" onClose={onClose}><form className="form-stack" onSubmit={(event) => { event.preventDefault(); onSave({ id: project?.id, name: name.trim(), description: description.trim(), tags, color, isFavorite: project?.isFavorite ?? false, isPinned: project?.isPinned ?? false, sortOrder: project?.sortOrder ?? 0 }); }}><label>业务项目名称<input autoFocus required value={name} onChange={(event) => setName(event.target.value)} placeholder="输入业务项目名称" /></label><label>业务项目说明（可选）<textarea value={description ?? ""} onChange={(event) => setDescription(event.target.value)} placeholder="补充业务项目说明" /></label><TagEditor value={tags} suggestions={tagSuggestions} onChange={setTags} /><label>业务项目颜色<div className="color-picker">{colors.map((item, index) => <button type="button" aria-label={`选择${colorNames[index]}`} title={colorNames[index]} key={item} className={color === item ? "selected" : ""} style={{ background: item }} onClick={() => setColor(item)} />)}</div></label><div className="modal-actions"><button type="button" className="secondary-button" onClick={onClose}>取消</button><button className="primary-button" disabled={!name.trim()}>{project ? "保存更改" : "创建业务项目"}</button></div></form></Modal>;
}

function ModuleModal({ project, module, definitions, installations, tagSuggestions, onClose, onSave }: { project: Project; module?: ProjectModule; definitions: IdeDefinition[]; installations: DashboardData["ideInstallations"]; tagSuggestions: string[]; onClose: () => void; onSave: (value: ModuleInput) => void }) {
  const [name, setName] = useState(module?.name ?? ""); const [moduleType, setModuleType] = useState<ModuleType>(module?.moduleType ?? "api"); const [pathKind, setPathKind] = useState<PathKind>(module?.path?.pathKind ?? (moduleType === "api" ? "file" : "directory")); const [path, setPath] = useState(module?.path?.path ?? ""); const [ideId, setIdeId] = useState(module?.ideId ?? ""); const [description, setDescription] = useState(module?.description ?? ""); const [tags, setTags] = useState(module?.tags ?? [module?.moduleType ?? "api"]); const [favorite, setFavorite] = useState(module?.isFavorite ?? false); const [linkedModuleIds, setLinkedModuleIds] = useState(module?.linkedModuleIds ?? []);
  const availableIdes = definitions.filter((ide) => ide.supportedPlatforms.includes(currentPlatform()) && supportsPath(ide, pathKind));
  async function pickPath() { const selected = await open({ multiple: false, directory: pathKind === "directory", filters: pathKind === "file" && moduleType === "api" ? [{ name: "解决方案文件", extensions: ["sln"] }] : undefined }); if (typeof selected === "string") setPath(selected); }
  return <Modal title={module ? "编辑项目" : `给“${project.name}”添加项目`} subtitle="文件和文件夹会按原始路径保存，不会复制或修改代码。" onClose={onClose} wide><form className="form-stack module-form" onSubmit={(event) => { event.preventDefault(); const selectedIde = definitions.find((ide) => ide.id === ideId); const argumentTemplate = module?.ideId === ideId ? module.argumentTemplate : selectedIde?.defaultArgumentTemplate ?? ["{path}"]; onSave({ id: module?.id, projectId: project.id, name: name.trim(), moduleType, description: description?.trim(), tags, path: { path: path.trim(), pathKind }, ideId: ideId || null, argumentTemplate, linkedModuleIds, isFavorite: favorite, sortOrder: module?.sortOrder ?? project.modules.length }); }}><div className="form-grid"><label>项目名称<input autoFocus required value={name} onChange={(event) => setName(event.target.value)} placeholder="输入项目名称" /></label><label>类型<select value={moduleType} onChange={(event) => { const value = event.target.value as ModuleType; if (!module) setTags((current) => { const withoutPreviousType = current.filter((tag) => tag.toLocaleLowerCase() !== moduleType); return withoutPreviousType.some((tag) => tag.toLocaleLowerCase() === value) ? withoutPreviousType : [...withoutPreviousType, value]; }); setModuleType(value); if (!module) setPathKind(value === "api" ? "file" : "directory"); }}>{Object.entries(moduleTypeLabel).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label></div><div className="path-config-grid"><label>路径类型<div className="segmented"><button type="button" className={pathKind === "file" ? "active" : ""} onClick={() => { setPathKind("file"); setIdeId(""); }}>文件</button><button type="button" className={pathKind === "directory" ? "active" : ""} onClick={() => { setPathKind("directory"); setIdeId(""); }}>文件夹</button></div></label><label>本地路径<div className="path-picker"><input required value={path} onChange={(event) => setPath(event.target.value)} placeholder={pathKind === "file" ? "选择项目文件" : "选择项目根目录"} /><button type="button" className="secondary-button" onClick={pickPath}><FolderOpen size={17} />选择</button></div></label></div><label>默认 IDE<select required value={ideId ?? ""} onChange={(event) => setIdeId(event.target.value)}><option value="">选择用于打开该项目的 IDE</option>{availableIdes.map((ide) => { const status = ideInstallation(ide, installations)?.status; const installed = status === "available" || status === "installed"; return <option key={ide.id} value={ide.id}>{ide.name}{installed ? "" : "（未检测到）"}</option>; })}</select><small className="field-help">未检测到的 IDE 仍可保存，之后可到 IDE 管理中重新配置。</small></label><label>描述（可选）<textarea value={description ?? ""} onChange={(event) => setDescription(event.target.value)} placeholder="补充项目说明" /></label><TagEditor value={tags} suggestions={tagSuggestions} onChange={setTags} />{project.modules.some((item) => item.id !== module?.id) && <div className="linked-launch"><span className="form-field-label">联动启动（可选）</span><small>打开当前项目时，先依次打开所选项目；只执行一层联动。</small><div className="linked-launch-options">{project.modules.filter((item) => item.id !== module?.id).map((item) => <label key={item.id}><input type="checkbox" checked={linkedModuleIds.includes(item.id)} onChange={(event) => setLinkedModuleIds((current) => event.target.checked ? [...current, item.id] : current.filter((id) => id !== item.id))} /><span>{item.name}</span></label>)}</div></div>}<label className="checkbox-row"><input type="checkbox" checked={favorite} onChange={(event) => setFavorite(event.target.checked)} /><span><strong>收藏项目</strong><small>在收藏筛选和快速启动中优先显示</small></span></label><div className="modal-actions"><button type="button" className="secondary-button" onClick={onClose}>取消</button><button className="primary-button" disabled={!name.trim() || !path.trim() || !ideId}>{module ? "保存更改" : "添加项目"}</button></div></form></Modal>;
}

function ConfirmModal({ title, body, onClose, onConfirm }: { title: string; body: string; onClose: () => void; onConfirm: () => void }) { return <Modal title={title} onClose={onClose}><p className="confirm-copy">{body}</p><div className="modal-actions"><button className="secondary-button" onClick={onClose}>取消</button><button className="danger-button" onClick={onConfirm}>删除配置</button></div></Modal>; }

function Modal({ title, subtitle, children, onClose, wide = false }: { title: string; subtitle?: string; children: React.ReactNode; onClose: () => void; wide?: boolean }) { return <div className="modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}><section className={`modal ${wide ? "wide" : ""}`}><header><div><h2>{title}</h2>{subtitle && <p>{subtitle}</p>}</div><button className="icon-button" onClick={onClose}><X size={19} /></button></header><div className="modal-body">{children}</div></section></div>; }

function recentTimestamp(project: Project) { return Math.max(Number(project.lastOpenedAt ?? 0), ...project.modules.map((module) => Number(module.lastOpenedAt ?? 0))); }
function currentPlatform(): "windows" | "macos" { return navigator.platform.toLocaleLowerCase().includes("mac") ? "macos" : "windows"; }
