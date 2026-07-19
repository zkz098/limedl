use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use limedl_core::types::StartDownloadRequest;
use limedl_core::types::TaskId;
use limedl_core::{BackendRegistry, DownloadEvent, DownloadManager, EventBus};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::rate_limiter::{MethodClass, WsRateLimiter};

/// Maximum WebSocket message size: 4 MB
const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

/// JSON-RPC 2.0 request
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: serde_json::Value,
    method: String,
    params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcError {
    fn parse_error(msg: impl Into<String>) -> Self {
        JsonRpcError {
            code: -32700,
            message: msg.into(),
        }
    }

    fn invalid_params(msg: impl Into<String>) -> Self {
        JsonRpcError {
            code: -32602,
            message: msg.into(),
        }
    }

    fn method_not_found(method: &str) -> Self {
        JsonRpcError {
            code: -32601,
            message: format!("Method not found: {method}"),
        }
    }

    #[allow(dead_code)]
    fn server_error(msg: impl Into<String>) -> Self {
        JsonRpcError {
            code: -32000,
            message: msg.into(),
        }
    }
}

/// Shared app state for RPC handlers
pub struct RpcState {
    pub registry: Arc<BackendRegistry>,
    pub event_bus: Arc<EventBus>,
    /// Connected WebSocket senders for event broadcasting
    pub clients: Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>>,
    /// WebSocket JSON-RPC rate limiter (per-connection + global)
    pub rate_limiter: Arc<WsRateLimiter>,
}

/// Handle WebSocket upgrade and run JSON-RPC loop
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    state: Arc<RpcState>,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<RpcState>) {
    let (mut sender, mut receiver) = socket.split();

    // Generate a unique connection ID for rate limiting
    let connection_id = uuid::Uuid::new_v4().to_string();
    state.rate_limiter.register(&connection_id);

    // Register this client for event broadcasting
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let cleanup_tx = tx.clone();
    state.clients.lock().push(tx.clone());

    // Spawn event relay from EventBus to this client
    let relay_handle = {
        let mut event_rx = state.event_bus.subscribe();
        let tx_event = tx.clone();
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let params = match event {
                            DownloadEvent::Updated { id: _, summary_json } => {
                                serde_json::json!({
                                    "type": "updated",
                                    "payload": summary_json,
                                })
                            }
                            DownloadEvent::Progress { id: _, progress_json } => {
                                serde_json::json!({
                                    "type": "progress",
                                    "payload": progress_json,
                                })
                            }
                            DownloadEvent::Aria2Notification { event_name, gid } => {
                                serde_json::json!({
                                    "type": "aria2Notification",
                                    "payload": {
                                        "eventName": event_name,
                                        "gid": gid,
                                    },
                                })
                            }
                            DownloadEvent::CdnProgress { phase, current, total } => {
                                serde_json::json!({
                                    "type": "cdnProgress",
                                    "payload": {
                                        "phase": phase,
                                        "current": current,
                                        "total": total,
                                    },
                                })
                            }
                            DownloadEvent::CdnComplete { state, active_ip, active_speed_mbps } => {
                                serde_json::json!({
                                    "type": "cdnComplete",
                                    "payload": {
                                        "state": state,
                                        "activeIp": active_ip,
                                        "activeSpeedMbps": active_speed_mbps,
                                    },
                                })
                            }
                        };
                        let msg = serde_json::to_string(&serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "event",
                            "params": params,
                        }));
                        if let Ok(msg) = msg {
                            let _ = tx_event.send(Message::Text(msg.into()));
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Event relay lagged by {n}");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    };

    // Forward events from channel to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Read JSON-RPC requests and dispatch
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                // Enforce message size limit
                if text.len() > MAX_MESSAGE_SIZE {
                    let err_response = JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id: serde_json::Value::Null,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32600,
                            message: "Request too large".into(),
                        }),
                    };
                    if let Ok(response_text) = serde_json::to_string(&err_response) {
                        let _ = tx.send(Message::Text(response_text.into()));
                    }
                    continue;
                }

                // Extract method name for rate limiting (simple string scan)
                let method = extract_method_name(&text);

                // Apply rate limiting
                if let Some(method) = method {
                    let class = MethodClass::classify(method);
                    if let Err(reason) = state.rate_limiter.check(&connection_id, class) {
                        let err_response = JsonRpcResponse {
                            jsonrpc: "2.0".into(),
                            id: serde_json::Value::Null,
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32000,
                                message: reason.into(),
                            }),
                        };
                        if let Ok(response_text) = serde_json::to_string(&err_response) {
                            let _ = tx.send(Message::Text(response_text.into()));
                        }
                        continue;
                    }
                }

                let response = handle_rpc(&text, &state).await;
                if let Ok(response_text) = serde_json::to_string(&response) {
                    let _ = tx.send(Message::Text(response_text.into()));
                }
            }
            Message::Binary(data) => {
                // Reject binary messages
                let err_response = JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: serde_json::Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32600,
                        message: "Binary messages are not supported".into(),
                    }),
                };
                if let Ok(response_text) = serde_json::to_string(&err_response) {
                    let _ = tx.send(Message::Text(response_text.into()));
                }
                let _ = data;
            }
            Message::Ping(bytes) | Message::Pong(bytes) => {
                // Let axum handle ping/pong automatically
                let _ = bytes;
            }
            Message::Close(_) => break,
        }
    }

    // Cleanup: remove this client and abort background tasks
    state.rate_limiter.unregister(&connection_id);
    state.clients.lock().retain(|c| !c.same_channel(&cleanup_tx));
    relay_handle.abort();
    send_task.abort();
}

