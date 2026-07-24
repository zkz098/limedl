use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use futures_util::{SinkExt, StreamExt};
use limedl_core::types::StartDownloadRequest;
use limedl_core::types::TaskId;
use limedl_core::{
    BackendRegistry, CdnService, Dispatcher, DownloadEvent, DownloadManager, EventBus,
};
use limedl_core::ws_manifest::WS_COMMANDS;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::rate_limiter::{MethodClass, WsRateLimiter};

/// Maximum WebSocket message size: 4 MB
const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

/// Maximum number of concurrent WebSocket connections
const MAX_WS_CONNECTIONS: usize = 50;

/// Bounded channel capacity for WebSocket event forwarding (per client)
const WS_EVENT_CHANNEL_CAPACITY: usize = 256;

/// JSON-RPC 2.0 request
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // fields read by serde Deserialize but not accessed directly
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
    pub clients: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<Message>>>>,
    /// WebSocket JSON-RPC rate limiter (per-connection + global)
    pub rate_limiter: Arc<WsRateLimiter>,
    /// CDN acceleration service
    pub cdn_service: Arc<CdnService>,
}

/// Handle WebSocket upgrade and run JSON-RPC loop
pub async fn ws_handler(ws: WebSocketUpgrade, state: Arc<RpcState>) -> axum::response::Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<RpcState>) {
    let (mut sender, mut receiver) = socket.split();

    // Generate a unique connection ID for rate limiting
    let connection_id = uuid::Uuid::new_v4().to_string();
    state.rate_limiter.register(&connection_id);

    // Check connection limit BEFORE registering
    let limit_reached = {
        let clients = state.clients.lock();
        clients.len() >= MAX_WS_CONNECTIONS
    };
    if limit_reached {
        tracing::warn!(
            "WebSocket connection rejected: max connections ({MAX_WS_CONNECTIONS}) reached"
        );
        // Close the connection gracefully
        let _ = sender.close().await;
        return;
    }

    // Register this client for event broadcasting
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(WS_EVENT_CHANNEL_CAPACITY);
    let cleanup_tx = tx.clone();
    state.clients.lock().push(tx.clone());

    // Spawn event relay from EventBus to this client
    let relay_handle = {
        let mut event_rx = state.event_bus.subscribe();
        let tx_event = tx.clone();
        let registry = state.registry.clone();
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let params = match event {
                            DownloadEvent::Updated {
                                id: _,
                                summary_json,
                            } => {
                                serde_json::json!({
                                    "type": "updated",
                                    "payload": summary_json,
                                })
                            }
                            DownloadEvent::Progress {
                                id: _,
                                progress_json,
                            } => {
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
                            DownloadEvent::CdnProgress {
                                phase,
                                current,
                                total,
                            } => {
                                serde_json::json!({
                                    "type": "cdnProgress",
                                    "payload": {
                                        "phase": phase,
                                        "current": current,
                                        "total": total,
                                    },
                                })
                            }
                            DownloadEvent::CdnComplete {
                                state,
                                active_ip,
                                active_speed_mbps,
                            } => {
                                serde_json::json!({
                                    "type": "cdnComplete",
                                    "payload": {
                                        "state": state,
                                        "activeIp": active_ip,
                                        "activeSpeedMbps": active_speed_mbps,
                                    },
                                })
                            }
                            DownloadEvent::Warning { id, message } => {
                                serde_json::json!({
                                    "type": "warning",
                                    "payload": {
                                        "id": id,
                                        "message": message,
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
                            let _ = tx_event.send(Message::Text(msg.into())).await;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Event relay lagged by {n}");
                        let all_downloads = registry.list_all().await;
                        let msg = serde_json::to_string(&serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "event",
                            "params": {
                                "type": "fullState",
                                "payload": all_downloads,
                            },
                        }));
                        if let Ok(msg) = msg {
                            let _ = tx_event.send(Message::Text(msg.into())).await;
                        }
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
                        let _ = tx.send(Message::Text(response_text.into())).await;
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
                            let _ = tx.send(Message::Text(response_text.into())).await;
                        }
                        continue;
                    }
                }

                let response = handle_rpc(&text, &state).await;
                if let Ok(response_text) = serde_json::to_string(&response) {
                    let _ = tx.send(Message::Text(response_text.into())).await;
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
                    let _ = tx.send(Message::Text(response_text.into())).await;
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
    state
        .clients
        .lock()
        .retain(|c| !c.same_channel(&cleanup_tx));
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
            };
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

/// Lookup table: rpc_method → tauri_name, built once from WS_COMMANDS.
static RPC_METHOD_TO_TAURI: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();

fn rpc_to_tauri(method: &str) -> Option<&'static str> {
    RPC_METHOD_TO_TAURI
        .get_or_init(|| {
            WS_COMMANDS
                .iter()
                .map(|c| (c.rpc_method, c.tauri_name))
                .collect()
        })
        .get(method)
        .copied()
}

/// Dispatch method name to the appropriate backend call.
/// Uses the rpc_method → tauri_name lookup from WS_COMMANDS to derive the
/// routing key, eliminating the dual-source problem between rpc.rs and
/// ws_manifest.rs.
async fn dispatch_method(
    method: &str,
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let tauri_name =
        rpc_to_tauri(method).ok_or_else(|| JsonRpcError::method_not_found(method))?;
    match tauri_name {
        "download_start" => handle_download_start(params, state).await,
        "download_list" => handle_download_list(state).await,
        "download_pause" | "download_resume" | "download_cancel" | "download_remove"
        | "download_purge" | "download_status" => {
            handle_download_action(method, params, state).await
        }
        "settings_get" => handle_settings_get(state).await,
        "settings_save" => handle_settings_save(params, state).await,
        "download_open_in_explorer" => handle_open_in_explorer(params, state).await,
        "toggle_game_mode" => handle_toggle_game_mode(params, state).await,
        "get_io_status" => handle_get_io_status(state).await,
        "detect_disk_type" => handle_detect_disk_type(params, state).await,
        "toggle_overclock_mode" => handle_toggle_overclock_mode(params, state).await,
        "get_overclock_mode" => handle_get_overclock_mode(state).await,
        "settings_fetch_tracker_list" => handle_fetch_tracker_list(params, state).await,
        "bt_runtime_status" => handle_bt_runtime_status(state).await,
        "bt_set_speed_limit" => handle_bt_set_speed_limit(params, state).await,
        "bt_preview_torrent" => handle_bt_preview_torrent(params, state).await,
        "bt_get_peers" | "bt_get_trackers" | "bt_get_pieces" | "get_bt_files" => {
            handle_bt_get_details(method, params, state).await
        }
        "update_bt_files" => handle_bt_update_files(params, state).await,
        "cdn_fetch_ranges" | "cdn_status" | "cdn_detail" | "cdn_test" | "cdn_apply"
        | "cdn_clear" | "cdn_cancel" | "cdn_candidates" => {
            handle_cdn_routes(method, params, state).await
        }
        "update_tray_language" => handle_update_tray_language(params, state).await,
        _ => Err(JsonRpcError::method_not_found(method)),
    }
}

// ── Handler: tray.updateLanguage ────────────────────────────────────

async fn handle_update_tray_language(
    params: Option<&serde_json::Value>,
    _state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let _language = params
        .get("language")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing language"))?;
    // NAS/web mode has no system tray — no-op
    Ok(serde_json::json!(true))
}

// ── Handler: download.start ────────────────────────────────────────

fn make_dispatcher(state: &RpcState) -> Dispatcher {
    Dispatcher::new(state.registry.clone(), state.event_bus.clone())
}

async fn handle_download_start(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let req: StartDownloadRequest = serde_json::from_value(params.cloned().unwrap_or_default())
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

    let dispatcher = make_dispatcher(state);
    let task_id = dispatcher
        .start(req)
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;

    // Emit initial state so the frontend sees the new task immediately.
    // (BT backend already emits via emit_pending_summary; extra emit is harmless.)
    if let Ok(snapshot) = dispatcher.status(&task_id).await {
        dispatcher.emit_updated(&snapshot);
    }

    Ok(serde_json::json!({ "taskId": task_id }))
}

// ── Handler: download.list ─────────────────────────────────────────

async fn handle_download_list(state: &RpcState) -> Result<serde_json::Value, JsonRpcError> {
    let dispatcher = make_dispatcher(state);
    let summaries = dispatcher
        .list()
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    serde_json::to_value(summaries).map_err(|e| JsonRpcError::server_error(e.to_string()))
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
    let task_id = TaskId::from_legacy_string(task_id_str)
        .map_err(|e| JsonRpcError::invalid_params(format!("Invalid task ID: {e}")))?;
    let dispatcher = make_dispatcher(state);
    let snapshot = match method {
        "download.status" => dispatcher.status(&task_id).await,
        "download.pause" => dispatcher.pause(&task_id).await,
        "download.resume" => dispatcher.resume(&task_id).await,
        "download.cancel" => dispatcher.cancel(&task_id).await,
        "download.remove" => dispatcher.remove(&task_id).await,
        "download.purge" => dispatcher.purge(&task_id).await,
        _ => return Err(JsonRpcError::method_not_found(method)),
    }
    .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    serde_json::to_value(snapshot).map_err(|e| JsonRpcError::server_error(e.to_string()))
}

// ── Handler: settings.get ──────────────────────────────────────────

async fn handle_settings_get(state: &RpcState) -> Result<serde_json::Value, JsonRpcError> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?;
    let settings = dm
        .settings()
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    serde_json::to_value(settings).map_err(|e| JsonRpcError::server_error(e.to_string()))
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
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?;
    let saved = dm
        .settings()
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    serde_json::to_value(saved).map_err(|e| JsonRpcError::server_error(e.to_string()))
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
    let task_id = TaskId::from_legacy_string(task_id_str)
        .map_err(|e| JsonRpcError::invalid_params(format!("Invalid task ID: {e}")))?;
    let backend = state
        .registry
        .dispatch(&task_id)
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
    let enabled = params
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?;
    dm.set_game_mode(enabled);
    Ok(serde_json::json!(enabled))
}

// ── Handler: settings.getIoStatus ──────────────────────────────────

async fn handle_get_io_status(state: &RpcState) -> Result<serde_json::Value, JsonRpcError> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?;
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

// ── Handler: settings.detectDiskType ────────────────────────────────

async fn handle_detect_disk_type(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let dir = params
        .get("dir")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing dir"))?;
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?;
    let disk_type = dm.resolve_disk_type(std::path::Path::new(dir)).await;
    Ok(serde_json::json!(match disk_type {
        limedl_core::types::DiskType::Hdd => "hdd",
        limedl_core::types::DiskType::Ssd => "ssd",
    }))
}

// ── Handler: settings.toggleOverclockMode ──────────────────────────

async fn handle_toggle_overclock_mode(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let enabled = params
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?;
    dm.set_overclock_mode(enabled);
    Ok(serde_json::json!(enabled))
}

// ── Handler: settings.getOverclockMode ─────────────────────────────

async fn handle_get_overclock_mode(state: &RpcState) -> Result<serde_json::Value, JsonRpcError> {
    let dm = state
        .registry
        .get_typed::<DownloadManager>()
        .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?;
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
        return Err(JsonRpcError::server_error(
            "tracker list is larger than 1 MiB",
        ));
    }
    let content = String::from_utf8(response.to_vec())
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    let result = limedl_core::normalize_tracker_list_lossy(&content);
    Ok(serde_json::json!(result))
}

