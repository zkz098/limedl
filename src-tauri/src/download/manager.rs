use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use foldhash::HashMap;
use parking_lot::{Mutex, MutexGuard};
use tauri::Emitter;

use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::{Client, StatusCode, Url, header};
use tokio::{
    sync::{Notify, RwLock},
    task::JoinSet,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    aimd::{self, AimdState},
    buffer_pool::BufferPool,
    database::Database,
    error::{DownloadError, Result},
    event_bus::{DownloadEvent, EventBus},
    file_ops::{
        check_disk_space, detect_disk_type, finalize_temp_file, open_download_file,
        reset_download_file, write_all_at,
    },
    http::{
        build_segment_request, extract_total_bytes, header_string, if_range_header,
        infer_file_name, supports_ranges, validate_probe_response, validate_segment_response,
    },
    lock,
    logging::apply_logging_settings,
    manifest::{
        CHUNK_SIZE, ChunkManifest, Manifest, RemoteMetadata, contiguous_prefix_end,
        has_partial_chunk_progress, plan_chunks, resolve_chunk_size, snapshot_from_manifest,
        validators_changed,
    },
    migration::migrate_json_manifests,
    protocol::DownloadBackend,
    rate_limiter::RateLimiter,
    types::{
        AdaptiveProfile, AppSettings, ChecksumMode, ChunkInfo, DiskType, DownloadProgress,
        DownloadSnapshot, DownloadState, DownloadSummary, SchedulerMode,
        StartDownloadRequest, TaskId, TaskKind, ThreadMode,
    },
};

use super::now_ms;
use super::mirror::rewrite as mirror_rewrite;
use super::http_client_factory::build_http_client;
use super::settings::{
    load_settings, normalize_settings, persist_settings, resolve_user_agent,
};

#[path = "http_executor.rs"]
mod http_executor;

pub(crate) const DEFAULT_FIXED_THREADS: usize = 8;
const DEFAULT_RETRIES: u32 = 4;
const PERSIST_INTERVAL: Duration = Duration::from_millis(300);
pub(crate) const MAX_TRADITIONAL_THREADS: usize = 32;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<super::backend_registry::BackendRegistry>,
    pub event_bus: Arc<EventBus>,
    pub rate_limiter: Arc<RateLimiter>,
    pub cdn_accelerator: Arc<super::cdn::CdnAccelerator>,
    pub app_handle: tauri::AppHandle,
    pub rpc_shutdown: Arc<parking_lot::Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
}

impl AppState {
    pub async fn emit_all_downloads(&self) {
        for backend in self.registry.iter() {
            if let Ok(summaries) = backend.list().await {
                for summary in summaries {
                    let _ = self.app_handle.emit("download-updated", &summary);
                }
            }
        }
    }
}

pub struct DownloadManager {
    client: Arc<RwLock<Client>>,
    state_dir: PathBuf,
    settings_path: PathBuf,
    pub(crate) settings: Arc<RwLock<AppSettings>>,
    pub(crate) downloads: Arc<RwLock<HashMap<String, Arc<ManagedDownload>>>>,
    pub(crate) db: Arc<Database>,
    pub(crate) rebalance_notify: Arc<Notify>,
    pub(crate) event_bus: Arc<EventBus>,
    cdn_accelerator: Arc<RwLock<Option<Arc<super::cdn::CdnAccelerator>>>>,
    rate_limiter: Arc<RateLimiter>,
    pub(crate) buffer_pool: Arc<BufferPool>,
    pub(crate) overclock_mode: AtomicBool,
    pub(crate) shutdown_token: CancellationToken,
}

impl Clone for DownloadManager {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            state_dir: self.state_dir.clone(),
            settings_path: self.settings_path.clone(),
            settings: self.settings.clone(),
            downloads: self.downloads.clone(),
            db: self.db.clone(),
            rebalance_notify: self.rebalance_notify.clone(),
            shutdown_token: self.shutdown_token.clone(),
            event_bus: self.event_bus.clone(),
            cdn_accelerator: self.cdn_accelerator.clone(),
            rate_limiter: self.rate_limiter.clone(),
            buffer_pool: self.buffer_pool.clone(),
            overclock_mode: AtomicBool::new(self.overclock_mode.load(Ordering::Relaxed)),
        }
    }
}

/// Merged core of snapshot + manifest, protected by a single Mutex.
/// This eliminates double-lock ordering in hot paths like record_progress().
pub(crate) struct DownloadCore {
    pub(crate) snapshot: DownloadSnapshot,
    pub(crate) manifest: Manifest,
}

pub(crate) struct ManagedDownload {
    pub(crate) core: Mutex<DownloadCore>,
    pub(crate) runtime: Mutex<Option<CancellationToken>>,
    pub(crate) aimd: Mutex<AimdState>,
    pub(crate) stop_notify: Notify,
}

impl ManagedDownload {
    pub(crate) fn lock_core(&self) -> MutexGuard<'_, DownloadCore> {
        lock(&self.core)
    }

    fn lock_runtime(&self) -> MutexGuard<'_, Option<CancellationToken>> {
        self.runtime.lock()
    }

    pub(crate) fn lock_aimd(&self) -> MutexGuard<'_, AimdState> {
        self.aimd.lock()
    }
}

#[derive(Debug)]
enum RunOutcome {
    Finished,
    Paused,
    Canceled,
}

#[derive(Debug)]
enum ChunkWorkerOutcome {
    Finished,
    RestartSingle,
    Paused,
    Canceled,
}

