use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use futures_util::StreamExt;
use reqwest::{Client, Proxy, Response, StatusCode, Url, header, redirect::Policy};
use tauri::Emitter;
use tokio::{
    fs as async_fs,
    sync::{Notify, RwLock, broadcast},
    task::JoinSet,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    aimd::{self, AimdState},
    database::Database,
    error::{DownloadError, Result},
    file_alloc::{finalize_temp_file, open_download_file, reset_download_file, write_all_at},
    http::{
        ResponseDisposition, build_segment_request, classify_download_response,
        extract_total_bytes, header_string, if_range_header, infer_file_name, supports_ranges,
        validate_probe_response, validate_segment_response,
    },
    logging::apply_logging_settings,
    manifest::{
        CHUNK_SIZE, ChunkManifest, Manifest, RemoteMetadata, contiguous_prefix_end,
        has_partial_chunk_progress, plan_chunks, snapshot_from_manifest, validators_changed,
    },
    metalink::parse_metalink,
    migration::migrate_json_manifests,
    torrent::TorrentManager,
    types::{
        AdaptiveProfile, AppSettings, Aria2RpcSettings, AutomaticSchedulerSettings, BtSettings,
        CdnAccelerationSettings, ChecksumMode, ChunkInfo,
        DeviceLearningMode, DownloadDefaultsSettings, DownloadSnapshot, DownloadState,
        DownloadSummary, LogSettings, NetworkLearningMetrics, NetworkLearningSettings,
        NetworkSceneProfile, ProxyMode, ProxySettings, SchedulerMode, SchedulerSettings,
        StartDownloadRequest, TaskId, TaskKind, ThreadMode, TraditionalSchedulerSettings,
        default_http_user_agent, default_tracker_list_url,
    },
};

const DEFAULT_FIXED_THREADS: usize = 8;
const DEFAULT_RETRIES: u32 = 4;
const PERSIST_INTERVAL: Duration = Duration::from_millis(300);
const SCHEDULER_TICK: Duration = Duration::from_secs(2);
const MAX_TRADITIONAL_THREADS: usize = 32;

#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<DownloadManager>,
    pub torrent_manager: Arc<TorrentManager>,
    pub sftp_manager: Arc<super::sftp::SftpManager>,
    pub cdn_accelerator: Arc<super::cdn::CdnAccelerator>,
    pub app_handle: tauri::AppHandle,
    pub rpc_shutdown: Arc<std::sync::Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
}

impl AppState {
    pub async fn emit_all_downloads(&self) {
        let mut summaries = Vec::new();

        if let Ok(http) = self.manager.list().await {
            summaries.extend(
                http.into_iter().map(|mut summary| {
                    summary.id = TaskId::make_http(summary.id);
                    summary
                }),
            );
        }
        if let Ok(bt) = self.torrent_manager.list().await {
            summaries.extend(bt);
        }
        if let Ok(sftp) = self.sftp_manager.list().await {
            summaries.extend(sftp);
        }

        for summary in &summaries {
            let _ = self.app_handle.emit("download-updated", summary);
        }
    }
}

pub struct DownloadManager {
    client: Arc<RwLock<Client>>,
    state_dir: PathBuf,
    settings_path: PathBuf,
    settings: Arc<RwLock<AppSettings>>,
    downloads: Arc<RwLock<HashMap<String, Arc<ManagedDownload>>>>,
    db: Arc<Database>,
    rebalance_notify: Arc<Notify>,
    event_tx: Arc<Mutex<Option<broadcast::Sender<String>>>>,
}

struct ManagedDownload {
    snapshot: Mutex<DownloadSnapshot>,
    manifest: Mutex<Manifest>,
    runtime: Mutex<Option<CancellationToken>>,
    aimd: Mutex<AimdState>,
}

