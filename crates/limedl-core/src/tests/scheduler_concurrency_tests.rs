use tempfile::TempDir;

use crate::types::{DownloadState, StartDownloadRequest, TaskId};

/// Start N downloads with max_parallel_tasks=2; verify only 2 run at a time.
#[tokio::test(flavor = "multi_thread")]
async fn scheduler_respects_max_parallel_tasks() {
    // Use a bandwidth-limited endpoint so downloads stay active long enough to observe
    let test_server = crate::test_harness::TestServer::new(256 * 1024).await;
    let url = test_server.file_url_bandwidth(16 * 1024); // 16 KB/s — very slow

    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    // Set mode to Traditional and max_parallel_tasks to 2
    {
        let mut settings = dm.settings().await.unwrap();
        settings.scheduler.mode = crate::types::SchedulerMode::Traditional;
        settings.scheduler.traditional.max_parallel_tasks = 2;
        dm.apply_settings(settings).await.unwrap();
    }

    // Start 5 downloads
    let mut ids: Vec<String> = Vec::new();
    for i in 0..5 {
        let request = StartDownloadRequest {
            url: url.clone(),
            destination_dir: dest_dir.to_string_lossy().to_string(),
            file_name: Some(format!("test_{i}.bin")),
            kind: None,
            thread_mode: None,
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            user_agent: None,
            priority: None,
        };
        let id = dm.start(request).await.unwrap();
        ids.push(id.to_string());
    }

    // Wait for scheduler to process — poll until we see the expected distribution
    let (downloading_count, queued_count) = loop {
        let list = dm.list().await.unwrap();
        let d: Vec<_> = list
            .iter()
            .filter(|s| s.state == DownloadState::Downloading)
            .collect();
        let q: Vec<_> = list
            .iter()
            .filter(|s| s.state == DownloadState::Queued)
            .collect();
        if d.len() == 2 && q.len() == 3 {
            break (d.len(), q.len());
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    };

    assert!(
        downloading_count <= 2,
        "Expected ≤2 downloading, got {}",
        downloading_count,
    );
    assert!(
        queued_count >= 3,
        "Expected ≥3 queued, got {}",
        queued_count,
    );

    // Cleanup: cancel all downloads
    for id in &ids {
        let task_id = TaskId::from_wire_string(id).unwrap();
        if let Some(inner) = task_id.as_http() {
            let _ = dm.cancel(&inner.to_string()).await;
        }
    }

    core.registry.shutdown_all().await;
}

/// Submitting many links (e.g. 50 links) at once must succeed for all tasks,
/// queuing any surplus beyond max_parallel_tasks rather than rejecting them.
#[tokio::test(flavor = "multi_thread")]
async fn batch_submitting_many_links_succeeds_and_queues() {
    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    // Set mode to Traditional with max_parallel_tasks=3
    {
        let mut settings = dm.settings().await.unwrap();
        settings.scheduler.mode = crate::types::SchedulerMode::Traditional;
        settings.scheduler.traditional.max_parallel_tasks = 3;
        dm.apply_settings(settings).await.unwrap();
    }

    // Start 50 downloads concurrently
    let mut tasks = Vec::new();
    for i in 0..50 {
        let dm = dm.clone();
        let dest_dir = dest_dir.to_string_lossy().to_string();
        tasks.push(tokio::spawn(async move {
            let request = StartDownloadRequest {
                url: format!("http://127.0.0.1:9/file_{i}.bin"),
                destination_dir: dest_dir,
                file_name: Some(format!("file_{i}.bin")),
                ..Default::default()
            };
            dm.start(request).await
        }));
    }

    let mut ids = Vec::new();
    for task in tasks {
        let res = task.await.unwrap();
        assert!(res.is_ok(), "task failed to start: {:?}", res.err());
        ids.push(res.unwrap());
    }
    assert_eq!(ids.len(), 50, "all 50 tasks must be accepted");

    let list = dm.list().await.unwrap();
    assert_eq!(list.len(), 50, "all 50 tasks must be present in manager");

    // Clean up
    for id in ids {
        let _ = dm.cancel(&id.to_string()).await;
    }

    core.registry.shutdown_all().await;
}
