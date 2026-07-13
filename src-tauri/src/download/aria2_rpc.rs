use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::{
    Router,
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, broadcast};
use tower_http::cors::{Any, CorsLayer};

use super::{
    manager::DownloadManager,
    torrent::TorrentManager,
    types::{
        Aria2RpcSettings, DownloadState, DownloadSummary, StartDownloadRequest, TaskId, TaskKind,
    },
};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Serialize)]
struct JsonRpcNotification {
    jsonrpc: &'static str,
    method: String,
    params: Vec<Value>,
}

const ERR_PARSE: i32 = -32700;
const ERR_INVALID_REQUEST: i32 = -32600;
const ERR_METHOD_NOT_FOUND: i32 = -32601;
const ERR_INVALID_PARAMS: i32 = -32602;
const ERR_INTERNAL: i32 = -32603;

fn make_error(code: i32, message: impl Into<String>) -> JsonRpcError {
    JsonRpcError {
        code,
        message: message.into(),
    }
}

fn error_response(id: Option<Value>, code: i32, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(make_error(code, message)),
    }
}

fn success_response(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

pub fn internal_id_to_gid(internal_id: &str) -> String {
    let hash = xxhash_rust::xxh3::xxh3_64(internal_id.as_bytes());
    format!("{:016x}", hash)
}

async fn resolve_gid(ctx: &RpcContext, gid: &str) -> Option<String> {
    // Check cache first (O(1))
    {
        let cache = ctx.gid_cache.lock().await;
        if let Some(id) = cache.get(gid) {
            return Some(id.clone());
        }
    }

    // Fall back to full scan and populate cache
    let mut cache = ctx.gid_cache.lock().await;
    // Double-check in case another caller already populated
    if let Some(id) = cache.get(gid) {
        return Some(id.clone());
    }

    if let Ok(list) = ctx.manager.list().await {
        for s in &list {
            let key = internal_id_to_gid(&s.id);
            cache.entry(key).or_insert_with(|| s.id.clone());
        }
    }
    if let Ok(list) = ctx.torrent_manager.list().await {
        for s in &list {
            let key = internal_id_to_gid(&s.id);
            cache.entry(key).or_insert_with(|| s.id.clone());
        }
    }

    cache.get(gid).cloned()
}

fn state_to_aria2(state: &DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "waiting",
        DownloadState::Downloading => "active",
        DownloadState::Paused => "paused",
        DownloadState::Retrying | DownloadState::Verifying => "active",
        DownloadState::Completed => "complete",
        DownloadState::Failed => "error",
        DownloadState::Canceled => "removed",
    }
}

fn summary_to_aria2_status(summary: &DownloadSummary) -> Value {
    let gid = internal_id_to_gid(&summary.id);
    let total_len = summary
        .total_bytes
        .map_or_else(|| "0".to_string(), |b| b.to_string());
    let speed = summary
        .speed_bytes_per_second
        .map_or_else(|| "0".to_string(), |s| s.to_string());
    let seeders = summary
        .peer_count
        .map_or_else(|| "0".to_string(), |p| p.to_string());
    let uploaded = summary
        .uploaded_bytes
        .map_or_else(|| "0".to_string(), |b| b.to_string());
    let upload_speed = summary
        .upload_speed_bytes_per_second
        .map_or_else(|| "0".to_string(), |s| s.to_string());
    let is_bt = matches!(summary.kind, TaskKind::Bt);

    let bt_block = if is_bt {
        serde_json::json!({
            "infoHash": summary.info_hash.as_deref().unwrap_or(""),
            "uploadLength": uploaded,
            "uploadSpeed": upload_speed,
            "numSeeders": seeders,
        })
    } else {
        serde_json::json!({
            "infoHash": "",
            "uploadLength": "0",
        })
    };

    serde_json::json!({
        "gid": gid,
        "status": state_to_aria2(&summary.state),
        "totalLength": total_len,
        "completedLength": summary.downloaded_bytes.to_string(),
        "downloadSpeed": speed,
        "uploadSpeed": upload_speed,
        "connections": summary.connection_count.to_string(),
        "numSeeders": seeders,
        "dir": summary.destination_path,
        "files": build_file_list(summary),
        "bittorrent": bt_block,
    })
}

