#[cfg(windows)]
use std::process::Command;
use std::{
    fs, io,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use foldhash::HashMap;
use parking_lot::{Mutex, MutexGuard, RwLock as ParkingRwLock};

use anyhow::Context;
use async_trait::async_trait;
use reqwest::{Client, Url};
use tokio::{
    sync::{Notify, RwLock},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    aimd::{self, AimdState},
    buffer_pool::{BufferPool, IoWorker},
    database::Database,
    error::{DownloadError, Result, io_error_with_path},
    event_bus::{DownloadEvent, EventBus},
    file_ops::detect_disk_type,
    http_executor::HttpExecutor,
    lock,
    logging::apply_logging_settings,
    manifest::{
        CHUNK_SIZE, Manifest, snapshot_from_manifest,
    },
    migration::migrate_json_manifests,
    protocol::DownloadBackend,
    rate_limiter::RateLimiter,
    scheduler::Scheduler,
    slot_guard::DownloadSlotGuard,
    task_lifecycle::TaskLifecycle,
    types::{
        AdaptiveProfile, AppSettings, ChecksumMode, ChunkInfo, DiskType,
        DownloadSnapshot, DownloadState, DownloadSummary, Priority, SchedulerMode, StartDownloadRequest,
        TaskId, ThreadMode,
    },
};

use super::http_client_factory::build_http_client;
use super::mirror::rewrite as mirror_rewrite;
use super::now_ms;
use super::settings::{load_settings, normalize_settings, persist_settings, resolve_user_agent};

pub const DEFAULT_FIXED_THREADS: usize = 8;
pub(crate) const DEFAULT_RETRIES: u32 = 4;
pub(crate) const PERSIST_INTERVAL: Duration = Duration::from_millis(300);
pub const MAX_TRADITIONAL_THREADS: usize = 32;

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<super::backend_registry::BackendRegistry>,
    pub event_bus: Arc<EventBus>,
    pub cdn_service: Arc<super::cdn::CdnService>,
    pub rpc_shutdown: Arc<parking_lot::Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
    pub settings: Arc<ParkingRwLock<AppSettings>>,
    /// Cancelled during shutdown to stop the periodic emit task gracefully
    /// before backends are torn down.
    pub emit_cancel: CancellationToken,
    /// Shared HTTP client for one-off requests (tracker list fetch, etc.).
    /// reqwest::Client is cheap to clone — it uses Arc internally.
    pub http_client: reqwest::Client,
}

impl AppState {
    pub async fn emit_all_downloads(&self) {
        for backend in self.registry.iter() {
            if let Ok(summaries) = backend.list().await {
                for summary in summaries {
                    let summary_json = serde_json::to_value(&summary).unwrap_or_default();
                    let id = summary.id.clone();
                    self.event_bus
                        .publish(DownloadEvent::Updated { id, summary_json });
                }
            }
        }
    }
}

// ── Sub-structures for field grouping ──────────────────────────────────

/// Concurrency limits and counters for HTTP/BT download throttling.
pub struct ConcurrencyLimits {
    /// Active HTTP download counter (for concurrent throttling)
    pub active_http_count: Arc<AtomicUsize>,
    /// Active BT download counter (for concurrent throttling)
    pub active_bt_count: Arc<AtomicUsize>,
    /// Maximum concurrent HTTP downloads
    pub max_concurrent_http: Arc<AtomicUsize>,
    /// Maximum concurrent BT downloads
    pub max_concurrent_bt: Arc<AtomicUsize>,
    /// Overclock mode flag (allows scheduler to pin all adaptive tasks at max threads)
    pub overclock_mode: Arc<AtomicBool>,
}

/// Runtime control signals for shutdown and scheduler rebalance coordination.
pub struct RuntimeControls {
    /// Cancellation token for graceful shutdown of scheduler loop and workers.
    pub shutdown_token: CancellationToken,
    /// Notify mechanism for triggering scheduler rebalance events.
    pub rebalance_notify: Arc<Notify>,
}

