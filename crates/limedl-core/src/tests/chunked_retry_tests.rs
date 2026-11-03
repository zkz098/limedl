//! A chunked run that fails must not leave its received counters ahead of the
//! file, or the retry resumes past data the failed run dropped and finalization
//! refuses the hole with "N byte(s) never reached the file".
//!
//! The scenario is deterministic: one worker, two 4 MiB chunks and a
//! write-combining half large enough to hold the whole file, so the first chunk
//! is received-complete while its bytes are still only in the buffer. The next
//! chunk's request then fails (one-shot 500) and ends the run. Everything the
//! buffer held is dropped by `DownloadBuffer::drop`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tempfile::TempDir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{
    AppSettings, DiskType, DownloadState, IoBaselineSettings, SchedulerSettings,
    StartDownloadRequest, ThreadMode,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

const TOTAL: u64 = 8 * 1024 * 1024;
/// The adaptive planner picks 4 MiB chunks for this size, so this is exactly
/// the start of chunk 1 — chunk 0 is fully received before the failure lands.
const FAIL_AFTER: u64 = 4 * 1024 * 1024;

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> crate::types::DownloadSnapshot {
    let start = Instant::now();
    loop {
        let status = manager.status(id).await.expect("status");
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "download {id} did not reach a terminal state"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

/// `resume()` silently no-ops while the failed run still holds its runtime
/// token, so wait for it to be released before retrying.
async fn wait_until_stopped(manager: &DownloadManager, id: &str) {
    let managed = {
        let downloads = manager.downloads.read().await;
        downloads
            .get(id)
            .cloned()
            .expect("managed download must still exist")
    };
    let start = Instant::now();
    while managed.lock_runtime().is_some() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "failed run did not release its runtime token"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_chunked_run_rewinds_progress_so_a_retry_refills_the_dropped_tail() -> TestResult {
    let server = TestServer::new(TOTAL).await;
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // Force the local ping-pong buffer and make each half hold the whole file:
    // nothing is flushed before the run fails, so the drop really discards
    // received-counted bytes.
    let mut overrides = foldhash::HashMap::default();
    overrides.insert(dest_dir.to_string_lossy().to_string(), DiskType::Ssd);
    manager
        .apply_settings(AppSettings {
            scheduler: SchedulerSettings {
                connection_warmup_enabled: false,
                tail_sprint_enabled: false,
                ..Default::default()
            },
            io_baseline: IoBaselineSettings {
                disk_type_overrides: overrides,
                // limit / 2, capped at 8 MiB per half.
                ssd_write_combine_mb: 16,
                ..Default::default()
            },
            ..Default::default()
        })
        .await?;

    let request = StartDownloadRequest {
        url: server.file_url_range_fail_once(FAIL_AFTER),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("retry.bin".into()),
        // One worker keeps the order deterministic: chunk 0 first, then the
        // request for chunk 1 that trips the one-shot 500.
        thread_mode: Some(ThreadMode::Fixed),
        thread_count: Some(1),
        // The 500 must end the run immediately instead of being retried inside
        // the worker; `resume()` uses its own (larger) budget.
        max_retries: Some(0),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
        kind: None,
    };

    let id = manager.start(request).await?;
    let failed = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        failed.state,
        DownloadState::Failed,
        "the injected 500 must fail the run: {:?}",
        failed.error
    );
    wait_until_stopped(&manager, &id.to_string()).await;

    // The invariant the finalize guard depends on: no chunk may claim to have
    // received bytes that are not on the file.
    {
        let downloads = manager.downloads.read().await;
        let managed = downloads.get(&id.to_string()).expect("managed download");
        let core = managed.lock_core();
        assert!(
            !core.manifest.chunks.is_empty(),
            "the download must have run in chunked mode"
        );
        for chunk in &core.manifest.chunks {
            assert_eq!(
                chunk.downloaded,
                chunk.durable_bytes(),
                "chunk {} claims {} received bytes but only {} reached the file",
                chunk.index,
                chunk.downloaded,
                chunk.durable_bytes(),
            );
            assert_eq!(
                chunk.completed,
                chunk.durable_bytes() >= chunk.byte_len(),
                "chunk {} completion must follow the durable count",
                chunk.index,
            );
        }
    }

    // Retrying must re-download what the failed run dropped. Before the fix the
    // received counter stayed ahead of the file, the retry skipped the missing
    // tail and finalization failed with "never reached the file".
    manager.resume(&id.to_string()).await?;
    let done = wait_for_terminal(&manager, &id.to_string()).await;
    assert_eq!(
        done.state,
        DownloadState::Completed,
        "the retry must complete: {:?}",
        done.error
    );
    assert_eq!(
        std::fs::read(dest_dir.join("retry.bin"))?,
        server.content(),
        "the published file must be byte-identical to the source"
    );

    Ok(())
}
