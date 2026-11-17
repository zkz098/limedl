use std::cmp::Reverse;

use anyhow::{Context, Result};
use redb::{Durability, ReadableTable, ReadableTableMetadata};

use super::chunk_repo::{decode_chunks, encode_chunks, merge_dirty_chunks};
use super::connection::Database;
use super::schema::{ACTIVE_DOWNLOADS, CHUNK_PROGRESS, TERMINAL_DOWNLOADS, is_terminal_state};
use crate::manifest::{ChunkManifest, Manifest};
use crate::types::DownloadState;

pub fn download_state_to_text(state: &DownloadState) -> &'static str {
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

pub(crate) fn text_to_download_state(s: &str) -> DownloadState {
    match s {
        "queued" => DownloadState::Queued,
        "downloading" => DownloadState::Downloading,
        "paused" => DownloadState::Paused,
        "retrying" => DownloadState::Retrying,
        "verifying" => DownloadState::Verifying,
        "completed" => DownloadState::Completed,
        "failed" => DownloadState::Failed,
        "canceled" => DownloadState::Canceled,
        _ => DownloadState::Queued,
    }
}

fn serialize_header(manifest: &Manifest) -> Result<Vec<u8>> {
    let mut header = manifest.clone();
    header.chunks.clear();
    serde_json::to_vec(&header)
        .with_context(|| format!("failed to serialize manifest {}", manifest.id))
}

fn deserialize_manifest(id: &str, bytes: &[u8]) -> Result<Manifest> {
    let mut manifest: Manifest = serde_json::from_slice(bytes)
        .with_context(|| format!("failed to deserialize manifest {id}"))?;
    manifest.chunks.clear();
    Ok(manifest)
}

fn sort_manifests_newest_first(manifests: &mut [Manifest]) {
    manifests.sort_by_key(|m| Reverse(m.created_at_ms));
}

impl Database {
    /// Insert a new download (or update if the id already exists).
    pub fn insert_download(&self, manifest: &Manifest) -> Result<()> {
        let header_bytes = serialize_header(manifest)?;
        let terminal = is_terminal_state(&manifest.state);

        let mut tx = self.begin_write()?;
        tx.set_durability(Durability::Immediate)?;

        {
            let mut active = tx.open_table(ACTIVE_DOWNLOADS)?;
            let mut term = tx.open_table(TERMINAL_DOWNLOADS)?;
            let mut chunks_tbl = tx.open_table(CHUNK_PROGRESS)?;

            let existed = active.get(manifest.id.as_str())?.is_some()
                || term.get(manifest.id.as_str())?.is_some();

            if terminal {
                let _ = active.remove(manifest.id.as_str())?;
                term.insert(manifest.id.as_str(), header_bytes.as_slice())
                    .context("failed to insert terminal download")?;
            } else {
                let _ = term.remove(manifest.id.as_str())?;
                active
                    .insert(manifest.id.as_str(), header_bytes.as_slice())
                    .context("failed to insert active download")?;
            }

            if manifest.chunks.is_empty() {
                let _ = chunks_tbl.remove(manifest.id.as_str())?;
            } else if !existed {
                let encoded = encode_chunks(&manifest.chunks);
                chunks_tbl
                    .insert(manifest.id.as_str(), encoded.as_slice())
                    .context("failed to write chunks")?;
            } else {
                let dirty_chunks: Vec<ChunkManifest> = manifest
                    .chunks
                    .iter()
                    .filter(|c| c.dirty)
                    .cloned()
                    .collect();
                if !dirty_chunks.is_empty() {
                    let existing = match chunks_tbl.get(manifest.id.as_str())? {
                        Some(guard) => decode_chunks(guard.value())?,
                        None => Vec::new(),
                    };
                    let merged = merge_dirty_chunks(existing, &dirty_chunks);
                    let encoded = encode_chunks(&merged);
                    chunks_tbl
                        .insert(manifest.id.as_str(), encoded.as_slice())
                        .context("failed to upsert dirty chunks")?;
                }
            }
        }

        tx.commit().context("failed to commit insert_download")?;
        Ok(())
    }

