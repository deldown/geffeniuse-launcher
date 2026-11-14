#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod discord;

use geffeniuse_core as core;
use core::curseforge::CurseForge;
use core::download::Progress;
use core::instance::{Instance, Loader};
use core::launch::Account;
use core::paths::Dirs;
use core::settings::Settings;
use serde::Serialize;
use tauri::http::{Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

struct App {
    dirs: Dirs,
    http: reqwest::Client,
    settings: Mutex<Settings>,
    discord: discord::Discord,
    sessions: core::auth::Sessions,
    /// One console buffer per instance.
    logs: std::sync::Mutex<std::collections::HashMap<String, LogBuf>>,
    /// One cancel handle per instance that is preparing or running.
    runs: std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Notify>>>,
    /// Set while a modpack is being installed; notified to abort it.
    pack_cancel: std::sync::Mutex<Option<std::sync::Arc<tokio::sync::Notify>>>,
}

/// Linux desktop identity. The taskbar finds the icon through the window's class, so the app gives
/// itself a unique name, and an AppImage (which no installer registers) adds its own menu entry.
#[cfg(target_os = "linux")]
mod desktop {
    use std::ffi::CString;
    use std::os::raw::c_char;
    use std::path::{Path, PathBuf};

    pub const APP_ID: &str = "de.deldown.geffeniuse";
    const ICON: &[u8] = include_bytes!("../icons/icon.png");

    #[link(name = "glib-2.0")]
    extern "C" {
        fn g_set_prgname(name: *const c_char);
    }

    /// Must run before GTK starts: GTK derives the window class and the Wayland app id from it.
    pub fn name_the_window() {
        let name = CString::new(APP_ID).expect("no NUL in the id");
        unsafe { g_set_prgname(name.as_ptr()) };
    }

    /// The .desktop text for an AppImage at `exec`, or None when the path cannot be written safely.
    pub fn entry_text(exec: &Path, icon: &Path) -> Option<String> {
        let (exec, icon) = (exec.to_str()?, icon.to_str()?);
        let unsafe_char = |c: char| c.is_control() || matches!(c, '"' | '\\' | '`' | '$' | '%');
        if exec.chars().any(unsafe_char) || icon.chars().any(unsafe_char) {
            return None;
        }
        Some(format!(
            "[Desktop Entry]\nType=Application\nName=Geffeniuse Launcher\nComment=Minecraft launcher with modpacks\nExec=\"{exec}\"\nIcon={icon}\nStartupWMClass={APP_ID}\nCategories=Game;\nTerminal=false\n"
        ))
    }

    fn data_home() -> Option<PathBuf> {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }

    /// Only for AppImages: writes the menu entry and icon into the user's own data folder, and only
    /// when they are missing or point at an older location.
    pub fn integrate_appimage() {
        let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_absolute()) else { return };
        let Some(data) = data_home() else { return };
        let icon = data.join("icons/hicolor/256x256/apps").join(format!("{APP_ID}.png"));
        let desktop = data.join("applications").join(format!("{APP_ID}.desktop"));
        let Some(text) = entry_text(&appimage, &icon) else { return };
        if std::fs::read(&icon).ok().as_deref() != Some(ICON) {
            let _ = icon.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(&icon, ICON);
        }
        if std::fs::read_to_string(&desktop).ok().as_deref() != Some(text.as_str()) {
            let _ = desktop.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(&desktop, text);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn entry_points_at_the_appimage_and_the_icon() {
            let t = entry_text(Path::new("/home/a b/Geffeniuse.AppImage"), Path::new("/home/a b/icon.png")).unwrap();
            assert!(t.contains("Exec=\"/home/a b/Geffeniuse.AppImage\"\n"));
            assert!(t.contains("Icon=/home/a b/icon.png\n") && t.contains("StartupWMClass=de.deldown.geffeniuse\n"));
        }

        #[test]
        fn paths_that_could_break_the_entry_are_refused() {
            for bad in ["/x/\"y.AppImage", "/x/$HOME.AppImage", "/x/a%f.AppImage", "/x/`id`.AppImage", "/x/a\nExec=evil"] {
                assert!(entry_text(Path::new(bad), Path::new("/i.png")).is_none(), "{bad}");
            }
        }
    }
}

type LogBuf = std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<String>>>;

impl App {
    fn log_of(&self, id: &str) -> LogBuf {
        self.logs.lock().unwrap().entry(id.to_string()).or_default().clone()
    }
}

const LOG_LINES: usize = 3000;
const LOG_LINE_LEN: usize = 2000;

/// Stream one child pipe into the shared ring buffer. Minecraft prints thousands of lines while
/// starting, so the window gets them in batches every 100 ms instead of one event per line.
fn pump_log<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    r: R,
    log: LogBuf,
    handle: AppHandle,
    id: String,
    dirs: Dirs,
    prefix: String,
) {
    use tokio::io::AsyncBufReadExt;
    tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(r).lines();
        let mut batch: Vec<String> = Vec::new();
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                line = lines.next_line() => match line {
                    Ok(Some(line)) => {
                        let line: String = format!("{prefix}{line}").chars().take(LOG_LINE_LEN).collect();
                        if let Some(addr) = core::recent::server_from_log(&line) {
                            core::recent::remember_server(&dirs, &id, &addr);
                        }
                        let mut l = log.lock().unwrap();
                        if l.len() >= LOG_LINES {
                            l.pop_front();
                        }
                        l.push_back(line.clone());
                        batch.push(line);
                    }
                    _ => break,
                },
                _ = tick.tick() => {
                    if !batch.is_empty() {
                        let _ = handle.emit("game-log", (&id, std::mem::take(&mut batch)));
                    }
                }
            }
        }
        if !batch.is_empty() {
            let _ = handle.emit("game-log", (&id, batch));
        }
    });
}

