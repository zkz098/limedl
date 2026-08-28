#[cfg(test)]
use foldhash::HashMap;

use anyhow::{Context, Result};
use rusqlite::{named_params, params, Connection};

use super::chunk_repo::chunk_to_params;
#[cfg(test)]
use super::chunk_repo::row_to_chunk;
use super::connection::Database;
use crate::manifest::{ChunkManifest, Manifest};
use crate::types::{AdaptiveProfile, ChecksumMode, DownloadState, ThreadMode};

type RusqliteResult<T> = std::result::Result<T, rusqlite::Error>;

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

pub(crate) fn text_to_state(s: &str) -> RusqliteResult<DownloadState> {
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

pub(crate) fn thread_mode_to_text(mode: ThreadMode) -> &'static str {
    match mode {
        ThreadMode::Fixed => "fixed",
        ThreadMode::Adaptive => "adaptive",
    }
}

pub(crate) fn text_to_thread_mode(s: &str) -> RusqliteResult<ThreadMode> {
    match s {
        "fixed" => Ok(ThreadMode::Fixed),
        "adaptive" => Ok(ThreadMode::Adaptive),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown thread mode: {other}"
        ))),
    }
}

pub(crate) fn checksum_mode_to_text(mode: ChecksumMode) -> &'static str {
    match mode {
        ChecksumMode::None => "none",
        ChecksumMode::Blake3 => "blake3",
        ChecksumMode::Sha256 => "sha256",
        ChecksumMode::Sha1 => "sha1",
        ChecksumMode::Xxh3128 => "xxh3_128",
    }
}

pub(crate) fn text_to_checksum_mode(s: &str) -> RusqliteResult<ChecksumMode> {
    match s {
        "none" => Ok(ChecksumMode::None),
        "blake3" => Ok(ChecksumMode::Blake3),
        "sha256" => Ok(ChecksumMode::Sha256),
        "sha1" => Ok(ChecksumMode::Sha1),
        "xxh3_128" => Ok(ChecksumMode::Xxh3128),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown checksum mode: {other}"
        ))),
    }
}

pub(crate) fn adaptive_profile_to_text(profile: AdaptiveProfile) -> &'static str {
    match profile {
        AdaptiveProfile::Conservative => "conservative",
        AdaptiveProfile::Balanced => "balanced",
        AdaptiveProfile::Aggressive => "aggressive",
    }
}

pub(crate) fn text_to_adaptive_profile(s: &str) -> RusqliteResult<AdaptiveProfile> {
    match s {
        "conservative" => Ok(AdaptiveProfile::Conservative),
        "balanced" => Ok(AdaptiveProfile::Balanced),
        "aggressive" => Ok(AdaptiveProfile::Aggressive),
        other => Err(rusqlite::Error::InvalidParameterName(format!(
            "unknown adaptive profile: {other}"
        ))),
    }
}

pub(crate) fn i64_to_bool(v: i64) -> bool {
    v != 0
}

/// INSERT a full manifest row using named parameters.
pub(crate) fn insert_manifest_row(conn: &Connection, manifest: &Manifest) -> Result<()> {
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
                chunk_size, mirror_url, mirror_urls, current_mirror_index, priority, cdn_accelerated, cdn_node_ip,
                expected_checksum
            ) VALUES (
                :id, :url, :final_url, :user_agent, :destination_dir, :file_name,
                :file_name_locked, :destination_path, :temp_path, :total_bytes,
                :downloaded_bytes, :supports_ranges, :connection_count, :thread_mode,
                :requested_thread_count, :desired_thread_count, :allocated_thread_count,
                :adaptive_profile_snapshot, :thread_note, :etag, :last_modified,
                :state, :checksum_mode, :checksum, :error, :created_at_ms, :updated_at_ms,
                :chunk_size, :mirror_url, :mirror_urls, :current_mirror_index, :priority, :cdn_accelerated, :cdn_node_ip,
                :expected_checksum
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
        ":priority": manifest.priority as u8,
        ":cdn_accelerated": manifest.cdn_accelerated,
        ":cdn_node_ip": manifest.cdn_node_ip.as_deref(),
        ":expected_checksum": manifest.expected_checksum.as_deref(),
    })
    .with_context(|| format!("failed to insert download {}", manifest.id))?;

    Ok(())
}

/// UPDATE a full manifest row using named parameters.
pub(crate) fn update_manifest_row(conn: &Connection, manifest: &Manifest) -> Result<()> {
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
                current_mirror_index = :current_mirror_index,
                priority = :priority,
                cdn_accelerated = :cdn_accelerated,
                cdn_node_ip = :cdn_node_ip,
                expected_checksum = :expected_checksum
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
        ":priority": manifest.priority as u8,
        ":cdn_accelerated": manifest.cdn_accelerated,
        ":cdn_node_ip": manifest.cdn_node_ip.as_deref(),
        ":expected_checksum": manifest.expected_checksum.as_deref(),
    })
    .with_context(|| format!("failed to update download {}", manifest.id))?;

    Ok(())
}

