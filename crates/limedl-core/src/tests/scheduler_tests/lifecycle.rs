//! Pause/cancel interactions with the scheduler.

use super::*;

/// Start a download with a global speed limit, observe progress, pause,
/// verify bytes are preserved, resume, and complete.
#[tokio::test]
#[timeout(180_000)]
async fn pause_resume_does_not_lose_progress() -> TestResult {
    let server = TestServer::new(50 * 1024 * 1024).await; // 50 MB

    let (_tmp, manager) = create_manager().await;

    // Apply a rate limit so the download takes long enough to observe progress.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 1_000_000, // 1 MB/s
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Traditional,
                traditional: TraditionalSchedulerSettings {
                    max_parallel_tasks: 2,
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        })
        .await?;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let id = manager
        .start(req_fixed(&server.file_url_range(), &out, "pause-test.bin"))
        .await?;

    // Wait for some progress, polling up to 10 seconds.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let before = loop {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let s = manager.status(&id.to_string()).await?;
        if s.downloaded_bytes > 0 {
            break s;
        }
        if tokio::time::Instant::now() > deadline {
            panic!("timed out waiting for download progress before pause");
        }
    };
    let bytes_before = before.downloaded_bytes;

    // Pause.
    let paused = manager.pause(&id.to_string()).await?;
    assert_eq!(paused.state, DownloadState::Paused);

    let bytes_at_pause = paused.downloaded_bytes;
    assert!(
        bytes_at_pause >= bytes_before,
        "regressed {} < {}",
        bytes_at_pause,
        bytes_before
    );

    // Resume.
    let resumed = manager.resume(&id.to_string()).await?;
    assert!(
        matches!(
            resumed.state,
            DownloadState::Queued | DownloadState::Downloading
        ),
        "resume gave {:?}",
        resumed.state
    );

    // Remove the speed limit so completion doesn't take forever.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 0, // unlimited
            ..AppSettings::default()
        })
        .await?;

    // Wait for completion.
    let done = tokio::time::timeout(
        Duration::from_secs(120),
        wait_for_terminal(&manager, &id.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for resume")?;

    assert!(
        done.downloaded_bytes >= bytes_at_pause,
        "progress regressed after resume: {} < {}",
        done.downloaded_bytes,
        bytes_at_pause
    );
    assert_eq!(done.state, DownloadState::Completed);

    let _ = manager.remove(&id.to_string()).await;
    Ok(())
}

/// Start a download with a significant startup delay, then cancel it mid-flight.
#[tokio::test]
#[timeout(180_000)]
async fn cancel_stops_download() -> TestResult {
    let server = TestServer::new(1024 * 1024).await; // 1 MB
    let url = server.file_url_slow(1000); // 1 s startup delay

    let (_tmp, manager) = create_manager().await;
    apply_settings(
        &manager,
        SchedulerSettings {
            mode: SchedulerMode::Traditional,
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 2,
            },
            ..SchedulerSettings::default()
        },
    )
    .await;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let id = manager
        .start(req_fixed(&url, &out, "cancel-test.bin"))
        .await?;

    // Poll until the download transitions to Downloading — on slow CI
    // runners the 500ms sleep may not be sufficient.
    let before = loop {
        let s = manager.status(&id.to_string()).await?;
        if matches!(s.state, DownloadState::Downloading) {
            break s;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(
        matches!(before.state, DownloadState::Downloading),
        "state was {:?}",
        before.state
    );

    let canceled = manager.cancel(&id.to_string()).await?;
    assert_eq!(canceled.state, DownloadState::Canceled);

    // Cancelled downloads should be removed from the list.
    let list = manager.list().await?;
    assert!(
        list.iter().all(|s| s.id != id.to_string()),
        "canceled download should not appear"
    );

    Ok(())
}

/// Start two downloads, pause one, verify the other continues normally,
/// resume the paused one, and confirm both complete.
#[tokio::test]
#[timeout(180_000)]
async fn pause_one_download_does_not_affect_other() -> TestResult {
    let server = TestServer::new(5 * 1024 * 1024).await; // 5 MB

    let (_tmp, manager) = create_manager().await;

    // Apply a modest speed limit so downloads overlap long enough.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 1_000_000, // 1 MB/s (shared)
            scheduler: SchedulerSettings {
                mode: SchedulerMode::Traditional,
                traditional: TraditionalSchedulerSettings {
                    max_parallel_tasks: 2,
                },
                ..SchedulerSettings::default()
            },
            ..AppSettings::default()
        })
        .await?;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let url = server.file_url_range();

    let id1 = manager
        .start(req_fixed_range(&url, &out, "pause-other-a.bin"))
        .await?;
    let id2 = manager
        .start(req_fixed_range(&url, &out, "pause-other-b.bin"))
        .await?;

    // Poll until both downloads have started — on slow CI runners the
    // 500ms sleep may not be sufficient for the scheduler to assign states.
    loop {
        let s2 = manager.status(&id2.to_string()).await?;
        if matches!(
            s2.state,
            DownloadState::Downloading | DownloadState::Completed
        ) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Pause the first.
    let paused = manager.pause(&id1.to_string()).await?;
    assert_eq!(paused.state, DownloadState::Paused);

    // The second download must still be active (Downloading or Completed).
    let other = manager.status(&id2.to_string()).await?;
    assert!(
        matches!(
            other.state,
            DownloadState::Downloading | DownloadState::Completed
        ),
        "second download should be running after first is paused, got {:?}",
        other.state,
    );

    // Resume the paused download.
    let resumed = manager.resume(&id1.to_string()).await?;
    assert!(
        matches!(
            resumed.state,
            DownloadState::Queued | DownloadState::Downloading
        ),
        "resume gave {:?}",
        resumed.state,
    );

    // Remove speed limit so completion is fast.
    manager
        .apply_settings(AppSettings {
            global_speed_limit_bps: 0,
            ..AppSettings::default()
        })
        .await?;

    // Wait for both to complete.
    let done1 = tokio::time::timeout(
        Duration::from_secs(60),
        wait_for_terminal(&manager, &id1.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for first download after resume")?;
    let done2 = tokio::time::timeout(
        Duration::from_secs(60),
        wait_for_terminal(&manager, &id2.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for second download")?;

    assert_eq!(
        done1.state,
        DownloadState::Completed,
        "error={:?}",
        done1.error
    );
    assert_eq!(
        done2.state,
        DownloadState::Completed,
        "error={:?}",
        done2.error
    );

    let _ = manager.remove(&id1.to_string()).await;
    let _ = manager.remove(&id2.to_string()).await;
    Ok(())
}

/// With max_parallel_tasks=1, start two downloads; the second is queued.
/// Cancel the first and verify the second transitions to Downloading or
/// Completed.
#[tokio::test]
#[timeout(120_000)]
async fn cancel_one_unblocks_queued() -> TestResult {
    let server = TestServer::new(512 * 1024).await; // 512 KB

    let (_tmp, manager) = create_manager().await;
    apply_settings(
        &manager,
        SchedulerSettings {
            mode: SchedulerMode::Traditional,
            traditional: TraditionalSchedulerSettings {
                max_parallel_tasks: 1,
            },
            ..SchedulerSettings::default()
        },
    )
    .await;

    let out = _tmp.path().join("out").to_string_lossy().to_string();
    let url = server.file_url_slow(2000); // 2s startup delay prevents instant completion

    let id1 = manager
        .start(req_fixed(&url, &out, "cancel-first.bin"))
        .await?;
    let id2 = manager
        .start(req_fixed(&url, &out, "cancel-second.bin"))
        .await?;

    // Second must be queued (max_parallel_tasks=1).
    // Poll until scheduler assigns a state — on slow CI runners the
    // transition from start() may not be instant; on fast machines
    // the first download (512KB) may complete before we observe Queued.
    loop {
        let s = manager.status(&id2.to_string()).await?;
        if !matches!(s.state, DownloadState::Queued | DownloadState::Downloading) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            continue;
        }
        break;
    }

    // Cancel the first — this removes it and triggers rebalance.
    let canceled = manager.cancel(&id1.to_string()).await?;
    assert_eq!(canceled.state, DownloadState::Canceled);

    // After cancel + rebalance, the second should be running or already complete.
    let s2 = manager.status(&id2.to_string()).await?;
    assert!(
        matches!(
            s2.state,
            DownloadState::Downloading | DownloadState::Completed
        ),
        "second download should be running after first canceled, got {:?}",
        s2.state,
    );

    // Wait for the second to finish.
    let done2 = tokio::time::timeout(
        Duration::from_secs(30),
        wait_for_terminal(&manager, &id2.to_string()),
    )
    .await
    .map_err(|_| "timeout waiting for second download after cancel")?;
    assert_eq!(done2.state, DownloadState::Completed);

    let _ = manager.remove(&id2.to_string()).await;
    Ok(())
}
