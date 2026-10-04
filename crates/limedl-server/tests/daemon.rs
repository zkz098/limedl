//! End-to-end test for the daemon: boot a real `limedl-server`, drive it over
//! JSON-RPC, and confirm `aria2.shutdown` actually stops it.
//!
//! This is the one test that proves the *wiring* (bootstrap → forced-enabled RPC
//! → `exit_on_shutdown` → graceful engine shutdown), which the core's own
//! `aria2_rpc` tests do not cover because they never run the daemon.

use std::time::Duration;

use limedl_server::{Config, RpcOverrides, run_with};
use ntest::timeout;
use serde_json::{Value, json};

const SECRET: &str = "integration-secret";

async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve a port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Send one JSON-RPC request, tolerating the connection-refused state that is
/// expected until the daemon's engine has bootstrapped and bound its port.
/// `None` means "not ready yet", not "failed".
async fn rpc_try(client: &reqwest::Client, url: &str, method: &str, params: Value) -> Option<Value> {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    let text = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).ok()?)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    serde_json::from_str(&text).ok()
}

/// Send one JSON-RPC request once the daemon is known to be up.
async fn rpc(client: &reqwest::Client, url: &str, method: &str, params: Value) -> Value {
    rpc_try(client, url, method, params)
        .await
        .expect("RPC request must succeed once the daemon is ready")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[timeout(120_000)]
async fn daemon_serves_rpc_and_stops_on_aria2_shutdown() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let port = free_port().await;
    let url = format!("http://127.0.0.1:{port}/jsonrpc");

    let config = Config {
        data_dir: tmp.path().to_path_buf(),
        state_dir: tmp.path().join("downloads"),
        rpc: RpcOverrides {
            listen_host: Some("127.0.0.1".to_string()),
            port: Some(port),
            secret: Some(SECRET.to_string()),
            allow_any_origin: false,
            extra_origins: vec![],
        },
        download_dir: None,
        log_level: None,
    };

    // `pending()` is intentional: the only way this daemon may stop is the
    // JSON-RPC shutdown below, which is exactly what the test asserts.
    let daemon = tokio::spawn(run_with(config, std::future::pending::<()>()));
    let client = reqwest::Client::new();

    // Poll until the listener answers; the engine bootstrap is not instant.
    let mut ready = false;
    let mut last = String::new();
    for _ in 0..200 {
        // A refused connection just means the engine is still bootstrapping;
        // poll until it answers instead of failing on the first attempt.
        if let Some(response) = rpc_try(
            &client,
            &url,
            "aria2.getVersion",
            json!([format!("token:{SECRET}")]),
        )
        .await
        {
            if response["result"]["version"].is_string() {
                ready = true;
                break;
            }
            last = response.to_string();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(ready, "the daemon never answered getVersion: {last}");

    // The token is enforced: a missing one is Unauthorized.
    let unauthorized = rpc(&client, &url, "aria2.getVersion", json!([])).await;
    assert_eq!(
        unauthorized["error"]["message"], "Unauthorized",
        "the daemon must require the configured secret: {unauthorized}"
    );

    // `aria2.shutdown` is the daemon's stop signal (exit_on_shutdown is forced).
    let ack = rpc(
        &client,
        &url,
        "aria2.shutdown",
        json!([format!("token:{SECRET}")]),
    )
    .await;
    assert!(ack["result"].is_string(), "shutdown must acknowledge: {ack}");

    let joined = tokio::time::timeout(Duration::from_secs(30), daemon)
        .await
        .expect("the daemon must exit after aria2.shutdown");
    joined
        .expect("the daemon task must not panic")
        .expect("the daemon must exit cleanly");
}
