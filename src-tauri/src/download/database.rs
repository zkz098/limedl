use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, params, types::Value};


use super::manifest::{ChunkManifest, Manifest};
use super::types::{AdaptiveProfile, ChecksumMode, DownloadState, ThreadMode};

type RusqliteResult<T> = std::result::Result<T, rusqlite::Error>;

// ── Database struct ──────────────────────────────────────────────

pub(crate) struct Database {
    conn: Mutex<Connection>,
}

// ── Enum ↔ TEXT conversions (serde snake_case representation) ────

fn state_to_text(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "queued",
        DownloadState::Downloading => "downloading",
        DownloadState::Paused => "paused",
        DownloadState::Retrying => "retrying",
        DownloadState::Verifying => "verifying",
        DownloadState::Completed => "completed",
        DownloadState::Failed => "failed",
        DownloadState::Canceled => "canceled",
    }
}

pub(crate) fn download_state_to_text(state: &DownloadState) -> &'static str {
    state_to_text(*state)
}

fn text_to_state(s: &str) -> RusqliteResult<DownloadState> {
    match s {
        "queued" => Ok(DownloadState::Queued),
        "downloading" => Ok(DownloadState::Downloading),
        "paused" => Ok(DownloadState::Paused),
        "retrying" => Ok(DownloadState::Retrying),
        "verifying" => Ok(DownloadState::Verifying),
        "completed" => Ok(DownloadState::Completed),
        "failed" => Ok(DownloadState::Failed),
        "canceled" => Ok(DownloadState::Canceled),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown download state: {other}"
        ))),
    }
}

fn thread_mode_to_text(mode: ThreadMode) -> &'static str {
    match mode {
        ThreadMode::Fixed => "fixed",
        ThreadMode::Adaptive => "adaptive",
    }
}

fn text_to_thread_mode(s: &str) -> RusqliteResult<ThreadMode> {
    match s {
        "fixed" => Ok(ThreadMode::Fixed),
        "adaptive" => Ok(ThreadMode::Adaptive),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown thread mode: {other}"
        ))),
    }
}

fn checksum_mode_to_text(mode: ChecksumMode) -> &'static str {
    match mode {
        ChecksumMode::None => "none",
        ChecksumMode::Blake3 => "blake3",
        ChecksumMode::Sha256 => "sha256",
        ChecksumMode::Xxh3128 => "xxh3_128",
    }
}

fn text_to_checksum_mode(s: &str) -> RusqliteResult<ChecksumMode> {
    match s {
        "none" => Ok(ChecksumMode::None),
        "blake3" => Ok(ChecksumMode::Blake3),
        "sha256" => Ok(ChecksumMode::Sha256),
        "xxh3_128" => Ok(ChecksumMode::Xxh3128),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown checksum mode: {other}"
        ))),
    }
}

fn adaptive_profile_to_text(profile: AdaptiveProfile) -> &'static str {
    match profile {
        AdaptiveProfile::Conservative => "conservative",
        AdaptiveProfile::Balanced => "balanced",
        AdaptiveProfile::Aggressive => "aggressive",
    }
}

fn text_to_adaptive_profile(s: &str) -> RusqliteResult<AdaptiveProfile> {
    match s {
        "conservative" => Ok(AdaptiveProfile::Conservative),
        "balanced" => Ok(AdaptiveProfile::Balanced),
        "aggressive" => Ok(AdaptiveProfile::Aggressive),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown adaptive profile: {other}"
        ))),
    }
}

// ── Primitives → Value helpers ───────────────────────────────────

fn bool_to_i64(b: bool) -> i64 {
    if b { 1 } else { 0 }
}

fn i64_to_bool(v: i64) -> bool {
    v != 0
}

fn opt_u64_to_value(v: Option<u64>) -> Value {
    match v {
        Some(n) => Value::Integer(n as i64),
        None => Value::Null,
    }
}

fn opt_usize_to_value(v: Option<usize>) -> Value {
    match v {
        Some(n) => Value::Integer(n as i64),
        None => Value::Null,
    }
}

// ── Manifest ↔ row conversion ────────────────────────────────────
//
// Column order in the downloads table:
//   0  id                           1  url
//   2  final_url                    3  user_agent
//   4  destination_dir              5  file_name
//   6  file_name_locked             7  destination_path
//   8  temp_path                    9  total_bytes
//  10  downloaded_bytes            11  supports_ranges
//  12  connection_count            13  thread_mode
//  14  requested_thread_count      15  desired_thread_count
//  16  allocated_thread_count      17  adaptive_profile_snapshot
//  18  thread_note                 19  etag
//  20  last_modified               21  state
//  22  checksum_mode               23  checksum
//  24  error                       25  created_at_ms
//  26  updated_at_ms               27  chunk_size

