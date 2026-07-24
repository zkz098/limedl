use ntest::timeout;
use tempfile::TempDir;

use crate::types::{DownloadState, StartDownloadRequest, TaskId};

/// Primary URL unreachable → falls back to mirror → download succeeds
///
/// Constructs mirror_urls so that the broken URL is tried first, and the
/// working TestServer URL is the fallback. The DownloadManager's
/// spawn_download uses mirror_urls directly as urls_to_try when
/// mirror_urls is non-empty (the original `url` field is NOT appended).
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn mirror_fallback_on_primary_failure() {
    // A working TestServer that serves as the mirror
    let test_server = crate::test_harness::TestServer::new(1024 * 1024).await;
    let mirror_url = test_server.file_url_range();

    let tmp = TempDir::new().unwrap();
    let state_dir = tmp.path().join("downloads");
    let dest_dir = tmp.path().join("output");
    std::fs::create_dir_all(&dest_dir).unwrap();

    let core = crate::bootstrap::bootstrap(state_dir).await.unwrap();
    let dm = &core.download_manager;

    // Primary URL is guaranteed unreachable (port 1 is privileged on Linux,
    // typically nothing listens there; on all platforms connection-refused is fast)
    let broken_url = "http://127.0.0.1:1/nonexistent";

    // mirror_urls is the full list of URLs to try in order.
    // spawn_download sets urls_to_try = mirror_urls when non-empty.
    // Put broken_url first, working_url second.
    let request = StartDownloadRequest {
        url: mirror_url.clone(),
        destination_dir: dest_dir.to_string_lossy().to_string(),
        file_name: Some("mirror_test.bin".into()),
        kind: None,
        thread_mode: None,
        thread_count: Some(1),
        max_retries: Some(1),
        checksum: None,
        expected_checksum: None,
        selected_file_indices: None,
        start_paused: false,
        mirror_urls: Some(vec![broken_url.to_string(), mirror_url.clone()]),
        user_agent: None,
    priority: None,
};
    let id = dm.start(request).await.unwrap();
    let task_id = TaskId::from_legacy_string(&id.to_string()).unwrap();
    let inner = match task_id {
        TaskId::Http(u) => u,
        TaskId::Bt(_) => unreachable!(),
    };

    // Wait for download to complete (should succeed via mirror fallback)
    let start = std::time::Instant::now();
    loop {
        let snapshot = dm.status(&inner.to_string()).await.unwrap();
        match snapshot.state {
            DownloadState::Completed => {
                // Verify the final URL is the mirror URL (not the broken primary)
                assert_eq!(
                    snapshot.final_url, mirror_url,
                    "Download should have used the mirror URL after primary failed"
                );
                break;
            }
            DownloadState::Failed => {
                panic!("Mirror fallback failed: {:?}", snapshot.error);
            }
            _ => {}
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            panic!("Download timed out after 30s");
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    core.registry.shutdown_all().await;
}
