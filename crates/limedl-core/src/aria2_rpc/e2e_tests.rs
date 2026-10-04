//! E2E tests for Aria2 RPC HTTP endpoint.
//!
//! Starts a real Aria2RpcServer backed by live subsystems, then sends
//! HTTP POST requests via reqwest to validate protocol compatibility.
//!
//! The harness returns the whole [`crate::bootstrap::CoreSystems`] so tests can
//! reach engine internals (e.g. lower `max_in_memory_downloads` to force
//! terminal-task eviction) without bootstrapping a second engine.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use ntest::timeout;
use tempfile::TempDir;

use crate::aria2_rpc::Aria2RpcServer;
use crate::event_bus::{DownloadEvent, EventBus};
use crate::types::{Aria2AuthMode, Aria2Client, Aria2RpcSettings};

/// Bootstrap subsystems, start an Aria2RpcServer on a random port, and return
/// the HTTP base URL, shutdown channel and live core systems.
pub(super) async fn start_rpc_server_with(
    secret: Option<&str>,
    cors_allowed_origins: Vec<String>,
) -> (
    String,
    tokio::sync::watch::Sender<bool>,
    TempDir,
    crate::bootstrap::CoreSystems,
) {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    tokio::fs::create_dir_all(&dest_dir).await.unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let (base_url, shutdown_tx) = spawn_rpc_server(&core, secret, cors_allowed_origins).await;
    (base_url, shutdown_tx, tmp, core)
}

/// Spawn an [`Aria2RpcServer`] over an already-bootstrapped core on a random
/// port and poll until it accepts connections.
///
/// Split out of [`start_rpc_server_with`] so a test can serve the *same* live
/// core from a second server: each `Aria2RpcServer` owns a fresh `RpcContext`,
/// hence a fresh (empty) GID cache — the cheapest faithful stand-in for an app
/// restart when the test needs a GID that only the database can resolve.
async fn spawn_rpc_server(
    core: &crate::bootstrap::CoreSystems,
    secret: Option<&str>,
    cors_allowed_origins: Vec<String>,
) -> (String, tokio::sync::watch::Sender<bool>) {
    let settings = Aria2RpcSettings {
        enabled: true,
        port: 0,
        secret: secret.map(str::to_string),
        cors_allowed_origins,
        ..Aria2RpcSettings::default()
    };
    spawn_rpc_server_with_settings(core, settings).await
}

/// Spawn an [`Aria2RpcServer`] from a full settings value on a random port and
/// poll until it accepts connections.
///
/// Split out of [`spawn_rpc_server`] so a test can exercise non-secret auth
/// modes (`auth_mode` / `clients`) without threading more parameters through
/// every call site.
async fn spawn_rpc_server_with_settings(
    core: &crate::bootstrap::CoreSystems,
    settings: Aria2RpcSettings,
) -> (String, tokio::sync::watch::Sender<bool>) {
    // Reserve a random port
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let settings = Aria2RpcSettings { port, ..settings };
    let rpc = Aria2RpcServer::new(core.registry.clone(), &settings, core.event_bus.clone());

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        let _ = rpc.serve(shutdown_rx, settings.cors_allowed_origins).await;
    });

    // Poll until the server is ready to accept connections.
    // On slow CI runners, a fixed 200ms sleep may not be sufficient.
    let client = reqwest::Client::new();
    let health_url = format!("http://127.0.0.1:{port}/jsonrpc");
    loop {
        if client.get(&health_url).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    (format!("http://127.0.0.1:{port}/jsonrpc"), shutdown_tx)
}

/// Start an RPC server without a secret (the settings a desktop install uses
/// by default).
pub(super) async fn start_rpc_server() -> (
    String,
    tokio::sync::watch::Sender<bool>,
    TempDir,
    crate::bootstrap::CoreSystems,
) {
    start_rpc_server_with(None, vec![]).await
}

/// Start an RPC server protected by a secret token.
pub(super) async fn start_rpc_server_with_secret(secret: Option<&str>) -> (
    String,
    tokio::sync::watch::Sender<bool>,
    TempDir,
    crate::bootstrap::CoreSystems,
) {
    start_rpc_server_with(secret, vec![]).await
}

/// Start an RPC server in per-client token mode with the given clients.
async fn start_rpc_server_with_clients(
    clients: Vec<Aria2Client>,
) -> (
    String,
    tokio::sync::watch::Sender<bool>,
    TempDir,
    crate::bootstrap::CoreSystems,
) {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    tokio::fs::create_dir_all(&dest_dir).await.unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let settings = Aria2RpcSettings {
        enabled: true,
        port: 0,
        auth_mode: Aria2AuthMode::PerClient,
        clients,
        ..Aria2RpcSettings::default()
    };
    let (base_url, shutdown_tx) = spawn_rpc_server_with_settings(&core, settings).await;
    (base_url, shutdown_tx, tmp, core)
}

