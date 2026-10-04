//! Chunk persistence and cascade behaviour.

use super::*;

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
            durable_downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 500,
            end: 999,
            downloaded: 0,
            durable_downloaded: 0,
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
        durable_downloaded: 100,
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
fn progress_empty_chunks_updates_row_only() {
    let db = Database::open_in_memory().unwrap();

    let mut m = new_test_manifest("empty-chunks", "https://example.com/ec", "ec.bin");
    m.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 499,
        downloaded: 0,
        durable_downloaded: 0,
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
        durable_downloaded: 50,
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
            durable_downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 200,
            end: 399,
            downloaded: 0,
            durable_downloaded: 0,
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
fn chunk_null_claimed_by_roundtrips() {
    let db = Database::open_in_memory().unwrap();
    let mut m = new_test_manifest("claim-none", "https://example.com/cn", "cn.bin");
    m.chunks = vec![ChunkManifest {
        index: 0,
        start: 0,
        end: 99,
        downloaded: 0,
        durable_downloaded: 0,
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
            durable_downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 100,
            end: 199,
            downloaded: 0,
            durable_downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 2,
            start: 200,
            end: 299,
            downloaded: 0,
            durable_downloaded: 0,
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
            durable_downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        },
        ChunkManifest {
            index: 1,
            start: 500,
            end: 999,
            downloaded: 0,
            durable_downloaded: 0,
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
            10,
            "expected user_version = 10 after migration"
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
    let loaded = db
        .get_download("compat-v0-a")
        .unwrap()
        .expect("should exist");
    assert_eq!(loaded.id, "compat-v0-a");
    assert_eq!(loaded.chunk_size, 4194304);
    assert_eq!(db.count_downloads().unwrap(), 1);
    assert!(loaded.mirror_urls.is_empty(), "mirror_urls should be empty");
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
                durable_downloaded: 256,
                completed: false,
                claimed_by: Some(1),
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 512,
                end: 1023,
                downloaded: 512,
                durable_downloaded: 512,
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
