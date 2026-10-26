import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowDown, ArrowUp, Check, FolderOpen, Layers, Plus, RefreshCw, Trash2, X } from "lucide-react";
import { api, errorMessage } from "./api";
import type { GroupItem, GroupResult, PathKind, PathRow, Project, WorkGroup } from "./types";

/** Shared keyboard-accessible dialog for maintenance and launch workflows. */
export function WorkspaceDialog({ title, children, onClose, busy = false }: { title: string; children: ReactNode; onClose: () => void; busy?: boolean }) {
  const root = useRef<HTMLDivElement>(null);
  const close = useRef(onClose); close.current = onClose;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    root.current?.focus();
    function key(event: KeyboardEvent) {
      const dialogs = document.querySelectorAll(".workspace-dialog");
      if (!root.current || dialogs[dialogs.length - 1] !== root.current) return;
      if (event.key === "Escape" && !busy) { event.stopPropagation(); close.current(); }
      if (event.key === "Tab") {
        const nodes = Array.from(root.current.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),a[href],[tabindex="0"]')).filter(n => n.getClientRects().length);
        const first = nodes[0], last = nodes[nodes.length - 1];
        if (!first) { event.preventDefault(); return; }
        if (!root.current.contains(document.activeElement)) { event.preventDefault(); first.focus(); return; }
        if (event.shiftKey && (document.activeElement === first || document.activeElement === root.current)) { event.preventDefault(); last?.focus(); }
        else if (!event.shiftKey && (document.activeElement === last || document.activeElement === root.current)) { event.preventDefault(); first.focus(); }
      }
    }
    document.addEventListener("keydown", key);
    return () => { document.removeEventListener("keydown", key); previous?.focus(); };
  }, [busy]);
  return createPortal(<div className="modal-backdrop workspace-backdrop"><div ref={root} tabIndex={-1} className="workspace-dialog" role="dialog" aria-modal="true" aria-label={title}><header><div><p className="eyebrow">PROJECT HUB</p><h2>{title}</h2></div><button disabled={busy} className="icon-button" title="关闭" onClick={onClose}><X size={20} /></button></header><div className="workspace-body">{children}</div></div></div>, document.body);
}

