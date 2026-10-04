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
            durable_downloaded: 0,
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
            durable_downloaded: 500,
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

// ---------------------------------------------------------------------------
// Integrity check / quarantine
// ---------------------------------------------------------------------------

/// Every `.corrupt-*` file left in `dir`.
fn quarantined_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| name.contains(".corrupt-"))
        })
        .collect()
}

#[timeout(30_000)]
#[test]
fn a_healthy_database_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let db = Database::open(&path).unwrap();
        db.insert_download(&new_test_manifest("keep", "https://example.com/keep", "k.bin"))
            .unwrap();
    }

    // Reopening must not quarantine a database that passes its integrity check.
    let db = Database::open(&path).unwrap();
    assert_eq!(db.count_downloads().unwrap(), 1);
    assert!(
        quarantined_files(dir.path()).is_empty(),
        "a healthy database must not be quarantined"
    );
}

#[timeout(30_000)]
#[test]
fn an_unreadable_database_is_quarantined_and_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    std::fs::write(&path, b"this is not a sqlite database at all").unwrap();

    let db = Database::open(&path).expect("a corrupt database must not block startup");
    assert_eq!(
        db.count_downloads().unwrap(),
        0,
        "the replacement database must be empty"
    );
    drop(db);

    let quarantined = quarantined_files(dir.path());
    assert_eq!(quarantined.len(), 1, "expected exactly one quarantined file");
    assert_eq!(
        std::fs::read(&quarantined[0]).unwrap(),
        b"this is not a sqlite database at all",
        "the original bytes must be preserved for inspection"
    );
    assert!(path.exists(), "a usable database must exist at the original path");
}

#[timeout(30_000)]
#[test]
fn sqlite_reports_a_non_database_file_as_unusable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("garbage.db");
    std::fs::write(&path, b"not a sqlite file at all").unwrap();

    let conn = Connection::open(&path).unwrap();
    let error = conn
        .pragma_query_value(None, "quick_check", |row| row.get::<_, String>(0))
        .expect_err("reading a non-database must fail");

    assert!(
        is_unusable_database(&error),
        "a non-database must be classified as unusable: {error}"
    );
}

#[timeout(30_000)]
#[test]
fn lock_contention_is_never_mistaken_for_corruption() {
    // A second writer holding the file is contention, not corruption: treating
    // it as corruption would silently destroy the user's task history.
    assert!(is_lock_contention("database is locked"));
    assert!(is_lock_contention("SQLITE_BUSY: database is busy"));
    assert!(is_lock_contention("Database Is Locked"));
    assert!(!is_lock_contention("database disk image is malformed"));
    assert!(!is_lock_contention("file is not a database"));
}
