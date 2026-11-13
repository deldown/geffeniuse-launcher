use crate::*;

#[tauri::command]
pub(crate) async fn list_mods(app: State<'_, App>, id: String) -> R<Vec<core::mods::ModFile>> {
    core::mods::list(&app.dirs, &id).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn search_mods(app: State<'_, App>, id: String, query: String, offset: u32, source: String) -> R<Vec<PackHit>> {
    let query: String = query.chars().filter(|c| !c.is_control()).take(100).collect();
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    if inst.loader == Loader::Vanilla {
        return Err("Vanilla loads no mods. Create the instance with Fabric, Quilt, Forge or NeoForge.".into());
    }
    match source.as_str() {
        "modrinth" => {
            let r = core::modrinth::search_mods(&app.http, &query, &inst.mc_version, inst.loader, offset, 20).await.map_err(err)?;
            Ok(r.hits
                .into_iter()
                .map(|h| PackHit { source: "modrinth", id: h.project_id, title: h.title, description: h.description, icon: h.icon_url, downloads: h.downloads, author: h.author })
                .collect())
        }
        "curseforge" => {
            let cf = cf_client(&*app.settings.lock().await)?;
            let r = cf.search_mods(&app.http, &query, &inst.mc_version, inst.loader, offset, 20).await.map_err(err)?;
            Ok(r.into_iter()
                .map(|m| PackHit { source: "curseforge", id: m.id.to_string(), title: m.name, description: m.summary, icon: m.logo.map(|l| l.thumbnail_url), downloads: m.download_count as u64, author: String::new() })
                .collect())
        }
        _ => Err("unknown source".into()),
    }
}

#[tauri::command]
pub(crate) async fn install_mod(app: State<'_, App>, id: String, project: String, source: String, deps: bool) -> R<core::mods::Installed> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    match source.as_str() {
        "modrinth" => core::mods::install(&app.dirs, &app.http, &inst, &project, deps).await.map_err(err),
        "curseforge" => {
            let cf = cf_client(&*app.settings.lock().await)?;
            let mod_id: u32 = project.parse().map_err(err)?;
            core::mods::install_curseforge(&app.dirs, &app.http, &cf, &inst, mod_id, deps).await.map_err(err)
        }
        _ => Err("unknown source".into()),
    }
}

#[tauri::command]
pub(crate) async fn mod_dependencies(app: State<'_, App>, id: String, project: String, source: String) -> R<Vec<core::mods::Dep>> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    match source.as_str() {
        "modrinth" => core::mods::dependencies(&app.dirs, &app.http, &inst, &project).await.map_err(err),
        "curseforge" => {
            let cf = cf_client(&*app.settings.lock().await)?;
            let mod_id: u32 = project.parse().map_err(err)?;
            core::mods::dependencies_curseforge(&app.dirs, &app.http, &cf, &inst, mod_id).await.map_err(err)
        }
        _ => Err("unknown source".into()),
    }
}

#[tauri::command]
pub(crate) async fn identify_mods(app: State<'_, App>, id: String) -> R<usize> {
    core::mods::identify(&app.dirs, &app.http, &id).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn toggle_mod(app: State<'_, App>, id: String, file: String, enabled: bool) -> R<()> {
    core::mods::set_enabled(&app.dirs, &id, &file, enabled).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn remove_mod(app: State<'_, App>, id: String, file: String) -> R<()> {
    core::mods::remove(&app.dirs, &id, &file).await.map_err(err)
}
