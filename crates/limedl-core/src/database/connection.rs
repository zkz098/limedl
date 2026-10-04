use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;

use super::schema::{MIGRATIONS, table_has_column};
#[cfg(test)]
use crate::error::DownloadError;

/// Databases above this size skip the startup integrity probe: `quick_check`
/// reads every page, so a very large history file would add seconds of startup
/// I/O for a condition SQLite already reports per-statement when it matters.
const MAX_INTEGRITY_CHECK_BYTES: u64 = 128 * 1024 * 1024;

pub struct Database {
    pub(crate) write_conn: Arc<Mutex<Connection>>,
    pub(crate) read_conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open (or create) the SQLite database at `path`.
    ///
    /// Enables WAL mode, foreign keys, and performance PRAGMAs,
    /// then runs schema migrations.
    pub fn open(path: &Path) -> Result<Self> {
        // Pre-flight: a corrupt file is moved aside here, before any statement
        // touches it. Otherwise the first real query fails with an opaque
        // "database disk image is malformed" and the app is unusable with no way
        // forward for the user.
        quarantine_if_corrupt(path)?;

        let mut write_conn = Connection::open(path)
            .with_context(|| format!("failed to open database at {}", path.display()))?;

        // ── PRAGMA configuration ─────────────────────────────────
        write_conn
            .execute_batch("PRAGMA journal_mode = WAL;")
            .context("failed to enable WAL mode")?;
        write_conn
            .execute_batch("PRAGMA wal_autocheckpoint = 4096;")
            .context("failed to set WAL auto-checkpoint")?;
        write_conn
            .execute_batch("PRAGMA foreign_keys = ON;")
            .context("failed to enable foreign keys")?;
        write_conn
            .execute_batch("PRAGMA busy_timeout = 5000;")
            .context("failed to set busy timeout")?;
        // NORMAL is safe with WAL mode — the WAL itself provides crash safety.
        // FULL would force an fsync on every checkpoint, doubling I/O overhead
        // when combined with the buffer pool's per-batch sync_data.
        write_conn
            .execute_batch("PRAGMA synchronous = NORMAL;")
            .context("failed to set synchronous mode")?;
        // 32 MB page cache (negative = kibibytes).  This reduces disk I/O during
        // migration and steady-state download progress persistence by keeping
        // more of the working dataset in memory.
        write_conn
            .execute_batch("PRAGMA cache_size = -32000;")
            .context("failed to set cache size")?;

        // ── Schema migrations ────────────────────────────────────
        let mut current_version: u32 = write_conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .context("failed to read schema version")?;

        // Compatibility: detect columns already backfilled by the old code that
        // added columns without setting `user_version`, so existing databases
        // don't fail migrations.
        //
        // The detected version only ever moves forward: a database whose
        // `user_version` is already correct must not be rewound by the probe.
        if current_version < 4 {
            let mut detected = current_version;
            if table_has_column(&write_conn, "downloads", "chunk_size")? {
                detected = detected.max(2);
            }
            if table_has_column(&write_conn, "downloads", "mirror_urls")? {
                detected = detected.max(3);
            }
            if detected > current_version {
                tracing::info!(
                    "legacy database without user_version detected; assuming schema v{detected}"
                );
                write_conn.pragma_update(None, "user_version", detected)?;
                current_version = detected;
            }
        }

        let mut migrations_ran = false;
        for migration in MIGRATIONS.iter().filter(|m| m.version > current_version) {
            tracing::info!(
                "Running migration v{}: {}",
                migration.version,
                migration.name
            );
            // The whole migration — every DDL statement *and* the `user_version`
            // bump — commits as one transaction. Without it a failure midway
            // (disk full, I/O error, kill) leaves the schema half-migrated while
            // `user_version` still names the old version; the next start would
            // replay the same migration and hit "duplicate column name", which
            // permanently bricks the database. Idempotent migration bodies
            // (`add_column_if_missing`) make an already-applied body a no-op as
            // well, which covers databases written by the pre-transaction code.
            let tx = write_conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .with_context(|| {
                    format!(
                        "failed to begin transaction for migration v{} ({})",
                        migration.version, migration.name
                    )
                })?;
            (migration.up)(&tx).with_context(|| {
                format!(
                    "migration v{} ({}) failed",
                    migration.version, migration.name
                )
            })?;
            tx.pragma_update(None, "user_version", migration.version)
                .with_context(|| {
                    format!(
                        "failed to update schema version to {}",
                        migration.version
                    )
                })?;
            tx.commit().with_context(|| {
                format!(
                    "failed to commit migration v{} ({})",
                    migration.version, migration.name
                )
            })?;
            migrations_ran = true;
        }

        // ── Update query planner statistics after schema changes ─────
        if migrations_ran {
            write_conn
                .execute_batch("ANALYZE;")
                .context("failed to run ANALYZE")?;
        }

        // ── Read connection (WAL-enabled, read-only) ────────────
        let read_conn = Connection::open(path)
            .with_context(|| format!("failed to open read database at {}", path.display()))?;
        read_conn
            .execute_batch("PRAGMA query_only = 1;")
            .context("failed to set query_only on read connection")?;
        read_conn
            .execute_batch("PRAGMA busy_timeout = 5000;")
            .context("failed to set busy timeout on read connection")?;

        write_conn.set_prepared_statement_cache_capacity(64);
        read_conn.set_prepared_statement_cache_capacity(64);

        Ok(Self {
            write_conn: Arc::new(Mutex::new(write_conn)),
            read_conn: Arc::new(Mutex::new(read_conn)),
        })
    }

