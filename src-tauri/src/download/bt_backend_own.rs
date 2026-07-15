//! Irontide-based BitTorrent backend implementation.
//!
//! This backend uses the `irontide` crate as the underlying BT engine,
//! replacing the previous stub implementation.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashMap;
use irontide::core::Id20;
use irontide::prelude::*;
use parking_lot::Mutex;
use tauri::Emitter;
use tokio::sync::broadcast;

use super::bt_backend::BtBackend;
use super::error::{DownloadError, Result};
use super::protocol::DownloadProtocol;
use super::settings::build_http_client;
use super::types::{
    AppSettings, BtBackendKind, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus,
    BtTrackerInfo, BtUploadStatus, ChecksumMode, DownloadSnapshot, DownloadState,
    DownloadSummary, StartDownloadRequest, TaskKind, ThreadMode, TorrentFileEntry,
};
use super::{lock, now_ms};

/// Task ID prefix for irontide-managed torrents.
const BT_PREFIX: &str = "bt:";

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
    session: irontide::session::SessionHandle,
    /// Directory for state / resume files.
    state_dir: PathBuf,
    /// Default download output directory.
    default_output_dir: PathBuf,
    /// Mirrored BT settings (shared with the alert bridge).
    bt_settings: Arc<Mutex<super::types::BtSettings>>,
    /// Broadcast sender for Aria2 RPC events.
    event_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    /// Map of task ID (`bt:<hex>`) → irontide info hash.
    task_map: Arc<DashMap<String, Id20>>,
    /// Join handle for the alert bridge background task.
    alert_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Join handle for the upload policy background task.
    upload_policy_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Tauri app handle (for emitting frontend events).
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    /// Reusable HTTP client with proxy support for .torrent URL fetches.
    http_client: Option<reqwest::Client>,
    /// Global download speed limit (bytes/sec) from AppSettings.
    global_speed_limit_bps: u64,
    /// Set of info-hashes whose upload has been paused by the upload policy loop.
    paused_by_limit: Arc<DashMap<Id20, ()>>,
}

impl OwnBtBackend {
    /// Create a new irontide session and wrap it in an `OwnBtBackend`.
    pub async fn new(
        settings: &super::types::AppSettings,
        state_dir: PathBuf,
        default_output_dir: PathBuf,
    ) -> Result<Self> {
        let bt = &settings.bt;

        let mut builder = irontide::ClientBuilder::new()
            .download_dir(&default_output_dir)
            .enable_dht(bt.dht_enabled)
            .enable_upnp(bt.upnp_enabled);

        // Set listen port if configured
        if let Some(range) = &bt.listen_port_range {
            builder = builder.listen_port(range.start);
        }

        // TODO(irontide): No `session_dir()` method exists on `ClientBuilder`.
        // Session state is persisted via `save_session_state()` in shutdown()
        // and loaded via `load_resume_state()` above. Irontide stores state
        // alongside the download dir by default. If a dedicated state dir is
        // needed, this path must be passed to the builder or session handle
        // once irontide exposes the API.

        let session = builder
            .start()
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        // Load resume data from any previous session so existing torrents
        // are restored.
        if let Err(e) = session.load_resume_state().await {
            tracing::warn!("irontide: failed to load resume state: {e}");
        }

        // Build an HTTP client with proxy support for .torrent URL fetching
        let http_client = build_http_client(settings).ok();

        // Apply global speed limit if set.
        // TODO(irontide): irontide's ClientBuilder does not expose a
        // `session_download_limit()` builder method. Once available, move this
        // setting to the builder chain above. For now, the limit is recorded
        // and applied per-torrent in start() via set_download_limit.
        if settings.global_speed_limit_bps > 0 {
            tracing::info!(
                "irontide: global speed limit {} B/s will be applied per-torrent",
                settings.global_speed_limit_bps
            );
        }

        Ok(Self {
            session,
            state_dir,
            default_output_dir,
            bt_settings: Arc::new(Mutex::new(bt.clone())),
            event_tx: Arc::new(Mutex::new(None)),
            task_map: Arc::new(DashMap::new()),
            alert_task: Arc::new(Mutex::new(None)),
            upload_policy_task: Arc::new(Mutex::new(None)),
            app_handle: Arc::new(Mutex::new(None)),
            http_client,
            global_speed_limit_bps: settings.global_speed_limit_bps,
            paused_by_limit: Arc::new(DashMap::new()),
        })
    }

    /// Spawn the alert bridge that listens for irontide alerts and forwards
    /// relevant events to the frontend / Aria2 RPC channel.
    pub async fn setup_alert_bridge(self: &Arc<Self>) {
        let session = self.session.clone();
        let event_tx = self.event_tx.clone();
        let task_map = self.task_map.clone();
        let app_handle = self.app_handle.clone();

        let handle = tokio::spawn(async move {
            alert_bridge_loop(session, event_tx, task_map, app_handle).await;
        });

        *lock(&self.alert_task) = Some(handle);
    }

    // ── Private helpers ────────────────────────────────────────────────

    /// Parse the info hash from a `bt:`-prefixed task ID.
    fn parse_info_hash(download_id: &str) -> Result<Id20> {
        let hex = download_id
            .strip_prefix(BT_PREFIX)
            .ok_or(DownloadError::NotFound)?;
        Id20::from_hex(hex).map_err(|_| DownloadError::NotFound)
    }

    /// Build a `DownloadSnapshot` from irontide stats.
    fn stats_to_snapshot(
        &self,
        task_id: &str,
        info_hash: &Id20,
        stats: &irontide::session::TorrentStats,
    ) -> DownloadSnapshot {
        let now = now_ms();
        let state = map_state(&stats.state);
        let downloaded = stats.total_done;
        let peer_count = stats.peers_connected;
        let total = stats.total;

        // Only show speed when not in a terminal state
        let speed = (stats.download_payload_rate > 0).then_some(stats.download_payload_rate as f64);
        let upload_speed =
            (stats.upload_payload_rate > 0).then_some(stats.upload_payload_rate as f64);

        DownloadSnapshot {
            id: task_id.to_string(),
            kind: TaskKind::Bt,
            state,
            url: info_hash.to_string(),
            final_url: info_hash.to_string(),
            file_name: stats.name.clone(),
            destination_path: self.default_output_dir.to_string_lossy().to_string(),
            temp_path: self.state_dir.to_string_lossy().to_string(),
            total_bytes: Some(total),
            downloaded_bytes: downloaded,
            supports_ranges: false,
            connection_count: peer_count,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: Some(String::from("BT task managed by irontide")),
            checksum: None,
            checksum_mode: ChecksumMode::None,
            etag: None,
            last_modified: None,
            error: if stats.error.is_empty() {
                None
            } else {
                Some(stats.error.clone())
            },
            speed_bytes_per_second: if state.is_terminal() { None } else { speed },
            eta_seconds: if state.is_terminal() {
                None
            } else {
                estimate_eta(total, downloaded, speed)
            },
            uploaded_bytes: Some(stats.uploaded),
            upload_speed_bytes_per_second: if state.is_terminal() { None } else { upload_speed },
            peer_count: Some(peer_count),
            upload_status: Some({
                if self.paused_by_limit.contains_key(info_hash) {
                    BtUploadStatus::PausedByLimit
                } else {
                    match state {
                        DownloadState::Paused => BtUploadStatus::Paused,
                        _ if stats.upload_payload_rate > 0 => BtUploadStatus::Uploading,
                        _ => BtUploadStatus::Idle,
                    }
                }
            }),
            info_hash: Some(info_hash.to_string()),
            created_at_ms: now,
            updated_at_ms: now,
            cdn_accelerated: false,
            chunks: vec![],
            seed_count: Some(stats.num_seeds as u64),
            leech_count: Some(stats.num_peers.saturating_sub(stats.num_seeds) as u64),
            download_limit_bps: None,
            upload_limit_bps: None,
            mirror_url: None,
            degraded: false,
            disk_type: None,
            flushing: false,
        }
    }