/// HTTP client infrastructure including CDN accelerator reference and client cache.
pub struct HttpClientInfra {
    /// Base HTTP client (reqwest), rebuilt when proxy/user-agent changes.
    client: Arc<RwLock<Client>>,
    /// CDN client cache keyed by (hostname, IP) for accelerated domain connections.
    pub cdn_client_cache: Arc<ParkingRwLock<HashMap<(String, Ipv4Addr), Client>>>,
    /// Optional CDN accelerator for Cloudflare IP probing and DNS rewriting.
    cdn_accelerator: Arc<RwLock<Option<Arc<super::cdn::CdnAccelerator>>>>,
}

/// File system paths for download state persistence and settings storage.
pub struct StateDirs {
    /// Directory for download state files (temp files, manifests).
    pub(crate) state_dir: PathBuf,
    /// Path to the settings.json file.
    pub(crate) settings_path: PathBuf,
}

impl StateDirs {
    /// Returns the state directory path (e.g. `<data>/downloads`).
    pub fn state_dir(&self) -> &PathBuf {
        &self.state_dir
    }
}

pub struct DownloadManager {
    /// HTTP client infrastructure (client, CDN cache, CDN accelerator).
    pub http: HttpClientInfra,
    /// File system paths for state and settings.
    pub dirs: StateDirs,
    pub settings: Arc<RwLock<AppSettings>>,
    pub downloads: Arc<RwLock<HashMap<String, Arc<ManagedDownload>>>>,
    pub db: Arc<Database>,
    pub event_bus: Arc<EventBus>,
    pub(crate) rate_limiter: Arc<RateLimiter>,
    pub buffer_pool: Arc<BufferPool>,
    /// Dedicated I/O worker thread for file flush operations.
    pub io_worker: IoWorker,
    pub controls: RuntimeControls,
    /// Cache disk type detections keyed by device ID to avoid redundant OS queries.
    pub(crate) disk_type_cache: Arc<parking_lot::Mutex<foldhash::HashMap<u64, DiskType>>>,
    pub limits: ConcurrencyLimits,
    /// HTTP download executor actor — handles probe, single/chunked download, finalize.
    pub http_executor: Arc<HttpExecutor>,
    /// Scheduler actor — handles background rebalance loop and thread allocation.
    pub scheduler: Arc<Scheduler>,
    /// Task lifecycle actor — handles state transitions, wait coordination, file ops,
    /// progress recording, and event emission.
    pub task_lifecycle: Arc<TaskLifecycle>,
}

// NOTE: `max_concurrent_http` and `overclock_mode` are now `Arc`-wrapped
// so Clone shares them correctly with the live `DownloadManager`.
// The `start()`/`resume()` methods use `self: &Arc<Self>` and pass
// `self.clone()` (Arc::clone) to spawned download futures — those futures
// hold an Arc aliasing the SAME atomics, so `apply_settings` /
// `toggle_overclock_mode` / `set_overclock_mode` propagate correctly.
// The trait impl (`DownloadBackend for DownloadManager`) still constructs
// a temporary Arc via `Clone` for the UFCS call — safe because atomics
// are now Arc-shared.
impl Clone for DownloadManager {
    fn clone(&self) -> Self {
        Self {
            http: HttpClientInfra {
                client: self.http.client.clone(),
                cdn_client_cache: self.http.cdn_client_cache.clone(),
                cdn_accelerator: self.http.cdn_accelerator.clone(),
            },
            dirs: StateDirs {
                state_dir: self.dirs.state_dir.clone(),
                settings_path: self.dirs.settings_path.clone(),
            },
            settings: self.settings.clone(),
            downloads: self.downloads.clone(),
            db: self.db.clone(),
            controls: RuntimeControls {
                shutdown_token: self.controls.shutdown_token.clone(),
                rebalance_notify: self.controls.rebalance_notify.clone(),
            },
            event_bus: self.event_bus.clone(),
            rate_limiter: self.rate_limiter.clone(),
            buffer_pool: self.buffer_pool.clone(),
            io_worker: self.io_worker.clone(),
            disk_type_cache: self.disk_type_cache.clone(),
            limits: ConcurrencyLimits {
                active_http_count: self.limits.active_http_count.clone(),
                active_bt_count: self.limits.active_bt_count.clone(),
                max_concurrent_http: self.limits.max_concurrent_http.clone(),
                max_concurrent_bt: self.limits.max_concurrent_bt.clone(),
                overclock_mode: self.limits.overclock_mode.clone(),
            },
            http_executor: self.http_executor.clone(),
            scheduler: self.scheduler.clone(),
            task_lifecycle: self.task_lifecycle.clone(),
        }
    }
}

