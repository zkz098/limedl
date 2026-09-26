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
//! | `list`                                                               | returns an empty list            |
//! | peer/tracker/piece/file/`runtime_status` queries                     | empty / disconnected result      |
//! | `update_settings`                                                    | stores settings for the warm-up  |
//! | `shutdown`                                                           | cancels the in-flight warm-up or shuts a running engine down |
//!
//! `list` and the read-only queries deliberately do **not** block: the task
//! list is loaded before the desktop window is shown, and it must not wait on
//! the BT session. The native client re-reads the list once the engine reports
//! ready so torrents restored from resume data appear as soon as they exist.
//!
//! A failed start is cached in `startup_error`: later calls fail fast with the
//! original message instead of silently re-attempting session creation (the
//! session holds exclusive resources such as the configured listen port).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use irontide::core::Id20;
use parking_lot::Mutex;
use tokio::sync::OnceCell;

use super::IrontideBtBackend;
use crate::error::{DownloadError, Result};
use crate::event_bus::{DownloadEvent, EventBus};
use crate::protocol::DownloadBackend;
use crate::types::{
    AppSettings, BtFileStatus, BtPeerInfo, BtPieceInfo, BtRuntimeStatus, BtSettings, BtTrackerInfo,
    DownloadSnapshot, DownloadSummary, StartDownloadRequest, TaskId, TorrentFileEntry,
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

/// BitTorrent backend with lazily started engine.
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
    /// Active BT download counter, shared with `DownloadManager`.
    pub(crate) active_bt_count: Arc<AtomicUsize>,
    /// Maximum concurrent BT downloads, shared with `DownloadManager`.
    pub(crate) max_concurrent_bt: Arc<AtomicUsize>,
    /// The started engine; empty until the warm-up finishes.
    engine: OnceCell<Arc<IrontideBtBackend>>,
    /// Join handle of the background warm-up started by `spawn_startup`.
    startup_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Cached startup failure message, so later calls fail fast.
    startup_error: Mutex<Option<String>>,
    /// Set by `shutdown`; new work is rejected and a late warm-up tears itself
    /// down again.
    shutting_down: AtomicBool,
    /// Guards the one-time engine shutdown (warm-up and `shutdown` may race).
    engine_shutdown: AtomicBool,
}

