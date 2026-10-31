//! Librqbit-based BitTorrent backend implementation.
//!
//! This backend uses the `librqbit` crate as the underlying BT engine.

pub mod alerts;
pub mod anti_leech;
pub mod fixtures;
pub mod lazy;
pub mod lifecycle;
pub mod queries;
pub mod session;
pub mod snapshot;
#[cfg(test)]
pub mod tests;
pub mod uploads;

pub use lazy::{LazyBtBackend, take_legacy_resume_files};

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};

// The layers below, imported at this crate root so the moved modules kept their
// existing `crate::types::…` / `crate::error::…` / `crate::database::…`
// spellings. Private on purpose: nothing downstream may reach the foundation
// layer through the BT crate.
use limedl_http::http_client_factory;
use limedl_store::database;
use limedl_types::{error, event_bus, lock, now_ms, platform, protocol, slot_guard, types};

/// The engine this backend drives, re-exported for callers/tests.
pub use librqbit;
pub use librqbit::dht::Id20;

use crate::error::DownloadError;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::protocol::DownloadBackend;
use crate::slot_guard::DownloadSlotGuard;
use crate::types::{
    AppSettings, DownloadSnapshot, DownloadSummary, FastDashMap, InfoHash, StartDownloadRequest,
    TaskId,
};

/// Translate a librqbit info-hash into the foundation layer's [`InfoHash`].
pub fn info_hash_from_engine(id: Id20) -> InfoHash {
    InfoHash::from(id.0)
}

/// Translate a foundation-layer [`InfoHash`] back into the engine's `Id20`.
pub fn info_hash_to_engine(hash: &InfoHash) -> Id20 {
    Id20::new(*hash.as_bytes())
}

/// Compute an Aria2-compatible GID from an info hash.
pub fn internal_id_to_gid(info_hash: &Id20) -> String {
    let hex = info_hash.as_string();
    let hash = xxhash_rust::xxh3::xxh3_64(hex.as_bytes());
    format!("{:016x}", hash)
}

// ---------------------------------------------------------------------------
//  RqbitBtBackend — librqbit-backed BT backend
// ---------------------------------------------------------------------------

/// A magnet link currently resolving metadata in the background.
#[derive(Debug, Clone)]
pub struct PendingMagnet {
    pub info_hash: Id20,
    pub magnet_url: String,
    pub name: String,
    pub destination_path: String,
    pub created_at_ms: u64,
    pub paused: Arc<AtomicBool>,
    pub abort_handle: tokio::task::AbortHandle,
}

