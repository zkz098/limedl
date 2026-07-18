//! DB persistence and migration-related code — extracted from manager.rs
//! (Phase 3 of the manager.rs split).
//!
//! Contains `impl DownloadManager` methods for loading/persisting downloads
//! as well as the free function `persist_manifest_snapshot`.

use std::{
    path::Path,
    sync::Arc,
};

use parking_lot::Mutex;

use anyhow::{Context, Result};

use super::{
    aimd::AimdState,
    database::{self, Database},
    manager::{DownloadCore, DownloadManager, ManagedDownload},
    manifest::{ChunkManifest, snapshot_from_manifest},
    now_ms,
    types::DownloadState,
};

// ── DownloadManager persistence methods ─────────────────────────────────────

impl DownloadManager {
    /// Load all downloads from the SQLite database on startup.
    ///
    /// Called from `DownloadManager::new()`. Reconstructs `ManagedDownload`
    /// entries from stored `Manifest` rows.
    pub fn load_downloads_from_db(&self) -> Result<()> {
        let manifests = self
            .db
            .list_download_headers()
            .context("failed to load downloads from database")?;

        for mut manifest in manifests {
            let destination_exists = Path::new(&manifest.destination_path).exists();
            let temp_exists = Path::new(&manifest.temp_path).exists();

            if manifest.state == DownloadState::Verifying && destination_exists && !temp_exists {
                manifest.state = DownloadState::Completed;
                manifest.updated_at_ms = now_ms();
            }

            if matches!(
                manifest.state,
                DownloadState::Downloading
                    | DownloadState::Retrying
                    | DownloadState::Verifying
                    | DownloadState::Queued
            ) {
                manifest.state = DownloadState::Paused;
                manifest.connection_count = 0;
                manifest.allocated_thread_count = Some(0);
                manifest.updated_at_ms = now_ms();
            }

            // Lazy chunk loading: only load chunks for non-terminal downloads.
            // Completed / Failed / Canceled downloads don't need chunks at startup;
            // they'll be loaded on-demand if the user resumes or inspects them.
            let needs_chunks = !matches!(
                manifest.state,
                DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
            );
            if needs_chunks {
                manifest.chunks = self
                    .db
                    .load_chunks(&manifest.id)
                    .unwrap_or_default();
            }

            let snapshot = snapshot_from_manifest(&manifest);
            let managed = Arc::new(ManagedDownload {
                core: Mutex::new(DownloadCore {
                    snapshot,
                    manifest: manifest.clone(),
                }),
                runtime: Mutex::new(None),
                aimd: Mutex::new(AimdState::initial(
                    manifest.adaptive_profile_snapshot,
                    manifest.desired_thread_count,
                )),
                stop_notify: tokio::sync::Notify::new(),
            });

            tokio::task::block_in_place(|| {
                self.downloads
                    .blocking_write()
                    .insert(manifest.id.clone(), managed);
            });
        }
        Ok(())
    }

    /// Full persistence of a download manifest to the database.
    ///
    /// Called on state transitions (start, pause, resume, etc.) and
    /// when a download finishes or fails.
    pub async fn persist(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || db.insert_download(&manifest))
            .await
            .context("persist task panicked")?
            .context("failed to persist download to database")?;
        Ok(())
    }
}

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
        let manifest = &mut core.manifest;
        let state_text = database::download_state_to_text(&manifest.state);

        // Drain the dirty flag: collect dirty chunks and reset the flag so
        // concurrent writes in the same 300 ms interval will re-dirty them.
        // Identify dirty chunk indices, reset their dirty flag, and
        // collect a snapshot of their data for the DB write.
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
            manifest.downloaded_bytes,
            state_text,
            manifest.updated_at_ms,
            dirty_chunks,
        )
    };

    let db = db.clone();
    tokio::task::spawn_blocking(move || {
        db.update_download_progress(
            &id,
            downloaded_bytes,
            &dirty_chunks,
            state_text,
            updated_at_ms,
        )
    })
    .await
    .context("persist snapshot task panicked")?
    .context("failed to persist download snapshot")?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/persistence_tests.rs"]
mod tests;
