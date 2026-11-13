use crate::*;

#[derive(Serialize)]
pub(crate) struct ContentItem {
    pub(crate) kind: &'static str,
    pub(crate) file: String,
    pub(crate) world: Option<String>,
    pub(crate) enabled: bool,
    pub(crate) size: u64,
    pub(crate) title: Option<String>,
    pub(crate) project_id: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) manual: bool,
    pub(crate) group: Option<String>,
    /// A file of the modpack that could not be downloaded; the user adds it by hand.
    pub(crate) missing: bool,
    pub(crate) cf_id: Option<u32>,
}

#[tauri::command]
pub(crate) async fn list_content(app: State<'_, App>, id: String) -> R<Vec<ContentItem>> {
    let mods = core::mods::list(&app.dirs, &id).await.map_err(err)?;
    let packs = core::content::list(&app.dirs, &id).await.map_err(err)?;
    Ok(mods
        .into_iter()
        .map(|m| ContentItem { kind: "mod", file: m.file, world: None, enabled: m.enabled, size: m.size, title: m.title, project_id: m.project_id, icon: m.icon, manual: m.manual, group: m.group, missing: false, cf_id: None })
        .chain(packs.into_iter().map(|c| ContentItem { kind: c.kind, file: c.file, world: c.world, enabled: c.enabled, size: c.size, title: c.title, project_id: c.project_id, icon: c.icon, manual: c.manual, group: c.group, missing: false, cf_id: None }))
        .chain(core::packs::missing(&app.dirs, &id).await.into_iter().map(|m| {
            let kind = match m.folder.as_str() {
                "resourcepacks" => "resourcepack",
                "shaderpacks" => "shader",
                _ => "mod",
            };
            ContentItem { kind, title: Some(m.name.trim_end_matches(".jar").to_string()), file: m.name, world: None, enabled: false, size: 0, project_id: None, icon: None, manual: false, group: None, missing: true, cf_id: Some(m.project_id) }
        }))
        .collect())
}

#[tauri::command]
pub(crate) async fn dismiss_missing(app: State<'_, App>, id: String, name: String) -> R<()> {
    core::packs::dismiss_missing(&app.dirs, &id, &name).await;
    Ok(())
}

#[tauri::command]
pub(crate) async fn toggle_content(app: State<'_, App>, id: String, kind: String, file: String, world: Option<String>, enabled: bool) -> R<()> {
    match kind.as_str() {
        "mod" => core::mods::set_enabled(&app.dirs, &id, &file, enabled).await.map_err(err),
        _ => core::content::set_enabled(&app.dirs, &id, &kind, world.as_deref(), &file, enabled).await.map_err(err),
    }
}

#[tauri::command]
pub(crate) async fn remove_content(app: State<'_, App>, id: String, kind: String, file: String, world: Option<String>) -> R<()> {
    match kind.as_str() {
        "mod" => core::mods::remove(&app.dirs, &id, &file).await.map_err(err),
        _ => core::content::remove(&app.dirs, &id, &kind, world.as_deref(), &file).await.map_err(err),
    }
}

/// The game's output in a window of its own. The id is checked, because it becomes part of the window label.
pub(crate) fn open_console_window(handle: &AppHandle, id: &str, name: &str) {
    if !core::paths::valid_id(id) {
        return;
    }
    let label = format!("console-{id}");
    if let Some(w) = handle.get_webview_window(&label) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let url = tauri::WebviewUrl::App(format!("console.html?id={id}").into());
    let _ = tauri::WebviewWindowBuilder::new(handle, &label, url).title(format!("Konsole · {name}")).inner_size(900.0, 560.0).min_inner_size(480.0, 320.0).build();
}

