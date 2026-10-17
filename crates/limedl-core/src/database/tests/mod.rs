use ntest::timeout;
use rusqlite::{Connection, params};
use std::sync::Arc;

use super::connection::{Database, is_lock_contention, is_unusable_database};
use super::schema::{CREATE_TABLES_SQL, Migration, add_column_if_missing, table_has_column};
use crate::error::DownloadError;
use crate::manifest::{CHUNK_SIZE, ChunkManifest, Manifest};
use crate::types::{
    AdaptiveProfile, ChecksumMode, DownloadState, Priority, ThreadMode, default_http_user_agent,
};

/// Helper: create a `Manifest` with sensible defaults for testing.
fn new_test_manifest(id: &str, url: &str, file_name: &str) -> Manifest {
    Manifest {
        id: id.to_string(),
        url: url.to_string(),
        final_url: url.to_string(),
        user_agent: default_http_user_agent(),
        extra_headers: vec![],
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
        cdn_node_ip: None,
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
        priority: Priority::Normal,
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

fn create_v1_schema(conn: &Connection) {
    conn.execute_batch(CREATE_TABLES_SQL).unwrap();
}

fn read_user_version(conn: &Connection) -> u32 {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn apply_v2(conn: &Connection) {
    conn.execute(
        "ALTER TABLE downloads ADD COLUMN chunk_size INTEGER NOT NULL DEFAULT 4194304",
        [],
    )
    .unwrap();
}

fn apply_v3(conn: &Connection) {
    conn.execute("ALTER TABLE downloads ADD COLUMN mirror_url TEXT", [])
        .unwrap();
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

mod bt_task_repo;
mod chunks;
mod concurrency;
mod connection;
mod downloads_crud;
mod schema_migrations;
