use librqbit::TorrentStats;
use librqbit::dht::Id20;

use crate::now_ms;
use crate::types::{
    BtUploadStatus, ChecksumMode, DownloadSnapshot, DownloadState, Priority, TaskKind, ThreadMode,
};

impl super::RqbitBtBackend {
    /// Build a `DownloadSnapshot` from librqbit stats.
    pub fn stats_to_snapshot(&self, info_hash: &Id20, stats: &TorrentStats) -> DownloadSnapshot {
        let now = now_ms();
        let state = map_state(stats);
        let downloaded = stats.progress_bytes;
        let total = stats.total_bytes;
        let peer_count = stats
            .live
            .as_ref()
            .map(|l| l.snapshot.peer_stats.live as usize)
            .unwrap_or(0);

        // Only show speed when not in a terminal state
        let speed = stats
            .live
            .as_ref()
            .map(|l| l.download_speed.as_bytes() as f64)
            .filter(|&s| s > 0.0);
        let upload_speed = stats
            .live
            .as_ref()
            .map(|l| l.upload_speed.as_bytes() as f64)
            .filter(|&s| s > 0.0);

        let file_name = self
            .session
            .get(librqbit::api::TorrentIdOrHash::Hash(*info_hash))
            .and_then(|h| h.name())
            .unwrap_or_else(|| info_hash.as_string());

        let destination_path = self
            .api
            .api_torrent_details(librqbit::api::TorrentIdOrHash::Hash(*info_hash))
            .ok()
            .map(|d| d.output_folder)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.default_output_dir.to_string_lossy().to_string());

        DownloadSnapshot {
            id: info_hash.as_string(),
            kind: TaskKind::Bt,
            state,
            url: info_hash.as_string(),
            final_url: info_hash.as_string(),
            file_name,
            destination_path,
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
            thread_note: Some(String::from("BT task managed by librqbit")),
            checksum: None,
            expected_checksum: None,
            checksum_mode: ChecksumMode::None,
            etag: None,
            last_modified: None,
            error: stats.error.clone(),
            speed_bytes_per_second: if state.is_terminal() { None } else { speed },
            eta_seconds: if state.is_terminal() {
                None
            } else {
                estimate_eta(total, downloaded, speed)
            },
            uploaded_bytes: Some(stats.uploaded_bytes),
            upload_speed_bytes_per_second: if state.is_terminal() {
                None
            } else {
                upload_speed
            },
            peer_count: Some(peer_count),
            upload_status: Some({
                if self.paused_by_limit.contains_key(info_hash) {
                    BtUploadStatus::PausedByLimit
                } else {
                    match state {
                        DownloadState::Paused => BtUploadStatus::Paused,
                        _ if upload_speed.unwrap_or(0.0) > 0.0 => BtUploadStatus::Uploading,
                        _ => BtUploadStatus::Idle,
                    }
                }
            }),
            info_hash: Some(info_hash.as_string()),
            created_at_ms: self
                .torrent_created_at
                .get(info_hash)
                .map(|r| *r.value())
                .unwrap_or(now),
            updated_at_ms: now,
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
        }
    }
}

// ---------------------------------------------------------------------------
//  Helper functions
// ---------------------------------------------------------------------------

/// Map librqbit `TorrentStats` to our `DownloadState`.
pub fn map_state(stats: &TorrentStats) -> DownloadState {
    use librqbit::TorrentStatsState;
    if stats.finished {
        return DownloadState::Completed;
    }
    match &stats.state {
        TorrentStatsState::Live => DownloadState::Downloading,
        TorrentStatsState::Paused => DownloadState::Paused,
        TorrentStatsState::Initializing { paused: true } => DownloadState::Paused,
        TorrentStatsState::Initializing { paused: false } => DownloadState::Verifying,
        TorrentStatsState::Error => DownloadState::Failed,
    }
}

/// Build a human-readable flags string from peer connection kind.
pub fn build_peer_flags(conn_kind: Option<impl std::fmt::Debug>) -> String {
    let mut flags = String::with_capacity(4);
    if let Some(conn) = conn_kind {
        let s = format!("{conn:?}");
        if s.contains("Utp") {
            flags.push('u');
        } else if s.contains("Tcp") {
            flags.push('T');
        }
    }
    flags
}

/// Estimate remaining time from total, downloaded, and speed.
pub fn estimate_eta(total: u64, downloaded: u64, speed: Option<f64>) -> Option<u64> {
    let speed = speed?;
    if total <= downloaded || speed <= 0.0 {
        return None;
    }
    Some(((total - downloaded) as f64 / speed).ceil() as u64)
}

// ---------------------------------------------------------------------------
//  StateHelpers trait
// ---------------------------------------------------------------------------

pub trait StateHelpers {
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