/** Check targets and stage individual or prefix-based repairs before committing. */
export function PathMaintenance({ projects, initialProjectId = "", onChanged, onClose }: { projects: Project[]; initialProjectId?: string; onChanged: () => Promise<void>; onClose: () => void }) {
  const [scope, setScope] = useState(initialProjectId);
  const [rows, setRows] = useState<PathRow[]>([]);
  const [preview, setPreview] = useState<PathRow[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [onlyInvalid, setOnlyInvalid] = useState(true);
  const [oldPrefix, setOldPrefix] = useState("");
  const [newPrefix, setNewPrefix] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const request = useRef(0);
  async function scan() {
    const token = ++request.current; setBusy(true); setError(""); setPreview([]); setSelected([]);
    try { const result = await api.inspectPaths(scope); if (token === request.current) setRows(result); }
    catch (e) { if (token === request.current) setError(errorMessage(e)); }
    finally { if (token === request.current) setBusy(false); }
  }
  useEffect(() => { void scan(); return () => { request.current++; }; }, [scope]);
  function stage(next: PathRow[]) { setPreview(next); setSelected(next.filter(r => r.status === "valid").map(r => r.change.moduleId)); setNotice(next.length ? "请核对新路径，只有勾选的有效项会被保存" : "没有匹配的路径，请检查旧目录或业务项目范围"); }
  async function relocate(row: PathRow, kind: PathKind) {
    setBusy(true); setError("");
    try {
      const path = await open({ directory: kind === "directory", multiple: false, title: `重新定位 ${row.moduleName}` });
      if (typeof path === "string") {
        const result = await api.previewPaths([{ ...row.change, newPath: path, pathKind: kind }]);
        stage([...preview.filter(r => r.change.moduleId !== row.change.moduleId), ...result]);
      }
    } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  }
  async function prefix() { setBusy(true); setError(""); try { stage(await api.prefixPaths(scope, oldPrefix, newPrefix)); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); } }
  async function apply() {
    setBusy(true); setError("");
    try {
      await api.applyPaths(preview.filter(r => selected.includes(r.change.moduleId)).map(r => r.change));
      setPreview([]); setSelected([]); setNotice("路径已保存，IDE、标签和联动配置保持不变");
      await onChanged(); await scan();
    } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  }
  const invalid = rows.filter(r => r.status !== "valid").length;
  return <WorkspaceDialog title="路径维护" onClose={onClose} busy={busy}>
    <p className="workspace-hint">检查本机项目目标并修复配置。不会移动、删除或修改真实代码文件。</p>
    <div className="workspace-toolbar workspace-scope-toolbar"><label>业务项目<select disabled={busy} value={scope} onChange={e => { setScope(e.target.value); setNotice(""); }}><option value="">全部业务项目</option>{projects.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label><button disabled={busy} className="secondary-button" onClick={() => void scan()}><RefreshCw size={16} />{busy ? "处理中…" : "重新检查"}</button><span className="workspace-hint workspace-scope-summary">共 {rows.length} 项 · {invalid} 项异常</span></div>
    <section className="workspace-section"><h3>批量替换目录前缀</h3><p className="workspace-hint">例如 D:\Project → E:\Code，按完整目录边界匹配。生成预览会替换当前待保存清单。</p><div className="prefix-form"><label>旧目录<input disabled={busy} value={oldPrefix} onChange={e => setOldPrefix(e.target.value)} placeholder="D:\Project" /></label><label>新目录<input disabled={busy} value={newPrefix} onChange={e => setNewPrefix(e.target.value)} placeholder="E:\Code" /></label><button className="secondary-button" disabled={busy || !oldPrefix.trim() || !newPrefix.trim()} onClick={() => void prefix()}>生成预览</button></div></section>
    {error && <p className="workspace-error" role="alert">{error}</p>}{notice && <p className="workspace-notice" role="status">{notice}</p>}
    {preview.length > 0 && <section className="workspace-section"><div className="workspace-toolbar"><h3>修改预览 · {preview.length} 项</h3><button disabled={busy} className="secondary-button small" onClick={() => { setPreview([]); setSelected([]); }}>取消待保存修改</button></div><div className="workspace-rows">{preview.map(row => <label className="path-preview" key={row.change.moduleId}><input type="checkbox" disabled={busy || row.status !== "valid"} checked={selected.includes(row.change.moduleId)} onChange={e => setSelected(ids => e.target.checked ? [...ids, row.change.moduleId] : ids.filter(id => id !== row.change.moduleId))} /><div><strong>{row.projectName} / {row.moduleName}</strong><small>原：{row.change.oldPath || "未配置"}</small><small>新：{row.change.newPath}</small><span className={row.status === "valid" ? "workspace-ok" : "workspace-error"}>{row.message}</span></div></label>)}</div><div className="workspace-footer"><span>已选 {selected.length} 项</span><button className="primary-button" disabled={busy || !selected.length} onClick={() => void apply()}><Check size={16} />确认保存</button></div></section>}
    <div className="workspace-toolbar"><h3>检查结果</h3><label className="workspace-check"><input type="checkbox" checked={onlyInvalid} onChange={e => setOnlyInvalid(e.target.checked)} />仅显示异常</label></div>
    <div className="workspace-rows">{rows.filter(r => !onlyInvalid || r.status !== "valid").map(row => <article className="workspace-row" key={row.change.moduleId}><div className="workspace-row-copy"><strong>{row.projectName} / {row.moduleName}</strong><small>{row.change.oldPath || "未配置路径"}</small><span className={row.status === "valid" ? "workspace-ok" : "workspace-error"}>{row.message}</span></div><div className="workspace-actions"><button disabled={busy} className="secondary-button small" onClick={() => void relocate(row, row.change.pathKind)}><FolderOpen size={15} />{row.change.oldPath ? "重新定位" : "选择目录"}</button>{!row.change.oldPath && <button disabled={busy} className="secondary-button small" onClick={() => void relocate(row, "file")}>选择文件</button>}</div></article>)}</div>
    {!busy && !rows.some(r => !onlyInvalid || r.status !== "valid") && <p className="workspace-empty">{rows.length ? "当前范围内的项目路径均有效" : "当前范围内还没有项目"}</p>}
  </WorkspaceDialog>;
}

/** Manage named groups within a business project, with ordered item editing. */
export function WorkGroupsPanel({ projects, groups, initialProjectId = "", initialGroupId, onChanged, onRun, onClose }: { projects: Project[]; groups: WorkGroup[]; initialProjectId?: string; initialGroupId?: string; onChanged: () => Promise<void>; onRun: (id: string) => void; onClose: () => void }) {
  const [scope, setScope] = useState(initialProjectId || projects[0]?.id || "");
  const [draft, setDraft] = useState<WorkGroup | null>(null);
  const [kind, setKind] = useState<GroupItem["kind"]>("module");
  const [target, setTarget] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState<string | null>(null);
  useEffect(() => {
    const selectedGroup = groups.find(g => g.id === initialGroupId);
    if (selectedGroup) { setScope(selectedGroup.projectId); setDraft({ ...selectedGroup, items: selectedGroup.items.map(item => ({ ...item })) }); }
  }, [initialGroupId]);
  const project = projects.find(p => p.id === scope);
  function addItem() {
    if (!draft || !target.trim()) return;
    if (draft.items.some(i => i.kind === kind && i.target === target.trim())) { setError("该启动项已经添加"); return; }
    setDraft({ ...draft, items: [...draft.items, { kind, target: target.trim() }] }); setTarget(""); setError("");
  }
  async function pickFolder() { setBusy(true); try { const path = await open({ directory: true, multiple: false }); if (typeof path === "string") setTarget(path); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); } }
  async function save() { if (!draft) return; setBusy(true); setError(""); try { await api.saveWorkGroup(draft); setDraft(null); await onChanged(); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); } }
  async function remove(id: string) { setBusy(true); setError(""); try { await api.deleteWorkGroup(id); setDeleting(null); await onChanged(); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); } }
  function move(index: number, direction: number) { if (!draft) return; const items = [...draft.items]; [items[index], items[index + direction]] = [items[index + direction], items[index]]; setDraft({ ...draft, items }); }
  const label = (item: GroupItem) => ["module", "terminal"].includes(item.kind) ? project?.modules.find(m => m.id === item.target)?.name ?? "项目已删除 · 请移除此项" : item.target;
  const names = { module: "项目 IDE", terminal: "项目终端", folder: "文件夹", url: "网址" };
  return <WorkspaceDialog title="工作组合" onClose={onClose} busy={busy}>
    <p className="workspace-hint">按清单依次发起打开，不展开项目原有联动。终端仅打开到项目目录，不自动执行命令。</p>
    <div className="workspace-toolbar workspace-scope-toolbar"><label>业务项目<select value={scope} disabled={busy || !!draft} onChange={e => { setScope(e.target.value); setDeleting(null); }}><option value="" disabled>请选择业务项目</option>{projects.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label>{!draft && <button disabled={!project || busy} className="primary-button" onClick={() => { setDraft({ id: "", projectId: scope, name: "", items: [] }); setError(""); }}><Plus size={16} />新建组合</button>}</div>
    {error && <p role="alert" className="workspace-error">{error}</p>}
    {draft ? <section className="workspace-section"><label className="workspace-field">组合名称<input disabled={busy} maxLength={100} value={draft.name} onChange={e => setDraft({ ...draft, name: e.target.value })} placeholder="例如：前后端联调" /></label><h3>启动清单 · {draft.items.length} 项</h3><div className="workspace-rows">{draft.items.map((item, index) => <article className="workspace-row" key={`${item.kind}:${item.target}`}><div className="workspace-row-copy"><strong>{index + 1}. {label(item)}</strong><small>{names[item.kind]}</small></div><div className="workspace-actions"><button className="icon-button" title="上移" disabled={busy || index === 0} onClick={() => move(index, -1)}><ArrowUp size={16} /></button><button className="icon-button" title="下移" disabled={busy || index === draft.items.length - 1} onClick={() => move(index, 1)}><ArrowDown size={16} /></button><button className="icon-button" title="移除启动项" disabled={busy} onClick={() => setDraft({ ...draft, items: draft.items.filter((_, i) => i !== index) })}><Trash2 size={16} /></button></div></article>)}</div>
    <div className="group-add"><label>类型<select disabled={busy} value={kind} onChange={e => { setKind(e.target.value as GroupItem["kind"]); setTarget(""); }}>{Object.entries(names).map(([value, name]) => <option key={value} value={value}>{name}</option>)}</select></label><label>目标{kind === "module" || kind === "terminal" ? <select disabled={busy} value={target} onChange={e => setTarget(e.target.value)}><option value="">选择项目</option>{project?.modules.map(m => <option key={m.id} value={m.id}>{m.name}</option>)}</select> : <input disabled={busy} value={target} onChange={e => setTarget(e.target.value)} placeholder={kind === "url" ? "https://…" : "完整文件夹路径"} />}</label>{kind === "folder" && <button disabled={busy} className="secondary-button" onClick={() => void pickFolder()}>选择</button>}<button disabled={busy || !target.trim() || draft.items.length >= 50} className="secondary-button" onClick={addItem}><Plus size={16} />加入</button></div><div className="workspace-footer"><button disabled={busy} className="secondary-button" onClick={() => { setDraft(null); setError(""); }}>取消编辑</button><button disabled={busy || !draft.name.trim() || !draft.items.length} className="primary-button" onClick={() => void save()}>保存组合</button></div></section> : <div className="workspace-rows">{groups.filter(g => g.projectId === scope).map(group => <article className="workspace-row" key={group.id}><Layers size={22} /><div className="workspace-row-copy"><strong>{group.name}</strong><small>{group.items.length} 个启动项</small>{deleting === group.id && <span className="workspace-error">删除此组合？项目配置和代码文件会保留。</span>}</div><div className="workspace-actions">{deleting === group.id ? <><button disabled={busy} className="secondary-button small" onClick={() => setDeleting(null)}>取消</button><button disabled={busy} className="danger-button small" onClick={() => void remove(group.id)}>确认删除</button></> : <><button disabled={busy} className="secondary-button small" onClick={() => { setDraft({ ...group, items: group.items.map(i => ({ ...i })) }); setTarget(""); setError(""); }}>编辑</button><button disabled={busy} className="icon-button" title="删除组合" onClick={() => setDeleting(group.id)}><Trash2 size={16} /></button><button disabled={busy} className="primary-button small" onClick={() => onRun(group.id)}>检查并启动</button></>}</div></article>)}{!groups.some(g => g.projectId === scope) && <p className="workspace-empty">{project ? "还没有工作组合，创建一个常用的启动清单" : "请先创建业务项目"}</p>}</div>}
  </WorkspaceDialog>;
}

