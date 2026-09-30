//! Download row CRUD and round-trips.

use super::*;

#[timeout(30_000)]
#[test]
fn insert_and_get_download_roundtrip() {
    let db = Database::open_in_memory().unwrap();
    let mut manifest = new_test_manifest("test-1", "https://example.com/file", "file.bin");
    manifest.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 511,
        downloaded: 0,
        completed: false,
        claimed_by: None,
        dirty: false,
    }];
    db.insert_download(&manifest).unwrap();
    let loaded = db.get_download("test-1").unwrap().expect("should exist");
    assert_eq!(loaded.id, "test-1");
    assert_eq!(loaded.url, "https://example.com/file");
    assert_eq!(loaded.file_name, "file.bin");
    assert_eq!(loaded.state, DownloadState::Queued);
    assert_eq!(loaded.chunks.len(), 1);
}

#[timeout(30_000)]
#[test]
fn insert_or_replace_same_id_replaces() {
    let db = Database::open_in_memory().unwrap();

    let m1 = new_test_manifest("id-1", "https://a.com/f1", "first.txt");
    db.insert_download(&m1).unwrap();
    assert_eq!(db.count_downloads().unwrap(), 1);

    let m2 = new_test_manifest("id-1", "https://b.com/f2", "second.txt");
    db.insert_download(&m2).unwrap();
    assert_eq!(db.count_downloads().unwrap(), 1);

    let loaded = db.get_download("id-1").unwrap().expect("should exist");
    assert_eq!(loaded.file_name, "second.txt");
    assert_eq!(loaded.url, "https://b.com/f2");
}

#[timeout(30_000)]
#[test]
fn get_nonexistent_download_returns_none() {
    let db = Database::open_in_memory().unwrap();
    let result = db.get_download("no-such-id").unwrap();
    assert!(result.is_none());
}

#[timeout(30_000)]
#[test]
fn delete_download_cascades_to_chunks() {
    let db = Database::open_in_memory().unwrap();
    let mut manifest = new_test_manifest("del-1", "https://example.com/file", "delete.bin");
    manifest.chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 511,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 512,
            end: 1023,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
    ];
    db.insert_download(&manifest).unwrap();
    assert_eq!(count_chunks(&db, "del-1"), 2);

    db.delete_download("del-1").unwrap();
    assert!(db.get_download("del-1").unwrap().is_none());
    assert_eq!(count_chunks(&db, "del-1"), 0);
}

#[timeout(30_000)]
#[test]
fn delete_nonexistent_does_not_panic() {
    let db = Database::open_in_memory().unwrap();
    let result = db.delete_download("no-such-id");
    assert!(result.is_ok());
}