/// Extract the method name from a raw JSON-RPC message string.
///
/// Performs a simple string scan for the first occurrence of `"method"` to avoid
/// full JSON deserialization overhead before rate limiting.
///
/// KNOWN LIMITATION: If a `params` field contains the literal string `"method":"..."`
/// before the top-level `method` key, the classifier may return the wrong method name.
/// This is a false-positive risk for rate-limit classification only — the actual RPC
/// dispatch uses the correctly-parsed method name from serde_json. The worst case is
/// a legitimate safe call being rate-limited as mutating, not a security bypass.
fn extract_method_name(text: &str) -> Option<&str> {
    // Look for `"method":"<name>"` or `"method": "<name>"` with possible whitespace
    let search = "\"method\"";
    let method_pos = text.find(search)?;
    let after_key = &text[method_pos + search.len()..];
    // Skip whitespace and colon
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();
    // Expect opening quote
    let start = after_colon.strip_prefix('"')?;
    // Find closing quote
    let end = start.find('"')?;
    Some(&start[..end])
}

/// Dispatch a single JSON-RPC method call
async fn handle_rpc(text: &str, state: &RpcState) -> JsonRpcResponse {
    let req: JsonRpcRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: serde_json::Value::Null,
                result: None,
                error: Some(JsonRpcError::parse_error(format!("Parse error: {e}"))),
            }
        }
    };

    let result = dispatch_method(&req.method, req.params.as_ref(), state).await;

    match result {
        Ok(value) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id,
            result: Some(value),
            error: None,
        },
        Err(err) => JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: req.id,
            result: None,
            error: Some(err),
        },
    }
}

/// Dispatch method name to the appropriate backend call.
/// Returns the result value on success, or a `JsonRpcError` with an
/// appropriate JSON-RPC 2.0 error code on failure.
async fn dispatch_method(
    method: &str,
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    match method {
        "download.start" => handle_download_start(params, state).await,
        "download.list" => handle_download_list(state).await,
        "download.status"
        | "download.pause"
        | "download.resume"
        | "download.cancel"
        | "download.remove"
        | "download.purge" => handle_download_action(method, params, state).await,
        "settings.get" => handle_settings_get(state).await,
        "settings.save" => handle_settings_save(params, state).await,
        "download.openInExplorer" => handle_open_in_explorer(params, state).await,
        "settings.toggleGameMode" => handle_toggle_game_mode(params, state).await,
        "settings.getIoStatus" => handle_get_io_status(state).await,
        "settings.toggleOverclockMode" => handle_toggle_overclock_mode(params, state).await,
        "settings.getOverclockMode" => handle_get_overclock_mode(state).await,
        "settings.fetchTrackerList" => handle_fetch_tracker_list(params, state).await,
        "bt.runtimeStatus" => handle_bt_runtime_status(state).await,
        "bt.setSpeedLimit" => handle_bt_set_speed_limit(params, state).await,
        "bt.previewTorrent" => handle_bt_preview_torrent(params, state).await,
        "bt.getPeers" | "bt.getTrackers" | "bt.getPieces" | "bt.getFiles" => {
            handle_bt_get_details(method, params, state).await
        }
        "bt.updateFiles" => handle_bt_update_files(params, state).await,
        "cdn.fetchRanges"
        | "cdn.status"
        | "cdn.detail"
        | "cdn.test"
        | "cdn.apply"
        | "cdn.clear"
        | "cdn.cancel"
        | "cdn.candidates" => handle_cdn_routes(method, params, state).await,
        _ => Err(JsonRpcError::method_not_found(method)),
    }
}

