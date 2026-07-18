use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{database::Database, manifest::Manifest};

/// Migrate old per-task JSON manifest files into the SQLite database.
///
/// Scans `state_dir` for `*.json` files (skipping `settings.json`),
/// reads each one as a `Manifest`, inserts into the database,
/// then renames the source file to `*.json.migrated` to avoid re-migration.
///
/// Returns the number of manifests migrated.
pub fn migrate_json_manifests(db: &Database, state_dir: &Path) -> Result<usize> {
    let mut migrated = 0usize;

    for entry in fs::read_dir(state_dir)
        .with_context(|| format!("读取状态目录失败: {}", state_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        if path.file_name().and_then(|v| v.to_str()) == Some("settings.json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(error) => {
                eprintln!(
                    "[flareget] migration: skip unreadable {}: {error}",
                    path.display()
                );
                continue;
            }
        };

        let manifest = match serde_json::from_str::<Manifest>(&content) {
            Ok(m) => m,
            Err(error) => {
                eprintln!(
                    "[flareget] migration: skip invalid manifest {}: {error}",
                    path.display()
                );
                continue;
            }
        };

        db.insert_download(&manifest)
            .with_context(|| format!("迁移 manifest 到数据库失败: {}", manifest.id))?;

        let migrated_path = path.with_extension("json.migrated");
        if let Err(error) = fs::rename(&path, &migrated_path) {
            eprintln!(
                "[flareget] migration: failed to rename {} → {}: {error}",
                path.display(),
                migrated_path.display()
            );
        }

        migrated += 1;
    }

    if migrated > 0 {
        tracing::info!("已迁移 {migrated} 个 JSON manifest 到 SQLite 数据库");
    }

    Ok(migrated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ntest::timeout;
    use std::fs;
    use tempfile::tempdir;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn make_manifest_json(id: &str, url: &str, file_name: &str) -> String {
        format!(
            r#"{{
                "id": "{id}",
                "url": "{url}",
                "final_url": "{url}",
                "destination_dir": "/tmp",
                "file_name": "{file_name}",
                "destination_path": "/tmp/{file_name}",
                "temp_path": "/tmp/{file_name}.part",
                "downloaded_bytes": 0,
                "supports_ranges": false,
                "connection_count": 1,
                "thread_mode": "adaptive",
                "state": "completed",
                "checksum_mode": "none",
                "created_at_ms": 1000,
                "updated_at_ms": 1000,
                "chunks": []
            }}"#
        )
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_single_manifest() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        let json = make_manifest_json("test-1", "https://example.com/file.zip", "file.zip");
        let manifest_path = temp.path().join("test-1.json");
        fs::write(&manifest_path, &json)?;

        let count = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count, 1);

        // Source file should be renamed to .json.migrated
        assert!(!manifest_path.exists());
        assert!(manifest_path.with_extension("json.migrated").exists());

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_multiple_manifests() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        for id in 1..=3 {
            let json = make_manifest_json(
                &format!("multi-{id}"),
                "https://example.com/file.zip",
                &format!("file{id}.zip"),
            );
            fs::write(temp.path().join(format!("manifest-{id}.json")), &json)?;
        }

        let count = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count, 3);

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_no_json_files_returns_zero() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        let count = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count, 0);

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_skips_settings_json() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        fs::write(temp.path().join("settings.json"), r#"{"key": "value"}"#)?;

        let count = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count, 0);

        // settings.json should NOT be renamed
        assert!(temp.path().join("settings.json").exists());
        assert!(!temp.path().join("settings.json.migrated").exists());

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_skips_non_json_files() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        fs::write(temp.path().join("readme.txt"), b"hello")?;
        fs::write(temp.path().join("data.bin"), b"\x00\x01\x02")?;

        let count = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count, 0);

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_is_idempotent() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        let json = make_manifest_json("idempotent", "https://example.com/file.zip", "file.zip");
        fs::write(temp.path().join("manifest.json"), &json)?;

        // First migration
        let count1 = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count1, 1);

        // Original file was renamed, so second run should see 0 new files
        let count2 = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count2, 0);

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_skips_subdirectories() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        // Create a subdirectory with a json file (should be skipped)
        let sub = temp.path().join("subdir");
        fs::create_dir(&sub)?;
        fs::write(
            sub.join("nested.json"),
            &make_manifest_json("nested", "https://example.com/nested.zip", "nested.zip"),
        )?;

        let count = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count, 0);

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_handles_invalid_json_gracefully() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        fs::write(
            temp.path().join("corrupt.json"),
            r#"{invalid json content}"#,
        )?;

        // Should not panic, should skip corrupt file
        let count = migrate_json_manifests(&db, temp.path())?;
        assert_eq!(count, 0);

        // Corrupt file should NOT be renamed (skip + continue)
        assert!(temp.path().join("corrupt.json").exists());

        Ok(())
    }

    #[timeout(30_000)]
    #[test]
    fn migrate_mixed_valid_and_invalid_files() -> TestResult {
        let temp = tempdir()?;
        let db_path = temp.path().join("test.db");
        let db = Database::open(&db_path)?;

        fs::write(
            temp.path().join("good.json"),
            &make_manifest_json("good", "https://example.com/good.zip", "good.zip"),
        )?;
        fs::write(temp.path().join("bad.json"), r#"{invalid}"#)?;

        let count = migrate_json_manifests(&db, temp.path())?;
        // Only the good one should be migrated
        assert_eq!(count, 1);

        assert!(temp.path().join("good.json.migrated").exists());
        assert!(temp.path().join("bad.json").exists());

        Ok(())
    }
}
