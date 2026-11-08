//! HTTP and WebSocket transports for the JSON-RPC endpoint.

use futures_util::{SinkExt, StreamExt};

use super::context::RpcContext;
use super::dispatch::dispatch_method;
use super::jsonrpc::{
    ERR_INVALID_REQUEST, ERR_PARSE, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    error_response, success_response,
};
use crate::event_bus::DownloadEvent;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::{Message, WebSocket};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Maximum top-level JSON-RPC batch size accepted per request.
const MAX_BATCH_SIZE: usize = 256;
/// Maximum WebSocket frame/message size (2 MiB, matching Axum's HTTP body cap).
const MAX_WS_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
/// Bounded outbound queue per WebSocket client.
const WS_OUTBOX_CAPACITY: usize = 128;
/// Per-frame send timeout for slow/stalled WebSocket clients.
const WS_SEND_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handle_jsonrpc_http(
    axum::extract::State(ctx): axum::extract::State<Arc<RpcContext>>,
    body: String,
) -> Response {
    match process_jsonrpc_payload(&ctx, &body).await {
        Some(response) => (StatusCode::OK, response).into_response(),
        // An all-notification batch produces no response body (JSON-RPC 2.0).
        None => (StatusCode::OK, String::new()).into_response(),
    }
}

/// Unauthenticated liveness probe for container / systemd `HEALTHCHECK`.
pub async fn handle_healthz() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        r#"{"status":"ok"}"#,
    )
}

pub async fn handle_websocket_upgrade(
    ws: WebSocketUpgrade,
    axum::extract::State(ctx): axum::extract::State<Arc<RpcContext>>,
) -> impl IntoResponse {
    ws.max_message_size(MAX_WS_MESSAGE_BYTES)
        .max_frame_size(MAX_WS_MESSAGE_BYTES)
        .on_upgrade(move |socket| websocket_loop(socket, ctx))
}

pub async fn websocket_loop(socket: WebSocket, ctx: Arc<RpcContext>) {
    let mut shutdown_rx = ctx.shutdown_tx.subscribe();
    if *shutdown_rx.borrow() {
        return;
    }

    let (mut sender, mut receiver) = socket.split();
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<Message>(WS_OUTBOX_CAPACITY);
    let mut event_rx = ctx.event_bus.subscribe_lifecycle();

    let mut writer_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            match tokio::time::timeout(WS_SEND_TIMEOUT, sender.send(msg)).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) | Err(_) => break,
            }
        }
        let _ = tokio::time::timeout(Duration::from_secs(1), sender.close()).await;
    });

    let out_tx_events = out_tx.clone();
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
                    match out_tx_events.try_send(Message::Text(notification.into())) {
                        Ok(()) => {}
                        // Slow consumer: drop transient notification rather than blocking
                        // or starving RPC responses.
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => continue,
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                Ok(_) => {} // ignore non-Aria2 events
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let out_tx_reqs = out_tx;
    let ctx2 = ctx.clone();
    let mut recv_requests = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    let Some(resp) = process_jsonrpc_payload(&ctx2, &text).await else {
                        // Nothing to send for an all-notification batch.
                        continue;
                    };
                    if out_tx_reqs.send(Message::Text(resp.into())).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = &mut writer_task => {
            send_events.abort();
            recv_requests.abort();
        }
        _ = &mut send_events => {
            recv_requests.abort();
            writer_task.abort();
        }
        _ = &mut recv_requests => {
            send_events.abort();
            let _ = tokio::time::timeout(Duration::from_secs(2), &mut writer_task).await;
            writer_task.abort();
        }
        _ = shutdown_rx.changed() => {
            send_events.abort();
            recv_requests.abort();
            writer_task.abort();
        }
    }
}

/// Process one HTTP or WebSocket body.
///
/// A single request object yields a single response object; a top-level array
/// is a JSON-RPC 2.0 batch and yields an array of responses.
///
/// Returns `None` only when the body was a batch containing **only**
/// notifications: the spec forbids answering those, so the caller must send
/// nothing (aria2 accepts batch requests the same way).
pub async fn process_jsonrpc_payload(ctx: &RpcContext, body: &str) -> Option<String> {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => {
            return Some(serialize_response(&error_response(
                None,
                ERR_PARSE,
                "Parse error",
            )));
        }
    };

    match value {
        Value::Array(items) => {
            if items.is_empty() {
                // Per the spec an empty batch is a single Invalid Request error.
                return Some(serialize_response(&error_response(
                    None,
                    ERR_INVALID_REQUEST,
                    "Invalid Request",
                )));
            }
            if items.len() > MAX_BATCH_SIZE {
                return Some(serialize_response(&error_response(
                    None,
                    ERR_INVALID_REQUEST,
                    format!("Batch size exceeds maximum of {MAX_BATCH_SIZE}"),
                )));
            }

            let mut responses = Vec::with_capacity(items.len());
            for item in items {
                // A notification is a batch element without an `id`. It still
                // executes (side effects such as `aria2.pauseAll`), but produces
                // no response entry.
                let is_notification = item.as_object().is_some_and(|o| !o.contains_key("id"));
                if let Some(response) = process_single(ctx, item).await
                    && !is_notification
                {
                    responses.push(response);
                }
            }

            if responses.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&responses).unwrap_or_default())
            }
        }
        value => process_single(ctx, value)
            .await
            .map(|response| serialize_response(&response)),
    }
}

/// Single-message test helper: the response body for `body`, or an empty string
/// for an all-notification batch. The live transports use
/// [`process_jsonrpc_payload`] so they can skip sending nothing.
#[cfg(test)]
pub async fn process_jsonrpc_message(ctx: &RpcContext, body: &str) -> String {
    process_jsonrpc_payload(ctx, body).await.unwrap_or_default()
}

/// Dispatch one request value. Returns `None` only when the value is not a
/// request at all (the caller then treats it as an invalid element).
async fn process_single(ctx: &RpcContext, value: Value) -> Option<JsonRpcResponse> {
    let req: JsonRpcRequest = match serde_json::from_value(value) {
        Ok(req) => req,
        // A body without a `method` (or with a wrong-typed field) has always
        // answered `-32700` here; keep that contract.
        Err(_) => return Some(error_response(None, ERR_PARSE, "Parse error")),
    };

    if req.jsonrpc != "2.0" {
        return Some(error_response(
            req.id,
            ERR_INVALID_REQUEST,
            "Invalid JSON-RPC version",
        ));
    }

    let params = req.params.unwrap_or_default();
    Some(match dispatch_method(ctx, &req.method, params).await {
        Ok(result) => success_response(req.id, result),
        Err(err) => error_response(req.id, err.code, err.message),
    })
}

fn serialize_response(response: &JsonRpcResponse) -> String {
    serde_json::to_string(response).unwrap_or_default()
}
