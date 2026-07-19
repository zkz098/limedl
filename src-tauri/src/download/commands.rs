use std::time::Duration;

use anyhow::{Context, anyhow};
use tauri::State;

use limedl_core::{
    error::extract_kind_from_anyhow,
    event_bus::DownloadEvent,
    lock,
    manager::{AppState, DownloadManager},
    settings::{normalize_tracker_list_lossy, normalize_tracker_list_url},
    types::{
        AppSettings, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtTrackerInfo,
        DownloadSnapshot, DownloadSummary, SerializableError,
        StartDownloadRequest, TaskId, TorrentFileEntry,
    },
};
use super::aria2_rpc::Aria2RpcServer;
use super::IrontideBtBackend;
use serde_json::json;

type CommandResult<T> = std::result::Result<T, SerializableError>;

fn into_command_result<T>(result: anyhow::Result<T>) -> CommandResult<T> {
    result.map_err(|error| {
        let kind = extract_kind_from_anyhow(&error).to_string();
        let message = format_anyhow_chain(error);
        SerializableError { kind, message }
    })
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

/// Emit a single `download-updated` event for the given snapshot.
fn emit_snapshot_update(state: &AppState, snapshot: &DownloadSnapshot) {
    let summary = DownloadSummary::from(snapshot);
    let summary_json = serde_json::to_value(&summary).unwrap_or_default();
    let id = summary.id.clone();
    state.event_bus.publish(DownloadEvent::Updated { id, summary_json });
}

#[tauri::command]
pub async fn download_start(
    state: State<'_, AppState>,
    mut request: StartDownloadRequest,
) -> CommandResult<TaskId> {
    let result = into_command_result(
        async {
            // Populate mirror URLs from settings before starting
            let mirror_urls = state.registry.get_typed::<DownloadManager>()
                .ok_or_else(|| internal_error("HTTP backend not found"))?
                .mirror_urls_for(&request.url).await;
            if mirror_urls.len() > 1 {
                request.mirror_urls = Some(mirror_urls);
            }

            let kind = request.classify_kind().map_err(|e| anyhow!(e)).context("无法识别下载任务类型")?;
            let backend = state.registry.by_kind(kind)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.start(request).await.context("启动下载任务失败")
        }
        .await,
    );
    if let Ok(ref task_id) = result {
        if let Ok(backend) = state.registry.dispatch(task_id)
            && let Ok(snapshot) = backend.status(task_id).await
        {
            emit_snapshot_update(&state, &snapshot);
        }
    }
    result
}

#[tauri::command]
pub async fn download_pause(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    let result = into_command_result(
        async {
            let backend = state.registry.dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.pause(&task_id).await
                .context("暂停下载任务失败")
        }.await,
    );
    if let Ok(ref snapshot) = result {
        emit_snapshot_update(&state, snapshot);
    }
    result
}

#[tauri::command]
pub async fn download_resume(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    let result = into_command_result(
        async {
            let backend = state.registry.dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.resume(&task_id).await
                .context("恢复下载任务失败")
        }.await,
    );
    if let Ok(ref snapshot) = result {
        emit_snapshot_update(&state, snapshot);
    }
    result
}

#[tauri::command]
pub async fn download_cancel(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    into_command_result(
        async {
            let backend = state.registry.dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.cancel(&task_id).await
                .context("取消下载任务失败")
        }.await,
    )
}

#[tauri::command]
pub async fn download_remove(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    into_command_result(
        async {
            let backend = state.registry.dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.remove(&task_id).await
                .context("移除下载任务失败")
        }.await,
    )
}

#[tauri::command]
pub async fn download_purge(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    into_command_result(
        async {
            let backend = state.registry.dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.purge(&task_id).await
                .context("彻底删除下载任务失败")
        }.await,
    )
}

#[tauri::command]
pub async fn download_open_in_explorer(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<()> {
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    into_command_result(
        async {
            let backend = state.registry.dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.open_in_explorer(&task_id).await
                .context("在资源管理器打开下载任务失败")
        }.await,
    )
}

#[tauri::command]
pub async fn download_status(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    into_command_result(
        async {
            let backend = state.registry.dispatch(&task_id)
                .map_err(|e| internal_error(&e.to_string()))?;
            backend.status(&task_id).await
                .context("查询下载任务状态失败")
        }.await,
    )
}

#[tauri::command]
pub async fn download_list(state: State<'_, AppState>) -> CommandResult<Vec<DownloadSummary>> {
    into_command_result(Ok(state.registry.list_all().await))
}

#[tauri::command]
pub async fn bt_runtime_status(state: State<'_, AppState>) -> CommandResult<BtRuntimeStatus> {
    into_command_result(
        async {
            let bt = state.registry.get_typed::<IrontideBtBackend>()
                .ok_or_else(|| internal_error("BT backend not registered"))?;
            Ok(bt.runtime_status())
        }
        .await,
    )
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> CommandResult<AppSettings> {
    into_command_result(
        async {
            let dm = state.registry.get_typed::<DownloadManager>()
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
            let dm = state.registry.get_typed::<DownloadManager>()
                .ok_or_else(|| internal_error("HTTP backend not found"))?;

            let old_rpc = dm
                .settings()
                .await
                .context("读取当前设置失败")?
                .aria2_rpc;

            // Broadcast settings to all backends (each backend extracts its subset)
            state.registry.update_all_settings(&settings).await;

            // Re-read normalized/saved settings for the return value
            let saved = dm.settings().await.context("读取设置失败")?;

            // Sync CDN acceleration settings — clear accelerator when disabled
            if !saved.cdn_acceleration.enabled {
                state.cdn_accelerator.clear().await;
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
pub async fn settings_fetch_tracker_list(tracker_list_url: String) -> CommandResult<String> {
    const MAX_TRACKER_LIST_BYTES: usize = 1024 * 1024;

    into_command_result(
        async {
            let tracker_list_url =
                normalize_tracker_list_url(&tracker_list_url).context("Tracker 列表 URL 无效")?;
            let response = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(5))
                .timeout(Duration::from_secs(15))
                .user_agent("limedl/0.1")
                .build()
                .context("创建 HTTP 客户端失败")?
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
    let task_id = TaskId::from_legacy_string(&download_id)
        .map_err(|e| SerializableError {
            kind: "parse".into(),
            message: format!("Invalid task ID: {e}"),
        })?;
    let TaskId::Bt(info_hash) = &task_id else {
        return Err(SerializableError {
            kind: String::from("unsupported"),
            message: String::from("speed limit only supported for BT tasks"),
        });
    };
    let bt = state.registry.get_typed::<IrontideBtBackend>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("BT backend not registered"),
        })?;
    bt.set_speed_limit(
        *info_hash,
        download_limit_bps,
        upload_limit_bps,
    );
    Ok(())
}

#[tauri::command]
pub async fn bt_preview_torrent(
    state: State<'_, AppState>,
    source: String,
) -> CommandResult<Vec<TorrentFileEntry>> {
    into_command_result(
        async {
            let bt = state.registry.get_typed::<IrontideBtBackend>()
                .ok_or_else(|| internal_error("BT backend not registered"))?;
            bt.preview_torrent(&source)
                .await
                .context("预览 BT 种子文件失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn bt_get_peers(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtPeerInfo>> {
    into_command_result(
        async {
            let task_id = TaskId::from_legacy_string(&download_id)
                .map_err(|e| anyhow!("Invalid task ID: {e}"))?;
            let TaskId::Bt(info_hash) = &task_id else {
                return Err(anyhow!("Not a BT task"));
            };
            let bt = state.registry.get_typed::<IrontideBtBackend>()
                .ok_or_else(|| internal_error("BT backend not registered"))?;
            bt.get_peers(*info_hash)
                .context("查询 BT 节点信息失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn bt_get_trackers(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtTrackerInfo>> {
    into_command_result(
        async {
            let task_id = TaskId::from_legacy_string(&download_id)
                .map_err(|e| anyhow!("Invalid task ID: {e}"))?;
            let TaskId::Bt(info_hash) = &task_id else {
                return Err(anyhow!("Not a BT task"));
            };
            let bt = state.registry.get_typed::<IrontideBtBackend>()
                .ok_or_else(|| internal_error("BT backend not registered"))?;
            bt.get_trackers(*info_hash)
                .context("查询 BT 追踪器信息失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn bt_get_pieces(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtPieceInfo>> {
    into_command_result(
        async {
            let task_id = TaskId::from_legacy_string(&download_id)
                .map_err(|e| anyhow!("Invalid task ID: {e}"))?;
            let TaskId::Bt(info_hash) = &task_id else {
                return Err(anyhow!("Not a BT task"));
            };
            let bt = state.registry.get_typed::<IrontideBtBackend>()
                .ok_or_else(|| internal_error("BT backend not registered"))?;
            bt.get_pieces(*info_hash)
                .context("查询 BT 分片信息失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn get_bt_files(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtFileStatus>> {
    into_command_result(
        async {
            let task_id = TaskId::from_legacy_string(&download_id)
                .map_err(|e| anyhow!("Invalid task ID: {e}"))?;
            let TaskId::Bt(info_hash) = &task_id else {
                return Err(anyhow!("Not a BT task"));
            };
            let bt = state.registry.get_typed::<IrontideBtBackend>()
                .ok_or_else(|| internal_error("BT backend not registered"))?;
            bt.get_torrent_files(*info_hash)
                .context("查询 BT 文件列表失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn update_bt_files(
    state: State<'_, AppState>,
    download_id: String,
    included_indices: Vec<usize>,
) -> CommandResult<()> {
    into_command_result(
        async {
            let task_id = TaskId::from_legacy_string(&download_id)
                .map_err(|e| anyhow!("Invalid task ID: {e}"))?;
            let TaskId::Bt(info_hash) = &task_id else {
                return Err(anyhow!("Not a BT task"));
            };
            let bt = state.registry.get_typed::<IrontideBtBackend>()
                .ok_or_else(|| internal_error("BT backend not registered"))?;
            bt.update_torrent_files(*info_hash, included_indices)
                .await
                .context("更新 BT 文件选择失败")
        }
        .await,
    )
}

#[tauri::command]
pub async fn toggle_game_mode(
    state: State<'_, AppState>,
    enabled: bool,
) -> CommandResult<bool> {
    let dm = state.registry.get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    dm.set_game_mode(enabled);
    Ok(enabled)
}

#[tauri::command]
pub async fn get_io_status(
    state: State<'_, AppState>,
) -> CommandResult<serde_json::Value> {
    let dm = state.registry.get_typed::<DownloadManager>()
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
    let dm = state.registry.get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    dm.set_overclock_mode(enabled);
    Ok(enabled)
}

#[tauri::command]
pub async fn get_overclock_mode(
    state: State<'_, AppState>,
) -> CommandResult<bool> {
    let dm = state.registry.get_typed::<DownloadManager>()
        .ok_or_else(|| SerializableError {
            kind: String::from("internal"),
            message: String::from("HTTP backend not found"),
        })?;
    Ok(dm.overclock_mode())
}

#[cfg(test)]
#[path = "tests/commands_tests.rs"]
mod tests;