impl ManagedDownload {
    fn lock_snapshot(&self) -> MutexGuard<'_, DownloadSnapshot> {
        match self.snapshot.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("snapshot lock poisoned, recovering with inner state");
                poisoned.into_inner()
            }
        }
    }

    fn lock_manifest(&self) -> MutexGuard<'_, Manifest> {
        match self.manifest.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("manifest lock poisoned, recovering with inner state");
                poisoned.into_inner()
            }
        }
    }

    fn lock_runtime(&self) -> MutexGuard<'_, Option<CancellationToken>> {
        match self.runtime.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("runtime lock poisoned, recovering with inner state");
                poisoned.into_inner()
            }
        }
    }

    fn lock_aimd(&self) -> MutexGuard<'_, AimdState> {
        match self.aimd.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("aimd lock poisoned, recovering with inner state");
                poisoned.into_inner()
            }
        }
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
    pub fn new(state_dir: PathBuf) -> Result<Self> {
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

        let manager = Self {
            client: Arc::new(RwLock::new(client)),
            state_dir,
            settings_path,
            settings: Arc::new(RwLock::new(settings)),
            downloads: Arc::new(RwLock::new(HashMap::new())),
            db,
            rebalance_notify: Arc::new(Notify::new()),
            event_tx: Arc::new(Mutex::new(None)),
        };

        manager.load_downloads_from_db()?;
        Ok(manager)
    }

    pub async fn settings(&self) -> Result<AppSettings> {
        Ok(self.settings.read().await.clone())
    }

    pub fn initial_settings(&self) -> AppSettings {
        self.settings.blocking_read().clone()
    }

    pub fn set_event_tx(&self, tx: broadcast::Sender<String>) {
        *self.event_tx.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("event_tx lock poisoned, recovering with inner state");
            poisoned.into_inner()
        }) = Some(tx);
    }

    pub async fn update_settings(&self, settings: AppSettings) -> Result<AppSettings> {
        let normalized = normalize_settings(settings)?;
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
        let thread_note = Some(String::from("等待服务器响应"));
        let now = now_ms();

        let manifest = Manifest {
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
            error: None,
            created_at_ms: now,
            updated_at_ms: now,
            chunks: Vec::new(),
        };

        let snapshot = snapshot_from_manifest(&manifest);
        let managed = Arc::new(ManagedDownload {
            snapshot: Mutex::new(snapshot),
            manifest: Mutex::new(manifest),
            runtime: Mutex::new(None),
            aimd: Mutex::new(AimdState::initial(adaptive_profile, desired_thread_count)),
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

    pub async fn start_metalink(&self, request: StartDownloadRequest) -> Result<String> {
        let settings = self.settings.read().await.clone();
        if !settings.download.enable_metalink {
            return Err(DownloadError::InvalidResponse(String::from(
                "metalink support is disabled in settings",
            )));
        }

        let user_agent = resolve_user_agent(
            request.user_agent.as_deref(),
            &settings.download.default_user_agent,
        )?;
        let content = self
            .load_metalink_source(request.url.trim(), &user_agent)
            .await?;
        let entries = parse_metalink(&content)?;
        let is_single_file = entries.len() == 1;
        let mut first_id = None;
        let mut errors = Vec::new();

        for (index, entry) in entries.into_iter().enumerate() {
            let mut next = request.clone();
            next.kind = Some(TaskKind::Http);
            next.url = entry.url;
            next.file_name = if is_single_file {
                request.file_name.clone().or(entry.file_name)
            } else {
                entry.file_name.or_else(|| {
                    request
                        .file_name
                        .as_ref()
                        .map(|name| numbered_file_name(name, index + 1))
                })
            };
            next.checksum = request.checksum.or(entry.checksum_mode);

            match self.start(next).await {
                Ok(id) if first_id.is_none() => first_id = Some(id),
                Ok(_) => {}
                Err(error) => errors.push(error.to_string()),
            }
        }

        first_id.ok_or_else(|| {
            DownloadError::InvalidResponse(format!(
                "metalink did not start any downloads{}",
                if errors.is_empty() {
                    String::new()
                } else {
                    format!(": {}", errors.join("; "))
                }
            ))
        })
    }

    async fn load_metalink_source(&self, source: &str, user_agent: &str) -> Result<String> {
        if source.is_empty() {
            return Err(DownloadError::InvalidResponse(String::from(
                "metalink source is empty",
            )));
        }

        if let Ok(url) = Url::parse(source) {
            return match url.scheme() {
                "http" | "https" => {
                    const MAX_METALINK_BYTES: usize = 8 * 1024 * 1024;

                    let response = self
                        .client
                        .read()
                        .await
                        .get(source)
                        .header(header::USER_AGENT, user_agent)
                        .send()
                        .await?
                        .error_for_status()?;
                    let bytes = response.bytes().await?;
                    if bytes.len() > MAX_METALINK_BYTES {
                        return Err(DownloadError::InvalidResponse(String::from(
                            "metalink document is larger than 8 MiB",
                        )));
                    }

                    String::from_utf8(bytes.to_vec()).map_err(|error| {
                        DownloadError::InvalidResponse(format!(
                            "metalink document is not utf-8: {error}"
                        ))
                    })
                }
                "file" => {
                    let path = url.to_file_path().map_err(|_| {
                        DownloadError::InvalidResponse(String::from("invalid metalink file url"))
                    })?;
                    async_fs::read_to_string(path)
                        .await
                        .map_err(DownloadError::Io)
                }
                _ => Err(DownloadError::UnsupportedScheme),
            };
        }

        async_fs::read_to_string(source)
            .await
            .map_err(DownloadError::Io)
    }

    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        {
            let mut snapshot = managed.lock_snapshot();
            if !matches!(
                snapshot.state,
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Queued
            ) {
                return Ok(snapshot.clone());
            }
            snapshot.state = DownloadState::Paused;
            snapshot.connection_count = 0;
            snapshot.allocated_thread_count = Some(0);
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.lock_manifest();
            manifest.state = DownloadState::Paused;
            manifest.connection_count = 0;
            manifest.allocated_thread_count = Some(0);
            manifest.updated_at_ms = now_ms();
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
            let mut snapshot = managed.lock_snapshot();
            snapshot.state = DownloadState::Canceled;
            snapshot.connection_count = 0;
            snapshot.allocated_thread_count = Some(0);
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.lock_manifest();
            manifest.state = DownloadState::Canceled;
            manifest.connection_count = 0;
            manifest.allocated_thread_count = Some(0);
            manifest.updated_at_ms = now_ms();
        }
        let token = { managed.lock_runtime().clone() };
        if let Some(token) = token {
            token.cancel();
            self.wait_until_stopped(&managed).await;
        }
        self.cleanup_files(&managed)?;
        self.downloads.write().await.remove(download_id);
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
        let manifest = managed.lock_manifest().clone();
        let destination_path = PathBuf::from(&manifest.destination_path);
        let directory_path = PathBuf::from(&manifest.destination_dir);

        if destination_path.exists() {
            #[cfg(windows)]
            { Command::new("explorer")
                .arg(format!("/select,{}", destination_path.display()))
                .spawn()?;
            }
            return Ok(());
        }

        if directory_path.exists() {
            #[cfg(windows)]
            { Command::new("explorer").arg(&directory_path).spawn()?; }
            return Ok(());
        }

        Err(DownloadError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            "download location does not exist",
        )))
    }

    pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        {
            let snapshot = managed.lock_snapshot();
            if matches!(
                snapshot.state,
                DownloadState::Downloading
                    | DownloadState::Retrying
                    | DownloadState::Queued
                    | DownloadState::Verifying
            ) {
                return Err(DownloadError::AlreadyRunning);
            }
            if matches!(snapshot.state, DownloadState::Canceled) {
                return Err(DownloadError::Canceled);
            }
            if matches!(snapshot.state, DownloadState::Completed) {
                return Err(DownloadError::NotResumable);
            }
        }

        {
            let mut snapshot = managed.lock_snapshot();
            snapshot.state = DownloadState::Queued;
            snapshot.error = None;
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.lock_manifest();
            manifest.state = DownloadState::Queued;
            manifest.error = None;
            manifest.updated_at_ms = now_ms();
        }
        {
            let manifest = managed.lock_manifest().clone();
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
        items.sort_by(|left, right| right.id.cmp(&left.id));
        Ok(items)
    }

    pub(crate) fn start_scheduler_loop(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::select! {
                    _ = sleep(SCHEDULER_TICK) => {}
                    _ = self.rebalance_notify.notified() => {}
                }

                if let Err(error) = self.update_adaptive_targets().await {
                    log_background_error("update adaptive targets", &error);
                }
                if let Err(error) = self.rebalance_allocations().await {
                    log_background_error("rebalance allocations", &error);
                }
            }
        });
    }

    fn load_downloads_from_db(&self) -> Result<()> {
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
            });

            self.downloads
                .blocking_write()
                .insert(manifest.id.clone(), managed);
        }
        Ok(())
    }

    async fn probe(&self, url: &str, user_agent: &str) -> Result<RemoteMetadata> {
        let client = self.client.read().await.clone();
        let head = client
            .head(url)
            .header(header::USER_AGENT, user_agent)
            .send()
            .await;
        let response = match head {
            Ok(response) if response.status().is_success() => response,
            _ => {
                client
                    .get(url)
                    .header(header::USER_AGENT, user_agent)
                    .header(header::RANGE, "bytes=0-0")
                    .send()
                    .await?
            }
        };

        validate_probe_response(&response)?;

        let final_url = response.url().to_string();
        let headers = response.headers().clone();
        let status = response.status();

        let total_bytes = extract_total_bytes(status, &headers);
        let supports_ranges = supports_ranges(status, &headers);
        let file_name =
            infer_file_name(&final_url, &headers).ok_or(DownloadError::MissingFileName)?;

        Ok(RemoteMetadata {
            final_url,
            file_name,
            total_bytes,
            etag: header_string(&headers, header::ETAG),
            last_modified: header_string(&headers, header::LAST_MODIFIED),
            supports_ranges,
        })
    }

    async fn spawn_download(&self, managed: Arc<ManagedDownload>, max_retries: u32) -> Result<()> {
        {
            let mut runtime = managed.lock_runtime();
            if runtime.is_some() {
                return Ok(());
            }
            *runtime = Some(CancellationToken::new());
        }

        let client = self.client.read().await.clone();
        let manager = self.clone_arc();
        let token = managed
            .lock_runtime()
            .clone()
            .ok_or_else(|| anyhow::anyhow!("runtime token not set after initialization"))?;

        self.persist(managed.clone()).await?;

        tauri::async_runtime::spawn(async move {
            let result = manager
                .run_download(managed.clone(), client, token.clone(), max_retries)
                .await;
            if let Err(error) = result {
                if matches!(error, DownloadError::Interrupted) {
                    let mut runtime = managed.lock_runtime();
                    *runtime = None;
                    return;
                }
                {
                    let mut snapshot = managed.lock_snapshot();
                    snapshot.state = DownloadState::Failed;
                    snapshot.error = Some(error.to_string());
                    snapshot.connection_count = 0;
                    snapshot.allocated_thread_count = Some(0);
                    snapshot.updated_at_ms = now_ms();
                }
                {
                    let mut manifest = managed.lock_manifest();
                    manifest.state = DownloadState::Failed;
                    manifest.error = Some(error.to_string());
                    manifest.connection_count = 0;
                    manifest.allocated_thread_count = Some(0);
                    manifest.updated_at_ms = now_ms();
                }
                if let Err(error) = manager.learn_from_download(managed.clone()).await {
                    log_background_error("learn from failed download", &error);
                }

                // Broadcast aria2.onDownloadError
                let event_tx = manager.event_tx.lock().unwrap_or_else(|poisoned| {
                    tracing::warn!("event_tx lock poisoned in spawn_download");
                    poisoned.into_inner()
                });
                if let Some(ref tx) = *event_tx {
                    let download_id = managed.lock_snapshot().id.clone();
                    let gid = format!("{:016x}", xxhash_rust::xxh3::xxh3_64(download_id.as_bytes()));
                    let _ = tx.send(build_rpc_notification("aria2.onDownloadError", &gid));
                }
            }

            let should_persist = {
                let snapshot = managed.lock_snapshot();
                snapshot.state != DownloadState::Canceled
            };
            if should_persist
                && let Err(error) = manager.persist(managed.clone()).await
            {
                log_background_error("persist background download state", &error);
            }
            let mut runtime = managed.lock_runtime();
            *runtime = None;
            manager.rebalance_notify.notify_waiters();
        });

        Ok(())
    }

    async fn run_download(
        &self,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<()> {
        let current_manifest = { managed.lock_manifest().clone() };
        let metadata = self
            .probe(&current_manifest.url, &current_manifest.user_agent)
            .await?;

        let supports_parallel =
            supports_parallelism(metadata.total_bytes, metadata.supports_ranges);
        let settings = self.settings.read().await.clone();
        let request = StartDownloadRequest {
            kind: Some(TaskKind::Http),
            url: current_manifest.url.clone(),
            destination_dir: current_manifest.destination_dir.clone(),
            file_name: Some(current_manifest.file_name.clone()),
            user_agent: Some(current_manifest.user_agent.clone()),
            thread_mode: Some(current_manifest.thread_mode),
            thread_count: current_manifest.requested_thread_count,
            max_retries: None,
            checksum: Some(current_manifest.checksum_mode),
        };
        let (thread_mode, requested_thread_count, desired_thread_count, adaptive_profile) =
            resolve_thread_settings(&settings, &request, supports_parallel);
        let mut reset_progress = false;
        let mut force_single_stream_restart = false;
        let mut refresh_aimd = false;
        {
            let mut manifest = managed.lock_manifest();
            if !manifest.file_name_locked && manifest.downloaded_bytes == 0 {
                let safe_name = sanitize_filename::sanitize(&metadata.file_name);
                if !safe_name.is_empty() && safe_name != manifest.file_name {
                    let destination_dir = PathBuf::from(&manifest.destination_dir);
                    manifest.file_name = safe_name.clone();
                    manifest.destination_path =
                        unique_destination_path(&destination_dir, &safe_name)
                            .to_string_lossy()
                            .to_string();
                }
                manifest.file_name_locked = true;
            }
            if validators_changed(&manifest, &metadata)
                || manifest.total_bytes != metadata.total_bytes
                || manifest.supports_ranges != supports_parallel
                || (supports_parallel && manifest.chunks.is_empty())
            {
                manifest.downloaded_bytes = 0;
                manifest.chunks = plan_chunks(metadata.total_bytes, supports_parallel);
                manifest.checksum = None;
                manifest.supports_ranges = supports_parallel;
                reset_progress = true;
            } else if !supports_parallel && has_partial_chunk_progress(&manifest) {
                manifest.downloaded_bytes = 0;
                manifest.connection_count = 1;
                manifest.supports_ranges = false;
                manifest.chunks.clear();
                manifest.checksum = None;
                reset_progress = true;
                force_single_stream_restart = true;
            }
            manifest.final_url = metadata.final_url.clone();
            manifest.supports_ranges = supports_parallel;
            manifest.total_bytes = metadata.total_bytes;
            manifest.etag = metadata.etag.clone();
            manifest.last_modified = metadata.last_modified.clone();
            manifest.thread_mode = thread_mode;
            manifest.requested_thread_count = requested_thread_count;
            if manifest.desired_thread_count != desired_thread_count
                || manifest.adaptive_profile_snapshot != adaptive_profile
            {
                refresh_aimd = true;
            }
            manifest.desired_thread_count = desired_thread_count;
            manifest.adaptive_profile_snapshot = adaptive_profile;
            manifest.thread_note = thread_note(supports_parallel, thread_mode, adaptive_profile);
            manifest.updated_at_ms = now_ms();
            manifest.error = None;
            if !supports_parallel {
                manifest.thread_note = Some(String::from("单线程（服务器不支持分段）"));
                manifest.desired_thread_count = Some(1);
            }
            sync_snapshot_with_manifest(&managed, &manifest);
        }
        if refresh_aimd {
            let mut aimd = managed.lock_aimd();
            *aimd = AimdState::initial(adaptive_profile, desired_thread_count);
        }
        self.rebalance_allocations().await?;
        self.rebalance_notify.notify_waiters();
        if reset_progress {
            self.prepare_fresh_temp_file(&managed)?;
            if force_single_stream_restart {
                self.reset_progress(&managed, true);
            }
        }

        let outcome = if supports_parallel {
            self.download_chunked(managed.clone(), client.clone(), token.clone(), max_retries)
                .await?
        } else {
            self.download_single(managed.clone(), client.clone(), token.clone(), max_retries)
                .await?
        };

        match outcome {
            RunOutcome::Finished => {
                self.finalize_download(managed.clone()).await?;
                if let Err(error) = self.learn_from_download(managed.clone()).await {
                    log_background_error("learn from completed download", &error);
                }
            }
            RunOutcome::Paused => {
                {
                    let mut snapshot = managed.lock_snapshot();
                    snapshot.state = DownloadState::Paused;
                    snapshot.connection_count = 0;
                    snapshot.allocated_thread_count = Some(0);
                    snapshot.updated_at_ms = now_ms();
                }

                {
                    let mut manifest = managed.lock_manifest();
                    manifest.state = DownloadState::Paused;
                    manifest.connection_count = 0;
                    manifest.allocated_thread_count = Some(0);
                    manifest.updated_at_ms = now_ms();
                }
                if let Err(error) = self.learn_from_download(managed.clone()).await {
                    log_background_error("learn from paused download", &error);
                }
            }
            RunOutcome::Canceled => {
                self.cleanup_files(&managed)?;
            }
        }

        Ok(())
    }

    async fn download_single(
        &self,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<RunOutcome> {
        let file_path = PathBuf::from(
            managed.lock_manifest()
                .temp_path
                .clone(),
        );
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = open_download_file(
            &file_path,
            managed.lock_manifest()
                .total_bytes,
        )?;

        let mut last_persist = Instant::now();

        loop {
            match self.wait_until_active(&managed, &token).await {
                WaitState::Running => {}
                WaitState::Paused => return Ok(RunOutcome::Paused),
                WaitState::Canceled => return Ok(RunOutcome::Canceled),
            }

            let (url, user_agent, validator, state) = {
                let manifest = managed.lock_manifest();
                (
                    manifest.final_url.clone(),
                    manifest.user_agent.clone(),
                    if_range_header(&manifest),
                    manifest.state,
                )
            };
            if state == DownloadState::Canceled {
                return Ok(RunOutcome::Canceled);
            }
            if token.is_cancelled() {
                return Ok(cancellation_outcome(&managed));
            }

            let start_offset = {
                let manifest = managed.lock_manifest();
                contiguous_prefix_end(&manifest)
            };

            let response = request_with_retry(
                || {
                    let client = client.clone();
                    let url = url.clone();
                    let user_agent = user_agent.clone();
                    let validator = validator.clone();
                    async move {
                        let mut builder = client.get(url).header(header::USER_AGENT, user_agent);
                        if start_offset > 0 {
                            builder =
                                builder.header(header::RANGE, format!("bytes={start_offset}-"));
                            if let Some((name, value)) = validator {
                                builder = builder.header(name, value);
                            }
                        }
                        builder.send().await
                    }
                },
                token.clone(),
                max_retries,
                managed.clone(),
            )
            .await?;

            let status = response.status();
            let mut stream = response.bytes_stream();
            let mut absolute_offset = if status == StatusCode::PARTIAL_CONTENT && start_offset > 0 {
                start_offset
            } else {
                if start_offset > 0 {
                    reset_download_file(
                        &file,
                        managed.lock_manifest()
                            .total_bytes,
                    )?;
                    self.reset_progress(&managed, true);
                }
                0
            };

            {
                let mut snapshot = managed.lock_snapshot();
                snapshot.state = DownloadState::Downloading;
                snapshot.connection_count = 1;
                snapshot.updated_at_ms = now_ms();
                let mut manifest = managed.lock_manifest();
                manifest.state = DownloadState::Downloading;
                manifest.connection_count = 1;
                manifest.updated_at_ms = now_ms();
            }

            while let Some(chunk) = tokio::select! {
                _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                chunk = stream.next() => chunk,
            } {
                let chunk = chunk?;
                write_all_at(&file, &chunk, absolute_offset)?;
                absolute_offset += chunk.len() as u64;
                self.record_progress(&managed, None, chunk.len() as u64);
                if last_persist.elapsed() >= PERSIST_INTERVAL {
                    persist_manifest_snapshot(&self.db, &managed).await?;
                    last_persist = Instant::now();
                }
            }

            let finished = {
                let manifest = managed.lock_manifest();
                match manifest.total_bytes {
                    Some(total) => manifest.downloaded_bytes >= total,
                    None => true,
                }
            };
            if finished {
                return Ok(RunOutcome::Finished);
            }
        }
    }

    async fn download_chunked(
        &self,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<RunOutcome> {
        let (file_path, total_size) = {
            let manifest = managed.lock_manifest();
            (
                PathBuf::from(manifest.temp_path.clone()),
                manifest.total_bytes,
            )
        };
        let file = Arc::new(open_download_file(&file_path, total_size)?);
        let mut workers = JoinSet::new();
        let mut next_worker_id = 0usize;

        loop {
            if token.is_cancelled() {
                return Ok(cancellation_outcome(&managed));
            }

            if all_chunks_completed(&managed) {
                return Ok(RunOutcome::Finished);
            }

            let allocation = current_allocation(&managed);
            if allocation == 0 && workers.is_empty() {
                match self.wait_until_active(&managed, &token).await {
                    WaitState::Running => {}
                    WaitState::Paused => return Ok(RunOutcome::Paused),
                    WaitState::Canceled => return Ok(RunOutcome::Canceled),
                }
            }

            let target_workers = current_allocation(&managed);
            while workers.len() < target_workers {
                let worker_id = next_worker_id;
                let chunk = {
                    let mut manifest = managed.lock_manifest();
                    claim_next_chunk(&mut manifest, worker_id)
                };
                let Some(chunk) = chunk else {
                    break;
                };

                {
                    let mut snapshot = managed.lock_snapshot();
                    snapshot.state = DownloadState::Downloading;
                    snapshot.connection_count = target_workers;
                    snapshot.updated_at_ms = now_ms();
                }
                {
                    let mut manifest = managed.lock_manifest();
                    manifest.state = DownloadState::Downloading;
                    manifest.connection_count = target_workers;
                    manifest.updated_at_ms = now_ms();
                }

                let db = self.db.clone();
                let managed = managed.clone();
                let client = client.clone();
                let token = token.clone();
                let file = file.clone();
                workers.spawn(async move {
                    download_chunk(managed, client, token, file, chunk, max_retries, db).await
                });
                next_worker_id = next_worker_id.saturating_add(1);
            }

            if workers.is_empty() {
                tokio::select! {
                    _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                    _ = self.rebalance_notify.notified() => {}
                    _ = sleep(Duration::from_millis(120)) => {}
                }
                continue;
            }

            let join_result = tokio::select! {
                _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                joined = workers.join_next() => joined,
            };

            let Some(join_result) = join_result else {
                continue;
            };
            match join_result
                .map_err(|error| DownloadError::InvalidResponse(error.to_string()))??
            {
                ChunkWorkerOutcome::Finished => {}
                ChunkWorkerOutcome::RestartSingle => {
                    self.reset_progress(&managed, true);
                    return self
                        .download_single(managed, client, token, max_retries)
                        .await;
                }
                ChunkWorkerOutcome::Paused => return Ok(RunOutcome::Paused),
                ChunkWorkerOutcome::Canceled => return Ok(RunOutcome::Canceled),
            }
        }
    }

    async fn finalize_download(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        {
            let mut snapshot = managed.lock_snapshot();
            snapshot.state = DownloadState::Verifying;
            snapshot.connection_count = 0;
            snapshot.allocated_thread_count = Some(0);
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.lock_manifest();
            manifest.state = DownloadState::Verifying;
            manifest.connection_count = 0;
            manifest.allocated_thread_count = Some(0);
            manifest.updated_at_ms = now_ms();
        }
        self.persist(managed.clone()).await?;

        let (temp_path, destination_path, checksum_mode) = {
            let manifest = managed.lock_manifest();
            (
                PathBuf::from(manifest.temp_path.clone()),
                PathBuf::from(manifest.destination_path.clone()),
                manifest.checksum_mode,
            )
        };

        let checksum = match checksum_mode {
            ChecksumMode::None => None,
            mode => Some(calculate_checksum(temp_path.clone(), mode).await?),
        };

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)?;
        }
        finalize_temp_file(&temp_path, &destination_path)?;

        {
            let mut snapshot = managed.lock_snapshot();
            snapshot.state = DownloadState::Completed;
            snapshot.downloaded_bytes = snapshot.total_bytes.unwrap_or(snapshot.downloaded_bytes);
            snapshot.checksum = checksum.clone();
            snapshot.destination_path = destination_path.to_string_lossy().to_string();
            snapshot.error = None;
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.lock_manifest();
            manifest.state = DownloadState::Completed;
            manifest.downloaded_bytes = manifest.total_bytes.unwrap_or(manifest.downloaded_bytes);
            manifest.checksum = checksum;
            manifest.destination_path = destination_path.to_string_lossy().to_string();
            manifest.error = None;
            manifest.updated_at_ms = now_ms();
            for chunk in &mut manifest.chunks {
                chunk.completed = true;
                chunk.downloaded = chunk.end.saturating_sub(chunk.start) + 1;
                chunk.claimed_by = None;
            }
        }
        self.persist(managed.clone()).await?;

        // Broadcast aria2.onDownloadComplete via RPC event channel
        let event_tx = self.event_tx.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("event_tx lock poisoned in finalize_download");
            poisoned.into_inner()
        });
        if let Some(ref tx) = *event_tx {
            let download_id = managed.lock_snapshot().id.clone();
            let gid = format!("{:016x}", xxhash_rust::xxh3::xxh3_64(download_id.as_bytes()));
            let _ = tx.send(build_rpc_notification("aria2.onDownloadComplete", &gid));
        }

        Ok(())
    }

    async fn persist(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_manifest().clone();
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || db.insert_download(&manifest))
            .await
            .context("persist task panicked")?
            .context("failed to persist download to database")?;
        Ok(())
    }

    async fn update_adaptive_targets(&self) -> Result<()> {
        let settings = self.settings.read().await.clone();
        if settings.scheduler.mode != SchedulerMode::Automatic {
            return Ok(());
        }

        if settings.proxy.mode != ProxyMode::Disabled {
            return Ok(());
        }

        let learning_metrics = active_learning_metrics(&settings.network_learning);
        let adaptive_cap = active_scene_thread_cap(&settings.network_learning)
            .unwrap_or(settings.scheduler.automatic.max_threads_per_task.max(1))
            .min(settings.scheduler.automatic.max_threads_per_task.max(1));
        let min_threads = settings.scheduler.automatic.min_threads_per_task.max(1);

        let downloads = self.downloads.read().await;
        for managed in downloads.values() {
            let mut manifest = managed.lock_manifest();
            if manifest.thread_mode != ThreadMode::Adaptive
                || manifest.state != DownloadState::Downloading
                || !manifest.supports_ranges
            {
                continue;
            }

            let mut aimd = managed.lock_aimd();
            let now = Instant::now();
            let throughput = aimd.sample_throughput( manifest.downloaded_bytes, now)
                .unwrap_or_else(|| {
                    manifest
                        .allocated_thread_count
                        .unwrap_or(0)
                        .saturating_mul(1) as f64
                });

            let current = manifest.desired_thread_count.unwrap_or(1).max(1);
            let allocated = manifest.allocated_thread_count.unwrap_or(0);
            let profile = manifest
                .adaptive_profile_snapshot
                .unwrap_or(settings.scheduler.automatic.adaptive_profile);

            if let Some(cooldown_until) = aimd.cooldown_until
                && now < cooldown_until
            {
                aimd.recent_penalty = false;
                continue;
            }

            let mut degrade_threshold: f64 = match profile {
                AdaptiveProfile::Conservative => 0.18,
                AdaptiveProfile::Balanced => 0.16,
                AdaptiveProfile::Aggressive => 0.20,
            };
            let mut increase_threshold: f64 = match profile {
                AdaptiveProfile::Conservative => 0.08,
                AdaptiveProfile::Balanced => 0.04,
                AdaptiveProfile::Aggressive => 0.0,
            };
            let mut samples_needed: u32 = match profile {
                AdaptiveProfile::Conservative => 2,
                AdaptiveProfile::Balanced => 1,
                AdaptiveProfile::Aggressive => 1,
            };
            let mut cooldown = aimd::cooldown_for_profile(profile);

            if let Some(metrics) = learning_metrics {
                if metrics.penalty_rate >= 0.2 || metrics.stability_score <= 0.6 {
                    degrade_threshold *= 0.7;
                    increase_threshold += 0.05;
                    samples_needed = samples_needed.saturating_add(1);
                    cooldown += Duration::from_secs(2);
                } else if metrics.stability_score >= 0.88
                    && metrics.penalty_rate <= 0.06
                    && metrics.estimated_bandwidth_bps >= 6.0 * 1024.0 * 1024.0
                {
                    degrade_threshold += 0.03;
                    increase_threshold = (increase_threshold * 0.5).max(0.0);
                    samples_needed = samples_needed.saturating_sub(1).max(1);
                    cooldown = cooldown.saturating_sub(Duration::from_secs(1));
                }
            }

            degrade_threshold = match profile {
                AdaptiveProfile::Conservative => degrade_threshold,
                AdaptiveProfile::Balanced => degrade_threshold.max(0.16),
                AdaptiveProfile::Aggressive => degrade_threshold.max(0.20),
            };

            let throughput_drop = aimd
                .last_throughput
                .is_some_and(|last| last > 0.0 && throughput < last * (1.0 - degrade_threshold));
            let should_decrease = current > 1
                && match profile {
                    AdaptiveProfile::Conservative => aimd.recent_penalty || throughput_drop,
                    AdaptiveProfile::Balanced | AdaptiveProfile::Aggressive => throughput_drop,
                };

            if should_decrease {
                manifest.desired_thread_count = Some(aimd::reduce_threads(current, profile, min_threads));
                manifest.updated_at_ms = now_ms();
                aimd.cooldown_until = Some(now + cooldown);
                aimd.consecutive_good_samples = 0;
                aimd.consecutive_bad_samples = aimd.consecutive_bad_samples.saturating_add(1);
                aimd.recent_penalty = false;
                aimd.record_sample( throughput);
                continue;
            }

            if allocated == current {
                let improved = match aimd.last_throughput {
                    Some(last) if last > 0.0 => throughput >= last * (1.0 + increase_threshold),
                    _ => true,
                };

                if improved {
                    aimd.consecutive_good_samples = aimd.consecutive_good_samples.saturating_add(1);
                    aimd.consecutive_bad_samples = 0;
                    if aimd.consecutive_good_samples >= samples_needed {
                        let next = (current + 1).min(adaptive_cap.max(1));
                        manifest.desired_thread_count = Some(next);
                        manifest.updated_at_ms = now_ms();
                        aimd.consecutive_good_samples = 0;
                    }
                }
            }

            aimd.last_throughput = Some(throughput);
            aimd.recent_penalty = false;
            aimd.record_sample( throughput);
        }

        Ok(())
    }

    async fn learn_from_download(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_manifest().clone();
        if manifest.thread_mode != ThreadMode::Adaptive {
            return Ok(());
        }

        let settings = self.settings.read().await.clone();
        if settings.scheduler.mode != SchedulerMode::Automatic
            || settings.network_learning.device_mode == DeviceLearningMode::Mobile
            || settings.proxy.mode != ProxyMode::Disabled
        {
            return Ok(());
        }

        let Some(scene) = settings.network_learning.scenes.first() else {
            return Ok(());
        };
        if !scene.learning_enabled {
            return Ok(());
        }

        let sample = {
            let aimd = managed.lock_aimd();
            build_learning_sample(&manifest, &aimd, &settings)?
        };
        let alpha = match settings.network_learning.device_mode {
            DeviceLearningMode::Fixed => 0.45,
            DeviceLearningMode::SemiMobile => 0.22,
            DeviceLearningMode::Mobile => 0.0,
        };

        let scheduler_cap = settings.scheduler.automatic.max_threads_per_task.max(1);
        let now = now_ms();
        let mut next_settings = settings.clone();
        let scene = &mut next_settings.network_learning.scenes[0];
        let next_metrics =
            blend_learning_metrics(scene.learned_metrics.as_ref(), sample, alpha, scheduler_cap);
        scene.learned_metrics = Some(next_metrics.clone());
        scene.updated_at_ms = now;

        let db = self.db.clone();
        let scene_id = scene.id.clone();
        tokio::task::spawn_blocking(move || db.upsert_learning_metrics(&scene_id, &next_metrics))
            .await
            .context("learn metrics persist task panicked")?
            .context("failed to persist learning metrics")?;

        *self.settings.write().await = next_settings;
        Ok(())
    }

    async fn rebalance_allocations(&self) -> Result<()> {
        let settings = self.settings.read().await.clone();
        let downloads = self.downloads.read().await;
        let mut entries = downloads.values().cloned().collect::<Vec<_>>();

        match settings.scheduler.mode {
            SchedulerMode::Traditional => {
                entries.sort_by_key(|managed| {
                    managed.lock_manifest()
                        .created_at_ms
                });

                let mut running = 0usize;
                for managed in entries {
                    let mut manifest = managed.lock_manifest();
                    let terminal = matches!(
                        manifest.state,
                        DownloadState::Paused
                            | DownloadState::Completed
                            | DownloadState::Failed
                            | DownloadState::Canceled
                            | DownloadState::Verifying
                    );
                    if terminal {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                        sync_snapshot_with_manifest(&managed, &manifest);
                        continue;
                    }

                    if running < settings.scheduler.traditional.max_parallel_tasks {
                        let allocation = effective_allocation_cap(&manifest, &settings).max(1);
                        manifest.allocated_thread_count = Some(allocation);
                        manifest.connection_count = allocation;
                        manifest.state = DownloadState::Downloading;
                        running = running.saturating_add(1);
                    } else {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                        manifest.state = DownloadState::Queued;
                    }
                    manifest.updated_at_ms = now_ms();
                    sync_snapshot_with_manifest(&managed, &manifest);
                }
            }
            SchedulerMode::Automatic => {
                let mut candidates = entries
                    .into_iter()
                    .filter(|managed| {
                        let manifest = managed.lock_manifest();
                        !matches!(
                            manifest.state,
                            DownloadState::Paused
                                | DownloadState::Completed
                                | DownloadState::Failed
                                | DownloadState::Canceled
                                | DownloadState::Verifying
                        )
                    })
                    .collect::<Vec<_>>();

                candidates.sort_by(|left, right| {
                    remaining_bytes(&right.lock_manifest()).cmp(
                        &remaining_bytes(&left.lock_manifest()),
                    )
                });

                let mut remaining_budget = settings.scheduler.automatic.max_parallel_threads;
                let min_per_task = settings.scheduler.automatic.min_threads_per_task.max(1);
                let mut allocations: HashMap<String, usize> = HashMap::new();
                for managed in &candidates {
                    let manifest = managed.lock_manifest();
                    if remaining_budget == 0 {
                        allocations.insert(manifest.id.clone(), 0);
                        continue;
                    }
                    let start = if remaining_budget >= min_per_task {
                        min_per_task
                    } else {
                        remaining_budget
                    };
                    allocations.insert(manifest.id.clone(), start);
                    remaining_budget = remaining_budget.saturating_sub(start);
                }

                while remaining_budget > 0 {
                    let mut granted = false;
                    for managed in &candidates {
                        let manifest = managed.lock_manifest();
                        let entry = allocations.entry(manifest.id.clone()).or_insert(0);
                        let cap = effective_allocation_cap(&manifest, &settings);
                        if *entry < cap {
                            *entry += 1;
                            remaining_budget -= 1;
                            granted = true;
                            if remaining_budget == 0 {
                                break;
                            }
                        }
                    }

                    if !granted {
                        break;
                    }
                }

                for managed in downloads.values() {
                    let mut manifest = managed.lock_manifest();
                    let allocation = allocations.get(&manifest.id).copied().unwrap_or(0);
                    if matches!(
                        manifest.state,
                        DownloadState::Paused
                            | DownloadState::Completed
                            | DownloadState::Failed
                            | DownloadState::Canceled
                            | DownloadState::Verifying
                    ) {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                    } else if allocation == 0 {
                        manifest.allocated_thread_count = Some(0);
                        manifest.connection_count = 0;
                        manifest.state = DownloadState::Queued;
                    } else {
                        manifest.allocated_thread_count = Some(allocation);
                        manifest.connection_count = allocation;
                        if manifest.state != DownloadState::Retrying {
                            manifest.state = DownloadState::Downloading;
                        }
                    }
                    manifest.updated_at_ms = now_ms();
                    sync_snapshot_with_manifest(managed, &manifest);
                }
            }
        }

        let mut first_error = None;
        for managed in downloads.values() {
            if let Err(error) = persist_manifest_snapshot(&self.db, managed).await {
                log_background_error("persist rebalanced manifest", &error);
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }
}

fn is_terminal(state: DownloadState) -> bool {
    matches!(state, DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled)
}

impl DownloadManager {
    fn build_snapshot(&self, managed: Arc<ManagedDownload>) -> DownloadSnapshot {
        let mut snapshot = managed.lock_snapshot().clone();
        let manifest = managed.lock_manifest();
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
            .ok()
            .and_then(|aimd| aimd.last_throughput)
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

    fn record_progress(
        &self,
        managed: &Arc<ManagedDownload>,
        chunk_index: Option<usize>,
        bytes: u64,
    ) {
        let now = now_ms();
        {
            let mut snapshot = managed.lock_snapshot();
            snapshot.downloaded_bytes = snapshot.downloaded_bytes.saturating_add(bytes);
            snapshot.error = None;
            snapshot.updated_at_ms = now;
        }
        {
            let mut manifest = managed.lock_manifest();
            manifest.downloaded_bytes = manifest.downloaded_bytes.saturating_add(bytes);
            manifest.error = None;
            manifest.updated_at_ms = now;
            if let Some(index) = chunk_index
                && let Some(chunk) = manifest
                    .chunks
                    .iter_mut()
                    .find(|candidate| candidate.index == index)
            {
                chunk.downloaded = chunk.downloaded.saturating_add(bytes);
                if chunk.downloaded > chunk.end.saturating_sub(chunk.start) {
                    chunk.completed = true;
                    chunk.claimed_by = None;
                }
            }
        }
    }

    fn reset_progress(&self, managed: &Arc<ManagedDownload>, force_single_stream: bool) {
        let now = now_ms();
        {
            let mut snapshot = managed.lock_snapshot();
            snapshot.downloaded_bytes = 0;
            snapshot.updated_at_ms = now;
            if force_single_stream {
                snapshot.connection_count = 1;
                snapshot.supports_ranges = false;
                snapshot.desired_thread_count = Some(1);
                snapshot.allocated_thread_count = Some(1);
                snapshot.thread_note = Some(String::from("单线程（服务器不支持分段）"));
            }
        }
        {
            let mut manifest = managed.lock_manifest();
            manifest.downloaded_bytes = 0;
            manifest.updated_at_ms = now;
            for chunk in &mut manifest.chunks {
                chunk.downloaded = 0;
                chunk.completed = false;
                chunk.claimed_by = None;
            }
            if force_single_stream {
                manifest.connection_count = 1;
                manifest.supports_ranges = false;
                manifest.chunks.clear();
                manifest.desired_thread_count = Some(1);
                manifest.allocated_thread_count = Some(1);
                manifest.thread_note = Some(String::from("单线程（服务器不支持分段）"));
            }
        }
    }

    fn cleanup_files(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_manifest().clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        if temp_path.exists() {
            remove_file_if_exists(&temp_path)?;
        }
        Ok(())
    }

    fn cleanup_destination_file(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_manifest().clone();
        let destination_path = PathBuf::from(manifest.destination_path);
        if destination_path.exists() {
            fs::remove_file(destination_path)?;
        }
        Ok(())
    }

    fn prepare_fresh_temp_file(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.lock_manifest().clone();
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
        for _ in 0..1000 {
            if managed.lock_runtime().is_none() {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
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
            {
                let mut snapshot = managed.lock_snapshot();
                snapshot.state = DownloadState::Canceled;
                snapshot.connection_count = 0;
                snapshot.allocated_thread_count = Some(0);
                snapshot.updated_at_ms = now_ms();
            }
            {
                let mut manifest = managed.lock_manifest();
                manifest.state = DownloadState::Canceled;
                manifest.connection_count = 0;
                manifest.allocated_thread_count = Some(0);
                manifest.updated_at_ms = now_ms();
            }
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
                let manifest = managed.lock_manifest();
                if manifest.state == DownloadState::Canceled {
                    return WaitState::Canceled;
                }
                if manifest.state == DownloadState::Paused {
                    return WaitState::Paused;
                }
                if manifest.allocated_thread_count.unwrap_or(0) > 0 {
                    return WaitState::Running;
                }
            }

            tokio::select! {
                _ = token.cancelled() => return match managed.lock_snapshot().state {
                    DownloadState::Canceled => WaitState::Canceled,
                    _ => WaitState::Paused,
                },
                _ = self.rebalance_notify.notified() => {}
                _ = sleep(Duration::from_millis(120)) => {}
            }
        }
    }

    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            client: self.client.clone(),
            state_dir: self.state_dir.clone(),
            settings_path: self.settings_path.clone(),
            settings: self.settings.clone(),
            downloads: self.downloads.clone(),
            db: self.db.clone(),
            rebalance_notify: self.rebalance_notify.clone(),
            event_tx: self.event_tx.clone(),
        })
    }
}

