//! Byte-level oracle E2E tests for HTTP downloads.
//!
//! These tests exercise the *full* engine path (HTTP probe → chunked range
//! download → buffered file writes → finalize) and then re-read the assembled
//! file from disk, hash it independently with SHA-256, and compare it against
//! the source content's known hash from the [`TestServer`].
//!
//! This is deliberately stronger than the existing checksum tests, which only
//! assert `state == Completed` and never verify the actual bytes on disk. The
//! target bug — a download that *finishes* but whose content differs from the
//! source (the "SHA doesn't match" user report) — is exactly what these tests
//! detect: a corrupt or misassembled file produces a different hash.
//!
//! The server content is deterministic (seeded PRNG, seed 42), so a download
//! that fails the oracle in *any* iteration is a real engine nondeterminism
//! (a flaky / sporadic corruption bug), not a test artifact. Treat such a
//! failure as a bug report and preserve the corrupt temp file for diagnosis.
//!
//! [`TestServer`]: crate::test_harness::TestServer

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::TempDir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::checksum::hash_slices;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{ChecksumMode, DownloadState, StartDownloadRequest};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Poll until the task reaches a terminal state and return its snapshot.
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

/// Start a download and wait for it to Complete.
async fn download_to_completion(
    manager: &Arc<DownloadManager>,
    server: &TestServer,
    dest_dir: &Path,
    threads: usize,
    mode: ChecksumMode,
    expected: Option<String>,
) -> crate::types::DownloadSnapshot {
    let request = StartDownloadRequest {
        kind: None,
        url: server.file_url_range(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        thread_mode: None,
        thread_count: Some(threads),
        max_retries: Some(2),
        checksum: Some(mode),
        expected_checksum: expected,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
    };
    let id = manager.start(request).await.expect("start download failed");
    let snap = wait_for_terminal(manager, &id.to_string()).await;
    assert!(
        matches!(snap.state, DownloadState::Completed),
        "download with threads={threads}, mode={mode:?} ended in {:?}: {:?}",
        snap.state,
        snap.error
    );
    snap
}

/// The core oracle: re-read the assembled destination file from disk, hash it
/// independently with SHA-256, and require it to equal the source's known hash.
fn assert_file_matches_source(snap: &crate::types::DownloadSnapshot, server: &TestServer) -> TestResult {
    let dest = PathBuf::from(&snap.destination_path);
    assert!(
        dest.exists(),
        "destination file {} does not exist",
        dest.display()
    );
    let bytes = std::fs::read(&dest)?;
    assert_eq!(
        bytes.len() as u64,
        server.file_size,
        "downloaded file length {} != source length {}",
        bytes.len(),
        server.file_size
    );
    // Independent oracle: hash the bytes on disk, not the engine's stored checksum.
    assert_eq!(
        hash_slices(ChecksumMode::Sha256, &[&bytes]),
        server.sha256_hash,
        "downloaded file SHA-256 does NOT match source (size={}, file may be corrupt)",
        server.file_size
    );
    Ok(())
}

/// Boot a manager and a dedicated output directory in a temp dir, run a download
/// to completion, and verify the on-disk bytes match the source.
async fn run_oracle_round(server: &TestServer, threads: usize, mode: ChecksumMode) -> TestResult {
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;

    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let snap = download_to_completion(&manager, server, &dest_dir, threads, mode, None).await;
    assert_file_matches_source(&snap, server)?;
    Ok(())
}

// ==========================================================================
// Parameterized oracle: multi-threaded range downloads must be byte-identical
// ==========================================================================

/// Multi-threaded range downloads across several thread counts and sizes must
/// produce a file whose SHA-256 equals the source. Repeated iterations surface
/// sporadic corruption that a single run would miss.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_threaded_downloads_byte_identical() -> TestResult {
    const SIZES: [u64; 2] = [8 * 1024 * 1024, 17 * 1024 * 1024]; // 2 / 4+1 chunks (ragged tail)
    const THREADS: [usize; 4] = [1, 2, 4, 8];
    const ITERATIONS: u32 = 2;

    for size in SIZES {
        for threads in THREADS {
            for _ in 0..ITERATIONS {
                let server = TestServer::new(size).await;
                run_oracle_round(&server, threads, ChecksumMode::Sha256).await?;
            }
        }
    }
    Ok(())
}

