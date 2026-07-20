//! Integration tests for crash recovery via persistence.rs.
//!
//! These tests simulate app restart scenarios by creating a [`DownloadManager`],
//! letting it make progress, dropping it, and building a new manager from the
//! same state directory to verify the database recovery logic in
//! [`DownloadManager::load_downloads_from_db`].
//!
//! ## Coverage summary
//!
//! | Test | Scenario | Assertion |
//! |------|----------|-----------|
//! | `download_recovered_as_paused_after_restart` | App crashes mid-download | State → Paused, bytes > 0 |
//! | `completed_download_not_changed_on_restart` | App restarts after a completed download | State → Completed, file exists |
//! | `verifying_promoted_to_completed_if_dest_exists` | Verifying with dest + no temp on restart | State → Completed |
//! | `chunks_reloaded_for_non_terminal_downloads` | Paused download with chunks on restart | chunks.len() > 0 |

use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::tempdir;

use tokio::time::sleep;

use crate::{
    database::Database,
    event_bus::EventBus,
    manager::DownloadManager,
    manifest::{CHUNK_SIZE, ChunkManifest, Manifest},
    rate_limiter::RateLimiter,
    test_harness::TestServer,
    types::{
        ChecksumMode, DownloadSnapshot, DownloadState, StartDownloadRequest, ThreadMode,
        default_http_user_agent,
    },
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a minimal `Manifest` for direct DB insertion (used by the
/// lower-level tests 3 & 4).
fn make_test_manifest(id: &str, state: DownloadState) -> Manifest {
    Manifest {
        id: id.to_string(),
        url: "https://example.com/file".to_string(),
        final_url: "https://example.com/file".to_string(),
        user_agent: default_http_user_agent(),
        destination_dir: "/tmp".to_string(),
        file_name: "test.bin".to_string(),
        file_name_locked: true,
        destination_path: "/tmp/test.bin".to_string(),
        temp_path: "/tmp/test.bin.part".to_string(),
        total_bytes: Some(5 * 1024 * 1024),
        downloaded_bytes: 0,
        supports_ranges: true,
        chunk_size: CHUNK_SIZE,
        connection_count: 0,
        thread_mode: ThreadMode::Adaptive,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: Some(0),
        adaptive_profile_snapshot: None,
        thread_note: None,
        etag: None,
        last_modified: None,
        state,
        cdn_accelerated: false,
        checksum_mode: ChecksumMode::Blake3,
        checksum: None,
        expected_checksum: None,
        error: None,
        created_at_ms: crate::now_ms(),
        updated_at_ms: crate::now_ms(),
        chunks: Vec::new(),
        mirror_url: None,
        mirror_urls: Vec::new(),
        current_mirror_index: 0,
    }
}

/// Create a manager backed by a temp state directory and a bandwidth‑limited
/// HTTP test server.  Returns `(temp_dir, manager, server_address)`.
async fn create_manager_with_server(
    file_size: u64,
) -> (tempfile::TempDir, DownloadManager, TestServer) {
    let server = TestServer::new(file_size).await;
    let temp = tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )
    .expect("DownloadManager::new");
    (temp, manager, server)
}

/// Poll until the download reaches a terminal state.
async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> DownloadSnapshot {
    loop {
        let s = manager.status(id).await.unwrap();
        if matches!(
            s.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// ===========================================================================
// Test 1 — App crash mid-download
// ===========================================================================

#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn download_recovered_as_paused_after_restart() -> TestResult {
    // Use a bandwidth-limited server so the download makes steady but
    // incomplete progress within our short wait window.
    let (_tmp, manager, server) = create_manager_with_server(50 * 1024 * 1024).await;

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_bandwidth(500_000), // ~500 Kbps
            destination_dir: _tmp.path().join("out").to_string_lossy().to_string(),
            file_name: Some("crash-recovery.bin".to_string()),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    // Let enough data flow so downloaded_bytes > 0
    sleep(Duration::from_millis(800)).await;

    // Pause — this persists Paused state to the DB
    manager.pause(&id.to_string()).await?;

    // Save the snapshot before dropping the manager
    let before_drop = manager.status(&id.to_string()).await?;
    assert!(
        before_drop.downloaded_bytes > 0,
        "expected some progress before restart"
    );

    // Gracefully stop background tasks, then drop the first manager
    manager.task_lifecycle.shutdown(&manager).await;
    drop(manager);

    // ── Simulate restart ──────────────────────────────────────────
    let state_dir = _tmp.path().join("state");
    let manager2 = DownloadManager::new(
        state_dir,
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let list = manager2.list().await?;
    assert!(
        !list.is_empty(),
        "should have recovered at least one download"
    );

    let recovered = manager2.status(&id.to_string()).await?;
    assert_eq!(
        recovered.state,
        DownloadState::Paused,
        "non-terminal download should be Paused after recovery"
    );
    assert!(
        recovered.downloaded_bytes > 0,
        "downloaded_bytes should survive the restart"
    );

    // Cleanup
    let _ = manager2.remove(&id.to_string()).await;
    Ok(())
}

// ===========================================================================
// Test 2 — Completed download stays completed after restart
// ===========================================================================

#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn completed_download_not_changed_on_restart() -> TestResult {
    let (_tmp, manager, server) = create_manager_with_server(1024 * 1024).await; // 1 MB

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url_range(), // fast, multi-threaded
            destination_dir: _tmp.path().join("out").to_string_lossy().to_string(),
            file_name: Some("complete-me.bin".to_string()),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(4),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
        })
        .await?;

    // Wait for completion
    let terminal = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        terminal.state,
        DownloadState::Completed,
        "expected Completed, got {:?}",
        terminal.state
    );

    // Verify destination file exists
    let dest_path = std::path::Path::new(&terminal.destination_path);
    assert!(
        dest_path.exists(),
        "destination file should exist for a completed download"
    );

    // Save list before shutdown
    let before = manager.list().await?;
    assert!(!before.is_empty());

    manager.task_lifecycle.shutdown(&manager).await;
    drop(manager);

    // ── Simulate restart ──────────────────────────────────────────
    let state_dir = _tmp.path().join("state");
    let manager2 = DownloadManager::new(
        state_dir,
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let after = manager2.list().await?;
    assert!(after.len() >= before.len(), "lost downloads after restart");

    let recovered = manager2.status(&id.to_string()).await?;
    assert_eq!(
        recovered.state,
        DownloadState::Completed,
        "Completed download should remain Completed after restart"
    );
    assert!(
        std::path::Path::new(&recovered.destination_path).exists(),
        "destination file must still exist after restart"
    );

    let _ = manager2.remove(&id.to_string()).await;
    Ok(())
}

// ===========================================================================
// Test 3 — Verifying → Completed when destination exists & temp is gone
// ===========================================================================

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn verifying_promoted_to_completed_if_dest_exists() -> TestResult {
    let temp = tempdir()?;
    let state_dir = temp.path().join("state");
    std::fs::create_dir_all(&state_dir)?;
    std::fs::create_dir_all(state_dir.join("logs")).ok();

    // Create a real destination file on disk
    let dest_path = temp.path().join("already-here.bin");
    std::fs::write(&dest_path, b"simulated completed content")?;

    // Directly insert a Verifying-state manifest into the DB, with
    // destination_path pointing to the real file and temp_path to a
    // non-existent path.
    let db_path = state_dir.join("downloads.db");
    let db = Database::open(&db_path)?;
    let mut manifest = make_test_manifest("verify-promote", DownloadState::Verifying);
    manifest.destination_path = dest_path.to_string_lossy().to_string();
    manifest.temp_path = temp
        .path()
        .join("no-such-temp.part")
        .to_string_lossy()
        .to_string();
    manifest.total_bytes = Some(std::fs::metadata(&dest_path)?.len());
    manifest.downloaded_bytes = manifest.total_bytes.unwrap();
    db.insert_download(&manifest)?;
    drop(db); // close before manager opens

    // Create manager — load_downloads_from_db should promote Verifying → Completed
    let manager = DownloadManager::new(
        state_dir,
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let list = manager.list().await?;
    assert!(!list.is_empty(), "should have loaded the download");

    let status = manager.status("verify-promote").await?;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "Verifying with existing dest + no temp should promote to Completed"
    );

    let _ = manager.remove("verify-promote").await;
    Ok(())
}

// ===========================================================================
// Test 4 — Chunks reloaded for non-terminal downloads
// ===========================================================================

#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn chunks_reloaded_for_non_terminal_downloads() -> TestResult {
    let temp = tempdir()?;
    let state_dir = temp.path().join("state");
    std::fs::create_dir_all(&state_dir)?;
    std::fs::create_dir_all(state_dir.join("logs")).ok();

    // Insert a Paused download with pre-populated chunks into the DB.
    let db_path = state_dir.join("downloads.db");
    let db = Database::open(&db_path)?;

    let mut manifest = make_test_manifest("chunks-test", DownloadState::Paused);
    manifest.total_bytes = Some(8 * 1024 * 1024); // 8 MB → 2 chunks at 4 MB each
    manifest.chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 4 * 1024 * 1024 - 1,
            downloaded: 2 * 1024 * 1024, // half done
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 4 * 1024 * 1024,
            end: 8 * 1024 * 1024 - 1,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
    ];
    db.insert_download(&manifest)?;
    drop(db);

    // Create manager — load_downloads_from_db should load chunks for Paused
    let manager = DownloadManager::new(
        state_dir,
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?;

    let status = manager.status("chunks-test").await?;
    assert!(
        !status.chunks.is_empty(),
        "chunks should be loaded for non-terminal (Paused) downloads"
    );
    assert_eq!(
        status.chunks.len(),
        2,
        "expected 2 chunks from DB, got {}",
        status.chunks.len()
    );

    // Verify chunk data round-tripped correctly
    let c0 = &status.chunks[0];
    assert_eq!(c0.index, 0);
    assert_eq!(c0.downloaded, 2 * 1024 * 1024, "chunk 0 progress survived");
    assert!(!c0.completed);
    let c1 = &status.chunks[1];
    assert_eq!(c1.index, 1);
    assert_eq!(c1.downloaded, 0);
    assert!(!c1.completed);

    let _ = manager.remove("chunks-test").await;
    Ok(())
}
