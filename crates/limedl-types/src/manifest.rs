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
    /// Extra HTTP headers (`"Name: Value"`) applied to the live download
    /// session. In-memory only in the CLI pass (not persisted as a DB column).
    #[serde(default)]
    pub extra_headers: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdn_node_ip: Option<String>,
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
    /// Bytes received for this chunk in this session — what the scheduler, the
    /// claim/unclaim logic and the UI follow. It can run ahead of the file by
    /// whatever the write buffer still holds.
    pub downloaded: u64,
    /// "The worker received the whole range", in the same received sense as
    /// [`Self::downloaded`]. It is *not* a statement about the file.
    pub completed: bool,
    /// Bytes of this chunk that provably reached the file: advanced only after a
    /// write-buffer flush (or a direct write) returned successfully. This — not
    /// [`Self::downloaded`] — is what gets persisted, so a hard kill can never
    /// leave the database claiming bytes that were only in memory and make the
    /// next resume skip over a hole. Always read it through [`Self::durable_bytes`].
    ///
    /// `serde(skip)`: internal bookkeeping, never part of a wire format.
    #[serde(skip)]
    pub durable_downloaded: u64,
    pub claimed_by: Option<usize>,
    /// Tracks whether this chunk changed since the last incremental DB persist.
    /// Reset to `false` after each `persist_manifest_snapshot` flush.
    /// Serialisation is skipped — this is an internal-only flag.
    #[serde(skip)]
    pub dirty: bool,
}

impl ChunkManifest {
    /// Bytes this chunk covers.
    ///
    /// Named `byte_len` (not `len`) because a chunk is never empty: it always
    /// spans at least one byte, so an `is_empty` companion would be a lie.
    pub fn byte_len(&self) -> u64 {
        self.end.saturating_sub(self.start).saturating_add(1)
    }

    /// Durable bytes for this chunk, clamped to what has actually been received.
    ///
    /// The clamp is what makes a stale counter harmless: a chunk re-planned on
    /// top of an old one, or a counter that a reset path forgot to clear, can
    /// only ever under-report, which costs a re-download — never the other
    /// direction, which would resume past data that is not on disk.
    pub fn durable_bytes(&self) -> u64 {
        self.durable_downloaded.min(self.downloaded)
    }

    /// `true` when every byte of this chunk is on the file.
    pub fn durable_complete(&self) -> bool {
        self.durable_bytes() >= self.byte_len()
    }
}

