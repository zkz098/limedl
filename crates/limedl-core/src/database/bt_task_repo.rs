//! Persisted index of BitTorrent tasks.
//!
//! The irontide session keeps the authoritative torrent state in its on-disk
//! resume files, but those are only readable by a running engine. Lightweight
//! BT mode keeps the engine unloaded while idle, so the desktop UI needs a
//! cheap place to read BT rows from without paying for session startup.
//!
//! This table is a **cache**, not the source of truth: the engine refreshes it
//! from `IrontideBtBackend::list()` whenever it is running, and the rows are
//! served (with volatile transfer metrics zeroed) only while the engine is
//! down. Store the serialized `DownloadSummary` instead of a wide typed schema
//! so a new summary field never requires a migration.

use anyhow::{Context, Result};
use rusqlite::params;

use super::connection::Database;
use crate::types::DownloadSummary;

impl Database {
    /// Replace the whole BT task index with the engine's current view.
    ///
    /// Runs in a single transaction so a crash can never leave the index
    /// half-updated. Called from the engine's periodic index sync, which is
    /// the only writer, so no row-level conflict resolution is needed.
    pub fn replace_bt_tasks(&self, summaries: &[DownloadSummary]) -> Result<()> {
        let conn = self.lock_write();
        conn.execute_batch("BEGIN IMMEDIATE")
            .context("failed to begin BT task index transaction")?;

        let result = (|| -> Result<()> {
            conn.execute("DELETE FROM bt_tasks", [])
                .context("failed to clear BT task index")?;

            let mut stmt = conn
                .prepare_cached(
                    "INSERT INTO bt_tasks (id, summary_json, created_at_ms) VALUES (?1, ?2, ?3)",
                )
                .context("failed to prepare BT task insert")?;
            for summary in summaries {
                let json = serde_json::to_string(summary)
                    .context("failed to serialize BT task summary")?;
                stmt.execute(params![summary.id, json, summary.created_at_ms as i64])
                    .context("failed to insert BT task index row")?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")
                    .context("failed to commit BT task index transaction")?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// All indexed BT tasks, newest first.
    ///
    /// Rows whose JSON fails to deserialize are skipped with a warning rather
    /// than failing the whole list: a stale cache entry must never hide the
    /// rest of the user's tasks.
    pub fn list_bt_tasks(&self) -> Result<Vec<DownloadSummary>> {
        let conn = self.lock_read();
        let mut stmt = conn
            .prepare_cached("SELECT id, summary_json FROM bt_tasks ORDER BY created_at_ms DESC")
            .context("failed to prepare BT task index query")?;

        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .context("failed to map BT task index rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect BT task index rows")?;

        let mut tasks = Vec::with_capacity(rows.len());
        for (id, json) in rows {
            match serde_json::from_str::<DownloadSummary>(&json) {
                Ok(summary) => tasks.push(summary),
                Err(e) => tracing::warn!("dropping unreadable BT task index row {id}: {e}"),
            }
        }
        Ok(tasks)
    }

    /// Remove a single BT task from the index (cancel/remove/purge).
    pub fn delete_bt_task(&self, id: &str) -> Result<()> {
        let conn = self.lock_write();
        conn.execute("DELETE FROM bt_tasks WHERE id = ?1", params![id])
            .context("failed to delete BT task index row")?;
        Ok(())
    }

    /// Drop the whole index (used when the engine's list is authoritative and
    /// empty, and by tests).
    #[cfg(test)]
    pub fn clear_bt_tasks(&self) -> Result<()> {
        let conn = self.lock_write();
        conn.execute("DELETE FROM bt_tasks", [])
            .context("failed to clear BT task index")?;
        Ok(())
    }
}