impl LazyBtBackend {
    /// Record the launch configuration. Does **not** touch the network or disk.
    pub fn new(
        settings: &AppSettings,
        state_dir: PathBuf,
        default_output_dir: PathBuf,
        event_bus: Arc<EventBus>,
        active_bt_count: Arc<AtomicUsize>,
        max_concurrent_bt: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            settings: Mutex::new(settings.clone()),
            state_dir,
            default_output_dir,
            event_bus,
            active_bt_count,
            max_concurrent_bt,
            engine: OnceCell::new(),
            startup_task: Mutex::new(None),
            startup_error: Mutex::new(None),
            shutting_down: AtomicBool::new(false),
            engine_shutdown: AtomicBool::new(false),
        }
    }

    /// Kick off the background engine warm-up. Idempotent; requires a tokio
    /// runtime. Returns immediately — `bootstrap()` does not wait.
    pub fn spawn_startup(self: &Arc<Self>) {
        let mut slot = lock(&self.startup_task);
        if slot.is_some() || self.engine.get().is_some() {
            return;
        }
        let this = self.clone();
        *slot = Some(tokio::spawn(async move {
            // `wait_ready` logs and caches failures; nothing else to do here.
            let _ = this.wait_ready().await;
        }));
    }

    /// Await the engine, creating it if nobody started the warm-up yet.
    ///
    /// Returns the cached startup error when a previous attempt failed. Racing
    /// callers are deduplicated by the `OnceCell`.
    pub async fn engine(&self) -> Result<Arc<IrontideBtBackend>> {
        if let Some(message) = lock(&self.startup_error).clone() {
            return Err(DownloadError::Torrent(message));
        }
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(DownloadError::Torrent(ENGINE_SHUTTING_DOWN.into()));
        }

        match self.engine.get_or_try_init(|| self.create_engine()).await {
            Ok(engine) => {
                // `shutdown` may have completed between the flag check above and
                // the cell being filled; the late engine must not survive it.
                if self.shutting_down.load(Ordering::Acquire) {
                    self.shutdown_engine(engine).await;
                    return Err(DownloadError::Torrent(ENGINE_SHUTTING_DOWN.into()));
                }
                Ok(engine.clone())
            }
            Err(e) => Err(self.record_startup_failure(e)),
        }
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

    /// The engine, if it is already running. Never blocks.
    pub fn engine_if_ready(&self) -> Option<Arc<IrontideBtBackend>> {
        self.engine.get().cloned()
    }

    /// Whether the engine is up and not shutting down.
    pub fn is_ready(&self) -> bool {
        !self.shutting_down.load(Ordering::Acquire) && self.engine.get().is_some()
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
        if let Some(engine) = self.engine.get() {
            engine.apply_settings(settings);
        }
    }

    /// Graceful shutdown. Safe to call before the engine was ever created: the
    /// in-flight warm-up observes the flag, tears down whatever it just created
    /// and returns without leaving a session behind.
    pub async fn shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);

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

        if let Some(engine) = self.engine.get() {
            self.shutdown_engine(engine).await;
        }
    }

    // ── BT-specific operations (used by Dispatcher / Aria2 RPC) ─────────

    /// BT engine runtime status. Reports a disconnected status while the engine
    /// is still warming up (the UI hides the BT status pills for it).
    pub fn runtime_status(&self) -> BtRuntimeStatus {
        match self.engine.get() {
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
        if let Some(engine) = self.engine.get() {
            engine.set_speed_limit(info_hash, download_limit_bps, upload_limit_bps);
        }
    }

    /// Peers of a torrent; empty while the engine is still warming up.
    pub fn get_peers(&self, info_hash: Id20) -> Result<Vec<BtPeerInfo>> {
        match self.engine.get() {
            Some(engine) => engine.get_peers(info_hash),
            None => Ok(Vec::new()),
        }
    }

    /// Trackers of a torrent; empty while the engine is still warming up.
    pub fn get_trackers(&self, info_hash: Id20) -> Result<Vec<BtTrackerInfo>> {
        match self.engine.get() {
            Some(engine) => engine.get_trackers(info_hash),
            None => Ok(Vec::new()),
        }
    }

    /// Piece states of a torrent; empty while the engine is still warming up.
    pub fn get_pieces(&self, info_hash: Id20) -> Result<Vec<BtPieceInfo>> {
        match self.engine.get() {
            Some(engine) => engine.get_pieces(info_hash),
            None => Ok(Vec::new()),
        }
    }

    /// File status of a torrent; empty while the engine is still warming up.
    pub fn get_torrent_files(&self, info_hash: Id20) -> Result<Vec<BtFileStatus>> {
        match self.engine.get() {
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

    /// Tear the engine down exactly once (warm-up and `shutdown` can race).
    async fn shutdown_engine(&self, engine: &Arc<IrontideBtBackend>) {
        if self.engine_shutdown.swap(true, Ordering::AcqRel) {
            return;
        }
        engine.shutdown().await;
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
        self.engine().await?.cancel(info_hash).await
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.remove(info_hash).await
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Bt(info_hash) = *task_id else {
            return Err(DownloadError::NotFound);
        };
        self.engine().await?.purge(info_hash).await
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

    /// List restored/running torrents. Returns an empty list while the engine
    /// is still warming up instead of blocking startup on the session.
    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        match self.engine_if_ready() {
            Some(engine) => engine.list().await,
            None => Ok(Vec::new()),
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
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(5)),
        ));
        (tmp, backend)
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
}
