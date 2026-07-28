use std::time::Duration;
use std::{path::PathBuf, sync::Arc};

use foldhash::HashMap;

use axum::{
    Router,
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use irontide::core::Id20;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use uuid::Uuid;

use crate::{
    backend_registry::BackendRegistry,
    bt_backend_own::IrontideBtBackend,
    dispatcher::Dispatcher,
    event_bus::{DownloadEvent, EventBus},
    manager::DownloadManager,
    types::{
        Aria2RpcSettings, BtPeerInfo, DownloadState, DownloadSummary, StartDownloadRequest, TaskId,
        TaskKind,
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

async fn resolve_gid(ctx: &RpcContext, gid: &str) -> Option<TaskId> {
    // Check cache first
    {
        let cache = ctx.gid_cache.lock().await;
        if let Some(task_id) = cache.get(gid) {
            return Some(*task_id);
        }
    }

    // Cache miss — scan all backends
    for backend in ctx.registry.iter() {
        if let Ok(list) = backend.list().await {
            for s in &list {
                if internal_id_to_gid(&s.id) == gid {
                    let task_id = match s.kind {
                        TaskKind::Http => TaskId::Http(Uuid::parse_str(&s.id).ok()?),
                        TaskKind::Bt => TaskId::Bt(Id20::from_hex(&s.id).ok()?),
                    };
                    let mut cache = ctx.gid_cache.lock().await;
                    cache.insert(gid.to_string(), task_id);
                    return Some(task_id);
                }
            }
        }
    }

    None
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
    registry: Arc<BackendRegistry>,
    dispatcher: Dispatcher,
    secret: Option<String>,
    event_bus: Arc<EventBus>,
    gid_cache: Mutex<HashMap<String, TaskId>>,
    session_id: String,
}

impl RpcContext {
    fn settings_default_download_dir(&self) -> String {
        self.registry
            .get_typed::<DownloadManager>()
            .and_then(|dm| dm.settings_default_download_dir())
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
        "aria2.multicall" | "system.multicall" => handle_multicall(ctx, params).await,
        "aria2.pause" | "aria2.forcePause" => handle_pause(ctx, params).await,
        "aria2.unpause" => handle_unpause(ctx, params).await,
        "aria2.pauseAll" | "aria2.forcePauseAll" => handle_pause_all(ctx).await,
        "aria2.purgeDownloadResult" => handle_purge_download_result(ctx).await,
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
        "aria2.getOption" => handle_get_option(ctx, params).await,
        "aria2.getUris" => handle_get_uris(ctx, params).await,
        "aria2.getPeers" => handle_get_peers(ctx, params).await,
        "aria2.getSessionInfo" => Ok(handle_get_session_info(ctx)),
        "aria2.saveSession" => Ok(handle_save_session()),
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
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    };

    // Dedup: if a non-terminal download for this URL already exists, return its GID
    let dm = ctx
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    if let Some(existing_id) = dm.find_active_by_url(&request.url).await {
        let gid = internal_id_to_gid(&existing_id);
        // Cache the GID so resolve_gid can find it without scanning.
        if let Ok(uuid) = Uuid::parse_str(&existing_id) {
            ctx.gid_cache
                .lock()
                .await
                .insert(gid.clone(), TaskId::Http(uuid));
        }
        return Ok(Value::String(gid));
    }
    // dm dropped — dispatcher.start will handle backend routing

    let task_id = ctx
        .dispatcher
        .start(request)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    let start_paused = options
        .and_then(|o| o.get("pause"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if start_paused {
        let _ = ctx.dispatcher.pause(&task_id).await;
    }

    // Emit initial Updated event so the frontend displays the task immediately.
    if let Ok(snapshot) = ctx.dispatcher.status(&task_id).await {
        ctx.dispatcher.emit_updated(&snapshot);
    }

    let gid = internal_id_to_gid(&task_id.raw_id());
    // Cache the GID so resolve_gid can find it without scanning.
    ctx.gid_cache
        .lock()
        .await
        .insert(gid.clone(), task_id);
    broadcast_event(ctx, "aria2.onDownloadStart", &gid);
    Ok(Value::String(gid))
}

/// Removes `.torrent` files in the aria2 temp directory that are older than 1 hour.
/// This is a best-effort cleanup — all errors are silently ignored.
pub fn cleanup_old_aria2_temp_files() {
    let temp_dir = std::env::temp_dir().join("limedl_aria2");
    let Ok(entries) = std::fs::read_dir(&temp_dir) else {
        return;
    };

    let now = std::time::SystemTime::now();
    let one_hour = std::time::Duration::from_secs(3600);
    let mut removed = 0u32;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("torrent") {
            continue;
        }
        let age = match std::fs::metadata(&path).and_then(|m| m.modified().or_else(|_| m.created()))
        {
            Ok(created) => now.duration_since(created).unwrap_or_default(),
            Err(_) => continue,
        };
        if age >= one_hour && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }

    if removed > 0 {
        tracing::debug!("Cleaned up {removed} old aria2 torrent temp file(s)");
    }
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

    cleanup_old_aria2_temp_files();

    let temp_dir = std::env::temp_dir().join("limedl_aria2");
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
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    };

    let task_id = ctx
        .dispatcher
        .start(request)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    // The BT backend's emit_pending_summary already emits Updated via the
    // event bus during start(), so no manual emit needed here.

    let gid = internal_id_to_gid(&task_id.raw_id());
    // Cache the GID so resolve_gid can find it without scanning.
    ctx.gid_cache
        .lock()
        .await
        .insert(gid.clone(), task_id);
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
    ctx.event_bus.publish(DownloadEvent::Aria2Notification {
        event_name: method.to_string(),
        gid: gid.to_string(),
    });
}

async fn handle_pause(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .pause(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    broadcast_event(ctx, "aria2.onDownloadPause", &gid);
    Ok(Value::String(gid))
}

async fn handle_unpause(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .resume(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    Ok(Value::String(gid))
}

async fn handle_pause_all(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    for s in &all {
        let task_id = match s.kind {
            TaskKind::Http => match Uuid::parse_str(&s.id) {
                Ok(uuid) => TaskId::Http(uuid),
                Err(_) => continue,
            },
            TaskKind::Bt => match Id20::from_hex(&s.id) {
                Ok(ih) => TaskId::Bt(ih),
                Err(_) => continue,
            },
        };
        let _ = ctx.dispatcher.pause(&task_id).await;
    }
    Ok(Value::String("OK".to_string()))
}

async fn handle_unpause_all(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;
    for s in &all {
        let task_id = match s.kind {
            TaskKind::Http => match Uuid::parse_str(&s.id) {
                Ok(uuid) => TaskId::Http(uuid),
                Err(_) => continue,
            },
            TaskKind::Bt => match Id20::from_hex(&s.id) {
                Ok(ih) => TaskId::Bt(ih),
                Err(_) => continue,
            },
        };
        let _ = ctx.dispatcher.resume(&task_id).await;
    }
    Ok(Value::String("OK".to_string()))
}

async fn handle_remove(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    ctx.dispatcher
        .remove(&task_id)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;
    broadcast_event(ctx, "aria2.onDownloadStop", &gid);
    Ok(Value::String(gid))
}

async fn handle_tell_status(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    // O(1) lookup on DownloadManager first (covers all HTTP downloads).
    let summary = if let Some(dm) = ctx.registry.get_typed::<DownloadManager>() {
        if let Some(s) = dm.get_summary(&raw_id).await {
            s
        } else {
            let all = get_all_summaries(ctx).await?;
            all.into_iter()
                .find(|s| s.id == raw_id)
                .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?
        }
    } else {
        let all = get_all_summaries(ctx).await?;
        all.into_iter()
            .find(|s| s.id == raw_id)
            .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?
    };

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
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == raw_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(build_file_list(&summary))
}

async fn handle_get_uris(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == raw_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    Ok(serde_json::json!([{
        "uri": summary.url,
        "status": "used"
    }]))
}

async fn handle_get_peers(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    // HTTP downloads don't have BitTorrent peers — return empty array.
    let TaskId::Bt(info_hash) = &task_id else {
        return Ok(Value::Array(vec![]));
    };

    let peers = ctx
        .registry
        .get_typed::<IrontideBtBackend>()
        .ok_or_else(|| make_error(ERR_INTERNAL, "BT backend not available"))?
        .get_peers(*info_hash)
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    let aria2_peers: Vec<Value> = peers.iter().map(peer_info_to_aria2_peer).collect();

    Ok(Value::Array(aria2_peers))
}

fn peer_info_to_aria2_peer(p: &BtPeerInfo) -> Value {
    let (ip, port) = match p.address.rsplit_once(':') {
        Some((ip, port_str)) => (ip.to_string(), port_str.parse::<u16>().unwrap_or(0)),
        None => (p.address.clone(), 0),
    };

    // Decode 'am_choking' from flags ('c' character)
    let am_choking = p.flags.contains('c');
    // seeder if progress >= 1.0 (100% complete)
    let seeder = p.progress >= 1.0;

    serde_json::json!({
        "peerId": "",
        "ip": ip,
        "port": port,
        "bitfield": "",
        "amChoking": if am_choking { "true" } else { "false" },
        "peerChoking": "false",
        "downloadSpeed": p.download_speed.to_string(),
        "uploadSpeed": p.upload_speed.to_string(),
        "seeder": if seeder { "true" } else { "false" },
    })
}

async fn handle_get_global_option(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let dm = ctx
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    let settings = dm
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

    let dm = ctx
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| make_error(ERR_INTERNAL, "HTTP backend not available"))?;
    let mut settings = dm
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

    dm.apply_settings(settings)
        .await
        .map_err(|e| make_error(ERR_INTERNAL, e.to_string()))?;

    Ok(Value::String("OK".to_string()))
}

async fn handle_shutdown(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    tracing::info!("aria2.shutdown requested from aria2 client — limedl runs as a managed subsystem; use the application UI to exit");
    ctx.event_bus.publish(DownloadEvent::Warning {
        id: "system".into(),
        message: "Aria2 client 请求关闭程序。请使用应用界面退出。".into(),
    });
    Ok(Value::String("Shutdown acknowledged. Use the application UI to exit.".to_string()))
}

async fn handle_multicall(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;

    let calls = params
        .first()
        .and_then(|v| v.as_array())
        .ok_or_else(|| make_error(ERR_INVALID_PARAMS, "Missing methods array"))?;

    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        let method = call
            .get("methodName")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let call_params = call
            .get("params")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        match Box::pin(dispatch_method(ctx, method, call_params)).await {
            Ok(result) => results.push(Value::Array(vec![Value::Null, result])),
            Err(e) => results.push(Value::Array(vec![
                serde_json::json!({ "code": e.code, "message": e.message }),
                Value::Null,
            ])),
        }
    }
    Ok(Value::Array(results))
}

async fn handle_get_option(ctx: &RpcContext, params: Vec<Value>) -> Result<Value, JsonRpcError> {
    let params = strip_token(params);
    check_token(ctx, &params)?;
    let gid = extract_gid(&params)?;
    let task_id = resolve_gid(ctx, &gid)
        .await
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    let raw_id = task_id.raw_id();
    let summary = get_all_summaries(ctx)
        .await?
        .into_iter()
        .find(|s| s.id == raw_id)
        .ok_or_else(|| make_error(1, format!("GID not found: {gid}")))?;

    // Extract parent directory from destination_path
    let dir = std::path::Path::new(&summary.destination_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(&summary.destination_path)
        .to_string();

    // Convert priority to aria2 position string if available
    let position = (summary.priority as u8).to_string();

    let mut map = serde_json::Map::new();
    map.insert("dir".to_string(), Value::String(dir));
    map.insert("out".to_string(), Value::String(summary.file_name));
    map.insert(
        "split".to_string(),
        Value::String(
            summary
                .requested_thread_count
                .map_or_else(|| "5".to_string(), |n| n.to_string()),
        ),
    );
    map.insert(
        "max-connection-per-server".to_string(),
        Value::String(summary.connection_count.to_string()),
    );
    map.insert("piece-length".to_string(), Value::String("1048576".to_string()));
    map.insert("allow-overwrite".to_string(), Value::String("false".to_string()));
    map.insert(
        "allow-piece-length-change".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("always-resume".to_string(), Value::String("true".to_string()));
    map.insert("async-dns".to_string(), Value::String("true".to_string()));
    map.insert(
        "auto-file-renaming".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "auto-save-interval".to_string(),
        Value::String("60".to_string()),
    );
    map.insert(
        "conditional-get".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "connect-timeout".to_string(),
        Value::String("60".to_string()),
    );
    map.insert(
        "content-disposition-default-utf8".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("continue".to_string(), Value::String("true".to_string()));
    map.insert("dry-run".to_string(), Value::String("false".to_string()));
    map.insert(
        "enable-http-keep-alive".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "enable-http-pipelining".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("enable-mmap".to_string(), Value::String("false".to_string()));
    map.insert(
        "enable-peer-exchange".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "file-allocation".to_string(),
        Value::String("none".to_string()),
    );
    map.insert(
        "follow-metalink".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "follow-torrent".to_string(),
        Value::String("true".to_string()),
    );
    map.insert("force-save".to_string(), Value::String("false".to_string()));
    map.insert("ftp-passwd".to_string(), Value::String("".to_string()));
    map.insert("ftp-user".to_string(), Value::String("".to_string()));
    map.insert("gid".to_string(), Value::String(gid));
    map.insert(
        "hash-check-only".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "http-accept-gzip".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "http-auth-challenge".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("http-no-cache".to_string(), Value::String("false".to_string()));
    map.insert(
        "lowest-speed-limit".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "max-file-not-found".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "max-resume-failure-tries".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "max-tries".to_string(),
        Value::String("5".to_string()),
    );
    map.insert(
        "max-upload-limit".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "metalink-base-uri".to_string(),
        Value::String("".to_string()),
    );
    map.insert(
        "metalink-enable-unique-protocol".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "metalink-language".to_string(),
        Value::String("".to_string()),
    );
    map.insert(
        "metalink-location".to_string(),
        Value::String("".to_string()),
    );
    map.insert("metalink-os".to_string(), Value::String("".to_string()));
    map.insert(
        "metalink-preferred-protocol".to_string(),
        Value::String("http".to_string()),
    );
    map.insert(
        "metalink-version".to_string(),
        Value::String("".to_string()),
    );
    map.insert("min-split-size".to_string(), Value::String("20M".to_string()));
    map.insert(
        "no-file-allocation-limit".to_string(),
        Value::String("5M".to_string()),
    );
    map.insert("no-netrc".to_string(), Value::String("false".to_string()));
    map.insert(
        "parameterized-uri".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("pause".to_string(), Value::String("false".to_string()));
    map.insert(
        "pause-metadata".to_string(),
        Value::String("false".to_string()),
    );
    map.insert(
        "proxy-method".to_string(),
        Value::String("get".to_string()),
    );
    map.insert(
        "realtime-chunk-checksum".to_string(),
        Value::String("true".to_string()),
    );
    map.insert("referer".to_string(), Value::String("".to_string()));
    map.insert("remote-time".to_string(), Value::String("false".to_string()));
    map.insert(
        "remove-control-file".to_string(),
        Value::String("false".to_string()),
    );
    map.insert("retry-wait".to_string(), Value::String("0".to_string()));
    map.insert("reuse-uri".to_string(), Value::String("true".to_string()));
    map.insert(
        "rpc-save-upload-metadata".to_string(),
        Value::String("true".to_string()),
    );
    map.insert("save-cookies".to_string(), Value::String("false".to_string()));
    map.insert(
        "save-not-found".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "save-session-interval".to_string(),
        Value::String("0".to_string()),
    );
    map.insert("seed-ratio".to_string(), Value::String("1.0".to_string()));
    map.insert("seed-time".to_string(), Value::String("0".to_string()));
    map.insert("server-stat-of".to_string(), Value::String("".to_string()));
    map.insert(
        "server-stat-timeout".to_string(),
        Value::String("86400".to_string()),
    );
    map.insert(
        "show-console-readout".to_string(),
        Value::String("true".to_string()),
    );
    map.insert(
        "socket-recv-buffer-size".to_string(),
        Value::String("0".to_string()),
    );
    map.insert("stderr".to_string(), Value::String("false".to_string()));
    map.insert("stop".to_string(), Value::String("0".to_string()));
    map.insert(
        "stop-with-process".to_string(),
        Value::String("0".to_string()),
    );
    map.insert(
        "stream-piece-selector".to_string(),
        Value::String("default".to_string()),
    );
    map.insert(
        "summary-interval".to_string(),
        Value::String("60".to_string()),
    );
    map.insert("timeout".to_string(), Value::String("60".to_string()));
    map.insert(
        "uri-selector".to_string(),
        Value::String("feedback".to_string()),
    );
    map.insert("use-head".to_string(), Value::String("false".to_string()));
    map.insert("user-agent".to_string(), Value::String("".to_string()));
    map.insert("position".to_string(), Value::String(position));

    Ok(Value::Object(map))
}

async fn handle_purge_download_result(ctx: &RpcContext) -> Result<Value, JsonRpcError> {
    let all = get_all_summaries(ctx).await?;

    // Collect TaskIds for terminal downloads
    let terminal: Vec<(TaskId, String)> = all
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            )
        })
        .filter_map(|s| {
            let task_id = match s.kind {
                TaskKind::Http => Uuid::parse_str(&s.id).ok().map(TaskId::Http),
                TaskKind::Bt => Id20::from_hex(&s.id).ok().map(TaskId::Bt),
            };
            task_id.map(|tid| (tid, s.id.clone()))
        })
        .collect();

    let purged_count = terminal.len();

    // Remove each terminal download (keep files on disk)
    for (task_id, _id) in &terminal {
        let _ = ctx.dispatcher.remove(task_id).await;
    }

    // Clean up gid_cache entries for purged downloads
    {
        let mut cache = ctx.gid_cache.lock().await;
        for (_task_id, raw_id) in &terminal {
            let gid = internal_id_to_gid(raw_id);
            cache.remove(&gid);
        }
    }

    tracing::info!("Purged {purged_count} completed/error/removed downloads");
    Ok(Value::String("OK".to_string()))
}

fn handle_get_session_info(ctx: &RpcContext) -> Value {
    serde_json::json!({ "sessionId": ctx.session_id })
}

fn handle_save_session() -> Value {
    // limedl auto-persists to SQLite on every state change; no explicit save needed
    Value::String("OK".to_string())
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
            "aria2.addTorrent",
            "aria2.addUri",
            "aria2.changeGlobalOption",
            "aria2.getFiles",
            "aria2.getGlobalOption",
            "aria2.getGlobalStat",
            "aria2.getOption",
            "aria2.getPeers",
            "aria2.getSessionInfo",
            "aria2.getUris",
            "aria2.getVersion",
            "aria2.multicall",
            "aria2.pause",
            "aria2.forcePause",
            "aria2.pauseAll",
            "aria2.forcePauseAll",
            "aria2.purgeDownloadResult",
            "aria2.remove",
            "aria2.forceRemove",
            "aria2.saveSession",
            "aria2.shutdown",
            "aria2.tellActive",
            "aria2.tellStatus",
            "aria2.tellStopped",
            "aria2.tellWaiting",
            "aria2.unpause",
            "aria2.unpauseAll",
            "system.listMethods",
            "system.listNotifications",
            "system.multicall",
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
    let mut all = Vec::new();
    for backend in ctx.registry.iter() {
        match backend.list().await {
            Ok(summaries) => all.extend(summaries),
            Err(e) => tracing::warn!("get_all_summaries: backend list failed: {e}"),
        }
    }
    Ok(all)
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
    let mut event_rx = ctx.event_bus.subscribe();

    let sender_events = sender.clone();
    let mut send_events = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(DownloadEvent::Aria2Notification { event_name, gid }) => {
                    let notification = serde_json::to_string(&JsonRpcNotification {
                        jsonrpc: "2.0",
                        method: event_name,
                        params: vec![serde_json::json!({"gid": gid})],
                    })
                    .unwrap_or_default();
                    if sender_events
                        .lock()
                        .await
                        .send(Message::Text(notification.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(_) => {} // ignore non-Aria2 events
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
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
        registry: Arc<BackendRegistry>,
        settings: &Aria2RpcSettings,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let secret = settings.secret.clone().filter(|s| !s.is_empty());

        let dispatcher = Dispatcher::new(registry.clone(), event_bus.clone());
        let ctx = Arc::new(RpcContext {
            registry,
            secret,
            event_bus,
            dispatcher,
            gid_cache: Mutex::new(HashMap::default()),
            session_id: uuid::Uuid::new_v4().to_string(),
        });

        Aria2RpcServer {
            ctx,
            addr: format!("127.0.0.1:{}", settings.port),
        }
    }

    pub async fn serve(
        self,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
        cors_allowed_origins: Vec<String>,
    ) -> anyhow::Result<()> {
        // Build CORS layer with configurable origins
        let cors = if cors_allowed_origins.is_empty() {
            // Default: localhost only
            CorsLayer::new()
                .allow_origin([
                    "http://localhost".parse::<HeaderValue>().unwrap(),
                    "http://127.0.0.1".parse::<HeaderValue>().unwrap(),
                ])
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                .allow_credentials(true)
                .max_age(Duration::from_secs(86400))
        } else {
            // Use configured origins
            let origins: Vec<HeaderValue> = cors_allowed_origins
                .iter()
                .filter_map(|o| o.parse::<HeaderValue>().ok())
                .collect();

            if origins.is_empty() {
                // All configured origins failed to parse — warn and fall back to localhost
                tracing::warn!(
                    "All configured CORS origins failed to parse: {:?}. Falling back to localhost.",
                    cors_allowed_origins
                );
                CorsLayer::new()
                    .allow_origin([
                        "http://localhost".parse::<HeaderValue>().unwrap(),
                        "http://127.0.0.1".parse::<HeaderValue>().unwrap(),
                    ])
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                    .allow_credentials(true)
                    .max_age(Duration::from_secs(86400))
            } else {
                CorsLayer::new()
                    .allow_origin(origins)
                    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
                    .allow_credentials(true)
                    .max_age(Duration::from_secs(86400))
            }
        };

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

#[cfg(test)]
#[path = "tests/aria2_rpc_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/aria2_rpc_e2e_tests.rs"]
mod e2e_tests;
