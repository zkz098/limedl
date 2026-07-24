use serde::{Deserialize, Serialize};

use super::types::{
    AdaptiveProfile, ChecksumMode, ChunkInfo, ChunkSizeStrategy, DownloadSnapshot, DownloadState,
    Priority, TaskKind, ThreadMode, default_http_user_agent,
};

pub const CHUNK_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub url: String,
    pub final_url: String,
    #[serde(default = "default_http_user_agent")]
    pub user_agent: String,
    pub destination_dir: String,
    pub file_name: String,
    #[serde(default = "default_true")]
    pub file_name_locked: bool,
    pub destination_path: String,
    pub temp_path: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub supports_ranges: bool,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u64,
    pub connection_count: usize,
    pub thread_mode: ThreadMode,
    pub requested_thread_count: Option<usize>,
    pub desired_thread_count: Option<usize>,
    pub allocated_thread_count: Option<usize>,
    pub adaptive_profile_snapshot: Option<AdaptiveProfile>,
    pub thread_note: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub state: DownloadState,
    #[serde(default)]
    pub cdn_accelerated: bool,
    pub checksum_mode: ChecksumMode,
    pub checksum: Option<String>,
    #[serde(default)]
    pub expected_checksum: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub priority: Priority,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub mirror_url: Option<String>,
    #[serde(default)]
    pub mirror_urls: Vec<String>,
    #[serde(default)]
    pub current_mirror_index: usize,
    pub chunks: Vec<ChunkManifest>,
}

fn default_true() -> bool {
    true
}

