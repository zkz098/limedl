use super::*;

/// Env var read by [`tauri_data_dir`] to point the migration at a fixture.
const TEST_SOURCE_ENV: &str = "LIMEDL_TAURI_DATA_DIR";

fn make_tauri_layout(root: &Path) {
    let downloads = root.join("downloads");
    std::fs::create_dir_all(downloads.join("torrents/resume")).unwrap();
    std::fs::create_dir_all(downloads.join("bt_files")).unwrap();
    std::fs::write(root.join("settings.json"), "{\"autostart\":true}").unwrap();
    // A real SQLite database, because the migration validates the copy by
    // opening it exactly like `bootstrap` does.
    let db_path = downloads.join(DB_FILE);
    limedl_core::database::Database::open(&db_path)
        .expect("create fixture database")
        .shutdown()
        .expect("flush fixture database");
    std::fs::write(downloads.join("downloads.db-wal"), b"wal").unwrap();
    std::fs::write(downloads.join("torrents/resume/dht_state.json"), b"{}").unwrap();
    std::fs::write(downloads.join("bt_files/payload.bin"), b"payload").unwrap();
}

fn layout(base: &Path) -> (PathBuf, PathBuf) {
    let state_dir = base.join("downloads");
    std::fs::create_dir_all(&state_dir).unwrap();
    (base.to_path_buf(), state_dir)
}

/// Run the real entry point against an explicit source directory.
///
/// The three tests below are the only users of the override, and they run
/// inside a single `#[test]` each, so mutating the process environment is
/// safe here.
fn migrate_with_source(base: &Path, state_dir: &Path, source: &Path) -> Option<MigrationReport> {
    unsafe {
        std::env::set_var(TEST_SOURCE_ENV, source);
    }
    let report = migrate_tauri_data_if_needed(base, state_dir);
    unsafe {
        std::env::remove_var(TEST_SOURCE_ENV);
    }
    report
}

#[test]
fn first_run_copies_everything_once() {
    let root = tempfile::tempdir().unwrap();
    let tauri = root.path().join("tauri");
    make_tauri_layout(&tauri);
    let base = root.path().join("native");
    let (base, state_dir) = layout(&base);

    let report = migrate_with_source(&base, &state_dir, &tauri).expect("report");
    assert!(report.settings && report.database && report.torrents && report.bt_files);
    assert!(base.join("settings.json").is_file());
    assert!(state_dir.join("downloads.db").is_file());
    // The copied database must be usable by the engine; validating it also
    // checkpoints and removes the copied WAL side-car, which is expected.
    assert!(database_opens(&state_dir.join(DB_FILE)));
    assert!(state_dir.join("torrents/resume/dht_state.json").is_file());
    assert!(state_dir.join("bt_files/payload.bin").is_file());

    // Second run is a no-op (stamp written, destinations exist).
    assert!(migrate_with_source(&base, &state_dir, &tauri).is_none());
}

#[test]
fn existing_native_data_is_never_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let tauri = root.path().join("tauri");
    make_tauri_layout(&tauri);
    let base = root.path().join("native");
    let (base, state_dir) = layout(&base);

    // Simulate the upgrade path: the old settings-only migration already ran.
    std::fs::write(base.join("settings.json"), "{\"native\":true}").unwrap();

    let report = migrate_with_source(&base, &state_dir, &tauri).expect("report");
    assert!(!report.settings, "settings must not be re-copied");
    assert!(report.database);
    assert_eq!(
        std::fs::read_to_string(base.join("settings.json")).unwrap(),
        "{\"native\":true}"
    );
    assert!(state_dir.join("downloads.db").is_file());
}

#[test]
fn invalid_source_database_is_never_copied() {
    let root = tempfile::tempdir().unwrap();
    let tauri = root.path().join("tauri");
    let downloads = tauri.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    // A text file masquerading as the database (e.g. an HTML error page).
    std::fs::write(downloads.join(DB_FILE), b"not a database").unwrap();
    let base = root.path().join("native");
    let (base, state_dir) = layout(&base);

    let report = migrate_with_source(&base, &state_dir, &tauri);
    assert!(report.is_none(), "nothing valid to migrate");
    assert!(
        !state_dir.join(DB_FILE).exists(),
        "an invalid database must not land in the native state dir"
    );
    // The stamp still marks the database as handled (retrying would copy the
    // same broken file again on every launch).
    let stamp = read_stamp(&base.join(STAMP_FILE));
    assert!(stamp.database);
}

#[test]
fn unparsable_settings_are_quarantined() {
    let root = tempfile::tempdir().unwrap();
    let tauri = root.path().join("tauri");
    std::fs::create_dir_all(&tauri).unwrap();
    // Valid JSON but missing mandatory fields → `AppSettings` rejects it.
    std::fs::write(tauri.join("settings.json"), "{\"download\":{}}").unwrap();
    let base = root.path().join("native");
    let (base, state_dir) = layout(&base);

    let report = migrate_with_source(&base, &state_dir, &tauri);
    assert!(
        report.is_none(),
        "a rejected settings file is not a migration"
    );
    assert!(
        !base.join("settings.json").exists(),
        "unreadable settings must not be left in place"
    );
    assert!(read_stamp(&base.join(STAMP_FILE)).settings);
    // A good settings file migrates normally (sanity check on the check).
    let tauri2 = root.path().join("tauri2");
    std::fs::create_dir_all(&tauri2).unwrap();
    let valid = serde_json::to_string(&limedl_core::types::AppSettings::default()).unwrap();
    std::fs::write(tauri2.join("settings.json"), valid).unwrap();
    let (base2, state2) = layout(&root.path().join("native2"));
    let report = migrate_with_source(&base2, &state2, &tauri2).expect("report");
    assert!(report.settings);
    assert!(settings_json_parses(&base2.join("settings.json")));
}

#[test]
fn torn_database_copy_is_quarantined() {
    let root = tempfile::tempdir().unwrap();
    let damaged = root.path().join("downloads.db");
    // Valid header with a truncated body — `Database::open` must reject it.
    let mut bytes = SQLITE_MAGIC.to_vec();
    bytes.extend_from_slice(&[0u8; 64]);
    std::fs::write(&damaged, &bytes).unwrap();
    assert!(looks_like_sqlite(&damaged));
    assert!(!database_opens(&damaged));

    quarantine_database(&damaged);
    assert!(!damaged.exists(), "the damaged file must be moved aside");
    let rejected = std::fs::read_dir(root.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().contains("rejected"));
    assert!(rejected, "the damaged file should be kept as .rejected-*");
}

#[test]
fn recursive_copy_skips_existing_files() {
    let root = tempfile::tempdir().unwrap();
    let from = root.path().join("from");
    let to = root.path().join("to");
    std::fs::create_dir_all(from.join("sub")).unwrap();
    std::fs::write(from.join("keep.txt"), b"new").unwrap();
    std::fs::write(from.join("sub/nested.txt"), b"nested").unwrap();
    std::fs::create_dir_all(&to).unwrap();
    std::fs::write(to.join("keep.txt"), b"old").unwrap();

    let (files, bytes) = copy_dir_recursive(&from, &to).unwrap();
    assert_eq!(files, 1, "existing file must be skipped");
    assert_eq!(bytes, 6);
    assert_eq!(std::fs::read_to_string(to.join("keep.txt")).unwrap(), "old");
    assert!(to.join("sub/nested.txt").is_file());
}
