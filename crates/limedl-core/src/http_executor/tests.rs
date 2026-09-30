use super::worker::*;

use crate::manifest::ChunkManifest;
use crate::types::{ChecksumMode, DownloadState, Priority, ThreadMode};

fn make_test_manifest(chunks: Vec<ChunkManifest>) -> crate::manifest::Manifest {
    crate::manifest::Manifest {
        id: "test-task".to_string(),
        url: "https://example.com/test.bin".to_string(),
        final_url: "https://example.com/test.bin".to_string(),
        user_agent: "limedl".to_string(),
        extra_headers: Vec::new(),
        destination_dir: "/tmp".to_string(),
        file_name: "test.bin".to_string(),
        file_name_locked: false,
        destination_path: "/tmp/test.bin".to_string(),
        temp_path: "/tmp/test.bin.part".to_string(),
        total_bytes: Some(chunks.iter().map(|c| c.end - c.start + 1).sum()),
        downloaded_bytes: chunks.iter().map(|c| c.downloaded).sum(),
        supports_ranges: true,
        chunk_size: 4 * 1024 * 1024,
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
        created_at_ms: 0,
        updated_at_ms: 0,
        chunks,
        mirror_url: None,
        mirror_urls: Vec::new(),
        current_mirror_index: 0,
        priority: Priority::Normal,
    }
}

#[test]
fn test_claim_next_chunk_stripe_and_fallback() {
    let chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 999,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 1000,
            end: 1999,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
    ];
    let mut manifest = make_test_manifest(chunks);

    let claimed_0 = claim_next_chunk(&mut manifest, 0, 2);
    assert!(claimed_0.is_some());
    assert_eq!(claimed_0.unwrap().index, 0);
    assert_eq!(manifest.chunks[0].claimed_by, Some(0));

    let claimed_1 = claim_next_chunk(&mut manifest, 1, 2);
    assert!(claimed_1.is_some());
    assert_eq!(claimed_1.unwrap().index, 1);
    assert_eq!(manifest.chunks[1].claimed_by, Some(1));

    let claimed_none = claim_next_chunk(&mut manifest, 0, 2);
    assert!(claimed_none.is_none());
}

#[test]
fn test_steal_chunk_splits_largest_remaining() {
    let chunks = vec![
        // Chunk 0: 0..9_999_999 (10 MB), 2 MB downloaded, remaining 8 MB
        ChunkManifest {
            index: 0,
            start: 0,
            end: 9_999_999,
            downloaded: 2_000_000,
            completed: false,
            claimed_by: Some(0),
            dirty: false,
        },
        // Chunk 1: 10_000_000..19_999_999 (10 MB), 7 MB downloaded, remaining 3 MB
        ChunkManifest {
            index: 1,
            start: 10_000_000,
            end: 19_999_999,
            downloaded: 7_000_000,
            completed: false,
            claimed_by: Some(1),
            dirty: false,
        },
    ];
    let mut manifest = make_test_manifest(chunks);

    // Worker 2 attempts work stealing
    let stolen = steal_chunk(&mut manifest, 2);
    assert!(stolen.is_some(), "should successfully steal a chunk");
    let stolen_chunk = stolen.unwrap();

    // Should have targeted Chunk 0 (8 MB remaining vs 3 MB remaining)
    assert_eq!(stolen_chunk.index, 2);
    assert_eq!(stolen_chunk.claimed_by, Some(2));
    assert_eq!(stolen_chunk.downloaded, 0);
    assert!(!stolen_chunk.completed);

    // Remaining was 8_000_000 bytes (from 2_000_000 to 9_999_999)
    // Midpoint = 2_000_000 + 3_999_999 = 5_999_999
    // Chunk 0 end should now be 5_999_999 (covers 0..5_999_999, which is 6 MB)
    assert_eq!(manifest.chunks[0].end, 5_999_999);
    assert!(manifest.chunks[0].dirty);

    // Stolen chunk should cover 6_000_000..9_999_999 (4 MB)
    assert_eq!(stolen_chunk.start, 6_000_000);
    assert_eq!(stolen_chunk.end, 9_999_999);

    // Total chunks should now be 3
    assert_eq!(manifest.chunks.len(), 3);
}

#[test]
fn test_steal_chunk_ignores_small_remaining() {
    let chunks = vec![
        // Chunk 0: only 1 MB remaining (< 2 MB threshold)
        ChunkManifest {
            index: 0,
            start: 0,
            end: 1_000_000,
            downloaded: 0,
            completed: false,
            claimed_by: Some(0),
            dirty: false,
        },
    ];
    let mut manifest = make_test_manifest(chunks);

    let stolen = steal_chunk(&mut manifest, 1);
    assert!(
        stolen.is_none(),
        "should not steal chunks smaller than threshold"
    );
    assert_eq!(manifest.chunks.len(), 1);
}

#[test]
fn test_claim_or_steal_chunk_prefers_unclaimed() {
    let chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 10_000_000,
            downloaded: 0,
            completed: false,
            claimed_by: Some(0),
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 10_000_001,
            end: 20_000_000,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
    ];
    let mut manifest = make_test_manifest(chunks);

    let claimed = claim_or_steal_chunk(&mut manifest, 1, 2);
    assert!(claimed.is_some());
    assert_eq!(claimed.unwrap().index, 1);
    // Chunk 0 end should not be modified
    assert_eq!(manifest.chunks[0].end, 10_000_000);
    assert_eq!(manifest.chunks.len(), 2);
}
