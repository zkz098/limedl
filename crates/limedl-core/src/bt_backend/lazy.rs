//! Lazy startup wrapper around [`IrontideBtBackend`].
//!
//! Creating the irontide session is not cheap: it binds TCP/uTP sockets, starts
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
//! one. While the engine is down, [`LazyBtBackend::list`] serves rows from the
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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use irontide::core::Id20;
use parking_lot::{Mutex, RwLock};

use super::IrontideBtBackend;
use crate::database::Database;
use crate::error::{DownloadError, Result};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::protocol::DownloadBackend;
use crate::types::{
    AppSettings, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtSettings, BtTrackerInfo,
    BtUploadStatus, DownloadSnapshot, DownloadState, DownloadSummary, StartDownloadRequest, TaskId,
    TorrentFileEntry,
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
        DownloadState::Downloading | DownloadState::Queued | DownloadState::Retrying
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
    pub(crate) active_bt_count: Arc<AtomicUsize>,
    /// Maximum concurrent BT downloads, shared with `DownloadManager`.
    pub(crate) max_concurrent_bt: Arc<AtomicUsize>,
    /// The started engine; empty while the engine is unloaded.
    ///
    /// A `parking_lot` lock (not an async one) so [`Self::engine_if_ready`] can
    /// stay synchronous on latency-sensitive read paths.
    engine: RwLock<Option<Arc<IrontideBtBackend>>>,
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
        active_bt_count: Arc<AtomicUsize>,
        max_concurrent_bt: Arc<AtomicUsize>,
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
        let this = self.clone();
        *slot = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(INDEX_SYNC_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                // Self-heal eager mode: leaving lightweight mode at runtime
                // asks for an always-on engine, and `apply_settings(&self)`
                // cannot spawn the warm-up because it only holds `&self`.
                // A cached startup error deliberately blocks the retry, matching
                // the "no silent retry after a failed session" contract.
                if !this.lightweight_mode()
                    && !this.is_ready()
                    && this.startup_error().is_none()
                {
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
        let this = self.clone();
        *slot = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(IDLE_CHECK_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                if this.shutting_down.load(Ordering::Acquire) {
                    // Leave the task instead of aborting it: `shutdown` may be
                    // waiting on a teardown this loop is in the middle of.
                    break;
                }
                if !this.lightweight_mode() {
                    continue;
                }
                // A task op merely *handing out* the engine (before its torrent
                // is in the session) must not be mistaken for idleness.
                let idle_for_ms = now_ms().saturating_sub(
                    this.last_activity_ms.load(Ordering::Acquire),
                );
                if idle_for_ms < u64::try_from(IDLE_SETTLE.as_millis()).unwrap_or(u64::MAX) {
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
    pub async fn engine(&self) -> Result<Arc<IrontideBtBackend>> {
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
    pub fn engine_if_ready(&self) -> Option<Arc<IrontideBtBackend>> {
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

    /// Whether irontide has any `.resume` file on disk.
    ///
    /// `irontide::resume_file::scan_resume_dir` reads `<resume_dir>/torrents`,
    /// so the probe mirrors that layout without paying for a session.
    fn resume_files_present(&self) -> bool {
        let torrents_dir = self.state_dir.join("resume").join("torrents");
        std::fs::read_dir(torrents_dir)
            .map(|entries| {
                entries.flatten().any(|entry| {
                    entry.path().extension().and_then(|e| e.to_str()) == Some("resume")
                })
            })
            .unwrap_or(false)
    }

    /// Create the irontide session and its background loops.
    async fn create_engine(&self) -> Result<Arc<IrontideBtBackend>> {
        let settings = lock(&self.settings).clone();
        let engine = Arc::new(
            IrontideBtBackend::new(
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

        tracing::info!("BT lightweight mode: engine idle, shutting the session down");
        let engine = self.engine.write().take();
        if let Some(engine) = engine {
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
    async fn sync_index_with(&self, engine: &Arc<IrontideBtBackend>) {
        match engine.list().await {
            Ok(summaries) => self.store_index(summaries).await,
            Err(e) => tracing::debug!("BT index sync skipped: {e}"),
        }
    }

    /// Persist the summaries off the async executor (SQLite is blocking).
    async fn store_index(&self, summaries: Vec<DownloadSummary>) {
        let db = self.db.clone();
        match tokio::task::spawn_blocking(move || db.replace_bt_tasks(&summaries)).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!("failed to persist the BT task index: {e}"),
            Err(e) => tracing::warn!("BT task index writer panicked: {e}"),
        }
    }

    /// Refresh the index and re-publish every task as an `Updated` event.
    ///
    /// Session creation fires `TorrentAdded` for restored torrents before the
    /// alert bridge subscribes, so without this the restored rows would only
    /// appear on the next 2 s progress tick (or never, if the bridge was not
    /// yet subscribed when the desktop UI bound its listener).
    async fn refresh_index_and_publish(&self, engine: &Arc<IrontideBtBackend>) {
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
    fn index_summaries(&self) -> Result<Vec<DownloadSummary>> {
        let mut summaries = self
            .db
            .list_bt_tasks()
            .map_err(|e| DownloadError::Torrent(format!("failed to read the BT task index: {e}")))?;
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

#[async_trait]
impl DownloadBackend for LazyBtBackend {
    async fn start(&self, request: StartDownloadRequest) -> Result<TaskId> {
        let info_hash = self.engine().await?.start(request).await?;
        Ok(TaskId::Bt(info_hash))
    }

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.pause(info_hash).await
    }

    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.resume(info_hash).await
    }

    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        let snapshot = self.engine().await?.cancel(info_hash).await?;
        self.db
            .delete_bt_task(&info_hash.to_hex())
            .unwrap_or_else(|e| tracing::warn!("failed to drop the BT index row: {e}"));
        Ok(snapshot)
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        let snapshot = self.engine().await?.remove(info_hash).await?;
        self.db
            .delete_bt_task(&info_hash.to_hex())
            .unwrap_or_else(|e| tracing::warn!("failed to drop the BT index row: {e}"));
        Ok(snapshot)
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        let snapshot = self.engine().await?.purge(info_hash).await?;
        self.db
            .delete_bt_task(&info_hash.to_hex())
            .unwrap_or_else(|e| tracing::warn!("failed to drop the BT index row: {e}"));
        Ok(snapshot)
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.open_in_explorer(info_hash).await
    }

    async fn open_file(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.open_file(info_hash).await
    }

    async fn open_dir(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.open_dir(info_hash).await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.status(info_hash).await
    }

    /// List running torrents from the engine, or the persisted index while the
    /// engine is unloaded instead of blocking startup on the session.
    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        match self.engine_if_ready() {
            Some(engine) => engine.list().await,
            None => self.index_summaries(),
        }
    }

    async fn update_settings(&self, settings: &AppSettings) -> Result<()> {
        self.apply_settings(settings);
        Ok(())
    }

    async fn shutdown(&self) {
        LazyBtBackend::shutdown(self).await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    /// Settings that start a session without any network I/O.
    fn offline_settings() -> AppSettings {
        let mut settings = AppSettings::default();
        settings.bt.dht_enabled = false;
        settings.bt.enable_lsd = false;
        settings.bt.upnp_enabled = false;
        settings.bt.enable_natpmp = false;
        settings.bt.enable_ipv6 = false;
        settings.bt.enable_pex = false;
        settings.bt.enable_utp = false;
        settings.bt.enable_holepunch = false;
        settings.bt.enable_fast_extension = false;
        settings.bt.enable_web_seed = false;
        settings.bt.listen_port = Some(0);
        settings
    }

    fn make_backend(settings: &AppSettings) -> (tempfile::TempDir, Arc<LazyBtBackend>) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state_dir = tmp.path().join("state");
        let output_dir = tmp.path().join("out");
        std::fs::create_dir_all(&state_dir).expect("create state dir");
        std::fs::create_dir_all(&output_dir).expect("create output dir");

        let backend = Arc::new(LazyBtBackend::new(
            settings,
            state_dir,
            output_dir,
            Arc::new(EventBus::new(64)),
            Arc::new(Database::open_in_memory().expect("in-memory db")),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(5)),
        ));
        (tmp, backend)
    }

    fn sample_summary(id: &str, state: DownloadState) -> DownloadSummary {
        DownloadSummary {
            id: id.to_string(),
            kind: crate::types::TaskKind::Bt,
            state,
            url: id.to_string(),
            file_name: "sample".into(),
            destination_path: "/tmp/sample".into(),
            total_bytes: Some(2048),
            downloaded_bytes: 1024,
            connection_count: 7,
            thread_mode: crate::types::ThreadMode::Fixed,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile: None,
            thread_note: None,
            speed_bytes_per_second: Some(4096.0),
            eta_seconds: Some(42),
            uploaded_bytes: Some(0),
            upload_speed_bytes_per_second: Some(12.0),
            peer_count: Some(7),
            upload_status: Some(BtUploadStatus::Uploading),
            info_hash: Some(id.to_string()),
            expected_checksum: None,
            error: None,
            cdn_accelerated: false,
            cdn_node_ip: None,
            created_at_ms: 123,
            priority: crate::types::Priority::Normal,
            seed_count: Some(1),
            leech_count: Some(2),
            download_limit_bps: None,
            upload_limit_bps: None,
            chunks: Vec::new(),
            mirror_url: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_only_calls_do_not_start_the_engine() {
        let (_tmp, backend) = make_backend(&offline_settings());

        assert!(!backend.is_ready(), "engine must not start on construction");
        assert!(backend.startup_error().is_none());

        let list = DownloadBackend::list(backend.as_ref())
            .await
            .expect("list must not fail before warm-up");
        assert!(list.is_empty());
        assert!(backend.get_peers(Id20::from([7u8; 20])).unwrap().is_empty());

        let status = backend.runtime_status();
        assert!(!status.connected, "not-ready status must be disconnected");
        assert_eq!(status.torrent_count, 0);
        assert!(
            !backend.is_ready(),
            "read-only calls must not start the engine"
        );

        backend.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn list_serves_the_index_with_volatile_metrics_zeroed() {
        let (_tmp, backend) = make_backend(&offline_settings());
        let id = "aa".repeat(20);
        backend
            .db
            .replace_bt_tasks(&[sample_summary(&id, DownloadState::Paused)])
            .expect("seed the index");

        let list = DownloadBackend::list(backend.as_ref())
            .await
            .expect("list must fall back to the index");

        assert_eq!(list.len(), 1, "indexed task must be served while the engine is down");
        let row = &list[0];
        assert_eq!(row.id, id);
        assert_eq!(row.state, DownloadState::Paused);
        assert_eq!(row.downloaded_bytes, 1024, "persisted progress is kept");
        assert_eq!(row.speed_bytes_per_second, None);
        assert_eq!(row.upload_speed_bytes_per_second, None);
        assert_eq!(row.eta_seconds, None);
        assert_eq!(row.connection_count, 0);
        assert_eq!(row.peer_count, Some(0));
        assert!(!backend.is_ready(), "serving the index must not start the engine");

        backend.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn has_unfinished_tasks_tracks_index_and_resume_files() {
        let (_tmp, backend) = make_backend(&offline_settings());
        assert!(
            !backend.has_unfinished_tasks(),
            "empty index and no resume files means no work"
        );

        backend
            .db
            .replace_bt_tasks(&[sample_summary("bb".repeat(20).as_str(), DownloadState::Paused)])
            .expect("seed the index");
        assert!(
            !backend.has_unfinished_tasks(),
            "a paused task alone must not start the engine"
        );

        backend
            .db
            .replace_bt_tasks(&[sample_summary("cc".repeat(20).as_str(), DownloadState::Downloading)])
            .expect("seed the index");
        assert!(backend.has_unfinished_tasks());

        // An empty index with a leftover resume file must still trigger a start.
        backend.db.clear_bt_tasks().expect("clear index");
        let resume_dir = backend.state_dir.join("resume").join("torrents");
        std::fs::create_dir_all(&resume_dir).expect("create resume dir");
        std::fs::write(resume_dir.join("some.resume"), b"data").expect("write resume file");
        assert!(backend.has_unfinished_tasks());

        backend.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shutdown_before_warm_up_never_starts_the_engine() {
        let (_tmp, backend) = make_backend(&offline_settings());

        backend.shutdown().await;

        assert!(!backend.is_ready());
        // Later work is rejected instead of resurrecting the engine.
        let err = match backend.engine().await {
            Ok(_) => panic!("engine must stay down after shutdown"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("shutting down"), "got: {err}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn warm_up_starts_the_engine_in_the_background() {
        let mut settings = offline_settings();
        settings.bt.dht_enabled = false;
        let (_tmp, backend) = make_backend(&settings);

        backend.spawn_startup();

        assert!(
            backend.wait_ready().await,
            "warm-up should bring the engine up"
        );
        assert!(backend.is_ready());
        assert!(
            !backend.runtime_status().dht_enabled,
            "launch settings must be used"
        );

        backend.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn settings_stored_before_warm_up_are_used_at_startup() {
        let settings = offline_settings();
        let (_tmp, backend) = make_backend(&AppSettings::default());

        // The engine would start with DHT enabled; disable it beforehand.
        backend.apply_settings(&settings);
        assert!(!backend.bt_settings().dht_enabled);

        backend.spawn_startup();
        assert!(backend.wait_ready().await);
        assert!(!backend.runtime_status().dht_enabled);

        backend.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn engine_restarts_after_an_idle_shutdown() {
        let settings = offline_settings();
        let (_tmp, backend) = make_backend(&settings);

        assert!(backend.engine().await.is_ok());
        assert!(backend.is_ready());

        // The idle supervisor path: no torrent in the session, so it unloads.
        backend.stop_idle_engine().await;
        assert!(!backend.is_ready(), "idle engine must be unloaded");

        // On-demand work must build a fresh session, not fail on a spent cell.
        assert!(backend.engine().await.is_ok());
        assert!(backend.is_ready());

        backend.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_startup_restarts_after_an_idle_shutdown() {
        let (_tmp, backend) = make_backend(&offline_settings());

        backend.spawn_startup();
        assert!(backend.wait_for_warmup().await);
        backend.stop_idle_engine().await;
        assert!(!backend.is_ready());

        // The finished warm-up handle must not make this a no-op (this is the
        // "leave lightweight mode" path).
        backend.spawn_startup();
        assert!(backend.wait_for_warmup().await);
        assert!(backend.is_ready());

        backend.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wait_for_warmup_does_not_start_the_engine() {
        let (_tmp, backend) = make_backend(&offline_settings());

        assert!(
            !backend.wait_for_warmup().await,
            "no warm-up was requested, so nothing should start"
        );
        assert!(!backend.is_ready());

        backend.spawn_startup();
        assert!(backend.wait_for_warmup().await);

        backend.shutdown().await;
    }
}
