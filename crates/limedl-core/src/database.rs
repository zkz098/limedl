use std::path::Path;
use std::sync::Arc;

use foldhash::HashMap;
use parking_lot::Mutex;

use anyhow::{Context, Result};
use rusqlite::{named_params, Connection, params, types::Value};

use super::manifest::{ChunkManifest, Manifest};
use super::types::{AdaptiveProfile, ChecksumMode, DownloadState, ThreadMode};

#[cfg(test)]
use super::error::DownloadError;

type RusqliteResult<T> = std::result::Result<T, rusqlite::Error>;

// ── Database struct ──────────────────────────────────────────────

pub struct Database {
    write_conn: Arc<Mutex<Connection>>,
    read_conn: Arc<Mutex<Connection>>,
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

pub fn download_state_to_text(state: &DownloadState) -> &'static str {
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

// ── Manifest named-param inserts/updates ──────────────────────────
//
// Replaces the old manifest_to_row() which cloned ~20 String fields into
// a Vec<Value>.  Now uses rusqlite named_params! to pass &str references
// directly, avoiding per-field clones.

/// INSERT a full manifest row using named parameters.
fn insert_manifest_row(conn: &Connection, manifest: &Manifest) -> Result<()> {
    let mirror_urls_json =
        serde_json::to_string(&manifest.mirror_urls).unwrap_or_else(|_| "[]".into());

    let mut stmt = conn
        .prepare_cached(
            "INSERT INTO downloads (
                id, url, final_url, user_agent, destination_dir, file_name,
                file_name_locked, destination_path, temp_path, total_bytes,
                downloaded_bytes, supports_ranges, connection_count, thread_mode,
                requested_thread_count, desired_thread_count, allocated_thread_count,
                adaptive_profile_snapshot, thread_note, etag, last_modified,
                state, checksum_mode, checksum, error, created_at_ms, updated_at_ms,
                chunk_size, mirror_url, mirror_urls, current_mirror_index
            ) VALUES (
                :id, :url, :final_url, :user_agent, :destination_dir, :file_name,
                :file_name_locked, :destination_path, :temp_path, :total_bytes,
                :downloaded_bytes, :supports_ranges, :connection_count, :thread_mode,
                :requested_thread_count, :desired_thread_count, :allocated_thread_count,
                :adaptive_profile_snapshot, :thread_note, :etag, :last_modified,
                :state, :checksum_mode, :checksum, :error, :created_at_ms, :updated_at_ms,
                :chunk_size, :mirror_url, :mirror_urls, :current_mirror_index
            )",
        )
        .context("failed to prepare insert manifest")?;

    stmt.execute(named_params! {
        ":id": manifest.id.as_str(),
        ":url": manifest.url.as_str(),
        ":final_url": manifest.final_url.as_str(),
        ":user_agent": manifest.user_agent.as_str(),
        ":destination_dir": manifest.destination_dir.as_str(),
        ":file_name": manifest.file_name.as_str(),
        ":file_name_locked": manifest.file_name_locked,
        ":destination_path": manifest.destination_path.as_str(),
        ":temp_path": manifest.temp_path.as_str(),
        ":total_bytes": manifest.total_bytes.map(|v| v as i64),
        ":downloaded_bytes": manifest.downloaded_bytes as i64,
        ":supports_ranges": manifest.supports_ranges,
        ":connection_count": manifest.connection_count as i64,
        ":thread_mode": thread_mode_to_text(manifest.thread_mode),
        ":requested_thread_count": manifest.requested_thread_count.map(|v| v as i64),
        ":desired_thread_count": manifest.desired_thread_count.map(|v| v as i64),
        ":allocated_thread_count": manifest.allocated_thread_count.map(|v| v as i64),
        ":adaptive_profile_snapshot": manifest.adaptive_profile_snapshot.map(adaptive_profile_to_text),
        ":thread_note": manifest.thread_note.as_deref(),
        ":etag": manifest.etag.as_deref(),
        ":last_modified": manifest.last_modified.as_deref(),
        ":state": state_to_text(manifest.state),
        ":checksum_mode": checksum_mode_to_text(manifest.checksum_mode),
        ":checksum": manifest.checksum.as_deref(),
        ":error": manifest.error.as_deref(),
        ":created_at_ms": manifest.created_at_ms as i64,
        ":updated_at_ms": manifest.updated_at_ms as i64,
        ":chunk_size": manifest.chunk_size as i64,
        ":mirror_url": manifest.mirror_url.as_deref(),
        ":mirror_urls": mirror_urls_json.as_str(),
        ":current_mirror_index": manifest.current_mirror_index as i64,
    })
    .with_context(|| format!("failed to insert download {}", manifest.id))?;

    Ok(())
}

