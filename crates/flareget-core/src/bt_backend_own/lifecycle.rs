use std::path::PathBuf;
use std::time::Duration;

use irontide::core::Id20;
use irontide::prelude::*;

use super::IrontideBtBackend;
use crate::error::{DownloadError, Result};
use crate::types::{
    ChecksumMode, DownloadSnapshot, DownloadState, DownloadSummary,
    StartDownloadRequest, TaskKind, ThreadMode,
};
use crate::event_bus::DownloadEvent;
use crate::{lock, now_ms};

impl IrontideBtBackend {
    // ── Private helpers ────────────────────────────────────────────────

    /// Parse the info hash from a `bt:`-prefixed task ID.
    pub(crate) fn parse_info_hash(download_id: &str) -> Result<Id20> {
        let hex = download_id
            .strip_prefix(super::BT_PREFIX)
            .ok_or(DownloadError::NotFound)?;
        Id20::from_hex(hex).map_err(|_| DownloadError::NotFound)
    }

    /// Fetch bytes from a URL using the configured HTTP client (with proxy support).
    pub(crate) async fn fetch_url_bytes(&self, url: &str) -> Result<Vec<u8>> {
        if let Some(ref client) = self.http_client {
            let resp = client
                .get(url)
                .send()
                .await
                .map_err(|e| DownloadError::TorrentNetwork(format!("failed to fetch torrent: {e}")))?;
            resp.bytes()
                .await
                .map_err(|e| DownloadError::TorrentNetwork(format!("failed to read torrent bytes: {e}")))
                .map(|b| b.to_vec())
        } else {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| DownloadError::TorrentNetwork(format!("failed to build http client: {e}")))?;
            let resp = client
                .get(url)
                .send()
                .await
                .map_err(|e| DownloadError::TorrentNetwork(format!("failed to fetch torrent: {e}")))?;
            resp.bytes()
                .await
                .map_err(|e| DownloadError::TorrentNetwork(format!("failed to read torrent bytes: {e}")))
                .map(|b| b.to_vec())
        }
    }

    /// Emit an event via the EventBus Aria2 notification.
    fn emit_aria2_event(&self, method: &str, task_id: &str) {
        let gid = super::internal_id_to_gid(task_id);
        self.event_bus.publish(DownloadEvent::Aria2Notification {
            event_name: method.to_string(),
            gid,
        });
    }

    // ── Download operations ───────────────────────────────────────────────

    pub async fn start(&self, request: StartDownloadRequest) -> Result<String> {
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
                let _ = self.session.add_tracker(info_hash, tracker.to_string()).await;
            }
        }

        let task_id = format!("{}{}", super::BT_PREFIX, info_hash.to_hex());

        self.task_map.insert(task_id.clone(), info_hash);

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
                        let _ = self
                            .session
                            .set_file_priority(info_hash, i, priority)
                            .await;
                    }
        }

        // Emit a pending summary so the frontend shows the task immediately.
        self.emit_pending_summary(&task_id);

        tracing::info!("irontide: started torrent {task_id}");
        Ok(task_id)
    }

    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .pause_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        self.emit_aria2_event("aria2.onDownloadPause", download_id);
        self.status(download_id).await
    }

    pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .resume_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        self.emit_aria2_event("aria2.onDownloadStart", download_id);
        self.status(download_id).await
    }

    pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
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
        let snapshot = self.status(download_id).await.unwrap_or_else(|_| fallback_snapshot());
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

    pub async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = self.status(download_id).await?;
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .remove_torrent(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        self.task_map.remove(download_id);
        Ok(snapshot)
    }

    pub async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let snapshot = self.status(download_id).await?;
        let info_hash = Self::parse_info_hash(download_id)?;
        self.session
            .remove_torrent_with_files(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        self.task_map.remove(download_id);
        Ok(snapshot)
    }

    pub async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        let snapshot = self.status(download_id).await?;
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

    pub async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let info_hash = Self::parse_info_hash(download_id)?;
        let stats = self
            .session
            .torrent_stats(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        Ok(self.stats_to_snapshot(download_id, &info_hash, &stats))
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let info_hashes = self
            .session
            .list_torrents()
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        let mut summaries = Vec::with_capacity(info_hashes.len());
        for info_hash in &info_hashes {
            let task_id = format!("{}{}", super::BT_PREFIX, info_hash.to_hex());
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
}
