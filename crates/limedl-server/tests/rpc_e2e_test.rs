//! WebSocket E2E tests for the JSON-RPC server.
//!
//! These tests start an actual axum server with the WebSocket handler,
//! connect via WebSocket using `tokio-tungstenite`, send JSON-RPC 2.0
//! requests over the wire, and verify responses — the same path a real
//! NAS WebUI frontend would take.
//!
//! # Protocol notes
//!
//! - The JSON-RPC `method` field uses the `rpc_method` value from
//!   `ws_manifest.rs` (dot-separated, e.g. `"download.list"`).
//! - The `params` field is passed directly as the handler argument
//!   (no `UnwrapField` / `Rename` transforms — those are frontend-only).
//! - Each test starts a fresh server on `127.0.0.1:0` (random port).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Router, routing::get};
use futures_util::{SinkExt, StreamExt};
use ntest::timeout;
use tokio::net::TcpListener;

use limedl_core::test_harness::TestServer;

/// A running test server with an active WebSocket JSON-RPC endpoint.
///
/// Drops clean up the server and temp directories on drop.
struct RpcE2eServer {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    _tmp: tempfile::TempDir,
}

impl RpcE2eServer {
    fn ws_url(&self) -> String {
        format!("ws://{}/ws", self.addr)
    }

    /// Path to the downloads state directory (valid for lifetime of server).
    #[allow(dead_code)]
    fn state_dir(&self) -> std::path::PathBuf {
        self._tmp.path().join("downloads")
    }
}

impl Drop for RpcE2eServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Bootstrap core subsystems and start an axum server with the WebSocket
/// RPC handler on a random localhost port.
async fn start_server() -> RpcE2eServer {
    let tmp = tempfile::TempDir::new().expect("create temp dir");
    let state_dir = tmp.path().join("downloads");
    std::fs::create_dir_all(&state_dir).expect("create state dir");

    let core = limedl_core::bootstrap::bootstrap(state_dir)
        .await
        .expect("core bootstrap");

    // CDN service setup (same as main.rs / Tauri setup)
    let cdn_accelerator = core.cdn_service.accelerator().clone();
    core.download_manager
        .set_cdn_accelerator(cdn_accelerator);
    core.cdn_service
        .init_from_settings(&core.settings)
        .await;

    let rpc_state = Arc::new(limedl_server::rpc::RpcState {
        registry: core.registry,
        event_bus: core.event_bus,
        dispatcher: core.dispatcher,
        clients: Arc::new(parking_lot::Mutex::new(Vec::new())),
        rate_limiter: Arc::new(limedl_server::rate_limiter::WsRateLimiter::new()),
        cdn_service: core.cdn_service,
        http_client: reqwest::Client::new(),
    });

    // Build minimal router with just the WS endpoint (no auth, no static files)
    let app = Router::new().route(
        "/ws",
        get(move |ws: axum::extract::WebSocketUpgrade| {
            let state = rpc_state.clone();
            async move { limedl_server::rpc::ws_handler(ws, state).await }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind random port");
    let addr = listener.local_addr().expect("get bound addr");

    let handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("axum serve");
    });

    RpcE2eServer {
        addr,
        handle,
        _tmp: tmp,
    }
}

// ── Helpers for sending JSON-RPC messages over WebSocket ────────────────

/// Open a WebSocket connection.
type WsStream = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

async fn connect(ws_url: &str) -> WsStream {
    let (stream, _) = tokio_tungstenite::connect_async(ws_url)
        .await
        .expect("WebSocket connect");
    stream
}

/// Send a JSON-RPC request and return the full response as a `serde_json::Value`.
async fn rpc_call(
    stream: &mut WsStream,
    method: &str,
    params: Option<serde_json::Value>,
    id: serde_json::Value,
) -> serde_json::Value {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });

    let msg = serde_json::to_string(&request).expect("serialize request");
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(msg.into()))
        .await
        .expect("send message");

    recv_response(stream).await
}

/// Receive one WebSocket text message and parse as JSON.
async fn recv_response(stream: &mut WsStream) -> serde_json::Value {
    loop {
        match stream.next().await {
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                return serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("parse response JSON: {e}\nraw: {text}"));
            }
            Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(_))) => {
                // tokio-tungstenite handles pong automatically — skip
                continue;
            }
            Some(Ok(tokio_tungstenite::tungstenite::Message::Pong(_))) => continue,
            Some(Ok(other)) => {
                panic!("unexpected WS message type: {other:?}");
            }
            Some(Err(e)) => panic!("WS recv error: {e}"),
            None => panic!("WS stream closed"),
        }
    }
}

/// Convenience: single-shot RPC — open connection, call, close, return.
async fn rpc_call_one_shot(
    ws_url: &str,
    method: &str,
    params: Option<serde_json::Value>,
    id: serde_json::Value,
) -> serde_json::Value {
    let mut stream = connect(ws_url).await;
    let result = rpc_call(&mut stream, method, params, id).await;
    let _ = stream.close(None).await;
    result
}

