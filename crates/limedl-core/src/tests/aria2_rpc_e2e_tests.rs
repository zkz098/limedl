//! E2E tests for Aria2 RPC HTTP endpoint.
//!
//! Starts a real Aria2RpcServer backed by live subsystems, then sends
//! HTTP POST requests via reqwest to validate protocol compatibility.

use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::TempDir;

use crate::aria2_rpc::Aria2RpcServer;
use crate::event_bus::EventBus;
use crate::types::Aria2RpcSettings;

/// Bootstrap subsystems, start an Aria2RpcServer on a random port,
/// and return the HTTP base URL and shutdown channel.
async fn start_rpc_server() -> (
    String,
    tokio::sync::watch::Sender<bool>,
    TempDir,
    Arc<EventBus>,
) {
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
    tokio::spawn(async move {
        let _ = rpc.serve(shutdown_rx, settings.cors_allowed_origins).await;
    });

    // Wait for server to be ready
    tokio::time::sleep(Duration::from_millis(200)).await;

    let base_url = format!("http://127.0.0.1:{port}/jsonrpc");
    (base_url, shutdown_tx, tmp, core.event_bus)
}

/// Send a JSON-RPC request and return the parsed response.
async fn rpc_call(
    client: &reqwest::Client,
    url: &str,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let text = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

// ── Tests ──────────────────────────────────────────────────────────

/// Full lifecycle: addUri → tellStatus → tellActive → dedup → getVersion
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn aria2_add_uri_lifecycle_and_dedup() {
    let test_server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let file_url = test_server.file_url();

    let (rpc_url, shutdown_tx, _tmp, _event_bus) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = _tmp.path().join("output");

    // ── Test 1: aria2.addUri ──
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "test.bin"}
        ]),
    )
    .await;

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let gid = resp["result"]
        .as_str()
        .expect("addUri should return GID string")
        .to_string();
    assert!(!gid.is_empty(), "GID must not be empty");
    assert_eq!(gid.len(), 16, "GID should be 16 hex chars");

    // ── Test 2: aria2.tellStatus ──
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid]),
    )
    .await;

    assert_eq!(resp["jsonrpc"], "2.0");
    let status = &resp["result"];
    assert_eq!(
        status["gid"].as_str().unwrap(),
        gid,
        "tellStatus must return matching GID"
    );
    assert!(
        status.get("totalLength").is_some(),
        "tellStatus must include totalLength"
    );
    assert!(
        status.get("completedLength").is_some(),
        "tellStatus must include completedLength"
    );
    assert!(
        status.get("downloadSpeed").is_some(),
        "tellStatus must include downloadSpeed"
    );
    assert!(
        status.get("status").is_some(),
        "tellStatus must include status"
    );
    assert!(
        status.get("files").and_then(|v| v.as_array()).is_some(),
        "tellStatus must include files array"
    );
    assert!(status.get("dir").is_some(), "tellStatus must include dir");

    // ── Test 3: aria2.tellActive ──
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellActive",
        serde_json::Value::Array(vec![]),
    )
    .await;

    assert_eq!(resp["jsonrpc"], "2.0");
    let active = resp["result"]
        .as_array()
        .expect("tellActive must return array");
    // At least our download should appear if it started downloading
    // (it may be queued if scheduler hasn't picked it up yet — either is valid)

    // ── Test 4: aria2.tellWaiting ──
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellWaiting",
        serde_json::json!([0, 100]),
    )
    .await;

    assert_eq!(resp["jsonrpc"], "2.0");
    let waiting = resp["result"]
        .as_array()
        .expect("tellWaiting must return array");
    // Download is either active or waiting — at least one of tellActive/tellWaiting should contain it
    let found = active.iter().any(|s| s["gid"] == gid) || waiting.iter().any(|s| s["gid"] == gid);
    assert!(
        found,
        "Download GID {gid} must appear in either tellActive or tellWaiting"
    );

    // ── Test 5: aria2.getVersion ──
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getVersion",
        serde_json::Value::Array(vec![]),
    )
    .await;

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["result"]["version"], "0.1.0");
    let features = resp["result"]["enabledFeatures"]
        .as_array()
        .expect("enabledFeatures must be array");
    assert!(
        features.iter().any(|f| f == "BitTorrent"),
        "Must advertise BitTorrent support"
    );

    // ── Test 6: Dedup — same URL twice returns same GID ──
    let resp2 = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "test.bin"}
        ]),
    )
    .await;

    let gid2 = resp2["result"].as_str().unwrap();
    assert_eq!(
        gid2, gid,
        "Dedup: same URL must return same GID (got {gid2}, expected {gid})"
    );

    // ── Test 7: Method not found ──
    let resp = rpc_call(
        &client,
        &rpc_url,
        "nonexistent.method",
        serde_json::Value::Array(vec![]),
    )
    .await;

    assert!(
        resp["error"].is_object(),
        "Unknown method must return error"
    );
    assert_eq!(
        resp["error"]["code"], -32601,
        "Unknown method error code must be -32601"
    );

    // ── Cleanup ──
    let _ = shutdown_tx.send(true);
}

/// Test that addUri with missing URIs returns error.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_add_uri_missing_uris_returns_error() {
    let (rpc_url, shutdown_tx, _tmp, _event_bus) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(&client, &rpc_url, "aria2.addUri", serde_json::json!([])).await;

    assert!(resp["error"].is_object(), "Missing URIs must return error");
    assert_eq!(resp["error"]["code"], -32602);

    let _ = shutdown_tx.send(true);
}

/// Test aria2.getGlobalStat returns expected counters.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_global_stat_returns_counters() {
    let (rpc_url, shutdown_tx, _tmp, _event_bus) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getGlobalStat",
        serde_json::Value::Array(vec![]),
    )
    .await;

    assert_eq!(resp["jsonrpc"], "2.0");
    let stat = &resp["result"];
    assert!(stat.get("downloadSpeed").is_some());
    assert!(stat.get("numActive").is_some());
    assert!(stat.get("numWaiting").is_some());
    assert!(stat.get("numStopped").is_some());

    let _ = shutdown_tx.send(true);
}
