//! Contract tests for the [`DownloadBackend`] trait.
//!
//! Every backend implementation must pass these shared contract tests.
//! The contract test functions are backend-agnostic (take `&dyn DownloadBackend`)
//! so they can be applied to any backend.
//!
//! Currently tested:
//! - [`DownloadManager`] (HTTP) — full lifecycle
//! - [`IrontideBtBackend`] (BitTorrent) — offline smoke tests + #[ignore] network test

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use ntest::timeout;
use tempfile::TempDir;

use crate::{
    DownloadBackend, DownloadManager, EventBus, IrontideBtBackend, RateLimiter,
    types::{AppSettings, ChecksumMode, DownloadState, StartDownloadRequest, ThreadMode},
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// HTTP backend helpers
// ---------------------------------------------------------------------------

/// Create a [`DownloadManager`] wrapped in `Arc` with scheduler loop running.
fn make_manager(state_dir: &std::path::Path) -> Arc<DownloadManager> {
    let rate_limiter = Arc::new(RateLimiter::default());
    let event_bus = Arc::new(EventBus::new(1024));
    let dm = DownloadManager::new(state_dir.to_path_buf(), rate_limiter, event_bus)
        .expect("DownloadManager::new");
    let dm = Arc::new(dm);
    dm.scheduler.clone().start_scheduler_loop(dm.clone());
    dm
}

/// Build a standard HTTP [`StartDownloadRequest`] for test file downloads.
fn http_request(url: String, dest_dir: &std::path::Path) -> StartDownloadRequest {
    StartDownloadRequest {
        kind: None,
        url,
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test-file.bin".into()),
        user_agent: None,
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(1),
        max_retries: Some(0),
        checksum: Some(ChecksumMode::None),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
    }
}

// ---------------------------------------------------------------------------
// Contract test functions (backend-agnostic, dispatched via trait object)
// ---------------------------------------------------------------------------

/// **Contract**: `start()` returns a valid `TaskId`; `status()` returns a
/// snapshot whose `id` matches the task id.
async fn contract_start_and_status(backend: &dyn DownloadBackend, request: StartDownloadRequest) {
    let task_id = backend.start(request).await.expect("start should succeed");

    let snapshot = backend.status(&task_id).await.expect("status should succeed");
    assert_eq!(
        snapshot.id,
        task_id.raw_id(),
        "status snapshot id should match task id"
    );
}

/// **Contract**: `start()` → `pause()` → status shows `Paused` →
/// `resume()` → status shows `Downloading` or `Queued`.
async fn contract_pause_resume(backend: &dyn DownloadBackend, request: StartDownloadRequest) {
    let task_id = backend.start(request).await.expect("start should succeed");

    // Allow a brief window for the download to begin transferring data
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        if let Ok(s) = backend.status(&task_id).await
            && (s.state == DownloadState::Downloading || s.downloaded_bytes > 0)
        {
            break;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("timed out waiting for download to begin");
        }
    }

    // Pause
    let snapshot = backend.pause(&task_id).await.expect("pause should succeed");
    assert_eq!(
        snapshot.state,
        DownloadState::Paused,
        "after pause(), state should be Paused, got {:?}",
        snapshot.state,
    );

    // Status should also reflect Paused
    let status = backend
        .status(&task_id)
        .await
        .expect("status should succeed after pause");
    assert_eq!(
        status.state,
        DownloadState::Paused,
        "status should show Paused after pause, got {:?}",
        status.state,
    );

    // Resume
    let snapshot = backend.resume(&task_id).await.expect("resume should succeed");
    assert!(
        snapshot.state == DownloadState::Downloading
            || snapshot.state == DownloadState::Queued,
        "after resume(), state should be Downloading or Queued, got {:?}",
        snapshot.state,
    );

    // Clean up
    backend.cancel(&task_id).await.ok();
}

/// **Contract**: `start()` → `cancel()` returns snapshot with `Canceled` state.
async fn contract_cancel(backend: &dyn DownloadBackend, request: StartDownloadRequest) {
    let task_id = backend.start(request).await.expect("start should succeed");

    let snapshot = backend.cancel(&task_id).await.expect("cancel should succeed");
    assert_eq!(
        snapshot.state,
        DownloadState::Canceled,
        "cancel() should return snapshot with Canceled state, got {:?}",
        snapshot.state,
    );
}

