//! Irontide-based BitTorrent backend implementation.
//!
//! This backend uses the `irontide` crate as the underlying BT engine,
//! replacing the previous stub implementation.

pub(crate) mod alerts;
pub(crate) mod protocol;
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
use tokio::sync::broadcast;

use super::error::Result;
use super::protocol::DownloadProtocol;
use super::types::{DownloadSnapshot, DownloadSummary};

/// Task ID prefix for irontide-managed torrents.
pub(crate) const BT_PREFIX: &str = "bt:";

// ---------------------------------------------------------------------------
//  OwnBtBackend — irontide-backed BT backend
// ---------------------------------------------------------------------------

/// Irontide-based BitTorrent backend (production, 35 BEPs).
///
/// Each torrent is tracked by a task ID of the form `bt:<info_hash_hex>`.
/// Metadata resolution is handled automatically by irontide (for magnet links),
/// so there is no separate "pending" phase — the info hash is known immediately.
pub struct OwnBtBackend {
    /// The irontide session handle.
    pub(crate) session: irontide::session::SessionHandle,
    /// Directory for state / resume files.
    pub(crate) state_dir: PathBuf,
    /// Default download output directory.
    pub(crate) default_output_dir: PathBuf,
    /// Mirrored BT settings (shared with the alert bridge).
    pub(crate) bt_settings: Arc<Mutex<super::types::BtSettings>>,
    /// Broadcast sender for Aria2 RPC events.
    pub(crate) event_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    /// Map of task ID (`bt:<hex>`) → irontide info hash.
    pub(crate) task_map: Arc<DashMap<String, Id20>>,
    /// Join handle for the alert bridge background task.
    pub(crate) alert_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Join handle for the upload policy background task.
    pub(crate) upload_policy_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Tauri app handle (for emitting frontend events).
    pub(crate) app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
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
//  DownloadProtocol implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl DownloadProtocol for OwnBtBackend {
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.pause(download_id).await
    }

    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.resume(download_id).await
    }

    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.cancel(download_id).await
    }

    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.remove(download_id).await
    }

    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.purge(download_id).await
    }

    async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        self.open_in_explorer(download_id).await
    }

    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.status(download_id).await
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        self.list().await
    }
}