fn build_file_list(summary: &DownloadSummary) -> Value {
    let total_len = summary
        .total_bytes
        .map_or_else(|| "0".to_string(), |b| b.to_string());

    Value::Array(vec![serde_json::json!({
        "index": "1",
        "path": summary.file_name,
        "length": total_len,
        "completedLength": summary.downloaded_bytes.to_string(),
        "selected": "true",
        "uris": [{"uri": summary.url, "status": "used"}]
    })])
}

struct RpcContext {
    manager: Arc<DownloadManager>,
    torrent_manager: Arc<TorrentManager>,
    secret: Option<String>,
    event_tx: broadcast::Sender<String>,
    gid_cache: Mutex<HashMap<String, String>>,
}

impl RpcContext {
    fn settings_default_download_dir(&self) -> String {
        self.manager
            .settings_default_download_dir()
            .unwrap_or_else(|| dirs_next().unwrap_or_else(default_downloads_dir))
    }
}

fn check_token(ctx: &RpcContext, params: &[Value]) -> Result<(), JsonRpcError> {
    let Some(secret) = &ctx.secret else {
        return Ok(());
    };
    let expected = format!("token:{secret}");
    if params
        .first()
        .and_then(|v| v.as_str())
        .is_none_or(|s| s != expected)
    {
        return Err(make_error(1, "Unauthorized"));
    }
    Ok(())
}

fn strip_token(params: Vec<Value>) -> Vec<Value> {
    if params
        .first()
        .and_then(|v| v.as_str())
        .is_some_and(|s| s.starts_with("token:"))
    {
        params.into_iter().skip(1).collect()
    } else {
        params
    }
}

async fn dispatch_method(
    ctx: &RpcContext,
    method: &str,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    match method {
        "aria2.addUri" => handle_add_uri(ctx, params).await,
        "aria2.addTorrent" => handle_add_torrent(ctx, params).await,
        "aria2.pause" | "aria2.forcePause" => handle_pause(ctx, params).await,
        "aria2.unpause" => handle_unpause(ctx, params).await,
        "aria2.pauseAll" | "aria2.forcePauseAll" => handle_pause_all(ctx).await,
        "aria2.unpauseAll" => handle_unpause_all(ctx).await,
        "aria2.remove" | "aria2.forceRemove" => handle_remove(ctx, params).await,
        "aria2.tellStatus" => handle_tell_status(ctx, params).await,
        "aria2.tellActive" => handle_tell_active(ctx, params).await,
        "aria2.tellWaiting" => handle_tell_waiting(ctx, params).await,
        "aria2.tellStopped" => handle_tell_stopped(ctx, params).await,
        "aria2.getGlobalStat" => handle_global_stat(ctx).await,
        "aria2.getGlobalOption" => handle_get_global_option(ctx).await,
        "aria2.changeGlobalOption" => handle_change_global_option(ctx, params).await,
        "aria2.getVersion" => Ok(handle_version()),
        "aria2.getFiles" => handle_get_files(ctx, params).await,
        "aria2.getUris" => handle_get_uris(ctx, params).await,
        "aria2.getPeers" => handle_get_peers(ctx, params).await,
        "aria2.shutdown" => handle_shutdown(ctx).await,
        "system.listMethods" => Ok(handle_list_methods()),
        "system.listNotifications" => Ok(handle_list_notifications()),
        _ => Err(make_error(
            ERR_METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )),
    }
}