    /// Full persist escape hatch — writes all fields and replaces all chunks.
    #[cfg(test)]
    pub fn update_download(&self, manifest: &Manifest) -> Result<()> {
        let header_bytes = serialize_header(manifest)?;
        let terminal = is_terminal_state(&manifest.state);

        let mut tx = self.begin_write()?;
        tx.set_durability(Durability::Immediate)?;

        {
            let mut active = tx.open_table(ACTIVE_DOWNLOADS)?;
            let mut term = tx.open_table(TERMINAL_DOWNLOADS)?;
            let mut chunks_tbl = tx.open_table(CHUNK_PROGRESS)?;

            if terminal {
                let _ = active.remove(manifest.id.as_str())?;
                term.insert(manifest.id.as_str(), header_bytes.as_slice())
                    .context("failed to update terminal download")?;
            } else {
                let _ = term.remove(manifest.id.as_str())?;
                active
                    .insert(manifest.id.as_str(), header_bytes.as_slice())
                    .context("failed to update active download")?;
            }

            if manifest.chunks.is_empty() {
                let _ = chunks_tbl.remove(manifest.id.as_str())?;
            } else {
                let encoded = encode_chunks(&manifest.chunks);
                chunks_tbl
                    .insert(manifest.id.as_str(), encoded.as_slice())
                    .context("failed to replace chunks")?;
            }
        }

        tx.commit().context("failed to commit update_download")?;
        Ok(())
    }

    /// Delete a download and its chunk progress.
    pub fn delete_download(&self, id: &str) -> Result<()> {
        let mut tx = self.begin_write()?;
        tx.set_durability(Durability::Immediate)?;
        {
            let mut active = tx.open_table(ACTIVE_DOWNLOADS)?;
            let mut term = tx.open_table(TERMINAL_DOWNLOADS)?;
            let mut chunks_tbl = tx.open_table(CHUNK_PROGRESS)?;
            let _ = active.remove(id)?;
            let _ = term.remove(id)?;
            let _ = chunks_tbl.remove(id)?;
        }
        tx.commit().context("failed to commit delete_download")?;
        Ok(())
    }

    /// Fetch a single download's metadata row without its chunks.
    pub fn get_download_header(&self, id: &str) -> Result<Option<Manifest>> {
        let tx = self.begin_read()?;
        let active = tx.open_table(ACTIVE_DOWNLOADS)?;
        if let Some(guard) = active.get(id)? {
            return Ok(Some(deserialize_manifest(id, guard.value())?));
        }
        let term = tx.open_table(TERMINAL_DOWNLOADS)?;
        if let Some(guard) = term.get(id)? {
            return Ok(Some(deserialize_manifest(id, guard.value())?));
        }
        Ok(None)
    }

    /// Number of downloads in a terminal state (completed / failed / canceled).
    ///
    /// $O(1)$ metadata read via `redb::Table::len()`.
    pub fn count_terminal_downloads(&self) -> Result<usize> {
        let tx = self.begin_read()?;
        let term = tx.open_table(TERMINAL_DOWNLOADS)?;
        Ok(term.len().context("failed to count terminal downloads")? as usize)
    }

    /// Fetch a single download with its chunks.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn get_download(&self, id: &str) -> Result<Option<Manifest>> {
        let tx = self.begin_read()?;
        let active = tx.open_table(ACTIVE_DOWNLOADS)?;
        let header = if let Some(guard) = active.get(id)? {
            Some(deserialize_manifest(id, guard.value())?)
        } else {
            let term = tx.open_table(TERMINAL_DOWNLOADS)?;
            match term.get(id)? {
                Some(guard) => Some(deserialize_manifest(id, guard.value())?),
                None => None,
            }
        };