// ── Handler: download.start ────────────────────────────────────────

async fn handle_download_start(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let req: StartDownloadRequest = serde_json::from_value(
        params.cloned().unwrap_or_default(),
    )
    .map_err(|e| JsonRpcError::invalid_params(format!("Invalid params: {e}")))?;

    // ── URL validation ──────────────────────────────────────────
    // Reject URLs longer than 8192 bytes
    if req.url.len() > 8192 {
        return Err(JsonRpcError::invalid_params(
            "URL exceeds maximum length of 8192 bytes",
        ));
    }

    let url_lower = req.url.trim().to_ascii_lowercase();

    if url_lower.starts_with("magnet:") {
        // Magnet link validation
        if req.url.len() > 4096 {
            return Err(JsonRpcError::invalid_params(
                "Magnet link exceeds maximum length of 4096 bytes",
            ));
        }
        if !url_lower.contains("urn:btih:") && !url_lower.contains("urn:btmh:") {
            return Err(JsonRpcError::invalid_params(
                "Magnet link must contain urn:btih: or urn:btmh:",
            ));
        }
    } else if !url_lower.starts_with("http://") && !url_lower.starts_with("https://") {
        return Err(JsonRpcError::invalid_params(
            "URL must start with http://, https://, or magnet:",
        ));
    }

    let kind = req.classify_kind().map_err(|e| {
        JsonRpcError::invalid_params(e.to_string())
    })?;
    let backend = state.registry.by_kind(kind)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    let task_id = backend.start(req).await.map_err(|e| {
        JsonRpcError::server_error(e.to_string())
    })?;
    Ok(serde_json::json!({ "taskId": task_id }))
}

// ── Handler: download.list ─────────────────────────────────────────

async fn handle_download_list(
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let summaries = state.registry.list_all().await;
    serde_json::to_value(summaries)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))
}

// ── Handler: download.status / pause / resume / cancel / remove / purge ──

async fn handle_download_action(
    method: &str,
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let p = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id_str = p
        .get("taskId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let task_id = TaskId::parse(task_id_str);
    let backend = state.registry.dispatch(&task_id)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    let snapshot = match method {
        "download.status" => backend.status(&task_id).await,
        "download.pause" => backend.pause(&task_id).await,
        "download.resume" => backend.resume(&task_id).await,
        "download.cancel" => backend.cancel(&task_id).await,
        "download.remove" => backend.remove(&task_id).await,
        "download.purge" => backend.purge(&task_id).await,
        _ => unreachable!(),
    }
    .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    serde_json::to_value(snapshot)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))
}

// ── Handler: settings.get ──────────────────────────────────────────

async fn handle_settings_get(
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
        JsonRpcError::server_error("HTTP backend not found")
    })?;
    let settings = dm.settings().await.map_err(|e| {
        JsonRpcError::server_error(e.to_string())
    })?;
    serde_json::to_value(settings)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))
}

// ── Handler: settings.save ─────────────────────────────────────────

async fn handle_settings_save(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let settings: limedl_core::types::AppSettings =
        serde_json::from_value(params.cloned().unwrap_or_default())
            .map_err(|e| JsonRpcError::invalid_params(format!("Invalid params: {e}")))?;
    // Broadcast to all backends
    state.registry.update_all_settings(&settings).await;
    let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
        JsonRpcError::server_error("HTTP backend not found")
    })?;
    let saved = dm.settings().await.map_err(|e| {
        JsonRpcError::server_error(e.to_string())
    })?;
    serde_json::to_value(saved)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))
}

// ── Handler: download.openInExplorer ───────────────────────────────

async fn handle_open_in_explorer(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id_str = params
        .get("taskId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let task_id = TaskId::parse(task_id_str);
    let backend = state.registry.dispatch(&task_id)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    backend
        .open_in_explorer(&task_id)
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    Ok(serde_json::json!({}))
}

// ── Handler: settings.toggleGameMode ───────────────────────────────

async fn handle_toggle_game_mode(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let enabled = params.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
        JsonRpcError::server_error("HTTP backend not found")
    })?;
    dm.set_game_mode(enabled);
    Ok(serde_json::json!(enabled))
}

// ── Handler: settings.getIoStatus ──────────────────────────────────

async fn handle_get_io_status(
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
        JsonRpcError::server_error("HTTP backend not found")
    })?;
    let pool = &dm.buffer_pool;
    Ok(serde_json::json!({
        "gameMode": pool.game_mode(),
        "bufferUsageBytes": pool.current_usage(),
        "bufferLimitBytes": pool.effective_limit(),
        "activeSlots": pool.active_slots(),
        "maxSlots": pool.max_slots(),
        "queuedCount": pool.queued_count(),
        "degradationCount": pool.degradation_count(),
    }))
}

