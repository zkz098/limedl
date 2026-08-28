//! Unified dispatch layer for download operations and system services.
//!
//! `Dispatcher` wraps [`BackendRegistry`], [`EventBus`], and shared system services
//! ([`SettingsService`], [`DiskIoService`], [`ConcurrencyManager`], [`CdnService`])
//! to provide a single code path for:
//! - Core download lifecycle operations (start/pause/resume/cancel/remove/purge/status/list)
//! - BT-specific queries (peers, trackers, pieces, files, speed limits)
//! - Settings management (get, save, reset, mirror resolution)
//! - Disk and I/O status (disk detection, game mode, IO baseline)
//! - Overclock mode toggle and query
//! - Multi-protocol aggregation (active download detection across all backends)
//!
//! Both Tauri IPC commands and the NAS WebSocket JSON-RPC handler delegate
//! to this layer, eliminating duplicated dispatch and `get_typed::<DownloadManager>`
//! downcasting throughout the codebase.

use std::path::Path;
use std::sync::Arc;

use crate::backend_registry::BackendRegistry;
#[cfg(feature = "bt")]
use crate::bt_backend::IrontideBtBackend;
use crate::cdn::CdnService;
use crate::error::{DownloadError, Result};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::services::{ConcurrencyManager, DiskIoService, SettingsService};
use crate::types::{
    AppSettings, DiskType, DownloadSnapshot, DownloadState, DownloadSummary, Priority,
    StartDownloadRequest, TaskId,
};
#[cfg(feature = "bt")]
use crate::types::{
    BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtTrackerInfo, TorrentFileEntry,
};

/// Unified dispatch layer holding references to the backend registry,
/// event bus, and core system services.
#[derive(Clone)]
pub struct Dispatcher {
    registry: Arc<BackendRegistry>,
    event_bus: Arc<EventBus>,
    settings_service: Option<Arc<SettingsService>>,
    disk_io: Option<Arc<DiskIoService>>,
    concurrency: Option<Arc<ConcurrencyManager>>,
    cdn_service: Option<Arc<CdnService>>,
    http_client: Option<reqwest::Client>,
}

impl Dispatcher {
    /// Construct a basic Dispatcher (for tests or minimal environments).
    pub fn new(registry: Arc<BackendRegistry>, event_bus: Arc<EventBus>) -> Self {
        Self {
            registry,
            event_bus,
            settings_service: None,
            disk_io: None,
            concurrency: None,
            cdn_service: None,
            http_client: None,
        }
    }

    /// Construct a fully equipped Dispatcher with all system services.
    pub fn full(
        registry: Arc<BackendRegistry>,
        event_bus: Arc<EventBus>,
        settings_service: Arc<SettingsService>,
        disk_io: Arc<DiskIoService>,
        concurrency: Arc<ConcurrencyManager>,
        cdn_service: Arc<CdnService>,
        http_client: reqwest::Client,
    ) -> Self {
        Self {
            registry,
            event_bus,
            settings_service: Some(settings_service),
            disk_io: Some(disk_io),
            concurrency: Some(concurrency),
            cdn_service: Some(cdn_service),
            http_client: Some(http_client),
        }
    }

    /// Get a reference to the underlying registry.
    pub fn registry(&self) -> &Arc<BackendRegistry> {
        &self.registry
    }

