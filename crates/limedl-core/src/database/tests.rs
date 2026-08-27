use ntest::timeout;
use rusqlite::{params, Connection};
use std::sync::Arc;

use super::connection::Database;
use super::schema::{table_has_column, Migration, CREATE_TABLES_SQL};
use crate::error::DownloadError;
use crate::manifest::{ChunkManifest, Manifest, CHUNK_SIZE};
use crate::types::{
    default_http_user_agent, AdaptiveProfile, ChecksumMode, DownloadState, Priority, ThreadMode,
};

/// Helper: create a `Manifest` with sensible defaults for testing.
fn new_test_manifest(id: &str, url: &str, file_name: &str) -> Manifest {
    Manifest {
        id: id.to_string(),
        url: url.to_string(),
        final_url: url.to_string(),
        user_agent: default_http_user_agent(),
        extra_headers: vec![],
        destination_dir: "/tmp".to_string(),
        file_name: file_name.to_string(),
        file_name_locked: true,
        destination_path: format!("/tmp/{file_name}"),
        temp_path: format!("/tmp/{file_name}.tmp"),
        total_bytes: Some(1024),
        downloaded_bytes: 0,
        supports_ranges: true,
        chunk_size: CHUNK_SIZE,
        connection_count: 1,
        thread_mode: ThreadMode::Adaptive,
        requested_thread_count: None,
        desired_thread_count: None,
        allocated_thread_count: None,
        adaptive_profile_snapshot: None,
        thread_note: None,
        etag: None,
        last_modified: None,
        state: DownloadState::Queued,
        cdn_accelerated: false,
        cdn_node_ip: None,
        checksum_mode: ChecksumMode::Blake3,
        checksum: None,
        expected_checksum: None,
        error: None,
        created_at_ms: 1000,
        updated_at_ms: 1000,
        chunks: Vec::new(),
        mirror_url: None,
        mirror_urls: Vec::new(),
        current_mirror_index: 0,
        priority: Priority::Normal,
    }
}

/// Helper: count chunks for a download by querying the chunks table directly.
fn count_chunks(db: &Database, download_id: &str) -> usize {
    let conn = db.lock_read();
    conn.query_row(
        "SELECT COUNT(*) FROM chunks WHERE download_id = ?1",
        params![download_id],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0) as usize
}

#[timeout(30_000)]
#[test]
fn open_in_memory_creates_empty_db() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.count_downloads().unwrap(), 0);
}

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
fn insert_existing_incremental_chunks() {
    let db = Database::open_in_memory().unwrap();

    let mut orig = new_test_manifest("dup", "https://a.com/orig", "original.txt");
    orig.chunks = vec![
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
    db.insert_download(&orig).unwrap();
    assert_eq!(db.count_downloads().unwrap(), 1);

    let mut replacement = new_test_manifest("dup", "https://b.com/replaced", "replaced.txt");
    replacement.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 199,
        downloaded: 100,
        completed: false,
        claimed_by: None,
        dirty: true,
    }];
    db.insert_download(&replacement).unwrap();

    let loaded = db.get_download("dup").unwrap().expect("should exist");
    assert_eq!(loaded.url, "https://b.com/replaced");
    assert_eq!(loaded.file_name, "replaced.txt");
    assert_eq!(loaded.chunks.len(), 2);
    assert_eq!(loaded.chunks[0].downloaded, 100);
    assert_eq!(loaded.chunks[0].end, 199);
    assert_eq!(loaded.chunks[1].downloaded, 0);
    assert_eq!(loaded.chunks[1].end, 999);
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
fn progress_empty_chunks_updates_row_only() {
    let db = Database::open_in_memory().unwrap();

    let mut m = new_test_manifest("empty-chunks", "https://example.com/ec", "ec.bin");
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

    db.update_download_progress("empty-chunks", 100, &[], "downloading", 3000)
        .unwrap();

    let loaded = db
        .get_download("empty-chunks")
        .unwrap()
        .expect("should exist");
    assert_eq!(loaded.downloaded_bytes, 100);
    assert_eq!(loaded.state, DownloadState::Downloading);
    assert_eq!(loaded.updated_at_ms, 3000);
    assert_eq!(loaded.chunks.len(), 1);
    assert_eq!(loaded.chunks[0].downloaded, 0);
}