        match header {
            Some(mut manifest) => {
                let chunks_tbl = tx.open_table(CHUNK_PROGRESS)?;
                if let Some(guard) = chunks_tbl.get(id)? {
                    manifest.chunks = decode_chunks(guard.value())?;
                }
                Ok(Some(manifest))
            }
            None => Ok(None),
        }
    }

    /// Return every download in the database, each with its chunks populated.
    #[cfg(test)]
    pub fn list_downloads(&self) -> Result<Vec<Manifest>> {
        let tx = self.begin_read()?;
        let active = tx.open_table(ACTIVE_DOWNLOADS)?;
        let term = tx.open_table(TERMINAL_DOWNLOADS)?;
        let chunks_tbl = tx.open_table(CHUNK_PROGRESS)?;

        let mut manifests = Vec::new();
        for entry in active.iter()? {
            let (k, v) = entry?;
            let mut m = deserialize_manifest(k.value(), v.value())?;
            if let Some(c) = chunks_tbl.get(k.value())? {
                m.chunks = decode_chunks(c.value())?;
            }
            manifests.push(m);
        }
        for entry in term.iter()? {
            let (k, v) = entry?;
            let mut m = deserialize_manifest(k.value(), v.value())?;
            if let Some(c) = chunks_tbl.get(k.value())? {
                m.chunks = decode_chunks(c.value())?;
            }
            manifests.push(m);
        }
        sort_manifests_newest_first(&mut manifests);
        Ok(manifests)
    }

    /// Return all download manifests WITHOUT chunks populated, newest first.
    pub fn list_download_headers(&self) -> Result<Vec<Manifest>> {
        let tx = self.begin_read()?;
        let active = tx.open_table(ACTIVE_DOWNLOADS)?;
        let term = tx.open_table(TERMINAL_DOWNLOADS)?;

        let mut manifests = Vec::with_capacity((active.len()? + term.len()?) as usize);
        for entry in active.iter()? {
            let (k, v) = entry?;
            manifests.push(deserialize_manifest(k.value(), v.value())?);
        }
        for entry in term.iter()? {
            let (k, v) = entry?;
            manifests.push(deserialize_manifest(k.value(), v.value())?);
        }
        sort_manifests_newest_first(&mut manifests);
        Ok(manifests)
    }

    /// Total number of downloads.
    #[cfg(test)]
    pub fn count_downloads(&self) -> Result<usize> {
        let tx = self.begin_read()?;
        let active = tx.open_table(ACTIVE_DOWNLOADS)?;
        let term = tx.open_table(TERMINAL_DOWNLOADS)?;
        Ok((active.len()? + term.len()?) as usize)
    }

    /// Update the priority of a download in the database.
    pub fn set_priority(&self, download_id: &str, priority: u8) -> Result<()> {
        let prio = match priority {
            0 => crate::types::Priority::Low,
            2 => crate::types::Priority::High,
            _ => crate::types::Priority::Normal,
        };

        let mut tx = self.begin_write()?;
        tx.set_durability(Durability::Immediate)?;
        {
            let mut active = tx.open_table(ACTIVE_DOWNLOADS)?;
            let active_bytes = active.get(download_id)?.map(|g| g.value().to_vec());
            if let Some(bytes) = active_bytes {
                let mut m = deserialize_manifest(download_id, &bytes)?;
                m.priority = prio;
                let updated = serialize_header(&m)?;
                active.insert(download_id, updated.as_slice())?;
            } else {
                let mut term = tx.open_table(TERMINAL_DOWNLOADS)?;
                let term_bytes = term.get(download_id)?.map(|g| g.value().to_vec());
                if let Some(bytes) = term_bytes {
                    let mut m = deserialize_manifest(download_id, &bytes)?;
                    m.priority = prio;
                    let updated = serialize_header(&m)?;
                    term.insert(download_id, updated.as_slice())?;
                }
            }
        }
        tx.commit()
            .with_context(|| format!("failed to commit set_priority for download {download_id}"))?;
        Ok(())
    }

    // ── Async wrappers ───────────────────────────────────────────────────────

    /// Async wrapper for [`Self::insert_download`] that runs on Tokio's blocking pool.
    pub async fn insert_download_async(
        self: &std::sync::Arc<Self>,
        manifest: Manifest,
    ) -> Result<()> {
        self.run_blocking(move |db| db.insert_download(&manifest))
            .await
    }

    /// Async wrapper for [`Self::delete_download`] that runs on Tokio's blocking pool.
    pub async fn delete_download_async(self: &std::sync::Arc<Self>, id: String) -> Result<()> {
        self.run_blocking(move |db| db.delete_download(&id)).await
    }

    /// Async wrapper for [`Self::get_download_header`] that runs on Tokio's blocking pool.
    pub async fn get_download_header_async(
        self: &std::sync::Arc<Self>,
        id: String,
    ) -> Result<Option<Manifest>> {
        self.run_blocking(move |db| db.get_download_header(&id))
            .await
    }

    /// Async wrapper for [`Self::count_terminal_downloads`] that runs on Tokio's blocking pool.
    pub async fn count_terminal_downloads_async(self: &std::sync::Arc<Self>) -> Result<usize> {
        self.run_blocking(Database::count_terminal_downloads).await
    }

    /// Async wrapper for [`Self::list_download_headers`] that runs on Tokio's blocking pool.
    pub async fn list_download_headers_async(self: &std::sync::Arc<Self>) -> Result<Vec<Manifest>> {
        self.run_blocking(Database::list_download_headers).await
    }

    /// Async wrapper for [`Self::set_priority`] that runs on Tokio's blocking pool.
    pub async fn set_priority_async(
        self: &std::sync::Arc<Self>,
        download_id: String,
        priority: u8,
    ) -> Result<()> {
        self.run_blocking(move |db| db.set_priority(&download_id, priority))
            .await
    }
}