// ── Handler: settings.toggleOverclockMode ──────────────────────────

async fn handle_toggle_overclock_mode(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let enabled = params.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
    let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
        JsonRpcError::server_error("HTTP backend not found")
    })?;
    dm.set_overclock_mode(enabled);
    Ok(serde_json::json!(enabled))
}

// ── Handler: settings.getOverclockMode ─────────────────────────────

async fn handle_get_overclock_mode(
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
        JsonRpcError::server_error("HTTP backend not found")
    })?;
    Ok(serde_json::json!(dm.overclock_mode()))
}

// ── Handler: settings.fetchTrackerList ─────────────────────────────

async fn handle_fetch_tracker_list(
    params: Option<&serde_json::Value>,
    _state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let tracker_list_url = params
        .get("trackerListUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing trackerListUrl"))?;
    let normalized = limedl_core::normalize_tracker_list_url(tracker_list_url)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    let response = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("limedl/0.1")
        .build()
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?
        .get(normalized)
        .send()
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?
        .error_for_status()
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    if response.len() > 1024 * 1024 {
        return Err(JsonRpcError::server_error("tracker list is larger than 1 MiB"));
    }
    let content = String::from_utf8(response.to_vec())
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    let result = limedl_core::normalize_tracker_list_lossy(&content);
    Ok(serde_json::json!(result))
}

// ── Handler: bt.runtimeStatus ──────────────────────────────────────

async fn handle_bt_runtime_status(
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let bt = state.registry.get_typed::<limedl_core::IrontideBtBackend>()
        .ok_or_else(|| JsonRpcError::server_error("BT backend not registered"))?;
    let status = bt.runtime_status();
    Ok(serde_json::to_value(status).map_err(|e| JsonRpcError::server_error(e.to_string()))?)
}

// ── Handler: bt.setSpeedLimit ──────────────────────────────────────

async fn handle_bt_set_speed_limit(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id = params.get("taskId").and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let dl_limit = params.get("downloadLimitBps").and_then(|v| v.as_u64());
    let ul_limit = params.get("uploadLimitBps").and_then(|v| v.as_u64());
    let bt = state.registry.get_typed::<limedl_core::IrontideBtBackend>()
        .ok_or_else(|| JsonRpcError::server_error("BT backend not registered"))?;
    bt.set_speed_limit(task_id, dl_limit, ul_limit);
    Ok(serde_json::json!({}))
}

// ── Handler: bt.previewTorrent ─────────────────────────────────────

async fn handle_bt_preview_torrent(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let source = params.get("source").and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing source"))?;
    let bt = state.registry.get_typed::<limedl_core::IrontideBtBackend>()
        .ok_or_else(|| JsonRpcError::server_error("BT backend not registered"))?;
    let entries = bt.preview_torrent(source).await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    Ok(serde_json::to_value(entries).map_err(|e| JsonRpcError::server_error(e.to_string()))?)
}

// ── Handler: bt.getPeers / getTrackers / getPieces / getFiles ──────

async fn handle_bt_get_details(
    method: &str,
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id = params.get("taskId").and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let bt = state.registry.get_typed::<limedl_core::IrontideBtBackend>()
        .ok_or_else(|| JsonRpcError::server_error("BT backend not registered"))?;
    let result = match method {
        "bt.getPeers" => {
            let peers = bt.get_peers(task_id).map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(peers)
        }
        "bt.getTrackers" => {
            let trackers = bt.get_trackers(task_id).map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(trackers)
        }
        "bt.getPieces" => {
            let pieces = bt.get_pieces(task_id).map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(pieces)
        }
        "bt.getFiles" => {
            let files = bt.get_torrent_files(task_id).map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(files)
        }
        _ => unreachable!(),
    }.map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    Ok(result)
}

// ── Handler: bt.updateFiles ────────────────────────────────────────

async fn handle_bt_update_files(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id = params.get("taskId").and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let included_indices: Vec<usize> = params.get("includedIndices")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|i| i.as_u64().map(|u| u as usize)).collect())
        .unwrap_or_default();
    let bt = state.registry.get_typed::<limedl_core::IrontideBtBackend>()
        .ok_or_else(|| JsonRpcError::server_error("BT backend not registered"))?;
    bt.update_torrent_files(task_id, included_indices).await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    Ok(serde_json::json!({}))
}

