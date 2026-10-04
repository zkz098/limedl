//! Tier 2 Mode B: a live `aria2c` oracle.
//!
//! These tests start a real `aria2c --enable-rpc` next to limedl's RPC server and
//! send both the same requests, then report how the two responses differ. They
//! exist because Tier 1 (`interop_tests.rs`) can only assert the shapes we wrote
//! down; it cannot catch "aria2 itself returns something else".
//!
//! The suite is **opt-in**: when `ARIA2_ORACLE_BIN` is unset every test returns
//! early, so the normal core gate never needs a third-party binary. The nightly
//! `check-aria2-oracle` CI job sets it. A set-but-unusable path panics instead of
//! skipping, so the job cannot pass by accident.
//!
//! Per the agreed rollout, gaps are **reported, not asserted**: the report is
//! printed (run nextest with `--success-output=final` to see it) and a later
//! milestone flips specific entries into hard failures. Infrastructure is still
//! asserted — both servers must answer with a `result` — so a transport
//! regression fails the job.
//!
//! Empirical baseline for the current CI image (aria2 1.37.0) is recorded in
//! `docs/aria2-interop-testing.md`; the allowlist consts below point there.

use std::collections::{BTreeMap, BTreeSet};

use ntest::timeout;
use serde_json::{Value, json};

use super::e2e_tests::{rpc_call, start_rpc_server_with_secret};

/// Secret shared by both servers, so the auth paths being compared are the same.
const SECRET: &str = "oracle";

/// Keys limedl intentionally omits from `tellStatus` in the states the oracle
/// exercises (a paused download before the range probe): limedl only reports the
/// piece map once chunks are planned, while aria2 always includes it. See
/// "Known, intentional deviations" in the runbook.
const TELL_STATUS_ALLOWED_MISSING: &[&str] = &["bitfield", "numPieces", "pieceLength"];

/// `getOption`/`getGlobalOption` are a fixed subset in limedl; their key sets are
/// reported but never asserted.
const GET_OPTION_KEYS_ALLOWED_MISSING: &[&str] = &[];

/// A live `aria2c --enable-rpc` child process.
struct Aria2Oracle {
    child: std::process::Child,
    url: String,
    /// Keeps `--dir` alive for the process lifetime.
    _dir: tempfile::TempDir,
}

