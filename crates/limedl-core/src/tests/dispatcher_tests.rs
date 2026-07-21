//! Task 2: Dispatcher cancel/remove/purge/pause emit `DownloadEvent::Updated` invariant.
//!
//! Contract: `Dispatcher` publishes `DownloadEvent::Updated` on the EventBus after
//! every state-changing operation (pause, cancel, remove, purge) so the frontend
//! receives live state synchronization.
//!
//! Each test injects a ManagedDownload with a specific state into the DownloadManager's
//! in-memory map, then calls the dispatcher operation and asserts an Updated event
//! with the matching task ID arrives on the EventBus.

use std::sync::Arc;

use ntest::timeout;
use parking_lot::Mutex as ParkingMutex;
use tempfile::tempdir;
use tokio::sync::Notify;

use crate::aimd::AimdState;
use crate::dispatcher::Dispatcher;
use crate::event_bus::{DownloadEvent, EventBus};
use crate::manager::{DownloadCore, DownloadManager, ManagedDownload};
use crate::manifest::{Manifest, CHUNK_SIZE};
use crate::rate_limiter::RateLimiter;
use crate::types::{
    ChecksumMode, DownloadSnapshot, DownloadState, TaskId, ThreadMode,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a DownloadManager with a temp state dir and return it along with the
/// tempdir guard.
fn make_manager() -> (tempfile::TempDir, Arc<DownloadManager>) {
    let tmp = tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(state_dir.join("logs")).ok();
    let dm = DownloadManager::new(
        state_dir,
        Arc::new(RateLimiter::default()),
        Arc::new(EventBus::new(1024)),
    )
    .expect("DownloadManager::new");
    (tmp, Arc::new(dm))
}

/// Build a minimal ManagedDownload with the given id and state.
/// The download has no runtime token (not spawned), which is safe for
/// cancel/remove/purge/pause testing because these operations tolerate
/// a missing runtime token.
fn make_download(id: &str, state: DownloadState) -> Arc<ManagedDownload> {
    Arc::new(ManagedDownload {
        core: ParkingMutex::new(DownloadCore {
            snapshot: DownloadSnapshot {
                id: id.to_string(),
                kind: crate::types::TaskKind::Http,
                state,
                url: "https://example.com/file.bin".into(),
                final_url: "https://example.com/file.bin".into(),
                file_name: "file.bin".into(),
                destination_path: "".into(),
                temp_path: "".into(),
                total_bytes: Some(1024),
                downloaded_bytes: 0,
                supports_ranges: false,
                connection_count: 0,
                thread_mode: ThreadMode::Fixed,
                requested_thread_count: Some(1),
                desired_thread_count: Some(1),
                allocated_thread_count: Some(0),
                adaptive_profile: None,
                thread_note: None,
                checksum: None,
                checksum_mode: ChecksumMode::None,
                etag: None,
                last_modified: None,
                error: None,
                speed_bytes_per_second: None,
                eta_seconds: None,
                uploaded_bytes: None,
                upload_speed_bytes_per_second: None,
                peer_count: None,
                upload_status: None,
                info_hash: None,
                created_at_ms: 1000,
                updated_at_ms: 1000,
                cdn_accelerated: false,
                chunks: vec![],
                seed_count: None,
                leech_count: None,
                download_limit_bps: None,
                upload_limit_bps: None,
                mirror_url: None,
                degraded: false,
                disk_type: None,
                flushing: false,
            },
            manifest: Manifest {
                id: id.to_string(),
                url: "https://example.com/file.bin".into(),
                final_url: "https://example.com/file.bin".into(),
                user_agent: "test".into(),
                destination_dir: "".into(),
                file_name: "file.bin".into(),
                file_name_locked: false,
                destination_path: "".into(),
                temp_path: "".into(),
                total_bytes: Some(1024),
                downloaded_bytes: 0,
                supports_ranges: false,
                chunk_size: CHUNK_SIZE,
                connection_count: 0,
                thread_mode: ThreadMode::Fixed,
                requested_thread_count: Some(1),
                desired_thread_count: Some(1),
                allocated_thread_count: Some(0),
                adaptive_profile_snapshot: None,
                thread_note: None,
                etag: None,
                last_modified: None,
                state,
                cdn_accelerated: false,
                checksum_mode: ChecksumMode::None,
                checksum: None,
                expected_checksum: None,
                error: None,
                created_at_ms: 1000,
                updated_at_ms: 1000,
                mirror_url: None,
                mirror_urls: vec![],
                current_mirror_index: 0,
                chunks: vec![],
            },
        }),
        runtime: ParkingMutex::new(None),
        aimd: ParkingMutex::new(AimdState::default()),
        stop_notify: Notify::new(),
    })
}

/// Set up a dispatcher with a DM registered as the HTTP backend.
fn make_dispatcher(dm: Arc<DownloadManager>) -> (Arc<EventBus>, Dispatcher) {
    use crate::backend_registry::BackendRegistry;
    let event_bus = Arc::new(EventBus::new(1024));
    let mut registry = BackendRegistry::new();
    registry.register_arc(crate::types::TaskKind::Http, dm.clone());
    let dispatcher = Dispatcher::new(Arc::new(registry), event_bus.clone());
    (event_bus, dispatcher)
}

/// Subscribe to the event bus and return a receiver.
fn subscribe(eb: &EventBus) -> tokio::sync::broadcast::Receiver<DownloadEvent> {
    eb.subscribe()
}

/// Inject a download into the DM's downloads map.
async fn inject_download(dm: &DownloadManager, id: &str, state: DownloadState) {
    let dl = make_download(id, state);
    let id_str = id.to_string();
    dm.downloads.write().await.insert(id_str, dl);
}

// ---------------------------------------------------------------------------
// Test: dispatcher.cancel emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_cancel_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(1));
    let id_str = task_id.to_string();

    // Inject a Paused download (cancel rejects Completed but Paused is fine)
    inject_download(&dm, &id_str, DownloadState::Paused).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.cancel(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Canceled);

    // Consume the event (should arrive immediately, no timeout needed)
    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { id, .. } => {
            assert_eq!(id, id_str, "cancel: Updated event id must match task id");
        }
        other => panic!("cancel: expected Updated, got {other:?}"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: dispatcher.remove emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_remove_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(2));
    let id_str = task_id.to_string();

    // Inject a Completed download (remove works on any state)
    inject_download(&dm, &id_str, DownloadState::Completed).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.remove(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Completed);

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { id, .. } => {
            assert_eq!(id, id_str, "remove: Updated event id must match task id");
        }
        other => panic!("remove: expected Updated, got {other:?}"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: dispatcher.purge emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_purge_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(3));
    let id_str = task_id.to_string();

    // Inject a Failed download
    inject_download(&dm, &id_str, DownloadState::Failed).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.purge(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Failed);

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { id, .. } => {
            assert_eq!(id, id_str, "purge: Updated event id must match task id");
        }
        other => panic!("purge: expected Updated, got {other:?}"),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Test: dispatcher.pause emits DownloadEvent::Updated
// ---------------------------------------------------------------------------
#[tokio::test]
#[timeout(10_000)]
async fn dispatcher_pause_emits_updated() -> TestResult {
    let (_tmp, dm) = make_manager();
    let (eb, dispatcher) = make_dispatcher(dm.clone());

    let task_id = TaskId::Http(uuid::Uuid::from_u128(4));
    let id_str = task_id.to_string();

    // Inject a Queued download (pause only works on Downloading/Retrying/Queued)
    inject_download(&dm, &id_str, DownloadState::Queued).await;

    let mut rx = subscribe(&eb);
    let snapshot = dispatcher.pause(&task_id).await?;
    assert_eq!(snapshot.state, DownloadState::Paused);

    let event = rx.try_recv()?;
    match event {
        DownloadEvent::Updated { id, .. } => {
            assert_eq!(id, id_str, "pause: Updated event id must match task id");
        }
        other => panic!("pause: expected Updated, got {other:?}"),
    }

    Ok(())
}
