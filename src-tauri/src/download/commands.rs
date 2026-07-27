use std::time::Duration;

use anyhow::{Context, anyhow};
use tauri::{Manager, State};

use limedl_core::aria2_rpc::Aria2RpcServer;
use limedl_core::{
    error::extract_kind_from_anyhow,
    lock,
    manager::{AppState, DownloadManager},
    settings::{normalize_tracker_list_lossy, normalize_tracker_list_url},
    types::{
        AppSettings, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtTrackerInfo,
        DiskType, DownloadSnapshot, DownloadSummary, Priority, SerializableError, StartDownloadRequest, TaskId,
        TorrentFileEntry,
    },
    Dispatcher,
};
use serde_json::json;

type CommandResult<T> = std::result::Result<T, SerializableError>;

fn into_command_result<T>(result: anyhow::Result<T>) -> CommandResult<T> {
    result.map_err(|error| {
        let kind = extract_kind_from_anyhow(&error).to_string();
        let message = format_anyhow_chain(error);
        SerializableError { kind, message }
    })
}

/// Shim: convert a dispatcher `Result<T, DownloadError>` into `CommandResult<T>`.
fn map_dl_err<T>(result: std::result::Result<T, limedl_core::DownloadError>) -> CommandResult<T> {
    into_command_result(result.map_err(|e| anyhow!(e)))
}

fn format_anyhow_chain(error: anyhow::Error) -> String {
    let mut chain = error.chain();
    let mut messages = Vec::new();
    if let Some(first) = chain.next() {
        messages.push(first.to_string());
    }
    for cause in chain {
        let cause = cause.to_string();
        if messages.last().is_none_or(|last| last != &cause) {
            messages.push(cause);
        }
    }
    messages.join(": ")
}

fn internal_error(msg: &str) -> anyhow::Error {
    anyhow!(msg.to_string())
}

fn make_dispatcher(state: &AppState) -> Dispatcher {
    Dispatcher::new(state.registry.clone(), state.event_bus.clone())
}

#[tauri::command]
pub async fn download_start(
    state: State<'_, AppState>,
    mut request: StartDownloadRequest,
) -> CommandResult<TaskId> {
    let result = into_command_result(
        async {
            // Populate mirror URLs from settings before starting
            let mirror_urls = state
                .registry
                .get_typed::<DownloadManager>()
                .ok_or_else(|| internal_error("HTTP backend not found"))?
                .mirror_urls_for(&request.url)
                .await;
            if mirror_urls.len() > 1 {
                request.mirror_urls = Some(mirror_urls);
            }

            let dispatcher = make_dispatcher(&state);
            dispatcher.start(request).await.map_err(|e| anyhow!(e))
        }
        .await,
    );
    // Emit after start so the frontend gets the initial summary immediately.
    // (BT backend already emits via emit_pending_summary; extra emit is harmless.)
    if let Ok(ref task_id) = result {
        let dispatcher = make_dispatcher(&state);
        if let Ok(snapshot) = dispatcher.status(task_id).await {
            dispatcher.emit_updated(&snapshot);
        }
    }
    result
}

#[tauri::command]
pub async fn download_pause(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).pause(&task_id).await)
}

#[tauri::command]
pub async fn download_resume(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).resume(&task_id).await)
}

#[tauri::command]
pub async fn download_cancel(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).cancel(&task_id).await)
}

#[tauri::command]
pub async fn download_remove(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).remove(&task_id).await)
}

#[tauri::command]
pub async fn download_purge(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).purge(&task_id).await)
}

