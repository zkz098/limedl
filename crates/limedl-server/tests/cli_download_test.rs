use ntest::timeout;
use tempfile::TempDir;

/// Integration test: download a file from the test HTTP server using the core
/// `DownloadManager` directly. Verifies file size and content checksum.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn cli_download_from_test_server() {
    // Start a test HTTP server with a 1MB test file
    let server = limedl_core::test_harness::TestServer::new(1024 * 1024).await;
    let url = server.file_url();

    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    // Bootstrap and download
    let core = limedl_core::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    let request = limedl_core::types::StartDownloadRequest {
        url: url.clone(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("test.bin".into()),
        kind: None,
        thread_mode: None,
        thread_count: None,
        max_retries: Some(3),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: None,
        user_agent: None,
    };
    let id = dm.start(request).await.unwrap();

    // Wait for download to complete (poll with timeout)
    let task_id = limedl_core::types::TaskId::parse(&id);
    let inner = task_id.http_inner().unwrap();
    let start = std::time::Instant::now();
    loop {
        let snapshot = dm.status(inner).await.unwrap();
        if snapshot.state == limedl_core::types::DownloadState::Completed {
            break;
        }
        if snapshot.state == limedl_core::types::DownloadState::Failed {
            panic!("Download failed: {:?}", snapshot.error);
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            panic!("Download timed out after 30s");
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Verify file exists and has correct size
    let output_path = dest_dir.join("test.bin");
    assert!(output_path.exists(), "output file does not exist");
    let metadata = std::fs::metadata(&output_path).unwrap();
    assert_eq!(metadata.len(), server.file_size, "file size mismatch");

    // Verify content checksum
    let content = std::fs::read(&output_path).unwrap();
    let slices: &[&[u8]] = &[&content];
    let hash = limedl_core::checksum::hash_slices(
        limedl_core::types::ChecksumMode::Blake3,
        slices,
    );
    assert_eq!(
        hash, server.blake3_hash,
        "Blake3 checksum mismatch after download"
    );

    // Cleanup
    core.registry.shutdown_all().await;
}