// ── Handler: bt.runtimeStatus ──────────────────────────────────────

async fn handle_bt_runtime_status(state: &RpcState) -> Result<serde_json::Value, JsonRpcError> {
    let dispatcher = make_dispatcher(state);
    let status = dispatcher
        .bt_runtime_status()
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    serde_json::to_value(status).map_err(|e| JsonRpcError::server_error(e.to_string()))
}

// ── Handler: bt.setSpeedLimit ──────────────────────────────────────

async fn handle_bt_set_speed_limit(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id_str = params
        .get("taskId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let dl_limit = params.get("downloadLimitBps").and_then(|v| v.as_u64());
    let ul_limit = params.get("uploadLimitBps").and_then(|v| v.as_u64());
    let task = TaskId::from_legacy_string(task_id_str)
        .map_err(|e| JsonRpcError::invalid_params(format!("Invalid task ID: {e}")))?;
    let dispatcher = make_dispatcher(state);
    dispatcher
        .bt_set_speed_limit(&task, dl_limit, ul_limit)
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    Ok(serde_json::json!({}))
}

// ── Handler: bt.previewTorrent ─────────────────────────────────────

async fn handle_bt_preview_torrent(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let source = params
        .get("source")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing source"))?;
    let dispatcher = make_dispatcher(state);
    let entries = dispatcher
        .bt_preview_torrent(source)
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    serde_json::to_value(entries).map_err(|e| JsonRpcError::server_error(e.to_string()))
}

// ── Handler: bt.getPeers / getTrackers / getPieces / getFiles ──────

async fn handle_bt_get_details(
    method: &str,
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id_str = params
        .get("taskId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let task = TaskId::from_legacy_string(task_id_str)
        .map_err(|e| JsonRpcError::invalid_params(format!("Invalid task ID: {e}")))?;
    let dispatcher = make_dispatcher(state);
    let result = match method {
        "bt.getPeers" => {
            let peers = dispatcher
                .bt_get_peers(&task)
                .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(peers)
        }
        "bt.getTrackers" => {
            let trackers = dispatcher
                .bt_get_trackers(&task)
                .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(trackers)
        }
        "bt.getPieces" => {
            let pieces = dispatcher
                .bt_get_pieces(&task)
                .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(pieces)
        }
        "bt.getFiles" => {
            let files = dispatcher
                .bt_get_files(&task)
                .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
            serde_json::to_value(files)
        }
        _ => return Err(JsonRpcError::method_not_found(method)),
    }
    .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    Ok(result)
}

// ── Handler: bt.updateFiles ────────────────────────────────────────

async fn handle_bt_update_files(
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let task_id_str = params
        .get("taskId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing taskId"))?;
    let included_indices: Vec<usize> = params
        .get("includedIndices")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|i| i.as_u64().map(|u| u as usize))
                .collect()
        })
        .unwrap_or_default();
    let task = TaskId::from_legacy_string(task_id_str)
        .map_err(|e| JsonRpcError::invalid_params(format!("Invalid task ID: {e}")))?;
    let dispatcher = make_dispatcher(state);
    dispatcher
        .bt_update_files(&task, included_indices)
        .await
        .map_err(|e| JsonRpcError::server_error(e.to_string()))?;
    Ok(serde_json::json!({}))
}

