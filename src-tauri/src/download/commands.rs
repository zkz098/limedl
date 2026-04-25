use std::time::Duration;

use anyhow::{Context, anyhow};
use tauri::State;

use super::{
    manager::{AppState, normalize_tracker_list_lossy, normalize_tracker_list_url},
    sftp::is_sftp_task_id,
    torrent::{
        DownloadSourceKind, classify_download_source, http_task_id, is_bt_task_id,
        normalize_http_task_id,
    },
    types::{AppSettings, DownloadSnapshot, DownloadSummary, StartDownloadRequest},
};

type CommandResult<T> = std::result::Result<T, String>;

fn into_command_result<T>(result: anyhow::Result<T>) -> CommandResult<T> {
    result.map_err(format_anyhow_error)
}

fn format_anyhow_error(error: anyhow::Error) -> String {
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

#[tauri::command]
pub async fn download_start(
    state: State<'_, AppState>,
    request: StartDownloadRequest,
) -> CommandResult<String> {
    into_command_result(
        async {
            match classify_download_source(&request).context("无法识别下载任务类型")? {
                DownloadSourceKind::Http => {
                    let id = state
                        .manager
                        .start(request)
                        .await
                        .context("启动 HTTP 下载失败")?;
                    Ok(http_task_id(id))
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
                    Ok(http_task_id(id))
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
    )
}

#[tauri::command]
pub async fn download_pause(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    if is_bt_task_id(&download_id) {
        return into_command_result(
            state
                .torrent_manager
                .pause(&download_id)
                .await
                .context("暂停 BT 下载失败"),
        );
    }
    if is_sftp_task_id(&download_id) {
        return into_command_result(
            state
                .sftp_manager
                .pause(&download_id)
                .await
                .context("暂停 SFTP 下载失败"),
        );
    }

    into_command_result(
        state
            .manager
            .pause(normalize_http_task_id(&download_id))
            .await
            .map(prefix_http_snapshot)
            .context("暂停 HTTP 下载失败"),
    )
}

#[tauri::command]
pub async fn download_resume(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    if is_bt_task_id(&download_id) {
        return into_command_result(
            state
                .torrent_manager
                .resume(&download_id)
                .await
                .context("恢复 BT 下载失败"),
        );
    }
    if is_sftp_task_id(&download_id) {
        return into_command_result(
            state
                .sftp_manager
                .resume(&download_id)
                .await
                .context("恢复 SFTP 下载失败"),
        );
    }

    into_command_result(
        state
            .manager
            .resume(normalize_http_task_id(&download_id))
            .await
            .map(prefix_http_snapshot)
            .context("恢复 HTTP 下载失败"),
    )
}

#[tauri::command]
pub async fn download_cancel(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    if is_bt_task_id(&download_id) {
        return into_command_result(
            state
                .torrent_manager
                .cancel(&download_id)
                .await
                .context("取消 BT 下载失败"),
        );
    }
    if is_sftp_task_id(&download_id) {
        return into_command_result(
            state
                .sftp_manager
                .cancel(&download_id)
                .await
                .context("取消 SFTP 下载失败"),
        );
    }

    into_command_result(
        state
            .manager
            .cancel(normalize_http_task_id(&download_id))
            .await
            .map(prefix_http_snapshot)
            .context("取消 HTTP 下载失败"),
    )
}

#[tauri::command]
pub async fn download_remove(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    if is_bt_task_id(&download_id) {
        return into_command_result(
            state
                .torrent_manager
                .remove(&download_id)
                .await
                .context("移除 BT 下载失败"),
        );
    }
    if is_sftp_task_id(&download_id) {
        return into_command_result(
            state
                .sftp_manager
                .remove(&download_id)
                .await
                .context("移除 SFTP 下载失败"),
        );
    }

    into_command_result(
        state
            .manager
            .remove(normalize_http_task_id(&download_id))
            .await
            .map(prefix_http_snapshot)
            .context("移除 HTTP 下载失败"),
    )
}

#[tauri::command]
pub async fn download_purge(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    if is_bt_task_id(&download_id) {
        return into_command_result(
            state
                .torrent_manager
                .purge(&download_id)
                .await
                .context("彻底删除 BT 下载失败"),
        );
    }
    if is_sftp_task_id(&download_id) {
        return into_command_result(
            state
                .sftp_manager
                .purge(&download_id)
                .await
                .context("彻底删除 SFTP 下载失败"),
        );
    }

    into_command_result(
        state
            .manager
            .purge(normalize_http_task_id(&download_id))
            .await
            .map(prefix_http_snapshot)
            .context("彻底删除 HTTP 下载失败"),
    )
}

#[tauri::command]
pub async fn download_open_in_explorer(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<()> {
    if is_bt_task_id(&download_id) {
        return into_command_result(
            state
                .torrent_manager
                .open_in_explorer(&download_id)
                .await
                .context("在资源管理器打开 BT 下载失败"),
        );
    }
    if is_sftp_task_id(&download_id) {
        return into_command_result(
            state
                .sftp_manager
                .open_in_explorer(&download_id)
                .await
                .context("在资源管理器打开 SFTP 下载失败"),
        );
    }

    into_command_result(
        state
            .manager
            .open_in_explorer(normalize_http_task_id(&download_id))
            .await
            .context("在资源管理器打开 HTTP 下载失败"),
    )
}

#[tauri::command]
pub async fn download_status(
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadSnapshot> {
    if is_bt_task_id(&download_id) {
        return into_command_result(
            state
                .torrent_manager
                .status(&download_id)
                .await
                .context("查询 BT 下载状态失败"),
        );
    }
    if is_sftp_task_id(&download_id) {
        return into_command_result(
            state
                .sftp_manager
                .status(&download_id)
                .await
                .context("查询 SFTP 下载状态失败"),
        );
    }

    into_command_result(
        state
            .manager
            .status(normalize_http_task_id(&download_id))
            .await
            .map(prefix_http_snapshot)
            .context("查询 HTTP 下载状态失败"),
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
                    summary.id = http_task_id(summary.id);
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
            downloads.sort_by(|left, right| right.id.cmp(&left.id));
            Ok(downloads)
        }
        .await,
    )
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
            let saved = state
                .manager
                .update_settings(settings)
                .await
                .context("保存设置失败")?;
            state.torrent_manager.update_settings(&saved);
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

fn prefix_http_snapshot(mut snapshot: DownloadSnapshot) -> DownloadSnapshot {
    snapshot.id = http_task_id(snapshot.id);
    snapshot
}
