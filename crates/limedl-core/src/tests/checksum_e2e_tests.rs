use ntest::timeout;
use tempfile::TempDir;

use crate::types::{ChecksumMode, DownloadState, StartDownloadRequest, TaskId};

/// Download with correct Blake3 checksum → Completed
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn checksum_correct_completes() {
    let test_server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let url = test_server.file_url_range();

    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    let request = StartDownloadRequest {
        url: url.clone(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        kind: None,
        thread_mode: None,
        thread_count: Some(1),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::Blake3),
        expected_checksum: Some(test_server.blake3_hash.clone()),
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        user_agent: None,
    priority: None,
};
    let id = dm.start(request).await.unwrap();
    let task_id = TaskId::from_wire_string(&id.to_string()).unwrap();
    let inner = match task_id {
        TaskId::Http(u) => u,
        #[cfg(feature = "bt")]
        TaskId::Bt(_) => unreachable!(),
    };

    // Wait for download to complete (poll with timeout)
    let start = std::time::Instant::now();
    loop {
        let snapshot = dm.status(&inner.to_string()).await.unwrap();
        match snapshot.state {
            DownloadState::Completed => break,
            DownloadState::Failed => panic!("Download failed: {:?}", snapshot.error),
            _ => {}
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            panic!("Download timed out after 30s");
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    core.registry.shutdown_all().await;
}

/// Download with WRONG checksum → Failed
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn checksum_wrong_fails() {
    let test_server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let url = test_server.file_url_range();

    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    // Use a deliberately wrong checksum
    let wrong_hash = "0".repeat(64); // 64 zeros — not the real Blake3 hash

    let request = StartDownloadRequest {
        url: url.clone(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        kind: None,
        thread_mode: None,
        thread_count: Some(1),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::Blake3),
        expected_checksum: Some(wrong_hash),
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        user_agent: None,
    priority: None,
};
    let id = dm.start(request).await.unwrap();
    let task_id = TaskId::from_wire_string(&id.to_string()).unwrap();
    let inner = match task_id {
        TaskId::Http(u) => u,
        #[cfg(feature = "bt")]
        TaskId::Bt(_) => unreachable!(),
    };

    // Wait for download to fail due to checksum mismatch
    let start = std::time::Instant::now();
    loop {
        let snapshot = dm.status(&inner.to_string()).await.unwrap();
        match snapshot.state {
            DownloadState::Failed => {
                assert!(
                    snapshot.error.is_some(),
                    "Failed state must include error message"
                );
                break;
            }
            DownloadState::Completed => {
                panic!("Download should have failed due to wrong checksum");
            }
            _ => {}
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            panic!("Download timed out after 30s");
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    core.registry.shutdown_all().await;
}