#[tauri::command]
fn get_log(app: State<'_, App>, id: String) -> Vec<String> {
    if !core::paths::valid_id(&id) {
        return Vec::new();
    }
    app.log_of(&id).lock().unwrap().iter().cloned().collect()
}

#[tauri::command]
fn clear_log(app: State<'_, App>, id: String) {
    if core::paths::valid_id(&id) {
        app.log_of(&id).lock().unwrap().clear();
    }
}

#[tauri::command]
async fn list_worlds(app: State<'_, App>, id: Option<String>) -> R<Vec<core::worlds::World>> {
    match id {
        Some(id) => core::worlds::list_instance(&app.dirs, &id).await.map_err(err),
        None => core::worlds::list(&app.dirs).await.map_err(err),
    }
}

type R<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    stage: String,
    done: usize,
    total: usize,
}

const PROGRESS_INTERVAL_MS: u64 = 100;

/// Progress fires once per downloaded file, thousands of times per launch. The window only needs
/// ~10 updates a second, so everything in between is dropped (first and last always pass).
fn progress_due(last_ms: &std::sync::atomic::AtomicU64, now_ms: u64, p: &Progress) -> bool {
    use std::sync::atomic::Ordering::Relaxed;
    if p.done == 0 || p.done >= p.total {
        last_ms.store(now_ms, Relaxed);
        return true;
    }
    let last = last_ms.load(Relaxed);
    if now_ms.saturating_sub(last) >= PROGRESS_INTERVAL_MS {
        last_ms.store(now_ms, Relaxed);
        return true;
    }
    false
}

fn emit_progress(app: &AppHandle, stage: &str, p: Progress) {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64;
    if progress_due(&LAST, now, &p) {
        let _ = app.emit("progress", ProgressEvent { stage: stage.into(), done: p.done, total: p.total });
    }
}

mod commands;
use commands::*;