/// Assert the response is a successful JSON-RPC result and return the
/// `result` field.
fn assert_rpc_ok(response: &serde_json::Value, id: &serde_json::Value) -> serde_json::Value {
    assert_eq!(
        response["jsonrpc"],
        "2.0",
        "response jsonrpc field"
    );
    assert_eq!(
        response["id"], *id,
        "response id should match request id"
    );
    assert!(
        response.get("error").is_none(),
        "unexpected error in response: {:?}",
        response.get("error")
    );
    assert!(
        response.get("result").is_some(),
        "response missing 'result': {response:?}"
    );
    response["result"].clone()
}

/// Assert the response is a JSON-RPC error with the given code.
fn assert_rpc_error(
    response: &serde_json::Value,
    id: &serde_json::Value,
    expected_code: i32,
) {
    assert_eq!(
        response["jsonrpc"], "2.0",
        "response jsonrpc field"
    );
    assert_eq!(
        response["id"], *id,
        "response id should match request id"
    );
    assert!(
        response.get("result").is_none(),
        "error response should not have 'result': {response:?}"
    );
    let err = response
        .get("error")
        .expect("error response missing 'error'");
    assert_eq!(
        err["code"].as_i64(),
        Some(expected_code as i64),
        "error code mismatch: {response:?}"
    );
    assert!(
        err["message"].as_str().is_some(),
        "error message should be a string: {response:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn connect_and_list_downloads() {
    let server = start_server().await;

    let response = rpc_call_one_shot(
        &server.ws_url(),
        "download.list",
        None,
        serde_json::json!(1),
    )
    .await;

    let result = assert_rpc_ok(&response, &serde_json::json!(1));
    assert!(result.is_array(), "download.list should return an array");
    assert!(
        result.as_array().unwrap().is_empty(),
        "initial download list should be empty"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn download_start_and_status() {
    let server = start_server().await;
    let test_server = TestServer::new(1024 * 1024).await; // 1 MB, bandwidth-limited below
    let ws_url = server.ws_url();
    let dest = server.state_dir();

    // ── 1. Start a bandwidth-limited download ─────────────────────────
    let params = serde_json::json!({
        "url": test_server.file_url_bandwidth(10 * 1024),
        "destinationDir": dest.to_string_lossy(),
        "fileName": "test.bin",
    });

    let response = rpc_call_one_shot(&ws_url, "download.start", Some(params), serde_json::json!(1)).await;
    let result = assert_rpc_ok(&response, &serde_json::json!(1));

    assert!(
        result.get("taskId").is_some(),
        "download.start response missing taskId: {result:?}"
    );
    let task_id_val = result["taskId"]["id"]
        .as_str()
        .expect("taskId.id string")
        .to_string();
    let task_kind = result["taskId"]["kind"]
        .as_str()
        .expect("taskId.kind string");
    assert_eq!(task_kind, "http", "expected HTTP task kind");

    let legacy_id = format!("{task_kind}:{task_id_val}");

    // ── 2. Query status ────────────────────────────────────────────────
    let status_params = serde_json::json!({ "taskId": legacy_id });
    let response2 = rpc_call_one_shot(&ws_url, "download.status", Some(status_params), serde_json::json!(2)).await;
    let status = assert_rpc_ok(&response2, &serde_json::json!(2));

    assert!(
        status.get("id").is_some(),
        "status response missing 'id': {status:?}"
    );
    assert!(
        status.get("state").is_some(),
        "status response missing 'state': {status:?}"
    );
    assert_eq!(status["url"], test_server.file_url_bandwidth(10 * 1024));

    // ── 3. Cancel to prevent background task leak ──────────────────────
    let cancel_params = serde_json::json!({ "taskId": legacy_id });
    let _ = rpc_call_one_shot(&ws_url, "download.cancel", Some(cancel_params), serde_json::json!(3)).await;
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn settings_get_and_save() {
    let server = start_server().await;
    let ws_url = server.ws_url();

    // ── 1. Get initial settings ────────────────────────────────────────
    let response = rpc_call_one_shot(&ws_url, "settings.get", None, serde_json::json!(1)).await;
    let settings = assert_rpc_ok(&response, &serde_json::json!(1));

    assert!(
        settings.get("download").is_some(),
        "settings.get response should have 'download' section"
    );

    // ── 2. Save them back unchanged ────────────────────────────────────
    let response2 = rpc_call_one_shot(
        &ws_url,
        "settings.save",
        Some(settings.clone()),
        serde_json::json!(2),
    )
    .await;
    let saved = assert_rpc_ok(&response2, &serde_json::json!(2));

    assert!(
        saved.get("download").is_some(),
        "settings.save response should have 'download' section"
    );
    assert_eq!(
        settings
            .get("download")
            .and_then(|v| v.as_object())
            .map(|o| o.len()),
        saved
            .get("download")
            .and_then(|v| v.as_object())
            .map(|o| o.len()),
        "settings.save should preserve download config structure"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn invalid_method_returns_error() {
    let server = start_server().await;

    let response = rpc_call_one_shot(
        &server.ws_url(),
        "nonexistent.method",
        None,
        serde_json::json!(1),
    )
    .await;

    assert_rpc_error(&response, &serde_json::json!(1), -32601);

    let msg = response["error"]["message"]
        .as_str()
        .expect("error message string");
    assert!(
        msg.contains("nonexistent.method"),
        "error message should mention the method name: {msg}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn invalid_params_returns_error() {
    let server = start_server().await;

    // download.start without a URL (url is required by StartDownloadRequest)
    let params = serde_json::json!({
        "destinationDir": "/tmp/test",
        "startPaused": true,
    });

    let response = rpc_call_one_shot(
        &server.ws_url(),
        "download.start",
        Some(params),
        serde_json::json!(1),
    )
    .await;

    assert_rpc_error(&response, &serde_json::json!(1), -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("Invalid params")
            || response["error"]["message"]
                .as_str()
                .unwrap_or("")
                .contains("missing field"),
        "error message should mention invalid params or missing field: {:?}",
        response["error"]["message"]
    );
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn invalid_json_returns_parse_error() {
    let server = start_server().await;
    let mut stream = connect(&server.ws_url()).await;

    // Send text that is not valid JSON
    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "this is not valid json".into(),
        ))
        .await
        .expect("send invalid json");

    let response = recv_response(&mut stream).await;

    // Parse error response has id = null because the JSON couldn't be parsed
    assert_rpc_error(&response, &serde_json::Value::Null, -32700);

    let msg = response["error"]["message"]
        .as_str()
        .expect("error message string");
    assert!(
        msg.contains("Parse error"),
        "error message should mention parse error: {msg}"
    );

    // Connection should still be alive after invalid message
    // (send a valid request to verify)
    let valid_response = rpc_call(
        &mut stream,
        "download.list",
        None,
        serde_json::json!(2),
    )
    .await;
    let result = assert_rpc_ok(&valid_response, &serde_json::json!(2));
    assert!(result.is_array(), "subsequent valid request should work");
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn missing_id_returns_parse_error() {
    let server = start_server().await;
    let mut stream = connect(&server.ws_url()).await;

    // Send a JSON-RPC-like request without the 'id' field.
    // The current JsonRpcRequest struct requires 'id', so this produces a
    // parse error rather than being treated as a JSON-RPC notification.
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "download.list",
        "params": null,
    });

    stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&request).unwrap().into(),
        ))
        .await
        .expect("send notification");

    let response = recv_response(&mut stream).await;

    assert_rpc_error(&response, &serde_json::Value::Null, -32700);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("missing field"),
        "error should mention missing field `id`: {:?}",
        response["error"]["message"]
    );
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn multiple_requests_over_single_connection() {
    let server = start_server().await;
    let mut stream = connect(&server.ws_url()).await;

    // Send 3 requests in quick succession (pipelining)
    let methods = ["download.list", "settings.get", "cdn.status"];
    for (idx, method) in methods.iter().enumerate() {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": (idx + 1),
            "method": method,
            "params": null,
        });
        stream
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::to_string(&request).unwrap().into(),
            ))
            .await
            .expect("send request");
    }

    // Read all 3 responses
    let mut seen_ids = std::collections::HashSet::new();
    for _ in 0..3 {
        let response = recv_response(&mut stream).await;

        assert_eq!(response["jsonrpc"], "2.0");
        let id = response["id"].as_i64().expect("response must have numeric id");
        assert!(
            seen_ids.insert(id),
            "duplicate response id: {id}"
        );
        assert!(
            response.get("result").is_some(),
            "response {id} missing 'result': {response:?}"
        );
        assert!(
            response.get("error").is_none(),
            "response {id} has unexpected error: {response:?}"
        );
    }

    // Verify all 3 method IDs were received
    assert_eq!(
        seen_ids.len(),
        3,
        "expected 3 unique responses, got {count}",
        count = seen_ids.len()
    );
    for i in 1..=3 {
        assert!(
            seen_ids.contains(&i),
            "missing response for request id={i}"
        );
    }

    // Verify no extra response
    let extra = tokio::time::timeout(std::time::Duration::from_millis(300), stream.next()).await;
    match extra {
        Err(_elapsed) => { /* expected — timeout with no extra message */ }
        Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(_)))) |
        Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Pong(_)))) => { /* ok */ }
        other => panic!("unexpected extra message after 3 responses: {other:?}"),
    }
}
