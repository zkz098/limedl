use std::sync::Arc;

use ntest::timeout;
use tempfile::TempDir;

use crate::DownloadManager;
use crate::EventBus;
use crate::RateLimiter;
use crate::bootstrap::bootstrap;
use crate::types::{StartDownloadRequest, TaskId};

// ---------------------------------------------------------------------------
// Test 1 – bootstrap creates all subsystems
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bootstrap_smoke() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");

    let core = bootstrap(state_dir.clone()).await.unwrap();

    // All fields must be populated
    assert!(core.download_manager.downloads.read().await.is_empty());
    // BT backend should report a valid runtime status (even with 0 torrents)
    let bt_status = core.bt_backend.runtime_status();
    assert_eq!(bt_status.torrent_count, 0);
    // EventBus should accept subscribers
    let _rx = core.event_bus.subscribe();
    // RateLimiter defaults to capacity 0
    assert_eq!(core.rate_limiter.capacity(), 0);

    // BackendRegistry should have HTTP and BT backends
    let list = core.registry.list_all().await;
    assert!(list.is_empty()); // no downloads yet

    // Clean shutdown
    core.registry.shutdown_all().await;
}

// ---------------------------------------------------------------------------
// Test 2 – bootstrap auto-creates directories
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn bootstrap_creates_state_dir() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("nonexistent").join("downloads");
    assert!(!state_dir.exists());

    let core = bootstrap(state_dir.clone()).await.unwrap();
    assert!(state_dir.exists());
    assert!(state_dir.join("torrents").exists());
    assert!(state_dir.join("bt_files").exists());

    core.registry.shutdown_all().await;
}

// ---------------------------------------------------------------------------
// Helper: create a download manager directly (no BT, no registry).
// Used by the find_active_by_url tests.
// ---------------------------------------------------------------------------
fn make_manager(state_dir: &std::path::Path) -> Arc<DownloadManager> {
    let rate_limiter = Arc::new(RateLimiter::default());
    let event_bus = Arc::new(EventBus::new(1024));
    let dm = DownloadManager::new(state_dir.to_path_buf(), rate_limiter, event_bus)
        .expect("DownloadManager::new");
    let dm = Arc::new(dm);
    dm.scheduler.clone().start_scheduler_loop(dm.clone());
    dm
}

// ---------------------------------------------------------------------------
// Test 3 – find_active_by_url: same URL returns existing ID
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(30_000)]
async fn find_active_by_url_dedup() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let dm = make_manager(&state_dir);

    let url = "https://example.com/test-file.bin";

    // First check: no active download exists
    assert!(dm.find_active_by_url(url).await.is_none());

    // Create a download
    let request = StartDownloadRequest {
        url: url.to_string(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test-file.bin".into()),
        kind: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: true,
        mirror_urls: None,
        user_agent: None,
    };
    let id = dm.start(request).await.unwrap();
    let task_id = TaskId::from_legacy_string(&id.to_string()).unwrap();
    let uuid = match task_id {
        TaskId::Http(u) => u,
        TaskId::Bt(_) => unreachable!(),
    };

    // Second check: now it should find the active download
    let found = dm.find_active_by_url(url).await;
    assert!(found.is_some());
    assert_eq!(found.unwrap(), uuid.to_string());

    // Cancel the download (makes it terminal)
    dm.cancel(&uuid.to_string()).await.unwrap();

    // Third check: terminal downloads should NOT be found
    assert!(dm.find_active_by_url(url).await.is_none());

    dm.task_lifecycle.shutdown(&dm).await;
}

// ---------------------------------------------------------------------------
// Test 4 – find_active_by_url: different URLs are independent
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(30_000)]
async fn find_active_by_url_different_urls() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let dm = make_manager(&state_dir);

    let request1 = StartDownloadRequest {
        url: "https://example.com/file1.bin".into(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("file1.bin".into()),
        kind: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: true,
        mirror_urls: None,
        user_agent: None,
    };
    let id1 = dm.start(request1).await.unwrap();

    let request2 = StartDownloadRequest {
        url: "https://example.com/file2.bin".into(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("file2.bin".into()),
        kind: None,
        thread_mode: None,
        thread_count: None,
        max_retries: None,
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: true,
        mirror_urls: None,
        user_agent: None,
    };
    let id2 = dm.start(request2).await.unwrap();

    // Each URL should find its own download
    let found1 = dm
        .find_active_by_url("https://example.com/file1.bin")
        .await
        .unwrap();
    let found2 = dm
        .find_active_by_url("https://example.com/file2.bin")
        .await
        .unwrap();
    assert_ne!(found1, found2);

    // Cleanup
    let tid1 = TaskId::from_legacy_string(&id1.to_string()).unwrap();
    let tid2 = TaskId::from_legacy_string(&id2.to_string()).unwrap();
    let TaskId::Http(uuid1) = tid1 else {
        unreachable!()
    };
    let TaskId::Http(uuid2) = tid2 else {
        unreachable!()
    };
    dm.cancel(&uuid1.to_string()).await.unwrap();
    dm.cancel(&uuid2.to_string()).await.unwrap();

    dm.task_lifecycle.shutdown(&dm).await;
}
