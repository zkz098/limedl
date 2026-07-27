use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use irontide::core::Id20;
use irontide::prelude::*;

use super::IrontideBtBackend;
use crate::error::{DownloadError, Result, io_error_with_path};
use crate::event_bus::DownloadEvent;
use crate::slot_guard::DownloadSlotGuard;
use crate::types::{
    ChecksumMode, DownloadSnapshot, DownloadState, DownloadSummary, Priority, StartDownloadRequest, TaskKind,
    ThreadMode,
};
use crate::{lock, now_ms};

impl IrontideBtBackend {
    // ── Private helpers ────────────────────────────────────────────────

    /// Fetch bytes from a URL using the configured HTTP client (with proxy support).
    pub(crate) async fn fetch_url_bytes(&self, url: &str) -> Result<Vec<u8>> {
        if let Some(ref client) = self.http_client {
            let resp = client.get(url).send().await.map_err(|e| {
                DownloadError::TorrentNetwork(format!("failed to fetch torrent: {e}"))
            })?;
            resp.bytes()
                .await
                .map_err(|e| {
                    DownloadError::TorrentNetwork(format!("failed to read torrent bytes: {e}"))
                })
                .map(|b| b.to_vec())
        } else {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| {
                    DownloadError::TorrentNetwork(format!("failed to build http client: {e}"))
                })?;
            let resp = client.get(url).send().await.map_err(|e| {
                DownloadError::TorrentNetwork(format!("failed to fetch torrent: {e}"))
            })?;
            resp.bytes()
                .await
                .map_err(|e| {
                    DownloadError::TorrentNetwork(format!("failed to read torrent bytes: {e}"))
                })
                .map(|b| b.to_vec())
        }
    }

    /// Emit an event via the EventBus Aria2 notification.
    fn emit_aria2_event(&self, method: &str, info_hash: &Id20) {
        let gid = super::internal_id_to_gid(info_hash);
        self.event_bus.publish(DownloadEvent::Aria2Notification {
            event_name: method.to_string(),
            gid,
        });
    }

    /// Try to acquire a BT download slot.
    /// Fails with `TooManyConcurrentDownloads` if at capacity.
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
            std::fs::create_dir_all(&p).map_err(|e| io_error_with_path(e, p.to_string_lossy()))?;
            p
        };

        // Build AddTorrentParams from source.
        let params = if source.to_ascii_lowercase().starts_with("magnet:") {
            let magnet = irontide::core::Magnet::parse(source).map_err(|e| {
                DownloadError::TorrentInvalidData(format!("invalid magnet link: {e}"))
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

        // Acquire a concurrent BT download slot (throttle)
        let _guard = self.try_acquire_bt_slot()?;

        let info_hash = params
            .add_to(&self.session)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        // Apply custom trackers from user settings
        let bt_settings = lock(&self.bt_settings).clone();
        if !bt_settings.tracker_list.trim().is_empty() {
            for tracker in bt_settings.tracker_list.lines() {
                let tracker = tracker.trim();
                if tracker.is_empty() {
                    continue;
                }
                let _ = self
                    .session
                    .add_tracker(info_hash, tracker.to_string())
                    .await;
            }
        }

        self.task_map.insert(info_hash, info_hash);
        // Store the guard so it lives for the torrent's lifetime
        self.bt_slot_guards.insert(info_hash, _guard);

        // Apply global download speed limit if configured
        if self.global_speed_limit_bps > 0 {
            let _ = self
                .session
                .set_download_limit(info_hash, self.global_speed_limit_bps)
                .await;
        }

        // Apply selected file priorities if given
        if let Some(indices) = &request.selected_file_indices
            && let Ok(files) = self.session.torrent_file(info_hash).await
            && let Some(meta) = files
        {
            let file_count = meta.info.files.map_or(1, |f| f.len());
            for i in 0..file_count {
                let priority = if indices.contains(&i) {
                    irontide::core::FilePriority::Normal
                } else {
                    irontide::core::FilePriority::Skip
                };
                let _ = self.session.set_file_priority(info_hash, i, priority).await;
            }
        }

        // Emit a pending summary so the frontend shows the task immediately.
        self.emit_pending_summary(info_hash);

        tracing::info!("irontide: started torrent {}", info_hash.to_hex());
        Ok(info_hash)
    }

    pub async fn pause(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        self.session
            .pause_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        self.emit_aria2_event("aria2.onDownloadPause", &info_hash);
        self.status(info_hash).await
    }

    pub async fn resume(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        self.session
            .resume_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        self.emit_aria2_event("aria2.onDownloadStart", &info_hash);
        self.status(info_hash).await
    }

    pub async fn cancel(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        // Try to get status, but proceed even if it fails (torrent might already be gone).
        let fallback_snapshot = || DownloadSnapshot {
            id: info_hash.to_hex(),
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
            cdn_node_ip: None,
            chunks: vec![],
            seed_count: None,
            leech_count: None,
            download_limit_bps: None,
            upload_limit_bps: None,
            mirror_url: None,
            priority: Priority::Normal,
            degraded: false,
            disk_type: None,
            flushing: false,
        };
        let snapshot = self
            .status(info_hash)
            .await
            .unwrap_or_else(|_| fallback_snapshot());
        let _ = self.session.remove_torrent(info_hash).await;
        self.bt_slot_guards.remove(&info_hash);
        self.task_map.remove(&info_hash);

        Ok(DownloadSnapshot {
            state: DownloadState::Canceled,
            updated_at_ms: now_ms(),
            ..snapshot
        })
    }

    pub async fn remove(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        let snapshot = self.status(info_hash).await?;
        self.session
            .remove_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        self.bt_slot_guards.remove(&info_hash);
        self.task_map.remove(&info_hash);
        Ok(snapshot)
    }

    pub async fn purge(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        let snapshot = self.status(info_hash).await?;
        self.session
            .remove_torrent_with_files(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        self.bt_slot_guards.remove(&info_hash);
        self.task_map.remove(&info_hash);
        Ok(snapshot)
    }

    pub async fn open_in_explorer(&self, info_hash: Id20) -> Result<()> {
        let snapshot = self.status(info_hash).await?;
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
        open::that(&path)
            .map_err(|e| DownloadError::Io(std::io::Error::other(e.to_string())))?;
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
        crate::platform::open_in_file_manager(&dir)
            .map_err(DownloadError::Io)?;        Ok(())
    }

    pub async fn status(&self, info_hash: Id20) -> Result<DownloadSnapshot> {
        let stats = self
            .session
            .torrent_stats(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        Ok(self.stats_to_snapshot(&info_hash, &stats))
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let info_hashes = self
            .session
            .list_torrents()
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        let mut summaries = Vec::with_capacity(info_hashes.len());
        for info_hash in &info_hashes {
            match self.session.torrent_stats(*info_hash).await {
                Ok(stats) => {
                    let snapshot = self.stats_to_snapshot(info_hash, &stats);
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
}