/// Splits a request into (kind, arguments). On Linux the kind is the host (`geffeniuse://icon/a/b`);
/// WebView2 on Windows serves custom schemes from `http://geffeniuse.localhost/icon/a/b`.
fn protocol_route<'a>(host: Option<&'a str>, path: &'a str) -> Option<(&'a str, Vec<&'a str>)> {
    let mut parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let kind = match host {
        Some("geffeniuse.localhost") if !parts.is_empty() => parts.remove(0),
        Some(h) if !h.contains('.') => h,
        _ => return None,
    };
    Some((kind, parts))
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        desktop::name_the_window();
        desktop::integrate_appimage();
    }
    // WebKitGTK's DMABUF renderer flashes white on many GPUs whenever the page repaints a lot.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    core::update::remember_exe();
    core::update::cleanup_leftovers();
    let dirs = Dirs::new(Dirs::default_root());
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).max_blocking_threads(8).enable_all().build().expect("runtime");
    let settings = rt.block_on(async {
        let _ = dirs.ensure().await;
        Settings::load(&dirs).await
    });
    let discord = rt.block_on(async { discord::Discord::new(&settings) });
    tauri::async_runtime::set(rt.handle().clone());
    let protocol_dirs = dirs.clone();
    let adopted = rt.block_on(core::running::scan(&dirs));
    let adopt_runtime = rt.handle().clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            use tauri::Manager;
            for found in adopted {
                adopt_run(app.handle().clone(), adopt_runtime.clone(), found);
            }
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = window.set_icon(tauri::image::Image::new(include_bytes!("../icons/window.rgba"), 256, 256)) { eprintln!("window icon: {e}"); }
            }
            Ok(())
        })
        .manage(App { dirs, discord, sessions: Default::default(), http: core::http_client(), settings: Mutex::new(settings), logs: Default::default(), runs: Default::default(), pack_cancel: Default::default() })
        .register_uri_scheme_protocol("geffeniuse", move |_ctx, req| {
            // geffeniuse://{shot,icon}/<instance>/<file> (Linux) or http://geffeniuse.localhost/{shot,icon}/... (Windows):
            // validated lookups only, never raw paths.
            let uri_path = req.uri().path().to_string();
            let route = protocol_route(req.uri().host(), &uri_path);
            let resp = |code: StatusCode, body: Vec<u8>, ct: &str| {
                Response::builder().status(code).header("Content-Type", ct).header("X-Content-Type-Options", "nosniff").body(body).unwrap()
            };
            let served: Option<(Vec<u8>, &str)> = match route.as_ref().map(|(k, p)| (*k, p.as_slice())) {
                Some(("shot", [inst, file])) => core::screenshots::path_of(&protocol_dirs, inst, file).ok().and_then(|p| std::fs::read(p).ok()).map(|b| {
                    let l = file.to_ascii_lowercase();
                    (b, if l.ends_with(".png") { "image/png" } else if l.ends_with(".webp") { "image/webp" } else { "image/jpeg" })
                }),
                Some(("icon", [inst, file])) => core::instance::icon_path(&protocol_dirs, inst, file).ok().and_then(|p| std::fs::read(p).ok()).map(|b| {
                    (b, if file.ends_with(".png") { "image/png" } else if file.ends_with(".webp") { "image/webp" } else { "image/jpeg" })
                }),
                _ => None,
            };
            match served {
                Some((bytes, ct)) => resp(StatusCode::OK, bytes, ct),
                None => resp(StatusCode::NOT_FOUND, vec![], "text/plain"),
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            list_instances,
            minecraft_versions,
            create_instance,
            delete_instance,
            duplicate_instance,
            export_instance,
            check_mod_updates,
            update_mods,
            dismiss_missing,
            cancel_pack_install,
            set_content_group,
            launch_instance,
            stop_instance,
            search_modpacks,
            modrinth_pack_versions,
            install_modrinth_pack,
            curseforge_pack_versions,
            install_curseforge_pack,
            import_pack_file,
            list_screenshots,
            pick_pack_file,
            pick_image,
            player_info,
            refresh_skin,
            rename_instance,
            project_details,
            open_url,
            app_version,
            restart_app,
            check_update,
            apply_update,
            set_instance_icon,
            clear_instance_icon,
            list_accounts,
            refresh_account_skins,
            remove_account,
            ms_login,
            get_log,
            clear_log,
            list_worlds,
            list_mods,
            search_mods,
            install_mod, mod_dependencies, modpack_contents, open_instance_folder,
            identify_mods,
            toggle_mod,
            remove_mod,
            list_content,
            toggle_content,
            remove_content,
            version_images,
            open_console,
            running_instances,
            recent_play,
            set_instance_memory,
            system_memory_mb,
            set_instance_sync,
            sync_now,
            add_files,
            search_content,
            install_content,
        ])
        .run(tauri::generate_context!())
        .expect("error while running geffeniuse");
}