fn default_chunk_size() -> u64 {
    CHUNK_SIZE
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkManifest {
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub downloaded: u64,
    pub completed: bool,
    pub claimed_by: Option<usize>,
    /// Tracks whether this chunk changed since the last incremental DB persist.
    /// Reset to `false` after each `persist_manifest_snapshot` flush.
    /// Serialisation is skipped — this is an internal-only flag.
    #[serde(skip)]
    pub dirty: bool,
}

#[derive(Debug, Clone)]
pub struct RemoteMetadata {
    pub final_url: String,
    pub file_name: String,
    pub total_bytes: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub supports_ranges: bool,
}

pub fn plan_chunks(
    total: Option<u64>,
    supports_ranges: bool,
    chunk_size: u64,
) -> Vec<ChunkManifest> {
    if !supports_ranges {
        return vec![];
    }

    let Some(total) = total else {
        return vec![];
    };

    let chunk_size = if chunk_size == 0 {
        CHUNK_SIZE
    } else {
        chunk_size
    };
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while start < total {
        let end = (start + chunk_size - 1).min(total - 1);
        chunks.push(ChunkManifest {
            index,
            start,
            end,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        });
        index += 1;
        start = end + 1;
    }

    chunks
}

pub fn adaptive_chunk_size(total: u64) -> u64 {
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * MIB;
    if total <= 256 * MIB {
        4 * MIB
    } else if total < 4 * GIB {
        8 * MIB
    } else {
        16 * MIB
    }
}

pub fn resolve_chunk_size(strategy: ChunkSizeStrategy, total: Option<u64>) -> u64 {
    match strategy {
        ChunkSizeStrategy::Fixed => CHUNK_SIZE,
        ChunkSizeStrategy::Adaptive => total.map(adaptive_chunk_size).unwrap_or(CHUNK_SIZE),
    }
}

pub fn snapshot_from_manifest(manifest: &Manifest) -> DownloadSnapshot {
    DownloadSnapshot {
        id: manifest.id.clone(),
        kind: TaskKind::Http,
        state: manifest.state,
        url: manifest.url.clone(),
        final_url: manifest.final_url.clone(),
        file_name: manifest.file_name.clone(),
        destination_path: manifest.destination_path.clone(),
        temp_path: manifest.temp_path.clone(),
        total_bytes: manifest.total_bytes,
        downloaded_bytes: manifest.downloaded_bytes,
        supports_ranges: manifest.supports_ranges,
        connection_count: manifest.connection_count,
        thread_mode: manifest.thread_mode,
        requested_thread_count: manifest.requested_thread_count,
        desired_thread_count: manifest.desired_thread_count,
        allocated_thread_count: manifest.allocated_thread_count,
        adaptive_profile: manifest.adaptive_profile_snapshot,
        thread_note: manifest.thread_note.clone(),
        checksum: manifest.checksum.clone(),
        checksum_mode: manifest.checksum_mode,
        etag: manifest.etag.clone(),
        last_modified: manifest.last_modified.clone(),
        error: manifest.error.clone(),
        speed_bytes_per_second: None,
        eta_seconds: None,
        uploaded_bytes: None,
        upload_speed_bytes_per_second: None,
        peer_count: None,
        upload_status: None,
        info_hash: None,
        created_at_ms: manifest.created_at_ms,
        updated_at_ms: manifest.updated_at_ms,
        priority: manifest.priority,
        cdn_accelerated: manifest.cdn_accelerated,
        mirror_url: manifest.mirror_url.clone(),
        chunks: manifest
            .chunks
            .iter()
            .map(|c| ChunkInfo {
                index: c.index,
                start: c.start,
                end: c.end,
                downloaded: c.downloaded,
                completed: c.completed,
                claimed_by: c.claimed_by,
            })
            .collect(),
        seed_count: None,
        leech_count: None,
        download_limit_bps: None,
        upload_limit_bps: None,
        degraded: false,
        disk_type: None,
        flushing: false,
    }
}

pub fn validators_changed(manifest: &Manifest, metadata: &RemoteMetadata) -> bool {
    match (&manifest.etag, &metadata.etag) {
        (Some(left), Some(right)) => left != right,
        _ => match (&manifest.last_modified, &metadata.last_modified) {
            (Some(left), Some(right)) => left != right,
            _ => false,
        },
    }
}

pub fn has_partial_chunk_progress(manifest: &Manifest) -> bool {
    manifest
        .chunks
        .iter()
        .any(|chunk| chunk.downloaded > 0 && !chunk.completed)
}

pub fn contiguous_prefix_end(manifest: &Manifest) -> u64 {
    if manifest.chunks.is_empty() {
        return manifest.downloaded_bytes;
    }

    let mut expected_start = 0;
    let mut contiguous = 0;
    for chunk in &manifest.chunks {
        if chunk.start != expected_start || !chunk.completed {
            break;
        }
        contiguous = chunk.end.saturating_add(1);
        expected_start = contiguous;
    }
    contiguous
}

#[cfg(test)]
mod tests {
    use ntest::timeout;

    use super::{CHUNK_SIZE, adaptive_chunk_size, plan_chunks};

    #[timeout(30_000)]
    #[test]
    fn plans_stable_chunk_boundaries() {
        let chunks = plan_chunks(Some(16 * 1024 * 1024), true, CHUNK_SIZE);

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, CHUNK_SIZE - 1);
        assert_eq!(chunks[3].start, CHUNK_SIZE * 3);
        assert_eq!(chunks[3].end, 16 * 1024 * 1024 - 1);
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_zero() {
        assert_eq!(adaptive_chunk_size(0), 4 * 1024 * 1024);
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_4_mib() {
        assert_eq!(adaptive_chunk_size(4 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_16_mib() {
        assert_eq!(adaptive_chunk_size(16 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_17_mib() {
        assert_eq!(adaptive_chunk_size(17 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_256_mib() {
        assert_eq!(adaptive_chunk_size(256 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_257_mib() {
        assert_eq!(adaptive_chunk_size(257 * 1024 * 1024), 8 * 1024 * 1024);
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_4gib_minus_1() {
        assert_eq!(
            adaptive_chunk_size(4 * 1024 * 1024 * 1024 - 1),
            8 * 1024 * 1024
        );
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_4gib() {
        assert_eq!(
            adaptive_chunk_size(4 * 1024 * 1024 * 1024),
            16 * 1024 * 1024
        );
    }

    #[timeout(30_000)]
    #[test]
    fn adaptive_chunk_size_10_gib() {
        assert_eq!(
            adaptive_chunk_size(10 * 1024 * 1024 * 1024),
            16 * 1024 * 1024
        );
    }
}
