//! Lazy startup wrapper around [`RqbitBtBackend`].
//!
//! Creating the BitTorrent session is not cheap: it binds TCP/uTP sockets, starts
//! DHT/LSD, loads resume data from disk, applies engine tuning and parses the
//! IP blocklist. Doing all of that inline in `bootstrap()` delayed app startup
//! (window creation) by the whole engine startup.
//!
//! [`LazyBtBackend`] moves that work to a background warm-up task:
//!
//! 1. construction only records the launch configuration (cheap, synchronous);
//! 2. [`LazyBtBackend::spawn_startup`] starts the engine in the background;
//! 3. operations that really need the engine await it via
//!    [`LazyBtBackend::engine`].
//!
//! ## Readiness policy
//!
//! | Operation                                                            | Before the engine is ready       |
//! |----------------------------------------------------------------------|----------------------------------|
//! | task ops (`start`/`pause`/`resume`/`cancel`/`remove`/`purge`/`status`/`open_*`) | awaits the warm-up    |
//! | `list`                                                               | returns the persisted index      |
//! | peer/tracker/piece/file/`runtime_status` queries                     | empty / disconnected result      |
//! | `update_settings`                                                    | stores settings for the warm-up  |
//! | `shutdown`                                                           | cancels the in-flight warm-up or shuts a running engine down |
//!
//! `list` and the read-only queries deliberately do **not** block: the task
//! list is loaded before the desktop window is shown, and it must not wait on
//! the BT session.
//!
//! ## Lightweight BT mode (`AppSettings.bt.lightweight_mode`)
//!
//! | | engine at launch | engine while idle | engine when work appears |
//! |---|---|---|---|
//! | off (default) | warmed up in the background | stays up | n/a |
//! | on | only if an unfinished task exists | shuts down once every torrent is paused | started on demand |
//!
//! The engine slot is **restartable** (unlike a `OnceCell`): the idle
//! supervisor tears the session down and a later `start`/`resume` builds a new
//! one. While the engine is down, [`DownloadBackend::list`](crate::protocol::DownloadBackend::list) serves rows from the
//! `bt_tasks` index that the engine refreshes whenever it is running, so BT
//! tasks stay visible in the UI without paying for session startup.
//!
//! A failed start is cached in `startup_error`: later calls fail fast with the
//! original message instead of silently re-attempting session creation (the
//! session holds exclusive resources such as the configured listen port). An
//! explicit `spawn_startup` clears that cache so the user can retry after
//! fixing the cause.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use librqbit::dht::Id20;
use parking_lot::{Mutex, RwLock};

use super::lazy_sync::idle_settle_elapsed;
pub use super::lazy_sync::take_legacy_resume_files;
use super::{RqbitBtBackend, info_hash_from_engine, info_hash_to_engine};
use crate::database::Database;
use crate::error::{DownloadError, Result};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::slot_guard::{SlotArc, SlotAtomicUsize};
use crate::types::{
    AppSettings, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtSettings, BtTrackerInfo,
    BtUploadStatus, DownloadState, DownloadSummary, TorrentFileEntry,
};
use crate::{lock, now_ms};

/// How long `shutdown` waits for an in-flight engine creation before giving up.
///
/// The wait only covers local work (socket binds, resume-data load, blocklist
/// parse) — the session actor is spawned, never joined — so exceeding this
/// grace period means the process is exiting while a broken disk hangs.
const STARTUP_SHUTDOWN_GRACE: Duration = Duration::from_secs(20);

/// Error returned to callers once `shutdown` started.
const ENGINE_SHUTTING_DOWN: &str = "BT engine is shutting down";

/// How often the running engine refreshes the persisted `bt_tasks` index.
const INDEX_SYNC_INTERVAL: Duration = Duration::from_secs(5);

/// How often lightweight mode checks whether the engine can be unloaded.
const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(10);

/// How long the engine must have seen no task operation before an idle check is
/// allowed to unload it.
///
/// Without this, a supervisor tick landing between `engine()` handing out the
/// session and `start()` inserting the torrent would see an empty session and
/// tear it down under the caller's feet. Task operations call [`engine`] and
/// bump the activity stamp, so the settle window is always newer than the last
/// user request.
const IDLE_SETTLE: Duration = Duration::from_secs(20);

/// Whether a persisted state means "there is still work to do".
///
/// Paused, completed, failed and canceled tasks do not justify loading the
/// engine at launch; a task that was mid-transfer does.
fn is_unfinished_state(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Downloading
            | DownloadState::Queued
            | DownloadState::Retrying
            | DownloadState::Verifying
    )
}

