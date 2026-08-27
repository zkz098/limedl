use anyhow::{Context, Result};
use rusqlite::Connection;

/// Check whether a table has a given column by querying PRAGMA table_info.
pub(crate) fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    debug_assert!(
        table == "downloads",
        "table_has_column: unknown table '{table}' — add it to the whitelist"
    );
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

pub(crate) const CREATE_TABLES_SQL: &str = "
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

pub(crate) struct Migration {
    pub(crate) version: u32,
    pub(crate) name: &'static str,
    pub(crate) up: fn(&Connection) -> Result<()>,
}

pub(crate) const MIGRATIONS: &[Migration] = &[
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
    Migration {
        version: 6,
        name: "add_priority",
        up: |conn| {
            conn.execute(
                "ALTER TABLE downloads ADD COLUMN priority INTEGER NOT NULL DEFAULT 1",
                [],
            )
            .context("failed to add priority column")?;
            Ok(())
        },
    },
    Migration {
        version: 7,
        name: "add_cdn_accelerated",
        up: |conn| {
            conn.execute(
                "ALTER TABLE downloads ADD COLUMN cdn_accelerated INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("failed to add cdn_accelerated column")?;
            Ok(())
        },
    },
    Migration {
        version: 8,
        name: "add_cdn_node_ip",
        up: |conn| {
            conn.execute(
                "ALTER TABLE downloads ADD COLUMN cdn_node_ip TEXT",
                [],
            )
            .context("failed to add cdn_node_ip column")?;
            Ok(())
        },
    },
];