/// Poll `aria2.tellStatus` until the aria2 status string is one of `expected`.
pub(super) async fn wait_for_status(
    client: &reqwest::Client,
    rpc_url: &str,
    gid: &str,
    expected: &[&str],
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let resp = rpc_call(client, rpc_url, "aria2.tellStatus", serde_json::json!([gid])).await;
        if let Some(status) = resp["result"]["status"].as_str()
            && expected.contains(&status)
        {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "gid {gid} never reached {expected:?}: {resp}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Send a JSON-RPC request and return the parsed response.
pub(super) async fn rpc_call(
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

/// POST an arbitrary body (valid or not) and parse the JSON-RPC answer.
pub(super) async fn rpc_post_raw(client: &reqwest::Client, url: &str, body: &str) -> serde_json::Value {
    let text = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
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
    // Throttled on purpose. `aria2.addUri` only dedups against a *non-terminal*
    // download (`find_active_by_url`, manager.rs), and this test relies on the
    // task it just added still being around several RPC round-trips later: test 4
    // expects it in tellActive/tellWaiting, test 6 expects the same GID back.
    // Over the plain `/file` endpoint the 1 MiB loopback transfer finished in
    // milliseconds — i.e. between two of those calls — so the test raced its own
    // fixture and failed on whichever assertion happened to land after the
    // download completed. 16 KiB/s leaves ~65s of headroom for the whole body.
    let file_url = test_server.file_url_bandwidth(16 * 1024);

    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
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
    // Spell the fixture invariant out rather than assuming it: if the download
    // ever reaches a terminal state before this point, dedup is *supposed* to
    // hand back a new GID, and the failure below would blame the product instead
    // of the fixture.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid]),
    )
    .await;
    let state = resp["result"]["status"].as_str().unwrap_or("<missing>");
    assert!(
        matches!(state, "active" | "waiting" | "paused"),
        "fixture must keep the download non-terminal for the dedup check, got status={state}"
    );

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
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(&client, &rpc_url, "aria2.addUri", serde_json::json!([])).await;

    assert!(resp["error"].is_object(), "Missing URIs must return error");
    assert_eq!(resp["error"]["code"], -32602);

    // An explicitly empty URI array is a different code path from a missing
    // array entirely, and must be rejected just as loudly.
    let resp = rpc_call(&client, &rpc_url, "aria2.addUri", serde_json::json!([[]])).await;
    assert!(
        resp["error"].is_object(),
        "An empty uris array must return error: {resp}"
    );
    assert_eq!(resp["error"]["code"], -32602);

    let _ = shutdown_tx.send(true);
}

/// Test aria2.getGlobalStat returns expected counters.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_global_stat_returns_counters() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
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

/// `aria2.addUri` must forward `header` / `referer` options to the HTTP layer.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn aria2_add_uri_forwards_headers_to_http_requests() {
    use axum::{
        Router,
        extract::State,
        http::{HeaderMap, StatusCode, header},
        response::IntoResponse,
        routing::get,
    };

    #[derive(Clone)]
    struct Capture {
        seen: Arc<tokio::sync::Mutex<Vec<HeaderMap>>>,
    }

    async fn serve(State(state): State<Capture>, headers: HeaderMap) -> impl IntoResponse {
        state.seen.lock().await.push(headers);
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::CONTENT_LENGTH, "5".parse().unwrap());
        response_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
        (StatusCode::OK, response_headers, b"hello".to_vec())
    }

    let seen = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let app = Router::new()
        .route("/file", get(serve))
        .with_state(Capture { seen: seen.clone() });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");
    let url = format!("http://127.0.0.1:{}/file", addr.port());

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [url],
            {
                "dir": dest_dir.to_string_lossy(),
                "out": "headers.bin",
                "header": ["X-Test-Header: abc", "Accept-Language: ja"],
                "referer": "https://example.com/page"
            }
        ]),
    )
    .await;
    assert!(resp["result"].is_string(), "addUri failed: {resp}");

    // Wait until the download actually hits the capture server.
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if !seen.lock().await.is_empty() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no HTTP request reached the capture server"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let seen = seen.lock().await;
    for headers in seen.iter() {
        assert_eq!(
            headers.get("x-test-header").and_then(|v| v.to_str().ok()),
            Some("abc")
        );
        assert_eq!(
            headers.get("accept-language").and_then(|v| v.to_str().ok()),
            Some("ja")
        );
        assert_eq!(
            headers.get("referer").and_then(|v| v.to_str().ok()),
            Some("https://example.com/page")
        );
    }

    let _ = shutdown_tx.send(true);
}

/// Multiple URIs are treated as an ordered mirror list (aria2 semantics).
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn aria2_add_uri_multiple_uris_use_mirror_fallback() {
    let test_server = crate::test_harness::TestServer::new(512 * 1024).await;
    let mirror_url = test_server.file_url_range();

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            ["http://127.0.0.1:1/broken", mirror_url],
            {"dir": dest_dir.to_string_lossy(), "out": "mirror.bin", "split": "1"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .expect("addUri should return GID")
        .to_string();

    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let status = rpc_call(
            &client,
            &rpc_url,
            "aria2.tellStatus",
            serde_json::json!([gid]),
        )
        .await;
        match status["result"]["status"].as_str() {
            Some("complete") => break,
            Some("error") => panic!("mirror fallback failed: {status}"),
            _ => {}
        }
        assert!(
            std::time::Instant::now() < deadline,
            "mirror fallback timed out: {status}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let _ = shutdown_tx.send(true);
}

/// `pause` / `unpause` / `pauseAll` / `unpauseAll` / `remove` on a live task.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_pause_unpause_bulk_and_remove_lifecycle() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    // Throttled so the task cannot reach a terminal state between two RPC calls.
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "life.bin"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();

    // pause → the GID comes back and the status flips.
    let resp = rpc_call(&client, &rpc_url, "aria2.pause", serde_json::json!([gid])).await;
    assert_eq!(
        resp["result"].as_str(),
        Some(gid.as_str()),
        "pause must echo the GID: {resp}"
    );
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    // unpause → schedulable again.
    let resp = rpc_call(&client, &rpc_url, "aria2.unpause", serde_json::json!([gid])).await;
    assert_eq!(
        resp["result"].as_str(),
        Some(gid.as_str()),
        "unpause must echo the GID: {resp}"
    );
    wait_for_status(&client, &rpc_url, &gid, &["active", "waiting"]).await;

    // The bulk variants act globally and answer "OK".
    let resp = rpc_call(&client, &rpc_url, "aria2.pauseAll", serde_json::json!([])).await;
    assert_eq!(resp["result"], "OK", "pauseAll must answer OK: {resp}");
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    let resp = rpc_call(&client, &rpc_url, "aria2.unpauseAll", serde_json::json!([])).await;
    assert_eq!(resp["result"], "OK", "unpauseAll must answer OK: {resp}");
    wait_for_status(&client, &rpc_url, &gid, &["active", "waiting"]).await;

    // remove drops the task record (files stay on disk).
    let resp = rpc_call(&client, &rpc_url, "aria2.remove", serde_json::json!([gid])).await;
    assert_eq!(
        resp["result"].as_str(),
        Some(gid.as_str()),
        "remove must echo the GID: {resp}"
    );
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid]),
    )
    .await;
    assert!(
        resp["error"].is_object(),
        "a removed GID must no longer resolve: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// `tellStopped` / `getFiles` / `getUris` / `getPeers` / `getOption` /
/// `getGlobalOption` / `changeGlobalOption` must return aria2-shaped payloads.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_query_methods_return_aria2_shapes() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");
    let dest = dest_dir.to_string_lossy().to_string();

    // `"pause": "true"` is how aria2 serialises a boolean option over JSON-RPC.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest, "out": "query.bin", "pause": "true"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    // getFiles: one entry per file, aria2's string fields.
    let resp = rpc_call(&client, &rpc_url, "aria2.getFiles", serde_json::json!([gid])).await;
    let files = resp["result"]
        .as_array()
        .unwrap_or_else(|| panic!("getFiles must return an array: {resp}"));
    assert_eq!(files.len(), 1, "an HTTP download has exactly one file: {resp}");
    assert_eq!(files[0]["index"], "1");
    assert!(
        files[0]["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("query.bin"))
    );
    assert_eq!(files[0]["selected"], "true");
    assert!(files[0]["uris"].is_array());

    // getUris: the primary URL with aria2's "used" status.
    let resp = rpc_call(&client, &rpc_url, "aria2.getUris", serde_json::json!([gid])).await;
    let uris = resp["result"]
        .as_array()
        .unwrap_or_else(|| panic!("getUris must return an array: {resp}"));
    assert_eq!(uris.len(), 1);
    assert_eq!(uris[0]["uri"].as_str(), Some(file_url.as_str()));
    assert_eq!(uris[0]["status"], "used");

    // getPeers: HTTP downloads have no BitTorrent peers, but the method must exist.
    let resp = rpc_call(&client, &rpc_url, "aria2.getPeers", serde_json::json!([gid])).await;
    assert_eq!(resp["result"], serde_json::json!([]));

    // getOption: per-task effective options.
    let resp = rpc_call(&client, &rpc_url, "aria2.getOption", serde_json::json!([gid])).await;
    let options = &resp["result"];
    assert_eq!(options["out"], "query.bin", "getOption: {resp}");
    assert_eq!(options["gid"].as_str(), Some(gid.as_str()));
    assert!(options["dir"].as_str().is_some_and(|d| !d.is_empty()));
    assert!(options["user-agent"].is_string());
    assert!(options["header"].is_array());

    // getGlobalOption: global defaults, serialised as strings.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getGlobalOption",
        serde_json::json!([]),
    )
    .await;
    assert!(resp["result"]["dir"].is_string());
    assert!(resp["result"]["max-concurrent-downloads"].is_string());

    // changeGlobalOption applies a valid value and reflects it back.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeGlobalOption",
        serde_json::json!([{"max-concurrent-downloads": "3"}]),
    )
    .await;
    assert_eq!(
        resp["result"], "OK",
        "changeGlobalOption must answer OK: {resp}"
    );
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getGlobalOption",
        serde_json::json!([]),
    )
    .await;
    assert_eq!(resp["result"]["max-concurrent-downloads"], "3");

    // A relative `dir` is refused instead of silently accepted.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeGlobalOption",
        serde_json::json!([{"dir": "relative/path"}]),
    )
    .await;
    assert_eq!(
        resp["error"]["code"], -32602,
        "a relative dir must be rejected: {resp}"
    );

    // tellStopped is well-formed even while nothing has stopped yet.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    assert!(
        resp["result"].is_array(),
        "tellStopped must return an array: {resp}"
    );

    // getSessionInfo / saveSession are trivial but are part of the client
    // handshake — they must answer instead of 404ing.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getSessionInfo",
        serde_json::json!([]),
    )
    .await;
    assert!(
        resp["result"]["sessionId"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "getSessionInfo must return a non-empty session id: {resp}"
    );
    let resp = rpc_call(&client, &rpc_url, "aria2.saveSession", serde_json::json!([])).await;
    assert_eq!(resp["result"], "OK");

    let _ = shutdown_tx.send(true);
}