    /// Fetch bytes from a URL using the configured HTTP client (with proxy support).
    async fn fetch_url_bytes(&self, url: &str) -> Result<Vec<u8>> {
        if let Some(ref client) = self.http_client {
            let resp = client
                .get(url)
                .send()
                .await
                .map_err(|e| DownloadError::Torrent(format!("failed to fetch torrent: {e}")))?;
            resp.bytes()
                .await
                .map_err(|e| DownloadError::Torrent(format!("failed to read torrent bytes: {e}")))
                .map(|b| b.to_vec())
        } else {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| DownloadError::Torrent(format!("failed to build http client: {e}")))?;
            let resp = client
                .get(url)
                .send()
                .await
                .map_err(|e| DownloadError::Torrent(format!("failed to fetch torrent: {e}")))?;
            resp.bytes()
                .await
                .map_err(|e| DownloadError::Torrent(format!("failed to read torrent bytes: {e}")))
                .map(|b| b.to_vec())
        }
    }

    /// Emit an event via the Aria2 RPC broadcast channel.
    fn emit_aria2_event(&self, method: &str, task_id: &str) {
        let tx_guard = lock(&self.event_tx);
        let Some(ref tx) = *tx_guard else { return };
        let gid = super::aria2_rpc::internal_id_to_gid(task_id);
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": [{"gid": gid}]
        });
        let _ = tx.send(serde_json::to_string(&payload).unwrap_or_default());
    }
}

// ---------------------------------------------------------------------------
//  DownloadProtocol — delegates via BtBackend
// ---------------------------------------------------------------------------

#[async_trait]
impl DownloadProtocol for OwnBtBackend {
    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        BtBackend::pause(self, download_id).await
    }

    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        BtBackend::resume(self, download_id).await
    }

    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        BtBackend::cancel(self, download_id).await
    }

    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        BtBackend::remove(self, download_id).await
    }

    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        BtBackend::purge(self, download_id).await
    }

    async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        BtBackend::open_in_explorer(self, download_id).await
    }

    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        BtBackend::status(self, download_id).await
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        BtBackend::list(self).await
    }
}