async fn handle_add_uri(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;

    let uris: Vec<String> = params
        .first()
        .and_then(|v| v.as_array())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing uris array"))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if uris.is_empty() {
        return Err(make_error(ERR_INVALID_PARAMS, "No URIs provided"));
    }

    let options = params.get(1).and_then(|v| v.as_object());
    // uris was checked for non-empty above; use expect with a message as safety net
    let url = uris
        .into_iter()
        .next()
        .ok_or_else(|| make_error(ERR_INTERNAL, "uris unexpectedly empty"))?;
    let destination_dir = options
        .and_then(|o| o.get("dir"))
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .map(String::from)
        .unwrap_or_else(|| {
            // Use the configured default download directory from settings
            // instead of the process working directory, which is unreliable
            // (changes on restart, may be relative, etc.)
            ctx.settings_default_download_dir()
        });

    let request = StartDownloadRequest {
        kind: Some(TaskKind::Http),
        url,
        destination_dir,
        file_name: extract_option_str(options, "out"),
        user_agent: extract_option_str(options, "user-agent"),
        thread_mode: None,
        thread_count: extract_option_usize(options, "split"),
        max_retries: extract_option_u32(options, "max-tries"),
        checksum: None,
        selected_file_indices: None,
        start_paused: false,
    };

    let id = ctx
        .manager
        .start(request)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    let task_id = TaskId::make_http(id);

    let start_paused = options
        .and_then(|o| o.get("pause"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if start_paused {
        let _ = ctx.manager.pause(&task_id).await;
    }

    let gid = internal_id_to_gid(&task_id);
    broadcast_event(ctx, "aria2.onDownloadStart", &gid);
    Ok(Value::String(gid))
}

async fn handle_add_torrent(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;

    let torrent_b64 = params
        .first()
        .and_then(|v| v.as_str())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing torrent base64 data"))?;

    let torrent_bytes = base64::engine::general_purpose::STANDARD
        .decode(torrent_b64)
        .map_err(|_| make_error(ERR_INVALID_PARAMS, "Invalid base64 torrent data"))?;

    let temp_dir = std::env::temp_dir().join("downloader_aria2");
    std::fs::create_dir_all(&temp_dir).ok();
    let torrent_path = temp_dir.join(format!("{}.torrent", uuid::Uuid::new_v4()));
    std::fs::write(&torrent_path, &torrent_bytes)
        .map_err(|e| make_error(ERR_INTERNAL, format!("Failed to write torrent file: {e}")))?;

    let options = params.get(1).and_then(|v| v.as_object());
    let destination_dir = options
        .and_then(|o| o.get("dir"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| String::from("."))
        });

    let request = StartDownloadRequest {
        kind: Some(TaskKind::Bt),
        url: torrent_path.to_string_lossy().to_string(),
        destination_dir,
        file_name: None,
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        selected_file_indices: None,
        start_paused: false,
    };

    let id = ctx
        .torrent_manager
        .start(request)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    let gid = internal_id_to_gid(&id);
    broadcast_event(ctx, "aria2.onDownloadStart", &gid);
    Ok(Value::String(gid))
}

fn extract_option_str(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<String> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

fn extract_option_usize(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<usize> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str().and_then(|s| s.parse::<usize>().ok()))
}

fn extract_option_u32(options: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<u32> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str().and_then(|s| s.parse::<u32>().ok()))
}

fn broadcast_event(ctx: &RpcContext, method: &str, gid: &str) {
    let _ = ctx.event_tx.send(
        serde_json::to_string(&JsonRpcNotification {
            jsonrpc: "2.0",
            method: method.to_string(),
            params: vec![serde_json::json!({"gid": gid})],
        })
        .unwrap_or_default(),
    );
}

async fn handle_pause(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let internal_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    rpc_dispatch_action(ctx, &internal_id, "pause").await?;
    broadcast_event(ctx, "aria2.onDownloadPause", &gid);
    Ok(Value::String(gid))
}

async fn handle_unpause(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let internal_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    rpc_dispatch_action(ctx, &internal_id, "resume").await?;
    Ok(Value::String(gid))
}

async fn handle_pause_all(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    for s in &all {
        let _ = rpc_dispatch_action(ctx, &s.id, "pause").await;
    }
    Ok(Value::String("OK".to_string()))
}

async fn handle_unpause_all(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    for s in &all {
        let _ = rpc_dispatch_action(ctx, &s.id, "resume").await;
    }
    Ok(Value::String("OK".to_string()))
}

async fn handle_remove(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let internal_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    rpc_dispatch_action(ctx, &internal_id, "remove").await?;
    broadcast_event(ctx, "aria2.onDownloadStop", &gid);
    Ok(Value::String(gid))
}

async fn handle_tell_status(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let internal_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == internal_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(summary_to_aria2_status(&summary))
}

async fn handle_tell_active(ctx: &RpcContext, _params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    let active: Vec<Value> = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying
            )
        })
        .map(summary_to_aria2_status)
        .collect();
    Ok(Value::Array(active))
}