impl Aria2Oracle {
    /// Spawn the oracle, or return `None` when `ARIA2_ORACLE_BIN` is unset.
    async fn start() -> Option<Self> {
        let bin = std::env::var("ARIA2_ORACLE_BIN").ok()?;

        // Reserve a free port, then release it for aria2c. The race window is
        // tiny and a genuine conflict surfaces as a readiness timeout.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe port");
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let dir = tempfile::TempDir::new().expect("aria2 oracle temp dir");
        let child = std::process::Command::new(&bin)
            .args([
                "--enable-rpc",
                &format!("--rpc-listen-port={port}"),
                &format!("--rpc-secret={SECRET}"),
                "--no-conf",
                "--quiet",
                "--enable-dht=false",
                "--bt-enable-lpd=false",
                "--max-concurrent-downloads=5",
                &format!("--dir={}", dir.path().display()),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap_or_else(|error| panic!("failed to spawn ARIA2_ORACLE_BIN={bin}: {error}"));

        let oracle = Self {
            child,
            url: format!("http://127.0.0.1:{port}/jsonrpc"),
            _dir: dir,
        };
        oracle.wait_ready().await;
        Some(oracle)
    }

    /// Poll `aria2.getVersion` until the RPC endpoint answers.
    async fn wait_ready(&self) {
        let client = oracle_client();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            let resp = rpc_call(
                &client,
                &self.url,
                "aria2.getVersion",
                json!([format!("token:{SECRET}")]),
            )
            .await;
            if resp["result"]["version"].is_string() {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "aria2c oracle never became ready: {resp}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Call a method with the shared token prepended, as an aria2 client does.
    async fn call(&self, method: &str, mut params: Vec<Value>) -> Value {
        params.insert(0, Value::String(format!("token:{SECRET}")));
        rpc_call(&oracle_client(), &self.url, method, Value::Array(params)).await
    }
}

impl Drop for Aria2Oracle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn oracle_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("oracle http client")
}

/// A request to limedl's server, with the shared token prepended.
async fn limedl_call(
    client: &reqwest::Client,
    url: &str,
    method: &str,
    mut params: Vec<Value>,
) -> Value {
    params.insert(0, Value::String(format!("token:{SECRET}")));
    rpc_call(client, url, method, Value::Array(params)).await
}

/// Collects the differences found across methods and prints them at the end.
#[derive(Default)]
struct GapReport {
    entries: Vec<String>,
}

impl GapReport {
    fn note(&mut self, message: impl Into<String>) {
        self.entries.push(message.into());
    }

    /// Report keys limedl lacks, excluding the documented allowlist.
    fn missing_keys(&mut self, method: &str, limedl: &Value, aria2: &Value, allowed: &[&str]) {
        let l = keys(limedl);
        let a = keys(aria2);
        for key in a.difference(&l) {
            let marker = if allowed.contains(&key.as_str()) {
                " (allowed)"
            } else {
                ""
            };
            self.note(format!("{method}: limedl missing `{key}`{marker}"));
        }
        for key in l.difference(&a) {
            self.note(format!("{method}: limedl extra `{key}`"));
        }
    }

    fn finish(&self) {
        if self.entries.is_empty() {
            eprintln!("\n=== aria2 oracle: no new gaps ===\n");
            return;
        }
        eprintln!(
            "\n=== aria2 oracle gaps ({} — report-only, not failing) ===\n{}\n",
            self.entries.len(),
            self.entries.join("\n")
        );
    }
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

/// `("string" | "array" | ...)` per key, for shape reporting.
fn value_kinds(value: &Value) -> BTreeMap<String, &'static str> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), kind(value)))
                .collect()
        })
        .unwrap_or_default()
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn set_of(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// ── Tests ──────────────────────────────────────────────────────────────────

/// `system.listMethods`: limedl must cover every aria2 method except the
/// documented Metalink gap, and `aria2.multicall` is a limedl-only alias.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_list_methods_diff() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_list_methods_diff: ARIA2_ORACLE_BIN unset");
        return;
    };
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let limedl = rpc_call(&client, &rpc_url, "system.listMethods", json!([])).await;
    let aria2 = oracle.call("system.listMethods", vec![]).await;
    assert!(limedl["result"].is_array(), "limedl listMethods: {limedl}");
    assert!(aria2["result"].is_array(), "aria2 listMethods: {aria2}");

    let mut report = GapReport::default();
    let l = set_of(&limedl["result"]);
    let a = set_of(&aria2["result"]);
    for method in a.difference(&l) {
        report.note(format!("limedl does not implement `{method}`"));
    }
    for method in l.difference(&a) {
        report.note(format!("limedl implements non-aria2 method `{method}`"));
    }
    report.finish();

    let _ = shutdown_tx.send(true);
}

/// `system.listNotifications` should be an exact match.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_list_notifications_diff() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_list_notifications_diff: ARIA2_ORACLE_BIN unset");
        return;
    };
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let limedl = rpc_call(&client, &rpc_url, "system.listNotifications", json!([])).await;
    let aria2 = oracle.call("system.listNotifications", vec![]).await;
    assert!(limedl["result"].is_array(), "limedl: {limedl}");
    assert!(aria2["result"].is_array(), "aria2: {aria2}");

    let mut report = GapReport::default();
    let l = set_of(&limedl["result"]);
    let a = set_of(&aria2["result"]);
    for method in a.difference(&l) {
        report.note(format!("limedl does not emit `{method}`"));
    }
    for method in l.difference(&a) {
        report.note(format!("limedl emits non-aria2 notification `{method}`"));
    }
    report.finish();

    let _ = shutdown_tx.send(true);
}

/// `getVersion`: both shapes must be `{version: string, enabledFeatures: [string]}`.
/// The feature lists differ (aria2 advertises Metalink/SFTP/XML-RPC) and are reported.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_get_version_shape() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_get_version_shape: ARIA2_ORACLE_BIN unset");
        return;
    };
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let limedl = rpc_call(&client, &rpc_url, "aria2.getVersion", json!(["token:oracle"])).await;
    let aria2 = oracle.call("aria2.getVersion", vec![]).await;
    for (label, resp) in [("limedl", &limedl), ("aria2", &aria2)] {
        assert!(
            resp["result"]["version"].is_string(),
            "{label} getVersion.version: {resp}"
        );
        assert!(
            resp["result"]["enabledFeatures"].is_array(),
            "{label} getVersion.enabledFeatures: {resp}"
        );
    }

    let mut report = GapReport::default();
    let l = set_of(&limedl["result"]["enabledFeatures"]);
    let a = set_of(&aria2["result"]["enabledFeatures"]);
    for feature in a.difference(&l) {
        report.note(format!("limedl getVersion does not advertise `{feature}`"));
    }
    for feature in l.difference(&a) {
        report.note(format!("limedl getVersion advertises non-aria2 `{feature}`"));
    }
    report.note(format!(
        "version strings: limedl={} aria2={}",
        limedl["result"]["version"], aria2["result"]["version"]
    ));
    report.finish();

    let _ = shutdown_tx.send(true);
}

