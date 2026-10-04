//! Tier 1 aria2 interoperability tests.
//!
//! These tests deliberately encode the *wire shapes real clients send*, not the
//! shapes limedl happens to expect. The distinction is not academic: AriaNg
//! sends `aria2.addTorrent` as `[torrent, [], options]`, and a suite that only
//! ever sends `[torrent, options]` passes while the client's `dir`/`out`/`pause`
//! options are silently dropped (that bug shipped once).
//!
//! The suite has two jobs:
//!   1. **Contract drift**: `system.listMethods` must equal the set of methods
//!      that actually route, and every advertised method must be reachable.
//!   2. **Client-shape fixtures**: real request shapes (AriaNg multicall,
//!      addTorrent with an empty `uris` array, tellStatus key expectations)
//!      must produce the aria2 response shape.
//!
//! It is pure Rust and needs no external binary, so it runs in the normal
//! `test-utils,aria2-rpc` nextest gate. A real `aria2c` oracle is Tier 2 and
//! lives outside this file (see `docs/aria2-interop-testing.md`).

use std::time::Duration;

use base64::Engine;
use ntest::timeout;
use serde_json::json;

use super::e2e_tests::{rpc_call, start_rpc_server, start_rpc_server_with_secret, wait_for_status};

/// The exact method surface limedl implements. `system.listMethods` must match
/// this — a method added to the dispatcher but not here (or vice versa) fails
/// the contract test, which is the point.
const EXPECTED_METHODS: &[&str] = &[
    "aria2.addTorrent",
    "aria2.addUri",
    "aria2.changeGlobalOption",
    "aria2.changeOption",
    "aria2.changePosition",
    "aria2.changeUri",
    "aria2.getFiles",
    "aria2.getGlobalOption",
    "aria2.getGlobalStat",
    "aria2.getOption",
    "aria2.getPeers",
    "aria2.getServers",
    "aria2.getSessionInfo",
    "aria2.getUris",
    "aria2.getVersion",
    "aria2.multicall",
    "aria2.pause",
    "aria2.forcePause",
    "aria2.pauseAll",
    "aria2.forcePauseAll",
    "aria2.purgeDownloadResult",
    "aria2.remove",
    "aria2.removeDownloadResult",
    "aria2.forceRemove",
    "aria2.saveSession",
    "aria2.shutdown",
    "aria2.forceShutdown",
    "aria2.tellActive",
    "aria2.tellStatus",
    "aria2.tellStopped",
    "aria2.tellWaiting",
    "aria2.unpause",
    "aria2.unpauseAll",
    "system.listMethods",
    "system.listNotifications",
    "system.multicall",
];

/// Find the position of `gid` in a `tellWaiting` reply.
fn index_in_waiting(resp: &serde_json::Value, gid: &str) -> Option<usize> {
    resp["result"].as_array()?.iter().position(|entry| entry["gid"] == gid)
}

// ── Contract ──────────────────────────────────────────────────────────────

/// `system.listMethods` must be exactly the implemented set, and every
/// advertised method must route (never `-32601`).
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn interop_list_methods_matches_the_routed_surface() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(&client, &rpc_url, "system.listMethods", json!([])).await;
    let mut advertised: Vec<String> = resp["result"]
        .as_array()
        .expect("listMethods must return an array")
        .iter()
        .filter_map(|value| value.as_str().map(String::from))
        .collect();
    advertised.sort();

    let mut expected: Vec<String> = EXPECTED_METHODS.iter().map(|s| (*s).to_string()).collect();
    expected.sort();
    assert_eq!(
        advertised, expected,
        "system.listMethods drifted from the implemented surface"
    );

    for method in &expected {
        if method == "system.listMethods" {
            continue;
        }
        // Empty params are enough: any routed method answers with invalid
        // params (-32602) or a result. Only method-not-found (-32601) means it
        // is advertised but not wired up.
        let resp = rpc_call(&client, &rpc_url, method, json!([])).await;
        assert_ne!(
            resp["error"]["code"].as_i64(),
            Some(-32601),
            "{method} is advertised but not routed: {resp}"
        );
    }

    let _ = shutdown_tx.send(true);
}