#[timeout(30_000)]
#[test]
fn list_returns_all_with_chunks() {
    let db = Database::open_in_memory().unwrap();

    let mut m1 = new_test_manifest("list-1", "https://a.com/f1", "first.txt");
    m1.created_at_ms = 2000;
    m1.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 99,
        downloaded: 50,
        completed: false,
        claimed_by: None,
        dirty: false,
    }];
    db.insert_download(&m1).unwrap();

    let mut m2 = new_test_manifest("list-2", "https://b.com/f2", "second.txt");
    m2.created_at_ms = 1000;
    m2.chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 199,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 200,
            end: 399,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
    ];
    db.insert_download(&m2).unwrap();

    let list = db.list_downloads().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, "list-1");
    assert_eq!(list[1].id, "list-2");
    assert_eq!(list[0].chunks.len(), 1);
    assert_eq!(list[1].chunks.len(), 2);
}

#[timeout(30_000)]
#[test]
fn empty_chunks_persisted_and_loaded() {
    let db = Database::open_in_memory().unwrap();
    let manifest = new_test_manifest("no-chunks", "https://example.com/nc", "nochunks.bin");
    assert!(manifest.chunks.is_empty());
    db.insert_download(&manifest).unwrap();
    let loaded = db.get_download("no-chunks").unwrap().expect("should exist");
    assert!(loaded.chunks.is_empty());
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

#[timeout(30_000)]
#[test]
fn chunk_null_claimed_by_roundtrips() {
    let db = Database::open_in_memory().unwrap();
    let mut m = new_test_manifest("claim-none", "https://example.com/cn", "cn.bin");
    m.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 99,
        downloaded: 0,
        completed: false,
        claimed_by: None,
        dirty: false,
    }];
    db.insert_download(&m).unwrap();
    let loaded = db
        .get_download("claim-none")
        .unwrap()
        .expect("should exist");
    assert_eq!(loaded.chunks.len(), 1);
    assert!(loaded.chunks[0].claimed_by.is_none());
}

#[timeout(30_000)]
#[test]
fn list_download_headers_returns_no_chunks() {
    let db = Database::open_in_memory().unwrap();

    let mut m = new_test_manifest("hdr", "https://example.com/hdr", "header.bin");
    m.chunks = vec![
        ChunkManifest {
            index: 0,
            start: 0,
            end: 99,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 100,
            end: 199,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 2,
            start: 200,
            end: 299,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
    ];
    db.insert_download(&m).unwrap();

    let headers = db.list_download_headers().unwrap();
    assert_eq!(headers.len(), 1);
    assert!(
        headers[0].chunks.is_empty(),
        "chunks should not be populated"
    );
}

#[timeout(30_000)]
#[test]
fn load_chunks_returns_chunks_on_demand() {
    let db = Database::open_in_memory().unwrap();

    let mut m = new_test_manifest("chk-id", "https://example.com/chk", "chunk.bin");
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

    let chunks = db.load_chunks("chk-id").unwrap();
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].index, 0);
    assert_eq!(chunks[0].start, 0);
    assert_eq!(chunks[0].end, 499);
    assert_eq!(chunks[1].index, 1);
    assert_eq!(chunks[1].start, 500);
    assert_eq!(chunks[1].end, 999);
}

#[timeout(30_000)]
#[test]
fn load_chunks_nonexistent_returns_empty() {
    let db = Database::open_in_memory().unwrap();
    let chunks = db.load_chunks("no-such-download").unwrap();
    assert!(chunks.is_empty());
}

