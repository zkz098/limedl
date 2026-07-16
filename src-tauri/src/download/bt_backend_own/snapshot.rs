use irontide::core::Id20;

use super::super::types::{
    BtUploadStatus, ChecksumMode, DownloadSnapshot, DownloadState, TaskKind, ThreadMode,
    TorrentFileEntry,
};
use super::super::now_ms;

impl super::OwnBtBackend {
    /// Build a `DownloadSnapshot` from irontide stats.
    pub(crate) fn stats_to_snapshot(
        &self,
        task_id: &str,
        info_hash: &Id20,
        stats: &irontide::session::TorrentStats,
    ) -> DownloadSnapshot {
        let now = now_ms();
        let state = map_state(&stats.state);
        // Use `downloaded` (all payload bytes) instead of `total_done`
        // (only verified pieces) so the frontend shows smooth progress
        // instead of 0B until each multi-MB piece is fully verified.
        let downloaded = stats.downloaded;
        let peer_count = stats.peers_connected;
        // Use `total_wanted` to reflect only the files selected for download,
        // not the full torrent size when file priorities are active.
        let total = stats.total_wanted;

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
}

// ---------------------------------------------------------------------------
//  Helper functions
// ---------------------------------------------------------------------------

/// Map an irontide `TorrentState` to our `DownloadState`.
pub(crate) fn map_state(state: &irontide::session::TorrentState) -> DownloadState {
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
pub(crate) fn build_peer_flags(peer: &irontide::session::PeerInfo) -> String {
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
pub(crate) fn preview_entries_from_meta(meta: &irontide::core::TorrentMeta) -> Vec<TorrentFileEntry> {
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

pub(crate) fn v1_file_entries(v1: &irontide::core::TorrentMetaV1) -> Vec<TorrentFileEntry> {
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
pub(crate) fn estimate_eta(total: u64, downloaded: u64, speed: Option<f64>) -> Option<u64> {
    let speed = speed?;
    if total <= downloaded || speed <= 0.0 {
        return None;
    }
    Some(((total - downloaded) as f64 / speed).ceil() as u64)
}

// ---------------------------------------------------------------------------
//  StateHelpers trait
// ---------------------------------------------------------------------------

pub(crate) trait StateHelpers {
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