impl DownloadManager {
    pub fn new(
        state_dir: PathBuf,
        rate_limiter: Arc<RateLimiter>,
        event_bus: Arc<EventBus>,
    ) -> Result<Self> {
        fs::create_dir_all(&state_dir)?;

        let settings_path = state_dir
            .parent()
            .unwrap_or(state_dir.as_path())
            .join("settings.json");
        let settings = load_settings(&settings_path)?;
        let client = build_http_client(&settings)?;

        let db_path = state_dir.join("downloads.db");
        let db = Database::open(&db_path)?;
        let db = Arc::new(db);

        migrate_json_manifests(&db, &state_dir)?;

        let io = &settings.io_baseline;
        let buffer_pool = Arc::new(BufferPool::new(
            io.buffer_limit_mb,
            io.game_mode_buffer_mb,
            io.max_parallel_hdd,
            io.game_mode_max_parallel,
        ));

        let manager = Self {
            client: Arc::new(RwLock::new(client)),
            state_dir,
            settings_path,
            settings: Arc::new(RwLock::new(settings)),
            downloads: Arc::new(RwLock::new(HashMap::default())),
            db,
            rebalance_notify: Arc::new(Notify::new()),
            event_bus,
            cdn_accelerator: Arc::new(RwLock::new(None)),
            rate_limiter,
            buffer_pool,
            overclock_mode: AtomicBool::new(false),
            shutdown_token: CancellationToken::new(),
        };

        manager.load_downloads_from_db()?;
        Ok(manager)
    }

    /// Signal the scheduler loop and all active chunk workers to stop gracefully.
    pub async fn shutdown(&self) {
        // Stop the scheduler loop
        self.shutdown_token.cancel();

        // Cancel all active download runtimes so chunk workers exit
        let downloads = self.downloads.read().await;
        for managed in downloads.values() {
            if let Some(token) = managed.lock_runtime().take() {
                token.cancel();
            }
            // Wake the stop_notify so any wait_until_stopped loops break
            managed.stop_notify.notify_one();
        }
    }

    pub async fn settings(&self) -> Result<AppSettings> {
        Ok(self.settings.read().await.clone())
    }

    pub fn initial_settings(&self) -> AppSettings {
        tokio::task::block_in_place(|| self.settings.blocking_read().clone())
    }

    pub fn settings_default_download_dir(&self) -> Option<String> {
        let dir = tokio::task::block_in_place(|| {
            self.settings
                .blocking_read()
                .download
                .default_download_dir
                .clone()
        });
        if dir.is_empty() { None } else { Some(dir) }
    }

    /// Inject the CDN accelerator reference after both manager and accelerator are created.
    pub fn set_cdn_accelerator(&self, acc: Arc<super::cdn::CdnAccelerator>) {
        tokio::task::block_in_place(|| {
            *self.cdn_accelerator.blocking_write() = Some(acc);
        });
    }

    /// Resolve the HTTP client to use for a given URL.
    ///
    /// If CDN acceleration is enabled and an accelerated IP is available, this builds
    /// a domain-specific client that resolves the URL's hostname to the best Cloudflare IP.
    /// Otherwise falls back to the standard client.
    async fn resolve_client(&self, url: &str) -> (Client, bool) {
        // Clone CDN settings under the read lock, then drop it immediately.
        // This prevents blocking update_settings() during the DNS lookup below.
        let (cdn_enabled, cdn_active_ip) = {
            let settings = self.settings.read().await;
            (
                settings.cdn_acceleration.enabled,
                settings.cdn_acceleration.active_ip.clone(),
            )
        };

        if !cdn_enabled {
            tracing::debug!("resolve_client: CDN acceleration disabled");
            return (self.client.read().await.clone(), false);
        }

        if !super::cdn::is_cloudflare_domain(url).await {
            tracing::debug!("resolve_client: domain is not Cloudflare, using standard client");
            return (self.client.read().await.clone(), false);
        }

        let Ok(parsed) = reqwest::Url::parse(url) else {
            tracing::debug!("resolve_client: failed to parse URL: {url}");
            return (self.client.read().await.clone(), false);
        };
        let Some(host) = parsed.host_str() else {
            tracing::debug!("resolve_client: no host in URL: {url}");
            return (self.client.read().await.clone(), false);
        };

        // IP resolution: in-memory accelerator → persisted settings fallback
        let ip = match self.cdn_accelerator.read().await.as_ref() {
            Some(acc) => match acc.active_ip().await {
                Some(ip) => {
                    tracing::debug!("resolve_client: using in-memory active IP: {ip}");
                    Some(ip)
                }
                None => cdn_active_ip
                    .as_deref()
                    .and_then(|s| s.parse::<std::net::Ipv4Addr>().ok()),
            },
            None => cdn_active_ip
                .as_deref()
                .and_then(|s| s.parse::<std::net::Ipv4Addr>().ok()),
        };

        if let Some(ip) = ip {
            // Briefly re-acquire settings read lock for build_accelerated_client
            // which needs proxy and user-agent settings.
            let settings = self.settings.read().await;
            match super::cdn::build_accelerated_client(host, ip, &settings) {
                Ok(accelerated) => {
                    tracing::info!("resolve_client: CDN acceleration active for {host} via {ip}");
                    return (accelerated, true);
                }
                Err(e) => {
                    tracing::warn!(
                        "resolve_client: failed to build accelerated client for {host}: {e}, falling back to standard"
                    );
                }
            }
        } else {
            tracing::debug!("resolve_client: no active IP available for {host}");
        }

        (self.client.read().await.clone(), false)
    }

    pub async fn update_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        let normalized = normalize_settings(settings)?;
        self.rate_limiter
            .set_rate(normalized.global_speed_limit_bps);
        self.buffer_pool.update_limits(
            normalized.io_baseline.buffer_limit_mb,
            normalized.io_baseline.game_mode_buffer_mb,
            normalized.io_baseline.max_parallel_hdd,
            normalized.io_baseline.game_mode_max_parallel,
        );
        self.buffer_pool.set_game_mode(normalized.io_baseline.game_mode);
        let next_client = build_http_client(&normalized)?;