#[derive(Serialize)]
struct PlayerInfo {
    signed_in: bool,
    name: String,
    /// `data:` URL of the account's skin; `None` shows the built-in default skin.
    skin: Option<String>,
    slim: bool,
    /// Why the real skin is not shown (shown to the user instead of failing silently).
    skin_error: Option<String>,
}

async fn current_player(app: &App, fetch: bool) -> PlayerInfo {
    let settings = app.settings.lock().await.clone();
    let list = core::auth::load(&app.dirs).await;
    let Some(acc) = settings.active_account.as_ref().and_then(|u| list.iter().find(|a| &a.uuid == u)) else {
        return PlayerInfo { signed_in: false, name: String::new(), skin: None, slim: false, skin_error: None };
    };
    let mut info = PlayerInfo { signed_in: true, name: acc.name.clone(), skin: None, slim: false, skin_error: None };
    let skin = if fetch {
        // A failed or empty answer from Mojang (e.g. the rate limit after switching accounts) keeps the skin on disk.
        match core::skin::ensure(&app.dirs, &app.http, &acc.uuid).await {
            Ok(Some(s)) => Some(s),
            Ok(None) => core::skin::cached(&app.dirs, &acc.uuid).await,
            Err(e) => {
                info.skin_error = Some(err(e));
                core::skin::cached(&app.dirs, &acc.uuid).await
            }
        }
    } else {
        core::skin::cached(&app.dirs, &acc.uuid).await
    };
    match skin {
        Some(s) => match core::skin::data_url(&app.dirs, &acc.uuid).await {
            Ok(url) => (info.skin, info.slim) = (Some(url), s.slim),
            Err(e) => info.skin_error = Some(err(e)),
        },
        None if fetch && info.skin_error.is_none() => info.skin_error = Some("Dein Konto hat keinen eigenen Skin".into()),
        None => {}
    }
    info
}

/// Instant: name and whatever skin is already cached. No network, so the start page never waits.
#[tauri::command]
async fn player_info(app: State<'_, App>) -> R<PlayerInfo> {
    Ok(current_player(&app, false).await)
}

/// Fetches the skin from Mojang when the cache is missing or older than six hours.
#[tauri::command]
async fn refresh_skin(app: State<'_, App>) -> R<PlayerInfo> {
    Ok(current_player(&app, true).await)
}

#[tauri::command]
async fn rename_instance(app: State<'_, App>, id: String, name: String) -> R<()> {
    core::instance::rename(&app.dirs, &id, &name).await.map_err(err)
}

#[tauri::command]
async fn project_details(app: State<'_, App>, source: String, id: String) -> R<core::details::ProjectDetails> {
    let key = {
        let s = app.settings.lock().await;
        if source == "curseforge" && !s.curseforge_usable() {
            return Err("CurseForge is switched off (Options)".into());
        }
        s.curseforge_key.clone()
    };
    core::details::fetch(&app.http, &key, &source, &id).await.map_err(err)
}

#[tauri::command]
async fn check_update(app: State<'_, App>) -> R<Option<core::update::Available>> {
    let client = core::update::client_for(&core::update::manifest_url(), &app.http);
    core::update::check(&client, core::APP_VERSION).await.map_err(err)
}

#[tauri::command]
fn app_version() -> &'static str {
    core::APP_VERSION
}

/// Starts the (possibly just replaced) AppImage again and quits this process.
#[tauri::command]
fn restart_app(handle: AppHandle) -> R<()> {
    let exe = core::update::running_appimage().or_else(core::update::exe_path).ok_or("cannot find the launcher file")?;
    std::process::Command::new(exe).stdin(std::process::Stdio::null()).spawn().map_err(err)?;
    handle.exit(0);
    Ok(())
}

#[tauri::command]
async fn apply_update(app: State<'_, App>) -> R<String> {
    let client = core::update::client_for(&core::update::manifest_url(), &app.http);
    core::update::apply(&client, core::APP_VERSION).await.map_err(err)
}

#[tauri::command]
async fn open_url(url: String) -> R<()> {
    let ok = url.len() <= 2000 && !url.chars().any(|c| c.is_control() || c.is_whitespace()) && reqwest::Url::parse(&url).is_ok_and(|u| u.scheme() == "https");
    if !ok {
        return Err("only https links can be opened".into());
    }
    #[cfg(target_os = "linux")]
    let mut cmd = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler").creation_flags(0x0800_0000);
        c
    };
    cmd.arg(&url).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map(|_| ()).map_err(err)
}

