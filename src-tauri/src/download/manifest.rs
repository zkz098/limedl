use serde::{Deserialize, Serialize};

use super::types::{
    AdaptiveProfile, ChecksumMode, ChunkInfo, DownloadSnapshot, DownloadState, TaskKind,
    ThreadMode, default_http_user_agent,
};

pub(super) const CHUNK_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Manifest {
    pub(super) id: String,
    pub(super) url: String,
    pub(super) final_url: String,
    #[serde(default = "default_http_user_agent")]
    pub(super) user_agent: String,
    pub(super) destination_dir: String,
    pub(super) file_name: String,
    #[serde(default = "default_true")]
    pub(super) file_name_locked: bool,
    pub(super) destination_path: String,
    pub(super) temp_path: String,
    pub(super) total_bytes: Option<u64>,
    pub(super) downloaded_bytes: u64,
    pub(super) supports_ranges: bool,
    pub(super) connection_count: usize,
    pub(super) thread_mode: ThreadMode,
    pub(super) requested_thread_count: Option<usize>,
    pub(super) desired_thread_count: Option<usize>,
    pub(super) allocated_thread_count: Option<usize>,
    pub(super) adaptive_profile_snapshot: Option<AdaptiveProfile>,
    pub(super) thread_note: Option<String>,
    pub(super) etag: Option<String>,
    pub(super) last_modified: Option<String>,
    pub(super) state: DownloadState,
    #[serde(default)]
    pub(super) cdn_accelerated: bool,
    pub(super) checksum_mode: ChecksumMode,
    pub(super) checksum: Option<String>,
    pub(super) error: Option<String>,
    pub(super) created_at_ms: u64,
    pub(super) updated_at_ms: u64,
    pub(super) chunks: Vec<ChunkManifest>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChunkManifest {
    pub(super) index: usize,
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) downloaded: u64,
    pub(super) completed: bool,
    pub(super) claimed_by: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) struct RemoteMetadata {
    pub(super) final_url: String,
    pub(super) file_name: String,
    pub(super) total_bytes: Option<u64>,
    pub(super) etag: Option<String>,
    pub(super) last_modified: Option<String>,
    pub(super) supports_ranges: bool,
}

pub(super) fn plan_chunks(total: Option<u64>, supports_ranges: bool) -> Vec<ChunkManifest> {
    if !supports_ranges {
        return vec![];
    }

    let Some(total) = total else {
        return vec![];
    };

    let mut chunks = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while start < total {
        let end = (start + CHUNK_SIZE - 1).min(total - 1);
        chunks.push(ChunkManifest {
            index,
            start,
            end,
            downloaded: 0,
            completed: false,
            claimed_by: None,
        });
        index += 1;
        start = end + 1;
    }

    chunks
}

pub(super) fn snapshot_from_manifest(manifest: &Manifest) -> DownloadSnapshot {
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
        cdn_accelerated: manifest.cdn_accelerated,
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
    }
}

pub(super) fn validators_changed(manifest: &Manifest, metadata: &RemoteMetadata) -> bool {
    match (&manifest.etag, &metadata.etag) {
        (Some(left), Some(right)) => left != right,
        _ => match (&manifest.last_modified, &metadata.last_modified) {
            (Some(left), Some(right)) => left != right,
            _ => false,
        },
    }
}

pub(super) fn has_partial_chunk_progress(manifest: &Manifest) -> bool {
    manifest
        .chunks
        .iter()
        .any(|chunk| chunk.downloaded > 0 && !chunk.completed)
}

pub(super) fn contiguous_prefix_end(manifest: &Manifest) -> u64 {
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

    use super::{CHUNK_SIZE, plan_chunks};

    #[timeout(30_000)]
    #[test]
    fn plans_stable_chunk_boundaries() {
        let chunks = plan_chunks(Some(16 * 1024 * 1024), true);

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, CHUNK_SIZE - 1);
        assert_eq!(chunks[3].start, CHUNK_SIZE * 3);
        assert_eq!(chunks[3].end, 16 * 1024 * 1024 - 1);
    }
}