/// A completed task is listed by `tellStopped` and cleared by
/// `purgeDownloadResult`.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_purge_download_result_clears_tell_stopped() {
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let file_url = test_server.file_url_range();

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "done.bin"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    wait_for_status(&client, &rpc_url, &gid, &["complete"]).await;

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    let stopped = resp["result"]
        .as_array()
        .unwrap_or_else(|| panic!("tellStopped must return an array: {resp}"));
    let entry = stopped
        .iter()
        .find(|s| s["gid"].as_str() == Some(gid.as_str()))
        .unwrap_or_else(|| panic!("completed GID missing from tellStopped: {resp}"));
    assert_eq!(entry["status"], "complete");
    assert!(entry["files"].is_array());

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.purgeDownloadResult",
        serde_json::json!([]),
    )
    .await;
    assert_eq!(resp["result"], "OK");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    assert!(
        resp["result"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["gid"].as_str() != Some(gid.as_str())),
        "purgeDownloadResult must clear the stopped list: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// `system.multicall` batches calls and wraps each result the way aria2 does:
/// `[value]` on success, `[{"code","message"}]` on failure.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_multicall_batches_calls_and_wraps_errors() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "multi.bin", "pause": "true"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    let resp = rpc_call(
        &client,
        &rpc_url,
        "system.multicall",
        serde_json::json!([[
            {"methodName": "aria2.getVersion", "params": []},
            {"methodName": "aria2.tellStatus", "params": [gid]},
            {"methodName": "no.such.method", "params": []},
        ]]),
    )
    .await;
    let results = resp["result"]
        .as_array()
        .unwrap_or_else(|| panic!("multicall must return an array: {resp}"));
    assert_eq!(results.len(), 3);
    // Success entries are single-element arrays: a client reading `entry[0]`
    // must receive the value (a two-element `[null, value]` reads as failure).
    assert_eq!(results[0].as_array().map(Vec::len), Some(1));
    assert_eq!(results[0][0]["version"], "0.1.0");
    assert_eq!(results[1].as_array().map(Vec::len), Some(1));
    assert_eq!(results[1][0]["gid"].as_str(), Some(gid.as_str()));
    // Failures carry the error object where the value would be.
    assert_eq!(results[2].as_array().map(Vec::len), Some(1));
    assert_eq!(results[2][0]["code"], -32601);

    // Missing methods array is an invalid-params error.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "system.multicall",
        serde_json::json!([]),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602);

    let _ = shutdown_tx.send(true);
}

