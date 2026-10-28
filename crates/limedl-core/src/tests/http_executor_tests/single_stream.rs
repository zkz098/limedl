//! Single-stream download paths.

use super::*;

#[tokio::test]
async fn single_stream_download_completes_successfully() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "expected Completed, got {:?} with error={:?}",
        status.state,
        status.error
    );
    assert_eq!(status.total_bytes, Some(server.file_size));
    assert_eq!(status.downloaded_bytes, server.file_size);

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(
        dest_path.exists(),
        "destination file should exist at {}",
        status.destination_path
    );
    let downloaded = tokio::fs::read(dest_path).await?;
    assert_eq!(downloaded.len() as u64, server.file_size);

    let expected = generate_test_content(server.file_size);
    assert_eq!(
        downloaded, expected,
        "downloaded file content does not match server data"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
async fn single_stream_download_with_blake3_checksum_match() -> TestResult {
    let server = TestServer::new(32 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(server.blake3_hash.clone()),
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "expected Completed with matching Blake3 checksum, got {:?} error={:?}",
        status.state,
        status.error
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
async fn single_stream_checksum_mismatch_fails() -> TestResult {
    let server = TestServer::new(16 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let wrong_checksum =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: server.file_url(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(wrong_checksum),
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Failed,
        "expected Failed on checksum mismatch, got {:?}",
        status.state
    );
    let error_msg = status.error.unwrap_or_default();
    assert!(
        error_msg.contains("Checksum mismatch"),
        "error should contain 'Checksum mismatch', got: {error_msg}"
    );

    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(
        !dest_path.exists(),
        "destination file should not exist on checksum mismatch"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
async fn single_stream_slow_link_emits_progress_before_filling_micro_batch() -> TestResult {
    let server = TestServer::new(256 * 1024).await;
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // TestServer serves throttled endpoints in 64 KiB chunks with delay = 64 KiB / bps.
    // At bps = 64 KiB/s, the first 64 KiB arrives after ~1.0s.
    // Without time-based micro-batch flushing, the client buffers this 64 KiB and does NOT
    // report progress or speed until the second chunk arrives to reach 128 KiB.
    // With MICRO_BATCH_MAX_INTERVAL, the 64 KiB is flushed and credited immediately.
    let slow_url = server.file_url_bandwidth(64 * 1024);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: slow_url,
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    let start_wait = std::time::Instant::now();
    let mut progress_seen = false;
    let mut speed_seen = false;
    while start_wait.elapsed() < Duration::from_secs(5) {
        let s = manager.status(&id.to_string()).await?;
        if s.downloaded_bytes > 0 && s.downloaded_bytes < 128 * 1024 {
            progress_seen = true;
        }
        if let Some(spd) = s.speed_bytes_per_second
            && spd > 0.0
        {
            speed_seen = true;
        }
        if progress_seen && speed_seen {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }

    assert!(
        progress_seen,
        "single stream progress should be recorded well before 128 KiB micro-batch is full"
    );
    assert!(
        speed_seen,
        "single stream speed should be positive on slow link before 128 KiB is reached"
    );

    let _ = manager.cancel(&id.to_string()).await;
    Ok(())
}