async fn handle_tell_waiting(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let offset: usize = parse_int_param(&params, 0).unwrap_or(0);
    let num: usize = parse_int_param(&params, 1).unwrap_or(1000);

    let all = get_all_summaries(ctx).await?;
    let waiting: Vec<Value> = all
        .iter()
        .filter(|s| matches!(s.state, DownloadState::Queued | DownloadState::Paused))
        .skip(offset)
        .take(num)
        .map(summary_to_aria2_status)
        .collect();
    Ok(Value::Array(waiting))
}

async fn handle_tell_stopped(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let offset: usize = parse_int_param(&params, 0).unwrap_or(0);
    let num: usize = parse_int_param(&params, 1).unwrap_or(1000);

    let all = get_all_summaries(ctx).await?;
    let stopped: Vec<Value> = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            )
        })
        .skip(offset)
        .take(num)
        .map(summary_to_aria2_status)
        .collect();
    Ok(Value::Array(stopped))
}

async fn handle_global_stat(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    let num_active = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Verifying
            )
        })
        .count();
    let num_waiting = all
        .iter()
        .filter(|s| matches!(s.state, DownloadState::Queued | DownloadState::Paused))
        .count();
    let num_stopped = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            )
        })
        .count();

    let total_speed: u64 = all
        .iter()
        .filter_map(|s| s.speed_bytes_per_second.map(|v| v as u64))
        .sum();

    Ok(serde_json::json!({
        "downloadSpeed": total_speed.to_string(),
        "uploadSpeed": "0",
        "numActive": num_active.to_string(),
        "numWaiting": num_waiting.to_string(),
        "numStopped": num_stopped.to_string(),
        "numStoppedTotal": num_stopped.to_string(),
    }))
}

fn handle_version() -> Value {
    serde_json::json!({
        "version": "0.1.0",
        "enabledFeatures": [
            "Async DNS", "BitTorrent", "Firefox3 Cookie", "GZip",
            "HTTPS", "Message Digest", "XML-RPC"
        ]
    })
}

async fn handle_get_files(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let internal_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == internal_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(build_file_list(&summary))
}

async fn handle_get_uris(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let internal_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == internal_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(serde_json::json!([{
        "uri": summary.url,
        "status": "used"
    }]))
}

async fn handle_get_peers(_ctx: &RpcContext, _params: Vec<Value>) -> Result<Value, JsonRpcError> {
    // Peers are tracked via librqbit internally; full peer enumeration requires
    // deeper integration. Return empty list as a placeholder.
    Ok(Value::Array(vec![]))
}

async fn handle_get_global_option(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let settings = ctx
        .manager
        .settings()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(serde_json::json!({
        "dir": settings.download.default_download_dir,
        "max-concurrent-downloads": settings.scheduler.traditional.max_parallel_tasks.to_string(),
        "max-connection-per-server": "16",
        "min-split-size": "20M",
        "split": "5",
        "max-overall-download-limit": "0",
    }))
}

async fn handle_change_global_option(
    ctx: &RpcContext,
    params: Vec<Value>,
) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;

    let options = params
        .first()
        .and_then(|v| v.as_object())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing options object"))?;

    let mut settings = ctx
        .manager
        .settings()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    if let Some(dir) = options.get("dir").and_then(|v| v.as_str()) {
        let dir = dir.trim();
        if dir.is_empty() {
            return Err(make_error(ERR_INVALID_PARAMS, "dir cannot be empty"));
        }
        if !PathBuf::from(dir).is_absolute() {
            return Err(make_error(
                ERR_INVALID_PARAMS,
                "dir must be an absolute path",
            ));
        }
        settings.download.default_download_dir = dir.to_string();
    }
    if let Some(max_tasks) = options
        .get("max-concurrent-downloads")
        .and_then(|v| v.as_str())
        && let Ok(n) = max_tasks.parse::<usize>()
    {
        settings.scheduler.traditional.max_parallel_tasks = n;
    }
    if let Some(limit) = options
        .get("max-overall-download-limit")
        .and_then(|v| v.as_str())
        && let Ok(_n) = limit.parse::<u64>()
    {
        // Per-task download limits are managed via the AIMD controller;
        // global limits are not yet implemented.
    }

    ctx.manager
        .update_settings(settings)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(Value::String("OK".to_string()))
}