#[timeout(30_000)]
#[test]
fn table_has_column_detects_existing_column() {
    let db = Database::open_in_memory().unwrap();
    let conn = db.lock_read();

    let mut stmt = conn.prepare("PRAGMA table_info(downloads)").unwrap();
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(
        columns.contains(&"chunk_size".to_string()),
        "chunk_size column should exist"
    );
    assert!(
        columns.contains(&"mirror_urls".to_string()),
        "mirror_urls column should exist"
    );
    assert!(
        columns.contains(&"mirror_url".to_string()),
        "mirror_url column should exist"
    );
    assert!(
        columns.contains(&"current_mirror_index".to_string()),
        "current_mirror_index column should exist"
    );
}

fn create_v1_schema(conn: &Connection) {
    conn.execute_batch(CREATE_TABLES_SQL).unwrap();
}

fn read_user_version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn apply_v2(conn: &Connection) {
    conn.execute(
        "ALTER TABLE downloads ADD COLUMN chunk_size INTEGER NOT NULL DEFAULT 4194304",
        [],
    )
    .unwrap();
}

fn apply_v3(conn: &Connection) {
    conn.execute("ALTER TABLE downloads ADD COLUMN mirror_url TEXT", [])
        .unwrap();
    conn.execute(
        "ALTER TABLE downloads ADD COLUMN mirror_urls TEXT NOT NULL DEFAULT '[]'",
        [],
    )
    .unwrap();
    conn.execute(
        "ALTER TABLE downloads ADD COLUMN current_mirror_index INTEGER NOT NULL DEFAULT 0",
        [],
    )
    .unwrap();
}

#[timeout(30_000)]
#[test]
fn migration_compat_v0_with_chunk_size_backfilled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let conn = Connection::open(&path).unwrap();
        create_v1_schema(&conn);
        apply_v2(&conn);
        assert_eq!(read_user_version(&conn), 0);
    }

    let db = Database::open(&path).unwrap();

    {
        let conn = db.lock_write();
        assert_eq!(
            read_user_version(&conn),
            8,
            "expected user_version = 8 after migration"
        );
        let has_mirror_urls = table_has_column(&conn, "downloads", "mirror_urls").unwrap();
        assert!(
            has_mirror_urls,
            "mirror_urls column should exist after migration"
        );
    }

    let mut m = new_test_manifest("compat-v0-a", "https://example.com/a", "a.bin");
    m.chunk_size = 4194304;
    db.insert_download(&m).unwrap();
    let loaded = db.get_download("compat-v0-a").unwrap().expect("should exist");
    assert_eq!(loaded.id, "compat-v0-a");
    assert_eq!(loaded.chunk_size, 4194304);
    assert_eq!(db.count_downloads().unwrap(), 1);
    assert!(loaded.mirror_urls.is_empty(), "mirror_urls should be empty");
}

#[timeout(30_000)]
#[test]
fn migration_compat_v1_with_mirror_columns_backfilled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let conn = Connection::open(&path).unwrap();
        create_v1_schema(&conn);
        apply_v2(&conn);
        apply_v3(&conn);
        conn.pragma_update(None, "user_version", 1).unwrap();
        assert_eq!(read_user_version(&conn), 1);
    }

    let db = Database::open(&path).unwrap();

    {
        let conn = db.lock_write();
        assert_eq!(
            read_user_version(&conn),
            8,
            "expected user_version = 8 after migration"
        );
    }

    let mut m = new_test_manifest("compat-v1-b", "https://example.com/b", "b.bin");
    m.chunk_size = 4194304;
    m.mirror_urls = vec!["https://mirror.example.com/b".into()];
    m.current_mirror_index = 0;
    db.insert_download(&m).unwrap();
    let loaded = db.get_download("compat-v1-b").unwrap().expect("should exist");
    assert_eq!(loaded.chunk_size, 4194304);
    assert_eq!(loaded.mirror_urls.len(), 1);
    assert_eq!(db.count_downloads().unwrap(), 1);
}

