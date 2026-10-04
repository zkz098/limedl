use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use ntest::timeout;
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::aria2_rpc::Aria2RpcServer;
use crate::types::Aria2RpcSettings;

/// Start an Aria2RpcServer on a random port, return the WebSocket URL
/// and shutdown handle.
async fn start_ws_server() -> (String, tokio::sync::watch::Sender<bool>, TempDir) {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();

    // Reserve a random port
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let settings = Aria2RpcSettings {
        enabled: true,
        port,
        secret: None,
        cors_allowed_origins: vec![],
        ..Aria2RpcSettings::default()
    };
    let rpc = Aria2RpcServer::new(core.registry.clone(), &settings, core.event_bus.clone());

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let _ = rpc.serve(shutdown_rx).await;
    });

    // Poll until the server is ready to accept connections.
    // On slow CI runners, a fixed 200ms sleep may not be sufficient.
    loop {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let ws_url = format!("ws://127.0.0.1:{port}/jsonrpc");
    (ws_url, shutdown_tx, tmp)
}

/// A connected JSON-RPC WebSocket plus the notifications read while looking
/// for responses.
///
/// Responses and notifications share one socket and are written by two
/// independent tasks, so a notification can arrive before the response that
/// triggered it (or after the *next* request was sent). Everything read while
/// waiting for a response is buffered, so a later `wait_notification` still
/// sees it.
struct WsClient {
    ws: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    next_id: u64,
    pending: Vec<Value>,
}

impl WsClient {
    async fn connect(ws_url: &str) -> Self {
        let (ws, _response) = connect_async(ws_url).await.expect("connect websocket");
        Self {
            ws,
            next_id: 1,
            pending: Vec::new(),
        }
    }

    /// Send a JSON-RPC request and return the response with the matching id.
    async fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.ws
            .send(Message::Text(request.to_string().into()))
            .await
            .expect("send JSON-RPC request");

        loop {
            let frame = self.next_json().await;
            if frame.get("id").and_then(Value::as_u64) == Some(id) {
                return frame;
            }
            assert!(
                frame.get("method").is_some(),
                "unexpected frame before the response: {frame}"
            );
            self.pending.push(frame);
        }
    }

    /// Read frames until the `method` notification for `gid` arrives.
    async fn wait_notification(&mut self, method: &str, gid: &str) {
        if let Some(index) = self
            .pending
            .iter()
            .position(|frame| is_notification(frame, method, gid))
        {
            self.pending.remove(index);
            return;
        }

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            assert!(
                Instant::now() < deadline,
                "no {method} notification for {gid}; buffered: {:?}",
                self.pending
            );
            let frame = self.next_json().await;
            if is_notification(&frame, method, gid) {
                return;
            }
            self.pending.push(frame);
        }
    }

    /// Assert that no further JSON-RPC notification arrives within `window`.
    async fn assert_quiet(&mut self, window: Duration) {
        let mut frames = std::mem::take(&mut self.pending);
        let deadline = Instant::now() + window;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match tokio::time::timeout(remaining, self.ws.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    frames.push(serde_json::from_str(&text).expect("valid JSON frame"));
                }
                Ok(Some(Ok(_))) => {} // ignore ping/pong/binary
                Ok(Some(Err(error))) => panic!("websocket error: {error}"),
                Ok(None) => break,
                Err(_) => break, // the quiet window elapsed
            }
        }
        let notifications: Vec<&Value> = frames
            .iter()
            .filter(|frame| frame.get("method").is_some())
            .collect();
        assert!(
            notifications.is_empty(),
            "unexpected notifications: {notifications:?}"
        );
    }

    async fn next_json(&mut self) -> Value {
        loop {
            match tokio::time::timeout(Duration::from_secs(10), self.ws.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    return serde_json::from_str(&text).expect("valid JSON-RPC frame");
                }
                Ok(Some(Ok(Message::Close(_))) | None) => panic!("websocket closed early"),
                Ok(Some(Ok(_))) => {} // ignore ping/pong/binary
                Ok(Some(Err(error))) => panic!("websocket error: {error}"),
                Err(_) => panic!("timed out waiting for a websocket frame"),
            }
        }
    }
}

fn is_notification(frame: &Value, method: &str, gid: &str) -> bool {
    frame["method"] == method && frame["params"][0]["gid"] == gid
}