// ---------------------------------------------------------------------------
//  BtBackend — full trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl BtBackend for OwnBtBackend {
    fn set_app_handle(&self, handle: tauri::AppHandle) {
        *lock(&self.app_handle) = Some(handle);
    }

    fn set_event_tx(&self, tx: broadcast::Sender<String>) {
        *lock(&self.event_tx) = Some(tx);
    }

    fn spawn_upload_policy_loop(self: Arc<Self>) {
        // Cancel any existing loop
        let handle = {
            let mut slot = lock(&self.upload_policy_task);
            slot.take()
        };
        if let Some(h) = handle {
            h.abort();
        }

        let session = self.session.clone();
        let bt_settings = self.bt_settings.clone();
        let task_map = self.task_map.clone();
        let app_handle = self.app_handle.clone();
        let paused_by_limit = self.paused_by_limit.clone();

        let join = tokio::spawn(async move {
            upload_policy_loop(session, bt_settings, task_map, app_handle, paused_by_limit).await;
        });

        *lock(&self.upload_policy_task) = Some(join);
    }

    async fn shutdown(&self) {
        tracing::info!("irontide backend shutting down...");

        // Phase 1: abort background tasks
        {
            let mut slot = lock(&self.upload_policy_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }
        {
            let mut slot = lock(&self.alert_task);
            if let Some(h) = slot.take() {
                h.abort();
            }
        }

        // Phase 2: persist session state
        let _ = self.session.save_session_state().await;

        // Phase 3: graceful shutdown
        let _ = self.session.shutdown().await;

        tracing::info!("irontide backend shut down.");
    }

    fn update_settings(&self, settings: &AppSettings) {
        *lock(&self.bt_settings) = settings.bt.clone();
        tracing::debug!("irontide settings updated");
    }

    async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        let source = request.url.trim();
        if source.is_empty() {
            return Err(DownloadError::InvalidResponse(
                "torrent source is empty".into(),
            ));
        }

        // Determine the effective download directory.
        let dest_dir = if request.destination_dir.trim().is_empty() {
            self.default_output_dir.clone()
        } else {
            let p = PathBuf::from(request.destination_dir.trim());
            if !p.is_absolute() {
                return Err(DownloadError::InvalidResponse(
                    "download destination directory must be an absolute path".into(),
                ));
            }
            std::fs::create_dir_all(&p).map_err(DownloadError::Io)?;
            p
        };

        // Build AddTorrentParams from source.
        let params = if source.to_ascii_lowercase().starts_with("magnet:") {
            let magnet =
                irontide::core::Magnet::parse(source).map_err(|e| {
                    DownloadError::Torrent(format!("invalid magnet link: {e}"))
                })?;
            AddTorrentParams::from_magnet(magnet)
        } else if source.starts_with("http://") || source.starts_with("https://") {
            // Fetch .torrent from URL (with proxy support)
            let bytes = self.fetch_url_bytes(source).await?;
            AddTorrentParams::from_bytes(bytes)
        } else {
            AddTorrentParams::from_file(source)
        };

        let params = params.download_dir(&dest_dir);

        // Apply start-paused if requested
        let params = if request.start_paused {
            params.paused(true)
        } else {
            params
        };

        let info_hash = params
            .add_to(&self.session)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        let task_id = format!("{}{}", BT_PREFIX, info_hash.to_hex());

        self.task_map.insert(task_id.clone(), info_hash);

        // Apply global download speed limit if configured
        if self.global_speed_limit_bps > 0 {
            let _ = self
                .session
                .set_download_limit(info_hash, self.global_speed_limit_bps)
                .await;
        }

        // Apply selected file priorities if given
        if let Some(indices) = &request.selected_file_indices {
            if let Ok(files) = self.session.torrent_file(info_hash).await {
                if let Some(meta) = files {
                    let file_count = meta.info.files.map_or(1, |f| f.len());
                    for i in 0..file_count {
                        let priority = if indices.contains(&i) {
                            irontide::core::FilePriority::Normal
                        } else {
                            irontide::core::FilePriority::Skip
                        };
                        let _ = self
                            .session
                            .set_file_priority(info_hash, i, priority)
                            .await;
                    }
                }
            }
        }

        // Emit a pending summary so the frontend shows the task immediately.
        self.emit_pending_summary(&task_id);

        tracing::info!("irontide: started torrent {task_id}");
        Ok(task_id)
    }

    async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .pause_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        self.emit_aria2_event("aria2.onDownloadPause", download_id);
        BtBackend::status(self, download_id).await
    }

    async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .resume_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        self.emit_aria2_event("aria2.onDownloadStart", download_id);
        BtBackend::status(self, download_id).await
    }

    async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        // Try to get status, but proceed even if it fails (torrent might already be gone).
        let fallback_snapshot = || DownloadSnapshot {
            id: download_id.to_string(),
            kind: TaskKind::Bt,
            state: DownloadState::Canceled,
            url: String::new(),
            final_url: String::new(),
            file_name: String::new(),
            destination_path: String::new(),
            temp_path: String::new(),
            total_bytes: None,
            downloaded_bytes: 0,
            supports_ranges: false,
            connection_count: 0,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: None,
            checksum: None,
            checksum_mode: ChecksumMode::None,
            etag: None,
            last_modified: None,
            error: None,
            speed_bytes_per_second: None,
            eta_seconds: None,
            uploaded_bytes: Some(0),
            upload_speed_bytes_per_second: None,
            peer_count: None,
            upload_status: None,
            info_hash: None,
            created_at_ms: now_ms(),
            updated_at_ms: now_ms(),
            cdn_accelerated: false,
            chunks: vec![],
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            mirror_url: None,
            degraded: false,
            disk_type: None,
            flushing: false,
        };
        let snapshot = BtBackend::status(self, download_id).await.unwrap_or_else(|_| fallback_snapshot());
        let info_hash = match Self::parse_info_hash(download_id) {
            Ok(h) => h,
            Err(_) => {
                // Already removed from task_map, just return canceled snapshot
                self.task_map.remove(download_id);
                return Ok(DownloadSnapshot {
                    state: DownloadState::Canceled,
                    updated_at_ms: now_ms(),
                    ..snapshot
                });
            }
        };
        let _ = self.session.remove_torrent(info_hash).await;
        self.task_map.remove(download_id);

        Ok(DownloadSnapshot {
            state: DownloadState::Canceled,
            updated_at_ms: now_ms(),
            ..snapshot
        })
    }

    async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = BtBackend::status(self, download_id).await?;
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .remove_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        self.task_map.remove(download_id);
        Ok(snapshot)
    }

    async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = BtBackend::status(self, download_id).await?;
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .remove_torrent_with_files(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        self.task_map.remove(download_id);
        Ok(snapshot)
    }

    async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        let snapshot = BtBackend::status(self, download_id).await?;
        let path = PathBuf::from(&snapshot.destination_path);
        if path.exists() {
            #[cfg(windows)]
            {
                std::process::Command::new("explorer").arg(&path).spawn()?;
            }
            return Ok(());
        }
        Err(DownloadError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "download location does not exist",
        )))
    }

    async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let info_hash = Self::parse_info_hash(download_id)?;
        let stats = self
            .session
            .torrent_stats(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        Ok(self.stats_to_snapshot(download_id, &info_hash, &stats))
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let info_hashes = self
            .session
            .list_torrents()
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        let mut summaries = Vec::with_capacity(info_hashes.len());
        for info_hash in &info_hashes {
            let task_id = format!("{}{}", BT_PREFIX, info_hash.to_hex());
            match self.session.torrent_stats(*info_hash).await {
                Ok(stats) => {
                    let snapshot = self.stats_to_snapshot(&task_id, info_hash, &stats);
                    summaries.push(DownloadSummary::from(&snapshot));
                }
                Err(e) => {
                    tracing::warn!("irontide: failed to get stats for {info_hash}: {e}");
                }
            }
        }

        summaries.sort_by_key(|s| std::cmp::Reverse(s.created_at_ms));
        Ok(summaries)
    }

    fn set_speed_limit(
        &self,
        download_id: &str,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    ) {
        let info_hash = match Self::parse_info_hash(download_id) {
            Ok(h) => h,
            Err(_) => {
                tracing::warn!("irontide: set_speed_limit: invalid task id {download_id}");
                return;
            }
        };

        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        if let Some(bps) = download_limit_bps {
                            let _ = self.session.set_download_limit(info_hash, bps).await;
                        }
                        if let Some(bps) = upload_limit_bps {
                            let _ = self.session.set_upload_limit(info_hash, bps).await;
                        }
                    });
                });
            }
            Err(_) => {
                tracing::warn!("irontide: no tokio runtime for set_speed_limit");
            }
        }
    }

    async fn preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>> {
        let source = source.trim();
        if source.to_ascii_lowercase().starts_with("magnet:") {
            return Err(DownloadError::Torrent(
                "cannot preview magnet link; metadata not yet available".into(),
            ));
        }

        // Read torrent bytes from URL (with proxy support) or local file
        let bytes: Vec<u8> = if source.starts_with("http://") || source.starts_with("https://") {
            self.fetch_url_bytes(source).await?
        } else {
            tokio::fs::read(source)
                .await
                .map_err(|e| DownloadError::Torrent(format!("failed to read torrent file: {e}")))?
        };

        // Parse the torrent metainfo and extract file list
        let meta = irontide::core::torrent_from_bytes_any(&bytes)
            .map_err(|e| DownloadError::Torrent(format!("failed to parse torrent: {e}")))?;

        let entries = preview_entries_from_meta(&meta);

        Ok(entries)
    }

    fn get_peers(&self, download_id: &str) -> Result<Vec<BtPeerInfo>> {
        let info_hash = Self::parse_info_hash(download_id)?;
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                let peers = tokio::task::block_in_place(|| {
                    handle
                        .block_on(self.session.get_peer_info(info_hash))
                })
                .map_err(|e| DownloadError::Torrent(e.to_string()))?;
                Ok(peers
                    .iter()
                    .map(|p| BtPeerInfo {
                        address: p.addr.to_string(),
                        client: p.client.clone(),
                        flags: build_peer_flags(p),
                        download_speed: p.download_rate as f64,
                        upload_speed: p.upload_rate as f64,
                        progress: p.progress as f64,
                    })
                    .collect())
            }
            Err(_) => Err(DownloadError::Torrent(
                "no tokio runtime for get_peers".into(),
            )),
        }
    }

    fn get_trackers(&self, download_id: &str) -> Result<Vec<BtTrackerInfo>> {
        let info_hash = Self::parse_info_hash(download_id)?;
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                let trackers = tokio::task::block_in_place(|| {
                    handle
                        .block_on(self.session.tracker_list(info_hash))
                })
                .map_err(|e| DownloadError::Torrent(e.to_string()))?;
                Ok(trackers
                    .iter()
                    .map(|t| BtTrackerInfo {
                        url: t.url.clone(),
                    })
                    .collect())
            }
            Err(_) => Err(DownloadError::Torrent(
                "no tokio runtime for get_trackers".into(),
            )),
        }
    }

    fn get_pieces(&self, download_id: &str) -> Result<Vec<BtPieceInfo>> {
        let info_hash = Self::parse_info_hash(download_id)?;
        // Use torrent_stats to get pieces_have/pieces_total and derive piece info
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                let stats = tokio::task::block_in_place(|| {
                    handle
                        .block_on(self.session.torrent_stats(info_hash))
                })
                .map_err(|e| DownloadError::Torrent(e.to_string()))?;

                let total = stats.pieces_total as u64;
                let have = stats.pieces_have as u64;
                Ok((0..total)
                    .map(|i| BtPieceInfo {
                        index: i,
                        completed: i < have,
                    })
                    .collect())
            }
            Err(_) => Err(DownloadError::Torrent(
                "no tokio runtime for get_pieces".into(),
            )),
        }
    }

    fn get_torrent_files(&self, download_id: &str) -> Result<Vec<BtFileStatus>> {
        let info_hash = Self::parse_info_hash(download_id)?;
        let rt = tokio::runtime::Handle::try_current();
        match rt {
            Ok(handle) => {
                // Use torrent_file + file_progress + file_status to build file status
                let meta_fut = self.session.torrent_file(info_hash);
                let progress_fut = self.session.file_progress(info_hash);
                let status_fut = self.session.file_status(info_hash);

                let (meta_result, progress_result, status_result) =
                    tokio::task::block_in_place(|| {
                        handle.block_on(async { tokio::join!(meta_fut, progress_fut, status_fut) })
                    });

                let file_progress = progress_result.map_err(|e| {
                    DownloadError::Torrent(format!("failed to get file progress: {e}"))
                })?;

                let file_statuses = status_result.ok();

                match meta_result {
                    Ok(Some(meta)) => {
                        let files = meta.info.files.unwrap_or_default();
                        Ok(files
                            .iter()
                            .enumerate()
                            .map(|(i, f)| {
                                let path: PathBuf = f.path.iter().collect();
                                // Use file_status mode as a proxy for included/excluded.
                                // Closed = skipped/excluded, ReadOnly/ReadWrite = included.
                                // NOTE: irontide does not expose a direct file_priority()
                                // query method. This is a best-effort heuristic.
                                let included = file_statuses.as_ref().map_or(true, |sts| {
                                    sts.get(i).map_or(true, |fs| {
                                        !matches!(fs.mode, irontide::session::FileMode::Closed)
                                    })
                                });
                                BtFileStatus {
                                    index: i,
                                    path: path.to_string_lossy().to_string(),
                                    size: f.length,
                                    downloaded_bytes: file_progress.get(i).copied().unwrap_or(0),
                                    included,
                                }
                            })
                            .collect())
                    }
                    Ok(None) => {
                        // No metadata yet (magnet still resolving)
                        Ok(Vec::new())
                    }
                    Err(e) => Err(DownloadError::Torrent(format!(
                        "failed to get torrent file info: {e}"
                    ))),
                }
            }
            Err(_) => Err(DownloadError::Torrent(
                "no tokio runtime for get_torrent_files".into(),
            )),
        }
    }

    async fn update_torrent_files(
        &self,
        download_id: &str,
        included_indices: Vec<usize>,
    ) -> Result<()> {
        let info_hash = Self::parse_info_hash(download_id)?;

        // Get the torrent metadata to know how many files there are
        let meta = self
            .session
            .torrent_file(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        let Some(meta) = meta else {
            return Err(DownloadError::Torrent(
                "torrent metadata not yet available".into(),
            ));
        };

        let file_count = meta.info.files.map_or(1, |f| f.len());
        let included_set: std::collections::HashSet<usize> =
            included_indices.into_iter().collect();

        for i in 0..file_count {
            let priority = if included_set.contains(&i) {
                irontide::core::FilePriority::Normal
            } else {
                irontide::core::FilePriority::Skip
            };
            self.session
                .set_file_priority(info_hash, i, priority)
                .await
                .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        }

        Ok(())
    }

    fn runtime_status(&self) -> BtRuntimeStatus {
        let dht_enabled = lock(&self.bt_settings).dht_enabled;

        let rt = tokio::runtime::Handle::try_current();
        let (dht_nodes, torrent_count, peer_count, upload_speed, uploaded) = match rt {
            Ok(handle) => tokio::task::block_in_place(|| {
                let dht_nodes = handle
                    .block_on(self.session.session_stats())
                    .ok()
                    .map(|s| s.dht_nodes);

                let torrents: Vec<Id20> = handle
                    .block_on(self.session.list_torrents())
                    .unwrap_or_default();

                let count = torrents.len();
                let mut peers = 0usize;
                let mut up_speed = 0.0f64;
                let mut uploaded: u64 = 0;
                for ih in &torrents {
                    if let Ok(stats) = handle.block_on(self.session.torrent_stats(*ih)) {
                        peers += stats.peers_connected;
                        up_speed += stats.upload_payload_rate as f64;
                        uploaded += stats.uploaded;
                    }
                }

                (dht_nodes, count, peers, up_speed, uploaded)
            }),
            Err(_) => (None, 0, 0, 0.0, 0),
        };

        let connected = peer_count > 0 || dht_nodes.unwrap_or(0) > 0 || upload_speed > 0.0;

        BtRuntimeStatus {
            connected,
            dht_enabled,
            dht_nodes,
            torrent_count,
            peer_count,
            upload_speed_bytes_per_second: (upload_speed > 0.0).then_some(upload_speed),
            uploaded_bytes: uploaded,
            updated_at_ms: now_ms(),
            seed_count: None,
            leech_count: None,
        }
    }

    fn emit_pending_summary(&self, pending_id: &str) {
        let handle_guard = lock(&self.app_handle);
        let Some(ref handle) = *handle_guard else { return };

        // Try to get stats; if not available yet, emit a minimal snapshot.
        match Self::parse_info_hash(pending_id) {
            Ok(info_hash) => {
                let rt = tokio::runtime::Handle::try_current();
                match rt {
                    Ok(handle_rt) => {
                        if let Ok(stats) = tokio::task::block_in_place(|| {
                            handle_rt.block_on(self.session.torrent_stats(info_hash))
                        })
                        {
                            let snapshot =
                                self.stats_to_snapshot(pending_id, &info_hash, &stats);
                            let summary = DownloadSummary::from(&snapshot);
                            let _ = handle.emit("download-updated", &summary);
                            return;
                        }
                    }
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }

        // Fallback: emit a queued-state summary
        let summary = DownloadSummary {
            id: pending_id.to_string(),
            kind: TaskKind::Bt,
            state: DownloadState::Queued,
            url: String::new(),
            file_name: String::from("Pending torrent"),
            destination_path: self.default_output_dir.to_string_lossy().to_string(),
            total_bytes: None,
            downloaded_bytes: 0,
            connection_count: 0,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: Some(String::from("Adding torrent to irontide session")),
            speed_bytes_per_second: None,
            eta_seconds: None,
            uploaded_bytes: Some(0),
            upload_speed_bytes_per_second: None,
            peer_count: Some(0),
            upload_status: Some(BtUploadStatus::Idle),
            info_hash: None,
            error: None,
            cdn_accelerated: false,
            created_at_ms: now_ms(),
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            chunks: vec![],
            mirror_url: None,
        };
        let _ = handle.emit("download-updated", &summary);
    }

    fn backend_kind(&self) -> BtBackendKind {
        BtBackendKind::Irontide
    }

    fn as_download_protocol(&self) -> &dyn DownloadProtocol {
        self
    }
}

// ---------------------------------------------------------------------------
//  Helper functions
// ---------------------------------------------------------------------------

/// Map an irontide `TorrentState` to our `DownloadState`.
fn map_state(state: &irontide::session::TorrentState) -> DownloadState {
    use irontide::session::TorrentState;
    match state {
        TorrentState::Downloading => DownloadState::Downloading,
        TorrentState::Seeding | TorrentState::Complete => DownloadState::Completed,
        TorrentState::Paused => DownloadState::Paused,
        TorrentState::Checking => DownloadState::Verifying,
        TorrentState::FetchingMetadata | TorrentState::Queued => DownloadState::Queued,
        TorrentState::Stopped => DownloadState::Canceled,
        TorrentState::Sharing => DownloadState::Downloading,
    }
}

/// Build a human-readable flags string from irontide peer info.
fn build_peer_flags(peer: &irontide::session::PeerInfo) -> String {
    let mut flags = String::with_capacity(8);
    if peer.is_encrypted {
        flags.push('E');
    }
    if peer.uses_utp {
        flags.push('u');
    }
    if peer.supports_fast {
        flags.push('F');
    }
    if peer.upload_only {
        flags.push('U');
    }
    if peer.snubbed {
        flags.push('S');
    }
    if peer.am_choking {
        flags.push('c');
    }
    if peer.peer_interested {
        flags.push('I');
    }
    flags
}

/// Extract file entries from parsed torrent metadata for preview.
fn preview_entries_from_meta(meta: &irontide::core::TorrentMeta) -> Vec<TorrentFileEntry> {
    match meta {
        irontide::core::TorrentMeta::V1(v1) => v1_file_entries(v1),
        irontide::core::TorrentMeta::Hybrid(v1, _) => v1_file_entries(v1),
        irontide::core::TorrentMeta::V2(_) => vec![TorrentFileEntry {
            index: 0,
            path: String::from("v2-torrent"),
            size: 0,
        }],
    }
}

fn v1_file_entries(v1: &irontide::core::TorrentMetaV1) -> Vec<TorrentFileEntry> {
    if let Some(ref file_list) = v1.info.files {
        file_list
            .iter()
            .enumerate()
            .map(|(i, f)| TorrentFileEntry {
                index: i,
                path: f.path.join("/"),
                size: f.length,
            })
            .collect()
    } else {
        vec![TorrentFileEntry {
            index: 0,
            path: v1.info.name.clone(),
            size: v1.info.length.unwrap_or(0),
        }]
    }
}

/// Estimate remaining time from total, downloaded, and speed.
fn estimate_eta(total: u64, downloaded: u64, speed: Option<f64>) -> Option<u64> {
    let speed = speed?;
    if total <= downloaded || speed <= 0.0 {
        return None;
    }
    Some(((total - downloaded) as f64 / speed).ceil() as u64)
}

trait StateHelpers {
    fn is_terminal(&self) -> bool;
}

impl StateHelpers for DownloadState {
    fn is_terminal(&self) -> bool {
        matches!(
            self,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        )
    }
}

// ---------------------------------------------------------------------------
//  Alert bridge — forwards irontide events to frontend / Aria2 RPC
// ---------------------------------------------------------------------------

/// Extract the `Id20` info hash from an `AlertKind`, if the variant carries one.
fn extract_info_hash<'a>(kind: &'a AlertKind) -> Option<&'a Id20> {
    use irontide::session::AlertKind::*;
    match kind {
        TorrentAdded { info_hash, .. }
        | TorrentRemoved { info_hash }
        | TorrentPaused { info_hash }
        | TorrentResumed { info_hash }
        | TorrentFinished { info_hash }
        | StateChanged { info_hash, .. }
        | MetadataReceived { info_hash, .. }
        | MetadataFailed { info_hash }
        | TorrentChecked { info_hash, .. }
        | CheckingProgress { info_hash, .. }
        | PieceFinished { info_hash, .. }
        | BlockFinished { info_hash, .. }
        | HashFailed { info_hash, .. }
        | PeerConnected { info_hash, .. }
        | PeerDisconnected { info_hash, .. }
        | PeerBanned { info_hash, .. }
        | TrackerReply { info_hash, .. }
        | TrackerWarning { info_hash, .. }
        | TrackerError { info_hash, .. }
        | ScrapeReply { info_hash, .. }
        | ScrapeError { info_hash, .. }
        | DhtGetPeers { info_hash, .. }
        | FileCompleted { info_hash, .. }
        | FileRenamed { info_hash, .. }
        | StorageMoved { info_hash, .. }
        | FileError { info_hash, .. }
        | ResumeDataSaved { info_hash }
        | TorrentError { info_hash, .. }
        | PerformanceWarning { info_hash, .. }
        | TorrentQueuePositionChanged { info_hash, .. }
        | TorrentAutoManaged { info_hash, .. }
        | WebSeedBanned { info_hash, .. }
        | HolepunchSucceeded { info_hash, .. }
        | HolepunchFailed { info_hash, .. }
        | PeerTurnover { info_hash, .. }
        | SslTorrentError { info_hash, .. }
        | InconsistentHashes { info_hash, .. } => Some(info_hash),
        _ => None,
    }
}