/// With a secret configured, **every** method requires the token — including
/// With a secret configured, every *routed data* method requires the token —
/// including the ones that used to skip the check entirely (`getVersion`,
/// `tellActive`, `pauseAll`, `shutdown`) — and a valid token is accepted by
/// single calls and by multicall. The read-only `system.listMethods` /
/// `system.listNotifications` are intentionally anonymous, as in aria2.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_secret_token_gates_every_method() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) =
        start_rpc_server_with_secret(Some("s3cr3t")).await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    for method in [
        "aria2.getVersion",
        "aria2.tellActive",
        "aria2.pauseAll",
        "aria2.shutdown",
    ] {
        let resp = rpc_call(&client, &rpc_url, method, serde_json::json!([])).await;
        assert_eq!(
            resp["error"]["code"], 1,
            "{method} must require the token: {resp}"
        );
    }

    // aria2 allows the capability-probe methods without a token.
    for method in ["system.listMethods", "system.listNotifications"] {
        let resp = rpc_call(&client, &rpc_url, method, serde_json::json!([])).await;
        assert!(
            resp["result"].is_array(),
            "{method} must not require the token: {resp}"
        );
    }

    // A wrong token is rejected too.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getVersion",
        serde_json::json!(["token:wrong"]),
    )
    .await;
    assert_eq!(resp["error"]["code"], 1, "wrong token must be rejected: {resp}");

    // The right token is accepted.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getVersion",
        serde_json::json!(["token:s3cr3t"]),
    )
    .await;
    assert_eq!(
        resp["result"]["version"], "0.1.0",
        "the valid token must be accepted: {resp}"
    );

    // Real work with the token: add, query, then a token-bearing multicall.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            "token:s3cr3t",
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "secret.bin"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("token-authenticated addUri failed: {resp}"))
        .to_string();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!(["token:s3cr3t", gid]),
    )
    .await;
    assert_eq!(resp["result"]["gid"].as_str(), Some(gid.as_str()));

    let resp = rpc_call(
        &client,
        &rpc_url,
        "system.multicall",
        serde_json::json!([
            "token:s3cr3t",
            [
                {"methodName": "aria2.getVersion", "params": ["token:s3cr3t"]},
                {"methodName": "aria2.tellStatus", "params": ["token:s3cr3t", gid]},
            ]
        ]),
    )
    .await;
    let results = resp["result"]
        .as_array()
        .unwrap_or_else(|| panic!("multicall with the token must succeed: {resp}"));
    assert_eq!(results[0][0]["version"], "0.1.0");
    assert_eq!(results[1][0]["gid"].as_str(), Some(gid.as_str()));

    let _ = shutdown_tx.send(true);
}

/// Per-client mode: every client authenticates with its own token, the shared
/// `secret` is ignored, and a valid token still reaches the handler with its
/// arguments intact (the `token:` element is stripped before dispatch).
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_per_client_tokens_authenticate_independently() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let token_a = "client-a-token";
    let token_b = "client-b-token";
    let clients = vec![
        Aria2Client {
            id: "a".to_string(),
            name: "AriaNg".to_string(),
            token_hash: crate::aria2_rpc::hash_token(token_a).expect("hash a"),
            created_at_ms: 1,
        },
        Aria2Client {
            id: "b".to_string(),
            name: "Motrix".to_string(),
            token_hash: crate::aria2_rpc::hash_token(token_b).expect("hash b"),
            created_at_ms: 2,
        },
    ];

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server_with_clients(clients).await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    // A missing or unknown token is rejected, and a bare value without the
    // aria2 `token:` prefix is rejected too.
    for params in [
        serde_json::json!([]),
        serde_json::json!(["token:nope"]),
        serde_json::json!(["nope"]),
    ] {
        let resp = rpc_call(&client, &rpc_url, "aria2.getVersion", params).await;
        assert_eq!(resp["error"]["code"], 1, "must be rejected: {resp}");
    }

    // A real download with the token proves the parameter was stripped.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            format!("token:{token_a}"),
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "per-client.bin"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("per-client addUri failed: {resp}"))
        .to_string();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([format!("token:{token_b}"), gid]),
    )
    .await;
    assert_eq!(
        resp["result"]["gid"].as_str(),
        Some(gid.as_str()),
        "the other client's token must work too: {resp}"
    );

    // Multicall carries authentication once at the outer level.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "system.multicall",
        serde_json::json!([
            format!("token:{token_a}"),
            [{"methodName": "aria2.getVersion", "params": [format!("token:{token_a}")]}],
        ]),
    )
    .await;
    assert_eq!(
        resp["result"][0][0]["version"], "0.1.0",
        "multicall with a per-client token must succeed: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// Per-client mode with no configured clients must reject everyone rather than
/// silently degrading to anonymous access.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn aria2_per_client_mode_without_clients_fails_closed() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_clients(Vec::new()).await;
    let client = reqwest::Client::new();
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getVersion",
        serde_json::json!(["token:anything"]),
    )
    .await;
    assert_eq!(
        resp["error"]["code"], 1,
        "an empty client list must reject every token: {resp}"
    );
    let _ = shutdown_tx.send(true);
}

/// Malformed JSON and a wrong `jsonrpc` version must come back as JSON-RPC
/// errors (`-32700` / `-32600`) over HTTP, not as a dropped connection or a
/// 500.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_transport_rejects_malformed_and_wrong_version_requests() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_post_raw(&client, &rpc_url, "{ not json").await;
    assert_eq!(resp["error"]["code"], -32700, "parse error: {resp}");
    assert!(resp["id"].is_null(), "a parse error has no request id: {resp}");

    let resp = rpc_post_raw(
        &client,
        &rpc_url,
        r#"{"jsonrpc":"1.0","id":7,"method":"aria2.getVersion","params":[]}"#,
    )
    .await;
    assert_eq!(resp["error"]["code"], -32600, "invalid version: {resp}");
    assert_eq!(resp["id"], 7, "the request id must be echoed: {resp}");

    let resp = rpc_post_raw(
        &client,
        &rpc_url,
        r#"{"jsonrpc":"2.0","id":8,"params":[]}"#,
    )
    .await;
    assert_eq!(
        resp["error"]["code"], -32700,
        "a request without a method is a parse error: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// CORS is the security boundary of the RPC endpoint: configured origins are
/// echoed back, everything else is refused, and unparsable configuration falls
/// back to localhost-only instead of allowing everybody.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_server_cors_follows_configured_origins() {
    let client = reqwest::Client::new();

    // A configured origin is allowed; an unknown one is not.
    let (rpc_url, shutdown_tx, _tmp, _core) =
        start_rpc_server_with(None, vec!["http://app.example".into()]).await;
    let resp = client
        .request(reqwest::Method::OPTIONS, &rpc_url)
        .header("Origin", "http://app.example")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .expect("preflight request");
    assert_eq!(
        resp.headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://app.example"),
        "the configured origin must be echoed"
    );
    let resp = client
        .request(reqwest::Method::OPTIONS, &rpc_url)
        .header("Origin", "http://evil.example")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .expect("preflight request");
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "an unlisted origin must not receive CORS headers"
    );
    let _ = shutdown_tx.send(true);

    // Unparsable origins fall back to localhost-only, never to `*`.
    let (rpc_url, shutdown_tx, _tmp, _core) =
        start_rpc_server_with(None, vec!["bad\norigin".into()]).await;
    let resp = client
        .request(reqwest::Method::OPTIONS, &rpc_url)
        .header("Origin", "http://localhost")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .expect("preflight request");
    assert_eq!(
        resp.headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://localhost"),
        "invalid configuration must fall back to localhost"
    );
    let resp = client
        .request(reqwest::Method::OPTIONS, &rpc_url)
        .header("Origin", "http://evil.example")
        .header("Access-Control-Request-Method", "POST")
        .send()
        .await
        .expect("preflight request");
    assert!(
        resp.headers().get("access-control-allow-origin").is_none(),
        "the fallback must still refuse foreign origins"
    );
    let _ = shutdown_tx.send(true);
}