async fn handle_shutdown(_ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    tracing::info!("Aria2 RPC shutdown requested");
    Ok(Value::String("OK".to_string()))
}

fn handle_list_notifications() -> Value {
    Value::Array(
        [
            "aria2.onDownloadStart",
            "aria2.onDownloadPause",
            "aria2.onDownloadStop",
            "aria2.onDownloadComplete",
            "aria2.onBtDownloadComplete",
            "aria2.onDownloadError",
        ]
        .iter()
        .map(|&s| Value::String(s.to_string()))
        .collect(),
    )
}

fn handle_list_methods() -> Value {
    Value::Array(
        [
            "aria2.addUri",
            "aria2.addTorrent",
            "aria2.pause",
            "aria2.forcePause",
            "aria2.unpause",
            "aria2.pauseAll",
            "aria2.forcePauseAll",
            "aria2.unpauseAll",
            "aria2.remove",
            "aria2.forceRemove",
            "aria2.tellStatus",
            "aria2.tellActive",
            "aria2.tellWaiting",
            "aria2.tellStopped",
            "aria2.getGlobalStat",
            "aria2.getGlobalOption",
            "aria2.changeGlobalOption",
            "aria2.getVersion",
            "aria2.getFiles",
            "aria2.getUris",
            "aria2.getPeers",
            "aria2.shutdown",
            "system.listMethods",
            "system.listNotifications",
        ]
        .iter()
        .map(|&s| Value::String(s.to_string()))
        .collect(),
    )
}

fn extract_gid(params: &[Value]) -> Result<String, JsonRpcError> {
    params
        .first()
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing GID parameter"))
}

fn parse_int_param(params: &[Value], index: usize) -> Option<usize> {
    params.get(index).and_then(|v| {
        v.as_str()
            .and_then(|s| s.parse::<usize>().ok())
            .or_else(|| v.as_u64().map(|n| n as usize))
    })
}

async fn get_all_summaries(ctx: &RpcContext) -> Result<Vec<DownloadSummary>, JsonRpcError> {
    let mut all = ctx
        .manager
        .list()
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    if let Ok(bt) = ctx.torrent_manager.list().await {
        all.extend(bt);
    }
    Ok(all)
}

async fn rpc_dispatch_action(
    ctx: &RpcContext,
    internal_id: &str,
    action: &str,
) -> Result<(), JsonRpcError> {
    let task_id = TaskId::parse(internal_id);
    let result: anyhow::Result<()> = match &task_id {
        TaskId::Bt(_) => match action {
            "pause" => ctx
                .torrent_manager
                .pause(internal_id)
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}")),
            "resume" => ctx
                .torrent_manager
                .resume(internal_id)
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}")),
            "remove" => ctx
                .torrent_manager
                .remove(internal_id)
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}")),
            _ => Err(anyhow::anyhow!("unsupported action for BT: {action}")),
        },
        TaskId::Http(_) => match action {
            "pause" => ctx
                .manager
                .pause(task_id.http_inner().unwrap_or(""))
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}")),
            "resume" => ctx
                .manager
                .resume(task_id.http_inner().unwrap_or(""))
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}")),
            "remove" => ctx
                .manager
                .remove(task_id.http_inner().unwrap_or(""))
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}")),
            _ => Err(anyhow::anyhow!("unsupported action for HTTP: {action}")),
        },
    };

    result.map_err(|e| make_error(ERR_INTERNAL, e.to_string()))
}