/// UPDATE a full manifest row using named parameters.
fn update_manifest_row(conn: &Connection, manifest: &Manifest) -> Result<()> {
    let mirror_urls_json =
        serde_json::to_string(&manifest.mirror_urls).unwrap_or_else(|_| "[]".into());

    let mut stmt = conn
        .prepare_cached(
            "UPDATE downloads SET
                url = :url, final_url = :final_url, user_agent = :user_agent,
                destination_dir = :destination_dir, file_name = :file_name,
                file_name_locked = :file_name_locked,
                destination_path = :destination_path, temp_path = :temp_path,
                total_bytes = :total_bytes,
                downloaded_bytes = :downloaded_bytes,
                supports_ranges = :supports_ranges,
                connection_count = :connection_count, thread_mode = :thread_mode,
                requested_thread_count = :requested_thread_count,
                desired_thread_count = :desired_thread_count,
                allocated_thread_count = :allocated_thread_count,
                adaptive_profile_snapshot = :adaptive_profile_snapshot,
                thread_note = :thread_note, etag = :etag,
                last_modified = :last_modified,
                state = :state, checksum_mode = :checksum_mode,
                checksum = :checksum, error = :error,
                created_at_ms = :created_at_ms,
                updated_at_ms = :updated_at_ms, chunk_size = :chunk_size,
                mirror_url = :mirror_url, mirror_urls = :mirror_urls,
                current_mirror_index = :current_mirror_index
             WHERE id = :id",
        )
        .context("failed to prepare update manifest")?;

    stmt.execute(named_params! {
        ":id": manifest.id.as_str(),
        ":url": manifest.url.as_str(),
        ":final_url": manifest.final_url.as_str(),
        ":user_agent": manifest.user_agent.as_str(),
        ":destination_dir": manifest.destination_dir.as_str(),
        ":file_name": manifest.file_name.as_str(),
        ":file_name_locked": manifest.file_name_locked,
        ":destination_path": manifest.destination_path.as_str(),
        ":temp_path": manifest.temp_path.as_str(),
        ":total_bytes": manifest.total_bytes.map(|v| v as i64),
        ":downloaded_bytes": manifest.downloaded_bytes as i64,
        ":supports_ranges": manifest.supports_ranges,
        ":connection_count": manifest.connection_count as i64,
        ":thread_mode": thread_mode_to_text(manifest.thread_mode),
        ":requested_thread_count": manifest.requested_thread_count.map(|v| v as i64),
        ":desired_thread_count": manifest.desired_thread_count.map(|v| v as i64),
        ":allocated_thread_count": manifest.allocated_thread_count.map(|v| v as i64),
        ":adaptive_profile_snapshot": manifest.adaptive_profile_snapshot.map(adaptive_profile_to_text),
        ":thread_note": manifest.thread_note.as_deref(),
        ":etag": manifest.etag.as_deref(),
        ":last_modified": manifest.last_modified.as_deref(),
        ":state": state_to_text(manifest.state),
        ":checksum_mode": checksum_mode_to_text(manifest.checksum_mode),
        ":checksum": manifest.checksum.as_deref(),
        ":error": manifest.error.as_deref(),
        ":created_at_ms": manifest.created_at_ms as i64,
        ":updated_at_ms": manifest.updated_at_ms as i64,
        ":chunk_size": manifest.chunk_size as i64,
        ":mirror_url": manifest.mirror_url.as_deref(),
        ":mirror_urls": mirror_urls_json.as_str(),
        ":current_mirror_index": manifest.current_mirror_index as i64,
    })
    .with_context(|| format!("failed to update download {}", manifest.id))?;

    Ok(())
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
        expected_checksum: None,
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

// ── Compatibility helpers ────────────────────────────────────────

/// Check whether a table has a given column by querying PRAGMA table_info.
fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    // SAFETY: `table` is always a hardcoded literal ("downloads"), never user input.
    // If this function is ever made generic over table names, the format! must be
    // replaced with a parameterized query or a whitelist check.
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .context("failed to query table info")?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .context("failed to map column names")?
        .filter_map(|r| r.ok())
        .any(|name| name == column);
    Ok(exists)
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
    updated_at_ms INTEGER NOT NULL
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

// ── Schema migrations ────────────────────────────────────────────

struct Migration {
    version: u32,
    name: &'static str,
    up: fn(&Connection) -> Result<()>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        up: |conn| {
            conn.execute_batch(CREATE_TABLES_SQL)
                .context("failed to create initial schema")?;
            Ok(())
        },
    },
    Migration {
        version: 2,
        name: "add_chunk_size",
        up: |conn| {
            conn.execute(
                "ALTER TABLE downloads ADD COLUMN chunk_size INTEGER NOT NULL DEFAULT 4194304",
                [],
            )
            .context("failed to add chunk_size column")?;
            Ok(())
        },
    },
    Migration {
        version: 3,
        name: "add_mirror_columns",
        up: |conn| {
            conn.execute("ALTER TABLE downloads ADD COLUMN mirror_url TEXT", [])
                .context("failed to add mirror_url column")?;
            conn.execute(
                "ALTER TABLE downloads ADD COLUMN mirror_urls TEXT NOT NULL DEFAULT '[]'",
                [],
            )
            .context("failed to add mirror_urls column")?;
            conn.execute(
                "ALTER TABLE downloads ADD COLUMN current_mirror_index INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("failed to add current_mirror_index column")?;
            Ok(())
        },
    },
    Migration {
        version: 4,
        name: "cleanup_sftp_tasks",
        up: |conn| {
            // One-time cleanup: remove SFTP download entries after protocol removal.
            conn.execute("DELETE FROM downloads WHERE id LIKE 'sftp:%'", [])
                .context("failed to clean up SFTP tasks")?;
            Ok(())
        },
    },
    Migration {
        version: 5,
        name: "add_business_indexes",
        up: |conn| {
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_downloads_state ON downloads(state)",
                [],
            )
            .context("failed to create idx_downloads_state")?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_downloads_created ON downloads(created_at_ms DESC)",
                [],
            )
            .context("failed to create idx_downloads_created")?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_chunks_claimed ON chunks(claimed_by) WHERE claimed_by IS NOT NULL",
                [],
            ).context("failed to create idx_chunks_claimed")?;
            Ok(())
        },
    },
];