/// Connect via WebSocket, send addUri, verify response + server-pushed event.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn websocket_add_uri_and_receive_event() {
    // Start a file server so the download has a real URL
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let file_url = test_server.file_url();

    let (ws_url, shutdown_tx, tmp) = start_ws_server().await;
    let dest_dir = tmp.path().join("output");

    let (mut ws, _response) = connect_async(&ws_url).await.unwrap();

    // Send addUri
    let add_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "aria2.addUri",
        "params": [[file_url], {"dir": dest_dir.to_string_lossy(), "out": "test.bin"}]
    });
    ws.send(Message::Text(add_req.to_string().into())).await.unwrap();

    // Read the JSON-RPC response
    let resp_text = match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => t,
        Ok(Some(Ok(other))) => panic!("Expected text message, got: {:?}", other),
        Ok(Some(Err(e))) => panic!("WebSocket error: {e}"),
        Ok(None) => panic!("WebSocket closed before response"),
        Err(_) => panic!("Timeout waiting for addUri response"),
    };
    let resp: serde_json::Value = serde_json::from_str(&resp_text).unwrap();
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let gid = resp["result"].as_str().unwrap().to_string();
    assert!(!gid.is_empty());

    // Wait for server-pushed aria2.onDownloadStart event
    let mut received_event = false;
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        match tokio::time::timeout(Duration::from_secs(2), ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                let event: serde_json::Value = serde_json::from_str(&t).unwrap();
                if event["method"] == "aria2.onDownloadStart" {
                    let params = event["params"].as_array().unwrap();
                    assert_eq!(params[0]["gid"], gid, "Event gid must match addUri gid");
                    received_event = true;
                    break;
                }
                // Other events (e.g., onDownloadComplete, onDownloadError) are fine
            }
            Ok(Some(Ok(_))) => {} // ignore non-text
            Ok(Some(Err(e))) => {
                panic!("WebSocket error during event read: {e}");
            }
            Ok(None) => break,
            Err(_) => {} // timeout, continue loop
        }
    }
    assert!(
        received_event,
        "Did not receive aria2.onDownloadStart event within 10s"
    );

    // Cleanup
    let _ = shutdown_tx.send(true);
}

/// Every HTTP lifecycle transition must push exactly one notification over
/// the socket, in causal order. The historical bug this guards against was a
/// handler and the engine both broadcasting, which made AriaNg show phantom
/// state changes.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn websocket_lifecycle_notifications_are_pushed_exactly_once() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    // Throttled so the task cannot finish between two RPC calls.
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let (ws_url, shutdown_tx, tmp) = start_ws_server().await;
    let dest_dir = tmp.path().join("output");
    let mut ws = WsClient::connect(&ws_url).await;

    let resp = ws
        .rpc(
            "aria2.addUri",
            json!([[file_url], {"dir": dest_dir.to_string_lossy(), "out": "ws-life.bin"}]),
        )
        .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    ws.wait_notification("aria2.onDownloadStart", &gid).await;

    for (method, event) in [
        ("aria2.pause", "aria2.onDownloadPause"),
        ("aria2.unpause", "aria2.onDownloadStart"),
        ("aria2.remove", "aria2.onDownloadStop"),
    ] {
        let resp = ws.rpc(method, json!([gid])).await;
        assert_eq!(
            resp["result"].as_str(),
            Some(gid.as_str()),
            "{method} must echo the GID: {resp}"
        );
        ws.wait_notification(event, &gid).await;
    }

    // No duplicate or stray lifecycle event on top of those four transitions.
    ws.assert_quiet(Duration::from_millis(500)).await;
    let _ = shutdown_tx.send(true);
}

/// A finished HTTP download pushes `aria2.onDownloadComplete` (from the
/// executor's finalize step, not the RPC handler).
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn websocket_pushes_completion_notification() {
    let test_server = crate::test_harness::TestServer::new(64 * 1024).await;

    let (ws_url, shutdown_tx, tmp) = start_ws_server().await;
    let dest_dir = tmp.path().join("output");
    let mut ws = WsClient::connect(&ws_url).await;

    let resp = ws
        .rpc(
            "aria2.addUri",
            json!([[test_server.file_url()], {"dir": dest_dir.to_string_lossy(), "out": "ws-done.bin"}]),
        )
        .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();

    ws.wait_notification("aria2.onDownloadStart", &gid).await;
    ws.wait_notification("aria2.onDownloadComplete", &gid).await;
    ws.assert_quiet(Duration::from_millis(500)).await;

    let _ = shutdown_tx.send(true);
}

