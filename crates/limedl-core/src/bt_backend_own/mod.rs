//! Irontide-based BitTorrent backend implementation.
//!
//! This backend uses the `irontide` crate as the underlying BT engine,
//! replacing the previous stub implementation.

pub(crate) mod alerts;
pub(crate) mod lifecycle;
pub(crate) mod queries;
pub(crate) mod session;
pub(crate) mod snapshot;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) mod uploads;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

use async_trait::async_trait;
use dashmap::DashMap;
use irontide::core::Id20;
use parking_lot::Mutex;

use crate::error::DownloadError;
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::protocol::DownloadBackend;
use crate::slot_guard::DownloadSlotGuard;
use crate::types::{AppSettings, DownloadSnapshot, DownloadSummary, StartDownloadRequest, TaskId};

/// Compute an Aria2-compatible GID from an info hash.
pub(crate) fn internal_id_to_gid(info_hash: &Id20) -> String {
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
    pub(crate) task_map: Arc<DashMap<Id20, Id20>>,
    /// Join handle for the alert bridge background task.
    pub(crate) alert_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Join handle for the upload policy background task.
    pub(crate) upload_policy_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Reusable HTTP client with proxy support for .torrent URL fetches.
    pub(crate) http_client: Option<reqwest::Client>,
    /// Global download speed limit (bytes/sec) from AppSettings.
    pub(crate) global_speed_limit_bps: u64,
    /// Set of info-hashes whose upload has been paused by the upload policy loop.
    pub(crate) paused_by_limit: Arc<DashMap<Id20, ()>>,
    /// Tokio runtime handle, captured at construction time.
    pub(crate) runtime_handle: tokio::runtime::Handle,
    /// Active BT download counter (shared with DownloadManager for global throttle).
    pub(crate) active_bt_count: Arc<AtomicUsize>,
    /// Maximum concurrent BT downloads allowed.
    pub(crate) max_concurrent_bt: Arc<AtomicUsize>,
    /// Guards holding BT download slots for active torrents.
    pub(crate) bt_slot_guards: Arc<DashMap<Id20, DownloadSlotGuard>>,
    /// Creation timestamps for active torrents (populated on start).
    pub(crate) torrent_created_at: Arc<DashMap<Id20, u64>>,
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
        Ok(TaskId::Bt(info_hash))
    }

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.pause(info_hash).await
    }

    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.resume(info_hash).await
    }

    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.cancel(info_hash).await
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.remove(info_hash).await
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.purge(info_hash).await
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.open_in_explorer(info_hash).await
    }

    async fn open_file(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.open_file(info_hash).await
    }

    async fn open_dir(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.open_dir(info_hash).await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.status(info_hash).await
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
