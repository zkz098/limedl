//! The shared engine state the actor modules are allowed to touch.
//!
//! `DownloadManager` owns four sibling actors — `HttpExecutor`, `Scheduler`,
//! `TaskLifecycle` — and every actor method needs the manager's shared state
//! (the database, event bus, rate limiter, download map, settings service, …).
//! Passing `&DownloadManager` would make each actor depend on the `manager`
//! module; because `manager` owns the actors, that is a dependency cycle, and
//! the module-boundary guard in `tests/module_boundaries_tests.rs` rejects it.
//!
//! This trait is the seam.  It is declared here, in a module that depends only
//! on the shared infrastructure, and implemented for `DownloadManager` in
//! `manager.rs`.  Actors take `&dyn ManagerState`; the operations that would
//! otherwise need a handle to a *sibling* actor (`emit_progress`,
//! `rebalance_allocations`, …) are trait methods too, so this module never has
//! to name `Scheduler`, `TaskLifecycle` or `HttpExecutor` — naming one of them
//! would reintroduce the cycle through the back door.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use foldhash::HashMap;
use reqwest::Client;
use tokio::sync::{Notify, RwLock};
use tokio_util::sync::CancellationToken;

use crate::alt_svc::AltSvcManager;
use crate::buffer_pool::{BufferPool, IoWorker};
use crate::database::Database;
use crate::download::{ManagedDownload, WaitState};
use crate::error::Result;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::services::{ConcurrencyManager, SettingsService};
use crate::types::{AppSettings, DiskType};

/// Runtime control signals for shutdown and scheduler rebalance coordination.
///
/// Lives here rather than in `manager.rs` so [`ManagerState::controls`] can name
/// it without depending on the `manager` module.
pub struct RuntimeControls {
    /// Cancellation token for graceful shutdown of scheduler loop and workers.
    pub shutdown_token: CancellationToken,
    /// Notify mechanism for triggering scheduler rebalance events.
    pub rebalance_notify: Arc<Notify>,
}

/// The shared state of a running engine, as seen by the actor modules.
///
/// Crate-private: it exists so the engine's own modules can cooperate, not as
/// part of the published `limedl_core` API.
#[async_trait]
pub trait ManagerState: Send + Sync {
    // ── Shared infrastructure ─────────────────────────────────────────
    /// SQLite database (manifests, chunk progress, Alt-Svc cache).
    fn db(&self) -> &Arc<Database>;
    /// Broadcast channel every adapter subscribes to.
    fn event_bus(&self) -> &Arc<EventBus>;
    /// Global download/upload rate limiter.
    fn rate_limiter(&self) -> &Arc<RateLimiter>;
    /// Shared write buffer pool (HDD double-buffer / SSD write-combining).
    fn buffer_pool(&self) -> &Arc<BufferPool>;
    /// Dedicated I/O worker thread for file flush operations.
    fn io_worker(&self) -> &IoWorker;
    /// Settings service with change notification.
    fn settings_service(&self) -> &Arc<SettingsService>;
    /// Concurrency manager (task/thread allocation bookkeeping).
    fn concurrency(&self) -> &Arc<ConcurrencyManager>;
    /// Live download map keyed by download id.
    fn downloads(&self) -> &Arc<RwLock<HashMap<String, Arc<ManagedDownload>>>>;
    /// Shutdown and rebalance coordination signals.
    fn controls(&self) -> &RuntimeControls;
    /// Cross-task persistent Alt-Svc cache for HTTP/3 upgrades.
    fn alt_svc(&self) -> &Arc<AltSvcManager>;

    // ── Behaviour ─────────────────────────────────────────────────────
    /// Current application settings.
    async fn settings(&self) -> Result<AppSettings>;
    /// Full persistence of a download manifest to the database.
    async fn persist(&self, managed: Arc<ManagedDownload>) -> Result<()>;
    /// Resolve the HTTP client for a URL, honouring the CDN accelerator.
    async fn resolve_client(&self, url: &str) -> (Client, bool, Option<String>);
    /// Classify the disk backing `dir`.
    async fn resolve_disk_type(&self, dir: &Path) -> DiskType;

    // ── TaskLifecycle operations ──────────────────────────────────────
    /// Prepare a fresh (empty) temp file for a download.
    fn prepare_fresh_temp_file(&self, managed: &Arc<ManagedDownload>) -> Result<()>;
    /// Reset all progress counters for a download.
    fn reset_progress(&self, managed: &Arc<ManagedDownload>, force_single_stream: bool);
    /// Publish a progress event for a download.
    fn emit_progress(&self, managed: &Arc<ManagedDownload>);
    /// Publish the completion summary for a single-stream download.
    fn emit_single_summary(&self, managed: &Arc<ManagedDownload>);
    /// Delete the temp file (and any corrupt leftover) for a download.
    fn cleanup_files(&self, managed: &Arc<ManagedDownload>) -> Result<()>;
    /// Record downloaded bytes on a managed download (chunk index optional).
    fn record_progress(
        &self,
        managed: &Arc<ManagedDownload>,
        chunk_index: Option<usize>,
        bytes: u64,
    );
    /// Wait until a download has threads allocated and is actively running.
    async fn wait_until_active(
        &self,
        managed: &Arc<ManagedDownload>,
        token: &CancellationToken,
    ) -> WaitState;
    /// Drop finished downloads from the in-memory map.
    async fn evict_completed(&self) -> usize;

    // ── Scheduler operations ──────────────────────────────────────────
    /// Recompute per-task thread allocations.
    async fn rebalance_allocations(&self) -> Result<()>;
}