fn manifest_to_row(manifest: &Manifest) -> Vec<Value> {
    vec![
        // 0: id
        Value::Text(manifest.id.clone()),
        // 1: url
        Value::Text(manifest.url.clone()),
        // 2: final_url
        Value::Text(manifest.final_url.clone()),
        // 3: user_agent
        Value::Text(manifest.user_agent.clone()),
        // 4: destination_dir
        Value::Text(manifest.destination_dir.clone()),
        // 5: file_name
        Value::Text(manifest.file_name.clone()),
        // 6: file_name_locked
        Value::Integer(bool_to_i64(manifest.file_name_locked)),
        // 7: destination_path
        Value::Text(manifest.destination_path.clone()),
        // 8: temp_path
        Value::Text(manifest.temp_path.clone()),
        // 9: total_bytes
        opt_u64_to_value(manifest.total_bytes),
        // 10: downloaded_bytes
        Value::Integer(manifest.downloaded_bytes as i64),
        // 11: supports_ranges
        Value::Integer(bool_to_i64(manifest.supports_ranges)),
        // 12: connection_count
        Value::Integer(manifest.connection_count as i64),
        // 13: thread_mode
        Value::Text(thread_mode_to_text(manifest.thread_mode).to_owned()),
        // 14: requested_thread_count
        opt_usize_to_value(manifest.requested_thread_count),
        // 15: desired_thread_count
        opt_usize_to_value(manifest.desired_thread_count),
        // 16: allocated_thread_count
        opt_usize_to_value(manifest.allocated_thread_count),
        // 17: adaptive_profile_snapshot
        manifest.adaptive_profile_snapshot.map_or(Value::Null, |p| {
            Value::Text(adaptive_profile_to_text(p).to_owned())
        }),
        // 18: thread_note
        manifest
            .thread_note
            .clone()
            .map_or(Value::Null, Value::Text),
        // 19: etag
        manifest.etag.clone().map_or(Value::Null, Value::Text),
        // 20: last_modified
        manifest
            .last_modified
            .clone()
            .map_or(Value::Null, Value::Text),
        // 21: state
        Value::Text(state_to_text(manifest.state).to_owned()),
        // 22: checksum_mode
        Value::Text(checksum_mode_to_text(manifest.checksum_mode).to_owned()),
        // 23: checksum
        manifest.checksum.clone().map_or(Value::Null, Value::Text),
        // 24: error
        manifest.error.clone().map_or(Value::Null, Value::Text),
        // 25: created_at_ms
        Value::Integer(manifest.created_at_ms as i64),
        // 26: updated_at_ms
        Value::Integer(manifest.updated_at_ms as i64),
        // 27: chunk_size
        Value::Integer(manifest.chunk_size as i64),
        // 28: mirror_url
        manifest.mirror_url.clone().map_or(Value::Null, Value::Text),
        // 29: mirror_urls (JSON-serialized)
        Value::Text(serde_json::to_string(&manifest.mirror_urls).unwrap_or_else(|_| "[]".into())),
        // 30: current_mirror_index
        Value::Integer(manifest.current_mirror_index as i64),
    ]
}

fn row_to_manifest(row: &rusqlite::Row) -> RusqliteResult<Manifest> {
    let thread_mode_str: String = row.get(13)?;
    let state_str: String = row.get(21)?;
    let checksum_mode_str: String = row.get(22)?;

    let adaptive_snapshot: Option<String> = row.get(17)?;
    let adaptive_profile_snapshot = adaptive_snapshot
        .as_deref()
        .map(text_to_adaptive_profile)
        .transpose()?;

    Ok(Manifest {
        id: row.get(0)?,
        url: row.get(1)?,
        final_url: row.get(2)?,
        user_agent: row.get(3)?,
        destination_dir: row.get(4)?,
        file_name: row.get(5)?,
        file_name_locked: i64_to_bool(row.get::<_, i64>(6)?),
        destination_path: row.get(7)?,
        temp_path: row.get(8)?,
        total_bytes: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
        downloaded_bytes: row.get::<_, i64>(10)? as u64,
        supports_ranges: i64_to_bool(row.get::<_, i64>(11)?),
        connection_count: row.get::<_, i64>(12)? as usize,
        thread_mode: text_to_thread_mode(&thread_mode_str)?,
        requested_thread_count: row.get::<_, Option<i64>>(14)?.map(|v| v as usize),
        desired_thread_count: row.get::<_, Option<i64>>(15)?.map(|v| v as usize),
        allocated_thread_count: row.get::<_, Option<i64>>(16)?.map(|v| v as usize),
        adaptive_profile_snapshot,
        thread_note: row.get(18)?,
        etag: row.get(19)?,
        last_modified: row.get(20)?,
        state: text_to_state(&state_str)?,
        checksum_mode: text_to_checksum_mode(&checksum_mode_str)?,
        checksum: row.get(23)?,
        error: row.get(24)?,
        created_at_ms: row.get::<_, i64>(25)? as u64,
        updated_at_ms: row.get::<_, i64>(26)? as u64,
        chunks: Vec::new(),
        cdn_accelerated: false,
        chunk_size: row.get::<_, i64>(27)? as u64,
        mirror_url: row.get(28)?,
        mirror_urls: row
            .get::<_, String>(29)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        current_mirror_index: row.get::<_, i64>(30).unwrap_or(0) as usize,
    })
}

