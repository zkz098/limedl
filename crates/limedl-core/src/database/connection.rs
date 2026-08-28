use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;

use super::schema::{table_has_column, MIGRATIONS};
#[cfg(test)]
use crate::error::DownloadError;

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
        let write_conn = Connection::open(path)
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

        // Compatibility: detect columns already backfilled by the old let _ = code
        // (which never set user_version), so existing databases don't fail migrations.
        if current_version < 4 {
            if table_has_column(&write_conn, "downloads", "chunk_size")? {
                write_conn.pragma_update(None, "user_version", 2)?;
                current_version = 2;
            }
            if table_has_column(&write_conn, "downloads", "mirror_urls")? {
                write_conn.pragma_update(None, "user_version", 3)?;
                current_version = 3;
            }
        }

        let mut migrations_ran = false;
        for migration in MIGRATIONS.iter().filter(|m| m.version > current_version) {
            tracing::info!(
                "Running migration v{}: {}",
                migration.version,
                migration.name
            );
            (migration.up)(&write_conn).with_context(|| {
                format!(
                    "migration v{} ({}) failed",
                    migration.version, migration.name
                )
            })?;
            write_conn
                .pragma_update(None, "user_version", migration.version)
                .with_context(|| {
                    format!("failed to update schema version to {}", migration.version)
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