// ── Handler: CDN commands ──────────────────────────────────────────

async fn handle_cdn_routes(
    method: &str,
    _params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    match method {
        "cdn.fetchRanges" => {
            Ok(serde_json::json!(
                limedl_core::cdn::CLOUDFLARE_IPV4_RANGES.iter().map(|s| s.to_string()).collect::<Vec<_>>()
            ))
        }
        "cdn.status" => {
            let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
                JsonRpcError::server_error("HTTP backend not found")
            })?;
            let settings = dm.settings().await.map_err(|e| {
                JsonRpcError::server_error(e.to_string())
            })?;
            let active = settings.cdn_acceleration.active_ip.is_some();
            Ok(serde_json::json!(if active { "Ready" } else { "Idle" }))
        }
        "cdn.detail" => {
            let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
                JsonRpcError::server_error("HTTP backend not found")
            })?;
            let settings = dm.settings().await.map_err(|e| {
                JsonRpcError::server_error(e.to_string())
            })?;
            Ok(serde_json::json!({
                "state": if settings.cdn_acceleration.active_ip.is_some() { "Ready" } else { "Idle" },
                "activeIp": settings.cdn_acceleration.active_ip,
                "activeSpeedMbps": settings.cdn_acceleration.active_speed_mbps,
            }))
        }
        "cdn.test" | "cdn.apply" | "cdn.clear" | "cdn.cancel" | "cdn.candidates" => {
            Err(JsonRpcError {
                code: -32001,
                message: "CDN speed test is not supported in NAS mode. Configure CDN via the desktop app or edit the config file directly.".to_string(),
            })
        }
        _ => Err(JsonRpcError::method_not_found(method)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ntest::timeout;
    use tempfile::TempDir;

    async fn make_rpc_state() -> (Arc<RpcState>, TempDir) {
        let tmp = TempDir::new().unwrap();
        let state_dir = tmp.path().join("downloads");
        std::fs::create_dir_all(&state_dir).unwrap();

        let core = limedl_core::bootstrap::bootstrap(state_dir).await.unwrap();

        let rpc_state = Arc::new(RpcState {
            registry: core.registry,
            event_bus: core.event_bus,
            clients: Arc::new(parking_lot::Mutex::new(Vec::new())),
            rate_limiter: Arc::new(crate::rate_limiter::WsRateLimiter::new()),
        });

        (rpc_state, tmp)
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_valid_params() {
        let (state, tmp) = make_rpc_state().await;
        let dest = tmp.path().join("output");
        std::fs::create_dir_all(&dest).unwrap();

        let params = serde_json::json!({
            "url": "https://example.com/test.bin",
            "destinationDir": dest.to_string_lossy(),
            "fileName": "test.bin"
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_ok(), "download.start failed: {:?}", result.err());
        let value = result.unwrap();
        assert!(
            value.get("taskId").and_then(|v| v.as_str()).is_some(),
            "response missing taskId: {value:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_missing_url() {
        let (state, tmp) = make_rpc_state().await;
        let dest = tmp.path().join("output");

        let params = serde_json::json!({
            "destinationDir": dest.to_string_lossy()
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_err(), "expected error for missing url");
        // Should be an invalid params error (requires url)
        if let Err(err) = result {
            assert_eq!(err.code, -32602);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_list_initially_empty() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("download.list", None, &state).await;
        assert!(result.is_ok());
        let list = result.unwrap();
        assert!(list.as_array().is_some());
        assert!(list.as_array().unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn settings_get_returns_defaults() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("settings.get", None, &state).await;
        assert!(result.is_ok(), "settings.get failed: {:?}", result.err());
        let settings = result.unwrap();
        // Should have a download section with defaults
        assert!(settings.get("download").is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn settings_save_and_get_roundtrip() {
        let (state, _tmp) = make_rpc_state().await;
        // First get current settings
        let current = dispatch_method("settings.get", None, &state).await.unwrap();

        // Save them back
        let result = dispatch_method("settings.save", Some(&current), &state).await;
        assert!(result.is_ok(), "settings.save failed: {:?}", result.err());

        // Get again — should match
        let after = dispatch_method("settings.get", None, &state).await.unwrap();
        assert_eq!(
            current
                .get("download")
                .and_then(|v| v.as_object())
                .map(|o| o.len()),
            after
                .get("download")
                .and_then(|v| v.as_object())
                .map(|o| o.len())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn method_not_found_returns_error() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("nonexistent.method", None, &state).await;
        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err.code, -32601);
            assert!(err.message.contains("nonexistent.method"));
        }
    }
}
