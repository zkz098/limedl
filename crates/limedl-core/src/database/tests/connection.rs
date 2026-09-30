//! Connection setup and reopen persistence.

use super::*;

#[timeout(30_000)]
#[test]
fn open_in_memory_creates_empty_db() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.count_downloads().unwrap(), 0);
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
