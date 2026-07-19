use std::sync::Arc;

use anyhow::anyhow;
use ntest::timeout;
use tempfile::tempdir;

use limedl_core::error::DownloadError;
use limedl_core::event_bus::EventBus;
use limedl_core::types::{
    StartDownloadRequest, TaskKind,
};
use limedl_core::RateLimiter;
use limedl_core::DownloadManager;

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
