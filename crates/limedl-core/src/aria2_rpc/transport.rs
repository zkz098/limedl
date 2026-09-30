//! HTTP and WebSocket transports for the JSON-RPC endpoint.

use super::*;

pub(crate) async fn handle_jsonrpc_http(
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

pub(crate) async fn process_jsonrpc_message(ctx: &RpcContext, body: &str) -> String {
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