/// Merged core of snapshot + manifest, protected by a single Mutex.
/// This eliminates double-lock ordering in hot paths like record_progress().
pub struct DownloadCore {
    pub snapshot: DownloadSnapshot,
    pub manifest: Manifest,
}

impl DownloadCore {
    /// Sync snapshot fields from manifest.
    /// NOTE: mirror_url is intentionally NOT synced — managed by mirror retry loop.
    pub fn sync_snapshot_from_manifest(&mut self) {
        let m = &self.manifest;
        self.snapshot.state = m.state;
        self.snapshot.final_url = m.final_url.clone();
        self.snapshot.file_name = m.file_name.clone();
        self.snapshot.destination_path = m.destination_path.clone();
        self.snapshot.total_bytes = m.total_bytes;
        self.snapshot.downloaded_bytes = m.downloaded_bytes;
        self.snapshot.supports_ranges = m.supports_ranges;
        self.snapshot.connection_count = m.connection_count;
        self.snapshot.thread_mode = m.thread_mode;
        self.snapshot.requested_thread_count = m.requested_thread_count;
        self.snapshot.desired_thread_count = m.desired_thread_count;
        self.snapshot.allocated_thread_count = m.allocated_thread_count;
        self.snapshot.adaptive_profile = m.adaptive_profile_snapshot;
        self.snapshot.thread_note = m.thread_note.clone();
        self.snapshot.etag = m.etag.clone();
        self.snapshot.last_modified = m.last_modified.clone();
        self.snapshot.error = m.error.clone();
        self.snapshot.updated_at_ms = m.updated_at_ms;
        // COW optimization: only rebuild Vec<ChunkInfo> when chunk structure
        // (count + offset boundaries) changes; otherwise update state fields
        // in-place to avoid per-tick allocation churn.
        // Guard: skip the fast path when snapshot has no chunks yet (initial state);
        // both empty or structure-mismatch → full rebuild.
        if !self.snapshot.chunks.is_empty()
            && self.snapshot.chunks.len() == m.chunks.len()
            && self
                .snapshot
                .chunks
                .iter()
                .zip(m.chunks.iter())
                .all(|(sc, mc)| sc.index == mc.index && sc.start == mc.start && sc.end == mc.end)
        {
            // Structure unchanged — update only state fields in-place
            for (sc, mc) in self.snapshot.chunks.iter_mut().zip(m.chunks.iter()) {
                sc.downloaded = mc.downloaded;
                sc.completed = mc.completed;
                sc.claimed_by = mc.claimed_by;
            }
        } else {
            // Structure changed or empty — full rebuild
            self.snapshot.chunks = m
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
    }
}

pub struct ManagedDownload {
    pub core: Mutex<DownloadCore>,
    pub runtime: Mutex<Option<CancellationToken>>,
    pub aimd: Mutex<AimdState>,
    pub stop_notify: Notify,
}

impl ManagedDownload {
    pub fn lock_core(&self) -> MutexGuard<'_, DownloadCore> {
        lock(&self.core)
    }

    pub fn lock_runtime(&self) -> MutexGuard<'_, Option<CancellationToken>> {
        self.runtime.lock()
    }

    pub fn lock_aimd(&self) -> MutexGuard<'_, AimdState> {
        self.aimd.lock()
    }
}

#[derive(Debug)]
pub(crate) enum RunOutcome {
    Finished,
    Paused,
    Canceled,
}

