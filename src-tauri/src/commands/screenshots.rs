use crate::*;

#[tauri::command]
pub(crate) async fn list_screenshots(app: State<'_, App>) -> R<Vec<core::screenshots::Shot>> {
    core::screenshots::list(&app.dirs).await.map_err(err)
}
