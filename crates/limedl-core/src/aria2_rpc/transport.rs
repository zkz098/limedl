//! HTTP and WebSocket transports for the JSON-RPC endpoint.

use futures_util::{SinkExt, StreamExt};

use super::{Arc, DownloadEvent, ERR_INVALID_REQUEST, ERR_PARSE, IntoResponse, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, Message, Response, RpcContext, StatusCode, Value, WebSocket, WebSocketUpgrade, dispatch_method, error_response, success_response};

pub(crate) async fn handle_jsonrpc_http(
    axum::extract::State(ctx): axum::extract::State<Arc<RpcContext>>,
    body: String,
) -> Response {
    match process_jsonrpc_payload(&ctx, &body).await {
        Some(response) => (StatusCode::OK, response).into_response(),
        // An all-notification batch produces no response body (JSON-RPC 2.0).
        None => (StatusCode::OK, String::new()).into_response(),
    }
}

pub(crate) async fn handle_websocket_upgrade(
    ws: WebSocketUpgrade,
    axum::extract::State(ctx): axum::extract::State<Arc<RpcContext>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket_loop(socket, ctx))
}

pub(crate) async fn websocket_loop(socket: WebSocket, ctx: Arc<RpcContext>) {
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
                    let Some(resp) = process_jsonrpc_payload(&ctx2, &text).await else {
                        // Nothing to send for an all-notification batch.
                        continue;
                    };
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

/// Process one HTTP or WebSocket body.
///
/// A single request object yields a single response object; a top-level array
/// is a JSON-RPC 2.0 batch and yields an array of responses.
///
/// Returns `None` only when the body was a batch containing **only**
/// notifications: the spec forbids answering those, so the caller must send
/// nothing (aria2 accepts batch requests the same way).
pub(crate) async fn process_jsonrpc_payload(ctx: &RpcContext, body: &str) -> Option<String> {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(_) => return Some(serialize_response(&error_response(None, ERR_PARSE, "Parse error"))),
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
pub(crate) async fn process_jsonrpc_message(ctx: &RpcContext, body: &str) -> String {
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