#[tauri::command]
pub async fn download_open_in_explorer(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<()> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    into_command_result(
        async {
            let backend = state
                .registry
                .dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend
                .open_in_explorer(&task_id)
                .await
                .context("在资源管理器打开下载任务失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn download_open_file(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<()> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    into_command_result(
        async {
            let backend = state
                .registry
                .dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend
                .open_file(&task_id)
                .await
                .context("打开下载文件失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn download_open_dir(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<()> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    into_command_result(
        async {
            let backend = state
                .registry
                .dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend
                .open_dir(&task_id)
                .await
                .context("打开下载目录失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn download_status(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).status(&task_id).await)
}

#[tauri::command]
pub async fn download_list(state: State<'_, AppState>) -> CommandResult<Vec<DownloadSummary>> {
    map_dl_err(make_dispatcher(&state).list().await)
}

#[tauri::command]
pub async fn download_set_priority(
    state: State<'_, AppState>,
    download_id: String,
    priority: Priority,
) -> CommandResult<()> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).set_priority(&task_id, priority).await)
}

#[tauri::command]
pub async fn bt_runtime_status(state: State<'_, AppState>) -> CommandResult<BtRuntimeStatus> {
    map_dl_err(make_dispatcher(&state).bt_runtime_status())
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> CommandResult<AppSettings> {
    into_command_result(
        async {
            let dm = state
                .registry
                .get_typed::<DownloadManager>()
                .ok_or_else(|| internal_error("HTTP backend not found"))?;
            dm.settings().await.context("读取设置失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn settings_save(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CommandResult<AppSettings> {
    into_command_result(
        async {
            let dm = state
                .registry
                .get_typed::<DownloadManager>()
                .ok_or_else(|| internal_error("HTTP backend not found"))?;

            let old_rpc = dm.settings().await.context("读取当前设置失败")?.aria2_rpc;

            // Broadcast settings to all backends (each backend extracts its subset)
            state
                .registry
                .update_all_settings(&settings)
                .await
                .context("保存设置失败")?;

            // Re-read normalized/saved settings for the return value
            let saved = dm.settings().await.context("读取设置失败")?;

            // Sync CDN acceleration settings — clear accelerator when disabled
            if !saved.cdn_acceleration.enabled {
                state.cdn_service.clear().await;
            }

            let new_rpc = &saved.aria2_rpc;
            if old_rpc != *new_rpc {
                // Signal existing RPC server to shut down gracefully
                if let Some(tx) = lock(&state.rpc_shutdown).take() {
                    let _ = tx.send(true);
                }
                if new_rpc.enabled {
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    let rpc_server = Aria2RpcServer::new(
                        state.registry.clone(),
                        new_rpc,
                        state.event_bus.clone(),
                    );
                    let cors_origins = new_rpc.cors_allowed_origins.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = rpc_server.serve(rx, cors_origins).await {
                            tracing::error!("Aria2 RPC server stopped: {error}");
                        }
                    });
                    *lock(&state.rpc_shutdown) = Some(tx);
                    tracing::info!("Aria2 RPC 服务器已重启 (port: {})", new_rpc.port);
                } else {
                    tracing::info!("Aria2 RPC 服务器已停止");
                }
            }

            Ok(saved)
        }
        .await,
    )
}

#[tauri::command]
pub async fn settings_fetch_tracker_list(
    state: State<'_, AppState>,
    tracker_list_url: String,
) -> CommandResult<String> {
    const MAX_TRACKER_LIST_BYTES: usize = 1024 * 1024;

    into_command_result(
        async {
            let tracker_list_url =
                normalize_tracker_list_url(&tracker_list_url).context("Tracker 列表 URL 无效")?;
            let response = state
                .http_client
                .get(tracker_list_url)
                .send()
                .await
                .context("下载 Tracker 列表失败")?
                .error_for_status()
                .context("Tracker 列表返回了错误状态码")?;

            let bytes = response
                .bytes()
                .await
                .context("读取 Tracker 列表响应体失败")?;
            if bytes.len() > MAX_TRACKER_LIST_BYTES {
                return Err(anyhow!("tracker list is larger than 1 MiB"));
            }

            let content =
                String::from_utf8(bytes.to_vec()).context("Tracker 列表不是有效 UTF-8")?;
            Ok(normalize_tracker_list_lossy(&content))
        }
        .await,
    )
}

/// Set per-torrent speed limits.
#[tauri::command]
pub async fn bt_set_speed_limit(
    state: State<'_, AppState>,
    download_id: String,
    download_limit_bps: Option<u64>,
    upload_limit_bps: Option<u64>,
) -> CommandResult<()> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(
        make_dispatcher(&state)
            .bt_set_speed_limit(&task_id, download_limit_bps, upload_limit_bps),
    )
}

#[tauri::command]
pub async fn bt_preview_torrent(
    state: State<'_, AppState>,
    source: String,
) -> CommandResult<Vec<TorrentFileEntry>> {
    map_dl_err(make_dispatcher(&state).bt_preview_torrent(&source).await)
}

#[tauri::command]
pub async fn bt_get_peers(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtPeerInfo>> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).bt_get_peers(&task_id))
}

#[tauri::command]
pub async fn bt_get_trackers(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtTrackerInfo>> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).bt_get_trackers(&task_id))
}

#[tauri::command]
pub async fn bt_get_pieces(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtPieceInfo>> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).bt_get_pieces(&task_id))
}

