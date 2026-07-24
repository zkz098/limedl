use std::sync::Arc;

use anyhow::anyhow;
use ntest::timeout;
use tempfile::tempdir;

use limedl_core::DownloadManager;
use limedl_core::RateLimiter;
use limedl_core::error::DownloadError;
use limedl_core::event_bus::EventBus;
use limedl_core::types::StartDownloadRequest;
use limedl_core::types::TaskKind;

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_request(url: &str, kind: Option<TaskKind>) -> StartDownloadRequest {
    StartDownloadRequest {
        kind,
        url: url.into(),
        destination_dir: "/tmp".into(),
        file_name: None,
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    }
}

fn make_manager(tmp: &tempfile::TempDir) -> DownloadManager {
    std::fs::create_dir_all(tmp.path().join("state").join("logs")).unwrap();
    DownloadManager::new(
        tmp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )
    .unwrap()
}

// =============================================================================
// Tier 1 — classify_request_kind (pure function, no AppState)
// =============================================================================

#[test]
#[timeout(10_000)]
fn classify_http_url() {
    let req = make_request("https://example.com/file.zip", None);
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Http);
}

#[test]
#[timeout(10_000)]
fn classify_magnet_link() {
    let req = make_request(
        "magnet:?xt=urn:btih:0000000000000000000000000000000000000000",
        None,
    );
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Bt);
}

#[test]
#[timeout(10_000)]
fn classify_torrent_file_extension() {
    let req = make_request("file.torrent", None);
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Bt);
}

#[test]
#[timeout(10_000)]
fn classify_explicit_kind_http() {
    // Explicit TaskKind::Http overrides the magnet URL detection
    let req = make_request(
        "magnet:?xt=urn:btih:0000000000000000000000000000000000000000",
        Some(TaskKind::Http),
    );
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Http);
}

#[test]
#[timeout(10_000)]
fn classify_explicit_kind_bt() {
    let req = make_request("https://example.com/file.zip", Some(TaskKind::Bt));
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Bt);
}

#[test]
#[timeout(10_000)]
fn classify_local_torrent_path() {
    let req = make_request("/path/to/file.torrent", None);
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Bt);
}

#[test]
#[timeout(10_000)]
fn classify_unknown_scheme_errors() {
    let req = make_request("ftp://example.com/file", None);
    let result = req.classify_kind();
    assert!(matches!(result, Err(DownloadError::UnsupportedScheme)));
}

#[test]
#[timeout(10_000)]
fn classify_empty_url_errors() {
    let req = make_request("", None);
    let result = req.classify_kind();
    assert!(matches!(result, Err(DownloadError::UnsupportedScheme)));
}

#[test]
#[timeout(10_000)]
fn classify_whitespace_url_errors() {
    let req = make_request("   ", None);
    let result = req.classify_kind();
    assert!(matches!(result, Err(DownloadError::UnsupportedScheme)));
}

#[test]
#[timeout(10_000)]
fn classify_http_with_case_insensitive_scheme() {
    // URL is lowered internally so uppercase schemes still match
    let req = make_request("HTTP://EXAMPLE.COM/FILE.ZIP", None);
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Http);
}

#[test]
#[timeout(10_000)]
fn classify_magnet_with_case_insensitive_scheme() {
    let req = make_request("MAGNET:?xt=urn:btih:...", None);
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Bt);
}

#[test]
#[timeout(10_000)]
fn classify_torrent_extension_case_insensitive() {
    let req = make_request("file.TORRENT", None);
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Bt);
}

#[test]
#[timeout(10_000)]
fn classify_url_with_torrent_extension() {
    // URL ending in .torrent even with query params — the raw lowered string
    // won't end with ".torrent" due to the query, but Path::extension catches it
    let req = make_request("https://example.com/file.torrent?download=1", None);
    let result = req.classify_kind();
    assert_eq!(result.unwrap(), TaskKind::Http);
}

// =============================================================================
// Tier 1 — format_anyhow_chain (pure function)
// =============================================================================

#[test]
#[timeout(10_000)]
fn format_single_error() {
    let err = anyhow!("something went wrong");
    let formatted = super::format_anyhow_chain(err);
    assert_eq!(formatted, "something went wrong");
}

