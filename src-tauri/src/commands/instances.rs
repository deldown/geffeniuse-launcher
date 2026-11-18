use crate::*;

#[tauri::command]
pub(crate) async fn list_instances(app: State<'_, App>) -> R<Vec<Instance>> {
    core::instance::list(&app.dirs).await.map_err(err)
}

#[derive(Serialize)]
pub(crate) struct VersionOption {
    pub(crate) id: String,
    pub(crate) category: &'static str,
}

#[tauri::command]
pub(crate) async fn minecraft_versions(app: State<'_, App>) -> R<Vec<VersionOption>> {
    let m = core::mojang::manifest(&app.http).await.map_err(err)?;
    let (snap, exp, april) = {
        let s = app.settings.lock().await;
        (s.show_snapshots, s.show_experimental, s.show_april_fools)
    };
    Ok(m
        .versions
        .iter()
        .filter_map(|v| core::mojang::category(v).map(|c| VersionOption { id: v.id.clone(), category: c }))
        .filter(|v| match v.category {
            "release" => true,
            "snapshot" => snap,
            "experimental" => exp,
            _ => april,
        })
        .collect())
}

#[tauri::command]
pub(crate) async fn create_instance(app: State<'_, App>, name: String, mc_version: String, loader: String) -> R<Instance> {
    let loader = Loader::parse(&loader).ok_or("unknown loader")?;
    let inst = core::instance::create(&app.dirs, &name, &mc_version, loader, None, None).await.map_err(err)?;
    if app.settings.lock().await.auto_api_mod {
        if let Some(api) = core::mods::api_mod(loader) {
            // A missing API mod must not block the new modpack; it can still be added under "Add".
            if let Err(e) = core::mods::install(&app.dirs, &app.http, &inst, api, true).await {
                eprintln!("api mod for {}: {e:#}", inst.id);
            }
        }
    }
    Ok(inst)
}

#[tauri::command]
pub(crate) async fn duplicate_instance(app: State<'_, App>, id: String) -> R<Instance> {
    core::transfer::duplicate(&app.dirs, &id).await.map_err(err)
}

