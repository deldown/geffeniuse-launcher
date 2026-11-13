use crate::*;

#[derive(Serialize)]
pub(crate) struct ImportResult {
    pub(crate) instance: Instance,
    pub(crate) manual: Vec<core::packs::ManualFile>,
}

#[tauri::command]
pub(crate) async fn import_pack_file(app: State<'_, App>, handle: AppHandle, path: String) -> R<ImportResult> {
    cancellable(&app, import_pack_file_inner(&app, handle, path)).await
}

pub(crate) async fn import_pack_file_inner(app: &App, handle: AppHandle, path: String) -> R<ImportResult> {
    let p = std::path::PathBuf::from(path);
    let settings = app.settings.lock().await.clone();
    let conc = settings.parallel_downloads as usize;
    let pc = p.clone();
    let kind = tokio::task::spawn_blocking(move || core::packs::detect(&pc)).await.map_err(err)?.map_err(err)?;
    let h = handle.clone();
    let prog = move |pr: Progress| emit_progress(&h, "pack", pr);
    if kind == "mrpack" {
        let instance = core::packs::import_mrpack(&app.dirs, &app.http, &p, conc, prog).await.map_err(err)?;
        let _ = core::mods::identify(&app.dirs, &app.http, &instance.id).await;
        Ok(ImportResult { instance, manual: vec![] })
    } else {
        let cf = cf_client(&settings)?;
        let r = core::packs::import_curseforge_zip(&app.dirs, &app.http, &cf, &p, conc, prog).await.map_err(err)?;
        Ok(ImportResult { instance: r.instance, manual: r.manual })
    }
}
