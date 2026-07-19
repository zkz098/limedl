use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use ntest::timeout;
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
    };
    let rpc = Aria2RpcServer::new(core.registry.clone(), &settings, core.event_bus.clone());

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move { let _ = rpc.serve(shutdown_rx, settings.cors_allowed_origins).await; });

    tokio::time::sleep(Duration::from_millis(200)).await;

    let ws_url = format!("ws://127.0.0.1:{port}/jsonrpc");
    (ws_url, shutdown_tx, tmp)
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
    ws.send(Message::Text(add_req.to_string())).await.unwrap();

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
