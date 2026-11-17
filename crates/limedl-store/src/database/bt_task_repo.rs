//! Persisted index of BitTorrent tasks in an isolated `redb` cache (`bt_tasks.redb`).
//!
//! The irontide session keeps the authoritative torrent state in its on-disk
//! resume files, but those are only readable by a running engine. Lightweight
//! BT mode keeps the engine unloaded while idle, so the desktop UI needs a
//! cheap place to read BT rows from without paying for session startup.
//!
//! This store is a **cache**, not the source of truth: the engine refreshes it
//! from `RqbitBtBackend::list()` whenever it is running, and the rows are
//! served (with volatile transfer metrics zeroed) only while the engine is
//! down.

use std::cmp::Reverse;

use anyhow::{Context, Result};
use redb::{ReadableTable, ReadableTableMetadata};

use super::connection::Database;
use super::schema::BT_TASKS;
use crate::types::DownloadSummary;

impl Database {
    /// Replace the whole BT task index with the engine's current view.
    pub fn replace_bt_tasks(&self, summaries: &[DownloadSummary]) -> Result<()> {
        let tx = self.begin_bt_write()?;
        {
            let mut table = tx.open_table(BT_TASKS)?;
            let existing_keys: Vec<String> = table
                .iter()?
                .filter_map(|res| res.ok().map(|(k, _)| k.value().to_owned()))
                .collect();
            for key in existing_keys {
                let _ = table.remove(key.as_str())?;
            }

            for summary in summaries {
                let json =
                    serde_json::to_vec(summary).context("failed to serialize BT task summary")?;
                table
                    .insert(summary.id.as_str(), json.as_slice())
                    .context("failed to insert BT task index row")?;
            }
        }
        tx.commit()
            .context("failed to commit BT task index transaction")?;
        Ok(())
    }

    /// All indexed BT tasks, newest first.
    ///
    /// Entries whose JSON fails to deserialize are skipped with a warning rather
    /// than failing the whole list: a stale cache entry must never hide the
    /// rest of the user's tasks.
    pub fn list_bt_tasks(&self) -> Result<Vec<DownloadSummary>> {
        let tx = self.begin_bt_read()?;
        let table = tx.open_table(BT_TASKS)?;
        let mut tasks = Vec::with_capacity(table.len()? as usize);

        for entry in table.iter()? {
            let (k, v) = entry?;
            let id = k.value();
            match serde_json::from_slice::<DownloadSummary>(v.value()) {
                Ok(summary) => tasks.push(summary),
                Err(e) => tracing::warn!("dropping unreadable BT task index row {id}: {e}"),
            }
        }

        tasks.sort_by_key(|s| Reverse(s.created_at_ms));
        Ok(tasks)
    }

    /// Remove a single BT task from the index (cancel/remove/purge).
    pub fn delete_bt_task(&self, id: &str) -> Result<()> {
        let tx = self.begin_bt_write()?;
        {
            let mut table = tx.open_table(BT_TASKS)?;
            let _ = table.remove(id)?;
        }
        tx.commit().context("failed to commit BT task delete")?;
        Ok(())
    }

    /// Async wrapper for [`Self::replace_bt_tasks`] that runs on Tokio's blocking pool.
    pub async fn replace_bt_tasks_async(
        self: &std::sync::Arc<Self>,
        summaries: Vec<DownloadSummary>,
    ) -> Result<()> {
        self.run_blocking(move |db| db.replace_bt_tasks(&summaries))
            .await
    }

    /// Async wrapper for [`Self::list_bt_tasks`] that runs on Tokio's blocking pool.
    pub async fn list_bt_tasks_async(self: &std::sync::Arc<Self>) -> Result<Vec<DownloadSummary>> {
        self.run_blocking(Database::list_bt_tasks).await
    }

    /// Async wrapper for [`Self::delete_bt_task`] that runs on Tokio's blocking pool.
    pub async fn delete_bt_task_async(self: &std::sync::Arc<Self>, id: String) -> Result<()> {
        self.run_blocking(move |db| db.delete_bt_task(&id)).await
    }

    /// Drop the whole index (used when the engine's list is authoritative and
    /// empty, and by tests).
    #[cfg(any(test, feature = "test-utils"))]
    pub fn clear_bt_tasks(&self) -> Result<()> {
        self.replace_bt_tasks(&[])
    }
}