/// A port conflict must surface as an error from `serve` instead of a silent
/// no-op (the desktop only logs it, but the server layer has to report it).
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_server_reports_a_port_conflict() {
    let blocker = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("blocker listener");
    let port = blocker.local_addr().unwrap().port();

    let settings = Aria2RpcSettings {
        enabled: true,
        port,
        secret: None,
        cors_allowed_origins: vec![],
        ..Aria2RpcSettings::default()
    };
    let server = Aria2RpcServer::new(
        std::sync::Arc::new(crate::backend_registry::BackendRegistry::new()),
        &settings,
        std::sync::Arc::new(EventBus::new(16)),
    );
    let (_tx, rx) = tokio::sync::watch::channel(false);

    let result = server.serve(rx, vec![]).await;
    assert!(
        result.is_err(),
        "binding an occupied port must be reported as an error"
    );
    drop(blocker);
}

/// `aria2.removeDownloadResult` drops one stopped result, and rejects tasks
/// that are still active (aria2 semantics).
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_remove_download_result_deletes_one_stopped_task() {
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let file_url = test_server.file_url_range();

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "stopped.bin"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    wait_for_status(&client, &rpc_url, &gid, &["complete"]).await;

    let stopped_contains = |resp: &serde_json::Value| {
        resp["result"]
            .as_array()
            .expect("tellStopped array")
            .iter()
            .any(|entry| entry["gid"].as_str() == Some(gid.as_str()))
    };
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    assert!(stopped_contains(&resp), "completed task must be listed: {resp}");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.removeDownloadResult",
        serde_json::json!([gid]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "removeDownloadResult: {resp}");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    assert!(!stopped_contains(&resp), "removed result must be gone: {resp}");
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid]),
    )
    .await;
    assert!(resp["error"].is_object(), "removed GID must not resolve: {resp}");

    // A still-running task is not a "stopped result".
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [test_server.file_url_bandwidth(16 * 1024)],
            {"dir": dest_dir.to_string_lossy(), "out": "active.bin"}
        ]),
    )
    .await;
    let active_gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.removeDownloadResult",
        serde_json::json!([active_gid]),
    )
    .await;
    assert_eq!(
        resp["error"]["code"], 1,
        "an active task must be rejected: {resp}"
    );
    let _ = rpc_call(
        &client,
        &rpc_url,
        "aria2.remove",
        serde_json::json!([active_gid]),
    )
    .await;

    let _ = shutdown_tx.send(true);
}

