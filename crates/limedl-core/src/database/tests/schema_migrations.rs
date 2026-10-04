//! Schema helpers and migration compatibility.

use super::*;

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
    assert!(
        columns.contains(&"expected_checksum".to_string()),
        "expected_checksum column should exist"
    );
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
            10,
            "expected user_version = 10 after migration"
        );
    }

    let mut m = new_test_manifest("compat-v1-b", "https://example.com/b", "b.bin");
    m.chunk_size = 4194304;
    m.mirror_urls = vec!["https://mirror.example.com/b".into()];
    m.current_mirror_index = 0;
    db.insert_download(&m).unwrap();
    let loaded = db
        .get_download("compat-v1-b")
        .unwrap()
        .expect("should exist");
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
            10,
            "expected user_version = 10 after migration"
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
    let loaded = db
        .get_download("compat-v0-c")
        .unwrap()
        .expect("should exist");
    assert_eq!(loaded.chunk_size, 2097152);
    assert_eq!(
        loaded.mirror_url.as_deref(),
        Some("https://mirror.example.com/c")
    );
    assert_eq!(loaded.mirror_urls.len(), 2);
    assert_eq!(loaded.current_mirror_index, 1);
    assert_eq!(db.count_downloads().unwrap(), 1);
}

#[timeout(30_000)]
#[test]
fn add_column_if_missing_is_idempotent() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(CREATE_TABLES_SQL).unwrap();

    add_column_if_missing(&conn, "downloads", "priority", "INTEGER NOT NULL DEFAULT 1").unwrap();
    // A second call must be a no-op instead of "duplicate column name".
    add_column_if_missing(&conn, "downloads", "priority", "INTEGER NOT NULL DEFAULT 1").unwrap();

    assert!(table_has_column(&conn, "downloads", "priority").unwrap());
}

/// A database whose schema is ahead of its `user_version` — the state a crash in
/// the middle of a migration body used to leave behind — must still open, because
/// every migration body is idempotent and each migration commits atomically.
#[timeout(30_000)]
#[test]
fn partially_applied_migration_is_recovered_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    {
        let conn = Connection::open(&path).unwrap();
        create_v1_schema(&conn);
        apply_v2(&conn);
        apply_v3(&conn);
        // Migration 6's DDL is present while `user_version` still says 3, which is
        // exactly what a half-committed migration used to look like.
        conn.execute(
            "ALTER TABLE downloads ADD COLUMN priority INTEGER NOT NULL DEFAULT 1",
            [],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 3).unwrap();
    }

    let db = Database::open(&path).expect("a half-migrated database must still open");
    {
        let conn = db.lock_write();
        assert_eq!(read_user_version(&conn), 10);
    }

    let mut m = new_test_manifest("recovered", "https://example.com/r", "r.bin");
    m.priority = Priority::High;
    db.insert_download(&m).unwrap();
    let loaded = db.get_download("recovered").unwrap().expect("should exist");
    assert_eq!(loaded.priority, Priority::High);
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