pub(crate) fn row_to_manifest(row: &rusqlite::Row) -> RusqliteResult<Manifest> {
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
        extra_headers: vec![],
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
        expected_checksum: row.get::<_, Option<String>>(34).unwrap_or(None),
        error: row.get(24)?,
        created_at_ms: row.get::<_, i64>(25)? as u64,
        updated_at_ms: row.get::<_, i64>(26)? as u64,
        chunks: Vec::new(),
        cdn_accelerated: i64_to_bool(row.get::<_, i64>(32)?),
        cdn_node_ip: row.get::<_, Option<String>>(33)?,
        chunk_size: row.get::<_, i64>(27)? as u64,
        mirror_url: row.get(28)?,
        mirror_urls: row
            .get::<_, String>(29)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default(),
        current_mirror_index: row.get::<_, i64>(30).unwrap_or(0) as usize,
        priority: row.get::<_, u8>(31).unwrap_or(1).into(),
    })
}

#[cfg(test)]
pub(crate) trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

#[cfg(test)]
impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Database {
    /// Insert a new download (or update if the id already exists).
    pub fn insert_download(&self, manifest: &Manifest) -> Result<()> {
        let conn = self.lock_write();

        let is_new: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM downloads WHERE id = ?1",
                params![manifest.id],
                |row| row.get::<_, i64>(0),
            )
            .map(|count| count == 0)
            .unwrap_or(true);

        if is_new {
            insert_manifest_row(&conn, manifest)?;

            if !manifest.chunks.is_empty() {
                self.replace_chunks_inner(&conn, &manifest.id, &manifest.chunks)?;
            }
        } else {
            conn.execute_batch("BEGIN IMMEDIATE")
                .context("failed to begin transaction")?;

            let result = (|| -> Result<()> {
                update_manifest_row(&conn, manifest)?;

                if !manifest.chunks.is_empty() {
                    let dirty_chunks: Vec<&ChunkManifest> =
                        manifest.chunks.iter().filter(|c| c.dirty).collect();
                    if !dirty_chunks.is_empty() {
                        let mut stmt = conn
                            .prepare_cached(
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
    #[cfg(test)]
    pub fn update_download(&self, manifest: &Manifest) -> Result<()> {
        let conn = self.lock_write();
        update_manifest_row(&conn, manifest)?;
        self.replace_chunks_inner(&conn, &manifest.id, &manifest.chunks)?;
        Ok(())
    }

    /// Delete a download and all its chunks (cascaded via FK).
    pub fn delete_download(&self, id: &str) -> Result<()> {
        let conn = self.lock_write();
        conn.prepare_cached("DELETE FROM downloads WHERE id = ?1")
            .context("failed to prepare delete download query")?
            .execute(params![id])
            .with_context(|| format!("failed to delete download {id}"))?;
        self.vacuum_if_needed(&conn, 100)?;
        Ok(())
    }

    /// Fetch a single download with its chunks.
    #[cfg(test)]
    pub fn get_download(&self, id: &str) -> Result<Option<Manifest>> {
        let conn = self.lock_read();

        let mut stmt = conn
            .prepare_cached("SELECT * FROM downloads WHERE id = ?1")
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
    #[cfg(test)]
    pub fn list_downloads(&self) -> Result<Vec<Manifest>> {
        let conn = self.lock_read();

        let mut stmt = conn
            .prepare_cached("SELECT * FROM downloads ORDER BY created_at_ms DESC")
            .context("failed to prepare list_downloads query")?;

        let mut manifests: Vec<Manifest> = stmt
            .query_map([], row_to_manifest)
            .context("failed to map download rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect downloads")?;

        let mut chunk_stmt = conn
            .prepare_cached("SELECT * FROM chunks ORDER BY download_id, chunk_index")
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

        let mut chunk_map: HashMap<String, Vec<ChunkManifest>> = HashMap::default();
        for (download_id, chunk) in all_chunks {
            chunk_map.entry(download_id).or_default().push(chunk);
        }

        for manifest in &mut manifests {
            if let Some(chunks) = chunk_map.remove(&manifest.id) {
                manifest.chunks = chunks;
            }
        }

        Ok(manifests)
    }

    /// Return all download manifests WITHOUT chunks populated.
    pub fn list_download_headers(&self) -> Result<Vec<Manifest>> {
        let conn = self.lock_read();

        let mut stmt = conn
            .prepare_cached("SELECT * FROM downloads ORDER BY created_at_ms DESC")
            .context("failed to prepare list_download_headers query")?;

        let manifests: Vec<Manifest> = stmt
            .query_map([], row_to_manifest)
            .context("failed to map download rows")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to collect downloads")?;

        Ok(manifests)
    }

    /// Total number of downloads.
    #[cfg(test)]
    pub fn count_downloads(&self) -> Result<usize> {
        let conn = self.lock_read();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
            .context("failed to count downloads")?;
        Ok(count as usize)
    }

    /// Update the priority of a download in the database.
    pub fn set_priority(&self, download_id: &str, priority: u8) -> Result<()> {
        let conn = self.lock_write();
        conn.execute(
            "UPDATE downloads SET priority = ?1 WHERE id = ?2",
            params![priority, download_id],
        )
        .with_context(|| format!("failed to set priority for download {download_id}"))?;
        Ok(())
    }
}
