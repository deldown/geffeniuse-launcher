use crate::*;

#[derive(Serialize)]
pub(crate) struct PackHit {
    pub(crate) source: &'static str,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) icon: Option<String>,
    pub(crate) downloads: u64,
    pub(crate) author: String,
}

#[tauri::command]
pub(crate) async fn search_modpacks(app: State<'_, App>, source: String, query: String, offset: u32) -> R<Vec<PackHit>> {
    let query: String = query.chars().filter(|c| !c.is_control()).take(100).collect();
    match source.as_str() {
        "modrinth" => {
            let r = core::modrinth::search_modpacks(&app.http, &query, offset, 20).await.map_err(err)?;
            Ok(r.hits
                .into_iter()
                .map(|h| PackHit {
                    source: "modrinth",
                    id: h.project_id,
                    title: h.title,
                    description: h.description,
                    icon: h.icon_url,
                    downloads: h.downloads,
                    author: h.author,
                })
                .collect())
        }
        "curseforge" => {
            let cf = cf_client(&*app.settings.lock().await)?;
            let r = cf.search_modpacks(&app.http, &query, offset, 20).await.map_err(err)?;
            Ok(r.into_iter()
                .map(|m| PackHit {
                    source: "curseforge",
                    id: m.id.to_string(),
                    title: m.name,
                    description: m.summary,
                    icon: m.logo.map(|l| l.thumbnail_url),
                    downloads: m.download_count as u64,
                    author: String::new(),
                })
                .collect())
        }
        _ => Err("unknown source".into()),
    }
}

#[derive(Serialize)]
pub(crate) struct PackVersion {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) game_versions: Vec<String>,
    pub(crate) loaders: Vec<String>,
}

#[tauri::command]
pub(crate) async fn modrinth_pack_versions(app: State<'_, App>, project: String) -> R<Vec<PackVersion>> {
    let v = core::modrinth::versions(&app.http, &project).await.map_err(err)?;
    Ok(v.into_iter()
        .map(|v| PackVersion { id: v.id, name: v.version_number, game_versions: v.game_versions, loaders: v.loaders })
        .collect())
}

/// Best effort: gives a freshly installed pack the project's own icon.
pub(crate) async fn apply_pack_icon(app: &App, id: &str, url: Option<String>) {
    let Some(url) = url.filter(|u| core::details::image_host_ok(u)) else { return };
    let Ok(resp) = app.http.get(&url).send().await else { return };
    if let Ok(bytes) = resp.bytes().await {
        let _ = core::instance::set_icon_bytes(&app.dirs, id, &bytes).await;
    }
}

pub(crate) fn cf_client(s: &Settings) -> Result<CurseForge, String> {
    if !s.curseforge_usable() {
        return Err("CurseForge is switched off (Options)".into());
    }
    Ok(CurseForge::new(s.curseforge_key.clone()))
}

#[tauri::command]
pub(crate) async fn curseforge_pack_versions(app: State<'_, App>, project: String) -> R<Vec<PackVersion>> {
    let id: u32 = project.parse().map_err(err)?;
    let cf = cf_client(&*app.settings.lock().await)?;
    let files = cf.mod_files(&app.http, id).await.map_err(err)?;
    Ok(files
        .into_iter()
        .map(|f| {
            let (loaders, versions): (Vec<String>, Vec<String>) = f.game_versions.into_iter().partition(|g| g.chars().next().is_some_and(|c| !c.is_ascii_digit()));
            PackVersion { id: f.id.to_string(), name: f.file_name, game_versions: versions, loaders }
        })
        .collect())
}

#[tauri::command]
pub(crate) async fn install_curseforge_pack(app: State<'_, App>, handle: AppHandle, project: String, version: String, icon: Option<String>) -> R<ImportResult> {
    cancellable(&app, install_curseforge_pack_inner(&app, handle, project, version, icon)).await
}