/** Preflight a group and display per-item results, retrying only unsuccessful items. */
export function GroupRunner({ group, onClose, onRepair, onEdit }: { group: WorkGroup; onClose: () => void; onRepair: () => void; onEdit: () => void }) {
  const [rows, setRows] = useState<GroupResult[]>([]);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [skip, setSkip] = useState(false);
  const [ran, setRan] = useState(false);
  const running = useRef(false);
  async function check() { setBusy(true); setError(""); try { setRows(await api.checkWorkGroup(group.id)); setRan(false); } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); } }
  useEffect(() => { void check(); }, [group.id, JSON.stringify(group.items)]);
  async function launch(retry = false) {
    if (running.current) return;
    running.current = true; setBusy(true); setError("");
    try {
      const result = await api.runWorkGroup(group.id, group.items, skip, retry ? rows.filter(r => r.status === "failed" || r.status === "skipped").map(r => r.index) : undefined);
      setRows(old => retry ? old.map(row => result.find(r => r.index === row.index) ?? row) : result); setRan(true);
    } catch (e) { setError(errorMessage(e)); } finally { running.current = false; setBusy(false); }
  }
  return <WorkspaceDialog title={`启动组合 · ${group.name}`} onClose={onClose} busy={busy}>
    <p className="workspace-hint">按清单顺序向系统提交打开请求，不等待 IDE 加载完成。项目联动不会额外展开。</p>
    {error && <p role="alert" className="workspace-error">{error}</p>}
    {busy && <p role="status" className="workspace-notice">正在检查或提交启动，请稍候…</p>}
    <div className="workspace-rows">{rows.map(row => <article className="workspace-row" key={row.index}><div className="workspace-row-copy"><strong>{row.index + 1}. {row.label}</strong><small className={row.status === "ready" || row.status === "started" ? "workspace-ok" : "workspace-error"}>{row.message}</small></div><span>{({ ready: "就绪", invalid: "异常", started: "已提交", failed: "失败", skipped: "已跳过" })[row.status]}</span></article>)}</div>
    {ran && <p className="workspace-notice">已提交 {rows.filter(r => r.status === "started").length} 项 · 失败 {rows.filter(r => r.status === "failed").length} 项 · 跳过 {rows.filter(r => r.status === "skipped").length} 项</p>}
    <label className="workspace-check"><input disabled={busy} type="checkbox" checked={skip} onChange={e => setSkip(e.target.checked)} />跳过异常项，启动其余项目</label>
    <div className="workspace-footer"><div className="workspace-actions"><button disabled={busy} className="secondary-button" onClick={onRepair}>修复路径</button><button disabled={busy} className="secondary-button" onClick={onEdit}>编辑组合</button>{!ran && <button disabled={busy} className="secondary-button" onClick={() => void check()}>重新检查</button>}</div>{ran ? <button className="primary-button" disabled={busy || !rows.some(r => r.status === "failed" || r.status === "skipped")} onClick={() => void launch(true)}>重试失败 / 跳过项</button> : <button className="primary-button" disabled={busy || !rows.length || (!skip && rows.some(r => r.status === "invalid")) || rows.every(r => r.status === "invalid")} onClick={() => void launch()}>启动组合</button>}</div>
  </WorkspaceDialog>;
}