/// Librqbit-based BitTorrent backend.
pub struct RqbitBtBackend {
    /// The librqbit session handle.
    pub(crate) session: Arc<librqbit::Session>,
    /// High-level API facade for queries.
    pub(crate) api: librqbit::api::Api,
    /// Directory for state / resume files.
    pub(crate) state_dir: PathBuf,
    /// Default download output directory.
    pub(crate) default_output_dir: PathBuf,
    /// Mirrored BT settings (shared with the alert bridge).
    pub bt_settings: Arc<Mutex<crate::types::BtSettings>>,
    /// Central event bus for publishing download events to subscribers and frontend.
    pub(crate) event_bus: Arc<EventBus>,
    /// Map of info hash → info hash (used as a set of active torrents).
    pub(crate) task_map: Arc<FastDashMap<Id20, Id20>>,
    /// Magnet links currently resolving metadata asynchronously.
    pub(crate) pending_magnets: Arc<FastDashMap<Id20, PendingMagnet>>,
    /// Join handle for the state monitor / alert bridge background task.
    pub(crate) alert_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Join handle for the upload policy background task.
    pub(crate) upload_policy_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Join handle for the anti-leech background task.
    pub(crate) anti_leech_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Anti-leech: IP → ban-expiry-ms for peers we banned. (Ban action only.)
    pub(crate) banned_leechers: Arc<FastDashMap<IpAddr, u64>>,
    /// Anti-leech: info-hash → original upload-slot count before the loop capped it.
    pub(crate) anti_leech_slot_state: Arc<FastDashMap<Id20, usize>>,
    /// Blocklist: key (`enabled:path`) of the last successfully applied IP filter.
    pub(crate) applied_blocklist_key: Arc<Mutex<Option<String>>>,
    /// Reusable HTTP client with proxy support for `.torrent` URL fetches.
    pub(crate) http_client: Arc<RwLock<reqwest::Client>>,
    /// Global download speed limit (bytes/sec) from AppSettings.
    pub(crate) global_speed_limit_bps: u64,
    /// Set of info-hashes whose upload has been paused by the upload policy loop.
    pub(crate) paused_by_limit: Arc<FastDashMap<Id20, ()>>,
    /// Tokio runtime handle, captured at construction time.
    pub(crate) runtime_handle: tokio::runtime::Handle,
    /// Active BT download counter (shared with DownloadManager for global throttle).
    pub(crate) active_bt_count: Arc<AtomicUsize>,
    /// Maximum concurrent BT downloads allowed.
    pub max_concurrent_bt: Arc<AtomicUsize>,
    /// Guards holding BT download slots for active torrents.
    pub(crate) bt_slot_guards: Arc<FastDashMap<Id20, DownloadSlotGuard>>,
    /// Creation timestamps for active torrents (populated on start).
    pub(crate) torrent_created_at: Arc<FastDashMap<Id20, u64>>,
}

impl Clone for RqbitBtBackend {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            api: self.api.clone(),
            state_dir: self.state_dir.clone(),
            default_output_dir: self.default_output_dir.clone(),
            bt_settings: self.bt_settings.clone(),
            event_bus: self.event_bus.clone(),
            task_map: self.task_map.clone(),
            pending_magnets: self.pending_magnets.clone(),
            alert_task: self.alert_task.clone(),
            upload_policy_task: self.upload_policy_task.clone(),
            anti_leech_task: self.anti_leech_task.clone(),
            banned_leechers: self.banned_leechers.clone(),
            anti_leech_slot_state: self.anti_leech_slot_state.clone(),
            applied_blocklist_key: self.applied_blocklist_key.clone(),
            http_client: self.http_client.clone(),
            global_speed_limit_bps: self.global_speed_limit_bps,
            paused_by_limit: self.paused_by_limit.clone(),
            runtime_handle: self.runtime_handle.clone(),
            active_bt_count: self.active_bt_count.clone(),
            max_concurrent_bt: self.max_concurrent_bt.clone(),
            bt_slot_guards: self.bt_slot_guards.clone(),
            torrent_created_at: self.torrent_created_at.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
//  DownloadBackend implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl DownloadBackend for RqbitBtBackend {
    async fn start(&self, request: StartDownloadRequest) -> Result<TaskId> {
        let info_hash = self.start(request).await?;
        Ok(TaskId::Bt(info_hash_from_engine(info_hash)))
    }

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.pause(info_hash_to_engine(&info_hash)).await
    }

    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.resume(info_hash_to_engine(&info_hash)).await
    }

    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.cancel(info_hash_to_engine(&info_hash)).await
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.remove(info_hash_to_engine(&info_hash)).await
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.purge(info_hash_to_engine(&info_hash)).await
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.open_in_explorer(info_hash_to_engine(&info_hash)).await
    }

    async fn open_file(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.open_file(info_hash_to_engine(&info_hash)).await
    }

    async fn open_dir(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.open_dir(info_hash_to_engine(&info_hash)).await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.status(info_hash_to_engine(&info_hash)).await
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        self.list().await
    }

    async fn update_settings(&self, settings: &AppSettings) -> Result<()> {
        self.apply_settings(settings);
        Ok(())
    }

    async fn shutdown(&self) {
        self.shutdown().await;
    }
}
