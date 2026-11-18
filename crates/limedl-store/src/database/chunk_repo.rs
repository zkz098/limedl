use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use redb::ReadableTable;

use super::connection::Database;
use super::schema::{CHUNK_PROGRESS, is_terminal_state};
use super::writer::{WriteCommand, async_reply, sync_reply};
use crate::manifest::{ChunkManifest, Manifest};
use crate::types::DownloadState;

const CHUNK_BLOB_VERSION: u8 = 1;
const CHUNK_RECORD_BYTES: usize = 41;

/// Single progress update entry for batch persist of multiple downloads.
#[derive(Clone)]
pub struct ProgressBatchEntry {
    pub id: String,
    pub downloaded_bytes: u64,
    pub dirty_chunks: Vec<ChunkManifest>,
    pub state: DownloadState,
    pub updated_at_ms: u64,
}

fn durable_snapshot(chunk: &ChunkManifest) -> ChunkManifest {
    // Persist the *durable* progress, not the received progress: the two differ by
    // whatever the write buffer still holds, and a chunk record is what the next
    // start resumes from.
    let durable = chunk.durable_bytes();
    let complete = durable >= chunk.byte_len();
    ChunkManifest {
        index: chunk.index,
        start: chunk.start,
        end: chunk.end,
        downloaded: durable,
        completed: complete,
        durable_downloaded: durable,
        claimed_by: chunk.claimed_by,
        dirty: false,
    }
}

pub(crate) fn encode_chunks(chunks: &[ChunkManifest]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5 + chunks.len() * CHUNK_RECORD_BYTES);
    buf.push(CHUNK_BLOB_VERSION);
    buf.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
    for chunk in chunks {
        let snap = durable_snapshot(chunk);
        buf.extend_from_slice(&(snap.index as u64).to_le_bytes());
        buf.extend_from_slice(&snap.start.to_le_bytes());
        buf.extend_from_slice(&snap.end.to_le_bytes());
        buf.extend_from_slice(&snap.durable_downloaded.to_le_bytes());
        buf.push(u8::from(snap.completed));
        let claimed = match snap.claimed_by {
            Some(n) => n as i64,
            None => -1,
        };
        buf.extend_from_slice(&claimed.to_le_bytes());
    }
    buf
}

pub(crate) fn decode_chunks(bytes: &[u8]) -> Result<Vec<ChunkManifest>> {
    if bytes.len() < 5 {
        bail!(
            "corrupt chunk blob: header too short ({} bytes)",
            bytes.len()
        );
    }
    if bytes[0] != CHUNK_BLOB_VERSION {
        bail!("unsupported chunk blob version {}", bytes[0]);
    }
    let count = u32::from_le_bytes(bytes[1..5].try_into().expect("4 bytes")) as usize;
    let expected_len = 5 + count * CHUNK_RECORD_BYTES;
    if bytes.len() != expected_len {
        bail!(
            "corrupt chunk blob: expected {expected_len} bytes for {count} chunks, got {}",
            bytes.len()
        );
    }

    let mut chunks = Vec::with_capacity(count);
    let mut offset = 5;
    for _ in 0..count {
        let index = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
        let start = u64::from_le_bytes(bytes[offset + 8..offset + 16].try_into().unwrap());
        let end = u64::from_le_bytes(bytes[offset + 16..offset + 24].try_into().unwrap());
        let durable = u64::from_le_bytes(bytes[offset + 24..offset + 32].try_into().unwrap());
        let completed = bytes[offset + 32] != 0;
        let claimed_raw = i64::from_le_bytes(bytes[offset + 33..offset + 41].try_into().unwrap());
        let claimed_by = if claimed_raw < 0 {
            None
        } else {
            Some(claimed_raw as usize)
        };
        chunks.push(ChunkManifest {
            index,
            start,
            end,
            downloaded: durable,
            completed,
            durable_downloaded: durable,
            claimed_by,
            dirty: false,
        });
        offset += CHUNK_RECORD_BYTES;
    }
    Ok(chunks)
}

pub(crate) fn merge_dirty_chunks(
    existing: Vec<ChunkManifest>,
    dirty_chunks: &[ChunkManifest],
) -> Vec<ChunkManifest> {
    let mut by_index: BTreeMap<usize, ChunkManifest> =
        existing.into_iter().map(|c| (c.index, c)).collect();
    for chunk in dirty_chunks {
        by_index.insert(chunk.index, durable_snapshot(chunk));
    }
    by_index.into_values().collect()
}