#[timeout(30_000)]
#[test]
fn insert_then_get_preserves_all_fields() {
    let db = Database::open_in_memory().unwrap();
    let mut manifest = new_test_manifest("all-fields", "https://example.com/file", "all.txt");
    manifest.state = DownloadState::Downloading;
    manifest.downloaded_bytes = 1024;
    manifest.etag = Some("\"abc123\"".into());
    manifest.thread_mode = ThreadMode::Fixed;
    manifest.requested_thread_count = Some(4);
    manifest.checksum_mode = ChecksumMode::Sha256;
    manifest.checksum = Some("sha256hash".into());
    manifest.expected_checksum = Some("sha256expected".into());
    manifest.error = Some("some error".into());
    manifest.adaptive_profile_snapshot = Some(AdaptiveProfile::Aggressive);
    manifest.thread_note = Some("my thread note".into());
    manifest.total_bytes = Some(99999);
    manifest.last_modified = Some("Mon, 01 Jan 2024 00:00:00 GMT".into());
    manifest.desired_thread_count = Some(6);
    manifest.allocated_thread_count = Some(4);
    manifest.final_url = "https://redirect.example.com/file".to_string();
    manifest.user_agent = "custom-agent/1.0".to_string();
    manifest.destination_dir = "/custom/path".to_string();
    manifest.file_name_locked = false;
    manifest.destination_path = "/custom/path/all.txt".to_string();
    manifest.temp_path = "/custom/path/all.txt.tmp".to_string();
    manifest.supports_ranges = false;
    manifest.connection_count = 3;
    manifest.chunk_size = 8192;
    manifest.created_at_ms = 5000;
    manifest.updated_at_ms = 6000;
    manifest.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 500,
        downloaded: 500,
        completed: true,
        claimed_by: Some(1),
        dirty: false,
    }];

    manifest.cdn_accelerated = true;
    manifest.cdn_node_ip = Some("1.2.3.4".to_string());

    db.insert_download(&manifest).unwrap();
    let loaded = db
        .get_download("all-fields")
        .unwrap()
        .expect("should exist");

    assert_eq!(loaded.id, "all-fields");
    assert_eq!(loaded.url, "https://example.com/file");
    assert_eq!(loaded.final_url, "https://redirect.example.com/file");
    assert_eq!(loaded.user_agent, "custom-agent/1.0");
    assert_eq!(loaded.destination_dir, "/custom/path");
    assert_eq!(loaded.file_name, "all.txt");
    assert!(!loaded.file_name_locked);
    assert_eq!(loaded.destination_path, "/custom/path/all.txt");
    assert_eq!(loaded.temp_path, "/custom/path/all.txt.tmp");
    assert_eq!(loaded.total_bytes, Some(99999));
    assert_eq!(loaded.downloaded_bytes, 1024);
    assert!(!loaded.supports_ranges);
    assert_eq!(loaded.connection_count, 3);
    assert_eq!(loaded.thread_mode, ThreadMode::Fixed);
    assert_eq!(loaded.requested_thread_count, Some(4));
    assert_eq!(loaded.desired_thread_count, Some(6));
    assert_eq!(loaded.allocated_thread_count, Some(4));
    assert_eq!(
        loaded.adaptive_profile_snapshot,
        Some(AdaptiveProfile::Aggressive)
    );
    assert_eq!(loaded.thread_note.as_deref(), Some("my thread note"));
    assert_eq!(loaded.etag.as_deref(), Some("\"abc123\""));
    assert_eq!(
        loaded.last_modified.as_deref(),
        Some("Mon, 01 Jan 2024 00:00:00 GMT")
    );
    assert_eq!(loaded.state, DownloadState::Downloading);
    assert_eq!(loaded.checksum_mode, ChecksumMode::Sha256);
    assert_eq!(loaded.checksum.as_deref(), Some("sha256hash"));
    assert_eq!(loaded.expected_checksum.as_deref(), Some("sha256expected"));
    assert_eq!(loaded.error.as_deref(), Some("some error"));
    assert_eq!(loaded.created_at_ms, 5000);
    assert_eq!(loaded.updated_at_ms, 6000);
    assert_eq!(loaded.chunks.len(), 1);
    assert_eq!(loaded.chunks[0].index, 0);
    assert_eq!(loaded.chunks[0].start, 0);
    assert_eq!(loaded.chunks[0].end, 500);
    assert_eq!(loaded.chunks[0].downloaded, 500);
    assert!(loaded.chunks[0].completed);
    assert_eq!(loaded.chunks[0].claimed_by, Some(1));
    assert!(!loaded.chunks[0].dirty);
    assert_eq!(loaded.chunk_size, 8192);
    assert!(loaded.cdn_accelerated);
    assert_eq!(
        loaded.cdn_node_ip.as_deref(),
        Some("1.2.3.4"),
        "cdn_node_ip should be preserved through DB round-trip"
    );
}

#[timeout(30_000)]
#[test]
fn update_download_modifies_all_fields() {
    let db = Database::open_in_memory().unwrap();

    let mut m = new_test_manifest("upd", "https://a.com/old", "old.txt");
    m.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 499,
        downloaded: 0,
        completed: false,
        claimed_by: None,
        dirty: false,
    }];
    db.insert_download(&m).unwrap();

    let mut updated = new_test_manifest("upd", "https://b.com/new", "new.txt");
    updated.state = DownloadState::Completed;
    updated.downloaded_bytes = 500;
    updated.updated_at_ms = 9999;
    updated.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 499,
        downloaded: 500,
        completed: true,
        claimed_by: None,
        dirty: false,
    }];
    db.update_download(&updated).unwrap();

    let loaded = db.get_download("upd").unwrap().expect("should exist");
    assert_eq!(loaded.state, DownloadState::Completed);
    assert_eq!(loaded.downloaded_bytes, 500);
    assert_eq!(loaded.file_name, "new.txt");
    assert_eq!(loaded.url, "https://b.com/new");
    assert_eq!(loaded.updated_at_ms, 9999);
    assert_eq!(loaded.chunks.len(), 1);
    assert!(loaded.chunks[0].completed);
    assert_eq!(loaded.chunks[0].downloaded, 500);
}

