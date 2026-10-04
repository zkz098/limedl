//! HTTP response decompression (gzip / brotli / zstd).
//!
//! The engine only negotiates compression on the plain single-stream GET. Every
//! probe and every `Range` request is forced to `Accept-Encoding: identity`,
//! because transparently decompressing a `206` destroys byte offsets, the
//! per-chunk `Content-Length` and the final checksum.
//!
//! These tests pin both halves against a server that compresses whenever it is
//! asked (`TestServer::file_url_encoded` / `file_url_encoded_range`) and counts
//! how often it did. A checksum pinned to the *decoded* content proves the
//! bytes are right; the counter proves the negotiation happened (or, for range
//! downloads, that it did not).

use super::*;

/// Run a full download with the SHA-256 checksum pinned to the server's decoded
/// content, then read the file back. The temp dir lives inside the helper, so
/// the bytes are returned before it is dropped.
async fn download_and_read(
    server: &TestServer,
    url: String,
    thread_count: usize,
) -> (crate::types::DownloadSnapshot, Vec<u8>) {
    let temp = tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("state").join("logs")).ok();

    let manager = Arc::new(
        DownloadManager::new_with_components(
            temp.path().join("state"),
            Arc::new(RateLimiter::default()),
            Arc::new(EventBus::new(1024)),
        )
        .unwrap(),
    );

    let id = manager
        .start(StartDownloadRequest {
            kind: None,
            url,
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            user_agent: None,
            thread_mode: Some(ThreadMode::Fixed),
            thread_count: Some(thread_count),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Sha256),
            expected_checksum: Some(server.sha256_hash.clone()),
            selected_file_indices: None,
            start_paused: false,
            headers: None,
            mirror_urls: None,
            priority: None,
        })
        .await
        .unwrap();

    let status = wait_for_terminal(&manager, &id.to_string()).await;
    let downloaded = tokio::fs::read(&status.destination_path)
        .await
        .unwrap_or_default();
    let _ = manager.remove(&id.to_string()).await;
    (status, downloaded)
}

/// Download the encoded endpoint and assert the decoded file, the reported
/// length and whether the server actually compressed.
async fn assert_decompressed(encoding: &str, threads: usize, expect_compression: bool) {
    // A range download only uses segments above 2× the 4 MiB chunk size.
    let size = if threads > 1 {
        9 * 1024 * 1024
    } else {
        256 * 1024
    };
    let server = TestServer::new(size).await;
    let url = if threads > 1 {
        server.file_url_encoded_range(encoding)
    } else {
        server.file_url_encoded(encoding)
    };

    let (status, downloaded) = download_and_read(&server, url, threads).await;
    assert_eq!(
        status.state,
        DownloadState::Completed,
        "{encoding} threads={threads}: state={:?} error={:?}",
        status.state,
        status.error
    );
    assert_eq!(
        status.total_bytes,
        Some(server.file_size),
        "{encoding} threads={threads}: the reported length must be the decoded size"
    );
    assert_eq!(
        downloaded,
        generate_test_content(server.file_size),
        "{encoding} threads={threads}: decoded content mismatch"
    );

    if expect_compression {
        assert!(
            server.encoded_responses() > 0,
            "{encoding}: the server never compressed — the engine did not negotiate compression"
        );
    } else {
        assert_eq!(
            server.encoded_responses(),
            0,
            "{encoding}: a Range request negotiated compression; offsets would be corrupted"
        );
    }
}

#[tokio::test]
#[timeout(120_000)]
async fn gzip_single_stream_is_decompressed() -> TestResult {
    assert_decompressed("gzip", 1, true).await;
    Ok(())
}

#[tokio::test]
#[timeout(120_000)]
async fn brotli_single_stream_is_decompressed() -> TestResult {
    assert_decompressed("brotli", 1, true).await;
    Ok(())
}

#[tokio::test]
#[timeout(120_000)]
async fn zstd_single_stream_is_decompressed() -> TestResult {
    assert_decompressed("zstd", 1, true).await;
    Ok(())
}

#[tokio::test]
#[timeout(180_000)]
async fn gzip_range_download_stays_identity() -> TestResult {
    assert_decompressed("gzip", 4, false).await;
    Ok(())
}

#[tokio::test]
#[timeout(180_000)]
async fn brotli_range_download_stays_identity() -> TestResult {
    assert_decompressed("brotli", 4, false).await;
    Ok(())
}

#[tokio::test]
#[timeout(180_000)]
async fn zstd_range_download_stays_identity() -> TestResult {
    assert_decompressed("zstd", 4, false).await;
    Ok(())
}