    /// Create an in-memory SQLite database for testing.
    ///
    /// Enables foreign keys and busy timeout, then runs all migrations.
    /// Does NOT enable WAL mode (unnecessary for in-memory single-connection tests
    /// and can cause "database is locked" errors).
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory database")?;
        conn.set_prepared_statement_cache_capacity(64);
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .context("failed to enable foreign keys")?;
        conn.execute_batch("PRAGMA busy_timeout = 5000;")
            .context("failed to set busy timeout")?;
        // Run all migrations (no WAL for in-memory — causes "database is locked" errors)
        for migration in MIGRATIONS {
            (migration.up)(&conn)
                .with_context(|| format!("test migration v{} failed", migration.version))?;
        }
        let current_version = MIGRATIONS
            .last()
            .map(|m| m.version)
            .ok_or_else(|| DownloadError::DatabaseInit("no migrations defined".into()))?;
        conn.pragma_update(None, "user_version", current_version)
            .context("failed to set schema version")?;
        let conn = Arc::new(Mutex::new(conn));
        Ok(Self {
            write_conn: conn.clone(),
            read_conn: conn,
        })
    }

    pub(crate) fn lock_write(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.write_conn.lock()
    }

    pub(crate) fn lock_read(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.read_conn.lock()
    }

    /// Perform a WAL checkpoint to truncate the WAL file on clean shutdown.
    /// Should be called after all downloads have stopped and before the process exits.
    pub fn shutdown(&self) -> Result<()> {
        let conn = self.lock_write();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .context("failed to checkpoint WAL on shutdown")?;
        Ok(())
    }

    /// Run incremental vacuum if the freelist page count exceeds `threshold`.
    pub(crate) fn vacuum_if_needed(&self, conn: &Connection, threshold: u32) -> Result<()> {
        let freelist: u32 = conn
            .pragma_query_value(None, "freelist_count", |row| row.get(0))
            .context("failed to query freelist_count")?;

        if freelist > threshold {
            tracing::debug!(
                "Freelist has {freelist} pages (> {threshold}), running incremental vacuum"
            );
            conn.execute_batch(&format!("PRAGMA incremental_vacuum({freelist});"))
                .context("failed to run incremental vacuum")?;
        }
        Ok(())
    }
}

/// Append `suffix` to a path's file name without touching its extension.
///
/// SQLite's sidecars are named by appending `-wal` / `-shm` to the *whole* path,
/// not by replacing the extension, so `Path::with_extension` cannot be used.
fn with_suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    std::path::PathBuf::from(name)
}

/// Where a corrupt database is preserved.
fn quarantine_path(path: &Path) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    with_suffix(path, &format!(".corrupt-{stamp}"))
}