/// `aria2.changeOption` drives the live task (pause) and fails loudly for
/// options the engine cannot change at runtime.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_change_option_pause_and_rejections() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "options.bin"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"pause": "true"}]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "pause via changeOption: {resp}");
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"pause": "false"}]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "resume via changeOption: {resp}");
    wait_for_status(&client, &rpc_url, &gid, &["active", "waiting"]).await;

    // `split` cannot be changed after the task started — the error names it.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"split": "4"}]),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602, "unsupported option: {resp}");
    assert!(
        resp["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("split")),
        "the error must name the option: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// aria2's `keys` parameter filters the status object per entry.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_keys_filter_status_fields() {
    let test_server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    let file_url = test_server.file_url_bandwidth(32 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [file_url],
            {"dir": dest_dir.to_string_lossy(), "out": "keys.bin", "pause": "true"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid, ["gid", "status"]]),
    )
    .await;
    let object = resp["result"].as_object().expect("status object");
    assert_eq!(object.len(), 2, "only requested keys: {resp}");
    assert_eq!(object["gid"].as_str(), Some(gid.as_str()));
    assert_eq!(object["status"], "paused");

    // Waiting list entries honour the third parameter.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellWaiting",
        serde_json::json!([0, 100, ["gid"]]),
    )
    .await;
    let waiting = resp["result"].as_array().expect("waiting array");
    assert!(!waiting.is_empty(), "paused task must be waiting: {resp}");
    assert!(
        waiting.iter().all(|entry| entry.as_object().is_some_and(|o| o.len() == 1)),
        "each entry must contain only gid: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// `aria2.getUris` must list every candidate (primary + mirrors), not just the
/// URL currently in use.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_get_uris_lists_all_candidates() {
    let test_server = crate::test_harness::TestServer::new(512 * 1024).await;
    let mirror_url = test_server.file_url_range();

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");
    let primary = "http://127.0.0.1:1/broken".to_string();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([
            [primary, mirror_url],
            {"dir": dest_dir.to_string_lossy(), "out": "uris.bin", "split": "1"}
        ]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();

    let resp = rpc_call(&client, &rpc_url, "aria2.getUris", serde_json::json!([gid])).await;
    let uris = resp["result"].as_array().expect("getUris array");
    assert_eq!(uris.len(), 2, "primary plus mirror: {resp}");
    let listed: Vec<&str> = uris.iter().filter_map(|u| u["uri"].as_str()).collect();
    assert!(listed.contains(&"http://127.0.0.1:1/broken"), "{resp}");
    assert!(listed.contains(&mirror_url.as_str()), "{resp}");
    assert_eq!(
        uris.iter()
            .filter(|u| u["status"] == "used")
            .count(),
        1,
        "exactly one URI is in use: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// Terminal downloads evicted from the in-memory map must stay visible through
/// the database until they are purged or individually removed.
#[tokio::test(flavor = "multi_thread")]
#[timeout(180_000)]
async fn aria2_evicted_terminal_downloads_stay_queryable() {
    let test_server = crate::test_harness::TestServer::new(64 * 1024).await;

    let (rpc_url, shutdown_tx, tmp, core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    // `max_in_memory_downloads` normalises to at least 10, so eleven completed
    // tasks force the oldest one out of memory while it stays in the database.
    let mut settings = core.dispatcher.get_settings().await.expect("settings");
    settings.max_in_memory_downloads = 10;
    core.dispatcher
        .save_settings(&settings)
        .await
        .expect("save settings");

    let mut gids = Vec::new();
    for index in 0..11 {
        let resp = rpc_call(
            &client,
            &rpc_url,
            "aria2.addUri",
            serde_json::json!([
                [test_server.file_url_range()],
                {
                    "dir": dest_dir.to_string_lossy(),
                    "out": format!("evict-{index}.bin")
                }
            ]),
        )
        .await;
        let gid = resp["result"]
            .as_str()
            .unwrap_or_else(|| panic!("addUri failed: {resp}"))
            .to_string();
        wait_for_status(&client, &rpc_url, &gid, &["complete"]).await;
        gids.push(gid);
    }

    // All stopped results are listed even though at least one was evicted.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    let stopped = resp["result"].as_array().expect("tellStopped array");
    for gid in &gids {
        assert!(
            stopped.iter().any(|entry| entry["gid"].as_str() == Some(gid.as_str())),
            "evicted GID {gid} must stay visible: {resp}"
        );
    }

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getGlobalStat",
        serde_json::Value::Array(vec![]),
    )
    .await;
    assert_eq!(resp["result"]["numStopped"], "11", "{resp}");

    // Detail lookup and per-result removal work for evicted entries too.
    for gid in &gids {
        let resp = rpc_call(
            &client,
            &rpc_url,
            "aria2.tellStatus",
            serde_json::json!([gid]),
        )
        .await;
        assert_eq!(resp["result"]["status"], "complete", "{resp}");

        let resp = rpc_call(
            &client,
            &rpc_url,
            "aria2.removeDownloadResult",
            serde_json::json!([gid]),
        )
        .await;
        assert_eq!(resp["result"], "OK", "{resp}");
    }

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    assert!(
        resp["result"]
            .as_array()
            .expect("tellStopped array")
            .is_empty(),
        "all results were removed: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// A magnet link added through `aria2.addUri` must be routed to the BT backend.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_add_uri_accepts_magnet_as_bt() {
    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let info_hash = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";
    let magnet = format!("magnet:?xt=urn:btih:{info_hash}&dn=test");
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([[magnet], {"dir": dest_dir.to_string_lossy() }]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("magnet addUri failed: {resp}"))
        .to_string();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid]),
    )
    .await;
    let reported = resp["result"]["infoHash"]
        .as_str()
        .unwrap_or_else(|| panic!("BT status must carry an infoHash: {resp}"));
    assert_eq!(reported.to_ascii_lowercase(), info_hash);
    assert_eq!(resp["result"]["files"], serde_json::json!([]));

    // Cleanup: stop the torrent before the fake DHT lookup goes anywhere.
    let resp = rpc_call(&client, &rpc_url, "aria2.remove", serde_json::json!([gid])).await;
    assert_eq!(resp["result"].as_str(), Some(gid.as_str()), "{resp}");

    let _ = shutdown_tx.send(true);
}

/// `aria2.addTorrent` validates the payload before touching the engine.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_add_torrent_rejects_invalid_base64() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addTorrent",
        serde_json::json!(["not base64 !!!"]),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602, "{resp}");

    let _ = shutdown_tx.send(true);
}