        persist_settings(&self.settings_path, &normalized).await?;
        *self.settings.write().await = normalized.clone();
        *self.client.write().await = next_client;
        apply_logging_settings(&normalized.logging, &self.state_dir).map_err(|error| {
            DownloadError::InvalidResponse(format!("failed to apply logging settings: {error}"))
        })?;
        self.rebalance_allocations().await?;
        self.rebalance_notify.notify_waiters();

        Ok(normalized)
    }

    pub async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        let url = Url::parse(&request.url)
            .map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(DownloadError::UnsupportedScheme);
        }

        let settings = self.settings.read().await.clone();
        let download_id = Uuid::new_v4().to_string();
        let user_agent = resolve_user_agent(
            request.user_agent.as_deref(),
            &settings.download.default_user_agent,
        )?;
        let destination_dir = PathBuf::from(&request.destination_dir);
        if destination_dir.as_os_str().is_empty() {
            return Err(DownloadError::InvalidResponse(String::from(
                "download destination directory is not set",
            )));
        }
        if !destination_dir.is_absolute() {
            return Err(DownloadError::InvalidResponse(String::from(
                "download destination directory must be an absolute path",
            )));
        }
        fs::create_dir_all(&destination_dir)?;

        let chosen_name = request
            .file_name
            .clone()
            .unwrap_or_else(|| initial_file_name_from_url(&request.url));
        let safe_name = sanitize_filename::sanitize(&chosen_name);
        if safe_name.is_empty() {
            return Err(DownloadError::MissingFileName);
        }

        let destination_path = unique_destination_path(&destination_dir, &safe_name);
        let temp_path = self.state_dir.join(format!("{download_id}.part"));
        let supports_parallel = true;
        let (thread_mode, requested_thread_count, desired_thread_count, adaptive_profile) =
            resolve_thread_settings(&settings, &request, supports_parallel);
        // Validate: if expected_checksum is provided, checksum_mode must not be None
        if request.expected_checksum.is_some() && request.checksum == Some(ChecksumMode::None) {
            return Err(DownloadError::InvalidRequest(
                "checksum_mode is required when expected_checksum is provided".into(),
            ));
        }

        let thread_note = Some(String::from("等待服务器响应"));
        let now = now_ms();

        let mut manifest = Manifest {
            id: download_id.clone(),
            url: request.url.clone(),
            final_url: request.url.clone(),
            user_agent,
            destination_dir: destination_dir.to_string_lossy().to_string(),
            file_name: safe_name.clone(),
            file_name_locked: request.file_name.is_some(),
            destination_path: destination_path.to_string_lossy().to_string(),
            temp_path: temp_path.to_string_lossy().to_string(),
            total_bytes: None,
            downloaded_bytes: 0,
            supports_ranges: false,
            chunk_size: CHUNK_SIZE,
            connection_count: 0,
            thread_mode,
            requested_thread_count,
            desired_thread_count,
            allocated_thread_count: Some(0),
            adaptive_profile_snapshot: adaptive_profile,
            thread_note,
            etag: None,
            last_modified: None,
            state: DownloadState::Queued,
            checksum_mode: request.checksum.unwrap_or_default(),
            checksum: None,
            expected_checksum: request.expected_checksum.clone(),
            error: None,
            created_at_ms: now,
            updated_at_ms: now,
            chunks: Vec::new(),
            cdn_accelerated: false,
            mirror_url: None,
            mirror_urls: Vec::new(),
            current_mirror_index: 0,
        };

        // If mirror URLs were populated by the commands layer, activate mirror mode
        if let Some(ref mirror_urls) = request.mirror_urls
            && !mirror_urls.is_empty()
        {
            manifest.mirror_urls = mirror_urls.clone();
            manifest.mirror_url = Some(mirror_urls[0].clone());
            manifest.final_url = mirror_urls[0].clone();
            manifest.current_mirror_index = 0;
        }

        let snapshot = snapshot_from_manifest(&manifest);
        let managed = Arc::new(ManagedDownload {
            core: Mutex::new(DownloadCore { snapshot, manifest }),
            runtime: Mutex::new(None),
            aimd: Mutex::new(AimdState::initial(adaptive_profile, desired_thread_count)),
            stop_notify: Notify::new(),
        });

        self.persist(managed.clone()).await?;
        self.downloads
            .write()
            .await
            .insert(download_id.clone(), managed.clone());

        self.spawn_download(managed, request.max_retries.unwrap_or(DEFAULT_RETRIES))
            .await?;
        self.rebalance_allocations().await?;
        self.rebalance_notify.notify_waiters();

        Ok(download_id)
    }

    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        {
            let mut core = managed.lock_core();
            if !matches!(
                core.snapshot.state,
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Queued
            ) {
                return Ok(core.snapshot.clone());
            }
            core.snapshot.state = DownloadState::Paused;
            core.snapshot.connection_count = 0;
            core.snapshot.allocated_thread_count = Some(0);
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Paused;
            core.manifest.connection_count = 0;
            core.manifest.allocated_thread_count = Some(0);
            core.manifest.updated_at_ms = now_ms();
        }

        let token = { managed.lock_runtime().clone() };
        if let Some(token) = token {
            token.cancel();
        }

        self.wait_until_stopped(&managed).await;
        self.persist(managed.clone()).await?;
        self.rebalance_allocations().await?;
        self.rebalance_notify.notify_waiters();
        Ok(self.build_snapshot(managed))
    }

    pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        {
            let mut core = managed.lock_core();
            if core.snapshot.state == DownloadState::Completed {
                return Ok(core.snapshot.clone());
            }
            core.snapshot.state = DownloadState::Canceled;
            core.snapshot.connection_count = 0;
            core.snapshot.allocated_thread_count = Some(0);
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Canceled;
            core.manifest.connection_count = 0;
            core.manifest.allocated_thread_count = Some(0);
            core.manifest.updated_at_ms = now_ms();
        }
        let token = { managed.lock_runtime().clone() };
        if let Some(token) = &token {
            token.cancel();
        }
        if token.is_some() {
            self.wait_until_stopped(&managed).await;
        }
        self.cleanup_files(&managed)?;
        self.downloads.write().await.remove(download_id);
        self.db
            .delete_download(download_id)
            .context("failed to delete canceled download from database")?;
        self.rebalance_allocations().await?;
        self.rebalance_notify.notify_waiters();
        Ok(self.build_snapshot(managed))
    }

    pub async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.remove_internal(download_id, false).await
    }

    pub async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.remove_internal(download_id, true).await
    }

    pub async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        let managed = self.get(download_id).await?;
        let manifest = managed.lock_core().manifest.clone();
        let destination_path = PathBuf::from(&manifest.destination_path);
        let directory_path = PathBuf::from(&manifest.destination_dir);

        if destination_path.exists() {
            #[cfg(windows)]
            {
                Command::new("explorer")
                    .arg(format!("/select,{}", destination_path.display()))
                    .spawn()?;
            }
            return Ok(());
        }

        if directory_path.exists() {
            #[cfg(windows)]
            {
                Command::new("explorer").arg(&directory_path).spawn()?;
            }
            return Ok(());
        }

        Err(DownloadError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "download location does not exist",
        )))
    }

    pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        // Phase 1: check state (read-only, drop lock before fallible ops)
        {
            let core = managed.lock_core();
            if matches!(
                core.snapshot.state,
                DownloadState::Downloading
                    | DownloadState::Retrying
                    | DownloadState::Queued
                    | DownloadState::Verifying
            ) {
                return Err(DownloadError::AlreadyRunning);
            }
            if matches!(core.snapshot.state, DownloadState::Canceled) {
                return Err(DownloadError::Canceled);
            }
            if matches!(core.snapshot.state, DownloadState::Completed) {
                return Err(DownloadError::NotResumable);
            }
        }

        // Phase 2: refresh mirror URL list from current settings and
        // set Queued state on both snapshot and manifest.
        // NOTE: extract the original URL first, then drop the lock before
        // calling mirror_urls_for() — MutexGuard is not Send and cannot be
        // held across an await point.
        let original_url = {
            let core = managed.lock_core();
            core.manifest.url.clone()
        };
        let new_mirror_urls = self.mirror_urls_for(&original_url).await;
        {
            let mut core = managed.lock_core();
            if new_mirror_urls.len() > 1 {
                // Try to preserve current mirror position across settings changes
                let current_url = core.manifest.mirror_url.as_deref();
                let new_idx = current_url
                    .and_then(|cur| new_mirror_urls.iter().position(|u| u == cur))
                    .unwrap_or(0);
                core.manifest.mirror_urls = new_mirror_urls;
                core.manifest.current_mirror_index = new_idx;
                if let Some(url) = core.manifest.mirror_urls.get(new_idx).cloned() {
                    core.manifest.mirror_url = Some(url.clone());
                    core.manifest.final_url = url;
                }
            }
            core.snapshot.state = DownloadState::Queued;
            core.snapshot.error = None;
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Queued;
            core.manifest.error = None;
            core.manifest.updated_at_ms = now_ms();
        }
        // Phase 3: reset AIMD if adaptive (reads manifest + aimd)
        {
            let manifest = managed.lock_core().manifest.clone();
            if manifest.thread_mode == ThreadMode::Adaptive {
                let mut aimd = managed.lock_aimd();
                *aimd = AimdState::initial(
                    manifest.adaptive_profile_snapshot,
                    manifest.desired_thread_count,
                );
            }
        }

        self.spawn_download(managed.clone(), DEFAULT_RETRIES)
            .await?;
        self.rebalance_allocations().await?;
        self.rebalance_notify.notify_waiters();
        Ok(self.build_snapshot(managed))
    }

    pub async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        Ok(self.build_snapshot(managed))
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let downloads = self.downloads.read().await;
        let mut items = downloads
            .values()
            .cloned()
            .map(|managed| DownloadSummary::from(&self.build_snapshot(managed)))
            .collect::<Vec<_>>();
        items.sort_by_key(|right| std::cmp::Reverse(right.created_at_ms));
        Ok(items)
    }

    /// Get a single download summary by internal ID.
    pub async fn get_summary(&self, download_id: &str) -> Option<DownloadSummary> {
        let downloads = self.downloads.read().await;
        downloads
            .get(download_id)
            .map(|managed| DownloadSummary::from(&self.build_snapshot(managed.clone())))
    }

    async fn spawn_download(&self, managed: Arc<ManagedDownload>, max_retries: u32) -> Result<()> {
        {
            let mut runtime = managed.lock_runtime();
            if runtime.is_some() {
                return Ok(());
            }
            *runtime = Some(CancellationToken::new());
        }

        let manager = Arc::new(self.clone());
        let token = managed
            .lock_runtime()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("runtime token not set after initialization"))?;

        self.persist(managed.clone()).await?;

        tauri::async_runtime::spawn(async move {
            // Build URL list for mirror retry.
            // mirror::rewrite() already includes the original URL as the last
            // element, so mirror_urls is always [mirror1, ..., original].
            // Resume from current_mirror_index to preserve state across
            // pause/resume cycles.
            let (urls_to_try, start_index): (Vec<String>, usize) = {
                let core = managed.lock_core();
                if core.manifest.mirror_urls.is_empty() {
                    (vec![core.manifest.url.clone()], 0)
                } else {
                    let urls = core.manifest.mirror_urls.clone();
                    let idx = core.manifest.current_mirror_index.min(urls.len().saturating_sub(1));
                    (urls, idx)
                }
            };

            // Mirror mode is active when we have more than just the original URL.
            let has_mirrors = urls_to_try.len() > 1;

            // Use 1 retry for mirror mode (gives each mirror one retry on
            // transient HTTP errors like 429/503), keep original retry count
            // for non-mirror mode.
            let actual_retries = if has_mirrors { 1 } else { max_retries };

            for index in start_index..urls_to_try.len() {
                let url_to_try = &urls_to_try[index];
                // Update mirror tracking on manifest and snapshot
                {
                    let mut core = managed.lock_core();
                    core.manifest.current_mirror_index = index;
                    if has_mirrors {
                        core.manifest.mirror_url = Some(url_to_try.clone());
                        core.manifest.final_url = url_to_try.clone();
                    }
                    core.snapshot.mirror_url = core.manifest.mirror_url.clone();
                    core.snapshot.final_url = core.manifest.final_url.clone();
                }

                // Resolve CDN-aware client for this URL
                let (client, cdn_accelerated) = manager.resolve_client(url_to_try).await;
                {
                    let mut core = managed.lock_core();
                    core.snapshot.cdn_accelerated = cdn_accelerated;
                    core.manifest.cdn_accelerated = cdn_accelerated;
                }

                let result = manager
                    .run_download(managed.clone(), client, token.clone(), actual_retries)
                    .await;

                match result {
                    Ok(()) => {
                        break;
                    }
                    Err(DownloadError::Interrupted) => {
                        {
                            let mut runtime = managed.lock_runtime();
                            *runtime = None;
                        }
                        managed.stop_notify.notify_one();
                        return;
                    }
                    Err(error) => {
                        let is_network = is_network_error(&error);
                        let has_more = index + 1 < urls_to_try.len();

                        if has_mirrors && is_network && has_more {
                            tracing::warn!(
                                "mirror {index} failed with network error, trying next: {error}"
                            );
                            continue;
                        }

                        // Non-retryable error or last URL — set Failed state
                        {
                            let mut core = managed.lock_core();
                            core.snapshot.state = DownloadState::Failed;
                            core.snapshot.error = Some(error.to_string());
                            core.snapshot.connection_count = 0;
                            core.snapshot.allocated_thread_count = Some(0);
                            core.snapshot.updated_at_ms = now_ms();
                            core.manifest.state = DownloadState::Failed;
                            core.manifest.error = Some(error.to_string());
                            core.manifest.connection_count = 0;
                            core.manifest.allocated_thread_count = Some(0);
                            core.manifest.updated_at_ms = now_ms();
                        }
                        manager.emit_single_summary(&managed);

                        // Broadcast aria2.onDownloadError via EventBus
                        let download_id = managed.lock_core().snapshot.id.clone();
                        let gid = format!(
                            "{:016x}",
                            xxhash_rust::xxh3::xxh3_64(download_id.as_bytes())
                        );
                        manager.event_bus.publish(DownloadEvent::Aria2Notification {
                            event_name: "aria2.onDownloadError".into(),
                            gid,
                        });

                        break;
                    }
                }
            }

            let should_persist = {
                let core = managed.lock_core();
                core.snapshot.state != DownloadState::Canceled
            };
            if should_persist && let Err(error) = manager.persist(managed.clone()).await {
                log_background_error("persist background download state", &error);
            }
            {
                let mut runtime = managed.lock_runtime();
                *runtime = None;
            }
            managed.stop_notify.notify_one();
            manager.rebalance_notify.notify_waiters();
        });

        Ok(())
    }
}