/// Asks where to save, then writes the modpack there. `None` means the dialog was cancelled.
#[tauri::command]
pub(crate) async fn export_instance(app: State<'_, App>, handle: AppHandle, id: String, format: String) -> R<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let format = core::transfer::Format::parse(&format).ok_or("unknown format")?;
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    let stem: String = inst.name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let (label, ext) = (if format == core::transfer::Format::Mrpack { "Modrinth-Modpack" } else { "Sicherung (zip)" }, format.extension());
    let picked = handle.dialog().file().add_filter(label, &[ext]).set_file_name(format!("{stem}.{ext}")).blocking_save_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    core::transfer::export(&app.dirs, &app.http, &id, format, &path).await.map_err(err)?;
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
pub(crate) async fn check_mod_updates(app: State<'_, App>, id: String) -> R<Vec<core::mods::ModUpdate>> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    core::mods::check_updates(&app.dirs, &app.http, &inst).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn update_mods(app: State<'_, App>, id: String, files: Vec<String>) -> R<Vec<String>> {
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    core::mods::update(&app.dirs, &app.http, &inst, &files).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn set_content_group(app: State<'_, App>, id: String, files: Vec<String>, group: Option<String>) -> R<()> {
    core::groups::set(&app.dirs, &id, &files, group.as_deref()).await.map_err(err)
}

#[tauri::command]
pub(crate) async fn delete_instance(app: State<'_, App>, id: String) -> R<()> {
    core::instance::delete(&app.dirs, &id).await.map_err(err)
}

/// Everything that happens once a game is gone, whether this launcher started it or took it over after a restart.
/// A clean end, or one the user asked for, closes the console window; a crash leaves it open to read.
pub(crate) async fn finish_run(handle: &AppHandle, id: &str, code: Option<i32>, killed: bool, restore: bool) {
    let state = handle.state::<App>();
    let dirs = state.dirs.clone();
    if state.settings.lock().await.sync_options {
        if let Ok(inst) = core::instance::get(&dirs, id).await {
            if inst.sync_settings {
                if let Err(e) = core::sync::pull_from_instance(&dirs, &inst).await {
                    eprintln!("settings sync (after exit): {e:#}");
                }
            }
        }
    }
    core::running::clear(&dirs, id).await;
    state.runs.lock().unwrap().remove(id);
    state.discord.game(id, false);
    if restore {
        if let Some(w) = handle.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    }
    let crashed = !killed && code != Some(0);
    if !crashed {
        if let Some(w) = handle.get_webview_window(&format!("console-{id}")) {
            let _ = w.close();
        }
    }
    let _ = handle.emit("game-exited", (id.to_string(), code));
    if crashed {
        let lines: Vec<String> = state.log_of(id).lock().unwrap().iter().cloned().collect();
        if let Some(hint) = core::crash::diagnose(&lines, code) {
            let _ = handle.emit("game-crashed", (id.to_string(), hint));
        }
    }
}

/// A game the previous launcher session started: watch it until it ends, and stop it on request.
pub(crate) fn adopt_run(handle: AppHandle, runtime: tokio::runtime::Handle, found: core::running::Found) {
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    if found.alive {
        handle.state::<App>().discord.game(&found.id, true);
        handle.state::<App>().runs.lock().unwrap().insert(found.id.clone(), cancel.clone());
    }
    runtime.spawn(async move {
        let dirs = handle.state::<App>().dirs.clone();
        let (id, state) = (found.id, found.state);
        if !found.alive {
            let end = core::running::ended_at(&dirs, &id, &state);
            let _ = core::instance::add_playtime(&dirs, &id, end.saturating_sub(state.last_saved)).await;
            finish_run(&handle, &id, None, false, false).await;
            return;
        }
        let game = core::instance::game_dir(&dirs, &id);
        let mut last = state.last_saved;
        let mut killed = false;
        // The time the launcher was closed counts as played.
        let now = core::running::now();
        let _ = core::instance::add_playtime(&dirs, &id, now.saturating_sub(last)).await;
        last = now;
        let _ = core::running::write(&dirs, &id, &core::running::RunState { last_saved: last, ..state }).await;
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
        let mut stop_asked: Option<std::time::Instant> = None;
        loop {
            tokio::select! {
                () = cancel.notified() => { killed = true; stop_asked.get_or_insert_with(std::time::Instant::now); core::running::kill(state.pid, false); }
                _ = tick.tick() => {
                    if !core::running::alive(state.pid, &game, state.started) {
                        break;
                    }
                    // A process that ignores the request, or that we cannot end, must not leave the button stuck.
                    match stop_asked.map(|t| t.elapsed().as_secs()) {
                        Some(s) if s >= 15 => break,
                        Some(s) if s >= 10 => core::running::kill(state.pid, true),
                        _ => {}
                    }
                    let now = core::running::now();
                    if now.saturating_sub(last) >= 60 {
                        let _ = core::instance::add_playtime(&dirs, &id, now - last).await;
                        last = now;
                        let _ = core::running::write(&dirs, &id, &core::running::RunState { last_saved: last, ..state }).await;
                    }
                }
            }
        }
        let _ = core::instance::add_playtime(&dirs, &id, core::running::now().saturating_sub(last)).await;
        finish_run(&handle, &id, None, killed, false).await;
    });
}

/// Ids of the instances whose game is running right now (not just preparing).
#[tauri::command]
pub(crate) async fn running_instances(app: State<'_, App>) -> R<Vec<String>> {
    let ids: Vec<String> = app.runs.lock().unwrap().keys().cloned().collect();
    let mut out = Vec::new();
    for id in ids {
        if core::running::read(&app.dirs, &id).await.is_some() {
            out.push(id);
        }
    }
    Ok(out)
}

#[tauri::command]
pub(crate) async fn launch_instance(app: State<'_, App>, handle: AppHandle, id: String, join: Option<core::launch::Join>, extra: Option<bool>) -> R<Option<&'static str>> {
    if extra == Some(true) {
        return launch_extra(&app, &handle, &id).await.map(|()| None);
    }
    let inst = core::instance::get(&app.dirs, &id).await.map_err(err)?;
    let (join_args, note) = match &join {
        None => (Vec::new(), None),
        Some(j) => {
            let world_ok = match j {
                core::launch::Join::World(name) => core::paths::file_name_ok(name) && core::worlds::is_world(&core::instance::game_dir(&app.dirs, &id).join("saves").join(name)),
                core::launch::Join::Server(_) => true,
            };
            match core::launch::join_args(&inst.mc_version, j) {
                Some(a) if world_ok => (a, None),
                _ => (Vec::new(), Some("join-unsupported")),
            }
        }
    };
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    {
        let mut runs = app.runs.lock().unwrap();
        if runs.contains_key(&id) {
            return Err("This instance is already starting or running".into());
        }
        runs.insert(id.clone(), cancel.clone());
    }
    let result = start_game(&app, &handle, &inst, &cancel, join_args, 0).await;
    match result {
        Ok(Some(child)) => {
            let (after, console) = {
                let s = app.settings.lock().await;
                (s.after_start.clone(), s.console_window)
            };
            if console {
                open_console_window(&handle, &id, &inst.name);
            }
            // Closing the launcher would end the console and the playtime count, so with a console window it only minimizes.
            let after = if console && after == "close" { "minimize".to_string() } else { after };
            match after.as_str() {
                "minimize" => {
                    if let Some(w) = handle.get_webview_window("main") {
                        let _ = w.minimize();
                    }
                }
                "close" => {
                    let h = handle.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                        h.exit(0);
                    });
                }
                _ => {}
            }
            let (h, id2, cancel2, app_handle) = (handle.clone(), id.clone(), cancel.clone(), handle.clone());
            let restore = after == "minimize";
            let pid = child.id().unwrap_or(0);
            let started_unix = core::running::now();
            if pid != 0 {
                let _ = core::running::write(&app.dirs, &id, &core::running::RunState { pid, started: started_unix, last_saved: started_unix }).await;
            }
            tokio::spawn(async move {
                let mut child = child;
                let dirs = app_handle.state::<App>().dirs.clone();
                let started = std::time::Instant::now();
                let mut counted = 0u64;
                let mut killed = false;
                // Playtime is saved every minute, so a crash or a closed launcher loses at most 60 s.
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
                tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                tick.tick().await;
                let code = loop {
                    tokio::select! {
                        s = child.wait() => break s.ok().and_then(|s| s.code()),
                        () = cancel2.notified() => { killed = true; let _ = child.kill().await; break None; }
                        _ = tick.tick() => {
                            let now = started.elapsed().as_secs();
                            let _ = core::instance::add_playtime(&dirs, &id2, now - counted).await;
                            counted = now;
                            if pid != 0 {
                                let _ = core::running::write(&dirs, &id2, &core::running::RunState { pid, started: started_unix, last_saved: core::running::now() }).await;
                            }
                        }
                    }
                };
                let _ = core::instance::add_playtime(&dirs, &id2, started.elapsed().as_secs() - counted).await;
                finish_run(&h, &id2, code, killed, restore).await;
            });
            Ok(note)
        }
        other => {
            app.runs.lock().unwrap().remove(&id);
            let _ = handle.emit("game-exited", (id.clone(), None::<i32>));
            other.map(|_| None)
        }
    }
}

/// Prepares and spawns the game. `Ok(None)` means the user cancelled while it was preparing.
pub(crate) async fn start_game(app: &App, handle: &AppHandle, inst: &Instance, cancel: &tokio::sync::Notify, join_args: Vec<String>, slot: u32) -> R<Option<tokio::process::Child>> {
    let settings = app.settings.lock().await.clone();
    let h = handle.clone();
    let prepare = core::launch::prepare_slot(&app.dirs, &app.http, &settings, inst, resolve_account(app, &settings), slot, move |stage, p| emit_progress(&h, stage, p));
    let mut prepared = tokio::select! {
        r = prepare => r.map_err(err)?,
        () = cancel.notified() => return Ok(None),
    };
    prepared.args.extend(join_args);
    if slot == 0 && settings.sync_options && inst.sync_settings {
        if let Err(e) = core::sync::push_to_instance(&app.dirs, inst).await {
            eprintln!("settings sync (before start): {e:#}");
        }
    }
    let mut child = core::launch::spawn(&prepared).await.map_err(err)?;
    let presence_key = if slot == 0 { inst.id.clone() } else { format!("{}#{slot}", inst.id) };
    app.discord.game(&presence_key, true);
    let _ = core::instance::touch(&app.dirs, &inst.id).await;
    let log = app.log_of(&inst.id);
    let prefix = if slot == 0 { String::new() } else { format!("[Fenster {}] ", slot + 1) };
    if slot == 0 {
        log.lock().unwrap().clear();
    }
    if let Some(o) = child.stdout.take() {
        pump_log(o, log.clone(), handle.clone(), inst.id.clone(), app.dirs.clone(), prefix.clone());
    }
    if let Some(e) = child.stderr.take() {
        pump_log(e, log.clone(), handle.clone(), inst.id.clone(), app.dirs.clone(), prefix);
    }
    if slot == 0 {
        let _ = handle.emit("game-started", &inst.id);
    }
    Ok(Some(child))
}

#[tauri::command]
pub(crate) fn stop_instance(app: State<'_, App>, id: String) {
    let extra = format!("{id}#");
    for (key, n) in app.runs.lock().unwrap().iter() {
        if *key == id || key.starts_with(&extra) {
            n.notify_one();
        }
    }
}

pub(crate) const MAX_WINDOWS: u32 = 4;

/// Starts one more game window of a modpack whose first window is already running.
/// Extra windows use their own natives folder, are never adopted after a launcher restart and do not
/// count playtime or touch the shared settings.
pub(crate) async fn launch_extra(app: &App, handle: &AppHandle, id: &str) -> R<()> {
    if !app.settings.lock().await.multi_start {
        return Err("Several windows are switched off (Options)".into());
    }
    let inst = core::instance::get(&app.dirs, id).await.map_err(err)?;
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    let slot = {
        let mut runs = app.runs.lock().unwrap();
        if !runs.contains_key(id) {
            return Err("Start the modpack first".into());
        }
        let Some(slot) = (1..MAX_WINDOWS).find(|n| !runs.contains_key(&format!("{id}#{n}"))) else {
            return Err(format!("At most {MAX_WINDOWS} windows per modpack"));
        };
        runs.insert(format!("{id}#{slot}"), cancel.clone());
        slot
    };
    let key = format!("{id}#{slot}");
    let done = |app: &App| {
        app.runs.lock().unwrap().remove(&key);
        let dirs = app.dirs.clone();
        let (id, handle) = (id.to_string(), handle.clone());
        tokio::spawn(async move {
            let _ = tokio::fs::remove_dir_all(core::launch::natives_dir(&dirs, &id, slot)).await;
            let _ = handle.emit("extra-exited", &id);
        });
    };
    match start_game(app, handle, &inst, &cancel, Vec::new(), slot).await {
        Ok(Some(mut child)) => {
            let (h, key2, id2) = (handle.clone(), key.clone(), id.to_string());
            tokio::spawn(async move {
                let (handle, key, id) = (h, key2, id2);
                tokio::select! {
                    _ = child.wait() => {}
                    () = cancel.notified() => { let _ = child.kill().await; }
                }
                let app = handle.state::<App>();
                app.runs.lock().unwrap().remove(&key);
                app.discord.game(&key, false);
                let _ = tokio::fs::remove_dir_all(core::launch::natives_dir(&app.dirs, &id, slot)).await;
                let _ = handle.emit("extra-exited", &id);
            });
            let _ = handle.emit("extra-started", id);
            Ok(())
        }
        other => {
            done(app);
            other.map(|_| ())
        }
    }
}
