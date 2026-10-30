use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::{AddTorrent, AddTorrentOptions};

use super::{PendingMagnet, RqbitBtBackend};
use crate::error::{DownloadError, Result, io_error_with_path};
use crate::slot_guard::DownloadSlotGuard;
use crate::types::{
    BtUploadStatus, ChecksumMode, DownloadSnapshot, DownloadState, DownloadSummary, Priority,
    StartDownloadRequest, TaskKind, ThreadMode,
};
use crate::{lock, now_ms};

impl RqbitBtBackend {
    // ── Private helpers ────────────────────────────────────────────────

    /// Fetch bytes from a URL using the configured HTTP client (with proxy support).
    pub async fn fetch_url_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let client = { self.http_client.read().clone() };
        let resp = client.get(url).send().await.map_err(|e| {
            DownloadError::TorrentNetwork(format!("failed to fetch .torrent URL {url}: {e}"))
        })?;

        if !resp.status().is_success() {
            return Err(DownloadError::TorrentNetwork(format!(
                "HTTP {} fetching .torrent URL {url}",
                resp.status()
            )));
        }

        let bytes = resp.bytes().await.map_err(|e| {
            DownloadError::TorrentNetwork(format!("failed to read .torrent response {url}: {e}"))
        })?;

        Ok(bytes.to_vec())
    }

    /// Try to acquire a slot for a new BT download.
    fn try_acquire_bt_slot(&self) -> Result<DownloadSlotGuard> {
        let max = self.max_concurrent_bt.load(Ordering::Acquire);
        let counter = &self.active_bt_count;
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                return Err(DownloadError::TooManyConcurrentDownloads);
            }
            if counter
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(DownloadSlotGuard::new(self.active_bt_count.clone()));
            }
        }
    }

    // ── Download operations ───────────────────────────────────────────────

    pub async fn start(&self, request: StartDownloadRequest) -> Result<Id20> {
        let source = request.url.trim();
        if source.is_empty() {
            return Err(DownloadError::InvalidResponse(
                "torrent source is empty".into(),
            ));
        }

        // Determine effective download directory
        let dest_dir = if request.destination_dir.trim().is_empty() {
            self.default_output_dir.clone()
        } else {
            let p = PathBuf::from(request.destination_dir.trim());
            if !p.is_absolute() {
                return Err(DownloadError::InvalidResponse(
                    "download destination directory must be an absolute path".into(),
                ));
            }
            tokio::fs::create_dir_all(&p)
                .await
                .map_err(|e| io_error_with_path(e, p.to_string_lossy()))?;
            p
        };

        let is_magnet = source.to_ascii_lowercase().starts_with("magnet:")
            || (source.len() == 40 && source.chars().all(|c| c.is_ascii_hexdigit()));

        let mut opts = AddTorrentOptions {
            output_folder: Some(dest_dir.to_string_lossy().to_string()),
            paused: request.start_paused,
            overwrite: true,
            ..Default::default()
        };

        if let Some(indices) = request.selected_file_indices {
            opts.only_files = Some(indices.into_iter().collect());
        }

        // Apply custom trackers from user settings
        let bt_settings = lock(&self.bt_settings).clone();
        if !bt_settings.tracker_list.trim().is_empty() {
            let trackers: Vec<String> = bt_settings
                .tracker_list
                .lines()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            if !trackers.is_empty() {
                opts.trackers = Some(trackers);
            }
        }

        if is_magnet {
            let magnet_str = if source.len() == 40 && source.chars().all(|c| c.is_ascii_hexdigit())
            {
                format!("magnet:?xt=urn:btih:{source}")
            } else {
                source.to_string()
            };

            let magnet = librqbit::Magnet::parse(&magnet_str).map_err(|e| {
                DownloadError::TorrentInvalidData(format!("invalid magnet link: {e}"))
            })?;
            let info_hash = magnet.as_id20().ok_or_else(|| {
                DownloadError::TorrentInvalidData("magnet link missing BTv1 infohash".into())
            })?;

            if self.task_map.contains_key(&info_hash) {
                return Ok(info_hash);
            }

            let _guard = self.try_acquire_bt_slot()?;
            let now = now_ms();
            self.task_map.insert(info_hash, info_hash);
            self.torrent_created_at.insert(info_hash, now);
            self.bt_slot_guards.insert(info_hash, _guard);

            let paused = Arc::new(AtomicBool::new(request.start_paused));
            let session = self.session.clone();
            let pending_map = self.pending_magnets.clone();
            let paused_flag = paused.clone();
            let magnet_source = magnet_str.clone();

            let bg_task = tokio::spawn(async move {
                let add_torrent = AddTorrent::from_url(magnet_source);
                match session.add_torrent(add_torrent, Some(opts)).await {
                    Ok(res) => {
                        pending_map.remove(&info_hash);
                        if let Some(h) = res.into_handle()
                            && paused_flag.load(Ordering::Acquire)
                        {
                            let _ = session.pause(&h).await;
                        }
                    }
                    Err(e) => {
                        let ih_str = info_hash.as_string();
                        tracing::debug!("magnet resolution ended for {ih_str}: {e}");
                    }
                }
            });

            self.pending_magnets.insert(
                info_hash,
                PendingMagnet {
                    info_hash,
                    magnet_url: magnet_str,
                    name: magnet.name.unwrap_or_else(|| info_hash.as_string()),
                    destination_path: dest_dir.to_string_lossy().to_string(),
                    created_at_ms: now,
                    paused,
                    abort_handle: bg_task.abort_handle(),
                },
            );

            self.emit_pending_summary(info_hash);
            tracing::info!("librqbit: started magnet {}", info_hash.as_string());
            return Ok(info_hash);
        }

        let bytes = if source.starts_with("http://") || source.starts_with("https://") {
            self.fetch_url_bytes(source).await?
        } else {
            tokio::fs::read(source).await.map_err(|e| {
                DownloadError::TorrentIo(format!("failed to read torrent file: {e}"))
            })?
        };

        let add_torrent = AddTorrent::from_bytes(bytes);

        // Acquire concurrent download slot
        let _guard = self.try_acquire_bt_slot()?;

        let res = self
            .session
            .add_torrent(add_torrent, Some(opts))
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        let handle = res
            .into_handle()
            .ok_or_else(|| DownloadError::Torrent("failed to add torrent".into()))?;

        let info_hash = handle.info_hash();

        self.task_map.insert(info_hash, info_hash);
        self.torrent_created_at.insert(info_hash, now_ms());
        self.bt_slot_guards.insert(info_hash, _guard);

        self.emit_pending_summary(info_hash);

        tracing::info!("librqbit: started torrent {}", info_hash.as_string());
        Ok(info_hash)
    }

    pub async fn pause(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        if let Some(handle) = self.session.get(TorrentIdOrHash::Hash(info_hash)) {
            self.session
                .pause(&handle)
                .await
                .map_err(|e| DownloadError::Torrent(e.to_string()))?;
            let snapshot = self.status(info_hash).await?;
            return Ok(DownloadSnapshot {
                state: DownloadState::Paused,
                speed_bytes_per_second: None,
                upload_speed_bytes_per_second: None,
                upload_status: Some(BtUploadStatus::Paused),
                updated_at_ms: now_ms(),
                ..snapshot
            });
        }

        if let Some(pending) = self.pending_magnets.get(&info_hash) {
            pending.paused.store(true, Ordering::Release);
            return Ok(self.pending_snapshot(&pending, DownloadState::Paused));
        }

        Err(DownloadError::NotFound)
    }

    pub async fn resume(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        if let Some(handle) = self.session.get(TorrentIdOrHash::Hash(info_hash)) {
            self.session
                .unpause(&handle)
                .await
                .map_err(|e| DownloadError::Torrent(e.to_string()))?;
            let snapshot = self.status(info_hash).await?;
            return Ok(DownloadSnapshot {
                updated_at_ms: now_ms(),
                ..snapshot
            });
        }

        if let Some(pending) = self.pending_magnets.get(&info_hash) {
            pending.paused.store(false, Ordering::Release);
            return Ok(self.pending_snapshot(&pending, DownloadState::Queued));
        }

        Err(DownloadError::NotFound)
    }

    pub async fn cancel(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        let snapshot = match self.status(info_hash).await {
            Ok(s) => DownloadSnapshot {
                state: DownloadState::Canceled,
                speed_bytes_per_second: None,
                upload_speed_bytes_per_second: None,
                upload_status: None,
                updated_at_ms: now_ms(),
                ..s
            },
            Err(_) => build_canceled_snapshot(&info_hash),
        };

        if let Some((_, p)) = self.pending_magnets.remove(&info_hash) {
            p.abort_handle.abort();
        }
        let _ = self
            .session
            .delete(TorrentIdOrHash::Hash(info_hash), false)
            .await;

        self.bt_slot_guards.remove(&info_hash);
        self.task_map.remove(&info_hash);
        self.torrent_created_at.remove(&info_hash);
        Ok(snapshot)
    }

    pub async fn remove(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        let snapshot = self.status(info_hash).await?;
        if let Some((_, p)) = self.pending_magnets.remove(&info_hash) {
            p.abort_handle.abort();
        }
        let _ = self
            .session
            .delete(TorrentIdOrHash::Hash(info_hash), false)
            .await;

        self.bt_slot_guards.remove(&info_hash);
        self.task_map.remove(&info_hash);
        self.torrent_created_at.remove(&info_hash);
        Ok(snapshot)
    }

    pub async fn purge(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        let snapshot = self.status(info_hash).await?;
        if let Some((_, p)) = self.pending_magnets.remove(&info_hash) {
            p.abort_handle.abort();
        }
        let _ = self
            .session
            .delete(TorrentIdOrHash::Hash(info_hash), true)
            .await;

        self.bt_slot_guards.remove(&info_hash);
        self.task_map.remove(&info_hash);
        self.torrent_created_at.remove(&info_hash);
        Ok(snapshot)
    }

    pub async fn open_in_explorer(&self, info_hash: Id20) -> Result<()> {
        let snapshot = self.status(info_hash).await?;
        let path = PathBuf::from(&snapshot.destination_path);
        if path.exists() {
            crate::platform::reveal_in_file_manager(&path)?;
            return Ok(());
        }
        Err(DownloadError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "download location does not exist",
        )))
    }

    pub async fn open_file(&self, info_hash: Id20) -> Result<()> {
        let snapshot = self.status(info_hash).await?;
        let path = PathBuf::from(&snapshot.destination_path);
        if path.is_dir() {
            return Err(DownloadError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot open a multi-file torrent as a single file. Use Open Folder instead.",
            )));
        }
        if !path.exists() {
            return Err(DownloadError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "download file does not exist",
            )));
        }
        open::that(&path).map_err(|e| DownloadError::Io(std::io::Error::other(e.to_string())))?;
        Ok(())
    }

    pub async fn open_dir(&self, info_hash: Id20) -> Result<()> {
        let snapshot = self.status(info_hash).await?;
        let path = PathBuf::from(&snapshot.destination_path);
        let dir = if path.is_file() {
            path.parent().map(PathBuf::from).unwrap_or(path)
        } else {
            path
        };
        crate::platform::open_in_file_manager(&dir).map_err(DownloadError::Io)?;
        Ok(())
    }

    pub async fn status(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        if let Some(handle) = self.session.get(TorrentIdOrHash::Hash(info_hash)) {
            let stats = handle.stats();
            return Ok(self.stats_to_snapshot(&info_hash, &stats));
        }

        if let Some(pending) = self.pending_magnets.get(&info_hash) {
            let state = if pending.paused.load(Ordering::Acquire) {
                DownloadState::Paused
            } else {
                DownloadState::Queued
            };
            return Ok(self.pending_snapshot(&pending, state));
        }

        Err(DownloadError::NotFound)
    }

    fn pending_snapshot(&self, pending: &PendingMagnet, state: DownloadState) -> DownloadSnapshot {
        let now = now_ms();
        DownloadSnapshot {
            id: pending.info_hash.as_string(),
            kind: TaskKind::Bt,
            state,
            created_at_ms: pending.created_at_ms,
            updated_at_ms: now,
            url: pending.magnet_url.clone(),
            final_url: pending.magnet_url.clone(),
            file_name: pending.name.clone(),
            destination_path: pending.destination_path.clone(),
            temp_path: self.state_dir.to_string_lossy().to_string(),
            total_bytes: None,
            downloaded_bytes: 0,
            supports_ranges: false,
            connection_count: 0,
            thread_mode: ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: Some("BT task resolving metadata".into()),
            checksum: None,
            expected_checksum: None,
            checksum_mode: ChecksumMode::None,
            etag: None,
            last_modified: None,
            error: None,
            speed_bytes_per_second: None,
            eta_seconds: None,
            uploaded_bytes: Some(0),
            upload_speed_bytes_per_second: None,
            peer_count: None,
            upload_status: Some(if state == DownloadState::Paused {
                BtUploadStatus::Paused
            } else {
                BtUploadStatus::Idle
            }),
            info_hash: Some(pending.info_hash.as_string()),
            priority: Priority::Normal,
            cdn_accelerated: false,
            cdn_node_ip: None,
            chunks: vec![],
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            mirror_url: None,
            degraded: false,
            disk_type: None,
            flushing: false,
        }
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let mut summaries = Vec::new();
        let hashes: Vec<Id20> = self.task_map.iter().map(|e| *e.key()).collect();
        for info_hash in hashes {
            if let Ok(snapshot) = self.status(info_hash).await {
                summaries.push(DownloadSummary::from(&snapshot));
            }
        }

        summaries.sort_by_key(|s| std::cmp::Reverse(s.created_at_ms));
        Ok(summaries)
    }
}

// NOTE: When adding fields to DownloadSnapshot, update this function.
fn build_canceled_snapshot(info_hash: &Id20) -> DownloadSnapshot {
    let id = info_hash.as_string();
    let now = now_ms();
    DownloadSnapshot {
        id: id.clone(),
        kind: TaskKind::Bt,
        state: DownloadState::Canceled,
        created_at_ms: now,
        updated_at_ms: now,
        url: id.clone(),
        final_url: id,
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
        expected_checksum: None,
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
        info_hash: Some(info_hash.as_string()),
        priority: Priority::Normal,
        cdn_accelerated: false,
        cdn_node_ip: None,
        chunks: vec![],
        seed_count: None,
        leech_count: None,
        download_limit_bps: None,
        upload_limit_bps: None,
        mirror_url: None,
        degraded: false,
        disk_type: None,
        flushing: false,
    }
}
