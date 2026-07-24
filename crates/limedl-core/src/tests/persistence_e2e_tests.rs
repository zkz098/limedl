use ntest::timeout;
use tempfile::TempDir;

use crate::database::Database;
use crate::types::{DownloadState, StartDownloadRequest, TaskId};

/// Verify that a download started before "crash" reappears after restart.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn download_survives_restart() {
    // Use a bandwidth-limited server so the download makes steady but
    // incomplete progress within our short wait window.
    let test_server = crate::test_harness::TestServer::new(50 * 1024 * 1024).await;
    let url = test_server.file_url_bandwidth(500_000); // ~500 Kbps

    let tmp = TempDir::new().unwrap();
    // state_dir is tmp/downloads — settings.json goes in tmp/ (parent)
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    // Phase 1: start a download, let it get some progress, then drop everything
    let download_id = {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;

        let request = StartDownloadRequest {
            url: url.clone(),
            destination_dir: dest_dir.to_string_lossy().to_string(),
            file_name: Some("test.bin".into()),
            kind: None,
            thread_mode: None,
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
            user_agent: None,
        priority: None,
    };
        let id = dm.start(request).await.unwrap();
        let task_id = TaskId::from_legacy_string(&id.to_string()).unwrap();
        let inner = match task_id {
            TaskId::Http(u) => u,
            TaskId::Bt(_) => unreachable!(),
        };

        // Wait briefly for some progress, then pause to get a stable snapshot
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if let Ok(s) = dm.status(&inner.to_string()).await
                && s.downloaded_bytes > 0
            {
                break;
            }
            if tokio::time::Instant::now() > deadline {
                panic!("timed out waiting for download progress");
            }
        }
        let _snapshot = dm.pause(&inner.to_string()).await.unwrap();

        // Drop core without calling shutdown_all() — simulates crash.
        // IMPORTANT: This test relies on CoreSystems::Drop NOT calling
        // shutdown_all(). If a Drop impl is added to CoreSystems that calls
        // shutdown, this test must be updated to drop the internals individually
        // instead. The test verifies crash recovery, not graceful shutdown.
        //
        // Verification: after drop, the database should still contain the
        // in-progress download in a non-terminal state (the persisted manifest
        // should show progress).
        drop(core);

        // Verify data was persisted before the "crash" — open a fresh DB
        // connection and assert the download exists with non-terminal progress.
        let db_path = state_dir.join("downloads.db");
        let db = Database::open(&db_path).unwrap();
        let persisted = db
            .get_download(&inner.to_string())
            .unwrap()
            .expect("Download must be persisted in DB after crash simulation");
        assert!(
            persisted.downloaded_bytes > 0
                || !matches!(
                    persisted.state,
                    DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
                ),
            "Download in DB should show non-terminal progress after crash \
             (downloaded_bytes={}, state={:?})",
            persisted.downloaded_bytes,
            persisted.state,
        );
        inner.to_string()
    };

    // Phase 2: re-bootstrap from the same state_dir
    {
        let core = crate::bootstrap::bootstrap(state_dir.clone())
            .await
            .unwrap();
        let dm = &core.download_manager;

        // The download should be in the list, in Paused state
        let list = dm.list().await.unwrap();
        let restored = list
            .iter()
            .find(|s| {
                let Ok(tid) = TaskId::from_legacy_string(&s.id) else {
                    return false;
                };
                match tid {
                    TaskId::Http(u) => u.to_string() == download_id,
                    _ => false,
                }
            })
            .expect("Download should survive restart and appear in list");

        // State must be Paused (in-flight states reset to Paused on restart)
        assert_eq!(
            restored.state,
            DownloadState::Paused,
            "Download should be Paused after restart, got {:?}",
            restored.state
        );

        // File info should be preserved
        assert_eq!(restored.url, url);
        assert!(
            restored.total_bytes.is_some(),
            "Total bytes should be known after first start"
        );

        core.registry.shutdown_all().await;
    }
}
