//! Metalink piece verification in the chunked HTTP executor.
//!
//! The delayed range endpoint gives the test a window to attach
//! [`PieceVerification`] after `start()` but before the executor plans its
//! chunks — the same ordering the Aria2 metalink flow relies on.

use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::bootstrap::bootstrap;
use crate::checksum::hash_slices;
use crate::metalink::PieceVerification;
use crate::test_harness::TestServer;
use crate::types::{ChecksumMode, DownloadSnapshot, DownloadState, StartDownloadRequest, TaskId};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const PIECE_LEN: u64 = 64 * 1024;

async fn wait_for_terminal(dm: &crate::manager::DownloadManager, id: &str) -> DownloadSnapshot {
    let start = Instant::now();
    loop {
        let snapshot = dm.status(id).await.expect("status");
        if matches!(
            snapshot.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return snapshot;
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "download {id} did not finish"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn request(url: String, destination_dir: &std::path::Path) -> StartDownloadRequest {
    StartDownloadRequest {
        url,
        destination_dir: destination_dir.to_string_lossy().to_string(),
        file_name: Some("piece.bin".into()),
        // Pin one worker: adaptive mode can start more than one chunk worker,
        // and this test is about piece verification rather than concurrency.
        thread_mode: Some(crate::types::ThreadMode::Fixed),
        thread_count: Some(1),
        max_retries: Some(1),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
        kind: None,
    }
}

fn piece_hashes(content: &[u8]) -> Vec<String> {
    content
        .chunks(PIECE_LEN as usize)
        .map(|piece| hash_slices(ChecksumMode::Sha256, &[piece]))
        .collect()
}

async fn start_with_pieces(
    core: &crate::bootstrap::CoreSystems,
    server: &TestServer,
    destination_dir: &std::path::Path,
    hashes: Vec<String>,
) -> TestResult<TaskId> {
    let started = core
        .download_manager
        .start(request(server.file_url_range_slow(300), destination_dir))
        .await?;
    let id = started.to_string();
    let task_id = TaskId::from_wire_string(&id)?;
    core.download_manager
        .set_piece_verification(
            &id,
            PieceVerification {
                piece_length: PIECE_LEN,
                algorithm: ChecksumMode::Sha256,
                hashes,
            },
        )
        .await;
    Ok(task_id)
}

#[tokio::test(flavor = "multi_thread")]
async fn chunked_download_accepts_matching_pieces() -> TestResult {
    let server = TestServer::new(128 * 1024).await;
    let tmp = TempDir::new()?;
    let download_dir = tmp.path().join("download");
    std::fs::create_dir_all(&download_dir)?;
    let core = bootstrap(tmp.path().join("state")).await?;

    let content = server.content().to_vec();
    let task_id = start_with_pieces(&core, &server, &download_dir, piece_hashes(&content)).await?;

    let snapshot = wait_for_terminal(&core.download_manager, &task_id.to_string()).await;
    assert_eq!(
        snapshot.state,
        DownloadState::Completed,
        "matching piece hashes must complete: {:?}",
        snapshot.error
    );
    assert_eq!(std::fs::read(download_dir.join("piece.bin"))?, content);

    core.registry.shutdown_all().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn chunked_download_rejects_mismatched_pieces() -> TestResult {
    let server = TestServer::new(128 * 1024).await;
    let tmp = TempDir::new()?;
    let download_dir = tmp.path().join("download");
    std::fs::create_dir_all(&download_dir)?;
    let core = bootstrap(tmp.path().join("state")).await?;

    let mut hashes = piece_hashes(server.content());
    hashes[0] = "0".repeat(64);
    let task_id = start_with_pieces(&core, &server, &download_dir, hashes).await?;

    let snapshot = wait_for_terminal(&core.download_manager, &task_id.to_string()).await;
    assert_eq!(
        snapshot.state,
        DownloadState::Failed,
        "a corrupt piece must fail the task"
    );
    // The mismatch aborts the worker pool; whichever supervisor path reports
    // first wins — the worker's own error or the finalization guard that
    // refuses to publish the now-incomplete buffer.
    let error = snapshot.error.unwrap_or_default();
    assert!(
        error.contains("piece checksum mismatch") || error.contains("never reached the file"),
        "corrupt piece must be rejected, got: {error}"
    );
    assert!(
        !download_dir.join("piece.bin").exists(),
        "a rejected download must not be published"
    );

    core.registry.shutdown_all().await;
    Ok(())
}
