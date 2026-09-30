//! Pause / resume / cancel state transitions.

use super::*;

#[tokio::test]
#[timeout(30_000)]
async fn pause_on_paused_task_returns_snapshot_unchanged() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "paused-task";
    let dl = make_managed(id, DownloadState::Paused, "http://example.com/file.bin");
    manager
        .downloads
        .write()
        .await
        .insert(id.to_string(), dl.clone());

    let snapshot_before = manager.status(id).await?;
    let result = manager.pause(id).await?;
    // pause() on paused state returns the clone atomically, without modifying state
    assert_eq!(result.state, DownloadState::Paused);
    let snapshot_after = manager.status(id).await?;
    assert_eq!(snapshot_after.state, DownloadState::Paused);
    // Task still in the list (pause does not remove it)
    assert!(manager.downloads.read().await.contains_key(id));
    // Snapshot returned by pause() should match the snapshot from status()
    assert_eq!(result.state, snapshot_before.state);

    let _ = manager.remove(id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn cancel_on_completed_task_skips_file_cleanup_and_removes() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "completed-task";
    let dl = make_managed(id, DownloadState::Completed, "http://example.com/file.bin");
    manager
        .downloads
        .write()
        .await
        .insert(id.to_string(), dl.clone());

    // cancel() on completed task skips the cancellation path, but still removes from list
    let snapshot = manager.cancel(id).await?;
    assert_eq!(snapshot.state, DownloadState::Completed);
    // Task should be removed from active list
    assert!(
        !manager.downloads.read().await.contains_key(id),
        "completed task should be removed from downloads after cancel"
    );

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn cancel_on_already_canceled_task_still_removes() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "already-canceled";
    let dl = make_managed(id, DownloadState::Canceled, "http://example.com/file.bin");
    manager
        .downloads
        .write()
        .await
        .insert(id.to_string(), dl.clone());

    let result = manager.cancel(id).await;
    assert!(
        result.is_ok(),
        "cancel on canceled task should return Ok: {:?}",
        result.err()
    );
    // Task should be removed from active list
    assert!(
        !manager.downloads.read().await.contains_key(id),
        "canceled task should be removed from downloads after second cancel"
    );

    // Second cancel on already-removed task should return NotFound
    let second = manager.cancel(id).await;
    assert!(matches!(second, Err(DownloadError::NotFound)));

    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn resume_canceled_task_returns_canceled_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "canceled-task";
    let dl = make_managed(id, DownloadState::Canceled, "http://example.com/file.bin");
    manager
        .downloads
        .write()
        .await
        .insert(id.to_string(), dl.clone());

    let result = manager.resume(id).await;
    assert!(matches!(result, Err(DownloadError::Canceled)));

    let _ = manager.remove(id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn resume_completed_task_returns_not_resumable_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "completed-task";
    let dl = make_managed(id, DownloadState::Completed, "http://example.com/file.bin");
    manager
        .downloads
        .write()
        .await
        .insert(id.to_string(), dl.clone());

    let result = manager.resume(id).await;
    assert!(matches!(result, Err(DownloadError::NotResumable)));

    let _ = manager.remove(id).await;
    Ok(())
}

#[tokio::test]
#[timeout(30_000)]
async fn resume_running_task_returns_already_running_error() -> TestResult {
    let temp = tempdir()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let id = "downloading-task";
    let dl = make_managed(
        id,
        DownloadState::Downloading,
        "http://example.com/file.bin",
    );
    manager
        .downloads
        .write()
        .await
        .insert(id.to_string(), dl.clone());

    let result = manager.resume(id).await;
    assert!(matches!(result, Err(DownloadError::AlreadyRunning)));

    let _ = manager.remove(id).await;
    Ok(())
}
