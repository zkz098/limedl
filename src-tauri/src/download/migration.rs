use std::{
    fs,
    path::Path,
};

use anyhow::{Context, Result};

use super::{
    database::Database,
    manifest::Manifest,
};

/// Migrate old per-task JSON manifest files into the SQLite database.
///
/// Scans `state_dir` for `*.json` files (skipping `settings.json`),
/// reads each one as a `Manifest`, inserts into the database,
/// then renames the source file to `*.json.migrated` to avoid re-migration.
///
/// Returns the number of manifests migrated.
pub(crate) fn migrate_json_manifests(db: &Database, state_dir: &Path) -> Result<usize> {
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
                    "[downloader] migration: skip unreadable {}: {error}",
                    path.display()
                );
                continue;
            }
        };

        let manifest = match serde_json::from_str::<Manifest>(&content) {
            Ok(m) => m,
            Err(error) => {
                eprintln!(
                    "[downloader] migration: skip invalid manifest {}: {error}",
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
                "[downloader] migration: failed to rename {} → {}: {error}",
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