/// A failed HTTP download pushes `aria2.onDownloadError` exactly once.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn websocket_pushes_error_notification() {
    let (ws_url, shutdown_tx, tmp) = start_ws_server().await;
    let dest_dir = tmp.path().join("output");
    let mut ws = WsClient::connect(&ws_url).await;

    // Port 1 refuses instantly; one try means one terminal error, no retries.
    let resp = ws
        .rpc(
            "aria2.addUri",
            json!([
                ["http://127.0.0.1:1/broken"],
                {"dir": dest_dir.to_string_lossy(), "out": "ws-fail.bin", "max-tries": "1"}
            ]),
        )
        .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();

    ws.wait_notification("aria2.onDownloadStart", &gid).await;
    ws.wait_notification("aria2.onDownloadError", &gid).await;
    ws.assert_quiet(Duration::from_millis(500)).await;

    let _ = shutdown_tx.send(true);
}

/// Malformed JSON over the WebSocket must be answered with a JSON-RPC parse
/// error, not dropped.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn websocket_rejects_malformed_json() {
    let (ws_url, shutdown_tx, _tmp) = start_ws_server().await;
    let mut ws = WsClient::connect(&ws_url).await;

    ws.ws
        .send(Message::Text("{ not json".into()))
        .await
        .expect("send malformed frame");
    let resp = ws.next_json().await;
    assert_eq!(resp["error"]["code"], -32700, "parse error: {resp}");
    assert!(resp["id"].is_null(), "a parse error has no request id: {resp}");

    let _ = shutdown_tx.send(true);
}

/// `aria2.addUri` with a magnet link must route to the BT backend, and the
/// BT alert bridge is the *only* emitter of its start notification — the
/// handler-side broadcast would make AriaNg show the torrent twice.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn websocket_bt_add_pushes_exactly_one_start() {
    let (ws_url, shutdown_tx, tmp) = start_ws_server().await;
    let dest_dir = tmp.path().join("output");
    let mut ws = WsClient::connect(&ws_url).await;

    let info_hash = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
    let magnet = format!("magnet:?xt=urn:btih:{info_hash}&dn=ws");
    let resp = ws
        .rpc(
            "aria2.addUri",
            json!([[magnet], {"dir": dest_dir.to_string_lossy()}]),
        )
        .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("magnet addUri failed: {resp}"))
        .to_string();

    ws.wait_notification("aria2.onDownloadStart", &gid).await;
    // Give a duplicate (the old handler-side broadcast) time to surface.
    ws.assert_quiet(Duration::from_secs(2)).await;

    // Cleanup: stop the torrent before the fake DHT lookup goes anywhere.
    let resp = ws.rpc("aria2.remove", json!([gid])).await;
    assert_eq!(resp["result"].as_str(), Some(gid.as_str()), "{resp}");

    let _ = shutdown_tx.send(true);
}

/// Regression: hot-reloading the RPC server (every settings save restarts it)
/// must not leave the endpoint dead. The predecessor with `with_graceful_shutdown`
/// drops its listener on a *different* task from the one that receives the
/// shutdown signal — and only after the signal propagates — while an aria2
/// client may still hold a WebSocket open. A replacement that binds once loses
/// that race with `AddrInUse`, `serve` returns, and AriaNg can no longer talk
/// to limedl. The replacement must wait the predecessor out and win the port.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn the_replacement_server_waits_out_the_predecessor_and_wins_the_port() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();
    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();

    // Stand in for the predecessor: it owns the port while the replacement
    // starts, and releases it only afterwards (as the real accept loop does).
    let holder = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = holder.local_addr().unwrap().port();

    let settings = Aria2RpcSettings {
        enabled: true,
        port,
        secret: None,
        cors_allowed_origins: vec![],
        ..Aria2RpcSettings::default()
    };
    let rpc = Aria2RpcServer::new(core.registry.clone(), &settings, core.event_bus.clone());
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let _ = rpc.serve(shutdown_rx).await;
    });

    // The replacement has already tried (and must have failed) to bind by now.
    tokio::time::sleep(Duration::from_millis(150)).await;
    drop(holder);

    // Only the retry can make this succeed; a plain bind already gave up.
    let port_open = {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    assert!(port_open, "the replacement never won port {port}");

    // It is the replacement answering, not a lucky connect to a dying listener.
    let mut ws = WsClient::connect(&format!("ws://127.0.0.1:{port}/jsonrpc")).await;
    let version = ws.rpc("aria2.getVersion", json!([])).await;
    assert!(version["result"]["version"].is_string(), "{version}");

    let _ = shutdown_tx.send(true);
}
