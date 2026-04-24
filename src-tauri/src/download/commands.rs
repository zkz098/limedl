use tauri::State;

use super::{
    manager::AppState,
    types::{AppSettings, DownloadSnapshot, DownloadSummary, StartDownloadRequest},
};

#[tauri::command]
pub async fn download_start(
    state: State<'_, AppState>,
    request: StartDownloadRequest,
) -> Result<String, String> {
    state
        .manager
        .start(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_pause(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    state
        .manager
        .pause(&download_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_resume(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    state
        .manager
        .resume(&download_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_cancel(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    state
        .manager
        .cancel(&download_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_remove(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    state
        .manager
        .remove(&download_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_purge(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    state
        .manager
        .purge(&download_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_open_in_explorer(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<(), String> {
    state
        .manager
        .open_in_explorer(&download_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_status(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    state
        .manager
        .status(&download_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_list(state: State<'_, AppState>) -> Result<Vec<DownloadSummary>, String> {
    state
        .manager
        .list()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    state
        .manager
        .settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn settings_save(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    state
        .manager
        .update_settings(settings)
        .await
        .map_err(|error| error.to_string())
}