/// Error responses: aria2 uses code 1 for everything; limedl uses JSON-RPC codes.
/// Only "both returned an error object" is asserted; the codes are reported.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_error_objects() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_error_objects: ARIA2_ORACLE_BIN unset");
        return;
    };
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let mut report = GapReport::default();
    // Each case: (method, params excluding the token, expected aria2 message shape).
    let cases: [(&str, Vec<Value>); 3] = [
        ("no.such.method", vec![]),
        ("aria2.tellStatus", vec![]),
        ("aria2.tellStatus", vec![json!("deadbeefdeadbeef")]),
    ];
    for (method, params) in cases {
        let limedl = limedl_call(&client, &rpc_url, method, params.clone()).await;
        let aria2 = oracle.call(method, params).await;
        assert!(
            limedl["error"].is_object(),
            "limedl must error for {method}: {limedl}"
        );
        assert!(
            aria2["error"].is_object(),
            "aria2 must error for {method}: {aria2}"
        );
        report.note(format!(
            "{method}: error codes limedl={} aria2={}",
            limedl["error"]["code"], aria2["error"]["code"]
        ));
    }
    report.finish();

    let _ = shutdown_tx.send(true);
}

/// Add the same paused HTTP download to both servers and compare the
/// `tellStatus` key sets.
#[tokio::test(flavor = "multi_thread")]
#[timeout(180_000)]
async fn oracle_tell_status_keys() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_tell_status_keys: ARIA2_ORACLE_BIN unset");
        return;
    };
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let url = test_server.file_url();
    let (rpc_url, shutdown_tx, tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();
    let dir = tmp.path().join("output");

    let a_gid = oracle
        .call(
            "aria2.addUri",
            vec![json!([url]), json!({"pause": "true"})],
        )
        .await["result"]
        .as_str()
        .expect("aria2 addUri gid")
        .to_string();
    let l_resp = limedl_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        vec![
            json!([url]),
            json!({"pause": "true", "dir": dir.to_string_lossy()}),
        ],
    )
    .await;
    let l_gid = l_resp["result"]
        .as_str()
        .expect("limedl addUri gid")
        .to_string();

    let aria2 = oracle.call("aria2.tellStatus", vec![json!(a_gid)]).await;
    let limedl = limedl_call(&client, &rpc_url, "aria2.tellStatus", vec![json!(l_gid)]).await;
    assert!(aria2["result"].is_object(), "aria2 tellStatus: {aria2}");
    assert!(limedl["result"].is_object(), "limedl tellStatus: {limedl}");

    let mut report = GapReport::default();
    report.missing_keys(
        "tellStatus",
        &limedl["result"],
        &aria2["result"],
        TELL_STATUS_ALLOWED_MISSING,
    );
    // Value types are part of the aria2 contract (everything is a string here).
    let limedl_kinds = value_kinds(&limedl["result"]);
    for (key, aria_kind) in value_kinds(&aria2["result"]) {
        match limedl_kinds.get(&key) {
            Some(limedl_kind) if *limedl_kind != aria_kind => report.note(format!(
                "tellStatus.{key}: type limedl={limedl_kind} aria2={aria_kind}"
            )),
            _ => {}
        }
    }
    report.finish();

    let _ = shutdown_tx.send(true);
}

/// `getOption` key set — reported only, limedl returns a fixed subset.
#[tokio::test(flavor = "multi_thread")]
#[timeout(180_000)]
async fn oracle_get_option_keys() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_get_option_keys: ARIA2_ORACLE_BIN unset");
        return;
    };
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let url = test_server.file_url();
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let a_gid = oracle
        .call("aria2.addUri", vec![json!([url]), json!({"pause": "true"})])
        .await["result"]
        .as_str()
        .expect("aria2 addUri gid")
        .to_string();
    let l_gid = limedl_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        vec![json!([url]), json!({"pause": "true"})],
    )
    .await["result"]
        .as_str()
        .expect("limedl addUri gid")
        .to_string();

    let aria2 = oracle.call("aria2.getOption", vec![json!(a_gid)]).await;
    let limedl = limedl_call(&client, &rpc_url, "aria2.getOption", vec![json!(l_gid)]).await;
    assert!(aria2["result"].is_object(), "aria2 getOption: {aria2}");
    assert!(limedl["result"].is_object(), "limedl getOption: {limedl}");

    let mut report = GapReport::default();
    report.missing_keys(
        "getOption",
        &limedl["result"],
        &aria2["result"],
        GET_OPTION_KEYS_ALLOWED_MISSING,
    );
    report.finish();

    let _ = shutdown_tx.send(true);
}