#[derive(Debug)]
pub(crate) enum ChunkWorkerOutcome {
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
            .ok_or_else(|| {
                DownloadError::Internal(format!(
                    "state directory '{}' has no parent — cannot determine settings path",
                    state_dir.display()
                ))
            })?
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
        let io_worker = IoWorker::spawn();

        let manager = Self {
            http: HttpClientInfra {
                client: Arc::new(RwLock::new(client)),
                cdn_client_cache: Arc::new(ParkingRwLock::new(HashMap::default())),
                cdn_accelerator: Arc::new(RwLock::new(None)),
            },
            dirs: StateDirs {
                state_dir,
                settings_path,
            },
            settings: Arc::new(RwLock::new(settings)),
            downloads: Arc::new(RwLock::new(HashMap::default())),
            db,
            event_bus,
            rate_limiter,
            buffer_pool,
            io_worker,
            controls: RuntimeControls {
                shutdown_token: CancellationToken::new(),
                rebalance_notify: Arc::new(Notify::new()),
            },
            disk_type_cache: Arc::new(parking_lot::Mutex::new(foldhash::HashMap::default())),
            limits: ConcurrencyLimits {
                active_http_count: Arc::new(AtomicUsize::new(0)),
                active_bt_count: Arc::new(AtomicUsize::new(0)),
                max_concurrent_http: Arc::new(AtomicUsize::new(5)),
                max_concurrent_bt: Arc::new(AtomicUsize::new(3)),
                overclock_mode: Arc::new(AtomicBool::new(false)),
            },
            http_executor: Arc::new(HttpExecutor),
            scheduler: Arc::new(Scheduler),
            task_lifecycle: Arc::new(TaskLifecycle),
        };

