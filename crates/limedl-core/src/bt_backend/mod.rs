//! Irontide-based BitTorrent backend implementation.
//!
//! This backend uses the `irontide` crate as the underlying BT engine,

pub mod alerts;
pub mod anti_leech;
pub mod lazy;
pub mod lifecycle;
pub mod queries;
pub mod session;
pub mod snapshot;
#[cfg(test)]
pub mod tests;
pub mod uploads;

pub use lazy::LazyBtBackend;

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use irontide::core::Id20;
use parking_lot::{Mutex, RwLock};

use crate::error::DownloadError;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::protocol::DownloadBackend;
use crate::slot_guard::DownloadSlotGuard;
use crate::types::{
    AppSettings, DownloadSnapshot, DownloadSummary, FastDashMap, InfoHash, StartDownloadRequest,
    TaskId,
};

/// Translate an irontide info-hash into the foundation layer's [`InfoHash`].
///
/// `TaskId::Bt` carries `limedl_types::types::InfoHash`, not `Id20`, so that the
/// BitTorrent engine is not a dependency of `limedl-types` (and therefore of
/// every limedl crate). These two functions are the *only* place the two id
/// types meet; everywhere else the engine speaks `Id20` and the rest of the
/// engine speaks `InfoHash`.
///
/// They cannot be `impl From<Id20> for InfoHash`: both types are foreign here,
/// so the orphan rule rejects it.
pub fn info_hash_from_engine(id: Id20) -> InfoHash {
    InfoHash::from(*id.as_bytes())
}

/// Translate a foundation-layer [`InfoHash`] back into the engine's `Id20`.
pub fn info_hash_to_engine(hash: &InfoHash) -> Id20 {
    Id20::from(*hash.as_bytes())
}

/// Compute an Aria2-compatible GID from an info hash.
pub fn internal_id_to_gid(info_hash: &Id20) -> String {
    let hex = info_hash.to_hex();
    let hash = xxhash_rust::xxh3::xxh3_64(hex.as_bytes());
    format!("{:016x}", hash)
}

// ---------------------------------------------------------------------------
//  IrontideBtBackend — irontide-backed BT backend
// ---------------------------------------------------------------------------

/// Irontide-based BitTorrent backend (production, 35 BEPs).
///
/// Each torrent is tracked by its raw info-hash hex string (no prefix).
/// Metadata resolution is handled automatically by irontide (for magnet links),
/// so there is no separate "pending" phase — the info hash is known immediately.
pub struct IrontideBtBackend {
    /// The irontide session handle.
    pub(crate) session: irontide::session::SessionHandle,
    /// Directory for state / resume files.
    pub(crate) state_dir: PathBuf,
    /// Default download output directory.
    pub(crate) default_output_dir: PathBuf,
    /// Mirrored BT settings (shared with the alert bridge).
    pub(crate) bt_settings: Arc<Mutex<crate::types::BtSettings>>,
    /// Central event bus for publishing download events to subscribers and frontend.
    pub(crate) event_bus: Arc<EventBus>,
    /// Map of info hash → info hash (used as a set of active torrents).
    pub(crate) task_map: Arc<FastDashMap<Id20, Id20>>,
    /// Join handle for the alert bridge background task.
    pub(crate) alert_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Join handle for the upload policy background task.
    pub(crate) upload_policy_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Join handle for the anti-leech background task.
    pub(crate) anti_leech_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Anti-leech: IP → ban-expiry-ms for peers we banned. (Ban action only.)
    pub(crate) banned_leechers: Arc<FastDashMap<IpAddr, u64>>,
    /// Anti-leech: info-hash → original upload-slot count before the loop capped
    /// it (LimitSlots action only), used to restore slots once leechers clear.
    pub(crate) anti_leech_slot_state: Arc<FastDashMap<Id20, usize>>,
    /// Blocklist: key (`enabled:path`) of the last successfully applied IP
    /// filter, so we don't reload/rewrite the filter on every settings save.
    pub(crate) applied_blocklist_key: Arc<Mutex<Option<String>>>,
    /// Reusable HTTP client with proxy support for `.torrent` URL fetches.
    ///
    /// Always present and rebuilt by `apply_settings` when proxy / UA settings
    /// change, so torrent fetches follow the same proxy as the download engine.
    /// The `RwLock` is shared across clones so a rebuild is visible everywhere.
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
    pub(crate) max_concurrent_bt: Arc<AtomicUsize>,
    /// Guards holding BT download slots for active torrents.
    pub(crate) bt_slot_guards: Arc<FastDashMap<Id20, DownloadSlotGuard>>,
    /// Creation timestamps for active torrents (populated on start).
    pub(crate) torrent_created_at: Arc<FastDashMap<Id20, u64>>,
}

impl Clone for IrontideBtBackend {
    fn clone(&self) -> Self {
        Self {
            session: self.session.clone(),
            state_dir: self.state_dir.clone(),
            default_output_dir: self.default_output_dir.clone(),
            bt_settings: self.bt_settings.clone(),
            event_bus: self.event_bus.clone(),
            task_map: self.task_map.clone(),
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
impl DownloadBackend for IrontideBtBackend {
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