#[derive(Debug)]
enum WaitState {
    Running,
    Paused,
    Canceled,
}

fn supports_parallelism(total: Option<u64>, supports_ranges: bool) -> bool {
    supports_ranges && total.map(|value| value >= CHUNK_SIZE * 2).unwrap_or(false)
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
                let learned_initial = active_learning_metrics(&settings.network_learning)
                    .map(|metrics| metrics.recommended_initial_threads);
                let learned_cap = active_scene_thread_cap(&settings.network_learning)
                    .unwrap_or(settings.scheduler.automatic.max_threads_per_task.max(1));
                let desired = learned_initial
                    .unwrap_or_else(|| aimd::initial_desired_threads(profile))
                    .min(learned_cap.max(1))
                    .min(settings.scheduler.automatic.max_threads_per_task.max(1));
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

fn remaining_bytes(manifest: &Manifest) -> u64 {
    manifest
        .total_bytes
        .unwrap_or(manifest.downloaded_bytes)
        .saturating_sub(manifest.downloaded_bytes)
}

fn effective_allocation_cap(manifest: &Manifest, settings: &AppSettings) -> usize {
    if !manifest.supports_ranges {
        return 1;
    }

    match settings.scheduler.mode {
        SchedulerMode::Traditional => manifest
            .requested_thread_count
            .or(manifest.desired_thread_count)
            .unwrap_or(DEFAULT_FIXED_THREADS)
            .clamp(1, MAX_TRADITIONAL_THREADS),
        SchedulerMode::Automatic => {
            let desired = match manifest.thread_mode {
                ThreadMode::Fixed => manifest.requested_thread_count.unwrap_or(1),
                ThreadMode::Adaptive => manifest.desired_thread_count.unwrap_or(1),
            };
            desired.clamp(1, effective_automatic_task_cap(settings))
        }
    }
}

fn effective_automatic_task_cap(settings: &AppSettings) -> usize {
    active_scene_thread_cap(&settings.network_learning)
        .unwrap_or(settings.scheduler.automatic.max_threads_per_task.max(1))
        .min(settings.scheduler.automatic.max_threads_per_task.max(1))
        .max(1)
}

fn active_learning_metrics(settings: &NetworkLearningSettings) -> Option<&NetworkLearningMetrics> {
    if settings.device_mode == DeviceLearningMode::Mobile {
        return None;
    }

    settings
        .scenes
        .first()
        .filter(|scene| scene.learning_enabled)
        .and_then(|scene| scene.learned_metrics.as_ref())
}

fn active_scene_thread_cap(settings: &NetworkLearningSettings) -> Option<usize> {
    active_learning_metrics(settings).map(|metrics| metrics.recommended_max_threads_per_task_cap)
}

fn current_allocation(managed: &Arc<ManagedDownload>) -> usize {
    managed.lock_manifest()
        .allocated_thread_count
        .unwrap_or(0)
}

fn all_chunks_completed(managed: &Arc<ManagedDownload>) -> bool {
    managed.lock_manifest()
        .chunks
        .iter()
        .all(|chunk| chunk.completed)
}

fn claim_next_chunk(manifest: &mut Manifest, worker_id: usize) -> Option<ChunkManifest> {
    let chunk = manifest
        .chunks
        .iter_mut()
        .find(|chunk| !chunk.completed && chunk.claimed_by.is_none())?;
    chunk.claimed_by = Some(worker_id);
    Some(chunk.clone())
}

fn mark_chunk_released(managed: &Arc<ManagedDownload>, chunk_index: usize) {
    let mut manifest = managed.lock_manifest();
    if let Some(chunk) = manifest
        .chunks
        .iter_mut()
        .find(|chunk| chunk.index == chunk_index)
    {
        chunk.claimed_by = None;
    }
}

fn sync_snapshot_with_manifest(managed: &Arc<ManagedDownload>, manifest: &Manifest) {
    let mut snapshot = managed.lock_snapshot();
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
    {
        let mut snapshot = managed.lock_snapshot();
        snapshot.downloaded_bytes = snapshot.downloaded_bytes.saturating_add(bytes);
        snapshot.error = None;
        snapshot.updated_at_ms = now;
    }
    {
        let mut manifest = managed.lock_manifest();
        manifest.downloaded_bytes = manifest.downloaded_bytes.saturating_add(bytes);
        manifest.error = None;
        manifest.updated_at_ms = now;
        if let Some(index) = chunk_index
            && let Some(chunk) = manifest
                .chunks
                .iter_mut()
                .find(|candidate| candidate.index == index)
        {
            chunk.downloaded = chunk.downloaded.saturating_add(bytes);
            if chunk.downloaded > chunk.end.saturating_sub(chunk.start) {
                chunk.completed = true;
                chunk.claimed_by = None;
            }
        }
    }
}

fn default_network_scene() -> NetworkSceneProfile {
    NetworkSceneProfile {
        id: String::from("default"),
        name: String::from("默认场景"),
        learning_enabled: true,
        learned_metrics: None,
        updated_at_ms: 0,
    }
}

fn normalize_network_learning_settings(
    settings: NetworkLearningSettings,
    scheduler_cap: usize,
) -> NetworkLearningSettings {
    let mut scenes = settings.scenes;
    let selected_scene = scenes
        .iter()
        .position(|scene| scene.id == settings.current_scene_id)
        .map(|index| scenes.remove(index))
        .or_else(|| scenes.into_iter().next());

    let mut scene = selected_scene.unwrap_or_else(default_network_scene);
    scene.id = String::from("default");
    scene.name = String::from("默认场景");
    scene.learned_metrics = scene
        .learned_metrics
        .map(|metrics| normalize_learning_metrics(metrics, scheduler_cap));

    NetworkLearningSettings {
        device_mode: settings.device_mode,
        current_scene_id: String::from("default"),
        scenes: vec![scene],
    }
}

fn normalize_learning_metrics(
    metrics: NetworkLearningMetrics,
    scheduler_cap: usize,
) -> NetworkLearningMetrics {
    NetworkLearningMetrics {
        estimated_bandwidth_bps: metrics.estimated_bandwidth_bps.max(0.0),
        stability_score: metrics.stability_score.clamp(0.0, 1.0),
        penalty_rate: metrics.penalty_rate.clamp(0.0, 1.0),
        recommended_initial_threads: metrics.recommended_initial_threads.clamp(1, scheduler_cap),
        recommended_max_threads_per_task_cap: metrics
            .recommended_max_threads_per_task_cap
            .clamp(1, scheduler_cap)
            .max(metrics.recommended_initial_threads.clamp(1, scheduler_cap)),
        sample_count: metrics.sample_count,
        last_observed_at_ms: metrics.last_observed_at_ms,
    }
}

fn build_learning_sample(
    manifest: &Manifest,
    aimd: &AimdState,
    settings: &AppSettings,
) -> Result<NetworkLearningMetrics> {
    let profile = manifest
        .adaptive_profile_snapshot
        .unwrap_or(settings.scheduler.automatic.adaptive_profile);
    let scheduler_cap = settings.scheduler.automatic.max_threads_per_task.max(1);
    let throughput = if aimd.throughput_sample_count > 0 {
        (aimd.throughput_sum / f64::from(aimd.throughput_sample_count)).max(0.0_f64)
    } else {
        0.0_f64
    };
    let penalty_rate = if aimd.throughput_sample_count > 0 {
        f64::from(aimd.penalty_count) / f64::from(aimd.throughput_sample_count.max(1))
    } else if aimd.penalty_count > 0 {
        1.0_f64
    } else {
        0.0_f64
    };

    let mut stability_score = (1.0 - penalty_rate * 1.35).clamp(0.0, 1.0);
    stability_score = match manifest.state {
        DownloadState::Completed => (stability_score + 0.08).clamp(0.0, 1.0),
        DownloadState::Failed => (stability_score - 0.3).clamp(0.0, 1.0),
        DownloadState::Paused => (stability_score - 0.08).clamp(0.0, 1.0),
        _ => stability_score,
    };

    let base_threads = recommended_threads_from_bandwidth(throughput, profile);
    let recommended_initial_threads =
        adjust_threads_for_stability(base_threads, stability_score, penalty_rate)
            .clamp(1, scheduler_cap);
    let recommended_max_threads_per_task_cap = derive_recommended_cap(
        recommended_initial_threads,
        stability_score,
        penalty_rate,
        scheduler_cap,
    );

    Ok(NetworkLearningMetrics {
        estimated_bandwidth_bps: throughput,
        stability_score,
        penalty_rate,
        recommended_initial_threads,
        recommended_max_threads_per_task_cap,
        sample_count: 1,
        last_observed_at_ms: now_ms(),
    })
}

fn blend_learning_metrics(
    previous: Option<&NetworkLearningMetrics>,
    sample: NetworkLearningMetrics,
    alpha: f64,
    scheduler_cap: usize,
) -> NetworkLearningMetrics {
    let Some(previous) = previous else {
        return normalize_learning_metrics(sample, scheduler_cap);
    };

    let alpha = alpha.clamp(0.05, 0.95);
    let blended_initial = ((previous.recommended_initial_threads as f64) * (1.0 - alpha)
        + (sample.recommended_initial_threads as f64) * alpha)
        .round() as usize;
    let blended_cap = ((previous.recommended_max_threads_per_task_cap as f64) * (1.0 - alpha)
        + (sample.recommended_max_threads_per_task_cap as f64) * alpha)
        .round() as usize;

    normalize_learning_metrics(
        NetworkLearningMetrics {
            estimated_bandwidth_bps: previous.estimated_bandwidth_bps * (1.0 - alpha)
                + sample.estimated_bandwidth_bps * alpha,
            stability_score: previous.stability_score * (1.0 - alpha)
                + sample.stability_score * alpha,
            penalty_rate: previous.penalty_rate * (1.0 - alpha) + sample.penalty_rate * alpha,
            recommended_initial_threads: blended_initial,
            recommended_max_threads_per_task_cap: blended_cap.max(blended_initial),
            sample_count: previous.sample_count.saturating_add(1),
            last_observed_at_ms: sample.last_observed_at_ms,
        },
        scheduler_cap,
    )
}

fn recommended_threads_from_bandwidth(throughput: f64, profile: AdaptiveProfile) -> usize {
    if throughput <= 0.0 {
        return aimd::initial_desired_threads(profile);
    }

    match throughput {
        value if value < 1.0 * 1024.0 * 1024.0 => 1,
        value if value < 4.0 * 1024.0 * 1024.0 => 2,
        value if value < 12.0 * 1024.0 * 1024.0 => 4,
        _ => 6,
    }
}

fn adjust_threads_for_stability(threads: usize, stability_score: f64, penalty_rate: f64) -> usize {
    if penalty_rate >= 0.25 || stability_score <= 0.55 {
        return threads.saturating_sub(1).max(1);
    }
    if penalty_rate <= 0.05 && stability_score >= 0.9 {
        return threads.saturating_add(1);
    }
    threads
}

fn derive_recommended_cap(
    recommended_initial_threads: usize,
    stability_score: f64,
    penalty_rate: f64,
    scheduler_cap: usize,
) -> usize {
    let extra = if penalty_rate <= 0.05 && stability_score >= 0.9 {
        3
    } else if penalty_rate <= 0.12 && stability_score >= 0.75 {
        2
    } else {
        1
    };

    recommended_initial_threads
        .saturating_add(extra)
        .clamp(recommended_initial_threads, scheduler_cap)
}

fn normalize_proxy_settings(settings: ProxySettings) -> Result<ProxySettings> {
    match settings.mode {
        ProxyMode::Disabled | ProxyMode::System => Ok(ProxySettings {
            mode: settings.mode,
            manual_url: String::new(),
        }),
        ProxyMode::Manual => {
            let manual_url = settings.manual_url.trim().to_string();
            if manual_url.is_empty() {
                return Err(DownloadError::InvalidProxy(String::from(
                    "manual proxy url is required",
                )));
            }

            Url::parse(&manual_url)
                .map_err(|error| DownloadError::InvalidProxy(error.to_string()))?;

            Ok(ProxySettings {
                mode: ProxyMode::Manual,
                manual_url,
            })
        }
    }
}

fn normalize_settings(settings: AppSettings) -> Result<AppSettings> {
    let proxy = normalize_proxy_settings(settings.proxy)?;
    let max_parallel_tasks = settings
        .scheduler
        .traditional
        .max_parallel_tasks
        .clamp(1, 32);
    let max_parallel_threads = settings
        .scheduler
        .automatic
        .max_parallel_threads
        .clamp(1, 64);
    let max_threads_per_task = settings
        .scheduler
        .automatic
        .max_threads_per_task
        .clamp(1, 32)
        .min(max_parallel_threads);
    let network_learning =
        normalize_network_learning_settings(settings.network_learning, max_threads_per_task.max(1));
    let bt = normalize_bt_settings(settings.bt)?;
    let logging = normalize_logging_settings(settings.logging);
    let default_user_agent = normalize_user_agent(&settings.download.default_user_agent)?;

    Ok(AppSettings {
        appearance: settings.appearance,
        proxy,
        scheduler: SchedulerSettings {
            mode: settings.scheduler.mode,
            traditional: TraditionalSchedulerSettings { max_parallel_tasks },
            automatic: AutomaticSchedulerSettings {
                max_parallel_threads,
                max_threads_per_task,
                min_threads_per_task: normalize_min_threads(
                    settings.scheduler.automatic.min_threads_per_task,
                    max_threads_per_task,
                ),
                adaptive_profile: settings.scheduler.automatic.adaptive_profile,
            },
        },
        download: DownloadDefaultsSettings {
            default_download_dir: settings.download.default_download_dir.trim().to_string(),
            default_max_retries: settings.download.default_max_retries.clamp(0, 20),
            default_checksum: settings.download.default_checksum,
            default_user_agent,
            enable_metalink: settings.download.enable_metalink,
            enable_sftp: settings.download.enable_sftp,
        },
        bt,
        network_learning,
        logging,
        aria2_rpc: settings.aria2_rpc.clone(),
        cdn_acceleration: settings.cdn_acceleration.clone(),
    })
}

fn normalize_min_threads(raw: usize, max_per_task: usize) -> usize {
    if raw == 0 {
        (max_per_task / 2).max(1)
    } else {
        raw.clamp(1, max_per_task)
    }
}

fn normalize_logging_settings(settings: LogSettings) -> LogSettings {
    LogSettings {
        enabled: settings.enabled,
        level: settings.level,
        file_path: settings.file_path.trim().to_string(),
    }
}

fn normalize_bt_settings(settings: BtSettings) -> Result<BtSettings> {
    const MAX_UPLOAD_LIMIT_BYTES: u64 = 10 * 1024 * 1024 * 1024 * 1024;
    let tracker_list = normalize_tracker_list(&settings.tracker_list)?;
    let tracker_list_url = normalize_tracker_list_url(&settings.tracker_list_url)?;

    Ok(BtSettings {
        dht_enabled: settings.dht_enabled,
        pex_enabled: settings.pex_enabled,
        tracker_list,
        tracker_list_url,
        pause_upload_when_limit_reached: settings.pause_upload_when_limit_reached,
        upload_limit_bytes: settings.upload_limit_bytes.min(MAX_UPLOAD_LIMIT_BYTES),
        upload_ratio_limit: if settings.upload_ratio_limit.is_finite() {
            settings.upload_ratio_limit.clamp(0.0, 100.0)
        } else {
            0.0
        },
    })
}

fn normalize_user_agent(user_agent: &str) -> Result<String> {
    let normalized = user_agent.trim();
    if normalized.is_empty() {
        return Ok(default_http_user_agent());
    }
    if normalized.len() > 512 || header::HeaderValue::from_str(normalized).is_err() {
        return Err(DownloadError::InvalidResponse(String::from(
            "invalid user-agent value",
        )));
    }

    Ok(normalized.to_string())
}

fn resolve_user_agent(
    request_user_agent: Option<&str>,
    default_user_agent: &str,
) -> Result<String> {
    match request_user_agent
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(user_agent) => normalize_user_agent(user_agent),
        None => normalize_user_agent(default_user_agent),
    }
}