#[timeout(30_000)]
#[test]
fn migration_compat_v0_fully_backfilled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let conn = Connection::open(&path).unwrap();
        create_v1_schema(&conn);
        apply_v2(&conn);
        apply_v3(&conn);
        assert_eq!(read_user_version(&conn), 0);
    }

    let db = Database::open(&path).unwrap();

    {
        let conn = db.lock_write();
        assert_eq!(
            read_user_version(&conn),
            8,
            "expected user_version = 8 after migration"
        );
    }

    let mut m = new_test_manifest("compat-v0-c", "https://example.com/c", "c.bin");
    m.chunk_size = 2097152;
    m.mirror_url = Some("https://mirror.example.com/c".into());
    m.mirror_urls = vec![
        "https://mirror1.example.com/c".into(),
        "https://mirror2.example.com/c".into(),
    ];
    m.current_mirror_index = 1;
    db.insert_download(&m).unwrap();
    let loaded = db.get_download("compat-v0-c").unwrap().expect("should exist");
    assert_eq!(loaded.chunk_size, 2097152);
    assert_eq!(
        loaded.mirror_url.as_deref(),
        Some("https://mirror.example.com/c")
    );
    assert_eq!(loaded.mirror_urls.len(), 2);
    assert_eq!(loaded.current_mirror_index, 1);
    assert_eq!(db.count_downloads().unwrap(), 1);
}

#[test]
fn empty_migrations_returns_err() {
    let empty: &[Migration] = &[];
    let result = empty
        .last()
        .map(|m| m.version)
        .ok_or_else(|| DownloadError::DatabaseInit("no migrations defined".into()));
    assert!(result.is_err(), "expected Err for empty migrations slice");
    let err = result.unwrap_err();
    assert_eq!(
        err.kind(),
        "database_init",
        "unexpected error kind: {}",
        err.kind()
    );
    assert!(
        err.to_string().contains("database initialization error"),
        "unexpected error message: {}",
        err
    );
}

#[timeout(30_000)]
#[test]
fn open_creates_new_database_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    assert!(!path.exists(), "database file should not exist before open");

    let db = Database::open(&path).unwrap();
    drop(db);

    assert!(
        path.exists(),
        "database file should exist after Database::open"
    );
}

#[timeout(30_000)]
#[test]
fn insert_and_reopen_preserves_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let db = Database::open(&path).unwrap();
        let manifest = new_test_manifest("persist-1", "https://example.com/file", "file.bin");
        db.insert_download(&manifest).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);
    }

    {
        let db = Database::open(&path).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);
        let loaded = db
            .get_download("persist-1")
            .unwrap()
            .expect("should exist after reopen");
        assert_eq!(loaded.id, "persist-1");
        assert_eq!(loaded.url, "https://example.com/file");
        assert_eq!(loaded.file_name, "file.bin");
        assert_eq!(loaded.state, DownloadState::Queued);
    }
}

#[timeout(30_000)]
#[test]
fn update_persists_across_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let db = Database::open(&path).unwrap();
        let mut manifest = new_test_manifest("upd-reopen", "https://example.com/old", "old.txt");
        manifest.state = DownloadState::Queued;
        manifest.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 499,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        }];
        db.insert_download(&manifest).unwrap();

        let mut updated = new_test_manifest("upd-reopen", "https://example.com/new", "new.txt");
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
    }

    {
        let db = Database::open(&path).unwrap();
        let loaded = db
            .get_download("upd-reopen")
            .unwrap()
            .expect("should exist after reopen");
        assert_eq!(loaded.url, "https://example.com/new");
        assert_eq!(loaded.file_name, "new.txt");
        assert_eq!(loaded.state, DownloadState::Completed);
        assert_eq!(loaded.downloaded_bytes, 500);
        assert_eq!(loaded.updated_at_ms, 9999);
        assert_eq!(loaded.chunks.len(), 1);
        assert!(loaded.chunks[0].completed);
        assert_eq!(loaded.chunks[0].downloaded, 500);
    }
}