        manager.load_downloads_from_db()?;
        Ok(manager)
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
            *self.http.cdn_accelerator.blocking_write() = Some(acc);
        });
        // Clear CDN client cache since the accelerator IP has changed.
        self.http.cdn_client_cache.write().clear();
    }

    /// Resolve the HTTP client to use for a given URL.
    ///
    /// If CDN acceleration is enabled and an accelerated IP is available, this builds
    /// a domain-specific client that resolves the URL's hostname to the best Cloudflare IP.
    /// Otherwise falls back to the standard client.
    pub(crate) async fn resolve_client(&self, url: &str) -> (Client, bool) {
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
            return (self.http.client.read().await.clone(), false);
        }

        if !super::cdn::is_cloudflare_domain(url).await {
            tracing::debug!("resolve_client: domain is not Cloudflare, using standard client");
            return (self.http.client.read().await.clone(), false);
        }

        let Ok(parsed) = reqwest::Url::parse(url) else {
            tracing::debug!("resolve_client: failed to parse URL: {url}");
            return (self.http.client.read().await.clone(), false);
        };
        let Some(host) = parsed.host_str() else {
            tracing::debug!("resolve_client: no host in URL: {url}");
            return (self.http.client.read().await.clone(), false);
        };

        // IP resolution: in-memory accelerator → persisted settings fallback
        let ip = match self.http.cdn_accelerator.read().await.as_ref() {
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
            // Check cache first
            let cache_key = (host.to_string(), ip);
            {
                let cache = self.http.cdn_client_cache.read();
                if let Some(cached_client) = cache.get(&cache_key) {
                    tracing::debug!("resolve_client: using cached CDN client for {host} via {ip}");
                    return (cached_client.clone(), true);
                }
            }
            // Cache miss — build new accelerated client
            let settings = self.settings.read().await;
            match super::cdn::build_accelerated_client(host, ip, &settings) {
                Ok(accelerated) => {
                    tracing::info!("resolve_client: CDN acceleration active for {host} via {ip}");
                    self.http.cdn_client_cache
                        .write()
                        .insert(cache_key, accelerated.clone());
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

        (self.http.client.read().await.clone(), false)
    }

    pub async fn apply_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        let normalized = normalize_settings(settings)?;

        // Apply non-client-affecting settings immediately
        self.rate_limiter
            .set_rate(normalized.global_speed_limit_bps);
        self.buffer_pool.update_limits(
            normalized.io_baseline.buffer_limit_mb,
            normalized.io_baseline.game_mode_buffer_mb,
            normalized.io_baseline.max_parallel_hdd,
            normalized.io_baseline.game_mode_max_parallel,
        );
        self.buffer_pool
            .set_game_mode(normalized.io_baseline.game_mode);

        // Only rebuild client when proxy or user-agent actually changed
        let client_changed = {
            let current = self.settings.read().await;
            current.proxy.mode != normalized.proxy.mode
                || current.proxy.manual_url != normalized.proxy.manual_url
                || current.download.default_user_agent != normalized.download.default_user_agent
        };

        persist_settings(&self.dirs.settings_path, &normalized).await?;
        *self.settings.write().await = normalized.clone();

        // Update concurrent download limits from settings
        if let Some(ref limits) = normalized.download_limits {
            self.limits.max_concurrent_http
                .store(limits.max_concurrent_http, Ordering::Release);
            self.limits.max_concurrent_bt
                .store(limits.max_concurrent_bt, Ordering::Release);
        }

        if client_changed {
            let next_client = build_http_client(&normalized)?;
            *self.http.client.write().await = next_client;
        }
        // Clear CDN client cache since settings may have changed the active IP
        self.http.cdn_client_cache.write().clear();

        apply_logging_settings(&normalized.logging, &self.dirs.state_dir).map_err(|error| {
            DownloadError::InvalidResponse(format!("failed to apply logging settings: {error}"))
        })?;
        self.scheduler.rebalance_allocations(self).await?;
        self.controls.rebalance_notify.notify_waiters();

        Ok(normalized)
    }

    pub async fn start(self: &Arc<Self>, request: StartDownloadRequest) -> Result<Uuid> {
        let url = Url::parse(&request.url)
            .map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(DownloadError::UnsupportedScheme);
        }

        // Acquire a concurrent download slot (HTTP throttle)
        let slot = self.try_acquire_http()?;

        let settings = self.settings.read().await.clone();
        let download_id = Uuid::new_v4();
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
        // Validate path: reject paths with '..' traversal components
        // to prevent writes outside the intended download directory.
        {
            let mut normalized = std::path::PathBuf::new();
            for component in destination_dir.components() {
                match component {
                    std::path::Component::ParentDir => {
                        if !normalized.pop() {
                            // Can't go above root — already at root, which is suspicious
                            return Err(DownloadError::InvalidResponse(String::from(
                                "download destination directory must not escape its parent via '..'",
                            )));
                        }
                    }
                    std::path::Component::Normal(c) => {
                        normalized.push(c);
                    }
                    std::path::Component::CurDir => {
                        // '.' — stay in current directory, nothing to push
                    }
                    std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                        normalized.push(component);
                    }
                }
            }
            // Re-check that the normalized path is still absolute
            if !normalized.is_absolute() {
                return Err(DownloadError::InvalidResponse(String::from(
                    "download destination directory resolves to a non-absolute path",
                )));
            }
        }
        fs::create_dir_all(&destination_dir)
            .map_err(|e| io_error_with_path(e, destination_dir.to_string_lossy()))?;

        let chosen_name = request
            .file_name
            .clone()
            .unwrap_or_else(|| initial_file_name_from_url(&request.url));
        let safe_name = sanitize_filename::sanitize(&chosen_name);
        if safe_name.is_empty() {
            return Err(DownloadError::MissingFileName);
        }

        let destination_path = unique_destination_path(&destination_dir, &safe_name);
        let temp_path = self.dirs.state_dir.join(format!("{download_id}.part"));
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
            id: download_id.to_string(),
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
            priority: request.priority.unwrap_or_default(),
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
            .insert(download_id.to_string(), managed.clone());

        let dm = self.clone();
        self.task_lifecycle.spawn_download(dm, managed, request.max_retries.unwrap_or(DEFAULT_RETRIES), slot)
            .await?;
        self.scheduler.rebalance_allocations(self).await?;
        self.controls.rebalance_notify.notify_waiters();

        Ok(download_id)
    }

    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.task_lifecycle.get(self, download_id).await?;
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

        self.task_lifecycle.wait_until_stopped(self, &managed).await;
        self.persist(managed.clone()).await?;
        self.scheduler.rebalance_allocations(self).await?;
        self.controls.rebalance_notify.notify_waiters();
        Ok(self.task_lifecycle.build_snapshot(self, managed))
    }

    pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.task_lifecycle.get(self, download_id).await?;

        let is_completed = {
            let core = managed.lock_core();
            core.snapshot.state == DownloadState::Completed
        };

        if !is_completed {
            {
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
            if let Some(token) = &token {
                token.cancel();
            }
            if token.is_some() {
                self.task_lifecycle.wait_until_stopped(self, &managed).await;
            }
            self.task_lifecycle.cleanup_files(self, &managed)?;
        }

        // Remove from active list and trigger rebalance even when the
        // download already completed — otherwise queued tasks may never
        // be unblocked if the background scheduler loop hasn't ticked yet.
        self.downloads.write().await.remove(download_id);
        self.db
            .delete_download(download_id)
            .context("failed to delete canceled download from database")?;
        self.scheduler.rebalance_allocations(self).await?;
        self.controls.rebalance_notify.notify_waiters();
        Ok(self.task_lifecycle.build_snapshot(self, managed))
    }

    pub async fn remove(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.task_lifecycle.remove_internal(self, download_id, false).await
    }

    pub async fn purge(&self, download_id: &str) -> Result<DownloadSnapshot> {
        self.task_lifecycle.remove_internal(self, download_id, true).await
    }

    pub async fn open_in_explorer(&self, download_id: &str) -> Result<()> {
        let managed = self.task_lifecycle.get(self, download_id).await?;
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
            #[cfg(target_os = "macos")]
            {
                Command::new("open").arg("-R").arg(&destination_path).spawn()?;
            }
            #[cfg(target_os = "linux")]
            {
                Command::new("xdg-open").arg(&directory_path).spawn()?;
            }
            return Ok(());
        }

        if directory_path.exists() {
            #[cfg(windows)]
            {
                Command::new("explorer").arg(&directory_path).spawn()?;
            }
            #[cfg(target_os = "macos")]
            {
                Command::new("open").arg(&directory_path).spawn()?;
            }
            #[cfg(target_os = "linux")]
            {
                Command::new("xdg-open").arg(&directory_path).spawn()?;
            }
            return Ok(());
        }

        Err(DownloadError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "download location does not exist",
        )))
    }

    pub async fn resume(self: &Arc<Self>, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.task_lifecycle.get(self, download_id).await?;
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

        // Acquire a concurrent download slot (HTTP throttle) for resume
        let slot = self.try_acquire_http()?;

        let dm = self.clone();
        self.task_lifecycle.spawn_download(dm, managed.clone(), DEFAULT_RETRIES, slot)
            .await?;
        self.scheduler.rebalance_allocations(self).await?;
        self.controls.rebalance_notify.notify_waiters();
        Ok(self.task_lifecycle.build_snapshot(self, managed))
    }

    pub async fn status(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.task_lifecycle.get(self, download_id).await?;
        Ok(self.task_lifecycle.build_snapshot(self, managed))
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let downloads = self.downloads.read().await;
        let mut items = downloads
            .values()
            .cloned()
            .map(|managed| DownloadSummary::from(&self.task_lifecycle.build_snapshot(self, managed)))
            .collect::<Vec<_>>();
        items.sort_by_key(|right| std::cmp::Reverse(right.created_at_ms));
        Ok(items)
    }

    /// Get a single download summary by internal ID.
    pub async fn get_summary(&self, download_id: &str) -> Option<DownloadSummary> {
        let downloads = self.downloads.read().await;
        downloads
            .get(download_id)
            .map(|managed| DownloadSummary::from(&self.task_lifecycle.build_snapshot(self, managed.clone())))
    }

    /// Find a non-terminal download by URL. Returns the internal download ID if found.
    pub async fn find_active_by_url(&self, url: &str) -> Option<String> {
        let downloads = self.downloads.read().await;
        for (id, managed) in downloads.iter() {
            let core = managed.lock_core();
            if core.manifest.url == url
                && !matches!(
                    core.manifest.state,
                    DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
                )
            {
                return Some(id.clone());
            }
        }
        None
    }
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

    #[allow(dead_code)]
    pub fn game_mode(&self) -> bool {
        self.buffer_pool.game_mode()
    }

    pub fn set_game_mode(&self, enabled: bool) {
        self.buffer_pool.set_game_mode(enabled);
    }

    pub fn set_overclock_mode(&self, enabled: bool) {
        self.limits.overclock_mode.store(enabled, Ordering::Relaxed);
        self.controls.rebalance_notify.notify_one();
    }

    pub fn overclock_mode(&self) -> bool {
        self.limits.overclock_mode.load(Ordering::Relaxed)
    }

    /// Resolve the disk type for a given directory path.
    /// Checks user overrides first, then a per-device cache, then OS detection.
    pub async fn resolve_disk_type(&self, dir: &Path) -> DiskType {
        let settings = self.settings.read().await;
        let dir_str = dir.to_string_lossy().to_string();
        if let Some(disk_type) = settings.io_baseline.disk_type_overrides.get(&dir_str) {
            return *disk_type;
        }
        drop(settings);

        // Check per-device cache on Unix to avoid repeated OS queries.
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata(dir) {
                let dev = meta.dev();
                let mut cache = self.disk_type_cache.lock();
                if let Some(cached) = cache.get(&dev) {
                    return *cached;
                }
                let detected = detect_disk_type(dir);
                cache.insert(dev, detected);
                return detected;
            }
        }

        detect_disk_type(dir)
    }

    // ── Concurrent download throttle ────────────────────────────

    /// Try to acquire an HTTP download slot.
    /// Returns `Ok(DownloadSlotGuard)` if under limit, `Err` if at capacity.
    pub fn try_acquire_http(&self) -> std::result::Result<DownloadSlotGuard, DownloadError> {
        let max = self.limits.max_concurrent_http.load(Ordering::Acquire);
        let counter = &self.limits.active_http_count;
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                return Err(DownloadError::TooManyConcurrentDownloads);
            }
            if counter
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(DownloadSlotGuard::new(self.limits.active_http_count.clone()));
            }
        }
    }

    /// Try to acquire a BT download slot.
    /// Returns `Ok(DownloadSlotGuard)` if under limit, `Err` if at capacity.
    pub fn try_acquire_bt(&self) -> std::result::Result<DownloadSlotGuard, DownloadError> {
        let max = self.limits.max_concurrent_bt.load(Ordering::Acquire);
        let counter = &self.limits.active_bt_count;
        loop {
            let current = counter.load(Ordering::Acquire);
            if current >= max {
                return Err(DownloadError::TooManyConcurrentDownloads);
            }
            if counter
                .compare_exchange_weak(current, current + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(DownloadSlotGuard::new(self.limits.active_bt_count.clone()));
            }
        }
    }
}

