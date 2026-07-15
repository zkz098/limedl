use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{
    bt_backend::BtBackend,
    error::{DownloadError, Result},
    protocol::DownloadProtocol,
    types::{
        AppSettings, BtBackendKind, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus,
        BtTrackerInfo, DownloadSnapshot, DownloadSummary, StartDownloadRequest, TorrentFileEntry,
    },
};

/// Placeholder for a self-owned BT backend (work-in-progress).
///
/// All operations return an error indicating the backend is not yet available.
/// This struct exists to wire up the multi-backend selection UI and to serve
/// as a skeleton for future implementation.
pub struct OwnBtBackend {
    kind: BtBackendKind,
}

impl OwnBtBackend {
    pub fn new() -> Self {
        Self {
            kind: BtBackendKind::Own,
        }
    }

    fn not_ready() -> DownloadError {
        DownloadError::Torrent(String::from(
            "own BT backend is under development; please use the rqbit backend",
        ))
    }
}

impl Default for OwnBtBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DownloadProtocol for OwnBtBackend {
    async fn pause(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn resume(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn cancel(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn remove(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn purge(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn open_in_explorer(&self, _download_id: &str) -> Result<()> {
        Err(Self::not_ready())
    }
    async fn status(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        Err(Self::not_ready())
    }
}

#[async_trait]
impl BtBackend for OwnBtBackend {
    fn set_app_handle(&self, _handle: tauri::AppHandle) {}
    fn set_event_tx(&self, _tx: broadcast::Sender<String>) {}
    fn spawn_upload_policy_loop(self: Arc<Self>) {}
    async fn shutdown(&self) {}

    fn update_settings(&self, _settings: &AppSettings) {}

    async fn start(&self, _request: StartDownloadRequest) -> Result<String> {
        Err(Self::not_ready())
    }
    async fn pause(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn resume(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn cancel(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn remove(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn purge(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn open_in_explorer(&self, _download_id: &str) -> Result<()> {
        Err(Self::not_ready())
    }
    async fn status(&self, _download_id: &str) -> Result<DownloadSnapshot> {
        Err(Self::not_ready())
    }
    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        Err(Self::not_ready())
    }

    fn set_speed_limit(
        &self,
        _download_id: &str,
        _download_limit_bps: Option<u64>,
        _upload_limit_bps: Option<u64>,
    ) {
    }

    async fn preview_torrent(&self, _source: &str) -> Result<Vec<TorrentFileEntry>> {
        Err(Self::not_ready())
    }
    fn get_peers(&self, _download_id: &str) -> Result<Vec<BtPeerInfo>> {
        Err(Self::not_ready())
    }
    fn get_trackers(&self, _download_id: &str) -> Result<Vec<BtTrackerInfo>> {
        Err(Self::not_ready())
    }
    fn get_pieces(&self, _download_id: &str) -> Result<Vec<BtPieceInfo>> {
        Err(Self::not_ready())
    }
    fn get_torrent_files(&self, _download_id: &str) -> Result<Vec<BtFileStatus>> {
        Err(Self::not_ready())
    }
    async fn update_torrent_files(
        &self,
        _download_id: &str,
        _included_indices: Vec<usize>,
    ) -> Result<()> {
        Err(Self::not_ready())
    }

    fn runtime_status(&self) -> BtRuntimeStatus {
        BtRuntimeStatus {
            connected: false,
            dht_enabled: false,
            dht_nodes: None,
            torrent_count: 0,
            peer_count: 0,
            upload_speed_bytes_per_second: None,
            uploaded_bytes: 0,
            updated_at_ms: 0,
            seed_count: None,
            leech_count: None,
        }
    }

    fn emit_pending_summary(&self, _pending_id: &str) {
        // no-op for the stub backend
    }

    fn backend_kind(&self) -> BtBackendKind {
        self.kind
    }

    fn as_download_protocol(&self) -> &dyn DownloadProtocol {
        self
    }
}
