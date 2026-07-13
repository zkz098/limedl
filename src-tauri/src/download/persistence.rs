//! DB persistence and migration-related code — extracted from manager.rs
//! (Phase 3 of the manager.rs split).
//!
//! Contains `impl DownloadManager` methods for loading/persisting downloads
//! as well as the free function `persist_manifest_snapshot`.

use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};

use super::{
    aimd::AimdState,
    database::{self, Database},
    manager::{DownloadManager, ManagedDownload, now_ms},
    manifest::snapshot_from_manifest,
    types::DownloadState,
};

// ── DownloadManager persistence methods ─────────────────────────────────────

impl DownloadManager {
    /// Load all downloads from the SQLite database on startup.
    ///
    /// Called from `DownloadManager::new()`. Reconstructs `ManagedDownload`
    /// entries from stored `Manifest` rows.
    pub(crate) fn load_downloads_from_db(&self) -> Result<()> {
        let manifests = self
            .db
            .list_downloads()
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

            let snapshot = snapshot_from_manifest(&manifest);
            let managed = Arc::new(ManagedDownload {
                snapshot: Mutex::new(snapshot),
                manifest: Mutex::new(manifest.clone()),
                runtime: Mutex::new(None),
                aimd: Mutex::new(AimdState::initial(
                    manifest.adaptive_profile_snapshot,
                    manifest.desired_thread_count,
                )),
                stop_notify: tokio::sync::Notify::new(),
            });

            self.downloads
                .blocking_write()
                .insert(manifest.id.clone(), managed);
        }
        Ok(())
    }

    /// Full persistence of a download manifest to the database.
    ///
    /// Called on state transitions (start, pause, resume, etc.) and
    /// when a download finishes or fails.
    pub(crate) async fn persist(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_manifest().clone();
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
pub(crate) async fn persist_manifest_snapshot(
    db: &Arc<Database>,
    managed: &Arc<ManagedDownload>,
) -> Result<()> {
    let manifest = managed.lock_manifest().clone();
    let state_text = database::download_state_to_text(&manifest.state);
    let db = db.clone();
    tokio::task::spawn_blocking(move || {
        db.update_download_progress(
            &manifest.id,
            manifest.downloaded_bytes,
            &manifest.chunks,
            state_text,
            manifest.updated_at_ms,
        )
    })
    .await
    .context("persist snapshot task panicked")?
    .context("failed to persist download snapshot")?;
    Ok(())
}