/// `getGlobalOption` key set — reported only.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_get_global_option_keys() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_get_global_option_keys: ARIA2_ORACLE_BIN unset");
        return;
    };
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let aria2 = oracle.call("aria2.getGlobalOption", vec![]).await;
    let limedl = limedl_call(&client, &rpc_url, "aria2.getGlobalOption", vec![]).await;
    assert!(aria2["result"].is_object(), "aria2 getGlobalOption: {aria2}");
    assert!(limedl["result"].is_object(), "limedl getGlobalOption: {limedl}");

    let mut report = GapReport::default();
    report.missing_keys(
        "getGlobalOption",
        &limedl["result"],
        &aria2["result"],
        GET_OPTION_KEYS_ALLOWED_MISSING,
    );
    report.finish();

    let _ = shutdown_tx.send(true);
}

/// `changePosition` must answer with an integer on both servers.
#[tokio::test(flavor = "multi_thread")]
#[timeout(180_000)]
async fn oracle_change_position_returns_integer() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_change_position_returns_integer: ARIA2_ORACLE_BIN unset");
        return;
    };
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let url = test_server.file_url();
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let a_gid = oracle
        .call("aria2.addUri", vec![json!([url]), json!({"pause": "true"})])
        .await["result"]
        .as_str()
        .expect("aria2 addUri gid")
        .to_string();
    let l_gid = limedl_call(
        &client,
        &rpc_url,
        "aria2.addUri",
        vec![json!([url]), json!({"pause": "true"})],
    )
    .await["result"]
        .as_str()
        .expect("limedl addUri gid")
        .to_string();

    let aria2 = oracle
        .call("aria2.changePosition", vec![json!(a_gid), json!(0), json!("POS_SET")])
        .await;
    let limedl = limedl_call(
        &client,
        &rpc_url,
        "aria2.changePosition",
        vec![json!(l_gid), json!(0), json!("POS_SET")],
    )
    .await;
    assert!(aria2["error"].is_null(), "aria2 changePosition: {aria2}");
    assert!(limedl["error"].is_null(), "limedl changePosition: {limedl}");
    assert!(aria2["result"].is_number(), "aria2 result must be an integer: {aria2}");
    assert!(
        limedl["result"].is_number(),
        "limedl result must be an integer: {limedl}"
    );

    let _ = shutdown_tx.send(true);
}

/// Transport-level gaps (aria2 serves HTTP GET/JSONP and JSON-RPC batch; limedl
/// serves POST + WebSocket). Reported, never asserted.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_transport_gaps() {
    let Some(oracle) = Aria2Oracle::start().await else {
        eprintln!("skipping oracle_transport_gaps: ARIA2_ORACLE_BIN unset");
        return;
    };
    let (rpc_url, shutdown_tx, _tmp, _core) = start_rpc_server_with_secret(Some(SECRET)).await;
    let client = oracle_client();

    let mut report = GapReport::default();

    // HTTP GET (base64-encoded params), as aria2 documents.
    let get_params = base64(&json!([format!("token:{SECRET}")]));
    for (label, url) in [("aria2", &oracle.url), ("limedl", &rpc_url)] {
        let get_url = format!("{url}?method=aria2.getVersion&id=1&params={get_params}");
        match client.get(&get_url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                if !parsed["result"].is_object() {
                    report.note(format!("{label} HTTP GET/JSONP unsupported (status {status})"));
                }
            }
            Err(error) => report.note(format!("{label} HTTP GET/JSONP request failed: {error}")),
        }
    }

    // JSON-RPC batch (a POST body that is a top-level array).
    let batch = serde_json::to_string(&json!([
        {"jsonrpc": "2.0", "id": 1, "method": "aria2.getVersion", "params": [format!("token:{SECRET}")]},
        {"jsonrpc": "2.0", "id": 2, "method": "aria2.tellActive", "params": [format!("token:{SECRET}")]},
    ]))
    .unwrap();
    for (label, url) in [("aria2", &oracle.url), ("limedl", &rpc_url)] {
        let text = match client
            .post(url)
            .header("Content-Type", "application/json")
            .body(batch.clone())
            .send()
            .await
        {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(error) => {
                report.note(format!("{label} batch POST request failed: {error}"));
                continue;
            }
        };
        let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if !parsed.is_array() {
            report.note(format!("{label} batch POST unsupported"));
        }
    }

    report.finish();
    let _ = shutdown_tx.send(true);
}

fn base64(value: &Value) -> String {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(value.to_string());
    percent_encoding::utf8_percent_encode(&encoded, percent_encoding::NON_ALPHANUMERIC).to_string()
}