// ── Database impl ────────────────────────────────────────────────

impl Database {
    /// Open (or create) the SQLite database at `path`.
    ///
    /// Enables WAL mode, foreign keys, and performance PRAGMAs,
    /// then runs schema migrations.
    pub fn open(path: &Path) -> Result<Self> {
        let write_conn = Connection::open(path)
            .with_context(|| format!("failed to open database at {}", path.display()))?;

        // ── PRAGMA configuration ─────────────────────────────────
        write_conn.execute_batch("PRAGMA journal_mode = WAL;")
            .context("failed to enable WAL mode")?;
        write_conn.execute_batch("PRAGMA wal_autocheckpoint = 4096;")
            .context("failed to set WAL auto-checkpoint")?;
        write_conn.execute_batch("PRAGMA foreign_keys = ON;")
            .context("failed to enable foreign keys")?;
        write_conn.execute_batch("PRAGMA busy_timeout = 5000;")
            .context("failed to set busy timeout")?;
        write_conn.execute_batch("PRAGMA synchronous = NORMAL;")
            .context("failed to set synchronous mode")?;
        write_conn.execute_batch("PRAGMA cache_size = -8000;")
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
            write_conn.pragma_update(None, "user_version", migration.version)
                .with_context(|| {
                    format!("failed to update schema version to {}", migration.version)
                })?;
        }

        // ── Read connection (WAL-enabled, read-only) ────────────
        let read_conn = Connection::open(path)
            .with_context(|| format!("failed to open read database at {}", path.display()))?;
        read_conn.execute_batch("PRAGMA query_only = 1;")
            .context("failed to set query_only on read connection")?;
        read_conn.execute_batch("PRAGMA busy_timeout = 5000;")
            .context("failed to set busy timeout on read connection")?;

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

    fn lock_write(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.write_conn.lock()
    }