fn is_terminal(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
    )
}

impl DownloadManager {
    /// Check whether the given URL should use GitHub mirrors, and if so return
    /// the list of mirror URLs (in priority order with the original URL appended
    /// as the final fallback).  Returns a single-element list (the original URL)
    /// if mirroring is disabled or the URL is not a GitHub URL.
    pub async fn mirror_urls_for(&self, url: &str) -> Vec<String> {
        let settings = self.settings.read().await;
        mirror_rewrite(url, &settings.github_mirror)
    }

    fn build_snapshot(&self, managed: Arc<ManagedDownload>) -> DownloadSnapshot {
        let core = managed.lock_core();
        let mut snapshot = core.snapshot.clone();
        let chunks: Vec<ChunkInfo> = core
            .manifest
            .chunks
            .iter()
            .map(|c| ChunkInfo {
                index: c.index,
                start: c.start,
                end: c.end,
                downloaded: c.downloaded,
                completed: c.completed,
                claimed_by: c.claimed_by,
            })
            .collect();
        snapshot.chunks = chunks;
        drop(core);
        let elapsed = (snapshot
            .updated_at_ms
            .saturating_sub(snapshot.created_at_ms))
        .max(1) as f64
            / 1000.0;
        let average_speed = if snapshot.downloaded_bytes == 0 {
            None
        } else {
            Some(snapshot.downloaded_bytes as f64 / elapsed)
        };
        let speed = managed
            .aimd
            .lock()
            .last_throughput
            .or(average_speed);
        let eta = match (snapshot.total_bytes, speed) {
            (Some(total), Some(speed)) if speed > 0.0 && total >= snapshot.downloaded_bytes => {
                Some(((total - snapshot.downloaded_bytes) as f64 / speed).ceil() as u64)
            }
            _ => None,
        };
        snapshot.speed_bytes_per_second = if is_terminal(snapshot.state) {
            None
        } else {
            speed
        };
        snapshot.eta_seconds = if is_terminal(snapshot.state) {
            None
        } else {
            eta
        };
        snapshot
    }