/// **Contract**: After starting two downloads, `list()` returns both.
async fn contract_list(
    backend: &dyn DownloadBackend,
    request_a: StartDownloadRequest,
    request_b: StartDownloadRequest,
) {
    let id_a = backend.start(request_a).await.expect("start A should succeed");
    let id_b = backend.start(request_b).await.expect("start B should succeed");

    let list = backend.list().await.expect("list should succeed");
    assert_eq!(
        list.len(),
        2,
        "list should return 2 items, got {}",
        list.len()
    );

    let ids: Vec<String> = list.iter().map(|s| s.id.clone()).collect();
    assert!(
        ids.contains(&id_a.raw_id()),
        "list should contain first task id"
    );
    assert!(
        ids.contains(&id_b.raw_id()),
        "list should contain second task id"
    );

    // Clean up
    backend.cancel(&id_a).await.ok();
    backend.cancel(&id_b).await.ok();
}

/// **Contract**: `start()` → `remove()` directly removes and `list()` returns empty.
///
/// `remove()` is expected to internally cancel + clean up, so a separate
/// `cancel()` call is **not** required before `remove()`.
async fn contract_remove(backend: &dyn DownloadBackend, request: StartDownloadRequest) {
    let task_id = backend.start(request).await.expect("start should succeed");

    let _snapshot = backend.remove(&task_id).await.expect("remove should succeed");

    let list = backend.list().await.expect("list should succeed");
    assert!(
        list.is_empty(),
        "list should be empty after remove, got {} item(s)",
        list.len(),
    );
}

/// **Contract**: `start()` → `purge()` removes the task and `list()` returns empty.
///
/// `purge()` is expected to internally cancel + delete downloaded files,
/// so a separate `cancel()` call is **not** required before `purge()`.
async fn contract_purge(backend: &dyn DownloadBackend, request: StartDownloadRequest) {
    let task_id = backend.start(request).await.expect("start should succeed");

    let _snapshot = backend.purge(&task_id).await.expect("purge should succeed");

    let list = backend.list().await.expect("list should succeed");
    assert!(
        list.is_empty(),
        "list should be empty after purge, got {} item(s)",
        list.len(),
    );
}