    fn lock_read(&self) -> parking_lot::MutexGuard<'_, Connection> {
        self.read_conn.lock()
    }

    // ── download CRUD ────────────────────────────────────────

    /// Insert a new download (or update if the id already exists).
    ///
    /// For new downloads (first insert), all chunks are written via
    /// [`replace_chunks_inner`].  For existing downloads, the 31-column
    /// download row is updated via `UPDATE` (avoiding FK CASCADE on chunks),
    /// and only **dirty** chunks are upserted — the 300 ms persist cycle
    /// already keeps non-dirty chunks up to date, so re-writing them all
    /// on every state transition would be wasteful I/O.
    pub fn insert_download(&self, manifest: &Manifest) -> Result<()> {
        let conn = self.lock_write();

        // ── Is this a new download or an update to an existing one? ──
        let is_new: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM downloads WHERE id = ?1",
                params![manifest.id],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count == 0)
            .unwrap_or(true); // query failure → safe to treat as new

        if is_new {
            // ── New download: INSERT + full chunk write ────────────
            insert_manifest_row(&conn, manifest)?;

            if !manifest.chunks.is_empty() {
                self.replace_chunks_inner(&conn, &manifest.id, &manifest.chunks)?;
            }
        } else {
            // ── Existing download: UPDATE (avoids FK CASCADE on chunks)
            //    + incremental dirty chunk upsert, atomic in a single
            //    transaction ────────────────────────────────────────────
            conn.execute_batch("BEGIN IMMEDIATE")
                .context("failed to begin transaction")?;

            let result = (|| -> Result<()> {
                update_manifest_row(&conn, manifest)?;

                // Only upsert chunks whose dirty flag is set.
                if !manifest.chunks.is_empty() {
                    let dirty_chunks: Vec<&ChunkManifest> =
                        manifest.chunks.iter().filter(|c| c.dirty).collect();
                    if !dirty_chunks.is_empty() {
                        let mut stmt = conn
                            .prepare(
                                "INSERT OR REPLACE INTO chunks (download_id, chunk_index, start_byte, end_byte,
                                         downloaded, completed, claimed_by)
                                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            )
                            .context("failed to prepare chunk upsert")?;
                        for chunk in dirty_chunks {
                            stmt.execute(rusqlite::params_from_iter(chunk_to_params(
                                &manifest.id,
                                chunk,
                            )))
                            .context("failed to upsert chunk")?;
                        }
                    }
                }

                Ok(())
            })();

            match result {
                Ok(()) => {
                    conn.execute_batch("COMMIT")
                        .context("failed to commit transaction")?;
                }
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Full persist escape hatch — writes all fields and replaces all chunks.
    ///
    /// Unlike [`insert_download`] (which does incremental dirty-chunk upserts for
    /// existing downloads), this method unconditionally deletes and re-inserts
    /// every chunk.  Use sparingly — only when a complete chunk replacement is
    /// required (e.g. the test helper).
    #[allow(dead_code)]
    pub fn update_download(&self, manifest: &Manifest) -> Result<()> {
        let conn = self.lock_write();

        update_manifest_row(&conn, manifest)?;

        self.replace_chunks_inner(&conn, &manifest.id, &manifest.chunks)?;

        Ok(())
    }

    /// Incremental update for the 300 ms persist cycle.
    ///
    /// Only writes chunks that have changed (dirty flag) since the last persist.
    /// Uses `INSERT OR REPLACE` so each chunk is upserted individually without
    /// a full DELETE + INSERT of the entire chunk set.
    /// All operations run within a single transaction for consistency.
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
    pub fn delete_download(&self, id: &str) -> Result<()> {
        let conn = self.lock_write();
        conn.execute("DELETE FROM downloads WHERE id = ?1", params![id])
            .with_context(|| format!("failed to delete download {id}"))?;
        Ok(())
    }

    /// Fetch a single download with its chunks.
    #[allow(dead_code)]
    pub fn get_download(&self, id: &str) -> Result<Option<Manifest>> {
        let conn = self.lock_read();

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
    #[allow(dead_code)]
    pub fn list_downloads(&self) -> Result<Vec<Manifest>> {
        let conn = self.lock_read();

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

        // Build a lookup map: download_id → Vec<ChunkManifest>
        let mut chunk_map: HashMap<String, Vec<ChunkManifest>> = HashMap::default();
        for (download_id, chunk) in all_chunks {
            chunk_map.entry(download_id).or_default().push(chunk);
        }

        // Assign chunks to manifests in O(1) per manifest
        for manifest in &mut manifests {
            if let Some(chunks) = chunk_map.remove(&manifest.id) {
                manifest.chunks = chunks;
            }
        }

        Ok(manifests)
    }

    /// Return all download manifests WITHOUT chunks populated.
    ///
    /// Used at startup for fast loading; chunks are loaded on-demand via
    /// [`load_chunks`] for non-terminal downloads only.
    pub fn list_download_headers(&self) -> Result<Vec<Manifest>> {
        let conn = self.lock_read();

        let mut stmt = conn
            .prepare("SELECT * FROM downloads ORDER BY created_at_ms DESC")
            .context("failed to prepare list_download_headers query")?;

        let manifests: Vec<Manifest> = stmt
            .query_map([], row_to_manifest)
            .context("failed to map download rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect downloads")?;

        Ok(manifests)
    }

    /// Load chunks for a single download on demand (lazy loading).
    pub fn load_chunks(&self, download_id: &str) -> Result<Vec<ChunkManifest>> {
        let conn = self.lock_read();
        self.fetch_chunks_inner(&conn, download_id)
    }

    /// Total number of downloads.
    #[allow(dead_code)]
    pub fn count_downloads(&self) -> Result<usize> {
        let conn = self.lock_read();
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

    use super::super::manifest::CHUNK_SIZE;
    use super::super::types::default_http_user_agent;
    use super::*;

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
            expected_checksum: None,
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
        let conn = db.lock_read();
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
        let loaded = db
            .get_download("all-fields")
            .unwrap()
            .expect("should exist");

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
    fn insert_existing_incremental_chunks() {
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

        // Update metadata + upsert only the dirty chunk (index 0).
        // Chunk 1 is kept from the original insert (non-dirty, unchanged).
        let mut replacement = new_test_manifest("dup", "https://b.com/replaced", "replaced.txt");
        replacement.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 199,
                downloaded: 100,
                completed: false,
                claimed_by: None,
                dirty: true, // only this chunk gets upserted
            },
            // chunk 1 is absent from the manifest — it stays in the DB from
            // the original insert because we do not DELETE all chunks.
        ];
        db.insert_download(&replacement).unwrap();

        let loaded = db.get_download("dup").unwrap().expect("should exist");
        assert_eq!(loaded.url, "https://b.com/replaced");
        assert_eq!(loaded.file_name, "replaced.txt");
        // Both original chunks remain; only chunk 0 was upserted with new values.
        assert_eq!(loaded.chunks.len(), 2);
        assert_eq!(loaded.chunks[0].downloaded, 100);
        assert_eq!(loaded.chunks[0].end, 199);
        // Chunk 1 is unchanged from the original insert.
        assert_eq!(loaded.chunks[1].downloaded, 0);
        assert_eq!(loaded.chunks[1].end, 999);
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

        let loaded = db
            .get_download("empty-chunks")
            .unwrap()
            .expect("should exist");
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
        let loaded = db
            .get_download("claim-none")
            .unwrap()
            .expect("should exist");
        assert_eq!(loaded.chunks.len(), 1);
        assert!(loaded.chunks[0].claimed_by.is_none());
    }

    // ── Follow-up 2: new tests ───────────────────────────────────

    #[timeout(30_000)]
    #[test]
    fn list_download_headers_returns_no_chunks() {
        let db = Database::open_in_memory().unwrap();

        let mut m = new_test_manifest("hdr", "https://example.com/hdr", "header.bin");
        m.chunks = vec![
            ChunkManifest {
                index: 0,
                start: 0,
                end: 99,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 1,
                start: 100,
                end: 199,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
            ChunkManifest {
                index: 2,
                start: 200,
                end: 299,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            },
        ];
        db.insert_download(&m).unwrap();

        let headers = db.list_download_headers().unwrap();
        assert_eq!(headers.len(), 1);
        assert!(
            headers[0].chunks.is_empty(),
            "chunks should not be populated"
        );
    }

    #[timeout(30_000)]
    #[test]
    fn load_chunks_returns_chunks_on_demand() {
        let db = Database::open_in_memory().unwrap();

        let mut m = new_test_manifest("chk-id", "https://example.com/chk", "chunk.bin");
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

        let chunks = db.load_chunks("chk-id").unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].index, 0);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 499);
        assert_eq!(chunks[1].index, 1);
        assert_eq!(chunks[1].start, 500);
        assert_eq!(chunks[1].end, 999);
    }

    #[timeout(30_000)]
    #[test]
    fn load_chunks_nonexistent_returns_empty() {
        let db = Database::open_in_memory().unwrap();
        let chunks = db.load_chunks("no-such-download").unwrap();
        assert!(chunks.is_empty());
    }

    #[timeout(30_000)]
    #[test]
    fn table_has_column_detects_existing_column() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.lock_read();

        // Verify the backfilled columns exist after migration
        let mut stmt = conn.prepare("PRAGMA table_info(downloads)").unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(
            columns.contains(&"chunk_size".to_string()),
            "chunk_size column should exist"
        );
        assert!(
            columns.contains(&"mirror_urls".to_string()),
            "mirror_urls column should exist"
        );
        assert!(
            columns.contains(&"mirror_url".to_string()),
            "mirror_url column should exist"
        );
        assert!(
            columns.contains(&"current_mirror_index".to_string()),
            "current_mirror_index column should exist"
        );
    }

    // ── Migration compatibility (dirty-state) tests ───────────────
    //
    // These tests simulate databases created by old code that never set
    // `user_version` but had columns backfilled by the "let _ = ..." pattern.
    // `Database::open()` must detect these columns and run the correct
    // remaining migrations.

    /// Helper: create a raw v1 SQLite schema at `path` (downloads + chunks tables).
    fn create_v1_schema(conn: &Connection) {
        conn.execute_batch(super::CREATE_TABLES_SQL).unwrap();
    }

    /// Helper: read `user_version` PRAGMA from an open connection.
    fn read_user_version(conn: &Connection) -> u32 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    /// Helper: apply migration v2 (add chunk_size column).
    fn apply_v2(conn: &Connection) {
        conn.execute("ALTER TABLE downloads ADD COLUMN chunk_size INTEGER NOT NULL DEFAULT 4194304", [])
            .unwrap();
    }

    /// Helper: apply migration v3 (add mirror columns).
    fn apply_v3(conn: &Connection) {
        conn.execute("ALTER TABLE downloads ADD COLUMN mirror_url TEXT", []).unwrap();
        conn.execute(
            "ALTER TABLE downloads ADD COLUMN mirror_urls TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .unwrap();
        conn.execute(
            "ALTER TABLE downloads ADD COLUMN current_mirror_index INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .unwrap();
    }

    #[timeout(30_000)]
    #[test]
    fn migration_compat_v0_with_chunk_size_backfilled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // ── Simulate an old v0 database with chunk_size column backfilled ──
        {
            let conn = Connection::open(&path).unwrap();
            create_v1_schema(&conn);
            apply_v2(&conn);
            // user_version is 0 (fresh database, never set)
            assert_eq!(read_user_version(&conn), 0);
        } // raw connection dropped — file is closed

        // ── Open via Database::open() — compat should detect chunk_size ──
        let db = Database::open(&path).unwrap();

        // Verify all migrations ran: final version should be 5 (latest)
        {
            let conn = db.lock_write();
            assert_eq!(
                read_user_version(&conn),
                5,
                "expected user_version = 5 after migration"
            );
            let has_mirror_urls =
                table_has_column(&conn, "downloads", "mirror_urls").unwrap();
            assert!(has_mirror_urls, "mirror_urls column should exist after migration");
        }

        // Verify the database is fully functional
        let mut m = new_test_manifest("compat-v0-a", "https://example.com/a", "a.bin");
        m.chunk_size = 4194304; // v2 column
        db.insert_download(&m).unwrap();
        let loaded = db.get_download("compat-v0-a").unwrap().expect("should exist");
        assert_eq!(loaded.id, "compat-v0-a");
        assert_eq!(loaded.chunk_size, 4194304);
        assert_eq!(db.count_downloads().unwrap(), 1);
        assert!(loaded.mirror_urls.is_empty(), "mirror_urls should be empty");
    }

    #[timeout(30_000)]
    #[test]
    fn migration_compat_v1_with_mirror_columns_backfilled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // ── Simulate a v1 database with both chunk_size AND mirror columns
        //    already backfilled by old code (user_version stuck at 1) ──
        {
            let conn = Connection::open(&path).unwrap();
            create_v1_schema(&conn);
            apply_v2(&conn);
            apply_v3(&conn);
            conn.pragma_update(None, "user_version", 1).unwrap();
            assert_eq!(read_user_version(&conn), 1);
        }

        // ── Open — compat should bump version from 1 → 2 → 3 ──
        let db = Database::open(&path).unwrap();

        {
            let conn = db.lock_write();
            assert_eq!(
                read_user_version(&conn),
                5,
                "expected user_version = 5 after migration"
            );
        }

        // Functional check
        let mut m = new_test_manifest("compat-v1-b", "https://example.com/b", "b.bin");
        m.chunk_size = 4194304;
        m.mirror_urls = vec!["https://mirror.example.com/b".into()];
        m.current_mirror_index = 0;
        db.insert_download(&m).unwrap();
        let loaded = db.get_download("compat-v1-b").unwrap().expect("should exist");
        assert_eq!(loaded.chunk_size, 4194304);
        assert_eq!(loaded.mirror_urls.len(), 1);
        assert_eq!(db.count_downloads().unwrap(), 1);
    }

    #[timeout(30_000)]
    #[test]
    fn migration_compat_v0_fully_backfilled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // ── Simulate a fully backfilled pre-v4 schema with user_version = 0 ──
        {
            let conn = Connection::open(&path).unwrap();
            create_v1_schema(&conn);
            apply_v2(&conn);
            apply_v3(&conn);
            // Leave user_version at 0 (old code never set it)
            assert_eq!(read_user_version(&conn), 0);
        }

        // ── Open — compat should lift 0 → 2 → 3, then run v4 + v5 ──
        let db = Database::open(&path).unwrap();

        {
            let conn = db.lock_write();
            assert_eq!(
                read_user_version(&conn),
                5,
                "expected user_version = 5 after migration"
            );
        }

        // Functional check: insert a download and verify all columns round-trip
        let mut m = new_test_manifest("compat-v0-c", "https://example.com/c", "c.bin");
        m.chunk_size = 2097152; // v2 column, non-default value
        m.mirror_url = Some("https://mirror.example.com/c".into());
        m.mirror_urls = vec![
            "https://mirror1.example.com/c".into(),
            "https://mirror2.example.com/c".into(),
        ];
        m.current_mirror_index = 1;
        db.insert_download(&m).unwrap();
        let loaded = db.get_download("compat-v0-c").unwrap().expect("should exist");
        assert_eq!(loaded.chunk_size, 2097152);
        assert_eq!(loaded.mirror_url.as_deref(), Some("https://mirror.example.com/c"));
        assert_eq!(loaded.mirror_urls.len(), 2);
        assert_eq!(loaded.current_mirror_index, 1);
        assert_eq!(db.count_downloads().unwrap(), 1);
    }

    #[test]
    fn empty_migrations_returns_err() {
        let empty: &[Migration] = &[];
        let result = empty
            .last()
            .map(|m| m.version)
            .ok_or_else(|| DownloadError::DatabaseInit("no migrations defined".into()));
        assert!(result.is_err(), "expected Err for empty migrations slice");
        let err = result.unwrap_err();
        assert_eq!(
            err.kind(),
            "database_init",
            "unexpected error kind: {}",
            err.kind()
        );
        assert!(
            err.to_string().contains("database initialization error"),
            "unexpected error message: {}",
            err
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // File persistence tests — verify data survives close/reopen
    // using production `Database::open(path)` with temp directories.
    // ═══════════════════════════════════════════════════════════════

    #[timeout(30_000)]
    #[test]
    fn open_creates_new_database_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        assert!(!path.exists(), "database file should not exist before open");

        let db = Database::open(&path).unwrap();
        drop(db); // close the database

        assert!(
            path.exists(),
            "database file should exist after Database::open"
        );
    }

    #[timeout(30_000)]
    #[test]
    fn insert_and_reopen_preserves_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // First session: insert a download
        {
            let db = Database::open(&path).unwrap();
            let manifest =
                new_test_manifest("persist-1", "https://example.com/file", "file.bin");
            db.insert_download(&manifest).unwrap();
            assert_eq!(db.count_downloads().unwrap(), 1);
        } // db dropped, connections closed

        // Second session: reopen and verify data survived
        {
            let db = Database::open(&path).unwrap();
            assert_eq!(db.count_downloads().unwrap(), 1);
            let loaded = db
                .get_download("persist-1")
                .unwrap()
                .expect("should exist after reopen");
            assert_eq!(loaded.id, "persist-1");
            assert_eq!(loaded.url, "https://example.com/file");
            assert_eq!(loaded.file_name, "file.bin");
            assert_eq!(loaded.state, DownloadState::Queued);
        }
    }

    #[timeout(30_000)]
    #[test]
    fn update_persists_across_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // First session: insert then update
        {
            let db = Database::open(&path).unwrap();
            let mut manifest =
                new_test_manifest("upd-reopen", "https://example.com/old", "old.txt");
            manifest.state = DownloadState::Queued;
            manifest.chunks = vec![ChunkManifest {
                index: 0,
                start: 0,
                end: 499,
                downloaded: 0,
                completed: false,
                claimed_by: None,
                dirty: false,
            }];
            db.insert_download(&manifest).unwrap();

            let mut updated =
                new_test_manifest("upd-reopen", "https://example.com/new", "new.txt");
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
        } // db dropped

        // Second session: verify the update persisted
        {
            let db = Database::open(&path).unwrap();
            let loaded = db
                .get_download("upd-reopen")
                .unwrap()
                .expect("should exist after reopen");
            assert_eq!(loaded.url, "https://example.com/new");
            assert_eq!(loaded.file_name, "new.txt");
            assert_eq!(loaded.state, DownloadState::Completed);
            assert_eq!(loaded.downloaded_bytes, 500);
            assert_eq!(loaded.updated_at_ms, 9999);
            assert_eq!(loaded.chunks.len(), 1);
            assert!(loaded.chunks[0].completed);
            assert_eq!(loaded.chunks[0].downloaded, 500);
        }
    }

    #[timeout(30_000)]
    #[test]
    fn delete_persists_across_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // First session: insert then delete
        {
            let db = Database::open(&path).unwrap();
            let manifest =
                new_test_manifest("del-reopen", "https://example.com/del", "del.bin");
            db.insert_download(&manifest).unwrap();
            assert_eq!(db.count_downloads().unwrap(), 1);

            db.delete_download("del-reopen").unwrap();
            assert_eq!(db.count_downloads().unwrap(), 0);
        } // db dropped

        // Second session: verify deletion persisted
        {
            let db = Database::open(&path).unwrap();
            assert_eq!(db.count_downloads().unwrap(), 0);
            let loaded = db.get_download("del-reopen").unwrap();
            assert!(
                loaded.is_none(),
                "deleted download should not exist after reopening"
            );
        }
    }

    #[timeout(30_000)]
    #[test]
    fn chunks_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");

        // First session: insert a download with multiple chunks
        {
            let db = Database::open(&path).unwrap();
            let mut manifest =
                new_test_manifest("chunks-reopen", "https://example.com/chunks", "chunks.bin");
            manifest.chunks = vec![
                ChunkManifest {
                    index: 0,
                    start: 0,
                    end: 511,
                    downloaded: 256,
                    completed: false,
                    claimed_by: Some(1),
                    dirty: false,
                },
                ChunkManifest {
                    index: 1,
                    start: 512,
                    end: 1023,
                    downloaded: 512,
                    completed: true,
                    claimed_by: None,
                    dirty: false,
                },
            ];
            db.insert_download(&manifest).unwrap();
            assert_eq!(count_chunks(&db, "chunks-reopen"), 2);
        } // db dropped

        // Second session: verify that chunks survived
        {
            let db = Database::open(&path).unwrap();
            let loaded = db
                .get_download("chunks-reopen")
                .unwrap()
                .expect("should exist after reopen");
            assert_eq!(loaded.chunks.len(), 2);
            // Chunk 0
            assert_eq!(loaded.chunks[0].index, 0);
            assert_eq!(loaded.chunks[0].start, 0);
            assert_eq!(loaded.chunks[0].end, 511);
            assert_eq!(loaded.chunks[0].downloaded, 256);
            assert!(!loaded.chunks[0].completed);
            assert_eq!(loaded.chunks[0].claimed_by, Some(1));
            // Chunk 1
            assert_eq!(loaded.chunks[1].index, 1);
            assert_eq!(loaded.chunks[1].start, 512);
            assert_eq!(loaded.chunks[1].end, 1023);
            assert_eq!(loaded.chunks[1].downloaded, 512);
            assert!(loaded.chunks[1].completed);
            assert!(loaded.chunks[1].claimed_by.is_none());
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Concurrent access tests — verify the Database can handle
    // simultaneous reads/writes from multiple threads.
    // Uses the production `Database::open(path)` with temp dirs.
    // ═══════════════════════════════════════════════════════════════

    #[timeout(30_000)]
    #[test]
    fn concurrent_insert_and_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Arc::new(Database::open(&path).unwrap());

        let db_reader = db.clone();
        let reader = std::thread::spawn(move || {
            // Reader queries while the writer hasn't inserted yet
            for i in 0..100 {
                let _ = db_reader.get_download("concurrent-1");
                let _ = db_reader.count_downloads();
                if i % 10 == 0 {
                    std::thread::yield_now();
                }
            }
            // Exercise the read path one more time before the thread exits.
            // Existence is verified post-join below.
            let _ = db_reader.get_download("concurrent-1");
        });

        let db_writer = db.clone();
        let writer = std::thread::spawn(move || {
            let manifest =
                new_test_manifest("concurrent-1", "https://example.com/con", "con.bin");
            db_writer.insert_download(&manifest).unwrap();
        });

        reader.join().expect("reader thread panicked");
        writer.join().expect("writer thread panicked");

        let loaded = db
            .get_download("concurrent-1")
            .unwrap()
            .expect("should exist after concurrent access");
        assert_eq!(loaded.url, "https://example.com/con");
        assert_eq!(db.count_downloads().unwrap(), 1);
    }

    #[timeout(30_000)]
    #[test]
    fn concurrent_multiple_writes_different_ids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Arc::new(Database::open(&path).unwrap());

        let mut handles = Vec::new();
        for i in 0..5 {
            let db_clone = db.clone();
            let id = format!("con-write-{i}");
            handles.push(std::thread::spawn(move || {
                let manifest = new_test_manifest(
                    &id,
                    &format!("https://example.com/file{i}"),
                    &format!("file{i}.bin"),
                );
                db_clone.insert_download(&manifest).unwrap();
            }));
        }

        for handle in handles {
            handle.join().expect("writer thread panicked");
        }

        assert_eq!(db.count_downloads().unwrap(), 5);
        for i in 0..5 {
            let id = format!("con-write-{i}");
            let loaded = db
                .get_download(&id)
                .unwrap()
                .unwrap_or_else(|| panic!("download {id} should exist"));
            assert_eq!(loaded.id, id);
            assert_eq!(loaded.file_name, format!("file{i}.bin"));
            assert_eq!(loaded.url, format!("https://example.com/file{i}"));
        }
    }

    #[timeout(30_000)]
    #[test]
    fn concurrent_read_write_same_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Arc::new(Database::open(&path).unwrap());

        // Pre-insert the download so both reader and writer operate on existing data
        {
            let initial =
                new_test_manifest("same-id", "https://example.com/initial", "initial.bin");
            db.insert_download(&initial).unwrap();
        }

        let db_writer = db.clone();
        let writer = std::thread::spawn(move || {
            for i in 0..15 {
                let mut manifest =
                    new_test_manifest("same-id", "https://example.com/updated", "updated.bin");
                manifest.downloaded_bytes = (i as u64 + 1) * 100;
                manifest.updated_at_ms = i as u64;
                manifest.state = DownloadState::Downloading;
                db_writer.update_download(&manifest).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(3));
            }
        });

        let db_reader = db.clone();
        let reader = std::thread::spawn(move || {
            for _ in 0..30 {
                let result = db_reader.get_download("same-id");
                assert!(
                    result.is_ok(),
                    "reader should not encounter DB errors during concurrent access"
                );
                if let Ok(Some(manifest)) = result {
                    assert_eq!(manifest.id, "same-id");
                    // downloaded_bytes should be a valid (non-corrupted) state
                    assert!(
                        manifest.downloaded_bytes <= 1500,
                        "downloaded_bytes should be bounded: got {}",
                        manifest.downloaded_bytes
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });

        writer.join().expect("writer thread panicked");
        reader.join().expect("reader thread panicked");

        // Verify final state
        let loaded = db
            .get_download("same-id")
            .unwrap()
            .expect("should exist after concurrent rw");
        assert_eq!(loaded.downloaded_bytes, 1500);
        assert_eq!(loaded.updated_at_ms, 14);
        assert_eq!(loaded.state, DownloadState::Downloading);
    }

    #[timeout(30_000)]
    #[test]
    fn concurrent_load_chunks_while_saving() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let db = Arc::new(Database::open(&path).unwrap());

        // Pre-insert with initial chunks
        {
            let mut manifest = new_test_manifest(
                "chunks-con",
                "https://example.com/cc",
                "cc.bin",
            );
            manifest.chunks = vec![
                ChunkManifest {
                    index: 0,
                    start: 0,
                    end: 500,
                    downloaded: 0,
                    completed: false,
                    claimed_by: None,
                    dirty: false,
                },
                ChunkManifest {
                    index: 1,
                    start: 500,
                    end: 1000,
                    downloaded: 0,
                    completed: false,
                    claimed_by: None,
                    dirty: false,
                },
            ];
            db.insert_download(&manifest).unwrap();
        }

        let db_writer = db.clone();
        let writer = std::thread::spawn(move || {
            for i in 0..20 {
                let progress = (i as u64 + 1) * 50;
                let mut manifest = new_test_manifest(
                    "chunks-con",
                    "https://example.com/cc",
                    "cc.bin",
                );
                manifest.downloaded_bytes = progress * 2;
                manifest.chunks = vec![
                    ChunkManifest {
                        index: 0,
                        start: 0,
                        end: 500,
                        downloaded: progress.min(500),
                        completed: progress >= 500,
                        claimed_by: None,
                        dirty: false,
                    },
                    ChunkManifest {
                        index: 1,
                        start: 500,
                        end: 1000,
                        downloaded: progress.saturating_sub(500),
                        completed: progress >= 1000,
                        claimed_by: None,
                        dirty: false,
                    },
                ];
                db_writer.update_download(&manifest).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });

        let db_reader = db.clone();
        let reader = std::thread::spawn(move || {
            for _ in 0..20 {
                // Read full download (manifest + chunks)
                let result = db_reader.get_download("chunks-con");
                assert!(
                    result.is_ok(),
                    "get_download should not error during concurrent chunk writes"
                );
                if let Ok(Some(manifest)) = &result {
                    for chunk in &manifest.chunks {
                        assert!(
                            chunk.downloaded <= chunk.end - chunk.start,
                            "chunk {} download {} exceeds range {}-{}",
                            chunk.index,
                            chunk.downloaded,
                            chunk.start,
                            chunk.end
                        );
                    }
                }
                // Load chunks directly
                let chunks = db_reader.load_chunks("chunks-con");
                assert!(
                    chunks.is_ok(),
                    "load_chunks should not error during concurrent chunk writes"
                );
                if let Ok(chunks) = &chunks {
                    for chunk in chunks {
                        assert!(
                            chunk.downloaded <= chunk.end - chunk.start,
                            "chunk {} download {} exceeds range {}-{}",
                            chunk.index,
                            chunk.downloaded,
                            chunk.start,
                            chunk.end
                        );
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        });

        writer.join().expect("writer thread panicked");
        reader.join().expect("reader thread panicked");

        // Verify final state
        let loaded = db
            .get_download("chunks-con")
            .unwrap()
            .expect("should exist after concurrent chunk access");
        assert_eq!(loaded.downloaded_bytes, 2000);
        assert_eq!(loaded.chunks.len(), 2);
        assert!(loaded.chunks[0].completed);
        assert_eq!(loaded.chunks[0].downloaded, 500);
        assert!(loaded.chunks[1].completed);
        assert_eq!(loaded.chunks[1].downloaded, 500);
    }
}
