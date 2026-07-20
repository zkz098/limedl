use std::collections::HashSet;
use std::path::PathBuf;

use irontide::core::Id20;

use super::IrontideBtBackend;
use super::snapshot::{build_peer_flags, preview_entries_from_meta};
use crate::error::{DownloadError, Result};
use crate::event_bus::DownloadEvent;
use crate::types::{
    BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtTrackerInfo, BtUploadStatus,
    DownloadState, DownloadSummary, TaskKind, ThreadMode, TorrentFileEntry,
};
use crate::{lock, now_ms};

impl IrontideBtBackend {
    pub fn set_speed_limit(
        &self,
        info_hash: Id20,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    ) {
        tokio::task::block_in_place(|| {
            self.runtime_handle.block_on(async {
                if let Some(bps) = download_limit_bps {
                    let _ = self.session.set_download_limit(info_hash, bps).await;
                }
                if let Some(bps) = upload_limit_bps {
                    let _ = self.session.set_upload_limit(info_hash, bps).await;
                }
            });
        });
    }

    pub async fn preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>> {
        let source = source.trim();
        if source.to_ascii_lowercase().starts_with("magnet:") {
            return Err(DownloadError::TorrentInvalidData(
                "cannot preview magnet link; metadata not yet available".into(),
            ));
        }

        // Read torrent bytes from URL (with proxy support) or local file
        let bytes: Vec<u8> = if source.starts_with("http://") || source.starts_with("https://") {
            self.fetch_url_bytes(source).await?
        } else {
            tokio::fs::read(source).await.map_err(|e| {
                DownloadError::TorrentIo(format!("failed to read torrent file: {e}"))
            })?
        };

        // Parse the torrent metainfo and extract file list
        let meta = irontide::core::torrent_from_bytes_any(&bytes).map_err(|e| {
            DownloadError::TorrentInvalidData(format!("failed to parse torrent: {e}"))
        })?;

        let entries = preview_entries_from_meta(&meta);

        Ok(entries)
    }

    pub fn get_peers(&self, info_hash: Id20) -> Result<Vec<BtPeerInfo>> {
        let peers = tokio::task::block_in_place(|| {
            self.runtime_handle
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

    pub fn get_trackers(&self, info_hash: Id20) -> Result<Vec<BtTrackerInfo>> {
        let trackers = tokio::task::block_in_place(|| {
            self.runtime_handle
                .block_on(self.session.tracker_list(info_hash))
        })
        .map_err(|e| DownloadError::Torrent(e.to_string()))?;
        Ok(trackers
            .iter()
            .map(|t| BtTrackerInfo { url: t.url.clone() })
            .collect())
    }

    pub fn get_pieces(&self, info_hash: Id20) -> Result<Vec<BtPieceInfo>> {
        // Use torrent_stats to get pieces_have/pieces_total and derive piece info
        let stats = tokio::task::block_in_place(|| {
            self.runtime_handle
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

    pub fn get_torrent_files(&self, info_hash: Id20) -> Result<Vec<BtFileStatus>> {
        // Use torrent_file + file_progress + file_status to build file status
        let meta_fut = self.session.torrent_file(info_hash);
        let progress_fut = self.session.file_progress(info_hash);
        let status_fut = self.session.file_status(info_hash);

        let (meta_result, progress_result, status_result) = tokio::task::block_in_place(|| {
            self.runtime_handle
                .block_on(async { tokio::join!(meta_fut, progress_fut, status_fut) })
        });

        let file_progress = progress_result
            .map_err(|e| DownloadError::TorrentIo(format!("failed to get file progress: {e}")))?;

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
                        let included = file_statuses.as_ref().is_none_or(|sts| {
                            sts.get(i).is_none_or(|fs| {
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
            Err(e) => Err(DownloadError::TorrentIo(format!(
                "failed to get torrent file info: {e}"
            ))),
        }
    }

    pub async fn update_torrent_files(
        &self,
        info_hash: Id20,
        included_indices: Vec<usize>,
    ) -> Result<()> {
        // Get the torrent metadata to know how many files there are
        let meta = self
            .session
            .torrent_file(info_hash)
            .await
            .map_err(|e| DownloadError::Torrent(e.to_string()))?;

        let Some(meta) = meta else {
            return Err(DownloadError::TorrentInvalidData(
                "torrent metadata not yet available".into(),
            ));
        };

        let file_count = meta.info.files.map_or(1, |f| f.len());
        let included_set: HashSet<usize> = included_indices.into_iter().collect();

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

    pub fn runtime_status(&self) -> BtRuntimeStatus {
        let dht_enabled = lock(&self.bt_settings).dht_enabled;

        let (dht_nodes, torrent_count, peer_count, upload_speed, uploaded) =
            tokio::task::block_in_place(|| {
                let dht_nodes = self
                    .runtime_handle
                    .block_on(self.session.session_stats())
                    .ok()
                    .map(|s| s.dht_nodes);

                let torrents: Vec<Id20> = self
                    .runtime_handle
                    .block_on(self.session.list_torrents())
                    .unwrap_or_default();

                let count = torrents.len();
                let mut peers = 0usize;
                let mut up_speed = 0.0f64;
                let mut uploaded: u64 = 0;
                for ih in &torrents {
                    if let Ok(stats) = self
                        .runtime_handle
                        .block_on(self.session.torrent_stats(*ih))
                    {
                        peers += stats.peers_connected;
                        up_speed += stats.upload_payload_rate as f64;
                        uploaded += stats.uploaded;
                    }
                }

                (dht_nodes, count, peers, up_speed, uploaded)
            });

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

    pub fn emit_pending_summary(&self, info_hash: Id20) {
        let id_hex = info_hash.to_hex();
        // Try to get stats; if not available yet, emit a minimal snapshot.
        let summary = match tokio::task::block_in_place(|| {
            self.runtime_handle
                .block_on(self.session.torrent_stats(info_hash))
        }) {
            Ok(stats) => {
                let snapshot = self.stats_to_snapshot(&info_hash, &stats);
                DownloadSummary::from(&snapshot)
            }
            Err(_) => fallback_pending_summary(&id_hex, &self.default_output_dir),
        };

        let summary_json = serde_json::to_value(&summary).unwrap_or_default();
        self.event_bus.publish(DownloadEvent::Updated {
            id: id_hex,
            summary_json,
        });
    }
}

/// Build a queued-state summary for a pending torrent.
fn fallback_pending_summary(
    pending_id: &str,
    default_output_dir: &std::path::Path,
) -> DownloadSummary {
    DownloadSummary {
        id: pending_id.to_string(),
        kind: TaskKind::Bt,
        state: DownloadState::Queued,
        url: String::new(),
        file_name: String::from("Pending torrent"),
        destination_path: default_output_dir.to_string_lossy().to_string(),
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
    }
}