    /// Get a reference to the event bus.
    pub fn event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }

    /// Get a reference to the settings service if configured.
    pub fn settings_service(&self) -> Option<&Arc<SettingsService>> {
        self.settings_service.as_ref()
    }

    /// Get a reference to the disk I/O service if configured.
    pub fn disk_io_service(&self) -> Option<&Arc<DiskIoService>> {
        self.disk_io.as_ref()
    }

    /// Get a reference to the concurrency manager if configured.
    pub fn concurrency(&self) -> Option<&Arc<ConcurrencyManager>> {
        self.concurrency.as_ref()
    }

    /// Get a reference to the CDN service if configured.
    pub fn cdn_service(&self) -> Option<&Arc<CdnService>> {
        self.cdn_service.as_ref()
    }

    /// Publish a `DownloadEvent::Updated` for the given snapshot.
    pub fn emit_updated(&self, snapshot: &DownloadSnapshot) {
        let summary = DownloadSummary::from(snapshot);
        let summary_json = serde_json::to_value(&summary).unwrap_or_default();
        let id = summary.id.clone();
        self.event_bus
            .publish(DownloadEvent::Updated { id, summary_json });
    }

    // ── Core download lifecycle ──────────────────────────────────────

    /// Start a new download.
    /// Automatically populates mirror URLs from settings if not explicitly provided.
    pub async fn start(&self, mut request: StartDownloadRequest) -> Result<TaskId> {
        if request.mirror_urls.is_none() {
            let mirrors = self.resolve_mirror_urls(&request.url).await;
            if mirrors.len() > 1 {
                request.mirror_urls = Some(mirrors);
            }
        }

        let kind = request
            .classify_kind()
            .map_err(|_| DownloadError::UnsupportedScheme)?;
        let backend = self.registry.by_kind(kind)?;
        let task_id = backend.start(request).await?;

        // Emit initial snapshot for immediate UI synchronization
        if let Ok(snapshot) = backend.status(&task_id).await {
            self.emit_updated(&snapshot);
        }

        Ok(task_id)
    }

    /// Pause a download. Emits `DownloadEvent::Updated` on success.
    pub async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.pause(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Resume a download. Emits `DownloadEvent::Updated` on success.
    pub async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.resume(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Cancel a download. Emits `DownloadEvent::Updated` on success.
    pub async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.cancel(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Remove a download (keep files). Emits `DownloadEvent::Updated` on success.
    pub async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.remove(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Purge a download (delete files too). Emits `DownloadEvent::Updated` on success.
    pub async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        let snapshot = backend.purge(task_id).await?;
        self.emit_updated(&snapshot);
        Ok(snapshot)
    }

    /// Set the priority of a download. Emits `DownloadEvent::Updated` on success.
    pub async fn set_priority(&self, task_id: &TaskId, priority: Priority) -> Result<()> {
        let backend = self.registry.dispatch(task_id)?;
        backend.set_priority(task_id, priority).await?;
        if let Ok(snapshot) = backend.status(task_id).await {
            self.emit_updated(&snapshot);
        }
        Ok(())
    }

    /// Open destination directory / select file in system file explorer.
    pub async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        let backend = self.registry.dispatch(task_id)?;
        backend.open_in_explorer(task_id).await
    }

    /// Get the current status (read-only, no emit).
    pub async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let backend = self.registry.dispatch(task_id)?;
        backend.status(task_id).await
    }

    /// List all downloads across all registered backends (polymorphic).
    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        Ok(self.registry.list_all().await)
    }

    /// Check if any registered backend currently has active downloads.
    /// Aggregates all protocols (HTTP + BT), eliminating protocol-specific blindness.
    pub async fn has_active_downloads(&self) -> bool {
        let all = self.registry.list_all().await;
        all.iter()
            .any(|s| matches!(s.state, DownloadState::Downloading))
    }

    // ── Settings management ──────────────────────────────────────────

    /// Retrieve current application settings.
    pub async fn get_settings(&self) -> Result<AppSettings> {
        if let Some(s) = &self.settings_service {
            Ok(s.get().await)
        } else {
            Err(DownloadError::Internal("SettingsService not configured".into()))
        }
    }

    /// Retrieve current application settings in a blocking context.
    pub fn get_settings_blocking(&self) -> Result<AppSettings> {
        if let Some(s) = &self.settings_service {
            Ok(s.get_blocking())
        } else {
            Err(DownloadError::Internal("SettingsService not configured".into()))
        }
    }

    /// Save new application settings: updates SettingsService (single source of truth),
    /// broadcasts to all backends, and syncs CDN and Concurrency limits.
    pub async fn save_settings(&self, new_settings: &AppSettings) -> Result<AppSettings> {
        let saved = if let Some(s) = &self.settings_service {
            s.update(new_settings).await?
        } else {
            new_settings.clone()
        };

        // Broadcast settings to all backends
        self.registry.update_all_settings(&saved).await?;

        // Sync CDN acceleration service
        if let Some(cdn) = &self.cdn_service
            && !saved.cdn_acceleration.enabled
        {
            cdn.clear().await;
        }

        // Sync ConcurrencyManager limits
        if let Some(concurrency) = &self.concurrency {
            concurrency.update_limits(
                saved.scheduler.traditional.max_parallel_tasks,
                saved.scheduler.traditional.max_parallel_tasks.min(3),
            );
        }

        Ok(saved)
    }

    /// Factory reset application settings to defaults and broadcast.
    pub async fn factory_reset(&self) -> Result<AppSettings> {
        let defaults = AppSettings::default();
        self.save_settings(&defaults).await
    }

    /// Returns the default download directory if set.
    pub async fn default_download_dir(&self) -> Option<String> {
        if let Some(s) = &self.settings_service {
            s.default_download_dir().await
        } else {
            None
        }
    }

    /// Resolve candidate mirror URLs based on URL rewrite rules.
    pub async fn resolve_mirror_urls(&self, url: &str) -> Vec<String> {
        if let Some(s) = &self.settings_service {
            let settings = s.get().await;
            if settings.url_rewrite.enabled {
                let rewritten = crate::url_rewrite::rewrite_url(url, &settings.url_rewrite);
                if rewritten.len() > 1 || (rewritten.len() == 1 && rewritten[0] != url) {
                    return rewritten;
                }
            }
        }
        vec![url.to_string()]
    }

    /// Fetch and normalize tracker list from remote URL.
    pub async fn fetch_tracker_list(&self, url: &str) -> Result<Vec<String>> {
        let client = if let Some(c) = &self.http_client {
            c.clone()
        } else {
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::limited(5))
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .map_err(|e| DownloadError::Internal(format!("Failed to build HTTP client: {e}")))?
        };

        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| DownloadError::Internal(format!("Failed to fetch tracker list: {e}")))?;
        let text = resp
            .text()
            .await
            .map_err(|e| DownloadError::Internal(format!("Failed to read tracker list: {e}")))?;
        let normalized = crate::settings::normalize_tracker_list_lossy(&text);
        Ok(normalized
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    // ── Disk & I/O operations ────────────────────────────────────────

    /// Detect disk type for a single path.
    pub fn detect_disk_type(&self, path: &Path) -> Result<DiskType> {
        if let Some(disk) = &self.disk_io {
            Ok(disk.detect_disk_type(path))
        } else {
            Ok(crate::file_ops::detect_disk_type(path))
        }
    }

    /// Detect disk types for all volumes.
    pub fn detect_all_disk_types(&self) -> std::collections::HashMap<String, DiskType> {
        if let Some(disk) = &self.disk_io {
            disk.detect_all_disk_types()
        } else {
            crate::file_ops::detect_all_disk_types()
        }
    }

    /// Get buffer pool and IO status payload.
    pub fn get_io_status(&self) -> Result<serde_json::Value> {
        if let Some(disk) = &self.disk_io {
            Ok(disk.get_io_status())
        } else {
            Err(DownloadError::Internal("DiskIoService not configured".into()))
        }
    }

    /// Toggle or set game mode on the buffer pool.
    pub fn toggle_game_mode(&self, enabled: Option<bool>) -> Result<bool> {
        if let Some(disk) = &self.disk_io {
            Ok(disk.toggle_game_mode(enabled))
        } else {
            Err(DownloadError::Internal("DiskIoService not configured".into()))
        }
    }

    pub fn game_mode(&self) -> bool {
        self.disk_io.as_ref().map(|d| d.game_mode()).unwrap_or(false)
    }

    // ── Concurrency & Overclock ──────────────────────────────────────

    /// Query overclock mode status.
    pub fn get_overclock_mode(&self) -> bool {
        self.concurrency
            .as_ref()
            .map(|c| c.overclock_mode())
            .unwrap_or(false)
    }

    /// Toggle or set overclock mode.
    pub fn toggle_overclock_mode(&self, enabled: Option<bool>) -> Result<bool> {
        if let Some(c) = &self.concurrency {
            Ok(c.toggle_overclock_mode(enabled))
        } else {
            Err(DownloadError::Internal("ConcurrencyManager not configured".into()))
        }
    }

    // ── BT-specific operations ───────────────────────────────────────

    #[cfg(feature = "bt")]
    fn bt_backend(&self) -> std::result::Result<&IrontideBtBackend, DownloadError> {
        self.registry
            .get_typed::<IrontideBtBackend>()
            .ok_or_else(|| DownloadError::Internal("BT backend not registered".into()))
    }

    /// Get BT engine runtime status (DHT, peer counts, etc.).
    #[cfg(feature = "bt")]
    pub fn bt_runtime_status(&self) -> Result<BtRuntimeStatus> {
        Ok(self.bt_backend()?.runtime_status())
    }

    /// Set per-torrent speed limits (download / upload, bytes/sec).
    #[cfg(feature = "bt")]
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
    #[cfg(feature = "bt")]
    pub async fn bt_preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>> {
        self.bt_backend()?.preview_torrent(source).await
    }

    /// Get peer info for a BT task.
    #[cfg(feature = "bt")]
    pub fn bt_get_peers(&self, task_id: &TaskId) -> Result<Vec<BtPeerInfo>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_peers(*info_hash)
    }

    /// Get tracker list for a BT task.
    #[cfg(feature = "bt")]
    pub fn bt_get_trackers(&self, task_id: &TaskId) -> Result<Vec<BtTrackerInfo>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_trackers(*info_hash)
    }

    /// Get piece info for a BT task.
    #[cfg(feature = "bt")]
    pub fn bt_get_pieces(&self, task_id: &TaskId) -> Result<Vec<BtPieceInfo>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_pieces(*info_hash)
    }

    /// Get file status for a BT task.
    #[cfg(feature = "bt")]
    pub fn bt_get_files(&self, task_id: &TaskId) -> Result<Vec<BtFileStatus>> {
        let TaskId::Bt(info_hash) = task_id else {
            return Err(DownloadError::InvalidRequest("Not a BT task".into()));
        };
        self.bt_backend()?.get_torrent_files(*info_hash)
    }

    /// Update which files are included in a BT download.
    #[cfg(feature = "bt")]
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

    /// Proactively probe candidate SHA-256 checksum files for a URL.
    pub async fn probe_checksum(&self, url: &str, file_name: Option<&str>) -> Result<Option<String>> {
        let client = self.http_client.clone().unwrap_or_default();
        let target_file_name = file_name
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                reqwest::Url::parse(url)
                    .ok()
                    .and_then(|u| {
                        u.path_segments()
                            .and_then(|mut segments| segments.next_back().map(ToOwned::to_owned))
                    })
                    .unwrap_or_else(|| String::from("download"))
            });
        let user_agent = self
            .get_settings()
            .await
            .map(|s| s.download.default_user_agent)
            .unwrap_or_else(|_| crate::types::default_http_user_agent());

        Ok(crate::checksum::detect_sha256(
            &client,
            url,
            &target_file_name,
            &user_agent,
            &[],
        )
        .await)
    }
}