/// Background loop that subscribes to irontide alerts and emits events,
/// with periodic progress emission every 2 seconds for all active torrents.
async fn alert_bridge_loop(
    session: irontide::session::SessionHandle,
    event_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
    task_map: Arc<DashMap<String, Id20>>,
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
) {
    use irontide::session::AlertKind;

    let mut rx = session.subscribe();
    let mut progress_timer = tokio::time::interval(Duration::from_secs(2));
    progress_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::info!("irontide alert bridge started");

    loop {
        tokio::select! {
            alert = rx.recv() => {
                let alert = match alert {
                    Ok(a) => a,
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("irontide alert bridge stopped (channel closed)");
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("irontide alert bridge lagged by {n} messages");
                        continue;
                    }
                };

                // Try to extract info_hash — many AlertKind variants carry it.
                let info_hash = extract_info_hash(&alert.kind);
                let Some(info_hash) = info_hash else {
                    continue;
                };

                let task_id = format!("{}{}", BT_PREFIX, info_hash.to_hex());

                match &alert.kind {
                    AlertKind::TorrentAdded { .. } => {
                        if !task_map.contains_key(&task_id) {
                            task_map.insert(task_id.clone(), *info_hash);
                        }
                        emit_alert_event(&app_handle, &event_tx, "aria2.onDownloadStart", &task_id);
                    }
                    AlertKind::TorrentRemoved { .. } => {
                        task_map.remove(&task_id);
                    }
                    AlertKind::TorrentPaused { .. } => {
                        emit_alert_event(&app_handle, &event_tx, "aria2.onDownloadPause", &task_id);
                    }
                    AlertKind::TorrentResumed { .. } => {
                        emit_alert_event(&app_handle, &event_tx, "aria2.onDownloadStart", &task_id);
                    }
                    AlertKind::TorrentFinished { .. } => {
                        emit_alert_event(&app_handle, &event_tx, "aria2.onDownloadComplete", &task_id);
                        emit_alert_event(
                            &app_handle,
                            &event_tx,
                            "aria2.onBtDownloadComplete",
                            &task_id,
                        );

                        // Fetch stats OUTSIDE the app_handle lock so the guard drops before .await
                        let stats = session.torrent_stats(*info_hash).await.ok();
                        if let Some(ref app) = *lock(&app_handle) {
                            let _ = app.emit("download-completed", serde_json::json!({"id": task_id}));
                            if let Some(ref s) = stats {
                                let progress = serde_json::json!({
                                    "id": task_id,
                                    "state": "completed",
                                    "progress": 1.0,
                                    "downloadedBytes": s.total_done,
                                    "totalBytes": s.total,
                                });
                                let _ = app.emit("download-progress", &progress);
                            }
                            // Emit download-updated so the frontend gets the full summary update
                            let updated = serde_json::json!({
                                "id": task_id,
                                "state": "completed",
                                "uploadStatus": "idle",
                            });
                            let _ = app.emit("download-updated", &updated);
                        }
                    }
                    AlertKind::MetadataReceived { name, .. } => {
                        tracing::debug!("irontide: metadata received for {info_hash} ({name})");
                        if let Some(ref app) = *lock(&app_handle) {
                            let summary = serde_json::json!({"id": task_id, "state": "downloading"});
                            let _ = app.emit("download-updated", &summary);
                        }
                    }
                    AlertKind::TorrentError { message, .. } => {
                        emit_alert_event(&app_handle, &event_tx, "aria2.onDownloadError", &task_id);
                        if let Some(ref app) = *lock(&app_handle) {
                            let _ = app.emit(
                                "download-error",
                                serde_json::json!({"id": task_id, "error": message}),
                            );
                        }
                    }
                    AlertKind::StateChanged { prev_state, new_state, .. } => {
                        tracing::trace!(
                            "irontide: state change for {info_hash}: {prev_state:?} -> {new_state:?}"
                        );
                    }
                    AlertKind::TorrentChecked { pieces_have, pieces_total, .. } => {
                        tracing::debug!("irontide: check complete for {info_hash} ({pieces_have}/{pieces_total})");
                    }
                    AlertKind::FileCompleted { file_index, .. } => {
                        tracing::debug!("irontide: file #{file_index} complete for {info_hash}");
                    }
                    AlertKind::TrackerReply { num_peers, url, .. } => {
                        if *num_peers > 0 {
                            if let Some(ref app) = *lock(&app_handle) {
                                let _ = app.emit(
                                    "tracker-info",
                                    serde_json::json!({"id": task_id, "tracker": url, "peers": num_peers}),
                                );
                            }
                        }
                    }
                    AlertKind::TrackerError { message, url, .. } => {
                        tracing::warn!("irontide: tracker error for {url}: {message}");
                    }
                    AlertKind::TrackerWarning { message, url, .. } => {
                        tracing::warn!("irontide: tracker warning for {url}: {message}");
                    }
                    AlertKind::HashFailed { piece, .. } => {
                        tracing::warn!("irontide: hash check failed for {info_hash} piece {piece}");
                    }
                    AlertKind::PeerConnected { addr, .. } => {
                        tracing::trace!("irontide: peer connected {addr}");
                    }
                    AlertKind::PeerDisconnected { addr, .. } => {
                        tracing::trace!("irontide: peer disconnected {addr}");
                    }
                    AlertKind::StorageMoved { new_path, .. } => {
                        tracing::info!("irontide: storage moved to {}", new_path.display());
                    }
                    AlertKind::FileError { path, message, .. } => {
                        tracing::warn!("irontide: file error at {}: {message}", path.display());
                    }
                    // Session stats / non-torrent alerts — ignore.
                    _ => {}
                }
            }
            _ = progress_timer.tick() => {
                // Periodic progress emission for all active torrents.
                let hashes: Vec<Id20> = task_map.iter().map(|e| *e.value()).collect();
                for info_hash in hashes {
                    if let Ok(stats) = session.torrent_stats(info_hash).await {
                        let task_id = format!("{}{}", BT_PREFIX, info_hash.to_hex());

                        // Build a compact progress JSON for the frontend.
                        let progress = serde_json::json!({
                            "id": task_id,
                            "state": match &stats.state {
                                irontide::session::TorrentState::Downloading => "downloading",
                                irontide::session::TorrentState::Seeding
                                | irontide::session::TorrentState::Complete => "completed",
                                irontide::session::TorrentState::Paused => "paused",
                                irontide::session::TorrentState::Checking => "verifying",
                                irontide::session::TorrentState::FetchingMetadata
                                | irontide::session::TorrentState::Queued => "queued",
                                irontide::session::TorrentState::Stopped => "canceled",
                                irontide::session::TorrentState::Sharing => "downloading",
                            },
                            "progress": if stats.total > 0 {
                                stats.total_done as f64 / stats.total as f64
                            } else {
                                0.0
                            },
                            "downloadedBytes": stats.total_done,
                            "totalBytes": stats.total,
                            "downloadSpeed": stats.download_payload_rate,
                            "uploadSpeed": stats.upload_payload_rate,
                            "uploadedBytes": stats.uploaded,
                            "connectedPeers": stats.peers_connected,
                        });

                        if let Some(ref app) = *lock(&app_handle) {
                            let _ = app.emit("download-progress", &progress);
                        }
                    }
                }
            }
        }
    }

    tracing::info!("irontide alert bridge stopped");
}