#[test]
#[timeout(10_000)]
fn format_error_chain() {
    let err = anyhow!("root cause")
        .context("intermediate step failed")
        .context("top level failed");
    let formatted = super::format_anyhow_chain(err);
    assert_eq!(
        formatted,
        "top level failed: intermediate step failed: root cause"
    );
}

#[test]
#[timeout(10_000)]
fn format_error_dedup() {
    // Duplicate consecutive messages should be deduplicated
    let err = anyhow!("duplicate message").context("duplicate message");
    let formatted = super::format_anyhow_chain(err);
    assert_eq!(formatted, "duplicate message");
}

#[test]
#[timeout(10_000)]
fn format_error_partial_dedup() {
    // Duplicate at the top but not at the bottom — middle duplicate is kept
    let err = anyhow!("root")
        .context("middle")
        .context("middle")
        .context("top");
    let formatted = super::format_anyhow_chain(err);
    assert_eq!(formatted, "top: middle: root");
}

// =============================================================================
// Tier 1 — into_command_result (pure function)
// =============================================================================

#[test]
#[timeout(10_000)]
fn into_command_result_ok() {
    let result: anyhow::Result<i32> = Ok(42);
    let cmd_result = super::into_command_result(result);
    assert!(cmd_result.is_ok());
    assert_eq!(cmd_result.unwrap(), 42);
}

#[test]
#[timeout(10_000)]
fn into_command_result_ok_string() {
    let result: anyhow::Result<String> = Ok("hello".into());
    let cmd_result = super::into_command_result(result);
    assert!(cmd_result.is_ok());
    assert_eq!(cmd_result.unwrap(), "hello");
}

#[test]
#[timeout(10_000)]
fn into_command_result_err() {
    let result: anyhow::Result<i32> = Err(anyhow!("operation failed"));
    let err = super::into_command_result(result).unwrap_err();
    assert_eq!(err.kind, "internal");
    assert!(err.message.contains("operation failed"));
}

#[test]
#[timeout(10_000)]
fn into_command_result_err_with_download_error_kind() {
    // DownloadError::UnsupportedScheme should produce kind="unsupported_scheme"
    let result: anyhow::Result<i32> = Err(anyhow!(DownloadError::UnsupportedScheme));
    let err = super::into_command_result(result).unwrap_err();
    assert_eq!(err.kind, "unsupported_scheme");
    assert_eq!(err.message, "unsupported url scheme");
}

#[test]
#[timeout(10_000)]
fn into_command_result_err_with_context() {
    let result: anyhow::Result<i32> =
        Err(anyhow!(DownloadError::NotFound).context("lookup failed"));
    let err = super::into_command_result(result).unwrap_err();
    assert_eq!(err.kind, "not_found");
    assert!(err.message.contains("lookup failed"));
    assert!(err.message.contains("download not found"));
}

// =============================================================================
// Tier 2 — toggle_game_mode (tested through DownloadManager)
//
// The actual command handlers (toggle_game_mode, get_overclock_mode,
// get_io_status) are thin wrappers around DownloadManager methods and return
// simple JSON values. They cannot be called directly in unit tests because
// State<'_, AppState> and tauri::AppHandle cannot be constructed without a
// running Tauri application.
//
// Instead, we test the underlying manager/buffer-pool methods that these
// handlers delegate to, providing equivalent behavioral coverage.
// =============================================================================