/// Every checksum mode must yield a byte-identical file. The oracle always hashes
/// with SHA-256 of the on-disk bytes regardless of the engine's mode, so this
/// catches corruption no matter which mode the download used.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_all_checksum_modes_byte_identical() -> TestResult {
    const SIZE: u64 = 17 * 1024 * 1024;
    const THREADS: usize = 4;

    for mode in [
        ChecksumMode::None,
        ChecksumMode::Blake3,
        ChecksumMode::Sha256,
        ChecksumMode::Xxh3128,
        ChecksumMode::Sha1,
    ] {
        let server = TestServer::new(SIZE).await;
        run_oracle_round(&server, THREADS, mode).await?;
    }
    Ok(())
}

/// When an explicit expected checksum is supplied and the download is correct,
/// it must still Complete and the file must match the source.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_with_expected_checksum_completes() -> TestResult {
    const SIZE: u64 = 17 * 1024 * 1024;
    const THREADS: usize = 4;

    let server = TestServer::new(SIZE).await;
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    let snap = download_to_completion(
        &manager,
        &server,
        &dest_dir,
        THREADS,
        ChecksumMode::Sha256,
        Some(server.sha256_hash.clone()),
    )
    .await;
    assert_file_matches_source(&snap, &server)?;
    Ok(())
}

/// Ragged tail sizes (not integer multiples of the chunk size) across a
/// multi-threaded download — exercises the off-by-one / partial-last-chunk paths
/// that are the classic source of silent corruption.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_ragged_tail_multithread() -> TestResult {
    const THREADS: usize = 8;
    const ITERATIONS: u32 = 3;
    let size = crate::manifest::CHUNK_SIZE * 2 + 999_999;

    for _ in 0..ITERATIONS {
        let server = TestServer::new(size).await;
        run_oracle_round(&server, THREADS, ChecksumMode::Sha256).await?;
    }
    Ok(())
}

/// Soak: repeatedly download the same small multi-threaded file and assert the
/// on-disk hash every time. A sporadic corruption bug that reproduces rarely
/// fails at least one iteration of this run, turning "sometimes wrong" into a
/// deterministic CI signal.
#[tokio::test(flavor = "multi_thread")]
#[timeout(120_000)]
async fn oracle_soak_small_multithread_repeated() -> TestResult {
    const SIZE: u64 = 8 * 1024 * 1024;
    const THREADS: usize = 8;
    const ITERATIONS: u32 = 5;

    for _ in 0..ITERATIONS {
        let server = TestServer::new(SIZE).await;
        run_oracle_round(&server, THREADS, ChecksumMode::Sha256).await?;
    }
    Ok(())
}

/// A server that advertises/downloads one byte fewer than the true content must
/// produce a file the oracle flags as NOT matching the source — the engine
/// reports `Completed`, but the assembled file is short. This documents the
/// "silently discarded bytes" class of the reported bug.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn oracle_short_download_flagged_not_matching_source() -> TestResult {
    const SIZE: u64 = 8 * 1024 * 1024;
    let server = TestServer::new(SIZE).await;
    let temp = TempDir::new()?;
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();
    let dest_dir = temp.path().join("out");
    std::fs::create_dir_all(&dest_dir)?;
    let manager = Arc::new(DownloadManager::new_with_components(
        temp.path().join("state"),
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )?);

    // wrong-length: server claims (and serves) SIZE-1 bytes while the true
    // content is SIZE bytes — the body is silently truncated and the engine may
    // mark the task Completed with a short file.
    let request = StartDownloadRequest {
        kind: None,
        url: server.file_url_wrong_length(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        thread_mode: None,
        thread_count: Some(1),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::Sha256),
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
    };
    let id = manager.start(request).await?.to_string();
    let snap = wait_for_terminal(&manager, &id).await;
    assert!(matches!(snap.state, DownloadState::Completed));

    let bytes = std::fs::read(PathBuf::from(&snap.destination_path))?;
    assert_ne!(
        bytes.len() as u64,
        server.file_size,
        "wrong-length download should differ in size"
    );
    assert_ne!(
        hash_slices(ChecksumMode::Sha256, &[&bytes]),
        server.sha256_hash,
        "short download must not hash to the source checksum"
    );
    Ok(())
}