pub(crate) fn apply_progress_entry(
    active_table: &mut redb::Table<&str, &[u8]>,
    chunk_table: &mut redb::Table<&str, &[u8]>,
    id: &str,
    downloaded_bytes: u64,
    dirty_chunks: &[ChunkManifest],
    state: DownloadState,
    updated_at_ms: u64,
) -> Result<()> {
    let existing_manifest_bytes = {
        let guard = active_table
            .get(id)
            .with_context(|| format!("failed to read active download {id}"))?;
        let Some(value) = guard else {
            // Task is already in `terminal_downloads` or deleted: never rewind it.
            return Ok(());
        };
        value.value().to_vec()
    };

    let mut manifest: Manifest = serde_json::from_slice(&existing_manifest_bytes)
        .with_context(|| format!("failed to deserialize manifest {id}"))?;

    if is_terminal_state(&manifest.state) {
        return Ok(());
    }

    manifest.downloaded_bytes = downloaded_bytes;
    manifest.state = state;
    manifest.updated_at_ms = updated_at_ms;
    manifest.chunks.clear();

    let updated_bytes =
        serde_json::to_vec(&manifest).with_context(|| format!("failed to serialize {id}"))?;
    active_table
        .insert(id, updated_bytes.as_slice())
        .with_context(|| format!("failed to update active download {id}"))?;

    if !dirty_chunks.is_empty() {
        let existing_chunks = match chunk_table
            .get(id)
            .with_context(|| format!("failed to read chunk blob for {id}"))?
        {
            Some(guard) => decode_chunks(guard.value())?,
            None => Vec::new(),
        };
        let merged = merge_dirty_chunks(existing_chunks, dirty_chunks);
        let encoded = encode_chunks(&merged);
        chunk_table
            .insert(id, encoded.as_slice())
            .with_context(|| format!("failed to write chunk blob for {id}"))?;
    }

    Ok(())
}

impl Database {
    /// Incremental update for the 300 ms persist cycle, routed through the
    /// single-writer persistence actor.
    pub fn update_download_progress(
        &self,
        id: &str,
        downloaded_bytes: u64,
        dirty_chunks: &[ChunkManifest],
        state: DownloadState,
        updated_at_ms: u64,
    ) -> Result<()> {
        let (reply_tx, reply_rx) = sync_reply();
        self.writer().send(WriteCommand::Progress {
            entry: ProgressBatchEntry {
                id: id.to_owned(),
                downloaded_bytes,
                dirty_chunks: dirty_chunks.to_vec(),
                state,
                updated_at_ms,
            },
            reply: Some(reply_tx),
        })?;
        reply_rx
            .recv()
            .context("database writer dropped before replying")?
    }

    /// Async incremental progress update routed directly to the single-writer
    /// persistence actor without `spawn_blocking`.
    pub async fn update_download_progress_async(
        &self,
        id: String,
        downloaded_bytes: u64,
        dirty_chunks: Vec<ChunkManifest>,
        state: DownloadState,
        updated_at_ms: u64,
    ) -> Result<()> {
        let (reply_tx, reply_rx) = async_reply();
        self.writer().send(WriteCommand::Progress {
            entry: ProgressBatchEntry {
                id,
                downloaded_bytes,
                dirty_chunks,
                state,
                updated_at_ms,
            },
            reply: Some(reply_tx),
        })?;
        reply_rx
            .await
            .context("database writer dropped before replying")?
    }

    /// Persist progress snapshots for multiple downloads in a single transaction,
    /// routed through the single-writer persistence actor.
    pub fn update_downloads_progress_batch(&self, entries: &[ProgressBatchEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let (reply_tx, reply_rx) = sync_reply();
        self.writer().send(WriteCommand::ProgressBatch {
            entries: entries.to_vec(),
            reply: Some(reply_tx),
        })?;
        reply_rx
            .recv()
            .context("database writer dropped before replying")?
    }

    /// Async batch progress update routed directly to the single-writer
    /// persistence actor without `spawn_blocking`.
    pub async fn update_downloads_progress_batch_async(
        &self,
        entries: Vec<ProgressBatchEntry>,
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let (reply_tx, reply_rx) = async_reply();
        self.writer().send(WriteCommand::ProgressBatch {
            entries,
            reply: Some(reply_tx),
        })?;
        reply_rx
            .await
            .context("database writer dropped before replying")?
    }

    /// Load persisted chunks for a download in ascending `index` order.
    pub fn load_chunks(&self, download_id: &str) -> Result<Vec<ChunkManifest>> {
        let tx = self.begin_read()?;
        let table = tx.open_table(CHUNK_PROGRESS)?;
        match table
            .get(download_id)
            .with_context(|| format!("failed to load chunks for {download_id}"))?
        {
            Some(guard) => decode_chunks(guard.value()),
            None => Ok(Vec::new()),
        }
    }
}
