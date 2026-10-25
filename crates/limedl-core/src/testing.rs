//! Test fixtures that more than one crate needs.
//!
//! `limedl-core` owns the manager facade, so the only way to give `limedl-rpc`
//! (or anything else above the engine) a real `DownloadManager` with a temp
//! state dir is to publish the fixture. These are the two `limedl-testkit`
//! cannot host: the test harness may not depend on `limedl-core` (the engine's
//! own `test-utils` already depends on the harness), so anything that needs a
//! `DownloadManager` lives here instead.
//!
//! Gated on `cfg(any(test, feature = "test-utils"))`: production builds do not
//! see it, and `limedl-rpc` gets it through its dev-dependency on this crate.

use std::sync::Arc;

use parking_lot::Mutex as ParkingMutex;
use tempfile::tempdir;
use tokio::sync::Notify;

use crate::aimd::AimdState;
use crate::dispatcher::Dispatcher;
use crate::download::{DownloadCore, ManagedDownload};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::manager::DownloadManager;
use crate::manifest::{CHUNK_SIZE, Manifest};
use crate::rate_limiter::RateLimiter;
use crate::types::{ChecksumMode, DownloadSnapshot, DownloadState, Priority, ThreadMode};

/// Create a DownloadManager with a temp state dir and return it along with the
/// tempdir guard.
///
/// Published (hence `pub`) because `limedl-rpc`'s tests reuse it for `resolve_gid`
/// coverage without duplicating the whole `ManagedDownload` fixture.
pub fn make_manager() -> (tempfile::TempDir, Arc<DownloadManager>) {
    let tmp = tempdir().expect("tempdir");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(state_dir.join("logs")).ok();
    let dm = DownloadManager::new_with_components(
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
pub fn make_download(id: &str, state: DownloadState) -> Arc<ManagedDownload> {
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
                expected_checksum: None,
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
                cdn_node_ip: None,
                chunks: vec![],
                seed_count: None,
                leech_count: None,
                download_limit_bps: None,
                upload_limit_bps: None,
                mirror_url: None,
                priority: Priority::Normal,
                degraded: false,
                disk_type: None,
                flushing: false,
            },
            manifest: Manifest {
                id: id.to_string(),
                url: "https://example.com/file.bin".into(),
                final_url: "https://example.com/file.bin".into(),
                user_agent: "test".into(),
                extra_headers: vec![],
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
                cdn_node_ip: None,
                priority: Priority::Normal,
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
            durable_bytes: 0,
            speed_tracker: Default::default(),
        }),
        runtime: ParkingMutex::new(None),
        aimd: ParkingMutex::new(AimdState::default()),
        stop_notify: Notify::new(),
        pieces: ParkingMutex::new(None),
    })
}

/// Set up a dispatcher with a DM registered as the HTTP backend.
pub fn make_dispatcher(dm: Arc<DownloadManager>) -> (Arc<EventBus>, Dispatcher) {
    use crate::backend_registry::BackendRegistry;
    let event_bus = Arc::new(EventBus::new(1024));
    let mut registry = BackendRegistry::new();
    registry.register_arc(crate::types::TaskKind::Http, dm.clone());
    let dispatcher = Dispatcher::new(Arc::new(registry), event_bus.clone());
    (event_bus, dispatcher)
}

/// Subscribe to the event bus and return a receiver.
pub fn subscribe(eb: &EventBus) -> tokio::sync::broadcast::Receiver<DownloadEvent> {
    eb.subscribe()
}

/// Inject a download into the DM's downloads map.
pub async fn inject_download(dm: &DownloadManager, id: &str, state: DownloadState) {
    let dl = make_download(id, state);
    let id_str = id.to_string();
    dm.downloads.write().await.insert(id_str, dl);
}