async fn handle_jsonrpc_http(
    axum::extract::State(ctx): axum::extract::State<Arc<RpcContext>>,
    body: String,
) -> Response {
    match serde_json::from_str::<JsonRpcRequest>(&body) {
        Ok(req) => {
            if req.jsonrpc != "2.0" {
                let resp = error_response(req.id, ERR_INVALID_REQUEST, "Invalid JSON-RPC version");
                return (
                    StatusCode::OK,
                    serde_json::to_string(&resp).unwrap_or_default(),
                )
                    .into_response();
            }

            let params = req.params.unwrap_or_default();
            match dispatch_method(&ctx, &req.method, params).await {
                Ok(result) => {
                    let resp = success_response(req.id, result);
                    (
                        StatusCode::OK,
                        serde_json::to_string(&resp).unwrap_or_default(),
                    )
                        .into_response()
                }
                Err(err) => {
                    let resp = error_response(req.id, err.code, err.message);
                    (
                        StatusCode::OK,
                        serde_json::to_string(&resp).unwrap_or_default(),
                    )
                        .into_response()
                }
            }
        }
        Err(_) => {
            let resp = error_response(None, ERR_PARSE, "Parse error");
            (
                StatusCode::OK,
                serde_json::to_string(&resp).unwrap_or_default(),
            )
                .into_response()
        }
    }
}

async fn handle_websocket_upgrade(
    ws: WebSocketUpgrade,
    axum::extract::State(ctx): axum::extract::State<Arc<RpcContext>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket_loop(socket, ctx))
}

async fn websocket_loop(socket: WebSocket, ctx: Arc<RpcContext>) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(tokio::sync::Mutex::new(sender));
    let mut event_rx = ctx.event_tx.subscribe();

    let sender_events = sender.clone();
    let mut send_events = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(msg) => {
                    if sender_events
                        .lock()
                        .await
                        .send(Message::Text(msg.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let sender_reqs = sender.clone();
    let ctx2 = ctx.clone();
    let recv_requests = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    let resp = process_jsonrpc_message(&ctx2, &text).await;
                    if sender_reqs
                        .lock()
                        .await
                        .send(Message::Text(resp.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = &mut send_events => {}
        _ = recv_requests => {}
    }
    send_events.abort();
}

async fn process_jsonrpc_message(ctx: &RpcContext, body: &str) -> String {
    let req: JsonRpcRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(_) => {
            let resp = error_response(None, ERR_PARSE, "Parse error");
            return serde_json::to_string(&resp).unwrap_or_default();
        }
    };

    if req.jsonrpc != "2.0" {
        let resp = error_response(req.id, ERR_INVALID_REQUEST, "Invalid JSON-RPC version");
        return serde_json::to_string(&resp).unwrap_or_default();
    }

    let params = req.params.unwrap_or_default();
    match dispatch_method(ctx, &req.method, params).await {
        Ok(result) => {
            let resp = success_response(req.id, result);
            serde_json::to_string(&resp).unwrap_or_default()
        }
        Err(err) => {
            let resp = error_response(req.id, err.code, err.message);
            serde_json::to_string(&resp).unwrap_or_default()
        }
    }
}

fn dirs_next() -> Option<String> {
    let home = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").ok()
    } else {
        std::env::var("HOME").ok()
    };
    home.map(|p| {
        PathBuf::from(p)
            .join("Downloads")
            .to_string_lossy()
            .to_string()
    })
}

fn default_downloads_dir() -> String {
    dirs_next().unwrap_or_else(|| String::from("."))
}

pub struct Aria2RpcServer {
    ctx: Arc<RpcContext>,
    addr: String,
}

impl Aria2RpcServer {
    pub fn new(
        manager: Arc<DownloadManager>,
        torrent_manager: Arc<TorrentManager>,
        settings: &Aria2RpcSettings,
        event_tx: broadcast::Sender<String>,
    ) -> Self {
        let secret = settings.secret.clone().filter(|s| !s.is_empty());

        let ctx = Arc::new(RpcContext {
            manager,
            torrent_manager,
            secret,
            event_tx,
            gid_cache: Mutex::new(HashMap::new()),
        });

        Aria2RpcServer {
            ctx,
            addr: format!("127.0.0.1:{}", settings.port),
        }
    }

    pub async fn serve(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> anyhow::Result<()> {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(Any);

        let app = Router::new()
            .route(
                "/jsonrpc",
                post(handle_jsonrpc_http).get(handle_websocket_upgrade),
            )
            .layer(cors)
            .with_state(self.ctx);

        tracing::info!("Aria2 RPC server listening on http://{}/jsonrpc", self.addr);

        let listener = tokio::net::TcpListener::bind(&self.addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown.changed().await;
            })
            .await?;
        Ok(())
    }
}