#[timeout(30_000)]
#[test]
fn delete_persists_across_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let db = Database::open(&path).unwrap();
        let manifest = new_test_manifest("del-reopen", "https://example.com/del", "del.bin");
        db.insert_download(&manifest).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);

        db.delete_download("del-reopen").unwrap();
        assert_eq!(db.count_downloads().unwrap(), 0);
    }

    {
        let db = Database::open(&path).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 0);
        let loaded = db.get_download("del-reopen").unwrap();
        assert!(
            loaded.is_none(),
            "deleted download should not exist after reopening"
        );
    }
}

#[timeout(30_000)]
#[test]
fn chunks_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let db = Database::open(&path).unwrap();
        let mut manifest =
            new_test_manifest("chunks-reopen", "https://example.com/chunks", "chunks.bin");
        manifest.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 511,
                downloaded: 256,
                completed: false,
                claimed_by: Some(1),
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 512,
                end: 1023,
                downloaded: 512,
                completed: true,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&manifest).unwrap();
        assert_eq!(count_chunks(&db, "chunks-reopen"), 2);
    }

    {
        let db = Database::open(&path).unwrap();
        let loaded = db
            .get_download("chunks-reopen")
            .unwrap()
            .expect("should exist after reopen");
        assert_eq!(loaded.chunks.len(), 2);
        assert_eq!(loaded.chunks[0].index, 0);
        assert_eq!(loaded.chunks[0].start, 0);
        assert_eq!(loaded.chunks[0].end, 511);
        assert_eq!(loaded.chunks[0].downloaded, 256);
        assert!(!loaded.chunks[0].completed);
        assert_eq!(loaded.chunks[0].claimed_by, Some(1));
        assert_eq!(loaded.chunks[1].index, 1);
        assert_eq!(loaded.chunks[1].start, 512);
        assert_eq!(loaded.chunks[1].end, 1023);
        assert_eq!(loaded.chunks[1].downloaded, 512);
        assert!(loaded.chunks[1].completed);
        assert!(loaded.chunks[1].claimed_by.is_none());
    }
}

#[timeout(30_000)]
#[test]
fn concurrent_insert_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    let db_reader = db.clone();
    let reader = std::thread::spawn(move || {
        for i in 0..100 {
            let _ = db_reader.get_download("concurrent-1");
            let _ = db_reader.count_downloads();
            if i % 10 == 0 {
                std::thread::yield_now();
            }
        }
        let _ = db_reader.get_download("concurrent-1");
    });

    let db_writer = db.clone();
    let writer = std::thread::spawn(move || {
        let manifest = new_test_manifest("concurrent-1", "https://example.com/con", "con.bin");
        db_writer.insert_download(&manifest).unwrap();
    });

    reader.join().expect("reader thread panicked");
    writer.join().expect("writer thread panicked");

    let loaded = db
        .get_download("concurrent-1")
        .unwrap()
        .expect("should exist after concurrent access");
    assert_eq!(loaded.url, "https://example.com/con");
    assert_eq!(db.count_downloads().unwrap(), 1);
}

#[timeout(30_000)]
#[test]
fn concurrent_multiple_writes_different_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    let mut handles = Vec::new();
    for i in 0..5 {
        let db_clone = db.clone();
        let id = format!("con-write-{i}");
        handles.push(std::thread::spawn(move || {
            let manifest = new_test_manifest(
                &id,
                &format!("https://example.com/file{i}"),
                &format!("file{i}.bin"),
            );
            db_clone.insert_download(&manifest).unwrap();
        }));
    }

    for handle in handles {
        handle.join().expect("writer thread panicked");
    }

    assert_eq!(db.count_downloads().unwrap(), 5);
    for i in 0..5 {
        let id = format!("con-write-{i}");
        let loaded = db
            .get_download(&id)
            .unwrap()
            .unwrap_or_else(|| panic!("download {id} should exist"));
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.file_name, format!("file{i}.bin"));
        assert_eq!(loaded.url, format!("https://example.com/file{i}"));
    }
}

