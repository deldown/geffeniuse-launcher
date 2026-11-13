use crate::*;

#[tauri::command]
pub(crate) async fn get_settings(app: State<'_, App>) -> R<Settings> {
    Ok(app.settings.lock().await.clone())
}

#[tauri::command]
pub(crate) async fn save_settings(app: State<'_, App>, settings: Settings) -> R<Settings> {
    let s = settings.clone().sanitized();
    if settings.discord_presence && !core::settings::valid_discord_application_id(settings.discord_application_id.trim()) {
        return Err("Discord Application ID must contain 17–20 digits".into());
    }
    let mut stored = app.settings.lock().await;
    s.save(&app.dirs).await.map_err(err)?;
    *stored = s.clone();
    app.discord.configure(&s);
    Ok(s)
}
