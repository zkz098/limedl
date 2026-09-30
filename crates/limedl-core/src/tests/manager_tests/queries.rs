//! Lookup, capacity and mode-toggle queries.

use super::*;

#[tokio::test]
#[timeout(30_000)]
async fn get_summary_nonexistent_id_returns_none() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let summary = manager.get_summary("nonexistent").await;
    assert!(summary.is_none());
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn find_active_by_url_no_match_returns_none() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let result = manager
        .find_active_by_url("http://example.com/nonexistent")
        .await;
    assert!(result.is_none());
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn find_active_by_url_match_returns_some() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "active-dl";
    let url = "http://example.com/active.bin";
    let dl = make_managed(id, DownloadState::Downloading, url);
    manager
        .downloads
        .write()
        .await
        .insert(id.to_string(), dl.clone());

    let found = manager.find_active_by_url(url).await;
    assert_eq!(found, Some(id.to_string()));

    let _ = manager.remove(id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn try_acquire_http_at_capacity_returns_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Set max to 0 to simulate at capacity
    manager
        .concurrency
        .max_concurrent_http
        .store(0, Ordering::Release);

    let result = manager.try_acquire_http();
    assert!(matches!(
        result,
        Err(DownloadError::TooManyConcurrentDownloads)
    ));
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn try_acquire_bt_at_capacity_returns_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Set max to 0 to simulate at capacity
    manager
        .concurrency
        .max_concurrent_bt
        .store(0, Ordering::Release);

    let result = manager.try_acquire_bt();
    assert!(matches!(
        result,
        Err(DownloadError::TooManyConcurrentDownloads)
    ));
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn apply_settings_proxy_change_triggers_client_rebuild() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let mut settings = manager.settings().await?;
    // Switch proxy from default (Disabled) to System — triggers client rebuild
    settings.proxy.mode = ProxyMode::System;
    let result = manager.apply_settings(settings).await;
    assert!(
        result.is_ok(),
        "apply_settings with proxy change should succeed: {:?}",
        result.err()
    );

    // Verify the proxy mode stuck
    let updated = manager.settings().await?;
    assert_eq!(updated.proxy.mode, ProxyMode::System);
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn game_mode_setter_and_getter() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    assert!(!manager.game_mode(), "game_mode should default to false");

    manager.set_game_mode(true);
    assert!(manager.game_mode(), "game_mode should be true after set");

    manager.set_game_mode(false);
    assert!(
        !manager.game_mode(),
        "game_mode should be false after unset"
    );
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn overclock_mode_setter_and_getter() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    assert!(
        !manager.overclock_mode(),
        "overclock_mode should default to false"
    );

    manager.set_overclock_mode(true);
    assert!(
        manager.overclock_mode(),
        "overclock_mode should be true after set"
    );

    manager.set_overclock_mode(false);
    assert!(
        !manager.overclock_mode(),
        "overclock_mode should be false after unset"
    );
    Ok(())
}
