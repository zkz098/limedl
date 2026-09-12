//! One-shot migration from the Tauri edition's data directory.
//!
//! The Tauri desktop app stores everything under
//! `<app_local_data_dir>/downloads` (identifier `com.zkz20.limedl`), while this
//! client uses `<local data dir>/limedl/downloads`. Without a migration a user
//! switching editions would face an empty task list, because the history lives
//! in `downloads.db` (plus the BT resume/DHT state in `torrents/` and the BT
//! default output folder `bt_files/`).
//!
//! Rules:
//! - only ever copies, never moves or deletes (the Tauri install keeps working),
//! - never overwrites an artifact that already exists in the native data dir,
//! - records per-artifact progress in a stamp file so a partially failed pass
//!   is retried on the next launch and a successful one is not repeated.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Stamp file written next to `settings.json` once migration ran.
const STAMP_FILE: &str = ".migrated-from-tauri.json";
/// SQLite database file name inside the state directory (see `limedl_core::context`).
const DB_FILE: &str = "downloads.db";
/// SQLite side-car files that must travel with the database.
const DB_SIDECARS: [&str; 2] = ["downloads.db-wal", "downloads.db-shm"];
/// Sub-directories of the state directory that hold user data.
const STATE_SUBDIRS: [&str; 2] = ["torrents", "bt_files"];

/// What the migration copied during this run.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub settings: bool,
    pub database: bool,
    pub torrents: bool,
    pub bt_files: bool,
    pub copied_files: usize,
    pub copied_bytes: u64,
    /// Source directory the data came from (for the log / toast text).
    pub source: PathBuf,
}

impl MigrationReport {
    /// True when anything at all was copied in this run.
    pub fn moved_anything(&self) -> bool {
        self.settings || self.database || self.torrents || self.bt_files
    }
}

/// Per-artifact completion flags persisted between runs.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationStamp {
    #[serde(default)]
    source: String,
    #[serde(default)]
    settings: bool,
    #[serde(default)]
    database: bool,
    #[serde(default)]
    torrents: bool,
    #[serde(default)]
    bt_files: bool,
}

/// Tauri's data directory (contains `settings.json` and `downloads/`).
///
/// Mirrors the path layout of `tauri::path().app_local_data_dir()` for the
/// identifier `com.zkz20.limedl` on each platform. `LIMEDL_TAURI_DATA_DIR`
/// overrides the lookup (non-standard installs, automated migration tests).
pub fn tauri_data_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("LIMEDL_TAURI_DATA_DIR") {
        return Some(PathBuf::from(dir));
    }
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("com.zkz20.limedl"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/com.zkz20.limedl"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|base| base.join("com.zkz20.limedl"))
    }
}