// ---------------------------------------------------------------------------
//  DownloadBackend implementation for DownloadManager (HTTP backend)
//  Adapts between typed TaskId and internal Uuid-based download IDs.
// ---------------------------------------------------------------------------

#[async_trait]
impl DownloadBackend for DownloadManager {
    async fn start(&self, request: StartDownloadRequest) -> Result<TaskId> {
        let arc_self = Arc::new(self.clone());
        let uuid = DownloadManager::start(&arc_self, request).await?;
        Ok(TaskId::Http(uuid))
    }

    async fn pause(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        DownloadManager::pause(self, &uuid.to_string()).await
    }

    async fn resume(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        let arc_self = Arc::new(self.clone());
        DownloadManager::resume(&arc_self, &uuid.to_string()).await
    }

    async fn cancel(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        DownloadManager::cancel(self, &uuid.to_string()).await
    }

    async fn remove(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        DownloadManager::remove(self, &uuid.to_string()).await
    }

    async fn purge(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        DownloadManager::purge(self, &uuid.to_string()).await
    }

    async fn open_in_explorer(&self, task_id: &TaskId) -> Result<()> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        DownloadManager::open_in_explorer(self, &uuid.to_string()).await
    }

    async fn status(&self, task_id: &TaskId) -> Result<DownloadSnapshot> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        DownloadManager::status(self, &uuid.to_string()).await
    }

    async fn list(&self) -> Result<Vec<DownloadSummary>> {
        DownloadManager::list(self).await
    }

    async fn update_settings(&self, settings: &AppSettings) -> Result<()> {
        self.apply_settings(settings.clone()).await?;
        Ok(())
    }

    async fn set_priority(&self, task_id: &TaskId, priority: Priority) -> Result<()> {
        let TaskId::Http(uuid) = task_id else {
            return Err(DownloadError::NotFound);
        };
        let download_id = uuid.to_string();
        let managed = self.task_lifecycle.get(self, &download_id).await?;
        {
            let mut core = managed.lock_core();
            core.manifest.priority = priority;
        }
        // Persist to DB
        self.db.set_priority(&download_id, priority as u8)?;
        Ok(())
    }

    async fn shutdown(&self) {
        tracing::info!("Shutting down download manager...");
        self.task_lifecycle.shutdown(self).await;
        // Wait for buffer pool to drain (max 15 seconds to allow large buffers to flush)
        let start = std::time::Instant::now();
        while self.buffer_pool.active_slots() > 0
            && start.elapsed() < std::time::Duration::from_secs(15)
        {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let remaining = self.buffer_pool.active_slots();
        if remaining > 0 {
            tracing::warn!("Buffer pool drain timed out after 15s, {remaining} slots still active; data may be lost");
        } else {
            tracing::info!("Buffer pool drained successfully");
        }
    }
}