/// AriaNg's `system.multicall`: each nested call repeats `token:<secret>`, and
/// each result is a single-element array. A two-element `[null, value]` wrapper
/// would be read as a failure by AriaNg.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn interop_ariang_multicall_shape() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some("s3cr3t")).await;
    let client = reqwest::Client::new();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "system.multicall",
        json!([[
            {"methodName": "aria2.getVersion", "params": ["token:s3cr3t"]},
            {"methodName": "aria2.tellActive", "params": ["token:s3cr3t"]},
            {"methodName": "aria2.tellStatus", "params": ["token:s3cr3t", "deadbeefdeadbeef"]},
        ]]),
    )
    .await;

    let results = resp["result"]
        .as_array()
        .unwrap_or_else(|| panic!("multicall must return an array: {resp}"));
    assert_eq!(results.len(), 3);

    assert_eq!(results[0].as_array().map(Vec::len), Some(1), "{resp}");
    assert!(results[0][0]["version"].is_string(), "{resp}");
    assert!(results[0][0]["enabledFeatures"].is_array(), "{resp}");
    assert_eq!(results[1][0], json!([]));
    // A nested failure is wrapped as `[{code, message}]`, never a bare error.
    assert!(results[2][0]["code"].is_number(), "{resp}");
    assert!(results[2][0]["message"].is_string(), "{resp}");

    let _ = shutdown_tx.send(true);
}

// ── addTorrent / addUri positional shapes ─────────────────────────────────