/// Poll `aria2.tellStatus` until the BitTorrent engine reports file metadata.
///
/// The fixture's metadata comes from the `.torrent` itself, so this converges
/// as soon as the engine has picked the torrent up — no peers involved.
async fn wait_for_bt_files(
    client: &reqwest::Client,
    rpc_url: &str,
    gid: &str,
) -> serde_json::Value {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let resp = rpc_call(client, rpc_url, "aria2.tellStatus", serde_json::json!([gid])).await;
        let files_ready = resp["result"]["files"]
            .as_array()
            .is_some_and(|files| !files.is_empty());
        if files_ready {
            return resp["result"].clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "BT metadata never produced a file list: {resp}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Poll `aria2.getFiles` until the files carry `expected` selection flags
/// ("true"/"false" as aria2 serialises them).
async fn wait_for_bt_selection(
    client: &reqwest::Client,
    rpc_url: &str,
    gid: &str,
    expected: &[&str],
) -> Vec<serde_json::Value> {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        let resp = rpc_call(client, rpc_url, "aria2.getFiles", serde_json::json!([gid])).await;
        if let Some(files) = resp["result"].as_array()
            && files.len() == expected.len()
            && files
                .iter()
                .zip(expected)
                .all(|(file, wanted)| file["selected"] == *wanted)
        {
            return files.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "file selection never became {expected:?}: {resp}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// `aria2.addTorrent` must add a real `.torrent`, expose the BitTorrent file
/// list through `getFiles`/`tellStatus`, and apply the options the engine can
/// actually change at runtime (`select-file`, per-torrent speed limits).
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_add_torrent_serves_bt_files_and_options() {
    let fixture = crate::bt_backend::tests::multi_file_torrent_bytes();
    let expected_hash = irontide::core::torrent_from_bytes(&fixture)
        .expect("the shared BT fixture must parse")
        .info_hash
        .to_hex();

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let encoded = base64::engine::general_purpose::STANDARD.encode(&fixture);
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addTorrent",
        serde_json::json!([encoded, {"dir": dest_dir.to_string_lossy()}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addTorrent failed: {resp}"))
        .to_string();

    // tellStatus: the real info hash and one aria2 entry per torrent file,
    // with 1-based string indices like aria2 itself emits.
    let status = wait_for_bt_files(&client, &rpc_url, &gid).await;
    assert_eq!(
        status["infoHash"]
            .as_str()
            .map(str::to_ascii_lowercase),
        Some(expected_hash.clone()),
        "tellStatus must report the fixture's info hash: {status}"
    );
    let files = status["files"].as_array().expect("files array");
    assert_eq!(files.len(), 2, "fixture has two files: {status}");
    assert_eq!(files[0]["index"], "1");
    assert_eq!(files[0]["length"], "10");
    assert!(
        files[0]["path"].as_str().is_some_and(|p| p.ends_with("a.txt")),
        "{status}"
    );
    assert_eq!(files[1]["index"], "2");
    assert_eq!(files[1]["length"], "20");
    assert!(
        files[1]["path"].as_str().is_some_and(|p| p.ends_with("b.txt")),
        "{status}"
    );
    assert_eq!(files[0]["selected"], "true", "{status}");

    // getFiles must agree with tellStatus.files.
    let resp = rpc_call(&client, &rpc_url, "aria2.getFiles", serde_json::json!([gid])).await;
    assert_eq!(
        resp["result"], status["files"],
        "getFiles and tellStatus.files must agree: {resp}"
    );

    // BitTorrent peers exist as a method; an offline fixture has none.
    let resp = rpc_call(&client, &rpc_url, "aria2.getPeers", serde_json::json!([gid])).await;
    assert_eq!(
        resp["result"],
        serde_json::json!([]),
        "an offline torrent has no peers: {resp}"
    );

    // select-file is 1-based and flips which files are wanted.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"select-file": "2"}]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "select-file must be accepted: {resp}");
    let files = wait_for_bt_selection(&client, &rpc_url, &gid, &["false", "true"]).await;
    assert_eq!(files[0]["selected"], "false", "{files:?}");
    assert_eq!(files[1]["selected"], "true", "{files:?}");

    // Speed limits accept aria2's string form; "0" means unlimited.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"max-download-limit": "0"}]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "max-download-limit: {resp}");
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"max-upload-limit": "1024"}]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "max-upload-limit: {resp}");

    // Malformed values and a 0 index are invalid params, not silent no-ops.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"max-download-limit": "fast"}]),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602, "invalid limit: {resp}");
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"select-file": "0"}]),
    )
    .await;
    assert_eq!(
        resp["error"]["code"], -32602,
        "0 is not a valid file index: {resp}"
    );

    // Cleanup: stop the torrent before the fake DHT lookup goes anywhere.
    let resp = rpc_call(&client, &rpc_url, "aria2.remove", serde_json::json!([gid])).await;
    assert_eq!(resp["result"].as_str(), Some(gid.as_str()), "{resp}");
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid]),
    )
    .await;
    assert!(
        resp["error"].is_object(),
        "a removed torrent must not resolve: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// Terminal rows evicted from memory stay in the database. A server whose GID
/// cache does not know them must still list them in `tellStopped`, remove them
/// individually and sweep them with `purgeDownloadResult`.
///
/// Starting a second `Aria2RpcServer` over the *same* live core is the minimal
/// restart simulation: only the cache (and the port) is new. Without it every
/// GID would be a cache hit and the database fallbacks in `remove_download_result`
/// / `purge_download_result` would never run.
#[tokio::test(flavor = "multi_thread")]
#[timeout(180_000)]
async fn aria2_evicted_results_are_removable_without_a_cached_gid() {
    let test_server = crate::test_harness::TestServer::new(64 * 1024).await;

    let (rpc_url, shutdown_tx, tmp, core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    // `max_in_memory_downloads` normalises to at least 10, so twelve completed
    // tasks leave at least two rows in the database but not in memory.
    let mut settings = core.dispatcher.get_settings().await.expect("settings");
    settings.max_in_memory_downloads = 10;
    core.dispatcher
        .save_settings(&settings)
        .await
        .expect("save settings");

    let mut gids = Vec::new();
    for index in 0..12 {
        let resp = rpc_call(
            &client,
            &rpc_url,
            "aria2.addUri",
            serde_json::json!([
                [test_server.file_url_range()],
                {
                    "dir": dest_dir.to_string_lossy(),
                    "out": format!("evict2-{index}.bin")
                }
            ]),
        )
        .await;
        let gid = resp["result"]
            .as_str()
            .unwrap_or_else(|| panic!("addUri failed: {resp}"))
            .to_string();
        wait_for_status(&client, &rpc_url, &gid, &["complete"]).await;
        gids.push(gid);
    }

    // Eviction happens after the lifecycle event; wait for the map to settle
    // before deciding which rows the second server can no longer resolve.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let in_memory = loop {
        let list = core.dispatcher.list().await.expect("list downloads");
        if list.len() <= 10 {
            break list;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "in-memory map never shrank to the limit: {} entries",
            list.len()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let in_memory_gids: Vec<String> = in_memory
        .iter()
        .map(|summary| crate::aria2_rpc::internal_id_to_gid(&summary.id))
        .collect();
    let evicted: Vec<String> = gids
        .iter()
        .filter(|gid| !in_memory_gids.contains(gid))
        .cloned()
        .collect();
    assert!(
        evicted.len() >= 2,
        "fixture must evict at least two rows (in memory: {}, added: {})",
        in_memory_gids.len(),
        gids.len()
    );

    // Fresh server over the same core: new `RpcContext`, empty GID cache.
    let _ = shutdown_tx.send(true);
    let (fresh_url, fresh_shutdown) = spawn_rpc_server(&core, None, vec![]).await;

    // tellStopped lists the evicted rows straight from the database.
    let resp = rpc_call(
        &client,
        &fresh_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    let stopped = resp["result"].as_array().expect("tellStopped array");
    for gid in &evicted {
        assert!(
            stopped
                .iter()
                .any(|entry| entry["gid"].as_str() == Some(gid.as_str())),
            "evicted GID {gid} must stay visible: {resp}"
        );
    }

    // removeDownloadResult finds the evicted row through its DB fallback.
    let resp = rpc_call(
        &client,
        &fresh_url,
        "aria2.removeDownloadResult",
        serde_json::json!([evicted[0]]),
    )
    .await;
    assert_eq!(
        resp["result"], "OK",
        "DB-fallback removal of an evicted row: {resp}"
    );

    // purgeDownloadResult sweeps the remaining evicted database rows too.
    let resp = rpc_call(
        &client,
        &fresh_url,
        "aria2.purgeDownloadResult",
        serde_json::json!([]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "purgeDownloadResult: {resp}");
    let resp = rpc_call(
        &client,
        &fresh_url,
        "aria2.tellStopped",
        serde_json::json!([0, 100]),
    )
    .await;
    let remaining = resp["result"].as_array().expect("tellStopped array");
    assert!(
        remaining.iter().all(|entry| entry["gid"]
            .as_str()
            .is_none_or(|gid| !evicted.iter().any(|e| e == gid))),
        "purgeDownloadResult must clear the evicted rows: {resp}"
    );

    let _ = fresh_shutdown.send(true);
}

/// `aria2.shutdown` cannot stop a managed subsystem, but it must answer the
/// client and warn the UI instead of looking like a no-op; the notification
/// list is part of the client handshake and must be complete.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn aria2_shutdown_warns_the_ui_and_notifications_are_listed() {
    let (rpc_url, shutdown_tx, _tmp, core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let mut events = core.event_bus.subscribe();

    let resp = rpc_call(&client, &rpc_url, "aria2.shutdown", serde_json::json!([])).await;
    assert!(
        resp["result"]
            .as_str()
            .is_some_and(|result| result.contains("application UI")),
        "shutdown must acknowledge the request: {resp}"
    );

    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("shutdown must publish a warning")
        .expect("event bus stays open");
    assert!(
        matches!(&event, DownloadEvent::Warning { id, .. } if id == "system"),
        "shutdown must warn the UI: {event:?}"
    );

    let resp = rpc_call(
        &client,
        &rpc_url,
        "system.listNotifications",
        serde_json::json!([]),
    )
    .await;
    let names: Vec<&str> = resp["result"]
        .as_array()
        .expect("listNotifications array")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "aria2.onDownloadStart",
            "aria2.onDownloadPause",
            "aria2.onDownloadStop",
            "aria2.onDownloadComplete",
            "aria2.onBtDownloadComplete",
            "aria2.onDownloadError",
        ],
        "the notification handshake must stay complete: {resp}"
    );

    let _ = shutdown_tx.send(true);
}

/// `changeGlobalOption` validates its inputs, and the `dir` it applies becomes
/// the destination of an `addUri` that omits `dir` — the file must land there
/// with exactly the bytes the server served.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_change_global_option_applies_default_dir_and_validates() {
    let test_server = crate::test_harness::TestServer::new(64 * 1024).await;

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let default_dir = tmp.path().join("global-default");
    tokio::fs::create_dir_all(&default_dir).await.unwrap();
    let default_dir = default_dir.to_string_lossy().to_string();

    // A missing options object is invalid params.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeGlobalOption",
        serde_json::json!([]),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602, "missing options: {resp}");

    // Empty and relative directories are refused.
    for bad_dir in ["", "relative/path"] {
        let resp = rpc_call(
            &client,
            &rpc_url,
            "aria2.changeGlobalOption",
            serde_json::json!([{"dir": bad_dir}]),
        )
        .await;
        assert_eq!(
            resp["error"]["code"], -32602,
            "dir {bad_dir:?} must be rejected: {resp}"
        );
    }

    // A valid directory applies and is reflected back.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeGlobalOption",
        serde_json::json!([{"dir": default_dir}]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "valid dir: {resp}");
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getGlobalOption",
        serde_json::json!([]),
    )
    .await;
    assert_eq!(
        resp["result"]["dir"], default_dir,
        "applied dir must be visible: {resp}"
    );

    // Unparsable values for the other recognised keys are ignored, not fatal.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeGlobalOption",
        serde_json::json!([{
            "max-overall-download-limit": "1024",
            "max-concurrent-downloads": "not-a-number"
        }]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "lenient limit parsing: {resp}");

    // addUri without a `dir` option uses the global default end to end.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        serde_json::json!([[test_server.file_url()], {"out": "default.bin"}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    wait_for_status(&client, &rpc_url, &gid, &["complete"]).await;

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.getOption",
        serde_json::json!([gid]),
    )
    .await;
    assert_eq!(
        resp["result"]["dir"], default_dir,
        "getOption must report the global default dir: {resp}"
    );
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.tellStatus",
        serde_json::json!([gid]),
    )
    .await;
    let written_path = std::path::PathBuf::from(
        resp["result"]["dir"]
            .as_str()
            .unwrap_or_else(|| panic!("tellStatus has no dir: {resp}")),
    );
    assert!(
        written_path.starts_with(&default_dir),
        "the file must land under the global default: {resp}"
    );

    let bytes = tokio::fs::read(&written_path)
        .await
        .unwrap_or_else(|e| panic!("read {}: {e}", written_path.display()));
    assert_eq!(
        bytes.len() as u64,
        test_server.file_size,
        "on-disk size must match the served file"
    );
    assert_eq!(
        blake3::hash(&bytes).to_hex().to_string(),
        test_server.blake3_hash,
        "on-disk bytes must match what the server served"
    );

    let _ = shutdown_tx.send(true);
}

