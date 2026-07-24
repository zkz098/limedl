//! Unified dispatch layer for download operations.
//!
//! `Dispatcher` wraps [`BackendRegistry`] and [`EventBus`] to provide a single
//! code path for core download lifecycle operations (start/pause/resume/cancel/
//! remove/purge/status/list) and BT-specific queries. Both Tauri commands
//! (`commands.rs`) and the NAS WebSocket JSON-RPC handler (`rpc.rs`) delegate
//! to this layer, eliminating duplicated dispatch/error-mapping logic.
//!
//! ## Event emit policy
//!
//! State-changing methods (`pause`, `resume`, `cancel`, `remove`, `purge`)
//! automatically publish `DownloadEvent::Updated` after a successful backend
//! call.  `start` does **not** emit here (the BT backend already publishes a
//! `DownloadEvent::Updated` via `emit_pending_summary`, and HTTP callers can
//! emit at the boundary layer).  Read-only methods (`status`, `list`) never
//! emit.

use std::sync::Arc;

use crate::backend_registry::BackendRegistry;
use crate::bt_backend_own::IrontideBtBackend;
use crate::error::{DownloadError, Result};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::types::{
    BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtTrackerInfo, DownloadSnapshot,
    DownloadSummary, StartDownloadRequest, TaskId, TorrentFileEntry,
};

/// Unified dispatch layer held by the Tauri [`AppState`] or constructed
/// on-the-fly in RPC handlers.
pub struct Dispatcher {
    registry: Arc<BackendRegistry>,
    event_bus: Arc<EventBus>,
}

impl Dispatcher {
    pub fn new(registry: Arc<BackendRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self { registry, event_bus }
    }

    /// Publish a `DownloadEvent::Updated` for the given snapshot.
    ///
    /// This is the single emit point for state-changing operations.  Both Tauri
    /// and RPC callers rely on the same path so the frontend always stays in
    /// sync.
    pub fn emit_updated(&self, snapshot: &DownloadSnapshot) {
        let summary = DownloadSummary::from(snapshot);
        let summary_json = serde_json::to_value(&summary).unwrap_or_default();
        let id = summary.id.clone();
        self.event_bus
            .publish(DownloadEvent::Updated { id, summary_json });
    }

    // ── Core download lifecycle ──────────────────────────────────────

    /// Start a new download.
    ///
    /// **Does NOT emit** `DownloadEvent::Updated` — the BT backend already
    /// emits a pending-summary via `emit_pending_summary`, and HTTP callers
    /// may emit at the boundary layer if needed.
    pub async fn start(&self, request: StartDownloadRequest) -> Result<TaskId> {
        let kind = request
            .classify_kind()
            .map_err(|_| DownloadError::UnsupportedScheme)?;
        let backend = self.registry.by_kind(kind)?;
        backend.start(request).await
    }

    /// Pause a download.  Emits `DownloadEvent::Updated` on success.
    pub async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.pause(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Resume a download.  Emits `DownloadEvent::Updated` on success.
    pub async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.resume(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Cancel a download.  Emits `DownloadEvent::Updated` on success.
    pub async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.cancel(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Remove a download (keep files).  Emits `DownloadEvent::Updated` on success.
    pub async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.remove(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Purge a download (delete files too).  Emits `DownloadEvent::Updated` on success.
    pub async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.purge(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Set the priority of a download. Emits `DownloadEvent::Updated` on success.
    pub async fn set_priority(
        &self,
        task_id: &TaskId,
        priority: crate::types::Priority,
    ) -> Result<()> {
        let backend = self.registry.dispatch(task_id)?;
        backend.set_priority(task_id, priority).await?;
        // After the backend sets priority, fetch updated status and emit
        if let Ok(snapshot) = backend.status(task_id).await {
            self.emit_updated(&snapshot);
        }
        Ok(())
    }

    /// Get the current status (read-only, no emit).
    pub async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        backend.status(task_id).await
    }

    /// List all downloads across all backends (read-only, no emit).
    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        Ok(self.registry.list_all().await)
    }

    // ── BT-specific operations ───────────────────────────────────────

    fn bt_backend(&self) -> std::result::Result<&IrontideBtBackend, DownloadError> {
        self.registry
            .get_typed::<IrontideBtBackend>()
            .ok_or_else(|| DownloadError::Internal("BT backend not registered".into()))
    }

    /// Get BT engine runtime status (DHT, peer counts, etc.).
    pub fn bt_runtime_status(&self) -> Result<BtRuntimeStatus> {
        Ok(self.bt_backend()?.runtime_status())
    }

    /// Set per-torrent speed limits (download / upload, bytes/sec).
    pub fn bt_set_speed_limit(
        &self,
        task_id: &TaskId,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    ) -> Result<()> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest(
                "speed limit only supported for BT tasks".into(),
            ));
        };
        self.bt_backend()?
            .set_speed_limit(*info_hash, download_limit_bps, upload_limit_bps);
        Ok(())
    }

    /// Preview a torrent file from URL or local path without starting a download.
    pub async fn bt_preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>> {
        self.bt_backend()?.preview_torrent(source).await
    }

    /// Get peer info for a BT task.
    pub fn bt_get_peers(&self, task_id: &TaskId) -> Result<Vec<BtPeerInfo>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_peers(*info_hash)
    }

    /// Get tracker list for a BT task.
    pub fn bt_get_trackers(&self, task_id: &TaskId) -> Result<Vec<BtTrackerInfo>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_trackers(*info_hash)
    }

    /// Get piece info for a BT task.
    pub fn bt_get_pieces(&self, task_id: &TaskId) -> Result<Vec<BtPieceInfo>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_pieces(*info_hash)
    }

    /// Get file status for a BT task.
    pub fn bt_get_files(&self, task_id: &TaskId) -> Result<Vec<BtFileStatus>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_torrent_files(*info_hash)
    }

    /// Update which files are included in a BT download.
    pub async fn bt_update_files(
        &self,
        task_id: &TaskId,
        included_indices: Vec<usize>,
    ) -> Result<()> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?
            .update_torrent_files(*info_hash, included_indices)
            .await
    }
}