/// Result of `wait_until_active()` — used by HttpExecutor.
#[derive(Debug)]
pub(crate) enum WaitState {
    Running,
    Paused,
    Canceled,
}

pub(crate) fn supports_parallelism(total: Option<u64>, supports_ranges: bool, chunk_size: u64) -> bool {
    supports_ranges && total.map(|value| value >= chunk_size * 2).unwrap_or(false)
}

pub(crate) fn resolve_thread_settings(
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

pub(crate) fn thread_note(
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
    core.sync_snapshot_from_manifest();
}

/// Records download progress directly on a [`ManagedDownload`].
///
/// # Design note (duplication with [`TaskLifecycle::record_progress`])
/// This free helper exists because chunk workers in [`http_executor`] hold
/// only an `&Arc<ManagedDownload>` reference and do **not** have access to
/// a `&DownloadManager` to call through `TaskLifecycle::record_progress`.
///
/// The body is **identical** to [`TaskLifecycle::record_progress`] in
/// `task_lifecycle.rs`. If you modify one, you **must** update the other.
pub(crate) fn record_progress_on_managed(
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

pub(crate) fn unique_destination_path(destination_dir: &Path, file_name: &str) -> PathBuf {
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

pub(crate) fn log_background_error(context: &str, error: impl std::fmt::Display) {
    tracing::warn!(context, %error, "background error");
}

pub(crate) fn cancellation_outcome(managed: &Arc<ManagedDownload>) -> RunOutcome {
    match managed.lock_core().snapshot.state {
        DownloadState::Canceled => RunOutcome::Canceled,
        _ => RunOutcome::Paused,
    }
}

pub(crate) fn cancellation_chunk_outcome(managed: &Arc<ManagedDownload>) -> ChunkWorkerOutcome {
    match managed.lock_core().snapshot.state {
        DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
        _ => ChunkWorkerOutcome::Paused,
    }
}