/// AriaNg's `addTorrent([torrent, [], options])` shape must keep the options.
/// Reading `params[1]` as the options object used to drop `dir`/`out`/`pause`.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn interop_ariang_add_torrent_shape_keeps_options() {
    let fixture = crate::bt_backend::tests::single_file_torrent_bytes();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&fixture);

    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addTorrent",
        json!([encoded, [], {"pause": "true"}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addTorrent failed: {resp}"))
        .to_string();

    // `pause` lives in the options object. Had the empty `uris` array been read
    // as the options, the task would have started unpaused; reaching "paused"
    // proves the options survived.
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    let _ = rpc_call(&client, &rpc_url, "aria2.remove", json!([gid])).await;
    let _ = shutdown_tx.send(true);
}

/// The legacy `addTorrent([torrent, options])` shape (used by limedl's own
/// tests) must keep working alongside the aria2/AriaNg shape.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn interop_add_torrent_legacy_shape_keeps_options() {
    let fixture = crate::bt_backend::tests::single_file_torrent_bytes();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&fixture);

    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addTorrent",
        json!([encoded, {"pause": "true"}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addTorrent failed: {resp}"))
        .to_string();

    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    let _ = rpc_call(&client, &rpc_url, "aria2.remove", json!([gid])).await;
    let _ = shutdown_tx.send(true);
}

/// AriaNg's `addUri([urls, options])` shape keeps `dir`/`out`.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn interop_add_uri_shape_keeps_options() {
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let url = test_server.file_url_bandwidth(64 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest = tmp.path().join("output");
    let dest_str = dest.to_string_lossy().to_string();

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        json!([[url], {"dir": dest_str, "out": "ariang.bin", "pause": "true"}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();

    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;
    let resp = rpc_call(&client, &rpc_url, "aria2.getOption", json!([gid])).await;
    assert_eq!(resp["result"]["out"], "ariang.bin", "{resp}");
    assert_eq!(resp["result"]["dir"], dest_str, "{resp}");

    let _ = rpc_call(&client, &rpc_url, "aria2.remove", json!([gid])).await;
    let _ = shutdown_tx.send(true);
}

// ── tellStatus schema ─────────────────────────────────────────────────────

/// The default `tellStatus` object must carry aria2's always-present keys, and
/// must expose the HTTP piece map once the download has started.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn interop_tell_status_http_field_schema() {
    // Must exceed 2× the 4 MiB chunk size for the engine to plan a piece map.
    let test_server = crate::test_harness::TestServer::new(16 * 1024 * 1024).await;
    // A range-capable endpoint, so the engine plans a piece map to report.
    let url = test_server.file_url_range_bandwidth(256 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        json!([[url], {"dir": dest.to_string_lossy(), "out": "schema.bin"}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();

    wait_for_status(&client, &rpc_url, &gid, &["active"]).await;
    // The piece map is planned once the range probe completes, which can lag
    // the first `active` observation, so poll instead of sleeping a fixed time.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let status = loop {
        let resp = rpc_call(&client, &rpc_url, "aria2.tellStatus", json!([gid])).await;
        if resp["result"]["numPieces"].is_string() {
            break resp["result"].clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "tellStatus never exposed the piece map: {resp}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    for key in [
        "gid",
        "status",
        "totalLength",
        "completedLength",
        "uploadLength",
        "downloadSpeed",
        "uploadSpeed",
        "connections",
        "dir",
        "files",
        "numPieces",
        "pieceLength",
        "bitfield",
    ] {
        assert!(
            status.get(key).is_some(),
            "tellStatus is missing {key}: {status}"
        );
    }
    // errorCode/errorMessage are only present for stopped downloads.
    assert!(status.get("errorCode").is_none(), "active download: {status}");
    assert_eq!(
        status["bitfield"].as_str().map(str::len),
        status["numPieces"]
            .as_str()
            .map(|n| n.parse::<usize>().unwrap().div_ceil(4)),
        "bitfield width must match numPieces: {status}"
    );
    assert_eq!(
        status["files"][0]["path"],
        dest.join("schema.bin").to_string_lossy().as_ref()
    );

    let _ = rpc_call(&client, &rpc_url, "aria2.remove", json!([gid])).await;
    let _ = shutdown_tx.send(true);
}

/// A BitTorrent `tellStatus` keeps `infoHash`/`numSeeders`/`seeder` at the top
/// level and a metadata `bittorrent` object, matching aria2.
#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn interop_tell_status_bt_field_schema() {
    let fixture = crate::bt_backend::tests::single_file_torrent_bytes();
    let expected_hash = irontide::core::torrent_from_bytes(&fixture)
        .expect("the shared BT fixture must parse")
        .info_hash
        .to_hex();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&fixture);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addTorrent",
        json!([encoded, [], {"dir": dest.to_string_lossy(), "pause": "true"}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addTorrent failed: {resp}"))
        .to_string();

    // Wait until the torrent metadata is reflected in tellStatus.files.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let status = loop {
        let resp = rpc_call(&client, &rpc_url, "aria2.tellStatus", json!([gid])).await;
        if resp["result"]["files"].as_array().is_some_and(|f| !f.is_empty()) {
            break resp["result"].clone();
        }
        assert!(std::time::Instant::now() < deadline, "no BT metadata: {resp}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };

    assert_eq!(
        status["infoHash"].as_str().map(str::to_ascii_lowercase),
        Some(expected_hash),
        "infoHash must be top-level: {status}"
    );
    assert_eq!(status["seeder"], "false", "{status}");
    assert!(status["numSeeders"].is_string(), "{status}");
    assert_eq!(status["bittorrent"]["mode"], "single", "{status}");
    assert!(
        status["bittorrent"]["info"]["name"].is_string(),
        "{status}"
    );
    assert!(status["files"].as_array().is_some_and(|f| f.len() == 1), "{status}");

    let _ = rpc_call(&client, &rpc_url, "aria2.remove", json!([gid])).await;
    let _ = shutdown_tx.send(true);
}

// ── getServers / changeUri / changePosition ───────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn interop_get_servers_and_change_uri() {
    let test_server = crate::test_harness::TestServer::new(512 * 1024).await;
    let primary = test_server.file_url_range();
    let mirror = format!("{}?mirror=1", test_server.file_url_range());

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest = tmp.path().join("output");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        json!([[primary, mirror], {"dir": dest.to_string_lossy(), "out": "servers.bin", "pause": "true"}]),
    )
    .await;
    let gid = resp["result"]
        .as_str()
        .unwrap_or_else(|| panic!("addUri failed: {resp}"))
        .to_string();
    wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;

    // getServers: one file index, one server per candidate.
    let resp = rpc_call(&client, &rpc_url, "aria2.getServers", json!([gid])).await;
    let entries = resp["result"].as_array().expect("getServers array");
    assert_eq!(entries.len(), 1, "{resp}");
    assert_eq!(entries[0]["index"], "1", "{resp}");
    let servers = entries[0]["servers"].as_array().expect("servers array");
    assert_eq!(servers.len(), 2, "{resp}");
    assert!(
        servers.iter().all(|server| server["uri"].is_string()
            && server["currentUri"].is_string()
            && server["downloadSpeed"].is_string()),
        "{resp}"
    );

    // changeUri: add a third candidate, then remove it, then refuse to empty
    // the list. aria2 answers `[deleted, added]`.
    let third = format!("{}?mirror=2", test_server.file_url_range());
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeUri",
        json!([gid, 1, [], [third]]),
    )
    .await;
    assert_eq!(resp["result"], json!([0, 1]), "{resp}");

    let resp = rpc_call(&client, &rpc_url, "aria2.getUris", json!([gid])).await;
    let uris: Vec<&str> = resp["result"]
        .as_array()
        .expect("getUris array")
        .iter()
        .filter_map(|entry| entry["uri"].as_str())
        .collect();
    assert!(uris.contains(&third.as_str()), "{resp}");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeUri",
        json!([gid, 1, [third], []]),
    )
    .await;
    assert_eq!(resp["result"], json!([1, 0]), "{resp}");

    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeUri",
        json!([gid, 1, [primary, mirror], []]),
    )
    .await;
    assert!(
        resp["error"].is_object(),
        "removing every URI must be refused: {resp}"
    );

    // A non-HTTP file index is invalid params, not a silent no-op.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changeUri",
        json!([gid, 2, [], [third]]),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602, "{resp}");

    let _ = rpc_call(&client, &rpc_url, "aria2.remove", json!([gid])).await;
    let _ = shutdown_tx.send(true);
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(90_000)]
async fn interop_change_position_returns_the_real_index() {
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let base = test_server.file_url_bandwidth(64 * 1024);

    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();
    let dest = tmp.path().join("output");

    let mut gids = Vec::new();
    for index in 0..3 {
        let url = format!("{base}?i={index}");
        let resp = rpc_call(
            &client,
            &rpc_url,
            "aria2.addUri",
            json!([[url], {"dir": dest.to_string_lossy(), "pause": "true"}]),
        )
        .await;
        let gid = resp["result"]
            .as_str()
            .unwrap_or_else(|| panic!("addUri failed: {resp}"))
            .to_string();
        wait_for_status(&client, &rpc_url, &gid, &["paused"]).await;
        gids.push(gid);
    }
    let (first, _middle, last) = (gids[0].clone(), gids[1].clone(), gids[2].clone());

    // Move the last-created task to the front; the answer is the real index.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changePosition",
        json!([last, 0, "POS_SET"]),
    )
    .await;
    assert_eq!(resp["result"], 0, "{resp}");
    let resp = rpc_call(&client, &rpc_url, "aria2.tellWaiting", json!([0, 100])).await;
    assert_eq!(index_in_waiting(&resp, &last), Some(0), "{resp}");

    // Move the first-created task to the end.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changePosition",
        json!([first, -1, "POS_END"]),
    )
    .await;
    assert_eq!(resp["result"], 2, "{resp}");
    let resp = rpc_call(&client, &rpc_url, "aria2.tellWaiting", json!([0, 100])).await;
    assert_eq!(index_in_waiting(&resp, &first), Some(2), "{resp}");

    // A bad `how` is invalid params, not a silent success.
    let resp = rpc_call(
        &client,
        &rpc_url,
        "aria2.changePosition",
        json!([last, 0, "POS_SIDEWAYS"]),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32602, "{resp}");

    let _ = shutdown_tx.send(true);
}