pub(super) fn normalize_tracker_list(tracker_list: &str) -> Result<String> {
    let mut normalized = Vec::new();

    for raw_tracker in tracker_list.lines() {
        let tracker = raw_tracker.trim();
        if tracker.is_empty() {
            continue;
        }

        normalized.push(parse_tracker_url(tracker)?);
    }

    Ok(finalize_tracker_list(normalized))
}

pub(super) fn normalize_tracker_list_lossy(tracker_list: &str) -> String {
    let normalized = tracker_list
        .lines()
        .map(str::trim)
        .filter(|tracker| !tracker.is_empty())
        .filter_map(|tracker| parse_tracker_url(tracker).ok())
        .collect::<Vec<_>>();

    finalize_tracker_list(normalized)
}

fn parse_tracker_url(tracker: &str) -> Result<String> {
    let parsed =
        Url::parse(tracker).map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https" | "udp") {
        return Err(DownloadError::InvalidResponse(format!(
            "unsupported tracker scheme: {}",
            parsed.scheme()
        )));
    }

    Ok(parsed.to_string())
}

fn finalize_tracker_list(mut normalized: Vec<String>) -> String {
    normalized.sort();
    normalized.dedup();
    normalized.join("\n")
}

pub(super) fn normalize_tracker_list_url(tracker_list_url: &str) -> Result<String> {
    let tracker_list_url = tracker_list_url.trim();
    if tracker_list_url.is_empty() {
        return Ok(default_tracker_list_url());
    }

    let parsed = Url::parse(tracker_list_url)
        .map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(DownloadError::InvalidResponse(format!(
            "unsupported tracker list url scheme: {}",
            parsed.scheme()
        )));
    }

    Ok(parsed.to_string())
}

