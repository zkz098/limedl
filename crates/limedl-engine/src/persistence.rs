//! DB persistence helpers shared by the download actors.
//!
//! Holds the free persistence functions used by the HTTP executor's periodic
//! 300 ms persist cycle and by the scheduler's rebalance cycle. Both route
//! through `Database`'s single-writer persistence actor, which coalesces
//! concurrent progress updates into a single transaction with zero lock
//! contention or `spawn_blocking` overhead.

use std::sync::Arc;

use anyhow::{Context, Result};

use super::{
    database::{self, Database, ProgressBatchEntry},
    download::ManagedDownload,
    manifest::ChunkManifest,
};

// ── Free persistence functions ──────────────────────────────────────────────

/// Persist a manifest snapshot (lightweight progress update).
///
/// Used by the periodic 300 ms persist cycle in HTTP download execution
/// and by `rebalance_allocations()`.
///
/// **Incremental chunk persist**: instead of writing every chunk, only chunks
/// whose `dirty` flag is set are persisted. The dirty flags are cleared
/// immediately after collection so that concurrent progress within the same
/// 300 ms window sets them again for the next cycle.
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
            state_text.to_string(),
            manifest.updated_at_ms,
            dirty_chunks,
        )
    };

    db.update_download_progress_async(
        id,
        downloaded_bytes,
        dirty_chunks,
        state_text,
        updated_at_ms,
    )
    .await
    .context("failed to persist download snapshot")
}

/// Persist progress snapshots for multiple downloads in a single coalesced
/// write transaction via the single-writer persistence actor.
pub async fn persist_manifest_snapshots_batch(
    db: &Arc<Database>,
    managed_list: &[Arc<ManagedDownload>],
) -> Result<()> {
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

    db.update_downloads_progress_batch_async(entries)
        .await
        .context("failed to persist download snapshots batch")
}