/// `aria2.forceShutdown` must be advertised and answered (limedl is a managed
/// subsystem, so it acknowledges instead of exiting), and `getVersion` must
/// carry the aria2 handshake keys.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn interop_shutdown_handshake() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(&client, &rpc_url, "aria2.forceShutdown", json!([])).await;
    assert!(resp["result"].is_string(), "{resp}");

    let resp = rpc_call(&client, &rpc_url, "aria2.getVersion", json!([])).await;
    assert!(resp["result"]["version"].is_string(), "{resp}");
    assert!(resp["result"]["enabledFeatures"].is_array(), "{resp}");

    let _ = shutdown_tx.send(true);
}

/// `getVersion` must describe limedl, not copy aria2's feature list. This is the
/// report the Tier 2 oracle compares against.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn interop_get_version_is_truthful() {
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server().await;
    let client = reqwest::Client::new();

    let resp = rpc_call(&client, &rpc_url, "aria2.getVersion", json!([])).await;
    assert_eq!(
        resp["result"]["version"],
        env!("CARGO_PKG_VERSION"),
        "{resp}"
    );
    let features: Vec<&str> = resp["result"]["enabledFeatures"]
        .as_array()
        .expect("enabledFeatures array")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();

    // Capabilities limedl does not have must not be advertised.
    for untrue in ["XML-RPC", "Firefox3 Cookie", "Metalink", "SFTP"] {
        assert!(
            !features.contains(&untrue),
            "getVersion must not advertise {untrue}: {resp}"
        );
    }
    for real in [
        "BitTorrent",
        "HTTPS",
        "Async DNS",
        "Message Digest",
        "GZip",
        "Brotli",
        "Zstd",
    ] {
        assert!(
            features.contains(&real),
            "getVersion is missing supported feature {real}: {resp}"
        );
    }

    let _ = shutdown_tx.send(true);
}