fn build_http_client(settings: &AppSettings) -> Result<Client> {
    let default_user_agent = normalize_user_agent(&settings.download.default_user_agent)?;
    let mut builder = Client::builder()
        .redirect(Policy::limited(10))
        .tcp_nodelay(true)
        .read_timeout(Duration::from_secs(15))
        .user_agent(default_user_agent);

    match settings.proxy.mode {
        ProxyMode::Disabled => {
            builder = builder.no_proxy();
        }
        ProxyMode::System => {}
        ProxyMode::Manual => {
            let proxy = Proxy::all(&settings.proxy.manual_url)
                .map_err(|error| DownloadError::InvalidProxy(error.to_string()))?;
            builder = builder.proxy(proxy);
        }
    }

    builder.build().map_err(DownloadError::from)
}

fn load_settings(settings_path: &Path) -> Result<AppSettings> {
    let content = match fs::read_to_string(settings_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppSettings::default());
        }
        Err(error) => return Err(error.into()),
    };

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content)
        && (value.get("appearance").is_some()
            || value.get("proxy").is_some()
            || value.get("scheduler").is_some()
            || value.get("download").is_some()
            || value.get("bt").is_some()
            || value.get("networkLearning").is_some()
            || value.get("logging").is_some()
            || value.get("cdnAcceleration").is_some())
    {
        let parsed = serde_json::from_value::<AppSettings>(value)?;
        return normalize_settings(parsed);
    }

    let legacy_proxy = serde_json::from_str::<ProxySettings>(&content)?;
    normalize_settings(AppSettings {
        appearance: Default::default(),
        proxy: legacy_proxy,
        scheduler: SchedulerSettings::default(),
        download: DownloadDefaultsSettings::default(),
        bt: BtSettings::default(),
        network_learning: NetworkLearningSettings::default(),
        logging: LogSettings::default(),
        aria2_rpc: Aria2RpcSettings::default(),
        cdn_acceleration: CdnAccelerationSettings::default(),
    })
}