#[tauri::command]
pub async fn get_bt_files(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtFileStatus>> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).bt_get_files(&task_id))
}

#[tauri::command]
pub async fn update_bt_files(
    state: State<'_, AppState>,
    download_id: String,
    included_indices: Vec<usize>,
) -> CommandResult<()> {
    let task_id = TaskId::from_legacy_string(&download_id).map_err(|e| SerializableError {
        kind: "parse".into(),
        message: format!("Invalid task ID: {e}"),
    })?;
    map_dl_err(make_dispatcher(&state).bt_update_files(&task_id, included_indices).await)
}

#[tauri::command]
pub async fn toggle_game_mode(state: State<'_, AppState>, enabled: bool) -> CommandResult<bool> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    dm.set_game_mode(enabled);
    Ok(enabled)
}

#[tauri::command]
pub async fn get_io_status(state: State<'_, AppState>) -> CommandResult<serde_json::Value> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    let pool = &dm.buffer_pool;
    Ok(json!({
        "gameMode": pool.game_mode(),
        "bufferUsageBytes": pool.current_usage(),
        "bufferLimitBytes": pool.effective_limit(),
        "activeSlots": pool.active_slots(),
        "maxSlots": pool.max_slots(),
        "queuedCount": pool.queued_count(),
        "degradationCount": pool.degradation_count(),
    }))
}

#[tauri::command]
pub async fn toggle_overclock_mode(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<bool> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    dm.set_overclock_mode(enabled);
    Ok(enabled)
}

#[tauri::command]
pub async fn get_overclock_mode(state: State<'_, AppState>) -> CommandResult<bool> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    Ok(dm.overclock_mode())
}

#[tauri::command]
pub async fn detect_disk_type(
    state: State<'_, AppState>,
    dir: String,
) -> CommandResult<String> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    let disk_type = dm.resolve_disk_type(std::path::Path::new(&dir)).await;
    Ok(match disk_type {
        DiskType::Hdd => "hdd".to_string(),
        DiskType::Ssd => "ssd".to_string(),
    })
}

#[tauri::command]
pub async fn detect_all_disk_types() -> CommandResult<std::collections::HashMap<String, String>> {
    let disk_types = limedl_core::file_ops::detect_all_disk_types();
    Ok(disk_types
        .into_iter()
        .map(|(drive, dt)| {
            (
                drive,
                match dt {
                    DiskType::Hdd => "hdd".to_string(),
                    DiskType::Ssd => "ssd".to_string(),
                },
            )
        })
        .collect())
}

/// Factory reset: deletes all application data and restores factory defaults.
/// After this returns, the frontend must restart the app (backends are shut down).
#[tauri::command]
pub async fn factory_reset(app: tauri::AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    into_command_result(
        async {
            // 1. Shut down all backends (stops active downloads, releases file handles)
            state.registry.shutdown_all().await;

            // 2. Compute the parent directory (contains downloads/ and settings.json)
            let parent_dir = state_dir_parent(&app);

            // 3. Delete the entire parent directory with retry for Windows file-locking.
            // Attempt removal directly (no existence check) to eliminate the TOCTOU
            // window between check and removal.
            for attempt in 0..3 {
                match std::fs::remove_dir_all(&parent_dir) {
                    Ok(_) => break,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                    Err(_e) if attempt < 2 => {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        continue;
                    }
                    Err(e) => return Err(e).context("failed to delete data directory"),
                }
            }

            Ok(())
        }
        .await,
    )
}

/// Returns the parent directory of the application's state storage.
/// Contains both the `downloads/` subdirectory and `settings.json`.
fn state_dir_parent(app: &tauri::AppHandle) -> std::path::PathBuf {
    let downloads_dir = app
        .path()
        .app_local_data_dir()
        .or_else(|_| app.path().app_data_dir())
        .unwrap_or_else(|_| std::env::temp_dir().join("limedl"))
        .join("downloads");
    // Parent directory holds both downloads/ and settings.json
    downloads_dir
        .parent()
        .unwrap_or(&downloads_dir)
        .to_path_buf()
}

#[cfg(test)]
#[path = "tests/commands_tests.rs"]
mod tests;
