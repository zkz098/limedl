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

use async_trait::async_trait;
use dashmap::DashMap;
use irontide::core::Id20;
use parking_lot::Mutex;

use crate::error::Result;
use crate::event_bus::EventBus;
use crate::protocol::DownloadBackend;
use crate::types::{DownloadSnapshot, DownloadSummary, StartDownloadRequest, TaskId};

/// Task ID prefix for irontide-managed torrents.
pub(crate) const BT_PREFIX: &str = "bt:";

/// Compute an Aria2-compatible GID from an internal task ID.
pub(crate) fn internal_id_to_gid(internal_id: &str) -> String {
    let hash = xxhash_rust::xxh3::xxh3_64(internal_id.as_bytes());
    format!("{:016x}", hash)
}

// ---------------------------------------------------------------------------
//  IrontideBtBackend — irontide-backed BT backend
// ---------------------------------------------------------------------------

/// Irontide-based BitTorrent backend (production, 35 BEPs).
///
/// Each torrent is tracked by a task ID of the form `bt:<info_hash_hex>`.
/// Metadata resolution is handled automatically by irontide (for magnet links),
/// so there is no separate "pending" phase — the info hash is known immediately.
#[derive(Clone)]
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
    /// Map of task ID (`bt:<hex>`) → irontide info hash.
    pub(crate) task_map: Arc<DashMap<String, Id20>>,
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
}

// ---------------------------------------------------------------------------
//  DownloadBackend implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl DownloadBackend for IrontideBtBackend {
    async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        self.start(request).await
    }

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        self.pause(task_id.as_str()).await
    }

    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        self.resume(task_id.as_str()).await
    }

    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        self.cancel(task_id.as_str()).await
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        self.remove(task_id.as_str()).await
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        self.purge(task_id.as_str()).await
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        self.open_in_explorer(task_id.as_str()).await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        self.status(task_id.as_str()).await
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        self.list().await
    }
}