#[derive(Debug, Clone)]
pub struct RemoteMetadata {
    pub final_url: String,
    pub file_name: String,
    pub total_bytes: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub supports_ranges: bool,
    pub extra_headers: Vec<String>,
    pub headers_checksum: Option<(crate::types::ChecksumMode, String)>,
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
            durable_downloaded: 0,
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
    } else if total < 16 * GIB {
        32 * MIB
    } else if total < 64 * GIB {
        64 * MIB
    } else {
        128 * MIB
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
        expected_checksum: manifest.expected_checksum.clone(),
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
        cdn_node_ip: manifest.cdn_node_ip.clone(),
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

    use super::{
        CHUNK_SIZE, ChunkManifest, Manifest, adaptive_chunk_size, contiguous_prefix_end,
        has_partial_chunk_progress, plan_chunks, resolve_chunk_size,
    };
    use crate::types::{
        ChecksumMode, ChunkSizeStrategy, DownloadState, Priority, ThreadMode,
        default_http_user_agent,
    };
    use proptest::prelude::*;

    #[test]
    fn plans_stable_chunk_boundaries() {
        let chunks = plan_chunks(Some(16 * 1024 * 1024), true, CHUNK_SIZE);

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, CHUNK_SIZE - 1);
        assert_eq!(chunks[3].start, CHUNK_SIZE * 3);
        assert_eq!(chunks[3].end, 16 * 1024 * 1024 - 1);
    }

    #[test]
    fn adaptive_chunk_size_zero() {
        assert_eq!(adaptive_chunk_size(0), 4 * 1024 * 1024);
    }

    #[test]
    fn adaptive_chunk_size_4_mib() {
        assert_eq!(adaptive_chunk_size(4 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[test]
    fn adaptive_chunk_size_16_mib() {
        assert_eq!(adaptive_chunk_size(16 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[test]
    fn adaptive_chunk_size_17_mib() {
        assert_eq!(adaptive_chunk_size(17 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[test]
    fn adaptive_chunk_size_256_mib() {
        assert_eq!(adaptive_chunk_size(256 * 1024 * 1024), 4 * 1024 * 1024);
    }

    #[test]
    fn adaptive_chunk_size_257_mib() {
        assert_eq!(adaptive_chunk_size(257 * 1024 * 1024), 8 * 1024 * 1024);
    }

    #[test]
    fn adaptive_chunk_size_4gib_minus_1() {
        assert_eq!(
            adaptive_chunk_size(4 * 1024 * 1024 * 1024 - 1),
            8 * 1024 * 1024
        );
    }

    #[test]
    fn adaptive_chunk_size_4gib() {
        assert_eq!(
            adaptive_chunk_size(4 * 1024 * 1024 * 1024),
            32 * 1024 * 1024
        );
    }

    #[test]
    fn adaptive_chunk_size_10_gib() {
        assert_eq!(
            adaptive_chunk_size(10 * 1024 * 1024 * 1024),
            32 * 1024 * 1024
        );
    }

    #[test]
    fn adaptive_chunk_size_16_gib() {
        assert_eq!(
            adaptive_chunk_size(16 * 1024 * 1024 * 1024),
            64 * 1024 * 1024
        );
    }

    #[test]
    fn adaptive_chunk_size_64_gib() {
        assert_eq!(
            adaptive_chunk_size(64 * 1024 * 1024 * 1024),
            128 * 1024 * 1024
        );
    }

    #[test]
    fn adaptive_chunk_size_15gib_boundary() {
        // Just below the 16 GiB boundary → still 32 MiB
        assert_eq!(
            adaptive_chunk_size(16 * 1024 * 1024 * 1024 - 1),
            32 * 1024 * 1024
        );
    }

    #[test]
    fn adaptive_chunk_size_63gib_boundary() {
        // Just below the 64 GiB boundary → still 64 MiB
        assert_eq!(
            adaptive_chunk_size(64 * 1024 * 1024 * 1024 - 1),
            64 * 1024 * 1024
        );
    }

    /// A manifest carrying `chunks`; only the fields these properties read are
    /// meaningful, but every field has to be present.
    fn manifest_with(chunks: Vec<ChunkManifest>, downloaded_bytes: u64) -> Manifest {
        Manifest {
            id: "prop".into(),
            url: "https://example.com/file.bin".into(),
            final_url: "https://example.com/file.bin".into(),
            user_agent: default_http_user_agent(),
            extra_headers: Vec::new(),
            destination_dir: "/tmp".into(),
            file_name: "file.bin".into(),
            file_name_locked: true,
            destination_path: "/tmp/file.bin".into(),
            temp_path: "/tmp/file.bin.part".into(),
            total_bytes: None,
            downloaded_bytes,
            supports_ranges: true,
            chunk_size: CHUNK_SIZE,
            connection_count: 0,
            thread_mode: ThreadMode::Adaptive,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile_snapshot: None,
            thread_note: None,
            etag: None,
            last_modified: None,
            state: DownloadState::Downloading,
            cdn_accelerated: false,
            cdn_node_ip: None,
            checksum_mode: ChecksumMode::None,
            checksum: None,
            expected_checksum: None,
            error: None,
            priority: Priority::Normal,
            created_at_ms: 0,
            updated_at_ms: 0,
            mirror_url: None,
            mirror_urls: Vec::new(),
            current_mirror_index: 0,
            chunks,
        }
    }

    proptest! {
        /// Chunks must tile `[0, total)` exactly: no gap, no overlap, no
        /// undersized/oversized chunk, and the planned size is capped.
        #[test]
        fn plan_chunks_covers_the_file_exactly(
            total in 1u64..=64 * 1024 * 1024,
            chunk_size in 1u64..=8 * 1024 * 1024,
        ) {
            let chunks = plan_chunks(Some(total), true, chunk_size);
            prop_assert!(!chunks.is_empty());
            prop_assert_eq!(chunks.first().unwrap().start, 0);
            prop_assert_eq!(chunks.last().unwrap().end, total - 1);

            let mut next_start = 0u64;
            for (index, chunk) in chunks.iter().enumerate() {
                prop_assert_eq!(chunk.index, index);
                prop_assert_eq!(chunk.start, next_start);
                prop_assert!(chunk.end >= chunk.start);
                prop_assert!(chunk.byte_len() >= 1);
                prop_assert!(chunk.byte_len() <= chunk_size);
                prop_assert!(!chunk.completed);
                prop_assert_eq!(chunk.downloaded, 0);
                prop_assert_eq!(chunk.durable_downloaded, 0);
                prop_assert!(chunk.claimed_by.is_none());
                prop_assert!(!chunk.dirty);
                next_start = chunk.end + 1;
            }
            prop_assert_eq!(next_start, total);
            prop_assert_eq!(chunks.iter().map(ChunkManifest::byte_len).sum::<u64>(), total);
        }

        #[test]
        fn plan_chunks_zero_chunk_size_uses_default(
            total in 1u64..=16 * 1024 * 1024,
        ) {
            let zero = plan_chunks(Some(total), true, 0);
            let default = plan_chunks(Some(total), true, CHUNK_SIZE);
            prop_assert_eq!(zero.len(), default.len());
            for (left, right) in zero.iter().zip(default.iter()) {
                prop_assert_eq!((left.start, left.end), (right.start, right.end));
            }
        }

        #[test]
        fn plan_chunks_is_empty_without_ranges_or_total(
            total in any::<u64>(),
            chunk_size in any::<u64>(),
        ) {
            prop_assert!(plan_chunks(Some(total), false, chunk_size).is_empty());
            prop_assert!(plan_chunks(None, true, chunk_size).is_empty());
            prop_assert!(plan_chunks(None, false, chunk_size).is_empty());
            prop_assert!(plan_chunks(Some(0), true, chunk_size).is_empty());
        }

        #[test]
        fn resolved_chunk_size_matches_the_strategy(
            strategy in prop_oneof![
                Just(ChunkSizeStrategy::Fixed),
                Just(ChunkSizeStrategy::Adaptive),
            ],
            total in prop::option::of(any::<u64>()),
        ) {
            let resolved = resolve_chunk_size(strategy, total);
            let expected = match strategy {
                ChunkSizeStrategy::Fixed => CHUNK_SIZE,
                ChunkSizeStrategy::Adaptive => total.map(adaptive_chunk_size).unwrap_or(CHUNK_SIZE),
            };
            prop_assert_eq!(resolved, expected);
        }

        #[test]
        fn adaptive_chunk_size_is_tiered_and_monotone(a in any::<u64>(), b in any::<u64>()) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            let small = adaptive_chunk_size(lo);
            let large = adaptive_chunk_size(hi);
            prop_assert!(small <= large);
            let mib = 1024 * 1024;
            prop_assert!([4, 8, 32, 64, 128].contains(&(small / mib)));
            prop_assert!([4, 8, 32, 64, 128].contains(&(large / mib)));
        }

        /// The resume prefix is the end (exclusive) of the longest run of
        /// completed chunks starting at offset 0.
        #[test]
        fn contiguous_prefix_end_tracks_the_completed_prefix(
            total in 1u64..=16 * 1024 * 1024,
            chunk_size in 64 * 1024u64..=4 * 1024 * 1024,
            completed in 0usize..=512,
        ) {
            let mut chunks = plan_chunks(Some(total), true, chunk_size);
            let complete_count = completed.min(chunks.len());
            for chunk in chunks.iter_mut().take(complete_count) {
                chunk.completed = true;
            }
            let expected = if complete_count == 0 {
                0
            } else {
                chunks[complete_count - 1].end + 1
            };
            let manifest = manifest_with(chunks, 0);
            prop_assert_eq!(contiguous_prefix_end(&manifest), expected);
        }

        #[test]
        fn contiguous_prefix_end_falls_back_to_received_without_chunks(
            downloaded in any::<u64>(),
        ) {
            let manifest = manifest_with(Vec::new(), downloaded);
            prop_assert_eq!(contiguous_prefix_end(&manifest), downloaded);
        }

        #[test]
        fn partial_progress_matches_the_chunk_view(
            total in 1u64..=16 * 1024 * 1024,
            chunk_size in 64 * 1024u64..=4 * 1024 * 1024,
            states in prop::collection::vec((any::<bool>(), 0u8..=255), 0..=512),
        ) {
            let mut chunks = plan_chunks(Some(total), true, chunk_size);
            for (chunk, (completed, downloaded)) in chunks.iter_mut().zip(states.iter()) {
                chunk.completed = *completed;
                chunk.downloaded = u64::from(*downloaded) % (chunk.byte_len() + 1);
            }
            let expected = chunks
                .iter()
                .any(|chunk| chunk.downloaded > 0 && !chunk.completed);
            let manifest = manifest_with(chunks, 0);
            prop_assert_eq!(has_partial_chunk_progress(&manifest), expected);
        }

        #[test]
        fn durable_bytes_never_exceeds_received(
            start in any::<u64>(),
            len in 1u64..=1_000_000,
            downloaded in any::<u64>(),
            durable in any::<u64>(),
        ) {
            let chunk = ChunkManifest {
                index: 0,
                start,
                end: start.saturating_add(len - 1),
                downloaded,
                completed: false,
                durable_downloaded: durable,
                claimed_by: None,
                dirty: false,
            };
            prop_assert!(chunk.durable_bytes() <= chunk.downloaded);
            prop_assert_eq!(
                chunk.durable_complete(),
                chunk.durable_bytes() >= chunk.byte_len()
            );
        }
    }
}
