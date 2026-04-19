use serde::{Deserialize, Serialize};

use super::types::{ChecksumMode, DownloadSnapshot, DownloadState};

const MIN_SEGMENT_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Manifest {
    pub(super) id: String,
    pub(super) url: String,
    pub(super) final_url: String,
    pub(super) destination_dir: String,
    pub(super) file_name: String,
    pub(super) destination_path: String,
    pub(super) temp_path: String,
    pub(super) manifest_path: String,
    pub(super) total_bytes: Option<u64>,
    pub(super) downloaded_bytes: u64,
    pub(super) supports_ranges: bool,
    pub(super) connection_count: usize,
    pub(super) etag: Option<String>,
    pub(super) last_modified: Option<String>,
    pub(super) state: DownloadState,
    pub(super) checksum_mode: ChecksumMode,
    pub(super) checksum: Option<String>,
    pub(super) error: Option<String>,
    pub(super) created_at_ms: u64,
    pub(super) updated_at_ms: u64,
    pub(super) segments: Vec<SegmentManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SegmentManifest {
    pub(super) index: usize,
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) downloaded: u64,
    pub(super) completed: bool,
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

pub(super) fn compute_connection_count(
    total: Option<u64>,
    supports_ranges: bool,
    requested: usize,
) -> usize {
    if !supports_ranges {
        return 1;
    }

    let requested = requested.clamp(1, 16);
    match total {
        Some(total) if total >= MIN_SEGMENT_SIZE * 2 => requested,
        _ => 1,
    }
}

pub(super) fn plan_segments(
    total: Option<u64>,
    supports_ranges: bool,
    connections: usize,
) -> Vec<SegmentManifest> {
    if !supports_ranges || connections <= 1 {
        return vec![];
    }

    let total = match total {
        Some(total) => total,
        None => return vec![],
    };
    let chunk_count = connections.max(1) as u64;
    let size = (total / chunk_count).max(MIN_SEGMENT_SIZE);
    let mut segments = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while start < total {
        let end = (start + size - 1).min(total - 1);
        segments.push(SegmentManifest {
            index,
            start,
            end,
            downloaded: 0,
            completed: false,
        });
        index += 1;
        start = end + 1;
    }

    segments
}

pub(super) fn snapshot_from_manifest(manifest: &Manifest) -> DownloadSnapshot {
    DownloadSnapshot {
        id: manifest.id.clone(),
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
        checksum: manifest.checksum.clone(),
        checksum_mode: manifest.checksum_mode.clone(),
        etag: manifest.etag.clone(),
        last_modified: manifest.last_modified.clone(),
        error: manifest.error.clone(),
        speed_bytes_per_second: None,
        eta_seconds: None,
        created_at_ms: manifest.created_at_ms,
        updated_at_ms: manifest.updated_at_ms,
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

pub(super) fn has_partial_segment_progress(manifest: &Manifest) -> bool {
    manifest
        .segments
        .iter()
        .any(|segment| segment.downloaded > 0 && !segment.completed)
}

pub(super) fn contiguous_prefix_end(manifest: &Manifest) -> u64 {
    if manifest.segments.is_empty() {
        return manifest.downloaded_bytes;
    }

    let mut expected_start = 0;
    let mut contiguous = 0;
    for segment in &manifest.segments {
        if segment.start != expected_start || !segment.completed {
            break;
        }
        contiguous = segment.end.saturating_add(1);
        expected_start = contiguous;
    }
    contiguous
}

#[cfg(test)]
mod tests {
    use super::{compute_connection_count, plan_segments};

    #[test]
    fn disables_parallelism_without_ranges() {
        assert_eq!(
            compute_connection_count(Some(64 * 1024 * 1024), false, 8),
            1
        );
    }

    #[test]
    fn plans_stable_segment_boundaries() {
        let segments = plan_segments(Some(16 * 1024 * 1024), true, 4);

        assert_eq!(segments.len(), 4);
        assert_eq!(segments[0].start, 0);
        assert_eq!(segments[0].end, 4 * 1024 * 1024 - 1);
        assert_eq!(segments[3].start, 12 * 1024 * 1024);
        assert_eq!(segments[3].end, 16 * 1024 * 1024 - 1);
    }
}
