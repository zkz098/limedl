//! `Dispatcher::verify_task_integrity` — file discovery, checksum-mode
//! resolution, comparison and persistence for managed and database-only tasks.

use std::path::Path;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::bootstrap::bootstrap;
use crate::checksum::hash_slices;
use crate::event_bus::DownloadEvent;
use crate::manager::DownloadManager;
use crate::test_harness::TestServer;
use crate::types::{ChecksumMode, DownloadSnapshot, DownloadState, StartDownloadRequest, TaskId};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn wait_for_terminal(dm: &DownloadManager, id: &str) -> DownloadSnapshot {
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

async fn completed_task(
    core: &crate::bootstrap::CoreSystems,
    server: &TestServer,
    destination_dir: &Path,
) -> TestResult<TaskId> {
    let request = StartDownloadRequest {
        url: server.file_url_range(),
        destination_dir: destination_dir.to_string_lossy().to_string(),
        file_name: Some("integrity.bin".into()),
        thread_mode: None,
        thread_count: Some(1),
        max_retries: Some(1),
        checksum: Some(ChecksumMode::Sha256),
        expected_checksum: Some(server.sha256_hash.clone()),
        selected_file_indices: None,
        start_paused: false,
        headers: None,
        mirror_urls: None,
        user_agent: None,
        priority: None,
        kind: None,
    };
    let started = core.download_manager.start(request).await?;
    let task_id = TaskId::from_wire_string(&started.to_string())?;
    let snapshot = wait_for_terminal(&core.download_manager, &task_id.to_string()).await;
    assert_eq!(
        snapshot.state,
        DownloadState::Completed,
        "download failed: {:?}",
        snapshot.error
    );
    Ok(task_id)
}

async fn managed_manifest(dm: &DownloadManager, id: &str) -> crate::manifest::Manifest {
    let downloads = dm.downloads.read().await;
    downloads
        .get(id)
        .expect("managed download")
        .lock_core()
        .manifest
        .clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn verify_integrity_completed_file_matches_and_persists_checksum() -> TestResult {
    let server = TestServer::new(1024 * 1024).await;
    let tmp = TempDir::new()?;
    let destination_dir = tmp.path().join("output");
    std::fs::create_dir_all(&destination_dir)?;
    let core = bootstrap(tmp.path().join("state")).await?;

    let task_id = completed_task(&core, &server, &destination_dir).await?;
    let id = task_id.to_string();

    let mut events = core.event_bus.subscribe();
    let result = core.dispatcher.verify_task_integrity(&task_id).await?;

    assert_eq!(result.mode, ChecksumMode::Sha256);
    assert_eq!(result.computed, server.sha256_hash);
    assert_eq!(
        result.expected.as_deref(),
        Some(server.sha256_hash.as_str())
    );
    assert_eq!(result.matches, Some(true));
    assert!(
        result.target_path.ends_with("integrity.bin"),
        "unexpected target: {}",
        result.target_path
    );

    let manifest = managed_manifest(&core.download_manager, &id).await;
    assert_eq!(
        manifest.checksum.as_deref(),
        Some(server.sha256_hash.as_str()),
        "the computed checksum must be persisted on the managed manifest"
    );

    let mut saw_verifying_progress = false;
    let mut saw_updated = false;
    while let Ok(event) = events.try_recv() {
        match event {
            DownloadEvent::Progress { progress } if progress.id == id => {
                if progress.state == DownloadState::Verifying {
                    saw_verifying_progress = true;
                    assert_eq!(progress.downloaded_bytes, server.file_size);
                    assert_eq!(progress.total_bytes, Some(server.file_size));
                }
            }
            DownloadEvent::Updated { summary } if summary.id == id => {
                saw_updated = true;
            }
            _ => {}
        }
    }
    assert!(saw_verifying_progress, "no Verifying progress event");
    assert!(saw_updated, "no Updated event after verification");

    core.registry.shutdown_all().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn verify_integrity_reports_expected_checksum_mismatch() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let tmp = TempDir::new()?;
    let destination_dir = tmp.path().join("output");
    std::fs::create_dir_all(&destination_dir)?;
    let core = bootstrap(tmp.path().join("state")).await?;

    let task_id = completed_task(&core, &server, &destination_dir).await?;
    let id = task_id.to_string();

    {
        let downloads = core.download_manager.downloads.read().await;
        let managed = downloads.get(&id).expect("managed download");
        managed.lock_core().manifest.expected_checksum = Some("0".repeat(64));
    }

    let result = core.dispatcher.verify_task_integrity(&task_id).await?;
    assert_eq!(result.computed, server.sha256_hash);
    assert_eq!(result.expected, Some("0".repeat(64)));
    assert_eq!(result.matches, Some(false));

    core.registry.shutdown_all().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn verify_integrity_locates_temp_corrupt_and_missing_files() -> TestResult {
    let server = TestServer::new(64 * 1024).await;
    let tmp = TempDir::new()?;
    let destination_dir = tmp.path().join("output");
    std::fs::create_dir_all(&destination_dir)?;
    let core = bootstrap(tmp.path().join("state")).await?;

    let task_id = completed_task(&core, &server, &destination_dir).await?;
    let base = managed_manifest(&core.download_manager, &task_id.to_string()).await;

    let work = tmp.path().join("integrity");
    std::fs::create_dir_all(&work)?;
    let missing_dest = work.join("missing.bin");

    // 1. Only the `.part` file exists: fall back to it and persist the checksum
    //    into the database (the task is not in the in-memory map).
    let part_bytes = b"integrity-part-bytes";
    let part_path = work.join("partial.bin.part");
    std::fs::write(&part_path, part_bytes)?;
    let part_id = uuid::Uuid::from_u128(0x1001).to_string();
    let mut part_manifest = base.clone();
    part_manifest.id = part_id.clone();
    part_manifest.destination_path = missing_dest.to_string_lossy().to_string();
    part_manifest.temp_path = part_path.to_string_lossy().to_string();
    part_manifest.checksum_mode = ChecksumMode::Sha256;
    part_manifest.expected_checksum = Some(hash_slices(ChecksumMode::Sha256, &[part_bytes]));
    core.download_manager.db.insert_download(&part_manifest)?;

    let part_task = TaskId::from_wire_string(&part_id)?;
    let part_result = core.dispatcher.verify_task_integrity(&part_task).await?;
    assert_eq!(part_result.target_path, part_path.to_string_lossy());
    assert_eq!(
        part_result.computed,
        part_manifest.expected_checksum.clone().unwrap()
    );
    assert_eq!(part_result.matches, Some(true));
    let persisted = core
        .download_manager
        .db
        .get_download_header(&part_id)?
        .expect("database-only task must be updated");
    assert_eq!(persisted.checksum, part_manifest.expected_checksum);

    // 2. Only the preserved `.part.corrupt` file exists. The manifest has no
    //    explicit mode, so the settings default (Blake3) applies.
    let corrupt_bytes = b"integrity-corrupt-bytes";
    let corrupt_base = work.join("corrupt.bin.part");
    let corrupt_path = corrupt_base.with_extension("part.corrupt");
    std::fs::write(&corrupt_path, corrupt_bytes)?;
    let corrupt_id = uuid::Uuid::from_u128(0x1002).to_string();
    let mut corrupt_manifest = base.clone();
    corrupt_manifest.id = corrupt_id.clone();
    corrupt_manifest.destination_path = missing_dest.to_string_lossy().to_string();
    corrupt_manifest.temp_path = corrupt_base.to_string_lossy().to_string();
    corrupt_manifest.checksum_mode = ChecksumMode::None;
    corrupt_manifest.expected_checksum = None;
    core.download_manager
        .db
        .insert_download(&corrupt_manifest)?;

    let corrupt_task = TaskId::from_wire_string(&corrupt_id)?;
    let corrupt_result = core.dispatcher.verify_task_integrity(&corrupt_task).await?;
    assert_eq!(corrupt_result.target_path, corrupt_path.to_string_lossy());
    assert_eq!(corrupt_result.mode, ChecksumMode::Blake3);
    assert_eq!(
        corrupt_result.computed,
        hash_slices(ChecksumMode::Blake3, &[corrupt_bytes])
    );
    assert_eq!(corrupt_result.matches, None);

    // 3. `ChecksumMode::None` for the manifest *and* the settings default means
    //    the on-demand verifier still falls back to SHA-256.
    core.dispatcher
        .save_settings_with(|settings| {
            settings.download.default_checksum = ChecksumMode::None;
            Ok(())
        })
        .await?;
    let fallback_id = uuid::Uuid::from_u128(0x1003).to_string();
    let mut fallback_manifest = corrupt_manifest;
    fallback_manifest.id = fallback_id.clone();
    core.download_manager
        .db
        .insert_download(&fallback_manifest)?;

    let fallback_task = TaskId::from_wire_string(&fallback_id)?;
    let fallback_result = core
        .dispatcher
        .verify_task_integrity(&fallback_task)
        .await?;
    assert_eq!(fallback_result.mode, ChecksumMode::Sha256);
    assert_eq!(
        fallback_result.computed,
        hash_slices(ChecksumMode::Sha256, &[corrupt_bytes])
    );

    // 4. Neither destination nor temp nor corrupt file exists.
    let gone_id = uuid::Uuid::from_u128(0x1004).to_string();
    let mut gone_manifest = base.clone();
    gone_manifest.id = gone_id.clone();
    gone_manifest.destination_path = work.join("gone.bin").to_string_lossy().to_string();
    gone_manifest.temp_path = work.join("gone.bin.part").to_string_lossy().to_string();
    core.download_manager.db.insert_download(&gone_manifest)?;

    let gone_task = TaskId::from_wire_string(&gone_id)?;
    let gone_error = core
        .dispatcher
        .verify_task_integrity(&gone_task)
        .await
        .expect_err("no candidate file must be a NotFound");
    assert!(matches!(gone_error, crate::error::DownloadError::NotFound));

    // 5. Unknown task id.
    let unknown = TaskId::from_wire_string(&uuid::Uuid::from_u128(0x1005).to_string())?;
    let unknown_error = core
        .dispatcher
        .verify_task_integrity(&unknown)
        .await
        .expect_err("unknown task must be a NotFound");
    assert!(matches!(
        unknown_error,
        crate::error::DownloadError::NotFound
    ));

    core.registry.shutdown_all().await;
    Ok(())
}
