use std::time::Duration;

use anyhow::{Context, anyhow};
use tauri::State;

use super::{
    Aria2RpcServer,
    error::DownloadError,
    manager::{AppState, normalize_tracker_list_lossy, normalize_tracker_list_url},
    torrent::{DownloadSourceKind, classify_download_source},
    types::{
        AppSettings, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtTrackerInfo, DownloadSnapshot,
        DownloadSummary, SerializableError, StartDownloadRequest, TaskId, TorrentFileEntry,
    },
};

type CommandResult<T> = std::result::Result<T, SerializableError>;

fn into_command_result<T>(result: anyhow::Result<T>) -> CommandResult<T> {
    result.map_err(|error| {
        let kind = error
            .downcast_ref::<DownloadError>()
            .map(|de| de.kind().to_string())
            .unwrap_or_else(|| "internal".to_string());
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

/// Routes a download action to the correct manager based on task ID prefix.
/// Eliminates the copy-paste `if bt → else if sftp → else http` pattern across all commands.
macro_rules! dispatch_download_action {
    ($state:expr, $download_id:expr, $action:ident, $http_err:literal, $bt_err:literal, $sftp_err:literal) => {{
        let task_id = TaskId::parse(&$download_id);
        match &task_id {
            TaskId::Bt(_) => into_command_result(
                $state
                    .torrent_manager
                    .$action(&$download_id)
                    .await
                    .context($bt_err),
            ),
            TaskId::Sftp(_) => into_command_result(
                $state
                    .sftp_manager
                    .$action(&$download_id)
                    .await
                    .context($sftp_err),
            ),
            TaskId::Http(_) => into_command_result(
                $state
                    .manager
                    .$action(task_id.http_inner())
                    .await
                    .map(prefix_http_snapshot)
                    .context($http_err),
            ),
        }
    }};
}

#[tauri::command]
pub async fn download_start(
    state: State<'_, AppState>,
    request: StartDownloadRequest,
) -> CommandResult<String> {
    let result = into_command_result(
        async {
            match classify_download_source(&request).context("无法识别下载任务类型")? {
                DownloadSourceKind::Http => {
                    let id = state
                        .manager
                        .start(request)
                        .await
                        .context("启动 HTTP 下载失败")?;
                    Ok(TaskId::make_http(id))
                }
                DownloadSourceKind::Torrent => state
                    .torrent_manager
                    .start(request)
                    .await
                    .context("启动 BT 下载失败"),
                DownloadSourceKind::Metalink => {
                    let id = state
                        .manager
                        .start_metalink(request)
                        .await
                        .context("启动 Metalink 下载失败")?;
                    Ok(TaskId::make_http(id))
                }
                DownloadSourceKind::Sftp => {
                    let settings = state.manager.settings().await.context("读取下载设置失败")?;
                    if !settings.download.enable_sftp {
                        return Err(anyhow!("SFTP support is disabled in settings"));
                    }
                    state
                        .sftp_manager
                        .start(request)
                        .await
                        .context("启动 SFTP 下载失败")
                }
            }
        }
        .await,
    );
    state.emit_all_downloads().await;
    result
}

#[tauri::command]
pub async fn download_pause(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let result = dispatch_download_action!(
        state,
        download_id,
        pause,
        "暂停 HTTP 下载失败",
        "暂停 BT 下载失败",
        "暂停 SFTP 下载失败"
    );
    state.emit_all_downloads().await;
    result
}

#[tauri::command]
pub async fn download_resume(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let result = dispatch_download_action!(
        state,
        download_id,
        resume,
        "恢复 HTTP 下载失败",
        "恢复 BT 下载失败",
        "恢复 SFTP 下载失败"
    );
    state.emit_all_downloads().await;
    result
}

#[tauri::command]
pub async fn download_cancel(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let result = dispatch_download_action!(
        state,
        download_id,
        cancel,
        "取消 HTTP 下载失败",
        "取消 BT 下载失败",
        "取消 SFTP 下载失败"
    );
    state.emit_all_downloads().await;
    result
}

#[tauri::command]
pub async fn download_remove(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let result = dispatch_download_action!(
        state,
        download_id,
        remove,
        "移除 HTTP 下载失败",
        "移除 BT 下载失败",
        "移除 SFTP 下载失败"
    );
    state.emit_all_downloads().await;
    result
}

#[tauri::command]
pub async fn download_purge(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    let result = dispatch_download_action!(
        state,
        download_id,
        purge,
        "彻底删除 HTTP 下载失败",
        "彻底删除 BT 下载失败",
        "彻底删除 SFTP 下载失败"
    );
    state.emit_all_downloads().await;
    result
}

#[tauri::command]
pub async fn download_open_in_explorer(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<()> {
    let task_id = TaskId::parse(&download_id);
    match &task_id {
        TaskId::Bt(_) => into_command_result(
            state
                .torrent_manager
                .open_in_explorer(&download_id)
                .await
                .context("在资源管理器打开 BT 下载失败"),
        ),
        TaskId::Sftp(_) => into_command_result(
            state
                .sftp_manager
                .open_in_explorer(&download_id)
                .await
                .context("在资源管理器打开 SFTP 下载失败"),
        ),
        TaskId::Http(_) => into_command_result(
            state
                .manager
                .open_in_explorer(task_id.http_inner())
                .await
                .context("在资源管理器打开 HTTP 下载失败"),
        ),
    }
}

#[tauri::command]
pub async fn download_status(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    dispatch_download_action!(
        state,
        download_id,
        status,
        "查询 HTTP 下载状态失败",
        "查询 BT 下载状态失败",
        "查询 SFTP 下载状态失败"
    )
}

#[tauri::command]
pub async fn download_list(state: State<'_, AppState>) -> CommandResult<Vec<DownloadSummary>> {
    into_command_result(
        async {
            let mut downloads = state
                .manager
                .list()
                .await
                .context("读取 HTTP 下载列表失败")?
                .into_iter()
                .map(|mut summary| {
                    summary.id = TaskId::make_http(summary.id);
                    summary
                })
                .collect::<Vec<_>>();
            downloads.extend(
                state
                    .torrent_manager
                    .list()
                    .await
                    .context("读取 BT 下载列表失败")?,
            );
            downloads.extend(
                state
                    .sftp_manager
                    .list()
                    .await
                    .context("读取 SFTP 下载列表失败")?,
            );
            downloads.sort_by_key(|right| std::cmp::Reverse(right.created_at_ms));
            Ok(downloads)
        }
        .await,
    )
}

#[tauri::command]
pub async fn bt_runtime_status(state: State<'_, AppState>) -> CommandResult<BtRuntimeStatus> {
    Ok(state.torrent_manager.runtime_status())
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> CommandResult<AppSettings> {
    into_command_result(state.manager.settings().await.context("读取设置失败"))
}

#[tauri::command]
pub async fn settings_save(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CommandResult<AppSettings> {
    into_command_result(
        async {
            let old_rpc = state
                .manager
                .settings()
                .await
                .context("读取当前设置失败")?
                .aria2_rpc;

            let saved = state
                .manager
                .update_settings(settings)
                .await
                .context("保存设置失败")?;
            state.torrent_manager.update_settings(&saved);

            // Sync CDN acceleration settings — clear accelerator when disabled
            if !saved.cdn_acceleration.enabled {
                state.cdn_accelerator.clear().await;
            }

            let new_rpc = &saved.aria2_rpc;
            if old_rpc != *new_rpc {
                // Signal existing RPC server to shut down gracefully
                if let Some(tx) = state.rpc_shutdown.lock().unwrap().take() {
                    let _ = tx.send(true);
                }
                if new_rpc.enabled {
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    let (event_tx, _event_rx) = tokio::sync::broadcast::channel(256);
                    state.manager.set_event_tx(event_tx.clone());
                    state.torrent_manager.set_event_tx(event_tx.clone());
                    state.sftp_manager.set_event_tx(event_tx.clone());
                    let rpc_server = Aria2RpcServer::new(
                        state.manager.clone(),
                        state.torrent_manager.clone(),
                        state.sftp_manager.clone(),
                        new_rpc,
                        event_tx,
                    );
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = rpc_server.serve(rx).await {
                            tracing::error!("Aria2 RPC server stopped: {error}");
                        }
                    });
                    *state.rpc_shutdown.lock().unwrap() = Some(tx);
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
                .user_agent("downloader/0.1")
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

/// Set per-torrent speed limits. The effective session limit is the minimum of
/// all per-torrent limits since librqbit applies ratelimits at the session level.
/// Passing `None` for both axes clears this torrent's override and recomputes.
#[tauri::command]
pub async fn bt_set_speed_limit(
    state: State<'_, AppState>,
    download_id: String,
    download_limit_bps: Option<u64>,
    upload_limit_bps: Option<u64>,
) -> CommandResult<()> {
    let task_id = TaskId::parse(&download_id);
    match &task_id {
        TaskId::Bt(_) => {
            state.torrent_manager.set_speed_limit(
                &download_id,
                download_limit_bps,
                upload_limit_bps,
            );
            Ok(())
        }
        _ => Err(SerializableError {
            kind: String::from("unsupported"),
            message: String::from("speed limit only supported for BT tasks"),
        }),
    }
}

#[tauri::command]
pub async fn bt_preview_torrent(
    state: State<'_, AppState>,
    source: String,
) -> CommandResult<Vec<TorrentFileEntry>> {
    into_command_result(
        state
            .torrent_manager
            .preview_torrent(&source)
            .await
            .context("预览 BT 种子文件失败"),
    )
}

#[tauri::command]
pub async fn bt_get_peers(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtPeerInfo>> {
    into_command_result(
        state
            .torrent_manager
            .get_peers(&download_id)
            .context("查询 BT 节点信息失败"),
    )
}

#[tauri::command]
pub async fn bt_get_trackers(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtTrackerInfo>> {
    into_command_result(
        state
            .torrent_manager
            .get_trackers(&download_id)
            .context("查询 BT 追踪器信息失败"),
    )
}

#[tauri::command]
pub async fn bt_get_pieces(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<Vec<BtPieceInfo>> {
    into_command_result(
        state
            .torrent_manager
            .get_pieces(&download_id)
            .context("查询 BT 分片信息失败"),
    )
}

fn prefix_http_snapshot(mut snapshot: DownloadSnapshot) -> DownloadSnapshot {
    snapshot.id = TaskId::make_http(snapshot.id);
    snapshot
}