// ── ChunkManifest ↔ params/row ────────────────────────────────────
//
// Column order in the chunks table:
//   0  download_id    1  chunk_index    2  start_byte
//   3  end_byte       4  downloaded     5  completed
//   6  claimed_by

fn chunk_to_params(download_id: &str, chunk: &ChunkManifest) -> Vec<Value> {
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

fn row_to_chunk(row: &rusqlite::Row) -> RusqliteResult<ChunkManifest> {
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

// ── Schema ───────────────────────────────────────────────────────

const CREATE_TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS downloads (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    final_url TEXT NOT NULL DEFAULT '',
    user_agent TEXT NOT NULL,
    destination_dir TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_name_locked INTEGER NOT NULL DEFAULT 1,
    destination_path TEXT NOT NULL,
    temp_path TEXT NOT NULL,
    total_bytes INTEGER,
    downloaded_bytes INTEGER NOT NULL DEFAULT 0,
    supports_ranges INTEGER NOT NULL DEFAULT 0,
    connection_count INTEGER NOT NULL DEFAULT 0,
    thread_mode TEXT NOT NULL DEFAULT 'adaptive',
    requested_thread_count INTEGER,
    desired_thread_count INTEGER,
    allocated_thread_count INTEGER,
    adaptive_profile_snapshot TEXT,
    thread_note TEXT,
    etag TEXT,
    last_modified TEXT,
    state TEXT NOT NULL DEFAULT 'queued',
    checksum_mode TEXT NOT NULL DEFAULT 'blake3',
    checksum TEXT,
    error TEXT,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    chunk_size INTEGER NOT NULL DEFAULT 4194304,
    mirror_url TEXT,
    mirror_urls TEXT NOT NULL DEFAULT '[]',
    current_mirror_index INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS chunks (
    download_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    start_byte INTEGER NOT NULL,
    end_byte INTEGER NOT NULL,
    downloaded INTEGER NOT NULL DEFAULT 0,
    completed INTEGER NOT NULL DEFAULT 0,
    claimed_by INTEGER,
    PRIMARY KEY (download_id, chunk_index),
    FOREIGN KEY (download_id) REFERENCES downloads(id) ON DELETE CASCADE
);
";

// ── Database impl ────────────────────────────────────────────────

impl Database {
    /// Open (or create) the SQLite database at `path`.
    ///
    /// Enables WAL mode and foreign keys, then ensures the schema exists.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database at {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("failed to enable WAL mode")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .context("failed to enable foreign keys")?;
        conn.execute_batch(CREATE_TABLES_SQL)
            .context("failed to create tables")?;

        // Backfill chunk_size for pre-existing rows; ignore error if column already exists.
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN chunk_size INTEGER NOT NULL DEFAULT 4194304",
            [],
        );

        // Backfill mirror columns for pre-existing rows (GitHub mirror feature).
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN mirror_url TEXT",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN mirror_urls TEXT NOT NULL DEFAULT '[]'",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE downloads ADD COLUMN current_mirror_index INTEGER NOT NULL DEFAULT 0",
            [],
        );

        // Remove any SFTP download entries whose IDs start with "sftp:"
        // after the SFTP protocol support was removed.
        let _ = conn.execute("DELETE FROM downloads WHERE id LIKE 'sftp:%'", []);

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite database for testing.
    ///
    /// Enables foreign keys and runs the CREATE TABLE statements.
    /// Does NOT enable WAL mode (unnecessary for in-memory single-connection tests
    /// and can cause "database is locked" errors).
    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("failed to open in-memory database")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .context("failed to enable foreign keys")?;
        conn.execute_batch(CREATE_TABLES_SQL)
            .context("failed to create tables")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("database connection lock poisoned, recovering with inner state");
            poisoned.into_inner()
        })
    }

    // ── download CRUD ────────────────────────────────────────

    /// Insert a new download (or replace if the id already exists).
    /// Also inserts all chunks.
    pub(crate) fn insert_download(&self, manifest: &Manifest) -> Result<()> {
        let conn = self.lock_conn();

        let params = manifest_to_row(manifest);
        conn.execute(
            "INSERT OR REPLACE INTO downloads (
                id, url, final_url, user_agent, destination_dir, file_name,
                file_name_locked, destination_path, temp_path, total_bytes,
                downloaded_bytes, supports_ranges, connection_count, thread_mode,
                requested_thread_count, desired_thread_count, allocated_thread_count,
                adaptive_profile_snapshot, thread_note, etag, last_modified,
                state, checksum_mode, checksum, error, created_at_ms, updated_at_ms,
                chunk_size, mirror_url, mirror_urls, current_mirror_index
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,
                      ?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,
                      ?22,?23,?24,?25,?26,?27,?28,?29,?30,?31)",
            rusqlite::params_from_iter(params),
        )
        .with_context(|| format!("failed to insert download {}", manifest.id))?;

        self.replace_chunks_inner(&conn, &manifest.id, &manifest.chunks)?;

        Ok(())
    }

    /// Full update of all download fields and chunks.
    #[allow(dead_code)]
    pub(crate) fn update_download(&self, manifest: &Manifest) -> Result<()> {
        let conn = self.lock_conn();

        // Reorder manifest_to_row: [id, url, final_url, ..., updated_at_ms]
        // → [url, final_url, ..., updated_at_ms, id] for the UPDATE + WHERE.
        let mut params = manifest_to_row(manifest);
        let id_value = params.remove(0); // extract id from position 0
        params.push(id_value); // append for WHERE clause

        conn.execute(
            "UPDATE downloads SET
                url = ?1, final_url = ?2, user_agent = ?3, destination_dir = ?4,
                file_name = ?5, file_name_locked = ?6, destination_path = ?7,
                temp_path = ?8, total_bytes = ?9, downloaded_bytes = ?10,
                supports_ranges = ?11, connection_count = ?12, thread_mode = ?13,
                requested_thread_count = ?14, desired_thread_count = ?15,
                allocated_thread_count = ?16, adaptive_profile_snapshot = ?17,
                thread_note = ?18, etag = ?19, last_modified = ?20,
                state = ?21, checksum_mode = ?22, checksum = ?23, error = ?24,
                created_at_ms = ?25, updated_at_ms = ?26, chunk_size = ?27,
                mirror_url = ?28, mirror_urls = ?29, current_mirror_index = ?30
             WHERE id = ?31",
            rusqlite::params_from_iter(params),
        )
        .with_context(|| format!("failed to update download {}", manifest.id))?;

        self.replace_chunks_inner(&conn, &manifest.id, &manifest.chunks)?;

        Ok(())
    }

    /// Incremental update for the 300 ms persist cycle.
    ///
    /// Only writes chunks that have changed (dirty flag) since the last persist.
    /// Uses `INSERT OR REPLACE` so each chunk is upserted individually without
    /// a full DELETE + INSERT of the entire chunk set.
    /// All operations run within a single transaction for consistency.
    pub(crate) fn update_download_progress(
        &self,
        id: &str,
        downloaded_bytes: u64,
        dirty_chunks: &[ChunkManifest],
        state: &str,
        updated_at_ms: u64,
    ) -> Result<()> {
        let conn = self.lock_conn();

        conn.execute_batch("BEGIN IMMEDIATE")
            .context("failed to begin transaction")?;

        let result = (|| -> Result<()> {
            conn.execute(
                "UPDATE downloads SET downloaded_bytes = ?1, state = ?2, updated_at_ms = ?3 WHERE id = ?4",
                params![downloaded_bytes as i64, state, updated_at_ms as i64, id],
            )
            .context("failed to update download progress row")?;

            // Incremental chunk persist: only UPSERT dirty chunks.
            if !dirty_chunks.is_empty() {
                let mut stmt = conn
                    .prepare(
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

    /// Delete a download and all its chunks (cascaded via FK).
    pub(crate) fn delete_download(&self, id: &str) -> Result<()> {
        let conn = self.lock_conn();
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])
            .with_context(|| format!("failed to delete download {id}"))?;
        Ok(())
    }

    /// Fetch a single download with its chunks.
    #[allow(dead_code)]
    pub(crate) fn get_download(&self, id: &str) -> Result<Option<Manifest>> {
        let conn = self.lock_conn();

        let mut stmt = conn
            .prepare("SELECT * FROM downloads WHERE id = ?1")
            .context("failed to prepare get_download query")?;

        let opt = stmt
            .query_row(params![id], row_to_manifest)
            .optional()
            .context("failed to query download")?;

        match opt {
            Some(mut manifest) => {
                manifest.chunks = self.fetch_chunks_inner(&conn, id)?;
                Ok(Some(manifest))
            }
            None => Ok(None),
        }
    }

    /// Return every download in the database, each with its chunks populated.
    pub(crate) fn list_downloads(&self) -> Result<Vec<Manifest>> {
        let conn = self.lock_conn();

        let mut stmt = conn
            .prepare("SELECT * FROM downloads ORDER BY created_at_ms DESC")
            .context("failed to prepare list_downloads query")?;

        let mut manifests: Vec<Manifest> = stmt
            .query_map([], row_to_manifest)
            .context("failed to map download rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect downloads")?;

        // Fetch all chunks in one pass and group by download_id.
        let mut chunk_stmt = conn
            .prepare("SELECT * FROM chunks ORDER BY download_id, chunk_index")
            .context("failed to prepare list_chunks query")?;

        let all_chunks: Vec<(String, ChunkManifest)> = chunk_stmt
            .query_map([], |row| {
                let download_id: String = row.get(0)?;
                let chunk = row_to_chunk(row)?;
                Ok((download_id, chunk))
            })
            .context("failed to map chunk rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect chunks")?;

        for (download_id, chunk) in all_chunks {
            if let Some(manifest) = manifests.iter_mut().find(|m| m.id == download_id) {
                manifest.chunks.push(chunk);
            }
        }

        Ok(manifests)
    }

    /// Total number of downloads.
    #[allow(dead_code)]
    pub(crate) fn count_downloads(&self) -> Result<usize> {
        let conn = self.lock_conn();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
            .context("failed to count downloads")?;
        Ok(count as usize)
    }

    // ── internal helpers ──────────────────────────────────────

    /// Replace all chunks for `download_id` (caller must hold the lock).
    fn replace_chunks_inner(
        &self,
        conn: &Connection,
        download_id: &str,
        chunks: &[ChunkManifest],
    ) -> Result<()> {
        conn.execute(
            "DELETE FROM chunks WHERE download_id = ?1",
            params![download_id],
        )
        .context("failed to clear old chunks")?;

        if chunks.is_empty() {
            return Ok(());
        }

        let mut stmt = conn
            .prepare(
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
    #[allow(dead_code)]
    fn fetch_chunks_inner(
        &self,
        conn: &Connection,
        download_id: &str,
    ) -> Result<Vec<ChunkManifest>> {
        let mut stmt = conn
            .prepare("SELECT * FROM chunks WHERE download_id = ?1 ORDER BY chunk_index")
            .context("failed to prepare fetch_chunks query")?;

        let chunks = stmt
            .query_map(params![download_id], row_to_chunk)
            .context("failed to map chunk rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect chunks")?;

        Ok(chunks)
    }
}

// ── Extension trait for rusqlite optional rows ───────────────────

#[allow(dead_code)]
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ntest::timeout;

    use super::*;
    use super::super::manifest::CHUNK_SIZE;
    use super::super::types::default_http_user_agent;

    /// Helper: create a `Manifest` with sensible defaults for testing.
    fn new_test_manifest(id: &str, url: &str, file_name: &str) -> Manifest {
        Manifest {
            id: id.to_string(),
            url: url.to_string(),
            final_url: url.to_string(),
            user_agent: default_http_user_agent(),
            destination_dir: "/tmp".to_string(),
            file_name: file_name.to_string(),
            file_name_locked: true,
            destination_path: format!("/tmp/{file_name}"),
            temp_path: format!("/tmp/{file_name}.tmp"),
            total_bytes: Some(1024),
            downloaded_bytes: 0,
            supports_ranges: true,
            chunk_size: CHUNK_SIZE,
            connection_count: 1,
            thread_mode: ThreadMode::Adaptive,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile_snapshot: None,
            thread_note: None,
            etag: None,
            last_modified: None,
            state: DownloadState::Queued,
            cdn_accelerated: false,
            checksum_mode: ChecksumMode::Blake3,
            checksum: None,
            error: None,
            created_at_ms: 1000,
            updated_at_ms: 1000,
            chunks: Vec::new(),
            mirror_url: None,
            mirror_urls: Vec::new(),
            current_mirror_index: 0,
        }
    }

    /// Helper: count chunks for a download by querying the chunks table directly.
    fn count_chunks(db: &Database, download_id: &str) -> usize {
        let conn = db.lock_conn();
        conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE download_id = ?1",
            params![download_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize
    }

    #[timeout(30_000)]
    #[test]
    fn open_in_memory_creates_empty_db() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.count_downloads().unwrap(), 0);
    }

    #[timeout(30_000)]
    #[test]
    fn insert_and_get_download_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let mut manifest = new_test_manifest("test-1", "https://example.com/file", "file.bin");
        manifest.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 511,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        }];
        db.insert_download(&manifest).unwrap();
        let loaded = db.get_download("test-1").unwrap().expect("should exist");
        assert_eq!(loaded.id, "test-1");
        assert_eq!(loaded.url, "https://example.com/file");
        assert_eq!(loaded.file_name, "file.bin");
        assert_eq!(loaded.state, DownloadState::Queued);
        assert_eq!(loaded.chunks.len(), 1);
    }

    #[timeout(30_000)]
    #[test]
    fn insert_or_replace_same_id_replaces() {
        let db = Database::open_in_memory().unwrap();

        let m1 = new_test_manifest("id-1", "https://a.com/f1", "first.txt");
        db.insert_download(&m1).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);

        let m2 = new_test_manifest("id-1", "https://b.com/f2", "second.txt");
        db.insert_download(&m2).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);

        let loaded = db.get_download("id-1").unwrap().expect("should exist");
        assert_eq!(loaded.file_name, "second.txt");
        assert_eq!(loaded.url, "https://b.com/f2");
    }

    #[timeout(30_000)]
    #[test]
    fn get_nonexistent_download_returns_none() {
        let db = Database::open_in_memory().unwrap();
        let result = db.get_download("no-such-id").unwrap();
        assert!(result.is_none());
    }

    #[timeout(30_000)]
    #[test]
    fn delete_download_cascades_to_chunks() {
        let db = Database::open_in_memory().unwrap();
        let mut manifest = new_test_manifest("del-1", "https://example.com/file", "delete.bin");
        manifest.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 511,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 512,
                end: 1023,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&manifest).unwrap();
        assert_eq!(count_chunks(&db, "del-1"), 2);

        db.delete_download("del-1").unwrap();
        assert!(db.get_download("del-1").unwrap().is_none());
        assert_eq!(count_chunks(&db, "del-1"), 0);
    }

    #[timeout(30_000)]
    #[test]
    fn delete_nonexistent_does_not_panic() {
        let db = Database::open_in_memory().unwrap();
        let result = db.delete_download("no-such-id");
        assert!(result.is_ok());
    }

    #[timeout(30_000)]
    #[test]
    fn insert_then_get_preserves_all_fields() {
        let db = Database::open_in_memory().unwrap();
        let mut manifest = new_test_manifest("all-fields", "https://example.com/file", "all.txt");
        // Override with non-default values for every field
        manifest.state = DownloadState::Downloading;
        manifest.downloaded_bytes = 1024;
        manifest.etag = Some("\"abc123\"".into());
        manifest.thread_mode = ThreadMode::Fixed;
        manifest.requested_thread_count = Some(4);
        manifest.checksum_mode = ChecksumMode::Sha256;
        manifest.checksum = Some("sha256hash".into());
        manifest.error = Some("some error".into());
        manifest.adaptive_profile_snapshot = Some(AdaptiveProfile::Aggressive);
        manifest.thread_note = Some("my thread note".into());
        manifest.total_bytes = Some(99999);
        manifest.last_modified = Some("Mon, 01 Jan 2024 00:00:00 GMT".into());
        manifest.desired_thread_count = Some(6);
        manifest.allocated_thread_count = Some(4);
        manifest.final_url = "https://redirect.example.com/file".to_string();
        manifest.user_agent = "custom-agent/1.0".to_string();
        manifest.destination_dir = "/custom/path".to_string();
        manifest.file_name_locked = false;
        manifest.destination_path = "/custom/path/all.txt".to_string();
        manifest.temp_path = "/custom/path/all.txt.tmp".to_string();
        manifest.supports_ranges = false;
        manifest.connection_count = 3;
        manifest.chunk_size = 8192;
        manifest.created_at_ms = 5000;
        manifest.updated_at_ms = 6000;
        manifest.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 500,
            downloaded: 500,
            completed: true,
            claimed_by: Some(1),
            dirty: false,
        }];

        db.insert_download(&manifest).unwrap();
        let loaded = db.get_download("all-fields").unwrap().expect("should exist");

        assert_eq!(loaded.id, "all-fields");
        assert_eq!(loaded.url, "https://example.com/file");
        assert_eq!(loaded.final_url, "https://redirect.example.com/file");
        assert_eq!(loaded.user_agent, "custom-agent/1.0");
        assert_eq!(loaded.destination_dir, "/custom/path");
        assert_eq!(loaded.file_name, "all.txt");
        assert!(!loaded.file_name_locked);
        assert_eq!(loaded.destination_path, "/custom/path/all.txt");
        assert_eq!(loaded.temp_path, "/custom/path/all.txt.tmp");
        assert_eq!(loaded.total_bytes, Some(99999));
        assert_eq!(loaded.downloaded_bytes, 1024);
        assert!(!loaded.supports_ranges);
        assert_eq!(loaded.connection_count, 3);
        assert_eq!(loaded.thread_mode, ThreadMode::Fixed);
        assert_eq!(loaded.requested_thread_count, Some(4));
        assert_eq!(loaded.desired_thread_count, Some(6));
        assert_eq!(loaded.allocated_thread_count, Some(4));
        assert_eq!(
            loaded.adaptive_profile_snapshot,
            Some(AdaptiveProfile::Aggressive)
        );
        assert_eq!(loaded.thread_note.as_deref(), Some("my thread note"));
        assert_eq!(loaded.etag.as_deref(), Some("\"abc123\""));
        assert_eq!(
            loaded.last_modified.as_deref(),
            Some("Mon, 01 Jan 2024 00:00:00 GMT")
        );
        assert_eq!(loaded.state, DownloadState::Downloading);
        assert_eq!(loaded.checksum_mode, ChecksumMode::Sha256);
        assert_eq!(loaded.checksum.as_deref(), Some("sha256hash"));
        assert_eq!(loaded.error.as_deref(), Some("some error"));
        assert_eq!(loaded.created_at_ms, 5000);
        assert_eq!(loaded.updated_at_ms, 6000);
        assert_eq!(loaded.chunks.len(), 1);
        assert_eq!(loaded.chunks[0].index, 0);
        assert_eq!(loaded.chunks[0].start, 0);
        assert_eq!(loaded.chunks[0].end, 500);
        assert_eq!(loaded.chunks[0].downloaded, 500);
        assert!(loaded.chunks[0].completed);
        assert_eq!(loaded.chunks[0].claimed_by, Some(1));
        // dirty is not persisted; always false when loaded from DB
        assert!(!loaded.chunks[0].dirty);
        assert_eq!(loaded.chunk_size, 8192);
        // cdn_accelerated is hardcoded to false in row_to_manifest
        assert!(!loaded.cdn_accelerated);
    }

    #[timeout(30_000)]
    #[test]
    fn insert_duplicate_id_replaces() {
        let db = Database::open_in_memory().unwrap();

        // Insert original with 2 chunks
        let mut orig = new_test_manifest("dup", "https://a.com/orig", "original.txt");
        orig.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 499,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 500,
                end: 999,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&orig).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);

        // Replace with 1 chunk and different url / file_name
        let mut replacement = new_test_manifest("dup", "https://b.com/replaced", "replaced.txt");
        replacement.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 199,
            downloaded: 100,
            completed: false,
            claimed_by: None,
            dirty: false,
        }];
        db.insert_download(&replacement).unwrap();

        let loaded = db.get_download("dup").unwrap().expect("should exist");
        assert_eq!(loaded.url, "https://b.com/replaced");
        assert_eq!(loaded.file_name, "replaced.txt");
        assert_eq!(loaded.chunks.len(), 1);
        assert_eq!(loaded.chunks[0].downloaded, 100);
    }

    #[timeout(30_000)]
    #[test]
    fn update_download_modifies_all_fields() {
        let db = Database::open_in_memory().unwrap();

        let mut m = new_test_manifest("upd", "https://a.com/old", "old.txt");
        m.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 499,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        }];
        db.insert_download(&m).unwrap();

        // Update with all new values (same id)
        let mut updated = new_test_manifest("upd", "https://b.com/new", "new.txt");
        updated.state = DownloadState::Completed;
        updated.downloaded_bytes = 500;
        updated.updated_at_ms = 9999;
        updated.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 499,
            downloaded: 500,
            completed: true,
            claimed_by: None,
            dirty: false,
        }];
        db.update_download(&updated).unwrap();

        let loaded = db.get_download("upd").unwrap().expect("should exist");
        assert_eq!(loaded.state, DownloadState::Completed);
        assert_eq!(loaded.downloaded_bytes, 500);
        assert_eq!(loaded.file_name, "new.txt");
        assert_eq!(loaded.url, "https://b.com/new");
        assert_eq!(loaded.updated_at_ms, 9999);
        assert_eq!(loaded.chunks.len(), 1);
        assert!(loaded.chunks[0].completed);
        assert_eq!(loaded.chunks[0].downloaded, 500);
    }

    #[timeout(30_000)]
    #[test]
    fn update_download_progress_incremental() {
        let db = Database::open_in_memory().unwrap();

        let mut m = new_test_manifest("prog", "https://example.com/prog", "progress.bin");
        m.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 499,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 500,
                end: 999,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&m).unwrap();

        // Update progress: chunk 0 gets partial progress, use state "downloading"
        let dirty_chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 499,
            downloaded: 250,
            completed: false,
            claimed_by: Some(0),
            dirty: true,
        }];
        db.update_download_progress("prog", 250, &dirty_chunks, "downloading", 2000)
            .unwrap();

        let loaded = db.get_download("prog").unwrap().expect("should exist");
        assert_eq!(loaded.downloaded_bytes, 250);
        assert_eq!(loaded.state, DownloadState::Downloading);
        assert_eq!(loaded.updated_at_ms, 2000);
        assert_eq!(loaded.chunks.len(), 2);
        // Dirty chunk was updated
        assert_eq!(loaded.chunks[0].downloaded, 250);
        assert!(!loaded.chunks[0].completed);
        assert_eq!(loaded.chunks[0].claimed_by, Some(0));
        // Non-dirty chunk untouched
        assert_eq!(loaded.chunks[1].downloaded, 0);
        assert!(!loaded.chunks[1].completed);
        // Other fields from insert should be unchanged
        assert_eq!(loaded.url, "https://example.com/prog");
        assert_eq!(loaded.file_name, "progress.bin");
    }

    #[timeout(30_000)]
    #[test]
    fn progress_empty_chunks_updates_row_only() {
        let db = Database::open_in_memory().unwrap();

        let mut m = new_test_manifest("empty-chunks", "https://example.com/ec", "ec.bin");
        m.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 499,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        }];
        db.insert_download(&m).unwrap();

        // Update with empty dirty chunks vec
        db.update_download_progress("empty-chunks", 100, &[], "downloading", 3000)
            .unwrap();

        let loaded = db.get_download("empty-chunks").unwrap().expect("should exist");
        assert_eq!(loaded.downloaded_bytes, 100);
        assert_eq!(loaded.state, DownloadState::Downloading);
        assert_eq!(loaded.updated_at_ms, 3000);
        // Chunks should remain unchanged
        assert_eq!(loaded.chunks.len(), 1);
        assert_eq!(loaded.chunks[0].downloaded, 0);
    }

    #[timeout(30_000)]
    #[test]
    fn list_returns_all_with_chunks() {
        let db = Database::open_in_memory().unwrap();

        let mut m1 = new_test_manifest("list-1", "https://a.com/f1", "first.txt");
        m1.created_at_ms = 2000;
        m1.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 99,
            downloaded: 50,
            completed: false,
            claimed_by: None,
            dirty: false,
        }];
        db.insert_download(&m1).unwrap();

        let mut m2 = new_test_manifest("list-2", "https://b.com/f2", "second.txt");
        m2.created_at_ms = 1000; // older timestamp
        m2.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 199,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 200,
                end: 399,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&m2).unwrap();

        let list = db.list_downloads().unwrap();
        assert_eq!(list.len(), 2);
        // Ordered by created_at_ms DESC: newer (2000) first, older (1000) second
        assert_eq!(list[0].id, "list-1");
        assert_eq!(list[1].id, "list-2");
        // Chunks populated
        assert_eq!(list[0].chunks.len(), 1);
        assert_eq!(list[1].chunks.len(), 2);
    }

    #[timeout(30_000)]
    #[test]
    fn empty_chunks_persisted_and_loaded() {
        let db = Database::open_in_memory().unwrap();
        let manifest = new_test_manifest("no-chunks", "https://example.com/nc", "nochunks.bin");
        assert!(manifest.chunks.is_empty());
        db.insert_download(&manifest).unwrap();
        let loaded = db.get_download("no-chunks").unwrap().expect("should exist");
        assert!(loaded.chunks.is_empty());
    }

    #[timeout(30_000)]
    #[test]
    fn count_downloads_accurate() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.count_downloads().unwrap(), 0);

        let m1 = new_test_manifest("cnt-1", "https://a.com/f1", "f1.bin");
        db.insert_download(&m1).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);

        let m2 = new_test_manifest("cnt-2", "https://b.com/f2", "f2.bin");
        db.insert_download(&m2).unwrap();
        assert_eq!(db.count_downloads().unwrap(), 2);

        db.delete_download("cnt-1").unwrap();
        assert_eq!(db.count_downloads().unwrap(), 1);

        db.delete_download("cnt-2").unwrap();
        assert_eq!(db.count_downloads().unwrap(), 0);
    }

    #[timeout(30_000)]
    #[test]
    fn null_optionals_roundtrip() {
        let db = Database::open_in_memory().unwrap();
        let mut manifest = new_test_manifest("null-opt", "https://example.com/null", "null.bin");
        // Set all Option fields to None
        manifest.total_bytes = None;
        manifest.etag = None;
        manifest.last_modified = None;
        manifest.checksum = None;
        manifest.error = None;
        manifest.requested_thread_count = None;
        manifest.desired_thread_count = None;
        manifest.allocated_thread_count = None;
        manifest.adaptive_profile_snapshot = None;
        manifest.thread_note = None;
        // With total_bytes=None, ensure no chunks are planned
        manifest.supports_ranges = false;
        manifest.chunks = Vec::new();

        db.insert_download(&manifest).unwrap();
        let loaded = db.get_download("null-opt").unwrap().expect("should exist");

        assert!(loaded.total_bytes.is_none());
        assert!(loaded.etag.is_none());
        assert!(loaded.last_modified.is_none());
        assert!(loaded.checksum.is_none());
        assert!(loaded.error.is_none());
        assert!(loaded.requested_thread_count.is_none());
        assert!(loaded.desired_thread_count.is_none());
        assert!(loaded.allocated_thread_count.is_none());
        assert!(loaded.adaptive_profile_snapshot.is_none());
        assert!(loaded.thread_note.is_none());
        assert!(loaded.chunks.is_empty());
    }

    #[timeout(30_000)]
    #[test]
    fn chunk_null_claimed_by_roundtrips() {
        let db = Database::open_in_memory().unwrap();
        let mut m = new_test_manifest("claim-none", "https://example.com/cn", "cn.bin");
        m.chunks = vec![ChunkManifest {
            index: 0,
            start: 0,
            end: 99,
            downloaded: 0,
            completed: false,
            claimed_by: None,
            dirty: false,
        }];
        db.insert_download(&m).unwrap();
        let loaded = db.get_download("claim-none").unwrap().expect("should exist");
        assert_eq!(loaded.chunks.len(), 1);
        assert!(loaded.chunks[0].claimed_by.is_none());
    }
}