/// Title pictures of the Minecraft releases, from the feed Mojang's own launcher uses: (version, image url).
#[tauri::command]
pub(crate) async fn version_images(app: State<'_, App>) -> R<Vec<(String, String)>> {
    #[derive(serde::Deserialize)]
    struct Feed { entries: Vec<Entry> }
    #[derive(serde::Deserialize)]
    struct Entry { version: String, #[serde(rename = "type")] kind: String, image: Option<Img> }
    #[derive(serde::Deserialize)]
    struct Img { url: String }
    let feed: Feed = app.http.get("https://launchercontent.mojang.com/v2/javaPatchNotes.json").send().await.map_err(err)?.error_for_status().map_err(err)?.json().await.map_err(err)?;
    Ok(feed
        .entries
        .into_iter()
        .filter(|e| e.kind == "release" || e.kind == "snapshot")
        .filter_map(|e| e.image.filter(|i| i.url.starts_with("/v2/images/") && !i.url.contains("..")).map(|i| (e.version, format!("https://launchercontent.mojang.com{}", i.url))))
        .collect())
}

#[tauri::command]
pub(crate) async fn open_console(app: State<'_, App>, handle: AppHandle, id: String) -> R<()> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    open_console_window(&handle, &id, &inst.name);
    Ok(())
}

#[tauri::command]
pub(crate) async fn recent_play(app: State<'_, App>, limit: usize) -> R<Vec<core::recent::Recent>> {
    core::recent::list(&app.dirs, limit).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn set_instance_memory(app: State<'_, App>, id: String, mb: Option<u32>) -> R<()> {
    core::instance::set_memory(&app.dirs, &id, mb).await.map_err(err)
}

/// Installed RAM in MB, or 0 when it cannot be read.
#[tauri::command]
pub(crate) fn system_memory_mb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.strip_prefix("MemTotal:").map(|v| v.trim().trim_end_matches("kB").trim().parse::<u64>().ok())).flatten())
            .map_or(0, |kb| kb / 1024)
    }
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        struct MemoryStatus {
            length: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virt: u64,
            avail_virt: u64,
            avail_ext: u64,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalMemoryStatusEx(buf: *mut MemoryStatus) -> i32;
        }
        let mut m = MemoryStatus { length: std::mem::size_of::<MemoryStatus>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virt: 0, avail_virt: 0, avail_ext: 0 };
        if unsafe { GlobalMemoryStatusEx(&mut m) } != 0 { m.total_phys / (1024 * 1024) } else { 0 }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        0
    }
}

#[tauri::command]
pub(crate) async fn set_instance_sync(app: State<'_, App>, id: String, on: bool) -> R<()> {
    core::instance::set_sync(&app.dirs, &id, on).await.map_err(err)
}

/// "save": this modpack's settings become the shared ones. "load": the shared ones are applied here.
#[tauri::command]
pub(crate) async fn sync_now(app: State<'_, App>, id: String, direction: String) -> R<Vec<String>> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    match direction.as_str() {
        "save" => core::sync::pull_from_instance(&app.dirs, &inst).await.map_err(err),
        "load" => core::sync::push_to_instance(&app.dirs, &inst).await.map_err(err),
        _ => Err("unknown direction".into()),
    }
}

#[tauri::command]
pub(crate) async fn add_files(app: State<'_, App>, id: String, paths: Vec<String>) -> R<core::content::AddResult> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    let paths = paths.into_iter().map(std::path::PathBuf::from).collect();
    core::content::add_files(&app.dirs, &inst, paths).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn search_content(app: State<'_, App>, id: String, kind: String, query: String, offset: u32) -> R<Vec<PackHit>> {
    let query: String = query.chars().filter(|c| !c.is_control()).take(100).collect();
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    let r = core::modrinth::search_content(&app.http, &query, &inst.mc_version, &kind, offset, 20).await.map_err(err)?;
    Ok(r.hits
        .into_iter()
        .map(|h| PackHit { source: "modrinth", id: h.project_id, title: h.title, description: h.description, icon: h.icon_url, downloads: h.downloads, author: h.author })
        .collect())
}

#[tauri::command]
pub(crate) async fn install_content(app: State<'_, App>, id: String, kind: String, project: String, world: Option<String>) -> R<core::mods::Installed> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    core::content::install(&app.dirs, &app.http, &inst, &kind, &project, world.as_deref()).await.map_err(err)
}