#[timeout(30_000)]
#[test]
fn update_download_progress_incremental() {
    let db = Database::open_in_memory().unwrap();

    let mut m = new_test_manifest("prog", "https://example.com/prog", "progress.bin");
    m.chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 499,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 500,
            end: 999,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
    ];
    db.insert_download(&m).unwrap();

    let dirty_chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 499,
        downloaded: 250,
        completed: false,
        claimed_by: Some(0),
        dirty: true,
    }];
    db.update_download_progress("prog", 250, &dirty_chunks, "downloading", 2000)
        .unwrap();

    let loaded = db.get_download("prog").unwrap().expect("should exist");
    assert_eq!(loaded.downloaded_bytes, 250);
    assert_eq!(loaded.state, DownloadState::Downloading);
    assert_eq!(loaded.updated_at_ms, 2000);
    assert_eq!(loaded.chunks.len(), 2);
    assert_eq!(loaded.chunks[0].downloaded, 250);
    assert!(!loaded.chunks[0].completed);
    assert_eq!(loaded.chunks[0].claimed_by, Some(0));
    assert_eq!(loaded.chunks[1].downloaded, 0);
    assert!(!loaded.chunks[1].completed);
    assert_eq!(loaded.url, "https://example.com/prog");
    assert_eq!(loaded.file_name, "progress.bin");
}

#[timeout(30_000)]
#[test]
fn count_downloads_accurate() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.count_downloads().unwrap(), 0);

    let m1 = new_test_manifest("cnt-1", "https://a.com/f1", "f1.bin");
    db.insert_download(&m1).unwrap();
    assert_eq!(db.count_downloads().unwrap(), 1);

    let m2 = new_test_manifest("cnt-2", "https://b.com/f2", "f2.bin");
    db.insert_download(&m2).unwrap();
    assert_eq!(db.count_downloads().unwrap(), 2);

    db.delete_download("cnt-1").unwrap();
    assert_eq!(db.count_downloads().unwrap(), 1);

    db.delete_download("cnt-2").unwrap();
    assert_eq!(db.count_downloads().unwrap(), 0);
}

#[timeout(30_000)]
#[test]
fn null_optionals_roundtrip() {
    let db = Database::open_in_memory().unwrap();
    let mut manifest = new_test_manifest("null-opt", "https://example.com/null", "null.bin");
    manifest.total_bytes = None;
    manifest.etag = None;
    manifest.last_modified = None;
    manifest.checksum = None;
    manifest.error = None;
    manifest.requested_thread_count = None;
    manifest.desired_thread_count = None;
    manifest.allocated_thread_count = None;
    manifest.adaptive_profile_snapshot = None;
    manifest.thread_note = None;
    manifest.supports_ranges = false;
    manifest.chunks = Vec::new();

    db.insert_download(&manifest).unwrap();
    let loaded = db.get_download("null-opt").unwrap().expect("should exist");

    assert!(loaded.total_bytes.is_none());
    assert!(loaded.etag.is_none());
    assert!(loaded.last_modified.is_none());
    assert!(loaded.checksum.is_none());
    assert!(loaded.error.is_none());
    assert!(loaded.requested_thread_count.is_none());
    assert!(loaded.desired_thread_count.is_none());
    assert!(loaded.allocated_thread_count.is_none());
    assert!(loaded.adaptive_profile_snapshot.is_none());
    assert!(loaded.thread_note.is_none());
    assert!(loaded.chunks.is_empty());
}