    /// Build a [`DownloadSummary`] from a managed download and emit a
    /// `download-updated` Tauri event to the frontend.
    ///
    /// This is the targeted alternative to `AppState::emit_all_downloads()` —
    /// it emits for a single download at its state transition point rather than
    /// scanning every download on a fixed schedule.
    pub(crate) fn emit_single_summary(&self, managed: &Arc<ManagedDownload>) {
        let snapshot = self.build_snapshot(managed.clone());
        let mut summary = DownloadSummary::from(&snapshot);
        summary.id = TaskId::make_http(summary.id);
        let json = serde_json::to_value(&summary).unwrap_or_default();
        self.event_bus.publish(DownloadEvent::Updated {
            id: summary.id.clone(),
            summary_json: json,
        });
    }

    /// Emit a lightweight `download-progress` event for incremental UI updates.
    /// Called after each persist cycle (~300ms for HTTP, ~2s for BT).
    pub(crate) fn emit_progress(&self, managed: &Arc<ManagedDownload>) {
        let snapshot = self.build_snapshot(managed.clone());
        let mut progress = DownloadProgress::from(&snapshot);
        progress.id = TaskId::make_http(progress.id);
        let json = serde_json::to_value(&progress).unwrap_or_default();
        self.event_bus.publish(DownloadEvent::Progress {
            id: progress.id.clone(),
            progress_json: json,
        });
    }