/// Helper: emit an event to both the Aria2 RPC channel and the Tauri frontend.
fn emit_alert_event(
    app_handle: &Arc<Mutex<Option<tauri::AppHandle>>>,
    event_tx: &Arc<Mutex<Option<broadcast::Sender<String>>>>,
    method: &str,
    task_id: &str,
) {
    // Aria2 RPC broadcast
    if let Some(ref tx) = *lock(event_tx) {
        let gid = super::aria2_rpc::internal_id_to_gid(task_id);
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": [{"gid": gid}]
        });
        let _ = tx.send(serde_json::to_string(&payload).unwrap_or_default());
    }

    // Tauri frontend event
    if let Some(ref app) = *lock(app_handle) {
        let _ = app.emit("download-updated", serde_json::json!({"id": task_id}));
    }
}

// ---------------------------------------------------------------------------
//  Upload policy loop
// ---------------------------------------------------------------------------

/// Background loop that periodically enforces upload limits per-torrent.
async fn upload_policy_loop(
    session: irontide::session::SessionHandle,
    bt_settings: Arc<Mutex<super::types::BtSettings>>,
    task_map: Arc<DashMap<String, Id20>>,
    app_handle: Arc<Mutex<Option<tauri::AppHandle>>>,
    paused_by_limit: Arc<DashMap<Id20, ()>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    tracing::info!("irontide upload policy loop started");

    loop {
        interval.tick().await;

        let settings = lock(&bt_settings).clone();
        if settings.upload_limit_bytes == 0 && settings.upload_ratio_limit == 0.0 {
            // If limits were cleared, un-pause any previously paused torrents.
            if !paused_by_limit.is_empty() {
                let to_unpause: Vec<Id20> =
                    paused_by_limit.iter().map(|e| *e.key()).collect();
                paused_by_limit.clear();
                for ih in &to_unpause {
                    let _ = session.set_upload_limit(*ih, 0).await;
                    // Emit a download-updated to reflect unpaused upload status
                    emit_upload_policy_event(&app_handle, *ih, "idle");
                }
            }
            continue;
        }

        for entry in task_map.iter() {
            let info_hash = *entry.value();
            match session.torrent_stats(info_hash).await {
                Ok(stats) => {
                    let limit_reached = settings.upload_limit_bytes > 0
                        && stats.uploaded >= settings.upload_limit_bytes;
                    let ratio_reached = settings.upload_ratio_limit > 0.0
                        && stats.total_done > 0
                        && (stats.uploaded as f64)
                            >= stats.total_done as f64 * settings.upload_ratio_limit;

                    if limit_reached || ratio_reached {
                        if settings.pause_upload_when_limit_reached {
                            if paused_by_limit.get(&info_hash).is_none() {
                                paused_by_limit.insert(info_hash, ());
                                let _ = session.set_upload_limit(info_hash, 1).await;
                                // Emit a download-updated reflecting PausedByLimit
                                emit_upload_policy_event(&app_handle, info_hash, "paused_by_limit");
                            }
                        }
                        // When pause_upload_when_limit_reached is false, we do NOT
                        // modify the upload rate — upload_limit_bytes is an absolute
                        // byte threshold, not a rate to enforce.
                    } else if paused_by_limit.get(&info_hash).is_some() {
                        // Was previously paused; un-pause by removing the rate cap.
                        // irontide treats 0 as unlimited.
                        paused_by_limit.remove(&info_hash);
                        let _ = session.set_upload_limit(info_hash, 0).await;
                        emit_upload_policy_event(&app_handle, info_hash, "idle");
                    }
                }
                Err(e) => {
                    tracing::trace!("upload policy: stats error for {info_hash}: {e}");
                }
            }
        }
    }
}