// ===========================================================================
//  HTTP backend (DownloadManager) contract tests
// ===========================================================================

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn http_contract_start_and_status() -> TestResult {
    let server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let tmp = TempDir::new()?;
    let dest_dir = tmp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let dm = make_manager(&tmp.path().join("state"));
    let backend: &dyn DownloadBackend = &*dm;

    contract_start_and_status(backend, http_request(server.file_url_bandwidth(10 * 1024), &dest_dir)).await;

    dm.task_lifecycle.shutdown(&dm).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn http_contract_pause_resume() -> TestResult {
    let server = crate::test_harness::TestServer::new(5 * 1024 * 1024).await;
    let tmp = TempDir::new()?;
    let dest_dir = tmp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let dm = make_manager(&tmp.path().join("state"));
    let backend: &dyn DownloadBackend = &*dm;

    // 1 KB/s — keeps the download in-flight throughout the pause/resume steps
    contract_pause_resume(backend, http_request(server.file_url_bandwidth(1024), &dest_dir)).await;

    dm.task_lifecycle.shutdown(&dm).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn http_contract_cancel() -> TestResult {
    let server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let tmp = TempDir::new()?;
    let dest_dir = tmp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let dm = make_manager(&tmp.path().join("state"));
    let backend: &dyn DownloadBackend = &*dm;

    contract_cancel(backend, http_request(server.file_url_bandwidth(1024), &dest_dir)).await;

    dm.task_lifecycle.shutdown(&dm).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn http_contract_list() -> TestResult {
    let server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let tmp = TempDir::new()?;
    let dest_dir = tmp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let dm = make_manager(&tmp.path().join("state"));
    let backend: &dyn DownloadBackend = &*dm;

    contract_list(
        backend,
        http_request(server.file_url_bandwidth(1024), &dest_dir),
        http_request(server.file_url_bandwidth(2048), &dest_dir),
    )
    .await;

    dm.task_lifecycle.shutdown(&dm).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn http_contract_remove() -> TestResult {
    let server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let tmp = TempDir::new()?;
    let dest_dir = tmp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let dm = make_manager(&tmp.path().join("state"));
    let backend: &dyn DownloadBackend = &*dm;

    contract_remove(backend, http_request(server.file_url_bandwidth(1024), &dest_dir)).await;

    dm.task_lifecycle.shutdown(&dm).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn http_contract_purge() -> TestResult {
    let server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let tmp = TempDir::new()?;
    let dest_dir = tmp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let dm = make_manager(&tmp.path().join("state"));
    let backend: &dyn DownloadBackend = &*dm;

    contract_purge(backend, http_request(server.file_url_bandwidth(1024), &dest_dir)).await;

    dm.task_lifecycle.shutdown(&dm).await;
    Ok(())
}

// ===========================================================================
//  BT backend (IrontideBtBackend) smoke tests
// ===========================================================================

/// Create a minimal [`IrontideBtBackend`] with all network features disabled.
async fn make_bt_backend(tmp: &TempDir) -> (IrontideBtBackend, Arc<EventBus>) {
    let state_dir = tmp.path().join("bt_state");
    let out_dir = tmp.path().join("bt_out");
    std::fs::create_dir_all(&state_dir).expect("create bt_state dir");
    std::fs::create_dir_all(&out_dir).expect("create bt_out dir");

    let event_bus = Arc::new(EventBus::new(1024));
    let active_bt_count = Arc::new(AtomicUsize::new(0));
    let max_concurrent_bt = Arc::new(AtomicUsize::new(3));

    let mut settings = AppSettings::default();
    // Disable every network feature so the session starts without I/O
    settings.bt.dht_enabled = false;
    settings.bt.upnp_enabled = false;
    settings.bt.enable_natpmp = false;
    settings.bt.enable_ipv6 = false;
    settings.bt.enable_pex = false;
    settings.bt.enable_lsd = false;
    settings.bt.enable_utp = false;
    settings.bt.listen_port = Some(0); // OS-assigned ephemeral port

    let backend = IrontideBtBackend::new(
        &settings,
        state_dir,
        out_dir,
        event_bus.clone(),
        active_bt_count,
        max_concurrent_bt,
    )
    .await
    .expect("create BT backend");

    (backend, event_bus)
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_contract_list_empty() -> TestResult {
    let tmp = TempDir::new()?;
    let (backend, _event_bus) = make_bt_backend(&tmp).await;
    let backend: &dyn DownloadBackend = &backend;

    let list = backend.list().await.expect("list on empty backend");
    assert!(
        list.is_empty(),
        "list should be empty on fresh BT backend"
    );

    backend.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_contract_update_settings() -> TestResult {
    let tmp = TempDir::new()?;
    let (backend, _event_bus) = make_bt_backend(&tmp).await;
    let backend: &dyn DownloadBackend = &backend;

    let settings = AppSettings::default();
    backend
        .update_settings(&settings)
        .await
        .expect("update_settings should succeed");

    backend.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bt_contract_shutdown() -> TestResult {
    let tmp = TempDir::new()?;
    let (backend, _event_bus) = make_bt_backend(&tmp).await;
    let backend: &dyn DownloadBackend = &backend;

    // Shutdown must not panic
    backend.shutdown().await;
    Ok(())
}

/// Start a magnet download and cancel it.
///
/// The magnet does **not** need metadata resolution for cancel to work —
/// irontide accepts `remove_torrent` on pending magnets.  However the
/// test still takes ~3 s for the irontide alert bridge to settle.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn bt_contract_start_magnet_and_cancel() -> TestResult {
    let tmp = TempDir::new()?;
    let out_dir = tmp.path().join("bt_out");
    std::fs::create_dir_all(&out_dir)?;

    let (backend, _event_bus) = make_bt_backend(&tmp).await;
    let backend: &dyn DownloadBackend = &backend;

    // A well-known magnet with no trackers — metadata resolution will fail
    // quietly, but cancel should still work immediately.
    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d&dn=test".into(),
        destination_dir: out_dir.to_string_lossy().to_string(),
        file_name: None,
        kind: None,
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: Some(0),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
    };

    contract_cancel(backend, request).await;

    backend.shutdown().await;
    Ok(())
}

/// Start a magnet download and remove it directly (without prior cancel).
///
/// `remove()` is expected to internally cancel + clean up, so this tests
/// the combined cancel-and-remove path for the BT backend.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn bt_contract_start_magnet_and_remove() -> TestResult {
    let tmp = TempDir::new()?;
    let out_dir = tmp.path().join("bt_out");
    std::fs::create_dir_all(&out_dir)?;

    let (backend, _event_bus) = make_bt_backend(&tmp).await;
    let backend: &dyn DownloadBackend = &backend;

    let request = StartDownloadRequest {
        url: "magnet:?xt=urn:btih:da39a3ee5e6b4b0d3255bfef95601890afd80709&dn=remove-test".into(),
        destination_dir: out_dir.to_string_lossy().to_string(),
        file_name: None,
        kind: None,
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: Some(0),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
    };

    let task_id = backend.start(request).await.expect("start should succeed");

    // Remove directly (no separate cancel)
    let _snapshot = backend.remove(&task_id).await.expect("remove should succeed");

    // List should be empty
    let list = backend.list().await.expect("list should succeed");
    assert!(list.is_empty(), "list should be empty after remove, got {} item(s)", list.len());

    backend.shutdown().await;
    Ok(())
}