/// Migrate Tauri data into the native data directory when needed.
///
/// `base_dir` is the native data root (parent of `settings.json`), `state_dir`
/// is `<base_dir>/downloads`. Returns a report when something was copied in
/// this run, `None` when there was nothing to do.
pub fn migrate_tauri_data_if_needed(base_dir: &Path, state_dir: &Path) -> Option<MigrationReport> {
    let source = tauri_data_dir()?;
    if !source.is_dir() {
        return None;
    }

    let stamp_path = base_dir.join(STAMP_FILE);
    let mut stamp = read_stamp(&stamp_path);
    // A migration from a different source directory (e.g. the user moved their
    // Tauri profile) starts a fresh pass.
    if stamp.source != source.to_string_lossy() {
        stamp = MigrationStamp {
            source: source.to_string_lossy().into_owned(),
            ..MigrationStamp::default()
        };
    }

    let mut report = MigrationReport {
        source: source.clone(),
        ..MigrationReport::default()
    };

    // ── settings.json ──
    let native_settings = base_dir.join("settings.json");
    let tauri_settings = source.join("settings.json");
    if !stamp.settings && !native_settings.exists() && tauri_settings.is_file() {
        match copy_file(&tauri_settings, &native_settings) {
            Ok(bytes) => {
                if settings_json_parses(&native_settings) {
                    stamp.settings = true;
                    report.settings = true;
                    report.copied_files += 1;
                    report.copied_bytes += bytes;
                } else {
                    // A settings file the current engine cannot read would abort
                    // bootstrap, so quarantine it and start from defaults.
                    tracing::warn!(
                        "迁移的 settings.json 无法解析，已隔离并改以默认设置启动（原文件：{}）",
                        tauri_settings.display()
                    );
                    quarantine_file(&native_settings, "settings.json.rejected");
                    stamp.settings = true;
                }
            }
            Err(e) => tracing::warn!(
                "迁移 settings.json 失败 ({} -> {}): {e:#}",
                tauri_settings.display(),
                native_settings.display()
            ),
        }
    } else if !stamp.settings {
        // Either the native settings already exist (never overwrite) or the
        // Tauri profile has none: nothing left to do for this artifact.
        stamp.settings = true;
    }

    // ── downloads.db (+ WAL/SHM side-cars) ──
    let native_db = state_dir.join(DB_FILE);
    let tauri_db = source.join("downloads").join(DB_FILE);
    if !stamp.database {
        if native_db.exists() || !tauri_db.is_file() {
            // Native history already exists (never overwrite) or the Tauri
            // profile has none: nothing to migrate for this artifact.
            stamp.database = true;
        } else if !looks_like_sqlite(&tauri_db) {
            // The source is not a SQLite database (leftover/HTML rewrite): leave
            // it alone and never copy it, or the engine would refuse to boot.
            tracing::warn!(
                "跳过数据库迁移：{} 不是有效的 SQLite 文件",
                tauri_db.display()
            );
            stamp.database = true;
        } else {
            match copy_file(&tauri_db, &native_db) {
                Ok(bytes) => {
                    let mut ok = true;
                    for sidecar in DB_SIDECARS {
                        let from = tauri_db.with_file_name(sidecar);
                        if from.is_file() {
                            match copy_file(&from, &native_db.with_file_name(sidecar)) {
                                Ok(extra) => report.copied_bytes += extra,
                                Err(e) => {
                                    ok = false;
                                    tracing::warn!("迁移 {sidecar} 失败: {e:#}");
                                }
                            }
                        }
                    }
                    report.copied_files += 1;
                    report.copied_bytes += bytes;
                    if ok && database_opens(&native_db) {
                        stamp.database = true;
                        report.database = true;
                    } else {
                        // Torn/unreadable copy (e.g. the Tauri app was writing
                        // while we copied): quarantine it instead of letting
                        // bootstrap fail forever, and keep the side-cars with it.
                        quarantine_database(&native_db);
                        tracing::warn!(
                            "迁移的数据库无法打开，已隔离并改以空历史启动（原文件：{}）",
                            tauri_db.display()
                        );
                        stamp.database = true;
                    }
                }
                Err(e) => tracing::warn!(
                    "迁移 {} 失败 ({} -> {}): {e:#}",
                    DB_FILE,
                    tauri_db.display(),
                    native_db.display()
                ),
            }
        }
    }

    // ── torrents/ and bt_files/ trees ──
    for (index, name) in STATE_SUBDIRS.iter().enumerate() {
        let already_done = if index == 0 {
            stamp.torrents
        } else {
            stamp.bt_files
        };
        if already_done {
            continue;
        }
        let from = source.join("downloads").join(name);
        let to = state_dir.join(name);
        if !from.is_dir() {
            // Nothing to migrate (the Tauri install never used BT, or the folder
            // was already consumed) — mark it so we stop checking.
            if index == 0 {
                stamp.torrents = true;
            } else {
                stamp.bt_files = true;
            }
            continue;
        }
        if dir_has_entries(&to) {
            if index == 0 {
                stamp.torrents = true;
            } else {
                stamp.bt_files = true;
            }
            continue;
        }
        match copy_dir_recursive(&from, &to) {
            Ok((files, bytes)) => {
                report.copied_files += files;
                report.copied_bytes += bytes;
                if index == 0 {
                    stamp.torrents = true;
                    report.torrents = true;
                } else {
                    stamp.bt_files = true;
                    report.bt_files = true;
                }
            }
            Err(e) => tracing::warn!(
                "迁移 {name} 目录失败 ({} -> {}): {e:#}",
                from.display(),
                to.display()
            ),
        }
    }

    let all_handled = stamp.settings && stamp.database && stamp.torrents && stamp.bt_files;
    if all_handled || report.moved_anything() {
        write_stamp(&stamp_path, &stamp);
    }

    if report.moved_anything() {
        tracing::info!(
            "已从 Tauri 数据目录迁移 {} 个文件 ({}) 到 {}",
            report.copied_files,
            report.source.display(),
            base_dir.display()
        );
        Some(report)
    } else {
        None
    }
}

