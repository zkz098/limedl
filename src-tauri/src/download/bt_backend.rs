use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{
    error::Result,
    protocol::DownloadProtocol,
    types::{
        AppSettings, BtBackendKind, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus,
        BtTrackerInfo, DownloadSnapshot, DownloadSummary, StartDownloadRequest, TorrentFileEntry,
    },
};

/// Abstraction over BT engine implementations (rqbit, own backend, etc.).
///
/// Each backend must also implement [`DownloadProtocol`] so that the existing
/// `protocol_for_task()` router continues to work.  The bridge is provided by
/// [`BtBackend::as_download_protocol`].
#[async_trait]
pub trait BtBackend: Send + Sync {
    // ── Lifecycle ──────────────────────────────────────────────────────

    fn set_app_handle(&self, handle: tauri::AppHandle);
    fn set_event_tx(&self, tx: broadcast::Sender<String>);
    /// Start the background upload-policy polling loop.
    ///
    /// Takes `self` by value (`Arc<Self>`) so callers must clone the `Arc` first
    /// (e.g. `backend.clone().spawn_upload_policy_loop()`). The loop runs until
    /// the `upload_policy_cancel` signal is dropped.
    fn spawn_upload_policy_loop(self: Arc<Self>);
    async fn shutdown(&self);

    // ── Settings ───────────────────────────────────────────────────────

    fn update_settings(&self, settings: &AppSettings);

    // ── Core download operations ───────────────────────────────────────

    /// Start a BT download.
    ///
    /// Returns a **`bt:`-prefixed** task ID (e.g. `"bt:42"` for an active torrent, or
    /// `"bt:pending:<uuid>"` while metadata is being resolved).
    async fn start(&self, request: StartDownloadRequest) -> Result<String>;
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn open_in_explorer(&self, download_id: &str) -> Result<()>;
    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot>;
    async fn list(&self) -> Result<Vec<DownloadSummary>>;

    // ── BT-specific operations ─────────────────────────────────────────

    fn set_speed_limit(
        &self,
        download_id: &str,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    );

    async fn preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>>;
    fn get_peers(&self, download_id: &str) -> Result<Vec<BtPeerInfo>>;
    fn get_trackers(&self, download_id: &str) -> Result<Vec<BtTrackerInfo>>;
    fn get_pieces(&self, download_id: &str) -> Result<Vec<BtPieceInfo>>;
    fn get_torrent_files(&self, download_id: &str) -> Result<Vec<BtFileStatus>>;
    async fn update_torrent_files(
        &self,
        download_id: &str,
        included_indices: Vec<usize>,
    ) -> Result<()>;

    fn runtime_status(&self) -> BtRuntimeStatus;

    /// Emit a `download-updated` event for a pending BT task so the frontend
    /// displays it immediately (before metadata resolution completes).
    fn emit_pending_summary(&self, pending_id: &str);

    // ── Identity ───────────────────────────────────────────────────────

    /// Which backend engine this is.
    fn backend_kind(&self) -> BtBackendKind;

    // ── Bridge to DownloadProtocol ─────────────────────────────────────

    /// Returns `self` as `&dyn DownloadProtocol` so that [`DownloadProtocol`]-based
    /// routing (e.g. `protocol_for_task`) continues to work with trait objects.
    fn as_download_protocol(&self) -> &dyn DownloadProtocol;
}