    fn record_progress(
        &self,
        managed: &Arc<ManagedDownload>,
        chunk_index: Option<usize>,
        bytes: u64,
    ) {
        let now = now_ms();
        let mut core = managed.lock_core();
        core.snapshot.downloaded_bytes = core.snapshot.downloaded_bytes.saturating_add(bytes);
        core.snapshot.error = None;
        core.snapshot.updated_at_ms = now;
        core.manifest.downloaded_bytes = core.manifest.downloaded_bytes.saturating_add(bytes);
        core.manifest.error = None;
        core.manifest.updated_at_ms = now;
        if let Some(index) = chunk_index
            && let Some(chunk) = core
                .manifest
                .chunks
                .iter_mut()
                .find(|candidate| candidate.index == index)
        {
            chunk.downloaded = chunk.downloaded.saturating_add(bytes);
            chunk.dirty = true;
            if chunk.downloaded > chunk.end.saturating_sub(chunk.start) {
                chunk.completed = true;
                chunk.claimed_by = None;
            }
        }
    }

    fn reset_progress(&self, managed: &Arc<ManagedDownload>, force_single_stream: bool) {
        let now = now_ms();
        let mut core = managed.lock_core();
        core.snapshot.downloaded_bytes = 0;
        core.snapshot.updated_at_ms = now;
        core.manifest.downloaded_bytes = 0;
        core.manifest.updated_at_ms = now;
        for chunk in &mut core.manifest.chunks {
            chunk.downloaded = 0;
            chunk.completed = false;
            chunk.claimed_by = None;
            chunk.dirty = true;
        }
        if force_single_stream {
            core.snapshot.connection_count = 1;
            core.snapshot.supports_ranges = false;
            core.snapshot.desired_thread_count = Some(1);
            core.snapshot.allocated_thread_count = Some(1);
            core.snapshot.thread_note = Some(String::from("单线程（服务器不支持分段）"));
            core.manifest.connection_count = 1;
            core.manifest.supports_ranges = false;
            core.manifest.chunks.clear();
            core.manifest.desired_thread_count = Some(1);
            core.manifest.allocated_thread_count = Some(1);
            core.manifest.thread_note = Some(String::from("单线程（服务器不支持分段）"));
        }
    }

    fn cleanup_files(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        if temp_path.exists() {
            remove_file_if_exists(&temp_path)?;
        }
        Ok(())
    }

    fn cleanup_destination_file(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let destination_path = PathBuf::from(manifest.destination_path);
        if destination_path.exists() {
            fs::remove_file(destination_path)?;
        }
        Ok(())
    }

    fn prepare_fresh_temp_file(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_core().manifest.clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        if temp_path.exists() {
            fs::remove_file(&temp_path)?;
        }
        let _file = open_download_file(&temp_path, manifest.total_bytes)?;
        Ok(())
    }

    async fn get(&self, download_id: &str) -> Result<Arc<ManagedDownload>> {
        self.downloads
            .read()
            .await
            .get(download_id)
            .cloned()
            .ok_or(DownloadError::NotFound)
    }

    async fn wait_until_stopped(&self, managed: &Arc<ManagedDownload>) {
        if managed.lock_runtime().is_none() {
            return;
        }
        // Register interest before re-checking to avoid a missed notification.
        // Notify stores one permit, so if the worker called notify_one() between
        // the first check and here, the notified() future resolves immediately.
        let notified = managed.stop_notify.notified();
        if managed.lock_runtime().is_none() {
            return;
        }
        notified.await;
    }

    async fn remove_internal(
        &self,
        download_id: &str,
        purge_file: bool,
    ) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        let snapshot_before = self.build_snapshot(managed.clone());
        let needs_cancel_state = matches!(
            snapshot_before.state,
            DownloadState::Queued
                | DownloadState::Downloading
                | DownloadState::Retrying
                | DownloadState::Verifying
                | DownloadState::Paused
        );

