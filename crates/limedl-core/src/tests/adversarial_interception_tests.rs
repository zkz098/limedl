//! Adversarial-server E2E tests: verify that the engine *detects* content
//! corruption from a misbehaving range server when an expected checksum is
//! supplied, instead of silently completing with a corrupt file.
//!
//! [`TestServer`] provides two hosts that hand back wrong bytes for valid ranges:
//! - `range-shifted`: each range's content is shifted relative to its advertised
//!   `Content-Range` (a CDN/proxy serving misaligned ranges).
//! - `range-bitflip`: each range's first byte is flipped (a flaky origin that
//!   returns different bytes on retry / drift).
//!
//! Both keep the byte counts identical, so every chunk is accepted and the
//! download reaches finalize — meaning only the checksum comparison can expose
//! the problem. These tests assert the download must end in `Failed`.
//!
//! [`TestServer`]: crate::test_harness::TestServer

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::TempDir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{ChecksumMode, DownloadState, StartDownloadRequest};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> crate::types::DownloadSnapshot {
    let start = std::time::Instant::now();
    loop {
        let status = manager.status(id).await.unwrap();
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "download {id} did not reach terminal state"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

/// Start a download against a corrupting server with an explicit expected
/// checksum (the SHA-256 of the true source) and require it to end in `Failed`.
async fn assert_corrupting_server_is_caught(url: &str, server: &TestServer) -> TestResult {
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let request = StartDownloadRequest {
        kind: None,
        url: url.to_string(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        thread_mode: None,
        thread_count: Some(8),
        max_retries: Some(2),
        checksum: Some(ChecksumMode::Sha256),
        expected_checksum: Some(server.sha256_hash.clone()),
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
    };
    let id = manager.start(request).await?;
    let snap = wait_for_terminal(&manager, &id.to_string()).await;

    assert!(
        matches!(snap.state, DownloadState::Failed),
        "corrupting server must produce Failed, got {:?}: {:?}",
        snap.state,
        snap.error
    );
    let err = snap.error.clone().unwrap_or_default();
    assert!(
        err.to_lowercase().contains("checksum") || err.to_lowercase().contains("mismatch"),
        "failure should mention checksum mismatch, got: {err}"
    );

    // The corrupt temp file must be preserved for diagnosis (renamed to `.corrupt`)
    // rather than silently deleted.
    let corrupt_path = dest_dir.join(format!("{id}.part.corrupt"));
    assert!(
        corrupt_path.exists(),
        "checksum-mismatch download should preserve {} for diagnosis",
        corrupt_path.display()
    );
    Ok(())
}

/// A server that shifts every range's content by 1 byte must be caught as failed
/// once the expected SHA-256 is supplied.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn range_shifted_server_detected_as_failed() -> TestResult {
    let server = TestServer::new(17 * 1024 * 1024).await;
    assert_corrupting_server_is_caught(&server.file_url_range_shifted(1), &server).await
}

/// A server that bit-flips the first byte of every range must be caught as failed
/// once the expected SHA-256 is supplied.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn range_bitflip_server_detected_as_failed() -> TestResult {
    let server = TestServer::new(17 * 1024 * 1024).await;
    assert_corrupting_server_is_caught(&server.file_url_range_bitflip(), &server).await
}

/// When a corrupting server is used WITHOUT an expected checksum, the engine
/// currently has no source of truth and completes with the corrupt file.
///
/// This documents the *current* product behavior (no auto-verification is
/// performed unless the caller supplies an expected checksum). It is intentionally
/// a behavioral snapshot, not an assertion that this is desirable — see the
/// corruption-oracle tests which independently flag such a file. If engine
/// behavior later changes to verify against server-supplied headers, update this
/// test accordingly.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn corrupting_server_without_expected_completes_but_file_is_bad() -> TestResult {
    let server = TestServer::new(17 * 1024 * 1024).await;
    let url = server.file_url_range_shifted(1);

    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let manager = Arc::new(DownloadManager::new(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let request = StartDownloadRequest {
        kind: None,
        url,
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        thread_mode: None,
        thread_count: Some(8),
        max_retries: Some(2),
        checksum: Some(ChecksumMode::Sha256),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
    };
    let id = manager.start(request).await?;
    let snap = wait_for_terminal(&manager, &id.to_string()).await;

    // Completes (current behavior), but the on-disk file differs from the source:
    // this is precisely the silent-corruption scenario the user reported.
    assert!(matches!(snap.state, DownloadState::Completed));
    let bytes = std::fs::read(PathBuf::from(&snap.destination_path))?;
    assert_ne!(
        crate::checksum::hash_slices(ChecksumMode::Sha256, &[&bytes]),
        server.sha256_hash,
        "no-expected-checksum download must still be flagged different from source"
    );
    Ok(())
}