// ── Handler: CDN commands ──────────────────────────────────────────

async fn handle_cdn_routes(
    method: &str,
    params: Option<&serde_json::Value>,
    state: &RpcState,
) -> Result<serde_json::Value, JsonRpcError> {
    match method {
        "cdn.fetchRanges" => {
            Ok(serde_json::json!(
                limedl_core::cdn::CLOUDFLARE_IPV4_RANGES.iter().map(|s| s.to_string()).collect::<Vec<_>>()
            ))
        }
        "cdn.status" => {
            let st = state.cdn_service.status().await;
            let status_str: String = match st {
                limedl_core::cdn::AccelState::Idle => "Idle".into(),
                limedl_core::cdn::AccelState::Testing => "Testing".into(),
                limedl_core::cdn::AccelState::Ready => "Ready".into(),
                limedl_core::cdn::AccelState::Error(msg) => format!("Error: {msg}"),
            };
            Ok(serde_json::json!(status_str))
        }
        "cdn.detail" => {
            let st = state.cdn_service.status().await;
            let ip = state.cdn_service.active_ip().await.map(|i| i.to_string());
            let speed = state.cdn_service.active_speed_mbps().await;
            let state_str = match &st {
                limedl_core::cdn::AccelState::Idle => "Idle".to_string(),
                limedl_core::cdn::AccelState::Testing => "Testing".to_string(),
                limedl_core::cdn::AccelState::Ready => "Ready".to_string(),
                limedl_core::cdn::AccelState::Error(msg) => format!("Error: {msg}"),
            };
            // ── phase ────────────────────────────────────────────
            let phase: Option<String> = state.cdn_service.phase().await.map(|p| match p {
                limedl_core::cdn::CdnTestPhase::FetchingRanges => "FetchingRanges".into(),
                limedl_core::cdn::CdnTestPhase::Screening => "Screening".into(),
                limedl_core::cdn::CdnTestPhase::MeasuringThroughput => "MeasuringThroughput".into(),
            });
            let (current, total) = state.cdn_service.phase_progress().await;
            let phase_progress: Option<serde_json::Value> = if total > 0 {
                Some(serde_json::json!({ "current": current, "total": total }))
            } else {
                None
            };
            let candidates = state.cdn_service.candidates().await;
            let default_node = state.cdn_service.default_node().await;
            Ok(serde_json::json!({
                "state": state_str,
                "activeIp": ip,
                "activeSpeedMbps": speed,
                "phase": phase,
                "phaseProgress": phase_progress,
                "candidates": candidates,
                "defaultNode": default_node,
            }))
        }
        "cdn.test" => {
            let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
                JsonRpcError::server_error("HTTP backend not found")
            })?;
            let settings = dm.settings().await.map_err(|e| {
                JsonRpcError::server_error(e.to_string())
            })?;
            state.cdn_service.start_test(settings).await.map_err(|e| {
                JsonRpcError::server_error(e.to_string())
            })?;
            let cdn = state.cdn_service.clone();
            let event_bus = state.event_bus.clone();
            let dm_for_monitor = state.registry.get_typed::<DownloadManager>()
                .ok_or_else(|| JsonRpcError::server_error("HTTP backend not found"))?
                .clone();
            tokio::spawn(async move {
                let outcome = cdn.monitor_test(event_bus).await;
                let now_ms = limedl_core::now_ms();
                if let Ok(mut current) = dm_for_monitor.settings().await {
                    use limedl_core::cdn::accelerator::AccelState;
                    match &outcome.state {
                        AccelState::Ready => {
                            current.cdn_acceleration.active_ip =
                                outcome.active_ip.map(|i| i.to_string());
                            current.cdn_acceleration.active_speed_mbps = outcome.active_speed_mbps;
                            current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                            current.cdn_acceleration.last_error = None;
                        }
                        AccelState::Error(msg) => {
                            current.cdn_acceleration.last_error = Some(msg.clone());
                            current.cdn_acceleration.last_test_at_ms = Some(now_ms);
                        }
                        _ => {}
                    }
                    let _ = dm_for_monitor.apply_settings(current).await;
                }
            });
            Ok(serde_json::json!(null))
        }
        "cdn.apply" => {
            let ip_str = params
                .and_then(|p| p.get("ip"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| JsonRpcError::invalid_params("Missing 'ip' parameter"))?;
            let speed = params
                .and_then(|p| p.get("speedMbps"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let ip: std::net::Ipv4Addr = ip_str.parse().map_err(|e| {
                JsonRpcError::invalid_params(format!("Invalid IP address: {e}"))
            })?;
            let dm = state.registry.get_typed::<DownloadManager>().ok_or_else(|| {
                JsonRpcError::server_error("HTTP backend not found")
            })?;
            let settings = dm.settings().await.map_err(|e| {
                JsonRpcError::server_error(e.to_string())
            })?;
            state.cdn_service.apply_ip(ip, speed, &settings).await.map_err(|e| {
                JsonRpcError::server_error(e.to_string())
            })?;
            // Persist
            if let Ok(mut current) = dm.settings().await {
                current.cdn_acceleration.active_ip = Some(ip_str.to_string());
                current.cdn_acceleration.active_speed_mbps = Some(speed);
                current.cdn_acceleration.last_test_at_ms = Some(limedl_core::now_ms());
                current.cdn_acceleration.last_error = None;
                let _ = dm.apply_settings(current).await;
            }
            Ok(serde_json::json!(null))
        }
        "cdn.clear" => {
            state.cdn_service.clear().await;
            Ok(serde_json::json!(null))
        }
        "cdn.cancel" => {
            state.cdn_service.cancel_test();
            Ok(serde_json::json!(null))
        }
        "cdn.candidates" => {
            let candidates = state.cdn_service.candidates().await;
            Ok(serde_json::to_value(candidates).unwrap_or_default())
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

        // Initialize CDN service (same as Tauri setup does)
        let cdn_accelerator = core.cdn_service.accelerator().clone();
        core.download_manager.set_cdn_accelerator(cdn_accelerator);
        core.cdn_service.init_from_settings(&core.settings).await;

        let rpc_state = Arc::new(RpcState {
            registry: core.registry,
            event_bus: core.event_bus,
            clients: Arc::new(parking_lot::Mutex::new(Vec::new())),
            rate_limiter: Arc::new(crate::rate_limiter::WsRateLimiter::new()),
            cdn_service: core.cdn_service,
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
            value.get("taskId").is_some() && value["taskId"]["id"].as_str().is_some(),
            "response missing taskId: {value:?}"
        );
        assert_eq!(
            value["taskId"]["kind"],
            "http",
            "expected HTTP task kind"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_url_too_long() {
        let (state, tmp) = make_rpc_state().await;
        let dest = tmp.path().join("output");

        // URL exceeds 8192 byte limit (prefix ~20 chars + 8200 padding)
        let long_url = format!("http://example.com/{}", "a".repeat(8200));
        assert!(long_url.len() > 8192, "test URL must exceed 8192 bytes");

        let params = serde_json::json!({
            "url": long_url,
            "destinationDir": dest.to_string_lossy(),
            "fileName": "test.bin"
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_err(), "expected error for overly long URL");
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(
            err.message.contains("8192"),
            "error message should mention 8192 byte limit: {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_magnet_too_long() {
        let (state, tmp) = make_rpc_state().await;
        let dest = tmp.path().join("output");

        // Magnet link exceeding 4096 byte limit (magnet:?xt=urn:btih:... ~20 chars + 4100 padding)
        let long_magnet = format!("magnet:?xt=urn:btih:{}", "a".repeat(4100));
        assert!(long_magnet.len() > 4096, "test magnet must exceed 4096 bytes");

        let params = serde_json::json!({
            "url": long_magnet,
            "destinationDir": dest.to_string_lossy(),
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_err(), "expected error for overly long magnet link");
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(
            err.message.contains("4096"),
            "error message should mention 4096 byte limit: {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_unsupported_scheme_ftp() {
        let (state, tmp) = make_rpc_state().await;
        let dest = tmp.path().join("output");

        let params = serde_json::json!({
            "url": "ftp://example.com/file.zip",
            "destinationDir": dest.to_string_lossy(),
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_err(), "expected error for ftp:// scheme");
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(
            err.message.contains("http") || err.message.contains("magnet"),
            "error message should mention supported schemes: {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_unsupported_scheme_file() {
        let (state, tmp) = make_rpc_state().await;
        let dest = tmp.path().join("output");

        let params = serde_json::json!({
            "url": "file:///etc/passwd",
            "destinationDir": dest.to_string_lossy(),
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_err(), "expected error for file:// scheme");
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_empty_url() {
        let (state, tmp) = make_rpc_state().await;
        let dest = tmp.path().join("output");

        let params = serde_json::json!({
            "url": "",
            "destinationDir": dest.to_string_lossy(),
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_err(), "expected error for empty URL");
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_start_missing_destination_dir() {
        let (state, _tmp) = make_rpc_state().await;

        let params = serde_json::json!({
            "url": "https://example.com/test.bin",
            "fileName": "test.bin"
        });

        let result = dispatch_method("download.start", Some(&params), &state).await;
        assert!(result.is_err(), "expected error for missing destinationDir");
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
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

    // ── Helper: start a download with startPaused=true and return the
    //    legacy taskId string ("http:<uuid>") for action method tests.

    async fn start_download_get_legacy_id(state: &Arc<RpcState>, tmp: &TempDir) -> String {
        let dest = tmp.path().join("output");
        std::fs::create_dir_all(&dest).unwrap();

        let params = serde_json::json!({
            "url": "https://example.com/test.bin",
            "destinationDir": dest.to_string_lossy(),
            "fileName": "test.bin",
            "startPaused": true,
        });

        let result = dispatch_method("download.start", Some(&params), state).await;
        let value = result.expect("download.start should succeed");
        let kind = value["taskId"]["kind"].as_str().expect("taskId.kind");
        let id = value["taskId"]["id"].as_str().expect("taskId.id");
        format!("{kind}:{id}")
    }

    // ── Download action focused tests ───────────────────────────────────

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_pause_succeeds() {
        let (state, tmp) = make_rpc_state().await;
        let task_id = start_download_get_legacy_id(&state, &tmp).await;
        let params = serde_json::json!({ "taskId": task_id });
        let result = dispatch_method("download.pause", Some(&params), &state).await;
        assert!(result.is_ok(), "download.pause failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_resume_succeeds() {
        let (state, tmp) = make_rpc_state().await;
        let task_id = start_download_get_legacy_id(&state, &tmp).await;
        // Pause first so resume has meaningful work
        let params = serde_json::json!({ "taskId": task_id });
        let _ = dispatch_method("download.pause", Some(&params), &state).await;
        let result = dispatch_method("download.resume", Some(&params), &state).await;
        assert!(result.is_ok(), "download.resume failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_cancel_succeeds() {
        let (state, tmp) = make_rpc_state().await;
        let task_id = start_download_get_legacy_id(&state, &tmp).await;
        let params = serde_json::json!({ "taskId": task_id });
        let result = dispatch_method("download.cancel", Some(&params), &state).await;
        assert!(result.is_ok(), "download.cancel failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_status_returns_state() {
        let (state, tmp) = make_rpc_state().await;
        let task_id = start_download_get_legacy_id(&state, &tmp).await;
        let params = serde_json::json!({ "taskId": task_id });
        let result = dispatch_method("download.status", Some(&params), &state).await;
        assert!(result.is_ok(), "download.status failed: {:?}", result.err());
        let value = result.unwrap();
        assert!(value.get("id").is_some(), "status missing 'id'");
        assert!(value.get("state").is_some(), "status missing 'state'");
        assert!(value.get("url").is_some(), "status missing 'url'");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_purge_succeeds() {
        let (state, tmp) = make_rpc_state().await;
        let task_id = start_download_get_legacy_id(&state, &tmp).await;
        let params = serde_json::json!({ "taskId": task_id });

        let result = dispatch_method("download.purge", Some(&params), &state).await;
        assert!(result.is_ok(), "download.purge failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_remove_succeeds() {
        let (state, tmp) = make_rpc_state().await;
        let task_id = start_download_get_legacy_id(&state, &tmp).await;
        let params = serde_json::json!({ "taskId": task_id });

        let result = dispatch_method("download.remove", Some(&params), &state).await;
        assert!(result.is_ok(), "download.remove failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_action_missing_params() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("download.pause", None, &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_action_missing_task_id() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("download.pause", Some(&serde_json::json!({})), &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_action_invalid_task_id() {
        let (state, _tmp) = make_rpc_state().await;
        let params = serde_json::json!({ "taskId": "not_a_valid_task_id" });
        let result = dispatch_method("download.pause", Some(&params), &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    // ── BT method tests ────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_runtime_status_succeeds() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("bt.runtimeStatus", None, &state).await;
        // Should succeed because BT backend is registered during bootstrap
        assert!(result.is_ok(), "bt.runtimeStatus failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_get_peers_missing_params() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("bt.getPeers", None, &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_get_trackers_missing_task_id() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method(
            "bt.getTrackers",
            Some(&serde_json::json!({})),
            &state,
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_get_pieces_invalid_task_id() {
        let (state, _tmp) = make_rpc_state().await;
        let params = serde_json::json!({ "taskId": "invalid" });
        let result = dispatch_method("bt.getPieces", Some(&params), &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_get_files_invalid_task_id() {
        let (state, _tmp) = make_rpc_state().await;
        let params = serde_json::json!({ "taskId": "invalid" });
        let result = dispatch_method("bt.getFiles", Some(&params), &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_set_speed_limit_missing_params() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("bt.setSpeedLimit", None, &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_preview_torrent_missing_source() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method(
            "bt.previewTorrent",
            Some(&serde_json::json!({})),
            &state,
        )
        .await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_update_files_missing_params() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("bt.updateFiles", None, &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn bt_update_files_invalid_task_id() {
        let (state, _tmp) = make_rpc_state().await;
        let params = serde_json::json!({ "taskId": "bad!id!" });
        let result = dispatch_method("bt.updateFiles", Some(&params), &state).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32602);
    }

    // ── CDN method tests ──────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_status_returns_idle() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("cdn.status", None, &state).await;
        assert!(result.is_ok(), "cdn.status failed: {:?}", result.err());
        let value = result.unwrap();
        // Initially no test has been run, so CDN should be Idle
        assert_eq!(value.as_str(), Some("Idle"));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_detail_returns() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("cdn.detail", None, &state).await;
        assert!(result.is_ok(), "cdn.detail failed: {:?}", result.err());
        let value = result.unwrap();
        assert!(value.get("state").is_some(), "cdn.detail missing 'state'");
        assert!(value.get("candidates").is_some(), "cdn.detail missing 'candidates'");
        assert_eq!(value["state"], "Idle");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_cancel_succeeds() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("cdn.cancel", None, &state).await;
        assert!(result.is_ok(), "cdn.cancel failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_fetch_ranges_succeeds() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("cdn.fetchRanges", None, &state).await;
        assert!(result.is_ok(), "cdn.fetchRanges failed: {:?}", result.err());
        let value = result.unwrap();
        assert!(
            value.as_array().is_some(),
            "cdn.fetchRanges should return an array"
        );
        // Cloudflare IPv4 ranges should be non-empty
        assert!(
            !value.as_array().unwrap().is_empty(),
            "cdn.fetchRanges should return at least one range"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_clear_succeeds() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("cdn.clear", None, &state).await;
        assert!(result.is_ok(), "cdn.clear failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_test_succeeds() {
        let (state, _tmp) = make_rpc_state().await;
        // cdn.test starts a speed test (spawns async task) — should return null
        let result = dispatch_method("cdn.test", None, &state).await;
        assert!(result.is_ok(), "cdn.test failed: {:?}", result.err());
        assert_eq!(result.unwrap(), serde_json::json!(null));
        // Clean up the test we just started
        let _ = dispatch_method("cdn.cancel", None, &state).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_apply_valid_ip() {
        let (state, _tmp) = make_rpc_state().await;
        let params = serde_json::json!({
            "ip": "1.1.1.1",
            "speedMbps": 100.0
        });
        let result = dispatch_method("cdn.apply", Some(&params), &state).await;
        assert!(result.is_ok(), "cdn.apply with valid IP failed: {:?}", result.err());
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_apply_invalid_ip() {
        let (state, _tmp) = make_rpc_state().await;
        let params = serde_json::json!({
            "ip": "not-an-ip"
        });
        let result = dispatch_method("cdn.apply", Some(&params), &state).await;
        assert!(result.is_err(), "cdn.apply should fail with invalid IP");
        let err = result.unwrap_err();
        assert_eq!(err.code, -32602);
        assert!(
            err.message.contains("Invalid IP address"),
            "error should mention IP: {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn cdn_candidates_returns_array() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("cdn.candidates", None, &state).await;
        assert!(result.is_ok(), "cdn.candidates failed: {:?}", result.err());
        let value = result.unwrap();
        assert!(value.is_array(), "cdn.candidates should return an array");
        // Initially no test has been run, so candidates should be empty
        assert_eq!(
            value.as_array().unwrap().len(),
            0,
            "candidates should be empty before any test"
        );
    }

    // ── extract_method_name ────────────────────────────────────────────

    #[test]
    fn extract_method_name_valid() {
        let text = r#"{"jsonrpc":"2.0","id":1,"method":"cdn.status","params":[]}"#;
        assert_eq!(extract_method_name(text), Some("cdn.status"));
    }

    #[test]
    fn extract_method_name_with_whitespace() {
        let text = r#"{"jsonrpc":"2.0","id":1,"method":  "cdn.status"  }"#;
        assert_eq!(extract_method_name(text), Some("cdn.status"));
    }

    #[test]
    fn extract_method_name_missing_field() {
        let text = r#"{"jsonrpc":"2.0","id":1}"#;
        assert_eq!(extract_method_name(text), None);
    }

    #[test]
    fn extract_method_name_non_string_method() {
        // method is a number (e.g. malformed JSON-RPC)
        let text = r#"{"jsonrpc":"2.0","id":1,"method":42}"#;
        assert_eq!(extract_method_name(text), None);
    }

    #[test]
    fn extract_method_name_empty_string() {
        let text = r#"{"jsonrpc":"2.0","id":1,"method":""}"#;
        assert_eq!(extract_method_name(text), Some(""));
    }

    #[test]
    fn extract_method_name_null_method() {
        let text = r#"{"jsonrpc":"2.0","id":1,"method":null}"#;
        assert_eq!(extract_method_name(text), None);
    }

    // ── Settings / UI method tests ────────────────────────────────────

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn settings_toggle_game_mode() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method(
            "settings.toggleGameMode",
            Some(&serde_json::json!({ "enabled": true })),
            &state,
        )
        .await;
        assert!(result.is_ok(), "settings.toggleGameMode failed: {:?}", result.err());
        assert_eq!(result.unwrap(), serde_json::json!(true));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn settings_get_io_status_returns() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("settings.getIoStatus", None, &state).await;
        assert!(result.is_ok(), "settings.getIoStatus failed: {:?}", result.err());
        let value = result.unwrap();
        assert!(value.get("gameMode").is_some(), "missing gameMode");
        assert!(value.get("bufferUsageBytes").is_some(), "missing bufferUsageBytes");
        assert!(value.get("activeSlots").is_some(), "missing activeSlots");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn settings_toggle_overclock_mode() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method(
            "settings.toggleOverclockMode",
            Some(&serde_json::json!({ "enabled": true })),
            &state,
        )
        .await;
        assert!(result.is_ok(), "settings.toggleOverclockMode failed: {:?}", result.err());
        assert_eq!(result.unwrap(), serde_json::json!(true));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn settings_get_overclock_mode_returns() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("settings.getOverclockMode", None, &state).await;
        assert!(result.is_ok(), "settings.getOverclockMode failed: {:?}", result.err());
        assert!(result.unwrap().is_boolean());
    }

    // ── Other handler edge cases and unknown methods ─────────────────

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn download_open_in_explorer_missing_params() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("download.openInExplorer", None, &state).await;
        assert!(result.is_err(), "expected error for missing params");
        assert_eq!(result.unwrap_err().code, -32602);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn method_not_found_returns_error() {
        let (state, _tmp) = make_rpc_state().await;
        let result = dispatch_method("nonexistent.method", None, &state).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("nonexistent.method"));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[timeout(30_000)]
    async fn unknown_download_method_returns_error() {
        let (state, _tmp) = make_rpc_state().await;
        // A plausible-looking but non-existent download sub-command
        let result = dispatch_method("download.unknownMethod", None, &state).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("download.unknownMethod"));
    }
}
