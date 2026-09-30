//! `start()` request validation.

use super::*;

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_unsupported_scheme() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "ftp://example.com/file.bin".into(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await;

    assert!(matches!(result, Err(DownloadError::UnsupportedScheme)));
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_empty_destination_dir() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "http://example.com/file.bin".into(),
            destination_dir: String::new(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await;

    assert!(
        matches!(result, Err(DownloadError::InvalidResponse(ref msg)) if msg.contains("not set"))
    );
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_relative_destination_dir() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "http://example.com/file.bin".into(),
            destination_dir: "relative/out".into(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: None,
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await;

    assert!(
        matches!(result, Err(DownloadError::InvalidResponse(ref msg)) if msg.contains("absolute path"))
    );
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn start_rejects_checksum_mode_mismatch() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .start(StartDownloadRequest {
            kind: None,
            url: "http://example.com/file.bin".into(),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some("test.bin".into()),
            user_agent: None,
            thread_mode: None,
            thread_count: None,
            max_retries: None,
            checksum: Some(ChecksumMode::None),
            expected_checksum: Some("abc123".into()),
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await;

    assert!(
        matches!(result, Err(DownloadError::InvalidRequest(ref msg)) if msg.contains("checksum_mode"))
    );
    Ok(())
}
