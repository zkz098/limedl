use tauri::State;

use super::{
    manager::AppState,
    torrent::{
        DownloadSourceKind, classify_download_source, http_task_id, is_bt_task_id,
        normalize_http_task_id,
    },
    types::{AppSettings, DownloadSnapshot, DownloadSummary, StartDownloadRequest},
};

#[tauri::command]
pub async fn download_start(
    state: State<'_, AppState>,
    request: StartDownloadRequest,
) -> Result<String, String> {
    match classify_download_source(&request).map_err(|error| error.to_string())? {
        DownloadSourceKind::Http => state
            .manager
            .start(request)
            .await
            .map(http_task_id)
            .map_err(|error| error.to_string()),
        DownloadSourceKind::Torrent => state
            .torrent_manager
            .start(request)
            .await
            .map_err(|error| error.to_string()),
    }
}

#[tauri::command]
pub async fn download_pause(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    if is_bt_task_id(&download_id) {
        return state
            .torrent_manager
            .pause(&download_id)
            .await
            .map_err(|error| error.to_string());
    }

    state
        .manager
        .pause(normalize_http_task_id(&download_id))
        .await
        .map(prefix_http_snapshot)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_resume(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    if is_bt_task_id(&download_id) {
        return state
            .torrent_manager
            .resume(&download_id)
            .await
            .map_err(|error| error.to_string());
    }

    state
        .manager
        .resume(normalize_http_task_id(&download_id))
        .await
        .map(prefix_http_snapshot)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_cancel(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    if is_bt_task_id(&download_id) {
        return state
            .torrent_manager
            .cancel(&download_id)
            .await
            .map_err(|error| error.to_string());
    }

    state
        .manager
        .cancel(normalize_http_task_id(&download_id))
        .await
        .map(prefix_http_snapshot)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_remove(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    if is_bt_task_id(&download_id) {
        return state
            .torrent_manager
            .remove(&download_id)
            .await
            .map_err(|error| error.to_string());
    }

    state
        .manager
        .remove(normalize_http_task_id(&download_id))
        .await
        .map(prefix_http_snapshot)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_purge(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    if is_bt_task_id(&download_id) {
        return state
            .torrent_manager
            .purge(&download_id)
            .await
            .map_err(|error| error.to_string());
    }

    state
        .manager
        .purge(normalize_http_task_id(&download_id))
        .await
        .map(prefix_http_snapshot)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_open_in_explorer(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<(), String> {
    if is_bt_task_id(&download_id) {
        return state
            .torrent_manager
            .open_in_explorer(&download_id)
            .await
            .map_err(|error| error.to_string());
    }

    state
        .manager
        .open_in_explorer(normalize_http_task_id(&download_id))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_status(
    state: State<'_, AppState>,
    download_id: String,
) -> Result<DownloadSnapshot, String> {
    if is_bt_task_id(&download_id) {
        return state
            .torrent_manager
            .status(&download_id)
            .await
            .map_err(|error| error.to_string());
    }

    state
        .manager
        .status(normalize_http_task_id(&download_id))
        .await
        .map(prefix_http_snapshot)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn download_list(state: State<'_, AppState>) -> Result<Vec<DownloadSummary>, String> {
    let mut downloads = state
        .manager
        .list()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|mut summary| {
            summary.id = http_task_id(summary.id);
            summary
        })
        .collect::<Vec<_>>();
    downloads.extend(
        state
            .torrent_manager
            .list()
            .await
            .map_err(|error| error.to_string())?,
    );
    downloads.sort_by(|left, right| right.id.cmp(&left.id));
    Ok(downloads)
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
    let saved = state
        .manager
        .update_settings(settings)
        .await
        .map_err(|error| error.to_string())?;
    state.torrent_manager.update_settings(&saved);
    Ok(saved)
}

fn prefix_http_snapshot(mut snapshot: DownloadSnapshot) -> DownloadSnapshot {
    snapshot.id = http_task_id(snapshot.id);
    snapshot
}