/// Move a database and its WAL sidecars out of the way.
///
/// The sidecars have to travel with it: leaving a stale `-wal` next to the freshly
/// created database would replay the old (possibly corrupt) frames into it.
fn quarantine_database(path: &Path) -> Result<std::path::PathBuf> {
    let destination = quarantine_path(path);
    std::fs::rename(path, &destination).with_context(|| {
        format!(
            "failed to move the corrupt database {} to {}",
            path.display(),
            destination.display()
        )
    })?;
    for suffix in ["-wal", "-shm"] {
        let from = with_suffix(path, suffix);
        if from.exists() {
            let _ = std::fs::rename(&from, with_suffix(&destination, suffix));
        }
    }
    Ok(destination)
}

/// `true` when an error message means "someone else is using it" rather than "it
/// is broken". Contention must never be treated as corruption.
pub(crate) fn is_lock_contention(message: &str) -> bool {
    let lowered = message.to_ascii_lowercase();
    lowered.contains("locked") || lowered.contains("busy")
}

/// `true` when SQLite itself says the bytes are not a database, or that its
/// pages are damaged — as opposed to a transient or environmental failure.
pub(crate) fn is_unusable_database(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(code, _)
            if matches!(
                code.code,
                rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt
            )
    )
}

/// Run `PRAGMA quick_check`, returning the first problem SQLite reports.
///
/// `None` also covers "no verdict": a locked database (another writer, or a lock
/// a crash left behind) must not be read as corruption, and an unreadable file is
/// reported by the normal open path anyway.
fn quick_check(conn: &Connection) -> Option<String> {
    match conn.pragma_query_value(None, "quick_check", |row| row.get::<_, String>(0)) {
        Ok(result) if result.eq_ignore_ascii_case("ok") => None,
        Ok(result) => Some(result),
        Err(error) if is_unusable_database(&error) => Some(error.to_string()),
        Err(error) => {
            tracing::warn!("skipping the database integrity check: {error}");
            None
        }
    }
}

/// Quarantine `path` when it is not a usable SQLite database.
///
/// Called before anything else opens it. A locked or busy database is *not*
/// corruption (another process or an interrupted shutdown) and is left alone —
/// only a file SQLite cannot read, or one that fails `quick_check`, is moved
/// aside so a fresh database can be created and the app stays usable. The
/// download files themselves are never touched.
fn quarantine_if_corrupt(path: &Path) -> Result<()> {
    let size = match std::fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        // Missing (first run) or unreadable: let the normal open path report it.
        Err(_) => return Ok(()),
    };
    if size == 0 {
        return Ok(());
    }
    if size > MAX_INTEGRITY_CHECK_BYTES {
        tracing::info!(
            "skipping the database integrity check: {} is {} bytes (limit {})",
            path.display(),
            size,
            MAX_INTEGRITY_CHECK_BYTES
        );
        return Ok(());
    }

    let probe = match Connection::open(path) {
        Ok(probe) => probe,
        Err(error) => {
            let message = error.to_string();
            if is_lock_contention(&message) {
                // Another instance (or a crash left the lock behind): a real
                // error, but not something to quarantine.
                return Err(error).with_context(|| {
                    format!("failed to open database at {}", path.display())
                });
            }
            tracing::error!(
                "database at {} cannot be opened ({message}); moving it aside",
                path.display()
            );
            let destination = quarantine_database(path)?;
            tracing::error!(
                "the unreadable database was preserved at {}",
                destination.display()
            );
            return Ok(());
        }
    };
    // Match the writer's patience so a transient lock is waited out rather than
    // reported as an unreadable file.
    let _ = probe.execute_batch("PRAGMA busy_timeout = 5000;");

    let problem = quick_check(&probe);
    drop(probe);

    let Some(problem) = problem else {
        return Ok(());
    };

    tracing::error!(
        "database at {} failed its integrity check ({problem}); moving it aside",
        path.display()
    );
    let destination = quarantine_database(path)?;
    tracing::error!(
        "the corrupt database was preserved at {} — download task history was reset, \
         files on disk are untouched",
        destination.display()
    );
    Ok(())
}