#[timeout(30_000)]
#[test]
fn concurrent_read_write_same_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    {
        let initial = new_test_manifest("same-id", "https://example.com/initial", "initial.bin");
        db.insert_download(&initial).unwrap();
    }

    let db_writer = db.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..15 {
            let mut manifest =
                new_test_manifest("same-id", "https://example.com/updated", "updated.bin");
            manifest.downloaded_bytes = (i as u64 + 1) * 100;
            manifest.updated_at_ms = i as u64;
            manifest.state = DownloadState::Downloading;
            db_writer.update_download(&manifest).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
    });

    let db_reader = db.clone();
    let reader = std::thread::spawn(move || {
        for _ in 0..30 {
            let result = db_reader.get_download("same-id");
            assert!(
                result.is_ok(),
                "reader should not encounter DB errors during concurrent access"
            );
            if let Ok(Some(manifest)) = result {
                assert_eq!(manifest.id, "same-id");
                assert!(
                    manifest.downloaded_bytes <= 1500,
                    "downloaded_bytes should be bounded: got {}",
                    manifest.downloaded_bytes
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    writer.join().expect("writer thread panicked");
    reader.join().expect("reader thread panicked");

    let loaded = db
        .get_download("same-id")
        .unwrap()
        .expect("should exist after concurrent rw");
    assert_eq!(loaded.downloaded_bytes, 1500);
    assert_eq!(loaded.updated_at_ms, 14);
    assert_eq!(loaded.state, DownloadState::Downloading);
}

#[timeout(30_000)]
#[test]
fn concurrent_load_chunks_while_saving() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let db = Arc::new(Database::open(&path).unwrap());

    {
        let mut manifest =
            new_test_manifest("chunks-con", "https://example.com/cc", "cc.bin");
        manifest.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 500,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 500,
                end: 1000,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&manifest).unwrap();
    }

    let db_writer = db.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..20 {
            let progress = (i as u64 + 1) * 50;
            let mut manifest =
                new_test_manifest("chunks-con", "https://example.com/cc", "cc.bin");
            manifest.downloaded_bytes = progress * 2;
            manifest.chunks = vec![
                ChunkManifest {
                    index: 0,
                    start: 0,
                    end: 500,
                    downloaded: progress.min(500),
                    completed: progress >= 500,
                    claimed_by: None,
                    dirty: false,
                },
                ChunkManifest {
                    index: 1,
                    start: 500,
                    end: 1000,
                    downloaded: progress.saturating_sub(500),
                    completed: progress >= 1000,
                    claimed_by: None,
                    dirty: false,
                },
            ];
            db_writer.update_download(&manifest).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    });

    let db_reader = db.clone();
    let reader = std::thread::spawn(move || {
        for _ in 0..20 {
            let result = db_reader.get_download("chunks-con");
            assert!(
                result.is_ok(),
                "get_download should not error during concurrent chunk writes"
            );
            if let Ok(Some(manifest)) = &result {
                for chunk in &manifest.chunks {
                    assert!(
                        chunk.downloaded <= chunk.end - chunk.start,
                        "chunk {} download {} exceeds range {}-{}",
                        chunk.index,
                        chunk.downloaded,
                        chunk.start,
                        chunk.end
                    );
                }
            }
            let chunks = db_reader.load_chunks("chunks-con");
            assert!(
                chunks.is_ok(),
                "load_chunks should not error during concurrent chunk writes"
            );
            if let Ok(chunks) = &chunks {
                for chunk in chunks {
                    assert!(
                        chunk.downloaded <= chunk.end - chunk.start,
                        "chunk {} download {} exceeds range {}-{}",
                        chunk.index,
                        chunk.downloaded,
                        chunk.start,
                        chunk.end
                    );
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    writer.join().expect("writer thread panicked");
    reader.join().expect("reader thread panicked");

    let loaded = db
        .get_download("chunks-con")
        .unwrap()
        .expect("should exist after concurrent chunk access");
    assert_eq!(loaded.downloaded_bytes, 2000);
    assert_eq!(loaded.chunks.len(), 2);
    assert!(loaded.chunks[0].completed);
    assert_eq!(loaded.chunks[0].downloaded, 500);
    assert!(loaded.chunks[1].completed);
    assert_eq!(loaded.chunks[1].downloaded, 500);
}
