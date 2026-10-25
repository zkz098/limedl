//! DB persistence helpers shared by the download actors.
//!
//! Holds the free persistence functions used by the HTTP executor's periodic
//! 300 ms persist cycle and by the scheduler's rebalance cycle.  The
//! `impl DownloadManager` methods that used to live here moved back into
//! `manager.rs` with the type they belong to: keeping them here made this
//! module import `manager::DownloadManager`, which closed the
//! `manager`/`http_executor`/`scheduler`/`task_lifecycle`/`persistence`
//! dependency cycle the module-boundary guard forbids.

use std::sync::Arc;

use anyhow::{Context, Result};

use super::{
    database::{self, Database},
    download::ManagedDownload,
    manifest::ChunkManifest,
};

// ── Free persistence functions ──────────────────────────────────────────────

/// Persist a manifest snapshot (lightweight progress update).
///
/// Used by the periodic 300 ms persist cycle in HTTP download execution
/// and by `rebalance_allocations()`.
///
/// **Incremental chunk persist**: instead of writing every chunk, only chunks
/// whose `dirty` flag is set are upserted via `INSERT OR REPLACE`. The dirty
/// flags are cleared immediately after collection so that concurrent progress
/// within the same 300 ms window sets them again for the next cycle.
pub async fn persist_manifest_snapshot(
    db: &Arc<Database>,
    managed: &Arc<ManagedDownload>,
) -> Result<()> {
    let (id, downloaded_bytes, state_text, updated_at_ms, dirty_chunks) = {
        let mut core = managed.lock_core();
        // The manifest's `downloaded_bytes` is the *received* counter; what goes to
        // the database is the durable one (clamped, so a reset path that forgot to
        // clear the counter cannot claim more than was received).
        let durable_bytes = core.durable_bytes.min(core.manifest.downloaded_bytes);
        let manifest = &mut core.manifest;
        let state_text = database::download_state_to_text(&manifest.state);

        // Drain the dirty flag: collect dirty chunks and reset the flag so
        // concurrent writes in the same 300 ms interval will re-dirty them.
        // Identify dirty chunk indices, reset their dirty flag, and
        // collect a snapshot of their data for the DB write. The chunks are
        // written through `chunk_to_params`, which stores each chunk's durable
        // counter rather than its received one.
        let dirty_chunks: Vec<ChunkManifest> = manifest
            .chunks
            .iter_mut()
            .filter_map(|chunk| {
                if chunk.dirty {
                    chunk.dirty = false;
                    Some(chunk.clone())
                } else {
                    None
                }
            })
            .collect();

        (
            manifest.id.clone(),
            durable_bytes,
            state_text,
            manifest.updated_at_ms,
            dirty_chunks,
        )
    };

    // Clone before the first move so retry closure can reuse them.
    let id2 = id.clone();
    let dirty_chunks2 = dirty_chunks.clone();

    let db = db.clone();
    let db2 = db.clone();
    let result = tokio::task::spawn_blocking(move || {
        db.update_download_progress(
            &id2,
            downloaded_bytes,
            &dirty_chunks2,
            state_text,
            updated_at_ms,
        )
    })
    .await
    .context("persist snapshot task panicked")?;

    if result.is_err() {
        // Retry once on transient failure (e.g. SQLITE_BUSY, I/O hiccup).
        // The dirty flags were already cleared when we collected dirty_chunks;
        // a successful retry ensures progress isn't silently dropped.
        tokio::task::spawn_blocking(move || {
            db2.update_download_progress(
                &id,
                downloaded_bytes,
                &dirty_chunks,
                state_text,
                updated_at_ms,
            )
        })
        .await
        .context("persist snapshot retry task panicked")?
        .context("failed to persist download snapshot after retry")?;
    } else {
        result.context("failed to persist download snapshot")?;
    }
    Ok(())
}

/// Persist progress snapshots for multiple downloads in a single database transaction.
///
/// Used by the scheduler's rebalance cycle to batch all active download
/// progress updates into one transaction, reducing lock contention.
pub async fn persist_manifest_snapshots_batch(
    db: &Arc<Database>,
    managed_list: &[Arc<ManagedDownload>],
) -> Result<()> {
    use super::database::ProgressBatchEntry;

    let mut entries: Vec<ProgressBatchEntry> = Vec::with_capacity(managed_list.len());

    for managed in managed_list {
        let mut core = managed.lock_core();
        // Durable, not received — see `persist_manifest_snapshot`.
        let durable_bytes = core.durable_bytes.min(core.manifest.downloaded_bytes);
        let manifest = &mut core.manifest;
        let state_text = database::download_state_to_text(&manifest.state);

        let dirty_chunks: Vec<ChunkManifest> = manifest
            .chunks
            .iter_mut()
            .filter_map(|chunk| {
                if chunk.dirty {
                    chunk.dirty = false;
                    Some(chunk.clone())
                } else {
                    None
                }
            })
            .collect();

        // Always include every active download — even those without dirty chunks —
        // because the rebalance cycle may have changed state, allocated_thread_count,
        // connection_count, or other metadata that needs to be persisted.
        entries.push(ProgressBatchEntry {
            id: manifest.id.clone(),
            downloaded_bytes: durable_bytes,
            dirty_chunks,
            state: state_text.to_string(),
            updated_at_ms: manifest.updated_at_ms,
        });
    }

    if entries.is_empty() {
        return Ok(());
    }

    let db = db.clone();
    let db_clone = db.clone();
    let entries_clone = entries.clone();
    let result = tokio::task::spawn_blocking(move || db.update_downloads_progress_batch(&entries))
        .await
        .context("persist batch task panicked")?;

    if result.is_err() {
        // Retry once on transient failure (e.g. SQLITE_BUSY, I/O hiccup).
        // The dirty flags were already cleared when we collected dirty_chunks;
        // a successful retry ensures progress isn't silently dropped.
        tokio::task::spawn_blocking(move || {
            db_clone.update_downloads_progress_batch(&entries_clone)
        })
        .await
        .context("persist batch retry task panicked")?
        .context("failed to persist download snapshots batch after retry")?;
    } else {
        result.context("failed to persist download snapshots batch")?;
    }

    Ok(())
}
