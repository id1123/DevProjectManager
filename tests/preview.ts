// Isolated UI fixture for browser-based layout and interaction checks.
const callbacks = new Map<number, Function>();
const listeners = new Map<number, {event: string; handler: number}>();
let serial = 0;
const makeModule = (id: string, name: string, type: string, path: string, valid: boolean) => ({ id, projectId: "business", name, moduleType: type, tags: [type.toUpperCase()], argumentTemplate: ["{path}"], linkedModuleIds: [], isFavorite: false, sortOrder: 0, ideId: "vscode", path: { platform: "windows", path, pathKind: type === "api" ? "file" : "directory", validationStatus: valid ? "valid" : "missing", lastValidatedAt: 0 } });
const data = { projects: [{ id: "business", name: "仓储管理系统", description: "API、Web 与现场调试", tags: ["WMS"], color: "#7065e8", isFavorite: true, isPinned: true, sortOrder: 0, modules: [makeModule("api", "后端 API", "api", "D:\\Project\\仓储\\Api.sln", true), makeModule("web", "管理后台", "web", "D:\\Project\\仓储\\web", false), makeModule("app", "手持终端", "app", "D:\\Project\\仓储\\app", true)] }], ideDefinitions: [{ id: "vscode", name: "VS Code", icon: "vscode", supportedPlatforms: ["windows","macos"], supportedPathKinds: ["file","directory"], defaultArgumentTemplate: ["{path}"], builtIn: true }], ideInstallations: [{ id: "vscode", ideId: "vscode", platform: "windows", executablePath: "code", launchKind: "command", status: "available", enabled: true, detectedSource: "auto" }], settings: { theme: "light", globalShortcut: "CommandOrControl+Shift+P", launcherWidth:720,launcherHeight:440,defaultIdeIds:{} } };
let groups = [{ id: "group", projectId:"business", name:"前后端联调", items:[{kind:"module",target:"api"},{kind:"module",target:"web"},{kind:"url",target:"https://example.com/swagger"}] }];
const emit = (event: string, payload: unknown) => listeners.forEach(x => { if(x.event===event) callbacks.get(x.handler)?.({event,payload}); });
const rows = () => data.projects.flatMap(p => p.modules.map(m => ({ change: { moduleId:m.id,oldPath:m.path.path,pathKind:m.path.pathKind,newPath:m.path.path },projectId:p.id,projectName:p.name,moduleName:m.name,status:m.path.validationStatus,message:m.path.validationStatus==="valid"?"有效":"路径不存在" })));
const invoke = async (cmd: string,args: any = {}) => {
  switch(cmd) {
    case "get_dashboard": return structuredClone(data);
    case "list_work_groups": return structuredClone(groups);
    case "git_statuses": return args.paths.map((path:string)=>({path,branch:"main",changedFiles:2,remoteUrl:null,error:null,detached:false}));
    case "inspect_paths": return rows();
    case "prefix_paths": return rows().filter(r=>r.change.oldPath.startsWith(args.oldPrefix)).map(r=>({...r,status:"valid",message:"有效",change:{...r.change,newPath:r.change.oldPath.replace(args.oldPrefix,args.newPrefix)}}));
    case "preview_paths": return args.changes.map((change:any)=>({...rows().find(r=>r.change.moduleId===change.moduleId),change,status:"valid",message:"有效"}));
    case "apply_paths": args.changes.forEach((c:any)=>{ const m=data.projects[0].modules.find(m=>m.id===c.moduleId)!; m.path.path=c.newPath; m.path.validationStatus="valid"; }); emit("project://launched",null); return;
    case "save_work_group": { const g=structuredClone(args.group); g.id ||= `new-${++serial}`; groups=groups.filter(x=>x.id!==g.id); groups.push(g); emit("workspace://changed",null); return g; }
    case "delete_work_group": groups=groups.filter(g=>g.id!==args.id); emit("workspace://changed",null); return;
    case "check_work_group": return groups.find(g=>g.id===args.id)!.items.map((i,index)=>({index,label:data.projects[0].modules.find(m=>m.id===i.target)?.name??i.target,status:i.target==="web"&&data.projects[0].modules[1].path.validationStatus!=="valid"?"invalid":"ready",message:i.target==="web"&&data.projects[0].modules[1].path.validationStatus!=="valid"?"路径不存在":"可以启动"}));
    case "run_work_group": return (await invoke("check_work_group",args)).filter((r:any)=>!args.indices||args.indices.includes(r.index)).map((r:any)=>({...r,status:r.status==="invalid"?"skipped":"started",message:r.status==="invalid"?"路径不存在":"已提交系统打开"}));
    case "show_work_group": emit("workspace://open-group",args.id); return;
    case "plugin:dialog|open": return args.options?.directory ? "E:\\Code\\仓储\\web" : "E:\\Code\\仓储\\Api.sln";
    case "plugin:event|listen": { const id=++serial; listeners.set(id,args); return id; }
    case "plugin:event|unlisten": listeners.delete(args.eventId); return;
    case "plugin:app|version": return "0.8.0-preview";
    case "plugin:autostart|is_enabled": return false;
    case "hide_launcher_window": return;
    default: throw new Error(`Preview fixture has no command: ${cmd}`);
  }
};
(window as any).__TAURI_INTERNALS__={ metadata:{currentWindow:{label:new URLSearchParams(location.search).has("launcher")?"launcher":"main"}}, transformCallback:(fn:Function)=>{const id=++serial;callbacks.set(id,fn);return id;}, invoke };
(window as any).__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
void import("../src/main");
