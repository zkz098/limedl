use anyhow::{Context, Result};
use rusqlite::{params, types::Value, Connection};

use super::connection::Database;
use crate::manifest::ChunkManifest;

type RusqliteResult<T> = std::result::Result<T, rusqlite::Error>;

fn bool_to_i64(b: bool) -> i64 {
    if b { 1 } else { 0 }
}

fn i64_to_bool(v: i64) -> bool {
    v != 0
}

/// Single progress update entry for batch persist of multiple downloads.
#[derive(Clone)]
pub struct ProgressBatchEntry {
    pub id: String,
    pub downloaded_bytes: u64,
    pub dirty_chunks: Vec<ChunkManifest>,
    pub state: String,
    pub updated_at_ms: u64,
}

pub(crate) fn chunk_to_params(download_id: &str, chunk: &ChunkManifest) -> Vec<Value> {
    vec![
        Value::Text(download_id.to_owned()),
        Value::Integer(chunk.index as i64),
        Value::Integer(chunk.start as i64),
        Value::Integer(chunk.end as i64),
        Value::Integer(chunk.downloaded as i64),
        Value::Integer(bool_to_i64(chunk.completed)),
        match chunk.claimed_by {
            Some(n) => Value::Integer(n as i64),
            None => Value::Null,
        },
    ]
}

pub(crate) fn row_to_chunk(row: &rusqlite::Row) -> RusqliteResult<ChunkManifest> {
    Ok(ChunkManifest {
        index: row.get::<_, i64>(1)? as usize,
        start: row.get::<_, i64>(2)? as u64,
        end: row.get::<_, i64>(3)? as u64,
        downloaded: row.get::<_, i64>(4)? as u64,
        completed: i64_to_bool(row.get::<_, i64>(5)?),
        claimed_by: row.get::<_, Option<i64>>(6)?.map(|v| v as usize),
        dirty: false,
    })
}

impl Database {
    /// Incremental update for the 300 ms persist cycle.
    pub fn update_download_progress(
        &self,
        id: &str,
        downloaded_bytes: u64,
        dirty_chunks: &[ChunkManifest],
        state: &str,
        updated_at_ms: u64,
    ) -> Result<()> {
        let conn = self.lock_write();

        conn.execute_batch("BEGIN IMMEDIATE")
            .context("failed to begin transaction")?;

        let result = (|| -> Result<()> {
            conn.prepare_cached(
                "UPDATE downloads SET downloaded_bytes = ?1, state = ?2, updated_at_ms = ?3 WHERE id = ?4",
            )
            .context("failed to prepare download progress update")?
            .execute(params![downloaded_bytes as i64, state, updated_at_ms as i64, id])
            .context("failed to update download progress row")?;

            if !dirty_chunks.is_empty() {
                let mut stmt = conn
                    .prepare_cached(
                        "INSERT OR REPLACE INTO chunks (download_id, chunk_index, start_byte, end_byte,
                                 downloaded, completed, claimed_by)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    )
                    .context("failed to prepare chunk upsert")?;

                for chunk in dirty_chunks {
                    stmt.execute(rusqlite::params_from_iter(chunk_to_params(id, chunk)))
                        .context("failed to upsert chunk")?;
                }
            }

            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")
                    .context("failed to commit transaction")?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Incremental update for multiple downloads in a single transaction.
    pub fn update_downloads_progress_batch(&self, entries: &[ProgressBatchEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let conn = self.lock_write();

        conn.execute_batch("BEGIN IMMEDIATE")
            .context("failed to begin batch transaction")?;

        let result = (|| -> Result<()> {
            for entry in entries {
                conn.prepare_cached(
                    "UPDATE downloads SET downloaded_bytes = ?1, state = ?2, updated_at_ms = ?3 WHERE id = ?4",
                )
                .context("failed to prepare progress update in batch")?
                .execute(params![entry.downloaded_bytes as i64, entry.state.as_str(), entry.updated_at_ms as i64, entry.id.as_str()])
                .with_context(|| format!("failed to update progress row for {}", entry.id))?;

                if !entry.dirty_chunks.is_empty() {
                    let mut stmt = conn
                        .prepare_cached(
                            "INSERT OR REPLACE INTO chunks (download_id, chunk_index, start_byte, end_byte,
                                     downloaded, completed, claimed_by)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        )
                        .context("failed to prepare chunk upsert in batch")?;

                    for chunk in &entry.dirty_chunks {
                        stmt.execute(rusqlite::params_from_iter(chunk_to_params(&entry.id, chunk)))
                            .context("failed to upsert chunk in batch")?;
                    }
                }
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT")
                    .context("failed to commit batch transaction")?;
                Ok(())
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Load chunks for a single download on demand (lazy loading).
    pub fn load_chunks(&self, download_id: &str) -> Result<Vec<ChunkManifest>> {
        let conn = self.lock_read();
        self.fetch_chunks_inner(&conn, download_id)
    }

    /// Replace all chunks for `download_id` (caller must hold the lock).
    pub(crate) fn replace_chunks_inner(
        &self,
        conn: &Connection,
        download_id: &str,
        chunks: &[ChunkManifest],
    ) -> Result<()> {
        conn.prepare_cached("DELETE FROM chunks WHERE download_id = ?1")
            .context("failed to prepare chunk clear")?
            .execute(params![download_id])
            .context("failed to clear old chunks")?;

        if chunks.is_empty() {
            return Ok(());
        }

        let mut stmt = conn
            .prepare_cached(
                "INSERT INTO chunks (download_id, chunk_index, start_byte, end_byte,
                         downloaded, completed, claimed_by)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .context("failed to prepare chunk insert")?;

        for chunk in chunks {
            stmt.execute(rusqlite::params_from_iter(chunk_to_params(
                download_id,
                chunk,
            )))
            .context("failed to insert chunk")?;
        }

        Ok(())
    }

    /// Fetch all chunks for a download (caller must hold the lock).
    pub(crate) fn fetch_chunks_inner(
        &self,
        conn: &Connection,
        download_id: &str,
    ) -> Result<Vec<ChunkManifest>> {
        let mut stmt = conn
            .prepare_cached("SELECT * FROM chunks WHERE download_id = ?1 ORDER BY chunk_index")
            .context("failed to prepare fetch_chunks query")?;

        let chunks = stmt
            .query_map(params![download_id], row_to_chunk)
            .context("failed to map chunk rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect chunks")?;

        Ok(chunks)
    }
}
