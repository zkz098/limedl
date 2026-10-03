//! Startup behaviour.

use super::*;

#[tokio::test]
#[timeout(30_000)]
async fn start_returns_before_http_probe_finishes() -> TestResult {
    let payload = Arc::new(vec![19_u8; 1024 * 1024]);
    let state = single_file_state("/slow.bin", payload, "\"slow-start\"", 800);

    let app = Router::new()
        .route("/slow.bin", get(file_get).head(delayed_file_head))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("[limedl:test] server stopped: {error}");
        }
    });

    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);
    let id = tokio::time::timeout(
        Duration::from_millis(5_000),
        manager.start(StartDownloadRequest {
            kind: None,
            url: format!("http://{address}/slow.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Adaptive),
            thread_count: None,
            max_retries: Some(1),
            checksum: Some(ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        }),
    )
    .await??;

    let initial = manager.status(&id.to_string()).await?;
    assert_eq!(initial.file_name, "slow.bin");
    assert!(matches!(
        initial.state,
        DownloadState::Queued | DownloadState::Downloading
    ));

    for _ in 0..30 {
        let status = manager.status(&id.to_string()).await?;
        if status.file_name == "server-name.bin" {
            let _ = manager.remove(&id.to_string()).await;
            return Ok(());
        }
        sleep(Duration::from_millis(100)).await;
    }

    let status = manager.status(&id.to_string()).await?;
    assert_eq!(status.file_name, "server-name.bin");
    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}