async fn persist_settings(settings_path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let temp_path = settings_path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, serde_json::to_vec_pretty(settings)?).await?;
    tokio::fs::rename(&temp_path, settings_path).await?;
    Ok(())
}

async fn download_chunk(
    managed: Arc<ManagedDownload>,
    client: Client,
    token: CancellationToken,
    file: Arc<std::fs::File>,
    chunk: ChunkManifest,
    max_retries: u32,
    db: Arc<Database>,
) -> Result<ChunkWorkerOutcome> {
    let mut current = chunk.start + chunk.downloaded;
    let end = chunk.end;
    if current > end {
        mark_chunk_released(&managed, chunk.index);
        return Ok(ChunkWorkerOutcome::Finished);
    }

    let mut last_persist = Instant::now();
    while current <= end {
        if token.is_cancelled() {
            mark_chunk_released(&managed, chunk.index);
            return Ok(
                match managed.lock_snapshot().state {
                    DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
                    _ => ChunkWorkerOutcome::Paused,
                },
            );
        }

        let (url, user_agent, validator) = {
            let manifest = managed.lock_manifest();
            (
                manifest.final_url.clone(),
                manifest.user_agent.clone(),
                if_range_header(&manifest),
            )
        };

        let response = request_with_retry(
            || {
                let client = client.clone();
                let url = url.clone();
                let user_agent = user_agent.clone();
                let validator = validator.clone();
                async move {
                    build_segment_request(&client, &url, &user_agent, current, end, validator)
                        .send()
                        .await
                }
            },
            token.clone(),
            max_retries,
            managed.clone(),
        )
        .await?;

        if response.status() == StatusCode::OK && current > chunk.start {
            mark_chunk_released(&managed, chunk.index);
            return Ok(ChunkWorkerOutcome::RestartSingle);
        }

        validate_segment_response(&response, current, end)?;

        let mut stream = response.bytes_stream();
        while let Some(bytes) = tokio::select! {
            _ = token.cancelled() => {
                mark_chunk_released(&managed, chunk.index);
                return Ok(cancellation_chunk_outcome(&managed));
            }
            next = stream.next() => next,
        } {
            let bytes = bytes?;
            if current + bytes.len() as u64 - 1 > end {
                mark_chunk_released(&managed, chunk.index);
                return Err(DownloadError::InvalidResponse(String::from(
                    "segment body exceeded requested range",
                )));
            }

            write_all_at(&file, &bytes, current)?;
            current += bytes.len() as u64;
            {
                record_progress_on_managed(&managed, Some(chunk.index), bytes.len() as u64);
            }
            if last_persist.elapsed() >= PERSIST_INTERVAL {
                persist_manifest_snapshot(&db, &managed).await?;
                last_persist = Instant::now();
            }
        }
    }

    {
        let mut manifest = managed.lock_manifest();
        if let Some(target) = manifest
            .chunks
            .iter_mut()
            .find(|candidate| candidate.index == chunk.index)
        {
            target.completed = true;
            target.downloaded = target.end.saturating_sub(target.start) + 1;
            target.claimed_by = None;
        }
        manifest.updated_at_ms = now_ms();
    }
    Ok(ChunkWorkerOutcome::Finished)
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

fn numbered_file_name(file_name: &str, index: usize) -> String {
    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(file_name);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();

    format!("{stem}-{index}{extension}")
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn request_with_retry<F, Fut>(
    mut factory: F,
    token: CancellationToken,
    max_retries: u32,
    managed: Arc<ManagedDownload>,
) -> Result<Response>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<Response, reqwest::Error>>,
{
    let mut attempt = 0;
    loop {
        if token.is_cancelled() {
            return Err(DownloadError::Interrupted);
        }

        let response = tokio::select! {
            _ = token.cancelled() => return Err(DownloadError::Interrupted),
            response = factory() => response,
        };

        match response {
            Ok(response) => match classify_download_response(response) {
                ResponseDisposition::Use(response) => return Ok(response),
                ResponseDisposition::Retryable(status) => {
                    if attempt >= max_retries {
                        return Err(DownloadError::InvalidResponse(format!(
                            "http status {status}"
                        )));
                    }
                    attempt += 1;
                    register_retry_penalty(&managed, format!("http status {status}"));
                    tokio::select! {
                        _ = token.cancelled() => return Err(DownloadError::Interrupted),
                        _ = sleep(backoff_delay(attempt)) => {}
                    }
                }
                ResponseDisposition::Invalid(status) => {
                    return Err(DownloadError::InvalidResponse(format!(
                        "http status {status}"
                    )));
                }
            },
            Err(error) => {
                if attempt >= max_retries {
                    return Err(error.into());
                }
                attempt += 1;
                register_retry_penalty(&managed, error.to_string());
                tokio::select! {
                    _ = token.cancelled() => return Err(DownloadError::Interrupted),
                    _ = sleep(backoff_delay(attempt)) => {}
                }
            }
        }
    }
}

fn register_retry_penalty(managed: &Arc<ManagedDownload>, error: String) {
    {
        let mut snapshot = managed.lock_snapshot();
        snapshot.state = DownloadState::Retrying;
        snapshot.error = Some(error.clone());
        snapshot.updated_at_ms = now_ms();
    }
    {
        let mut manifest = managed.lock_manifest();
        manifest.state = DownloadState::Retrying;
        manifest.error = Some(error);
        manifest.updated_at_ms = now_ms();
    }
    let mut aimd = managed.lock_aimd();
    aimd.recent_penalty = true;
    aimd.penalty_count = aimd.penalty_count.saturating_add(1);
}

async fn persist_manifest_snapshot(
    db: &Arc<Database>,
    managed: &Arc<ManagedDownload>,
) -> Result<()> {
    let manifest = managed.lock_manifest().clone();
    let state_text = super::database::download_state_to_text(&manifest.state);
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

fn log_background_error(context: &str, error: impl std::fmt::Display) {
    eprintln!("[downloader] {context}: {error}");
}

fn cancellation_outcome(managed: &Arc<ManagedDownload>) -> RunOutcome {
    match managed.lock_snapshot().state {
        DownloadState::Canceled => RunOutcome::Canceled,
        _ => RunOutcome::Paused,
    }
}

fn cancellation_chunk_outcome(managed: &Arc<ManagedDownload>) -> ChunkWorkerOutcome {
    match managed.lock_snapshot().state {
        DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
        _ => ChunkWorkerOutcome::Paused,
    }
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis((250_u64).saturating_mul(2_u64.saturating_pow(attempt.min(4))))
}

async fn calculate_checksum(path: PathBuf, mode: ChecksumMode) -> Result<String> {
    tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut hasher = ChecksumHasher::new(mode);
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hasher.finalize())
    })
    .await
    .map_err(|error| DownloadError::InvalidResponse(error.to_string()))?
}