const INSTANCE_FOLDERS: &[&str] = &["", "mods", "resourcepacks", "shaderpacks", "saves", "screenshots", "logs", "config"];

#[tauri::command]
async fn open_instance_folder(app: State<'_, App>, id: String, folder: String) -> R<()> {
    if !core::paths::valid_id(&id) || !INSTANCE_FOLDERS.contains(&folder.as_str()) {
        return Err("unknown folder".into());
    }
    let dir = core::instance::game_dir(&app.dirs, &id).join(&folder);
    tokio::fs::create_dir_all(&dir).await.map_err(err)?;
    #[cfg(target_os = "linux")]
    let mut cmd = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("explorer");
        c.creation_flags(0x0800_0000);
        c
    };
    cmd.arg(&dir).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    cmd.spawn().map(|_| ()).map_err(err)
}

#[tauri::command]
async fn pick_image(handle: AppHandle) -> R<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let f = handle.dialog().file().add_filter("Bild", &["png", "jpg", "jpeg", "webp"]).blocking_pick_file();
    Ok(f.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

#[tauri::command]
async fn set_instance_icon(app: State<'_, App>, id: String, path: String) -> R<()> {
    core::instance::set_icon(&app.dirs, &id, std::path::Path::new(&path)).await.map_err(err)
}

#[tauri::command]
async fn clear_instance_icon(app: State<'_, App>, id: String) -> R<()> {
    core::instance::clear_icon(&app.dirs, &id).await.map_err(err)
}

#[tauri::command]
async fn pick_pack_file(handle: AppHandle) -> R<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let f = handle.dialog().file().add_filter("Modpack", &["mrpack", "zip"]).blocking_pick_file();
    Ok(f.and_then(|p| p.into_path().ok()).map(|p| p.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn progress_is_throttled_but_first_and_last_pass() {
        let last = AtomicU64::new(0);
        let p = |done| Progress { done, total: 1000 };
        assert!(progress_due(&last, 5, &p(0)), "first event always passes");
        assert!(!progress_due(&last, 50, &p(10)));
        assert!(!progress_due(&last, 104, &p(11)));
        assert!(progress_due(&last, 105, &p(12)), "100 ms after the last emit");
        assert!(!progress_due(&last, 110, &p(13)));
        assert!(progress_due(&last, 111, &p(1000)), "completion always passes");
    }

    #[tokio::test]
    async fn cancel_wins_even_when_signalled_before_waiting() {
        let n = tokio::sync::Notify::new();
        n.notify_one();
        let slow = tokio::time::sleep(std::time::Duration::from_secs(30));
        let r = tokio::select! { () = slow => "finished", () = n.notified() => "cancelled" };
        assert_eq!(r, "cancelled");
    }

    #[tokio::test]
    async fn cancel_interrupts_a_running_wait() {
        let n = std::sync::Arc::new(tokio::sync::Notify::new());
        let n2 = n.clone();
        tokio::spawn(async move { tokio::time::sleep(std::time::Duration::from_millis(20)).await; n2.notify_one(); });
        let r = tokio::select! { () = tokio::time::sleep(std::time::Duration::from_secs(30)) => "finished", () = n.notified() => "cancelled" };
        assert_eq!(r, "cancelled");
    }

    #[test]
    fn protocol_routes_work_for_both_url_shapes() {
        assert_eq!(protocol_route(Some("icon"), "/inst1/icon-1.png"), Some(("icon", vec!["inst1", "icon-1.png"])));
        assert_eq!(protocol_route(Some("geffeniuse.localhost"), "/icon/inst1/icon-1.png"), Some(("icon", vec!["inst1", "icon-1.png"])));
        assert_eq!(protocol_route(Some("shot"), "/a/b.png"), Some(("shot", vec!["a", "b.png"])));
        assert_eq!(protocol_route(Some("evil.example"), "/icon/a/b"), None);
        assert_eq!(protocol_route(None, "/icon/a/b"), None);
        assert_eq!(protocol_route(Some("geffeniuse.localhost"), ""), Some(("", vec![])), "an empty path routes to a kind that serves nothing");
    }
}
