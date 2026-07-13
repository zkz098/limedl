use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, params, types::Value};

use super::manifest::{ChunkManifest, Manifest};
use super::types::{
    AdaptiveProfile, ChecksumMode, DownloadState, NetworkLearningMetrics, ThreadMode,
};

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
    chunk_size INTEGER NOT NULL DEFAULT 4194304
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

CREATE TABLE IF NOT EXISTS learning_metrics (
    scene_id TEXT NOT NULL PRIMARY KEY,
    estimated_bandwidth_bps REAL NOT NULL,
    stability_score REAL NOT NULL,
    penalty_rate REAL NOT NULL,
    recommended_initial_threads INTEGER NOT NULL,
    recommended_max_threads_per_task_cap INTEGER NOT NULL,
    sample_count INTEGER NOT NULL,
    last_observed_at_ms INTEGER NOT NULL
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

        // Remove any SFTP download entries whose IDs start with "sftp:"
        // after the SFTP protocol support was removed.
        let _ = conn.execute(
            "DELETE FROM downloads WHERE id LIKE 'sftp:%'",
            [],
        );

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
                chunk_size
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,
                      ?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,
                      ?22,?23,?24,?25,?26,?27,?28)",
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
                created_at_ms = ?25, updated_at_ms = ?26, chunk_size = ?27
             WHERE id = ?28",
            rusqlite::params_from_iter(params),
        )
        .with_context(|| format!("failed to update download {}", manifest.id))?;

        self.replace_chunks_inner(&conn, &manifest.id, &manifest.chunks)?;

        Ok(())
    }

    /// Efficient incremental update for the 300 ms persist cycle.
    ///
    /// Uses a single transaction so the download row and chunk set stay
    /// consistent.
    pub(crate) fn update_download_progress(
        &self,
        id: &str,
        downloaded_bytes: u64,
        chunks: &[ChunkManifest],
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

            conn.execute("DELETE FROM chunks WHERE download_id = ?1", params![id])
                .context("failed to delete old chunks")?;

            if !chunks.is_empty() {
                let mut stmt = conn
                    .prepare(
                        "INSERT INTO chunks (download_id, chunk_index, start_byte, end_byte,
                                 downloaded, completed, claimed_by)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    )
                    .context("failed to prepare chunk insert")?;

                for chunk in chunks {
                    stmt.execute(rusqlite::params_from_iter(chunk_to_params(id, chunk)))
                        .context("failed to insert chunk")?;
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

    // ── learning_metrics CRUD ─────────────────────────────────

    /// Insert or replace a scene's learning metrics.
    pub(crate) fn upsert_learning_metrics(
        &self,
        scene_id: &str,
        metrics: &NetworkLearningMetrics,
    ) -> Result<()> {
        let conn = self.lock_conn();
        conn.execute(
            "INSERT OR REPLACE INTO learning_metrics (
                scene_id, estimated_bandwidth_bps, stability_score, penalty_rate,
                recommended_initial_threads, recommended_max_threads_per_task_cap,
                sample_count, last_observed_at_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                scene_id,
                metrics.estimated_bandwidth_bps,
                metrics.stability_score,
                metrics.penalty_rate,
                metrics.recommended_initial_threads as i64,
                metrics.recommended_max_threads_per_task_cap as i64,
                metrics.sample_count as i64,
                metrics.last_observed_at_ms as i64,
            ],
        )
        .with_context(|| format!("failed to upsert learning metrics for {scene_id}"))?;
        Ok(())
    }

    /// Fetch learning metrics for a scene.
    #[allow(dead_code)]
    pub(crate) fn get_learning_metrics(
        &self,
        scene_id: &str,
    ) -> Result<Option<NetworkLearningMetrics>> {
        let conn = self.lock_conn();

        let mut stmt = conn
            .prepare("SELECT * FROM learning_metrics WHERE scene_id = ?1")
            .context("failed to prepare get_learning_metrics query")?;

        let opt = stmt
            .query_row(params![scene_id], |row| {
                Ok(NetworkLearningMetrics {
                    estimated_bandwidth_bps: row.get(1)?,
                    stability_score: row.get(2)?,
                    penalty_rate: row.get(3)?,
                    recommended_initial_threads: row.get::<_, i64>(4)? as usize,
                    recommended_max_threads_per_task_cap: row.get::<_, i64>(5)? as usize,
                    sample_count: row.get::<_, i64>(6)? as u32,
                    last_observed_at_ms: row.get::<_, i64>(7)? as u64,
                })
            })
            .optional()
            .context("failed to query learning metrics")?;

        Ok(opt)
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