        if needs_cancel_state {
            let mut core = managed.lock_core();
            core.snapshot.state = DownloadState::Canceled;
            core.snapshot.connection_count = 0;
            core.snapshot.allocated_thread_count = Some(0);
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Canceled;
            core.manifest.connection_count = 0;
            core.manifest.allocated_thread_count = Some(0);
            core.manifest.updated_at_ms = now_ms();
        }

        let token = { managed.lock_runtime().clone() };
        if let Some(token) = token {
            token.cancel();
            self.wait_until_stopped(&managed).await;
        }

        self.cleanup_files(&managed)?;
        if purge_file {
            self.cleanup_destination_file(&managed)?;
        }
        self.downloads.write().await.remove(download_id);
        self.db
            .delete_download(download_id)
            .context("failed to delete download from database")?;
        self.rebalance_allocations().await?;
        self.rebalance_notify.notify_waiters();
        Ok(self.build_snapshot(managed))
    }

    async fn wait_until_active(
        &self,
        managed: &Arc<ManagedDownload>,
        token: &CancellationToken,
    ) -> WaitState {
        loop {
            {
                let core = managed.lock_core();
                if core.manifest.state == DownloadState::Canceled {
                    return WaitState::Canceled;
                }
                if core.manifest.state == DownloadState::Paused {
                    return WaitState::Paused;
                }
                if core.manifest.allocated_thread_count.unwrap_or(0) > 0 {
                    return WaitState::Running;
                }
            }

            tokio::select! {
                _ = token.cancelled() => return match managed.lock_core().snapshot.state {
                    DownloadState::Canceled => WaitState::Canceled,
                    _ => WaitState::Paused,
                },
                _ = self.rebalance_notify.notified() => {}
                _ = sleep(Duration::from_millis(120)) => {}
            }
        }
    }

    #[allow(dead_code)]
    pub fn game_mode(&self) -> bool {
        self.buffer_pool.game_mode()
    }

    pub fn set_game_mode(&self, enabled: bool) {
        self.buffer_pool.set_game_mode(enabled);
    }

    pub fn set_overclock_mode(&self, enabled: bool) {
        self.overclock_mode.store(enabled, Ordering::Relaxed);
        self.rebalance_notify.notify_one();
    }

    pub fn overclock_mode(&self) -> bool {
        self.overclock_mode.load(Ordering::Relaxed)
    }

    /// Resolve the disk type for a given directory path.
    /// Checks user overrides first, then falls back to OS detection.
    pub async fn resolve_disk_type(&self, dir: &Path) -> DiskType {
        let settings = self.settings.read().await;
        // Check user overrides
        let dir_str = dir.to_string_lossy().to_string();
        if let Some(disk_type) = settings.io_baseline.disk_type_overrides.get(&dir_str) {
            return *disk_type;
        }
        drop(settings);
        detect_disk_type(dir)
    }
}

// ---------------------------------------------------------------------------
//  DownloadBackend implementation for DownloadManager (HTTP backend)
//  Handles http: prefix stripping/adding.
// ---------------------------------------------------------------------------

#[async_trait]
impl DownloadBackend for DownloadManager {
    async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        let id = DownloadManager::start(self, request).await?;
        Ok(TaskId::make_http(id))
    }

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let inner = task_id.http_inner().ok_or(DownloadError::NotFound)?;
        DownloadManager::pause(self, inner).await
    }

    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let inner = task_id.http_inner().ok_or(DownloadError::NotFound)?;
        DownloadManager::resume(self, inner).await
    }

    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let inner = task_id.http_inner().ok_or(DownloadError::NotFound)?;
        DownloadManager::cancel(self, inner).await
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let inner = task_id.http_inner().ok_or(DownloadError::NotFound)?;
        DownloadManager::remove(self, inner).await
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let inner = task_id.http_inner().ok_or(DownloadError::NotFound)?;
        DownloadManager::purge(self, inner).await
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        let inner = task_id.http_inner().ok_or(DownloadError::NotFound)?;
        DownloadManager::open_in_explorer(self, inner).await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let inner = task_id.http_inner().ok_or(DownloadError::NotFound)?;
        DownloadManager::status(self, inner).await
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        DownloadManager::list(self).await
    }

    async fn update_settings(&self, settings: &AppSettings) -> Result<()> {
        let _ = DownloadManager::update_settings(self, settings.clone()).await?;
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        DownloadManager::shutdown(self).await;
        Ok(())
    }
}

#[derive(Debug)]
enum WaitState {
    Running,
    Paused,
    Canceled,
}

fn supports_parallelism(total: Option<u64>, supports_ranges: bool, chunk_size: u64) -> bool {
    supports_ranges && total.map(|value| value >= chunk_size * 2).unwrap_or(false)
}

fn resolve_thread_settings(
    settings: &AppSettings,
    request: &StartDownloadRequest,
    supports_parallel: bool,
) -> (
    ThreadMode,
    Option<usize>,
    Option<usize>,
    Option<AdaptiveProfile>,
) {
    if !supports_parallel {
        return (ThreadMode::Fixed, Some(1), Some(1), None);
    }

    match settings.scheduler.mode {
        SchedulerMode::Traditional => {
            let requested = request
                .thread_count
                .unwrap_or(DEFAULT_FIXED_THREADS)
                .clamp(1, MAX_TRADITIONAL_THREADS);
            (ThreadMode::Fixed, Some(requested), Some(requested), None)
        }
        SchedulerMode::Automatic => match request.thread_mode.unwrap_or(ThreadMode::Adaptive) {
            ThreadMode::Adaptive => {
                let profile = settings.scheduler.automatic.adaptive_profile;
                let max_threads = settings.scheduler.automatic.max_threads_per_task.max(1);
                let desired = aimd::initial_desired_threads(profile).min(max_threads);
                (
                    ThreadMode::Adaptive,
                    None,
                    Some(desired.max(1)),
                    Some(profile),
                )
            }
            ThreadMode::Fixed => {
                let requested = request
                    .thread_count
                    .unwrap_or(DEFAULT_FIXED_THREADS)
                    .clamp(1, settings.scheduler.automatic.max_threads_per_task.max(1));
                (ThreadMode::Fixed, Some(requested), Some(requested), None)
            }
        },
    }
}