/// A single-file `.torrent` has no per-file entries in its metadata; aria2
/// still expects one entry in `files`, synthesized from `name`/`length`.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn aria2_add_torrent_single_file_reports_one_entry() {
    let fixture = crate::bt_backend::tests::single_file_torrent_bytes();
    let expected_hash = irontide::core::torrent_from_bytes(&fixture)
        .expect("the single-file fixture must parse")
        .info_hash
        .to_hex();

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest_dir = tmp.path().join("output");

    let encoded = base64::engine::general_purpose::STANDARD.encode(&fixture);
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addTorrent",
        serde_json::json!([encoded, {"dir": dest_dir.to_string_lossy()}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addTorrent failed: {resp}"))
        .to_string();

    let status = wait_for_bt_files(&client, &rpc_url, &gid).await;
    assert_eq!(
        status["infoHash"]
            .as_str()
            .map(str::to_ascii_lowercase),
        Some(expected_hash),
        "tellStatus must report the fixture's info hash: {status}"
    );
    let files = status["files"].as_array().expect("files array");
    assert_eq!(
        files.len(),
        1,
        "a single-file torrent reports exactly one entry: {status}"
    );
    assert_eq!(files[0]["index"], "1");
    assert_eq!(files[0]["path"], "single.bin");
    assert_eq!(files[0]["length"], "30");
    assert_eq!(files[0]["selected"], "true");

    // aria2's empty `select-file` deselects everything; the synthesized entry
    // must follow it like any other file.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeOption",
        serde_json::json!([gid, {"select-file": ""}]),
    )
    .await;
    assert_eq!(resp["result"], "OK", "empty select-file: {resp}");
    let files = wait_for_bt_selection(&client, &rpc_url, &gid, &["false"]).await;
    assert_eq!(files[0]["selected"], "false", "{files:?}");

    // Cleanup: stop the torrent before the fake DHT lookup goes anywhere.
    let resp = rpc_call(&client, &rpc_url, "aria2.remove", serde_json::json!([gid])).await;
    assert_eq!(resp["result"].as_str(), Some(gid.as_str()), "{resp}");

    let _ = shutdown_tx.send(true);
}
