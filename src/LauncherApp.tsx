import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowUpDown, Code2, CornerDownLeft, FileCode2, Search, Star, X } from "lucide-react";
import { api, errorMessage } from "./api";
import type { DashboardData, LauncherItem } from "./types";
import { launcherItems, moduleTypeLabel, moduleTypeTone, searchLauncherItems } from "./utils";

const empty: DashboardData = {
  projects: [],
  ideDefinitions: [],
  ideInstallations: [],
  settings: { theme: "system", globalShortcut: "CommandOrControl+Shift+P", launcherWidth: 720, launcherHeight: 440 },
};

function highlightSearchText(text: string, query: string, keyPrefix: string): ReactNode {
  const normalizedQuery = query.trim();
  if (!normalizedQuery) return text;

  const lowerText = text.toLocaleLowerCase();
  const lowerQuery = normalizedQuery.toLocaleLowerCase();
  const parts: ReactNode[] = [];
  let cursor = 0;
  let matchIndex = lowerText.indexOf(lowerQuery, cursor);

  while (matchIndex >= 0) {
    if (matchIndex > cursor) parts.push(text.slice(cursor, matchIndex));
    parts.push(<span key={`${keyPrefix}-highlight-${matchIndex}`} className="search-highlight">{text.slice(matchIndex, matchIndex + normalizedQuery.length)}</span>);
    cursor = matchIndex + normalizedQuery.length;
    matchIndex = lowerText.indexOf(lowerQuery, cursor);
  }

  if (!parts.length) return text;
  if (cursor < text.length) parts.push(text.slice(cursor));
  return parts;
}

export default function LauncherApp() {
  const [dashboard, setDashboard] = useState(empty);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [error, setError] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const currentWindow = getCurrentWindow();

  async function refresh() {
    try {
      const value = await api.dashboard();
      setDashboard({ ...empty, ...value, projects: value.projects ?? [], ideDefinitions: value.ideDefinitions ?? [], ideInstallations: value.ideInstallations ?? [] });
      document.documentElement.dataset.theme = value.settings?.theme ?? "system";
      setError("");
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  useEffect(() => {
    void refresh();
    const focusSearch = () => {
      setQuery("");
      setSelected(0);
      void refresh();
      window.setTimeout(() => inputRef.current?.focus(), 30);
    };
    const unlistenPromise = currentWindow.onFocusChanged(({ payload }) => {
      if (payload) focusSearch();
      else void hide();
    });
    const unlistenShownPromise = currentWindow.listen("launcher://shown", focusSearch);
    return () => {
      void unlistenPromise.then((unlisten) => unlisten());
      void unlistenShownPromise.then((unlisten) => unlisten());
    };
  }, []);

  const searchableItems = useMemo(() => launcherItems(dashboard.projects, dashboard.ideDefinitions), [dashboard.projects, dashboard.ideDefinitions]);
  const items = useMemo(() => searchLauncherItems(searchableItems, query).slice(0, 9), [searchableItems, query]);

  useEffect(() => setSelected((value) => Math.min(value, Math.max(0, items.length - 1))), [items.length]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelected((value) => Math.min(items.length - 1, value + 1));
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelected((value) => Math.max(0, value - 1));
      }
      if (event.key === "Enter" && items[selected]) {
        event.preventDefault();
        void launch(items[selected]);
      }
      if (event.key === "Escape") {
        event.preventDefault();
        void hide();
      }
      if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey && document.activeElement !== inputRef.current) {
        event.preventDefault();
        inputRef.current?.focus();
        setQuery((value) => value + event.key);
        setSelected(0);
      }
    };
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [items, selected]);

  async function hide() {
    try {
      await api.hideLauncher();
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  async function launch(item: LauncherItem) {
    try {
      await api.launchModule(item.module.id);
      await hide();
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  return <main className="launcher-shell">
    <div className="launcher-search"><Search size={22} /><input ref={inputRef} autoFocus value={query} onChange={(event) => { setQuery(event.target.value); setSelected(0); }} placeholder="搜索项目名称或拼音、标签、路径、IDE…" />{query && <button className="icon-button" title="清空搜索" onClick={() => setQuery("")}><X size={17} /></button>}<button className="icon-button launcher-close" title="关闭快速启动" onClick={() => void hide()}><X size={18} /></button></div>
    {error ? <div className="launcher-error">{error}</div> : <div className="launcher-results">{items.length ? items.map((item, index) => <button key={item.module.id} className={`launcher-item ${selected === index ? "selected" : ""}`} onMouseEnter={() => setSelected(index)} onClick={() => void launch(item)}><div className={`module-icon ${moduleTypeTone[item.module.moduleType]}`}><FileCode2 size={20} /></div><div className="launcher-copy"><strong>{highlightSearchText(item.project.name, query, "project")} <span>/</span> {highlightSearchText(item.module.name, query, "module")}</strong><small>{moduleTypeLabel[item.module.moduleType]} · {item.module.tags.join(" · ") || item.module.path?.path || "未配置路径"}</small></div>{item.module.isFavorite || item.project.isFavorite ? <Star size={15} fill="currentColor" className="favorite-star" /> : null}<span className="launcher-ide"><Code2 size={15} />{item.ide?.name ?? "未指定 IDE"}</span></button>) : <div className="launcher-empty"><Search size={25} /><strong>{query ? "没有匹配的项目" : "还没有可启动的项目"}</strong><span>{query ? "试试项目名、拼音首字母、标签、路径或 IDE" : "请先在主窗口中完成项目配置"}</span></div>}</div>}
    <footer className="launcher-footer"><span><ArrowUpDown size={14} />选择</span><span><CornerDownLeft size={14} />打开</span><span><kbd>Esc</kbd>关闭</span><span className="launcher-count">{items.length} 个结果</span></footer>
  </main>;
}
