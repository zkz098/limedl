//! Checksum verification paths.

use super::*;

#[tokio::test]
#[timeout(30_000)]
async fn checksum_match_succeeds() -> TestResult {
    let payload = Arc::new(vec![42_u8; 64 * 1024]);
    let state = single_file_state("/file.bin", payload.clone(), "\"chk-good\"", 50);

    let app = Router::new()
        .route("/file.bin", get(file_get).head(file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Compute the expected good checksum for the payload
    let expected_good = crate::checksum::hash_slices(ChecksumMode::Blake3, &[&payload]);

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("chk-good.bin")),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(expected_good),
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    // Wait for terminal state
    let status = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "expected Completed with matching checksum, got {:?} error={:?}",
        status.state,
        status.error
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn checksum_mismatch_detected() -> TestResult {
    let payload = Arc::new(vec![42_u8; 64 * 1024]);
    let state = single_file_state("/file.bin", payload, "\"chk-bad\"", 50);

    let app = Router::new()
        .route("/file.bin", get(file_get).head(file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Wrong expected checksum — should cause mismatch
    let expected_bad =
        String::from("0000000000000000000000000000000000000000000000000000000000000000");

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("chk-bad.bin")),
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
            expected_checksum: Some(expected_bad),
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await?;

    // Wait for terminal state
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

    // Verify the temp file was NOT renamed to destination
    let dest_path = std::path::Path::new(&status.destination_path);
    assert!(
        !dest_path.exists(),
        "destination file should not exist on checksum mismatch"
    );

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}