/// Copy a single file, creating the destination directory when needed.
fn copy_file(from: &Path, to: &Path) -> std::io::Result<u64> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to)
}

/// SQLite files start with this 16-byte magic header.
const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// Cheap header check — rejects text files, HTML error pages and empty stubs
/// without opening SQLite.
fn looks_like_sqlite(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 16];
    file.read_exact(&mut header).is_ok() && &header == SQLITE_MAGIC
}

/// True when the migrated settings file can be deserialized by the current
/// engine (an unreadable file makes `bootstrap` fail, which would leave the app
/// unable to start).
fn settings_json_parses(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<limedl_core::types::AppSettings>(&raw).ok())
        .is_some()
}

/// Full check: the copied database must be openable by the engine (this is the
/// same open `bootstrap` performs, so a failure here would abort startup).
fn database_opens(path: &Path) -> bool {
    limedl_core::database::Database::open(path).is_ok()
}

/// Move a file out of the way (timestamped suffix) so the engine can recreate a
/// default instead of failing to boot. Falls back to deletion.
fn quarantine_file(path: &Path, suffix: &str) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let rejected = path.with_extension(format!("{suffix}-{stamp}"));
    if let Err(e) = std::fs::rename(path, &rejected) {
        tracing::warn!("隔离 {} 失败 ({e:#})，改为删除", path.display());
        let _ = std::fs::remove_file(path);
    } else {
        tracing::info!("已将 {} 重命名为 {}", path.display(), rejected.display());
    }
}

/// Move an unusable copied database (and its side-cars) out of the way so the
/// engine can create a fresh one instead of failing to boot.
fn quarantine_database(db_path: &Path) {
    for sidecar in DB_SIDECARS {
        let path = db_path.with_file_name(sidecar);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
    quarantine_file(db_path, "db.rejected");
}

/// Recursively copy `from` into `to`, skipping files that already exist.
/// Returns `(copied files, copied bytes)`.
fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<(usize, u64)> {
    std::fs::create_dir_all(to)?;
    let mut files = 0usize;
    let mut bytes = 0u64;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = to.join(entry.file_name());
        if file_type.is_dir() {
            let (child_files, child_bytes) = copy_dir_recursive(&entry.path(), &target)?;
            files += child_files;
            bytes += child_bytes;
        } else if file_type.is_file() {
            if target.exists() {
                continue;
            }
            bytes += copy_file(&entry.path(), &target)?;
            files += 1;
        }
    }
    Ok((files, bytes))
}

/// True when `dir` exists and contains at least one entry.
fn dir_has_entries(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

fn read_stamp(path: &Path) -> MigrationStamp {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_stamp(path: &Path, stamp: &MigrationStamp) {
    match serde_json::to_string_pretty(stamp) {
        Ok(raw) => {
            if let Err(e) = std::fs::write(path, raw) {
                tracing::debug!("写入迁移标记失败 ({}): {e:#}", path.display());
            }
        }
        Err(e) => tracing::debug!("序列化迁移标记失败: {e:#}"),
    }
}

#[cfg(test)]
mod tests {
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
    fn migrate_with_source(
        base: &Path,
        state_dir: &Path,
        source: &Path,
    ) -> Option<MigrationReport> {
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
        assert!(report.is_none(), "a rejected settings file is not a migration");
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
}