pub(crate) async fn install_curseforge_pack_inner(app: &App, handle: AppHandle, project: String, version: String, icon: Option<String>) -> R<ImportResult> {
    let (pid, fid): (u32, u32) = (project.parse().map_err(err)?, version.parse().map_err(err)?);
    let settings = app.settings.lock().await.clone();
    let cf = cf_client(&settings)?;
    let files = cf.mod_files(&app.http, pid).await.map_err(err)?;
    let f = files.into_iter().find(|f| f.id == fid).ok_or("version not found")?;
    let url = f.download_url.clone().ok_or("The author does not allow downloads outside curseforge.com")?;
    if !core::download::pack_host_allowed(&url) || !f.file_name.ends_with(".zip") {
        return Err("unexpected pack file".into());
    }
    let dest = app.dirs.cache().join(format!("cf-{fid}.zip"));
    let job = core::download::Job { url, dest: dest.clone(), sha1: f.sha1(), size: Some(f.file_length) };
    core::download::download_one(&app.http, &job).await.map_err(err)?;
    let h = handle.clone();
    let r = core::packs::import_curseforge_zip(&app.dirs, &app.http, &cf, &dest, settings.parallel_downloads as usize, move |p| emit_progress(&h, "pack", p)).await;
    let _ = tokio::fs::remove_file(&dest).await;
    let r = r.map_err(err)?;
    let mut inst = r.instance;
    inst.source = Some(format!("curseforge:{pid}"));
    core::instance::save(&app.dirs, &inst).await.map_err(err)?;
    apply_pack_icon(app, &inst.id, icon).await;
    let _ = core::mods::identify(&app.dirs, &app.http, &inst.id).await;
    let instance = core::instance::get(&app.dirs, &inst.id).await.map_err(err)?;
    Ok(ImportResult { instance, manual: r.manual })
}

#[tauri::command]
pub(crate) async fn modpack_contents(app: State<'_, App>, source: String, project: String) -> R<Vec<core::packs::PackEntry>> {
    match source.as_str() {
        "modrinth" => core::packs::contents_modrinth(&app.dirs, &app.http, &project).await.map_err(err),
        "curseforge" => {
            let cf = cf_client(&*app.settings.lock().await)?;
            let id: u32 = project.parse().map_err(err)?;
            core::packs::contents_curseforge(&app.dirs, &app.http, &cf, id).await.map_err(err)
        }
        _ => Err("unknown source".into()),
    }
}

#[tauri::command]
pub(crate) async fn install_modrinth_pack(app: State<'_, App>, handle: AppHandle, project: String, version: String, icon: Option<String>) -> R<Instance> {
    cancellable(&app, install_modrinth_pack_inner(&app, handle, project, version, icon)).await
}

pub(crate) async fn install_modrinth_pack_inner(app: &App, handle: AppHandle, project: String, version: String, icon: Option<String>) -> R<Instance> {
    let versions = core::modrinth::versions(&app.http, &project).await.map_err(err)?;
    let v = versions.iter().find(|v| v.id == version).ok_or("version not found")?;
    let f = core::modrinth::primary_file(v).ok_or("version has no file")?;
    if !core::download::pack_host_allowed(&f.url) || !f.filename.ends_with(".mrpack") {
        return Err("unexpected pack file".into());
    }
    let dest = app.dirs.cache().join(format!("{}.mrpack", v.id));
    let job = core::download::Job { url: f.url.clone(), dest: dest.clone(), sha1: f.hashes.sha1.clone(), size: Some(f.size) };
    core::download::download_one(&app.http, &job).await.map_err(err)?;
    let conc = app.settings.lock().await.parallel_downloads as usize;
    let h = handle.clone();
    let mut inst = core::packs::import_mrpack(&app.dirs, &app.http, &dest, conc, move |p| emit_progress(&h, "pack", p)).await.map_err(err)?;
    inst.source = Some(format!("modrinth:{project}"));
    core::instance::save(&app.dirs, &inst).await.map_err(err)?;
    let _ = tokio::fs::remove_file(dest).await;
    apply_pack_icon(app, &inst.id, icon).await;
    let _ = core::mods::identify(&app.dirs, &app.http, &inst.id).await;
    core::instance::get(&app.dirs, &inst.id).await.map_err(err)
}

/// Runs a modpack installation that the user can abort. Everything the run created is removed again.
pub(crate) async fn cancellable<T>(app: &App, work: impl std::future::Future<Output = R<T>>) -> R<T> {
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    {
        let mut slot = app.pack_cancel.lock().unwrap();
        if slot.is_some() {
            return Err("Another installation is still running".into());
        }
        *slot = Some(cancel.clone());
    }
    let before: std::collections::HashSet<String> = core::instance::list(&app.dirs).await.map(|l| l.into_iter().map(|i| i.id).collect()).unwrap_or_default();
    let result = tokio::select! {
        r = work => r,
        () = cancel.notified() => {
            for i in core::instance::list(&app.dirs).await.unwrap_or_default().into_iter().filter(|i| !before.contains(&i.id)) {
                let _ = core::instance::delete(&app.dirs, &i.id).await;
            }
            Err("cancelled".to_string())
        }
    };
    *app.pack_cancel.lock().unwrap() = None;
    result
}

#[tauri::command]
pub(crate) fn cancel_pack_install(app: State<'_, App>) {
    if let Some(n) = app.pack_cancel.lock().unwrap().as_ref() {
        n.notify_one();
    }
}