enum ChecksumHasher {
    Blake3(Box<blake3::Hasher>),
    Sha256(sha2::Sha256),
    Xxh3_128(Box<xxhash_rust::xxh3::Xxh3>),
}

impl ChecksumHasher {
    fn new(mode: ChecksumMode) -> Self {
        match mode {
            ChecksumMode::None => unreachable!("none checksum mode is handled before hashing"),
            ChecksumMode::Blake3 => Self::Blake3(Box::new(blake3::Hasher::new())),
            ChecksumMode::Sha256 => {
                use sha2::Digest;
                Self::Sha256(sha2::Sha256::new())
            }
            ChecksumMode::Xxh3128 => Self::Xxh3_128(Box::new(xxhash_rust::xxh3::Xxh3::new())),
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Blake3(hasher) => {
                hasher.update(bytes);
            }
            Self::Sha256(hasher) => {
                use sha2::Digest;
                hasher.update(bytes);
            }
            Self::Xxh3_128(hasher) => {
                hasher.update(bytes);
            }
        }
    }

    fn finalize(self) -> String {
        match self {
            Self::Blake3(hasher) => hasher.finalize().to_hex().to_string(),
            Self::Sha256(hasher) => {
                use sha2::Digest;
                format!("{:x}", hasher.finalize())
            }
            Self::Xxh3_128(hasher) => format!("{:032x}", hasher.digest128()),
        }
    }
}

fn build_rpc_notification(method: &str, gid: &str) -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": [{"gid": gid}]
    }))
    .unwrap_or_default()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

#[cfg(test)]
#[path = "tests/manager_tests.rs"]
mod tests;