/// Emit a `download-updated` event from the upload policy loop with the given upload status.
fn emit_upload_policy_event(
    app_handle: &Arc<Mutex<Option<tauri::AppHandle>>>,
    info_hash: Id20,
    upload_status: &str,
) {
    if let Some(ref app) = *lock(app_handle) {
        let task_id = format!("{}{}", BT_PREFIX, info_hash.to_hex());
        let _ = app.emit(
            "download-updated",
            serde_json::json!({"id": task_id, "uploadStatus": upload_status}),
        );
    }
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::str::FromStr;

    // ── map_state ──────────────────────────────────────────────────────────

    #[test]
    fn test_map_state_downloading() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Downloading),
            DownloadState::Downloading
        );
    }

    #[test]
    fn test_map_state_seeding() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Seeding),
            DownloadState::Completed
        );
    }

    #[test]
    fn test_map_state_complete() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Complete),
            DownloadState::Completed
        );
    }

    #[test]
    fn test_map_state_paused() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Paused),
            DownloadState::Paused
        );
    }

    #[test]
    fn test_map_state_checking() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Checking),
            DownloadState::Verifying
        );
    }

    #[test]
    fn test_map_state_fetching_metadata() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::FetchingMetadata),
            DownloadState::Queued
        );
    }

    #[test]
    fn test_map_state_queued() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Queued),
            DownloadState::Queued
        );
    }

    #[test]
    fn test_map_state_stopped() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Stopped),
            DownloadState::Canceled
        );
    }

    #[test]
    fn test_map_state_sharing() {
        assert_eq!(
            map_state(&irontide::session::TorrentState::Sharing),
            DownloadState::Downloading
        );
    }

    // ── build_peer_flags ───────────────────────────────────────────────────

    fn make_peer() -> irontide::session::PeerInfo {
        irontide::session::PeerInfo {
            addr: SocketAddr::from_str("127.0.0.1:6881").unwrap(),
            client: String::new(),
            peer_choking: false,
            peer_interested: false,
            am_choking: false,
            am_interested: false,
            download_rate: 0,
            upload_rate: 0,
            num_pieces: 0,
            source: irontide::session::PeerSource::Tracker,
            supports_fast: false,
            upload_only: false,
            snubbed: false,
            connected_duration_secs: 0,
            num_pending_requests: 0,
            num_incoming_requests: 0,
            is_optimistic: false,
            is_encrypted: false,
            uses_utp: false,
            uses_holepunch: false,
            in_flight_requests: 0,
            target_pipeline_depth: 0,
            relevance: 0.0,
            connection_kind: irontide::session::PeerConnectionKind::Tcp,
            progress: 0.0,
            country_code: None,
        }
    }

    #[test]
    fn test_build_peer_flags_empty() {
        let peer = make_peer();
        assert_eq!(build_peer_flags(&peer), "");
    }

    #[test]
    fn test_build_peer_flags_all() {
        let mut peer = make_peer();
        peer.is_encrypted = true;
        peer.uses_utp = true;
        peer.supports_fast = true;
        peer.upload_only = true;
        peer.snubbed = true;
        peer.am_choking = true;
        peer.peer_interested = true;
        assert_eq!(build_peer_flags(&peer), "EuFUScI");
    }

    #[test]
    fn test_build_peer_flags_encrypted() {
        let mut peer = make_peer();
        peer.is_encrypted = true;
        assert_eq!(build_peer_flags(&peer), "E");
    }

    #[test]
    fn test_build_peer_flags_utp() {
        let mut peer = make_peer();
        peer.uses_utp = true;
        assert_eq!(build_peer_flags(&peer), "u");
    }

    #[test]
    fn test_build_peer_flags_fast() {
        let mut peer = make_peer();
        peer.supports_fast = true;
        assert_eq!(build_peer_flags(&peer), "F");
    }

    #[test]
    fn test_build_peer_flags_upload_only() {
        let mut peer = make_peer();
        peer.upload_only = true;
        assert_eq!(build_peer_flags(&peer), "U");
    }

    #[test]
    fn test_build_peer_flags_snubbed() {
        let mut peer = make_peer();
        peer.snubbed = true;
        assert_eq!(build_peer_flags(&peer), "S");
    }

    #[test]
    fn test_build_peer_flags_am_choking() {
        let mut peer = make_peer();
        peer.am_choking = true;
        assert_eq!(build_peer_flags(&peer), "c");
    }

    #[test]
    fn test_build_peer_flags_interested() {
        let mut peer = make_peer();
        peer.peer_interested = true;
        assert_eq!(build_peer_flags(&peer), "I");
    }

    #[test]
    fn test_build_peer_flags_combination() {
        let mut peer = make_peer();
        peer.is_encrypted = true;
        peer.supports_fast = true;
        peer.am_choking = true;
        // E + F + c
        assert_eq!(build_peer_flags(&peer), "EFc");
    }

    // ── estimate_eta ───────────────────────────────────────────────────────

    #[test]
    fn test_estimate_eta_normal() {
        assert_eq!(estimate_eta(1000, 500, Some(100.0)), Some(5));
    }

    #[test]
    fn test_estimate_eta_zero_speed() {
        assert_eq!(estimate_eta(1000, 500, Some(0.0)), None);
    }

    #[test]
    fn test_estimate_eta_completed() {
        assert_eq!(estimate_eta(1000, 1000, Some(100.0)), None);
    }

    #[test]
    fn test_estimate_eta_over_downloaded() {
        assert_eq!(estimate_eta(1000, 1500, Some(100.0)), None);
    }

    #[test]
    fn test_estimate_eta_none_speed() {
        assert_eq!(estimate_eta(1000, 500, None), None);
    }

    #[test]
    fn test_estimate_eta_small_speed() {
        // 1 byte remaining at 0.5 B/s => ceil(1.0 / 0.5) = 2
        assert_eq!(estimate_eta(1000, 999, Some(0.5)), Some(2));
    }

    #[test]
    fn test_estimate_eta_exact_division() {
        // 100 bytes remaining at 50 B/s => 2 seconds
        assert_eq!(estimate_eta(200, 100, Some(50.0)), Some(2));
    }

    // ── extract_info_hash ──────────────────────────────────────────────────

    #[test]
    fn test_extract_info_hash_torrent_added() {
        let ih = Id20::from([1u8; 20]);
        let kind = irontide::session::AlertKind::TorrentAdded {
            info_hash: ih,
            name: "test".into(),
        };
        assert_eq!(extract_info_hash(&kind), Some(&ih));
    }

    #[test]
    fn test_extract_info_hash_torrent_finished() {
        let ih = Id20::from([2u8; 20]);
        let kind = irontide::session::AlertKind::TorrentFinished { info_hash: ih };
        assert_eq!(extract_info_hash(&kind), Some(&ih));
    }

    #[test]
    fn test_extract_info_hash_torrent_paused() {
        let ih = Id20::from([3u8; 20]);
        let kind = irontide::session::AlertKind::TorrentPaused { info_hash: ih };
        assert_eq!(extract_info_hash(&kind), Some(&ih));
    }

    #[test]
    fn test_extract_info_hash_state_changed() {
        let ih = Id20::from([4u8; 20]);
        let kind = irontide::session::AlertKind::StateChanged {
            info_hash: ih,
            prev_state: irontide::session::TorrentState::Downloading,
            new_state: irontide::session::TorrentState::Seeding,
        };
        assert_eq!(extract_info_hash(&kind), Some(&ih));
    }

    #[test]
    fn test_extract_info_hash_tracker_reply() {
        let ih = Id20::from([5u8; 20]);
        let kind = irontide::session::AlertKind::TrackerReply {
            info_hash: ih,
            url: "http://tracker.example.com/announce".into(),
            num_peers: 10,
        };
        assert_eq!(extract_info_hash(&kind), Some(&ih));
    }

    #[test]
    fn test_extract_info_hash_session_stats_update() {
        // SessionStatsUpdate is a tuple variant without info_hash
        let stats = irontide::session::SessionStats {
            active_torrents: 0,
            total_downloaded: 0,
            total_uploaded: 0,
            dht_nodes: 0,
            external_address: None,
            incoming_peer_connections: 0,
        };
        let kind = irontide::session::AlertKind::SessionStatsUpdate(stats);
        assert_eq!(extract_info_hash(&kind), None);
    }

    #[test]
    fn test_extract_info_hash_settings_changed() {
        // SettingsChanged is a unit variant with no fields at all
        let kind = irontide::session::AlertKind::SettingsChanged;
        assert_eq!(extract_info_hash(&kind), None);
    }

    #[test]
    fn test_extract_info_hash_listen_succeeded() {
        // ListenSucceeded has port but no info_hash
        let kind = irontide::session::AlertKind::ListenSucceeded { port: 6881 };
        assert_eq!(extract_info_hash(&kind), None);
    }

    #[test]
    fn test_extract_info_hash_dht_bootstrap() {
        // DhtBootstrapComplete is unit, no info_hash
        let kind = irontide::session::AlertKind::DhtBootstrapComplete;
        assert_eq!(extract_info_hash(&kind), None);
    }

    #[test]
    fn test_extract_info_hash_peer_blocked() {
        // PeerBlocked has addr but no info_hash
        let kind = irontide::session::AlertKind::PeerBlocked {
            addr: SocketAddr::from_str("10.0.0.1:6881").unwrap(),
        };
        assert_eq!(extract_info_hash(&kind), None);
    }

    // ── v1_file_entries / preview_entries_from_meta ────────────────────────

    #[test]
    fn test_v1_file_entries_single_file() {
        let info = irontide::core::InfoDict {
            name: "ubuntu.iso".into(),
            piece_length: 262144,
            pieces: vec![0u8; 20],
            length: Some(1_000_000_000),
            files: None,
            private: None,
            source: None,
            ssl_cert: None,
            similar: vec![],
            collections: vec![],
        };
        let v1 = irontide::core::TorrentMetaV1 {
            info_hash: Id20::from([0u8; 20]),
            announce: None,
            announce_list: None,
            comment: None,
            created_by: None,
            creation_date: None,
            info,
            url_list: vec![],
            httpseeds: vec![],
            info_bytes: None,
            ssl_cert: None,
        };
        let entries = v1_file_entries(&v1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].path, "ubuntu.iso");
        assert_eq!(entries[0].size, 1_000_000_000);
    }

    #[test]
    fn test_v1_file_entries_multi_file() {
        let files = vec![
            irontide::core::FileEntry {
                length: 500,
                path: vec!["dir".into(), "file1.txt".into()],
                attr: None,
                mtime: None,
                symlink_path: None,
            },
            irontide::core::FileEntry {
                length: 1200,
                path: vec!["file2.txt".into()],
                attr: None,
                mtime: None,
                symlink_path: None,
            },
        ];
        let info = irontide::core::InfoDict {
            name: "mydir".into(),
            piece_length: 16384,
            pieces: vec![0u8; 20],
            length: None,
            files: Some(files),
            private: None,
            source: None,
            ssl_cert: None,
            similar: vec![],
            collections: vec![],
        };
        let v1 = irontide::core::TorrentMetaV1 {
            info_hash: Id20::from([0u8; 20]),
            announce: None,
            announce_list: None,
            comment: None,
            created_by: None,
            creation_date: None,
            info,
            url_list: vec![],
            httpseeds: vec![],
            info_bytes: None,
            ssl_cert: None,
        };
        let entries = v1_file_entries(&v1);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].path, "dir/file1.txt");
        assert_eq!(entries[0].size, 500);
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].path, "file2.txt");
        assert_eq!(entries[1].size, 1200);
    }

    #[test]
    fn test_v1_file_entries_empty_file_list() {
        // A torrent with no files (unusual but code handles it)
        let info = irontide::core::InfoDict {
            name: "empty".into(),
            piece_length: 16384,
            pieces: vec![0u8; 20],
            length: Some(0),
            files: None,
            private: None,
            source: None,
            ssl_cert: None,
            similar: vec![],
            collections: vec![],
        };
        let v1 = irontide::core::TorrentMetaV1 {
            info_hash: Id20::from([0u8; 20]),
            announce: None,
            announce_list: None,
            comment: None,
            created_by: None,
            creation_date: None,
            info,
            url_list: vec![],
            httpseeds: vec![],
            info_bytes: None,
            ssl_cert: None,
        };
        let entries = v1_file_entries(&v1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].path, "empty");
        assert_eq!(entries[0].size, 0);
    }

    #[test]
    fn test_preview_entries_from_meta_v1() {
        // Delegates to v1_file_entries, so a single smoke test suffices
        let info = irontide::core::InfoDict {
            name: "test.iso".into(),
            piece_length: 16384,
            pieces: vec![0u8; 20],
            length: Some(42),
            files: None,
            private: None,
            source: None,
            ssl_cert: None,
            similar: vec![],
            collections: vec![],
        };
        let v1 = irontide::core::TorrentMetaV1 {
            info_hash: Id20::from([0u8; 20]),
            announce: None,
            announce_list: None,
            comment: None,
            created_by: None,
            creation_date: None,
            info,
            url_list: vec![],
            httpseeds: vec![],
            info_bytes: None,
            ssl_cert: None,
        };
        let meta = irontide::core::TorrentMeta::V1(v1);
        let entries = preview_entries_from_meta(&meta);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "test.iso");
        assert_eq!(entries[0].size, 42);
    }
}