/// BitTorrent backend with a lazily started, restartable engine.
///
/// Registered in `BackendRegistry` as the `TaskKind::Bt` backend; `bootstrap()`
/// returns while the irontide session is still being created in the background.
pub struct LazyBtBackend {
    /// Latest settings snapshot — also the launch configuration for the engine.
    settings: Mutex<AppSettings>,
    /// Directory for BT state / resume files.
    state_dir: PathBuf,
    /// Default download output directory.
    default_output_dir: PathBuf,
    /// Central event bus (forwarded to the engine).
    event_bus: Arc<EventBus>,
    /// SQLite handle for the persisted BT task index.
    db: Arc<Database>,
    /// Active BT download counter, shared with `DownloadManager`.
    pub(crate) active_bt_count: SlotArc<SlotAtomicUsize>,
    /// Maximum concurrent BT downloads, shared with `DownloadManager`.
    /// Read by `limedl-core`'s BT integration tests (concurrency assertion).
    pub max_concurrent_bt: SlotArc<SlotAtomicUsize>,
    /// The started engine; empty while the engine is unloaded.
    ///
    /// A `parking_lot` lock (not an async one) so [`Self::engine_if_ready`] can
    /// stay synchronous on latency-sensitive read paths.
    engine: RwLock<Option<Arc<RqbitBtBackend>>>,
    /// Serializes engine creation and teardown so a restart can never race a
    /// shutdown into binding the same listen port twice.
    lifecycle: tokio::sync::Mutex<()>,
    /// Join handle of the background warm-up started by `spawn_startup`.
    startup_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Whether the most recently requested warm-up has finished.
    warmup_completed: AtomicBool,
    /// Cached startup failure message, so later calls fail fast.
    startup_error: Mutex<Option<String>>,
    /// Set by `shutdown`; new work is rejected and a late warm-up tears itself
    /// down again.
    shutting_down: AtomicBool,
    /// Wall-clock stamp of the last operation that needed the engine.
    last_activity_ms: AtomicU64,
    /// Background task that mirrors the running engine's tasks into `bt_tasks`.
    index_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Background task that unloads the engine once it is idle in lightweight mode.
    idle_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl LazyBtBackend {
    /// Record the launch configuration. Does **not** touch the network or disk.
    pub fn new(
        settings: &AppSettings,
        state_dir: PathBuf,
        default_output_dir: PathBuf,
        event_bus: Arc<EventBus>,
        db: Arc<Database>,
        active_bt_count: SlotArc<SlotAtomicUsize>,
        max_concurrent_bt: SlotArc<SlotAtomicUsize>,
    ) -> Self {
        Self {
            settings: Mutex::new(settings.clone()),
            state_dir,
            default_output_dir,
            event_bus,
            db,
            active_bt_count,
            max_concurrent_bt,
            engine: RwLock::new(None),
            lifecycle: tokio::sync::Mutex::new(()),
            startup_task: Mutex::new(None),
            warmup_completed: AtomicBool::new(true),
            startup_error: Mutex::new(None),
            shutting_down: AtomicBool::new(false),
            last_activity_ms: AtomicU64::new(now_ms()),
            index_task: Mutex::new(None),
            idle_task: Mutex::new(None),
        }
    }

    /// Kick off the background engine warm-up. Idempotent; requires a tokio
    /// runtime. Returns immediately — `bootstrap()` does not wait.
    ///
    /// Clears a cached startup failure so an explicit (re)start retries.
    pub fn spawn_startup(self: &Arc<Self>) {
        let mut slot = lock(&self.startup_task);
        if self.is_ready() || self.shutting_down.load(Ordering::Acquire) {
            return;
        }
        // A finished handle from an earlier warm-up must not block a new one:
        // after the idle supervisor unloaded the engine, this is exactly how
        // "leave lightweight mode" brings it back.
        if slot.as_ref().is_some_and(|handle| !handle.is_finished()) {
            return;
        }
        *slot = None;
        *lock(&self.startup_error) = None;
        self.warmup_completed.store(false, Ordering::Release);
        let this = self.clone();
        *slot = Some(tokio::spawn(async move {
            // `wait_ready` logs and caches failures; nothing else to do here.
            let _ = this.wait_ready().await;
            this.warmup_completed.store(true, Ordering::Release);
        }));
    }

    /// Start the persisted-index mirror loop (5 s cadence). Idempotent.
    pub fn spawn_index_sync_loop(self: &Arc<Self>) {
        let mut slot = lock(&self.index_task);
        if slot.is_some() {
            return;
        }
        let weak = Arc::downgrade(self);
        *slot = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(INDEX_SYNC_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Some(this) = weak.upgrade() else {
                    break;
                };
                // Self-heal eager mode: leaving lightweight mode at runtime
                // asks for an always-on engine, and `apply_settings(&self)`
                // cannot spawn the warm-up because it only holds `&self`.
                // A cached startup error deliberately blocks the retry, matching
                // the "no silent retry after a failed session" contract.
                if !this.lightweight_mode() && !this.is_ready() && this.startup_error().is_none() {
                    this.spawn_startup();
                }
                this.sync_index().await;
            }
        }));
    }

    /// Start the lightweight-mode idle supervisor (10 s cadence). Idempotent.
    ///
    /// The loop is a no-op while lightweight mode is disabled, so it can be
    /// spawned unconditionally and reacts to a runtime settings change.
    pub fn spawn_idle_supervisor(self: &Arc<Self>) {
        let mut slot = lock(&self.idle_task);
        if slot.is_some() {
            return;
        }
        let weak = Arc::downgrade(self);
        *slot = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(IDLE_CHECK_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Some(this) = weak.upgrade() else {
                    break;
                };
                if this.shutting_down.load(Ordering::Acquire) {
                    // Leave the task instead of aborting it: `shutdown` may be
                    // waiting on a teardown this loop is in the middle of.
                    break;
                }
                if !this.lightweight_mode() {
                    continue;
                }
                if !idle_settle_elapsed(
                    this.last_activity_ms.load(Ordering::Acquire),
                    now_ms(),
                    IDLE_SETTLE.as_millis() as u64,
                ) {
                    continue;
                }
                let Some(engine) = this.engine_if_ready() else {
                    continue;
                };
                match engine.has_active_torrents().await {
                    // Seeding, downloading, checking or queued: still work.
                    Ok(true) => {}
                    Ok(false) => this.stop_idle_engine().await,
                    // Unknown state must not tear a live session down.
                    Err(e) => tracing::debug!("BT idle check failed, keeping engine up: {e}"),
                }
            }
        }));
    }

    /// Whether the persisted index contains work that justifies loading the
    /// engine at launch. Falls back to the on-disk resume files when the index
    /// is empty (first run after enabling lightweight mode, or an upgrade from
    /// a build that had no index).
    pub fn has_unfinished_tasks(&self) -> bool {
        match self.db.list_bt_tasks() {
            Ok(tasks) if !tasks.is_empty() => {
                tasks.iter().any(|task| is_unfinished_state(task.state))
            }
            Ok(_) => self.resume_files_present(),
            Err(e) => {
                tracing::warn!("failed to read the BT task index: {e}");
                self.resume_files_present()
            }
        }
    }

    /// Await the engine, creating it if nobody started the warm-up yet.
    ///
    /// Returns the cached startup error when a previous attempt failed. Racing
    /// callers are deduplicated by the lifecycle mutex, and a concurrent idle
    /// shutdown is waited out before a fresh session is created.
    pub async fn engine(&self) -> Result<Arc<RqbitBtBackend>> {
        if let Some(message) = lock(&self.startup_error).clone() {
            return Err(DownloadError::Torrent(message));
        }
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(DownloadError::Torrent(ENGINE_SHUTTING_DOWN.into()));
        }
        // Any engine use is activity: the idle supervisor must not unload the
        // session between this call returning and the torrent being inserted.
        self.last_activity_ms.store(now_ms(), Ordering::Release);
        if let Some(engine) = self.engine_if_ready() {
            return Ok(engine);
        }

        // Creation and teardown are serialized: without this, an idle shutdown
        // in flight and a fresh `engine()` could both bind the listen port.
        let _guard = self.lifecycle.lock().await;

        // Re-check: another caller may have created the engine while we waited.
        if let Some(engine) = self.engine_if_ready() {
            return Ok(engine);
        }
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(DownloadError::Torrent(ENGINE_SHUTTING_DOWN.into()));
        }

        let engine = match self.create_engine().await {
            Ok(engine) => engine,
            Err(e) => return Err(self.record_startup_failure(e)),
        };

        // `shutdown` may have completed between the flag check above and the
        // session coming up; the late engine must not survive it.
        if self.shutting_down.load(Ordering::Acquire) {
            engine.shutdown().await;
            return Err(DownloadError::Torrent(ENGINE_SHUTTING_DOWN.into()));
        }

        *self.engine.write() = Some(engine.clone());
        // A session is up, so a stale failure from an earlier attempt is gone.
        *lock(&self.startup_error) = None;
        // Mirror the restored torrents into the index and re-publish them: the
        // alert bridge's `TorrentAdded` alerts fire during session creation,
        // before it subscribes.
        self.refresh_index_and_publish(&engine).await;
        self.last_activity_ms.store(now_ms(), Ordering::Release);
        drop(_guard);

        Ok(engine)
    }

    /// Await the startup attempt (success or failure). Never fails — callers
    /// that need the reason use [`Self::startup_error`].
    pub async fn wait_ready(&self) -> bool {
        match self.engine().await {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!("BT engine not ready: {e}");
                false
            }
        }
    }

    /// Wait for a warm-up that was already requested, **without** starting the
    /// engine if none is in flight.
    ///
    /// Used by the desktop client's post-startup BT list refresh: in lightweight
    /// mode the engine may legitimately stay unloaded, and that must not be
    /// turned into a session startup by a read path.
    pub async fn wait_for_warmup(&self) -> bool {
        if self.is_ready()
            || self.startup_error().is_some()
            || self.warmup_completed.load(Ordering::Acquire)
        {
            return self.is_ready();
        }
        if lock(&self.startup_task).is_none() {
            return self.is_ready();
        }
        // Poll the completion flag instead of awaiting a `Notify`: the warm-up
        // task may finish before this future is polled, and a lost notification
        // would stall the caller. The loop only runs during startup.
        let deadline = tokio::time::Instant::now() + STARTUP_SHUTDOWN_GRACE;
        while !self.warmup_completed.load(Ordering::Acquire) {
            if self.is_ready() || self.startup_error().is_some() {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                tracing::warn!("BT warm-up did not finish within the grace period");
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        self.is_ready()
    }

    /// The engine, if it is already running. Never blocks.
    pub fn engine_if_ready(&self) -> Option<Arc<RqbitBtBackend>> {
        if self.shutting_down.load(Ordering::Acquire) {
            return None;
        }
        self.engine.read().clone()
    }

    /// Whether the engine is up and not shutting down.
    pub fn is_ready(&self) -> bool {
        self.engine_if_ready().is_some()
    }

    /// Cached startup failure, if the engine never came up.
    pub fn startup_error(&self) -> Option<String> {
        lock(&self.startup_error).clone()
    }

    /// Current BT settings snapshot (also the configuration the engine would
    /// start with if it has not started yet).
    pub fn bt_settings(&self) -> BtSettings {
        lock(&self.settings).bt.clone()
    }

    /// Store the new settings and, when the engine is already running, apply
    /// them to the session. Before the engine starts the stored snapshot is
    /// used as the launch configuration, so no apply is needed.
    pub fn apply_settings(&self, settings: &AppSettings) {
        *lock(&self.settings) = settings.clone();

        if let Some(engine) = self.engine_if_ready() {
            engine.apply_settings(settings);
        }

        // Leaving lightweight mode means "always-on": the index-sync loop
        // notices and warms the engine up (it owns an `Arc<Self>`, which
        // `apply_settings` does not). Entering lightweight mode needs no action
        // — the idle supervisor unloads the session on its next tick when there
        // is no work.
    }

    /// Graceful shutdown. Safe to call before the engine was ever created: the
    /// in-flight warm-up observes the flag, tears down whatever it just created
    /// and returns without leaving a session behind.
    pub async fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
        // Only the index mirror is aborted: the idle supervisor is left to
        // finish (or skip) so a teardown it already started is not interrupted
        // halfway through `session.shutdown()`.
        self.abort_index_sync();

        // Wait for the warm-up so it cannot publish the engine after we looked.
        let startup_task = lock(&self.startup_task).take();
        if let Some(task) = startup_task
            && tokio::time::timeout(STARTUP_SHUTDOWN_GRACE, task)
                .await
                .is_err()
        {
            tracing::warn!(
                "BT engine warm-up did not finish within {}s; continuing shutdown",
                STARTUP_SHUTDOWN_GRACE.as_secs()
            );
        }

        // The lifecycle guard makes this wait for any in-flight creation.
        let _guard = self.lifecycle.lock().await;
        let engine = self.engine.write().take();
        if let Some(engine) = engine {
            self.sync_index_with(&engine).await;
            engine.shutdown().await;
        }
    }

    // ── BT-specific operations (used by Dispatcher / Aria2 RPC) ─────────

    /// BT engine runtime status. Reports a disconnected status while the engine
    /// is still warming up (the UI hides the BT status pills for it).
    pub fn runtime_status(&self) -> BtRuntimeStatus {
        match self.engine_if_ready() {
            Some(engine) => engine.runtime_status(),
            None => BtRuntimeStatus {
                connected: false,
                dht_enabled: self.bt_settings().dht_enabled,
                dht_nodes: None,
                torrent_count: 0,
                peer_count: 0,
                upload_speed_bytes_per_second: None,
                uploaded_bytes: 0,
                updated_at_ms: now_ms(),
                seed_count: None,
                leech_count: None,
            },
        }
    }

    /// Set per-torrent speed limits. No-op before the engine is ready: no
    /// torrent can exist yet.
    pub fn set_speed_limit(
        &self,
        info_hash: Id20,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    ) {
        if let Some(engine) = self.engine_if_ready() {
            engine.set_speed_limit(info_hash, download_limit_bps, upload_limit_bps);
        }
    }

    /// Peers of a torrent; empty while the engine is still warming up.
    pub fn get_peers(&self, info_hash: Id20) -> Result<Vec<BtPeerInfo>> {
        match self.engine_if_ready() {
            Some(engine) => engine.get_peers(info_hash),
            None => Ok(Vec::new()),
        }
    }

    /// Trackers of a torrent; empty while the engine is still warming up.
    pub fn get_trackers(&self, info_hash: Id20) -> Result<Vec<BtTrackerInfo>> {
        match self.engine_if_ready() {
            Some(engine) => engine.get_trackers(info_hash),
            None => Ok(Vec::new()),
        }
    }

    /// Piece states of a torrent; empty while the engine is still warming up.
    pub fn get_pieces(&self, info_hash: Id20) -> Result<Vec<BtPieceInfo>> {
        match self.engine_if_ready() {
            Some(engine) => engine.get_pieces(info_hash),
            None => Ok(Vec::new()),
        }
    }

    /// File status of a torrent; empty while the engine is still warming up.
    pub fn get_torrent_files(&self, info_hash: Id20) -> Result<Vec<BtFileStatus>> {
        match self.engine_if_ready() {
            Some(engine) => engine.get_torrent_files(info_hash),
            None => Ok(Vec::new()),
        }
    }

    /// Preview a .torrent file without adding it to the session.
    pub async fn preview_torrent(&self, source: &str) -> Result<Vec<TorrentFileEntry>> {
        self.engine().await?.preview_torrent(source).await
    }

    /// Change the file selection of a torrent.
    pub async fn update_torrent_files(
        &self,
        info_hash: Id20,
        included_indices: Vec<usize>,
    ) -> Result<()> {
        self.engine()
            .await?
            .update_torrent_files(info_hash, included_indices)
            .await
    }

    // ── Internal helpers ────────────────────────────────────────────────

    fn lightweight_mode(&self) -> bool {
        lock(&self.settings).bt.lightweight_mode
    }

    /// Whether the BT backend has any resume file on disk.
    fn resume_files_present(&self) -> bool {
        let resume_dir = self.state_dir.join("resume");
        if resume_dir.join("session.json").exists() {
            return true;
        }
        let has_json_in = |dir: &std::path::Path| -> bool {
            std::fs::read_dir(dir)
                .map(|entries| {
                    entries.flatten().any(|entry| {
                        entry.path().extension().and_then(std::ffi::OsStr::to_str) == Some("json")
                    })
                })
                .unwrap_or(false)
        };
        has_json_in(&resume_dir) || has_json_in(&resume_dir.join("torrents"))
    }

    /// Create the BT session and its background loops.
    async fn create_engine(&self) -> Result<Arc<RqbitBtBackend>> {
        take_legacy_resume_files(&self.state_dir);
        let settings = lock(&self.settings).clone();
        let engine = Arc::new(
            RqbitBtBackend::new(
                &settings,
                self.state_dir.clone(),
                self.default_output_dir.clone(),
                self.event_bus.clone(),
                self.active_bt_count.clone(),
                self.max_concurrent_bt.clone(),
            )
            .await?,
        );
        engine.clone().spawn_upload_policy_loop();
        engine.clone().spawn_anti_leech_loop();
        engine.setup_alert_bridge().await;
        tracing::info!("BT engine ready");
        Ok(engine)
    }

    /// Unload the engine because lightweight mode found no work for it.
    ///
    /// Serialized with creation/teardown by the lifecycle mutex, and re-checks
    /// activity under it so a task that arrived during the check keeps the
    /// session alive.
    async fn stop_idle_engine(&self) {
        let _guard = self.lifecycle.lock().await;
        let Some(engine) = self.engine.read().clone() else {
            return;
        };
        match engine.has_active_torrents().await {
            Ok(false) => {}
            // Work appeared between the supervisor's check and this one.
            Ok(true) => return,
            Err(e) => {
                tracing::debug!("BT idle re-check failed, keeping engine up: {e}");
                return;
            }
        }

        // Re-check activity under `self.engine.write()` so an `engine()` fast-path
        // caller cannot read `Some(engine)` while we unload it.
        let engine = {
            let mut slot = self.engine.write();
            if !idle_settle_elapsed(
                self.last_activity_ms.load(Ordering::Acquire),
                now_ms(),
                IDLE_SETTLE.as_millis() as u64,
            ) {
                return;
            }
            slot.take()
        };
        if let Some(engine) = engine {
            tracing::info!("BT lightweight mode: engine idle, shutting the session down");
            self.sync_index_with(&engine).await;
            engine.shutdown().await;
        }
    }

    /// Abort the index mirror loop (best effort).
    fn abort_index_sync(&self) {
        if let Some(handle) = lock(&self.index_task).take() {
            handle.abort();
        }
    }

    /// Mirror the running engine's task list into the persisted index.
    async fn sync_index(&self) {
        let Some(engine) = self.engine_if_ready() else {
            return;
        };
        self.sync_index_with(&engine).await;
    }

    /// Store `engine.list()` into the index, replacing the previous snapshot.
    async fn sync_index_with(&self, engine: &Arc<RqbitBtBackend>) {
        match engine.list().await {
            Ok(summaries) => self.store_index(summaries).await,
            Err(e) => tracing::debug!("BT index sync skipped: {e}"),
        }
    }

    /// Persist the summaries off the async executor (SQLite is blocking).
    async fn store_index(&self, summaries: Vec<DownloadSummary>) {
        if let Err(e) = self.db.replace_bt_tasks_async(summaries).await {
            tracing::warn!("failed to persist the BT task index: {e}");
        }
    }

    /// Refresh the index and re-publish every task as an `Updated` event.
    ///
    /// Session creation fires `TorrentAdded` for restored torrents before the
    /// alert bridge subscribes, so without this the restored rows would only
    /// appear on the next 2 s progress tick (or never, if the bridge was not
    /// yet subscribed when the desktop UI bound its listener).
    async fn refresh_index_and_publish(&self, engine: &Arc<RqbitBtBackend>) {
        let summaries = match engine.list().await {
            Ok(summaries) => summaries,
            Err(e) => {
                tracing::warn!("BT engine ready but its task list could not be read: {e}");
                return;
            }
        };
        for summary in &summaries {
            self.event_bus.publish(DownloadEvent::Updated {
                summary: Box::new(summary.clone()),
            });
        }
        self.store_index(summaries).await;
    }

    /// Serve the persisted index while the engine is unloaded.
    ///
    /// Volatile transfer metrics are zeroed so a stopped session cannot show a
    /// stale speed, peer count or ETA.
    async fn index_summaries(&self) -> Result<Vec<DownloadSummary>> {
        let mut summaries = self.db.list_bt_tasks_async().await.map_err(|e| {
            DownloadError::Torrent(format!("failed to read the BT task index: {e}"))
        })?;
        for summary in &mut summaries {
            summary.speed_bytes_per_second = None;
            summary.upload_speed_bytes_per_second = None;
            summary.connection_count = 0;
            summary.peer_count = Some(0);
            summary.eta_seconds = None;
            summary.upload_status = Some(BtUploadStatus::Idle);
        }
        Ok(summaries)
    }

    /// Cache the first startup failure and surface it to the user.
    fn record_startup_failure(&self, error: DownloadError) -> DownloadError {
        let message = error.to_string();
        let first_failure = {
            let mut slot = lock(&self.startup_error);
            if slot.is_none() {
                *slot = Some(message.clone());
                true
            } else {
                false
            }
        };
        if first_failure {
            tracing::error!("BT engine failed to start: {message}");
            self.event_bus.publish(DownloadEvent::Warning {
                id: String::new(),
                message: format!("BT engine failed to start: {message}"),
            });
        }
        DownloadError::Torrent(message)
    }
}

mod backend;

#[cfg(test)]
mod tests;