fn thread_note(
    supports_parallel: bool,
    thread_mode: ThreadMode,
    adaptive_profile: Option<AdaptiveProfile>,
) -> Option<String> {
    if !supports_parallel {
        return Some(String::from("单线程（服务器不支持分段）"));
    }

    match thread_mode {
        ThreadMode::Fixed => Some(String::from("固定线程")),
        ThreadMode::Adaptive => adaptive_profile.map(|profile| match profile {
            AdaptiveProfile::Conservative => String::from("自适应 / 保守"),
            AdaptiveProfile::Balanced => String::from("自适应 / 平衡"),
            AdaptiveProfile::Aggressive => String::from("自适应 / 激进"),
        }),
    }
}

pub(crate) fn sync_snapshot_with_manifest(core: &mut DownloadCore) {
    // NOTE: mirror_url is intentionally NOT synced here — it is managed
    // exclusively by the mirror retry loop in spawn_download.
    let snapshot = &mut core.snapshot;
    let manifest = &core.manifest;
    snapshot.state = manifest.state;
    snapshot.final_url = manifest.final_url.clone();
    snapshot.file_name = manifest.file_name.clone();
    snapshot.destination_path = manifest.destination_path.clone();
    snapshot.total_bytes = manifest.total_bytes;
    snapshot.downloaded_bytes = manifest.downloaded_bytes;
    snapshot.supports_ranges = manifest.supports_ranges;
    snapshot.connection_count = manifest.connection_count;
    snapshot.thread_mode = manifest.thread_mode;
    snapshot.requested_thread_count = manifest.requested_thread_count;
    snapshot.desired_thread_count = manifest.desired_thread_count;
    snapshot.allocated_thread_count = manifest.allocated_thread_count;
    snapshot.adaptive_profile = manifest.adaptive_profile_snapshot;
    snapshot.thread_note = manifest.thread_note.clone();
    snapshot.etag = manifest.etag.clone();
    snapshot.last_modified = manifest.last_modified.clone();
    snapshot.error = manifest.error.clone();
    snapshot.updated_at_ms = manifest.updated_at_ms;
    snapshot.chunks = manifest
        .chunks
        .iter()
        .map(|c| ChunkInfo {
            index: c.index,
            start: c.start,
            end: c.end,
            downloaded: c.downloaded,
            completed: c.completed,
            claimed_by: c.claimed_by,
        })
        .collect();
}

fn record_progress_on_managed(
    managed: &Arc<ManagedDownload>,
    chunk_index: Option<usize>,
    bytes: u64,
) {
    let now = now_ms();
    let mut core = managed.lock_core();
    core.snapshot.downloaded_bytes = core.snapshot.downloaded_bytes.saturating_add(bytes);
    core.snapshot.error = None;
    core.snapshot.updated_at_ms = now;
    core.manifest.downloaded_bytes = core.manifest.downloaded_bytes.saturating_add(bytes);
    core.manifest.error = None;
    core.manifest.updated_at_ms = now;
    if let Some(index) = chunk_index
        && let Some(chunk) = core
            .manifest
            .chunks
            .iter_mut()
            .find(|candidate| candidate.index == index)
    {
        chunk.downloaded = chunk.downloaded.saturating_add(bytes);
        chunk.dirty = true;
        if chunk.downloaded > chunk.end.saturating_sub(chunk.start) {
            chunk.completed = true;
            chunk.claimed_by = None;
        }
    }
}

fn unique_destination_path(destination_dir: &Path, file_name: &str) -> PathBuf {
    let base = destination_dir.join(file_name);
    if !base.exists() {
        return base;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    for index in 1..10_000 {
        let candidate = destination_dir.join(format!("{stem} ({index}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    destination_dir.join(format!("{}-{}{}", stem, Uuid::new_v4(), extension))
}

fn initial_file_name_from_url(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(ToOwned::to_owned))
        })
        .map(sanitize_filename::sanitize)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("download"))
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn log_background_error(context: &str, error: impl std::fmt::Display) {
    tracing::warn!(context, %error, "background error");
}

fn cancellation_outcome(managed: &Arc<ManagedDownload>) -> RunOutcome {
    match managed.lock_core().snapshot.state {
        DownloadState::Canceled => RunOutcome::Canceled,
        _ => RunOutcome::Paused,
    }
}

fn cancellation_chunk_outcome(managed: &Arc<ManagedDownload>) -> ChunkWorkerOutcome {
    match managed.lock_core().snapshot.state {
        DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
        _ => ChunkWorkerOutcome::Paused,
    }
}

/// Returns `true` if the error represents a transport-level network failure
/// (connection refused, timeout, DNS resolution failure, TLS handshake failure)
/// that might succeed with a different mirror.  Returns `false` for application-
/// level errors (HTTP 4xx/5xx, invalid responses, I/O errors, etc.).
fn is_network_error(error: &DownloadError) -> bool {
    match error {
        DownloadError::Http(e) => e.is_connect() || e.is_timeout() || e.is_body(),
        _ => false,
    }
}

#[cfg(test)]
#[path = "tests/manager_tests.rs"]
mod tests;
