use crate::{
    database::{launch_target_available, new_id, now, platform, validate_path},
    models::*,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;

fn bool_from(i: i64) -> bool {
    i != 0
}
fn json_list(value: String) -> Vec<String> {
    serde_json::from_str(&value).unwrap_or_default()
}

pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    tags.into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty() && seen.insert(tag.clone()))
        .collect()
}

fn json_tags(value: String) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(&value)
        .map(normalize_tags)
        .unwrap_or_default()
}

pub fn list_ide_definitions(db: &Connection) -> Result<Vec<IdeDefinition>, String> {
    let mut q = db.prepare("SELECT id,name,icon,supported_platforms,supported_path_kinds,built_in,default_argument_template FROM ide_definitions ORDER BY name").map_err(|e| e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok(IdeDefinition {
                id: r.get(0)?,
                name: r.get(1)?,
                icon: r.get(2)?,
                supported_platforms: json_list(r.get(3)?),
                supported_path_kinds: json_list(r.get(4)?),
                built_in: bool_from(r.get(5)?),
                default_argument_template: json_list(r.get(6)?),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn save_ide_definition(
    db: &Connection,
    input: IdeDefinitionInput,
) -> Result<IdeDefinition, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("IDE 名称不能为空".into());
    }
    let id = input.id.unwrap_or_else(|| format!("custom-{}", new_id()));
    let template = if input.default_argument_template.is_empty() {
        vec!["{path}".into()]
    } else {
        input.default_argument_template
    };
    let platforms = if input.supported_platforms.is_empty() {
        vec![platform().into()]
    } else {
        input.supported_platforms
    };
    let kinds = if input.supported_path_kinds.is_empty() {
        vec!["file".into(), "directory".into()]
    } else {
        input.supported_path_kinds
    };
    let existing_built_in: Option<i64> = db
        .query_row(
            "SELECT built_in FROM ide_definitions WHERE id=?1",
            [&id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let encoded_platforms = serde_json::to_string(&platforms).map_err(|e| e.to_string())?;
    let encoded_kinds = serde_json::to_string(&kinds).map_err(|e| e.to_string())?;
    let encoded_template = serde_json::to_string(&template).map_err(|e| e.to_string())?;
    db.execute("INSERT INTO ide_definitions(id,name,icon,supported_platforms,supported_path_kinds,default_argument_template,built_in) VALUES(?1,?2,?3,?4,?5,?6,0) ON CONFLICT(id) DO UPDATE SET name=excluded.name,icon=excluded.icon,supported_platforms=excluded.supported_platforms,supported_path_kinds=excluded.supported_path_kinds,default_argument_template=excluded.default_argument_template", params![id,name,input.icon,encoded_platforms,encoded_kinds,encoded_template]).map_err(|e| e.to_string())?;
    if existing_built_in == Some(1) {
        return get_ide_definition(db, &id)?.ok_or_else(|| "保存 IDE 配置失败".into());
    }
    get_ide_definition(db, &id)?.ok_or_else(|| "保存 IDE 配置失败".into())
}

fn get_ide_definition(db: &Connection, id: &str) -> Result<Option<IdeDefinition>, String> {
    list_ide_definitions(db).map(|items| items.into_iter().find(|item| item.id == id))
}

pub fn delete_ide_definition(db: &Connection, id: &str) -> Result<(), String> {
    let built_in: Option<i64> = db
        .query_row(
            "SELECT built_in FROM ide_definitions WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match built_in {
        Some(1) => Err("内置 IDE 不能删除".into()),
        Some(_) => {
            db.execute("DELETE FROM ide_definitions WHERE id=?1", [id])
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        None => Err("IDE 不存在或已删除".into()),
    }
}

pub fn list_installations(db: &Connection) -> Result<Vec<IdeInstallation>, String> {
    let mut q = db.prepare("SELECT i.id,i.ide_id,i.platform,i.executable_path,i.launch_kind,i.detected_source,i.status,i.enabled,d.name FROM ide_installations i JOIN ide_definitions d ON d.id=i.ide_id ORDER BY d.name,CASE WHEN i.detected_source='manual' THEN 0 ELSE 1 END,i.executable_path").map_err(|e|e.to_string())?;
    let rows = q
        .query_map([], |r| {
            Ok(IdeInstallation {
                id: r.get(0)?,
                ide_id: r.get(1)?,
                platform: r.get(2)?,
                executable_path: r.get(3)?,
                launch_kind: r.get(4)?,
                detected_source: r.get(5)?,
                status: r.get(6)?,
                enabled: bool_from(r.get(7)?),
                ide_name: Some(r.get(8)?),
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn save_installation(
    db: &Connection,
    input: IdeInstallationInput,
) -> Result<IdeInstallation, String> {
    let id = input.id.unwrap_or_else(new_id);
    let exe = input.executable_path.trim().to_string();
    if exe.is_empty() {
        return Err("IDE 路径不能为空".into());
    }
    let status = if launch_target_available(&exe, &input.launch_kind) {
        "available"
    } else {
        "missing"
    };
    if input.detected_source == "manual" {
        db.execute(
            "DELETE FROM ide_installations WHERE ide_id=?1 AND platform=?2 AND detected_source='manual' AND id<>?3",
            params![input.ide_id, platform(), id],
        )
        .map_err(|e| e.to_string())?;
    }
    db.execute("INSERT INTO ide_installations(id,ide_id,platform,executable_path,launch_kind,detected_source,status,enabled) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(ide_id,platform,executable_path) DO UPDATE SET launch_kind=excluded.launch_kind,detected_source=excluded.detected_source,status=excluded.status,enabled=excluded.enabled", params![id,input.ide_id,platform(),exe,input.launch_kind,input.detected_source,status,if input.enabled {1}else{0}]).map_err(|e|e.to_string())?;
    list_installations(db)?
        .into_iter()
        .find(|i| i.ide_id == input.ide_id && i.platform == platform() && i.executable_path == exe)
        .ok_or_else(|| "保存 IDE 配置失败".into())
}

pub fn save_project(db: &Connection, input: ProjectInput) -> Result<Project, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("项目名称不能为空".into());
    };
    let timestamp = now();
    let id = input.id.unwrap_or_else(new_id);
    let tags = serde_json::to_string(&normalize_tags(input.tags)).map_err(|e| e.to_string())?;
    db.execute("INSERT INTO projects(id,name,description,icon,color,tags,is_favorite,is_pinned,sort_order,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,icon=excluded.icon,color=excluded.color,tags=excluded.tags,is_favorite=excluded.is_favorite,is_pinned=excluded.is_pinned,sort_order=excluded.sort_order,updated_at=excluded.updated_at", params![id,name,input.description,input.icon,input.color,tags,if input.is_favorite{1}else{0},if input.is_pinned{1}else{0},input.sort_order,timestamp]).map_err(|e|e.to_string())?;
    get_project(db, &id)?.ok_or_else(|| "保存项目失败".into())
}

pub fn delete_project(db: &Connection, id: &str) -> Result<(), String> {
    if db
        .execute("DELETE FROM projects WHERE id=?1", [id])
        .map_err(|e| e.to_string())?
        == 0
    {
        return Err("项目不存在或已删除".into());
    };
    Ok(())
}

pub fn save_module(db: &mut Connection, input: ModuleInput) -> Result<ProjectModule, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("项目名称不能为空".into());
    }
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [&input.project_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        return Err("所属业务项目不存在".into());
    }
    let timestamp = now();
    let id = input.id.unwrap_or_else(new_id);
    let tags = serde_json::to_string(&normalize_tags(input.tags)).map_err(|e| e.to_string())?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO project_modules(id,project_id,name,module_type,description,tags,is_favorite,sort_order,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?9) ON CONFLICT(id) DO UPDATE SET project_id=excluded.project_id,name=excluded.name,module_type=excluded.module_type,description=excluded.description,tags=excluded.tags,is_favorite=excluded.is_favorite,sort_order=excluded.sort_order,updated_at=excluded.updated_at",params![id,input.project_id,name,input.module_type,input.description,tags,if input.is_favorite {1}else{0},input.sort_order,timestamp]).map_err(|e|e.to_string())?;
    if let Some(path) = input.path {
        let p = path.path.trim().to_string();
        let status = validate_path(&p, &path.path_kind);
        tx.execute("INSERT INTO module_paths(module_id,platform,path,path_kind,validation_status,last_validated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(module_id,platform) DO UPDATE SET path=excluded.path,path_kind=excluded.path_kind,validation_status=excluded.validation_status,last_validated_at=excluded.last_validated_at",params![id,platform(),p,path.path_kind,status,timestamp]).map_err(|e|e.to_string())?;
    }
    if let Some(ide_id) = input.ide_id {
        let args = if input.argument_template.is_empty() {
            vec!["{path}".to_string()]
        } else {
            input.argument_template
        };
        let encoded = serde_json::to_string(&args).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO module_launch_profiles(module_id,platform,ide_id,argument_template) VALUES(?1,?2,?3,?4) ON CONFLICT(module_id,platform) DO UPDATE SET ide_id=excluded.ide_id,argument_template=excluded.argument_template",params![id,platform(),ide_id,encoded]).map_err(|e|e.to_string())?;
    } else {
        tx.execute(
            "DELETE FROM module_launch_profiles WHERE module_id=?1 AND platform=?2",
            params![id, platform()],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    get_module(db, &id)?.ok_or_else(|| "保存项目失败".into())
}

pub fn delete_module(db: &Connection, id: &str) -> Result<(), String> {
    if db
        .execute("DELETE FROM project_modules WHERE id=?1", [id])
        .map_err(|e| e.to_string())?
        == 0
    {
        return Err("项目不存在或已删除".into());
    };
    Ok(())
}

pub fn get_project(db: &Connection, id: &str) -> Result<Option<Project>, String> {
    let project=db.query_row("SELECT p.id,p.name,p.description,p.icon,p.color,p.tags,p.is_favorite,p.is_pinned,p.sort_order,p.created_at,p.updated_at,(SELECT MAX(h.opened_at) FROM open_history h JOIN project_modules m ON m.id=h.module_id WHERE m.project_id=p.id AND h.result='started') FROM projects p WHERE p.id=?1",[id],|r|Ok(Project{id:r.get(0)?,name:r.get(1)?,description:r.get(2)?,icon:r.get(3)?,color:r.get(4)?,tags:json_tags(r.get(5)?),is_favorite:bool_from(r.get(6)?),is_pinned:bool_from(r.get(7)?),sort_order:r.get(8)?,created_at:r.get(9)?,updated_at:r.get(10)?,modules:vec![],last_opened_at:r.get(11)?})).optional().map_err(|e|e.to_string())?;
    if let Some(mut p) = project {
        p.modules = list_modules_for_project(db, &p.id)?;
        Ok(Some(p))
    } else {
        Ok(None)
    }
}

pub fn list_projects(db: &Connection) -> Result<Vec<Project>, String> {
    let mut q=db.prepare("SELECT p.id,p.name,p.description,p.icon,p.color,p.tags,p.is_favorite,p.is_pinned,p.sort_order,p.created_at,p.updated_at,(SELECT MAX(h.opened_at) FROM open_history h JOIN project_modules m ON m.id=h.module_id WHERE m.project_id=p.id AND h.result='started') FROM projects p ORDER BY p.is_pinned DESC,p.is_favorite DESC,p.sort_order,p.updated_at DESC").map_err(|e|e.to_string())?;
    let bare = q
        .query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                icon: r.get(3)?,
                color: r.get(4)?,
                tags: json_tags(r.get(5)?),
                is_favorite: bool_from(r.get(6)?),
                is_pinned: bool_from(r.get(7)?),
                sort_order: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
                modules: vec![],
                last_opened_at: r.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    bare.into_iter()
        .map(|mut p| {
            p.modules = list_modules_for_project(db, &p.id)?;
            Ok(p)
        })
        .collect()
}

fn get_module(db: &Connection, id: &str) -> Result<Option<ProjectModule>, String> {
    let mut result = list_modules_query(db, "WHERE m.id=?1", params![id])?;
    Ok(result.pop())
}
fn list_modules_for_project(db: &Connection, pid: &str) -> Result<Vec<ProjectModule>, String> {
    list_modules_query(db, "WHERE m.project_id=?1", params![pid])
}
fn list_modules_query<P: rusqlite::Params>(
    db: &Connection,
    where_sql: &str,
    params: P,
) -> Result<Vec<ProjectModule>, String> {
    let sql=format!("SELECT m.id,m.project_id,m.name,m.module_type,m.description,m.tags,m.is_favorite,m.sort_order,m.created_at,m.updated_at,mp.platform,mp.path,mp.path_kind,mp.validation_status,mp.last_validated_at,lp.ide_id,lp.argument_template,(SELECT MAX(opened_at) FROM open_history h WHERE h.module_id=m.id AND h.result='started') FROM project_modules m LEFT JOIN module_paths mp ON mp.module_id=m.id AND mp.platform='{}' LEFT JOIN module_launch_profiles lp ON lp.module_id=m.id AND lp.platform='{}' {} ORDER BY m.is_favorite DESC,m.sort_order,m.updated_at DESC",platform(),platform(),where_sql);
    let mut q = db.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = q
        .query_map(params, |r| {
            let template: Option<String> = r.get(16)?;
            Ok(ProjectModule {
                id: r.get(0)?,
                project_id: r.get(1)?,
                name: r.get(2)?,
                module_type: r.get(3)?,
                description: r.get(4)?,
                tags: json_tags(r.get(5)?),
                is_favorite: bool_from(r.get(6)?),
                sort_order: r.get(7)?,
                created_at: r.get(8)?,
                updated_at: r.get(9)?,
                path: match r.get::<_, Option<String>>(11)? {
                    Some(path) => Some(ModulePath {
                        platform: r.get(10)?,
                        path,
                        path_kind: r.get(12)?,
                        validation_status: r.get(13)?,
                        last_validated_at: r.get(14)?,
                    }),
                    None => None,
                },
                ide_id: r.get(15)?,
                argument_template: template
                    .map(json_list)
                    .unwrap_or_else(|| vec!["{path}".into()]),
                last_opened_at: r.get(17)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
