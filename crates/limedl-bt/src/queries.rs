use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::atomic::Ordering;

use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse};

use super::RqbitBtBackend;
use super::snapshot::build_peer_flags;
use crate::error::{DownloadError, Result};
use crate::event_bus::DownloadEvent;
use crate::lock;
use crate::types::{
    BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtTrackerInfo, ChecksumMode,
    DownloadSnapshot, DownloadState, DownloadSummary, Priority, TaskKind, ThreadMode,
    TorrentFileEntry,
};

impl RqbitBtBackend {
    pub fn set_speed_limit(
        &self,
        _info_hash: Id20,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    ) {
        if let Some(bps) = download_limit_bps {
            let limit = NonZeroU32::new(bps as u32);
            self.session.ratelimits.set_download_bps(limit);
        }
        if let Some(bps) = upload_limit_bps {
            let limit = NonZeroU32::new(bps as u32);
            self.session.ratelimits.set_upload_bps(limit);
        }
    }

    pub async fn preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>> {
        let source = source.trim();
        if source.to_ascii_lowercase().starts_with("magnet:") {
            return Err(DownloadError::TorrentInvalidData(
                "cannot preview magnet link; metadata not yet available".into(),
            ));
        }

        let bytes = if source.starts_with("http://") || source.starts_with("https://") {
            self.fetch_url_bytes(source).await?
        } else {
            tokio::fs::read(source).await.map_err(|e| {
                DownloadError::TorrentIo(format!("failed to read torrent file: {e}"))
            })?
        };

        let res = self
            .session
            .add_torrent(
                AddTorrent::from_bytes(bytes),
                Some(AddTorrentOptions {
                    list_only: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| {
                DownloadError::TorrentInvalidData(format!("failed to parse torrent: {e}"))
            })?;

        let AddTorrentResponse::ListOnly(list) = res else {
            return Err(DownloadError::TorrentInvalidData(
                "unexpected response for list-only".into(),
            ));
        };

        let mut entries = Vec::new();
        for (i, f) in list.info.iter_file_details().enumerate() {
            let path = f.filename.to_string();
            entries.push(TorrentFileEntry {
                index: i,
                path,
                size: f.len,
            });
        }

        Ok(entries)
    }

    pub fn get_peers(&self, info_hash: Id20) -> Result<Vec<BtPeerInfo>> {
        if self.pending_magnets.contains_key(&info_hash) {
            return Ok(Vec::new());
        }
        if !self.task_map.contains_key(&info_hash)
            && self.session.get(TorrentIdOrHash::Hash(info_hash)).is_none()
        {
            return Err(DownloadError::NotFound);
        }
        let snapshot = match self
            .api
            .api_peer_stats(TorrentIdOrHash::Hash(info_hash), Default::default())
        {
            Ok(s) => s,
            Err(_) => {
                // If the torrent is not live (e.g. initializing, paused, or stopped),
                // it currently has no active peers.
                return Ok(Vec::new());
            }
        };

        let mut peers = Vec::with_capacity(snapshot.peers.len());
        for (addr, p) in snapshot.peers {
            peers.push(BtPeerInfo {
                address: addr,
                client: p
                    .client_name
                    .as_deref()
                    .map(sanitize_peer_client)
                    .unwrap_or("Unknown")
                    .to_string(),
                flags: build_peer_flags(p.conn_kind),
                download_speed: 0.0,
                upload_speed: 0.0,
                progress: if p.counters.fetched_bytes > 0 {
                    1.0
                } else {
                    0.0
                },
            });
        }

        Ok(peers)
    }

    pub fn get_trackers(&self, info_hash: Id20) -> Result<Vec<BtTrackerInfo>> {
        if self.pending_magnets.contains_key(&info_hash) {
            return Ok(Vec::new());
        }
        let handle = self
            .session
            .get(TorrentIdOrHash::Hash(info_hash))
            .ok_or(DownloadError::NotFound)?;

        let trackers = handle
            .shared()
            .trackers
            .iter()
            .map(|u| BtTrackerInfo { url: u.to_string() })
            .collect();

        Ok(trackers)
    }

    pub fn get_pieces(&self, info_hash: Id20) -> Result<Vec<BtPieceInfo>> {
        if self.pending_magnets.contains_key(&info_hash) {
            return Ok(Vec::new());
        }
        let handle = self
            .session
            .get(TorrentIdOrHash::Hash(info_hash))
            .ok_or_else(|| {
                if self.task_map.contains_key(&info_hash) {
                    DownloadError::Torrent("torrent not yet available".into())
                } else {
                    DownloadError::NotFound
                }
            })?;

        match self.api.api_dump_haves(TorrentIdOrHash::Hash(info_hash)) {
            Ok((bf, total)) => {
                let mut pieces = Vec::with_capacity(total as usize);
                for i in 0..total {
                    let completed = bf.get(i as usize).map(|b| *b).unwrap_or(false);
                    pieces.push(BtPieceInfo {
                        index: i as u64,
                        completed,
                    });
                }
                Ok(pieces)
            }
            Err(e) => {
                if let Some(m) = &*handle.metadata.load() {
                    let total = m.lengths().total_pieces();
                    Ok((0..total)
                        .map(|i| BtPieceInfo {
                            index: i as u64,
                            completed: false,
                        })
                        .collect())
                } else {
                    Err(DownloadError::Torrent(e.to_string()))
                }
            }
        }
    }

    pub fn get_torrent_files(&self, info_hash: Id20) -> Result<Vec<BtFileStatus>> {
        if self.pending_magnets.contains_key(&info_hash) {
            return Ok(Vec::new());
        }
        if !self.task_map.contains_key(&info_hash)
            && self.session.get(TorrentIdOrHash::Hash(info_hash)).is_none()
        {
            return Err(DownloadError::NotFound);
        }
        let details = self
            .api
            .api_torrent_details(TorrentIdOrHash::Hash(info_hash))
            .map_err(|e| DownloadError::TorrentIo(format!("failed to get torrent details: {e}")))?;

        let stats = self.api.api_stats_v1(TorrentIdOrHash::Hash(info_hash)).ok();

        let progress = stats.map(|s| s.file_progress).unwrap_or_default();

        let files = details.files.unwrap_or_default();
        let mut result = Vec::with_capacity(files.len());
        for (i, f) in files.into_iter().enumerate() {
            result.push(BtFileStatus {
                index: i,
                path: f.components.join("/"),
                size: f.length,
                downloaded_bytes: progress.get(i).copied().unwrap_or(0),
                included: f.included,
            });
        }

        Ok(result)
    }

    pub async fn update_torrent_files(
        &self,
        info_hash: Id20,
        included_indices: Vec<usize>,
    ) -> Result<()> {
        if self.pending_magnets.contains_key(&info_hash) {
            return Err(DownloadError::TorrentInvalidData(
                "cannot select files before metadata is fetched".into(),
            ));
        }
        let set: HashSet<usize> = included_indices.into_iter().collect();

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(1000);
        loop {
            match self
                .api
                .api_torrent_action_update_only_files(TorrentIdOrHash::Hash(info_hash), &set)
                .await
            {
                Ok(_) => return Ok(()),
                Err(e) if e.to_string().contains("can't update initializing torrent") => {
                    if tokio::time::Instant::now() >= deadline {
                        return Err(DownloadError::Torrent(e.to_string()));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(15)).await;
                }
                Err(e) => return Err(DownloadError::Torrent(e.to_string())),
            }
        }
    }

    pub fn runtime_status(&self) -> BtRuntimeStatus {
        let dht_enabled = lock(&self.bt_settings).dht_enabled;
        let dht_nodes = if dht_enabled {
            self.api
                .api_dht_stats()
                .map(|s| s.routing_table_size as u32)
                .unwrap_or(0)
        } else {
            0
        };

        let torrent_count = self.task_map.len();
        let (peer_count, upload_speed, uploaded) = self.session.with_torrents(|torrents| {
            let mut peer_count = 0usize;
            let mut upload_speed = 0.0;
            let mut uploaded = 0u64;
            for (_, t) in torrents {
                let stats = t.stats();
                if let Some(live) = &stats.live {
                    peer_count += live.snapshot.peer_stats.live as usize;
                    upload_speed += live.upload_speed.as_bytes() as f64;
                }
                uploaded += stats.uploaded_bytes;
            }
            (peer_count, upload_speed, uploaded)
        });

        let connected = peer_count > 0 || dht_nodes > 0 || upload_speed > 0.0;

        BtRuntimeStatus {
            connected,
            dht_enabled,
            dht_nodes: if dht_enabled {
                Some(dht_nodes as usize)
            } else {
                None
            },
            torrent_count,
            peer_count,
            upload_speed_bytes_per_second: (upload_speed > 0.0).then_some(upload_speed),
            uploaded_bytes: uploaded,
            updated_at_ms: crate::now_ms(),
            seed_count: None,
            leech_count: None,
        }
    }

    /// Whether any torrent still needs the engine running.
    ///
    /// Lightweight BT mode unloads the session when this returns `false`.
    pub async fn has_active_torrents(&self) -> Result<bool> {
        for entry in self.pending_magnets.iter() {
            if !entry.value().paused.load(Ordering::Acquire) {
                return Ok(true);
            }
        }
        let has_active = self.session.with_torrents(|torrents| {
            for (_, t) in torrents {
                let stats = t.stats();
                match stats.state {
                    librqbit::TorrentStatsState::Paused
                    | librqbit::TorrentStatsState::Initializing { paused: true } => {}
                    _ => return true,
                }
            }
            false
        });
        Ok(has_active)
    }

    /// Broadcast a pending summary so the UI displays the task immediately.
    pub fn emit_pending_summary(&self, info_hash: Id20) {
        let summary = if let Some(handle) = self.session.get(TorrentIdOrHash::Hash(info_hash)) {
            let snap = self.stats_to_snapshot(&info_hash, &handle.stats());
            DownloadSummary::from(&snap)
        } else {
            self.default_pending_summary(&info_hash.as_string())
        };
        self.event_bus.publish(DownloadEvent::Updated {
            summary: Box::new(summary),
        });
    }

    fn default_pending_summary(&self, id_hex: &str) -> DownloadSummary {
        let now = crate::now_ms();
        let snap = DownloadSnapshot {
            id: id_hex.to_string(),
            kind: TaskKind::Bt,
            state: DownloadState::Queued,
            created_at_ms: now,
            updated_at_ms: now,
            url: id_hex.to_string(),
            final_url: id_hex.to_string(),
            file_name: "Pending torrent".to_string(),
            destination_path: self.default_output_dir.to_string_lossy().to_string(),
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
            info_hash: Some(id_hex.to_string()),
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
        };
        DownloadSummary::from(&snap)
    }
}

/// Sanitize peer client identification string.
pub fn sanitize_peer_client(client: &str) -> &str {
    client.trim_matches(|c: char| c.is_ascii_whitespace() || c.is_ascii_control())
}