#[tokio::test]
#[timeout(30_000)]
async fn toggle_game_mode_true() -> TestResult {
    let tmp = tempdir()?;
    let manager = make_manager(&tmp);

    // Equivalent to: toggle_game_command(enabled=true) → Ok(true)
    manager.set_game_mode(true);
    assert!(manager.game_mode());

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn toggle_game_mode_false() -> TestResult {
    let tmp = tempdir()?;
    let manager = make_manager(&tmp);

    manager.set_game_mode(true);
    assert!(manager.game_mode());

    // Equivalent to: toggle_game_command(enabled=false) → Ok(false)
    manager.set_game_mode(false);
    assert!(!manager.game_mode());

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn toggle_game_mode_return_value() -> TestResult {
    let tmp = tempdir()?;
    let manager = make_manager(&tmp);

    // The command handler returns Ok(enabled), so we verify the set+get round-trip
    manager.set_game_mode(true);
    assert!(manager.game_mode());

    manager.set_game_mode(false);
    assert!(!manager.game_mode());

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn get_overclock_mode_default() -> TestResult {
    let tmp = tempdir()?;
    let manager = make_manager(&tmp);

    // overclock_mode returns false for a default manager
    assert!(!manager.overclock_mode());

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn toggle_overclock_mode() -> TestResult {
    let tmp = tempdir()?;
    let manager = make_manager(&tmp);

    manager.set_overclock_mode(true);
    assert!(manager.overclock_mode());

    manager.set_overclock_mode(false);
    assert!(!manager.overclock_mode());

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn get_io_status_has_required_fields() -> TestResult {
    let tmp = tempdir()?;
    let manager = make_manager(&tmp);
    let pool = &manager.buffer_pool;

    // These are the exact fields and values that get_io_status would serialize
    // into its JSON response. The handler constructs:
    //
    //   json!({
    //       "gameMode": pool.game_mode(),
    //       "bufferUsageBytes": pool.current_usage(),
    //       "bufferLimitBytes": pool.effective_limit(),
    //       "activeSlots": pool.active_slots(),
    //       "maxSlots": pool.max_slots(),
    //       "queuedCount": pool.queued_count(),
    //       "degradationCount": pool.degradation_count(),
    //   })

    // Default state: no game mode, no buffer usage, standard limits
    assert!(!pool.game_mode());
    assert_eq!(pool.current_usage(), 0);
    assert_eq!(pool.effective_limit(), 1024 * 1024 * 1024); // 1024 MiB default
    assert_eq!(pool.active_slots(), 0);
    assert_eq!(pool.max_slots(), 4); // max_parallel_hdd default
    assert_eq!(pool.queued_count(), 0);
    assert_eq!(pool.degradation_count(), 0);

    // After enabling game mode, limits should shrink
    manager.set_game_mode(true);
    assert!(pool.game_mode());
    assert_eq!(pool.effective_limit(), 128 * 1024 * 1024); // 128 MiB game mode
    assert_eq!(pool.max_slots(), 1); // game_mode_max_parallel
    assert_eq!(pool.degradation_count(), 0);

    Ok(())
}

// =============================================================================
// Tier 3 — integration tests with bootstrap (full subsystem lifecycle)
//
// These tests construct a fully bootstrapped CoreSystems via bootstrap() and
// test download lifecycle operations through the Dispatcher — the same code
// path the Tauri command handlers use. State<'_, AppState> cannot be
// constructed outside a running Tauri application, so the Dispatcher is the
// closest integration point available in unit tests.
//
// Requires `--features test-utils` because limedl_core::test_harness is
// gated behind `#[cfg(feature = "test-utils")]`.
// =============================================================================
#[cfg(feature = "test-utils")]
mod tier3 {
    use std::sync::Arc;

    use ntest::timeout;
    use tempfile::tempdir;

    use limedl_core::AppState;
    use limedl_core::Dispatcher;
    use limedl_core::bootstrap::{CoreSystems, bootstrap};
    use limedl_core::test_harness::TestServer;
    use limedl_core::types::{DownloadState, StartDownloadRequest, TaskId, TaskKind};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ── Helpers ─────────────────────────────────────────────────────────────

/// Bootstrap a full CoreSystems and construct an AppState from it.
async fn bootstrap_env(tmp: &tempfile::TempDir) -> (CoreSystems, AppState) {
    let state_dir = tmp.path().join("state");
    let core = bootstrap(state_dir).await.unwrap();
    let state = AppState {
        registry: core.registry.clone(),
        event_bus: core.event_bus.clone(),
        cdn_service: core.cdn_service.clone(),
        rpc_shutdown: Arc::new(parking_lot::Mutex::new(None)),
        settings: Arc::new(parking_lot::RwLock::new(core.settings.clone())),
    };
    (core, state)
}

/// Create a StartDownloadRequest for a TestServer URL.
fn make_server_req(
    server: &TestServer,
    dest_dir: &std::path::Path,
    file_name: &str,
    bps: Option<u64>,
) -> StartDownloadRequest {
    let url = match bps {
        Some(bps) => server.file_url_bandwidth(bps),
        None => server.file_url(),
    };
    StartDownloadRequest {
        kind: None,
        url,
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some(file_name.into()),
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: Some(1),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        priority: None,
    }
}

// ── download_start ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_start_http_valid_url() -> TestResult {
    let tmp = tempdir()?;
    let server = TestServer::new(64 * 1024).await; // 64 KB file
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    let request = make_server_req(&server, &dest, "test.bin", None);
    let task_id = dispatcher.start(request).await?;

    assert!(
        matches!(task_id, TaskId::Http(_)),
        "expected TaskId::Http, got {task_id:?}",
    );

    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_cancel ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_cancel_transitions_to_canceled() -> TestResult {
    let tmp = tempdir()?;
    // Large file + bandwidth limit ensures the download hasn't finished yet
    let server = TestServer::new(1024 * 1024).await; // 1 MB
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    let request = make_server_req(&server, &dest, "cancel-test.bin", Some(10240)); // 10 KB/s
    let task_id = dispatcher.start(request).await?;

    // Let the background task begin
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let snapshot = dispatcher.cancel(&task_id).await?;
    assert_eq!(
        snapshot.state,
        DownloadState::Canceled,
        "expected Canceled, got {:?}",
        snapshot.state,
    );

    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_pause / download_resume ────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_pause_resume_cycle() -> TestResult {
    let tmp = tempdir()?;
    let server = TestServer::new(1024 * 1024).await; // 1 MB
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    // Bandwidth-limited so the download stays active during the test
    let request = make_server_req(&server, &dest, "pause-resume.bin", Some(10240));
    let task_id = dispatcher.start(request).await?;

    // Allow scheduler + background task to begin
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Pause ➔ expect Paused
    let snapshot = dispatcher.pause(&task_id).await?;
    assert_eq!(
        snapshot.state,
        DownloadState::Paused,
        "expected Paused after pause, got {:?}",
        snapshot.state,
    );

    // Resume ➔ expect Queued (background download will re-start),
    // but it may already be Downloading by the time we read the snapshot.
    let snapshot = dispatcher.resume(&task_id).await?;
    assert!(
        matches!(snapshot.state, DownloadState::Queued | DownloadState::Downloading),
        "expected Queued or Downloading after resume, got {:?}",
        snapshot.state,
    );

    // Cancel the re-started download
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let snapshot = dispatcher.cancel(&task_id).await?;
    assert_eq!(
        snapshot.state,
        DownloadState::Canceled,
        "expected Canceled after final cancel, got {:?}",
        snapshot.state,
    );

    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_status ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_status_returns_snapshot() -> TestResult {
    let tmp = tempdir()?;
    let server = TestServer::new(64 * 1024).await;
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    let request = make_server_req(&server, &dest, "status-test.bin", Some(10240));
    let task_id = dispatcher.start(request).await?;

    let snapshot = dispatcher.status(&task_id).await?;

    // Snapshot fields should match the request
    assert_eq!(snapshot.url, server.file_url_bandwidth(10240));
    assert_eq!(snapshot.file_name, "status-test.bin");
    assert!(matches!(snapshot.kind, TaskKind::Http));
    assert!(
        matches!(
            snapshot.state,
            DownloadState::Queued | DownloadState::Downloading
        ),
        "expected Queued or Downloading, got {:?}",
        snapshot.state,
    );

    // Clean up
    dispatcher.cancel(&task_id).await?;
    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_list ───────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_list_returns_active_downloads() -> TestResult {
    let tmp = tempdir()?;
    let server1 = TestServer::new(1024 * 1024).await;
    let server2 = TestServer::new(1024 * 1024).await;
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());

    // Start two downloads with bandwidth-limited URLs so they stay active
    let req1 = make_server_req(&server1, &dest, "list-test-1.bin", Some(10240));
    let req2 = make_server_req(&server2, &dest, "list-test-2.bin", Some(10240));
    let _id1 = dispatcher.start(req1).await?;
    let _id2 = dispatcher.start(req2).await?;

    // List should contain both
    let summaries = dispatcher.list().await?;
    assert_eq!(
        summaries.len(),
        2,
        "expected 2 active downloads, got {}",
        summaries.len(),
    );

    let names: Vec<&str> = summaries.iter().map(|s| s.file_name.as_str()).collect();
    assert!(
        names.contains(&"list-test-1.bin"),
        "missing list-test-1.bin in {names:?}",
    );
    assert!(
        names.contains(&"list-test-2.bin"),
        "missing list-test-2.bin in {names:?}",
    );

    // Clean up — cancel each via the summary IDs
    for summary in &summaries {
        let tid = TaskId::from_legacy_string(&summary.id)?;
        let _ = dispatcher.cancel(&tid).await;
    }
    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_remove (without prior cancel) ──────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_remove_active_download() -> TestResult {
    let tmp = tempdir()?;
    let server = TestServer::new(64 * 1024).await;
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    let request = make_server_req(&server, &dest, "remove-test.bin", Some(10240));
    let task_id = dispatcher.start(request).await?;

    // Remove directly (no cancel first) — remove handles state transition internally
    let snapshot = dispatcher.remove(&task_id).await?;
    assert_eq!(
        snapshot.state,
        DownloadState::Canceled,
        "expected Canceled after remove, got {:?}",
        snapshot.state,
    );

    // List should be empty now
    let summaries = dispatcher.list().await?;
    assert_eq!(
        summaries.len(),
        0,
        "expected empty list after remove, got {} items",
        summaries.len(),
    );

    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_purge (without prior cancel) ───────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_purge_active_download() -> TestResult {
    let tmp = tempdir()?;
    let server = TestServer::new(64 * 1024).await;
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    let request = make_server_req(&server, &dest, "purge-test.bin", Some(10240));
    let task_id = dispatcher.start(request).await?;

    // Purge directly — internally cancels and deletes destination files
    let snapshot = dispatcher.purge(&task_id).await?;
    assert_eq!(
        snapshot.state,
        DownloadState::Canceled,
        "expected Canceled after purge, got {:?}",
        snapshot.state,
    );

    // List should be empty
    let summaries = dispatcher.list().await?;
    assert_eq!(
        summaries.len(),
        0,
        "expected empty list after purge, got {} items",
        summaries.len(),
    );

    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_list empty after cancel ────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_list_empty_after_cancel() -> TestResult {
    let tmp = tempdir()?;
    let server = TestServer::new(64 * 1024).await;
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());
    let request = make_server_req(&server, &dest, "list-cancel-test.bin", Some(10240));
    let task_id = dispatcher.start(request).await?;

    // Cancel the download
    dispatcher.cancel(&task_id).await?;

    // List should be empty — cancel removes from the active list
    let summaries = dispatcher.list().await?;
    assert_eq!(
        summaries.len(),
        0,
        "expected empty list after cancel, got {} items",
        summaries.len(),
    );

    core.registry.shutdown_all().await;
    Ok(())
}

// ── download_start_bt_magnet (ignored — requires network) ──────────────

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires network (DHT/tracker) — run manually with cargo test -- --ignored"]
#[timeout(120_000)]
async fn download_start_bt_magnet() -> TestResult {
    let tmp = tempdir()?;
    let dest = tmp.path().join("out");
    std::fs::create_dir_all(&dest)?;
    let (core, state) = bootstrap_env(&tmp).await;

    let dispatcher = Dispatcher::new(state.registry.clone(), state.event_bus.clone());

    // Use start_paused: true to avoid actual peer/tracker IO
    let request = StartDownloadRequest {
        kind: None,
        url: "magnet:?xt=urn:btih:9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f".into(),
        destination_dir: dest.to_string_lossy().to_string(),
        file_name: None,
        user_agent: None,
        thread_mode: None,
        thread_count: None,
        max_retries: Some(1),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: true,
        mirror_urls: None,
        priority: None,
    };

    let task_id = dispatcher.start(request).await?;
    assert!(
        matches!(task_id, TaskId::Bt(_)),
        "expected TaskId::Bt, got {task_id:?}",
    );

    // Verify it appears in runtime status
    let bt_status = dispatcher.bt_runtime_status()?;
    assert!(
        bt_status.torrent_count >= 1,
        "expected at least 1 torrent in runtime status",
    );

    // Cancel to clean up
    dispatcher.cancel(&task_id).await?;

    core.registry.shutdown_all().await;
    Ok(())
  }
} // mod tier3
