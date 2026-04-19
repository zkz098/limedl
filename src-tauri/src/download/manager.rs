use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use futures_util::StreamExt;
use reqwest::{
    header,
    redirect::Policy,
    Client, Proxy, Response, StatusCode, Url,
};
use tokio::{
    sync::{Mutex as AsyncMutex, RwLock},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{
    error::{DownloadError, Result},
    file_alloc::{finalize_temp_file, open_download_file, reset_download_file, write_all_at},
    http::{
        build_segment_request, classify_download_response, extract_total_bytes, header_string,
        if_range_header, infer_file_name, supports_ranges, validate_probe_response,
        validate_segment_response, ResponseDisposition,
    },
    manifest::{
        compute_connection_count, contiguous_prefix_end, has_partial_segment_progress, plan_segments,
        snapshot_from_manifest, validators_changed, Manifest, RemoteMetadata, SegmentManifest,
    },
    types::{ChecksumMode, DownloadSnapshot, DownloadState, DownloadSummary, StartDownloadRequest},
    types::{ProxyMode, ProxySettings},
};

const DEFAULT_CONNECTIONS: usize = 8;
const DEFAULT_RETRIES: u32 = 4;
const PERSIST_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<DownloadManager>,
}

impl AppState {
    pub fn new(manager: DownloadManager) -> Self {
        Self {
            manager: Arc::new(manager),
        }
    }
}

pub struct DownloadManager {
    client: Arc<RwLock<Client>>,
    state_dir: PathBuf,
    settings_path: PathBuf,
    proxy_settings: Arc<RwLock<ProxySettings>>,
    downloads: Arc<RwLock<HashMap<String, Arc<ManagedDownload>>>>,
    persist_lock: Arc<AsyncMutex<()>>,
}

struct ManagedDownload {
    snapshot: Mutex<DownloadSnapshot>,
    manifest: Mutex<Manifest>,
    runtime: Mutex<Option<CancellationToken>>,
    persist_lock: Arc<AsyncMutex<()>>,
}

#[derive(Debug)]
enum RunOutcome {
    Finished,
    Paused,
    Canceled,
}

#[derive(Debug)]
enum SegmentOutcome {
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
        let proxy_settings = load_proxy_settings(&settings_path)?;
        let client = build_http_client(&proxy_settings)?;

        let manager = Self {
            client: Arc::new(RwLock::new(client)),
            state_dir,
            settings_path,
            proxy_settings: Arc::new(RwLock::new(proxy_settings)),
            downloads: Arc::new(RwLock::new(HashMap::new())),
            persist_lock: Arc::new(AsyncMutex::new(())),
        };

        manager.load_existing_manifests()?;

        Ok(manager)
    }

    pub async fn proxy_settings(&self) -> Result<ProxySettings> {
        Ok(self.proxy_settings.read().await.clone())
    }

    pub async fn update_proxy_settings(&self, settings: ProxySettings) -> Result<ProxySettings> {
        let normalized = normalize_proxy_settings(settings)?;
        let next_client = build_http_client(&normalized)?;

        persist_proxy_settings(&self.settings_path, &normalized).await?;
        *self.proxy_settings.write().await = normalized.clone();
        *self.client.write().await = next_client;

        Ok(normalized)
    }

    pub async fn start(&self, request: StartDownloadRequest) -> Result<String> {
        let url = Url::parse(&request.url)
            .map_err(|error| DownloadError::InvalidResponse(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(DownloadError::UnsupportedScheme);
        }

        let download_id = Uuid::new_v4().to_string();
        let metadata = self.probe(&request.url).await?;
        let destination_dir = PathBuf::from(&request.destination_dir);
        fs::create_dir_all(&destination_dir)?;

        let chosen_name = request
            .file_name
            .clone()
            .unwrap_or(metadata.file_name.clone());
        let safe_name = sanitize_filename::sanitize(&chosen_name);
        if safe_name.is_empty() {
            return Err(DownloadError::MissingFileName);
        }

        let destination_path = unique_destination_path(&destination_dir, &safe_name);
        let temp_path = self.state_dir.join(format!("{download_id}.part"));
        let manifest_path = self.state_dir.join(format!("{download_id}.json"));
        let connection_count = compute_connection_count(
            metadata.total_bytes,
            metadata.supports_ranges,
            request.max_connections.unwrap_or(DEFAULT_CONNECTIONS),
        );

        let manifest = Manifest {
            id: download_id.clone(),
            url: request.url.clone(),
            final_url: metadata.final_url.clone(),
            destination_dir: destination_dir.to_string_lossy().to_string(),
            file_name: safe_name.clone(),
            destination_path: destination_path.to_string_lossy().to_string(),
            temp_path: temp_path.to_string_lossy().to_string(),
            manifest_path: manifest_path.to_string_lossy().to_string(),
            total_bytes: metadata.total_bytes,
            downloaded_bytes: 0,
            supports_ranges: metadata.supports_ranges,
            connection_count,
            etag: metadata.etag.clone(),
            last_modified: metadata.last_modified.clone(),
            state: DownloadState::Queued,
            checksum_mode: request.checksum.clone().unwrap_or_default(),
            checksum: None,
            error: None,
            created_at_ms: now_ms(),
            updated_at_ms: now_ms(),
            segments: plan_segments(
                metadata.total_bytes,
                metadata.supports_ranges,
                connection_count,
            ),
        };

        let snapshot = snapshot_from_manifest(&manifest);
        let managed = Arc::new(ManagedDownload {
            snapshot: Mutex::new(snapshot),
            manifest: Mutex::new(manifest),
            runtime: Mutex::new(None),
            persist_lock: self.persist_lock.clone(),
        });

        self.persist(managed.clone()).await?;
        self.downloads
            .write()
            .await
            .insert(download_id.clone(), managed.clone());

        self.spawn_download(managed, request.max_retries.unwrap_or(DEFAULT_RETRIES))
            .await?;

        Ok(download_id)
    }

    pub async fn pause(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            if !matches!(
                snapshot.state,
                DownloadState::Downloading | DownloadState::Retrying | DownloadState::Queued
            ) {
                return Ok(snapshot.clone());
            }
            snapshot.state = DownloadState::Paused;
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.state = DownloadState::Paused;
            manifest.updated_at_ms = now_ms();
        }

        let token = { managed.runtime.lock().expect("runtime poisoned").clone() };
        if let Some(token) = token {
            token.cancel();
        }

        self.wait_until_stopped(&managed).await;

        self.persist(managed.clone()).await?;
        Ok(self.build_snapshot(managed))
    }

    pub async fn cancel(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.state = DownloadState::Canceled;
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.state = DownloadState::Canceled;
            manifest.updated_at_ms = now_ms();
        }
        let token = { managed.runtime.lock().expect("runtime poisoned").clone() };
        if let Some(token) = token {
            token.cancel();
            self.wait_until_stopped(&managed).await;
            self.cleanup_files(&managed)?;
        } else {
            self.cleanup_files(&managed)?;
        }
        self.downloads.write().await.remove(download_id);
        Ok(self.build_snapshot(managed))
    }

    pub async fn resume(&self, download_id: &str) -> Result<DownloadSnapshot> {
        let managed = self.get(download_id).await?;
        {
            let snapshot = managed.snapshot.lock().expect("snapshot poisoned");
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

        let retry_count = DEFAULT_RETRIES;
        self.spawn_download(managed.clone(), retry_count).await?;
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

    fn load_existing_manifests(&self) -> Result<()> {
        for entry in fs::read_dir(&self.state_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let mut manifest = match serde_json::from_str::<Manifest>(&content) {
                Ok(manifest) => manifest,
                Err(_) => continue,
            };

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
                manifest.updated_at_ms = now_ms();
            }

            let snapshot = snapshot_from_manifest(&manifest);
            let managed = Arc::new(ManagedDownload {
                snapshot: Mutex::new(snapshot),
                manifest: Mutex::new(manifest.clone()),
                runtime: Mutex::new(None),
                persist_lock: self.persist_lock.clone(),
            });

            self.downloads
                .blocking_write()
                .insert(manifest.id.clone(), managed);
        }
        Ok(())
    }

    async fn probe(&self, url: &str) -> Result<RemoteMetadata> {
        let client = self.client.read().await.clone();
        let head = client.head(url).send().await;
        let response = match head {
            Ok(response) if response.status().is_success() => response,
            _ => {
                client
                    .get(url)
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
            let mut runtime = managed.runtime.lock().expect("runtime poisoned");
            if runtime.is_some() {
                return Err(DownloadError::AlreadyRunning);
            }
            *runtime = Some(CancellationToken::new());
        }

        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.state = DownloadState::Queued;
            snapshot.error = None;
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.state = DownloadState::Queued;
            manifest.error = None;
            manifest.updated_at_ms = now_ms();
        }

        self.persist(managed.clone()).await?;

        let client = self.client.read().await.clone();
        let manager = self.clone_arc();
        let token = managed
            .runtime
            .lock()
            .expect("runtime poisoned")
            .clone()
            .expect("runtime just set");

        tauri::async_runtime::spawn(async move {
            let result = manager
                .run_download(managed.clone(), client, token.clone(), max_retries)
                .await;
            if let Err(error) = result {
                if matches!(error, DownloadError::Interrupted) {
                    let mut runtime = managed.runtime.lock().expect("runtime poisoned");
                    *runtime = None;
                    return;
                }
                let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                snapshot.state = DownloadState::Failed;
                snapshot.error = Some(error.to_string());
                snapshot.updated_at_ms = now_ms();
                drop(snapshot);

                let mut manifest = managed.manifest.lock().expect("manifest poisoned");
                manifest.state = DownloadState::Failed;
                manifest.error = Some(error.to_string());
                manifest.updated_at_ms = now_ms();
            }

            let should_persist = {
                let snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                snapshot.state != DownloadState::Canceled
            };
            if should_persist {
                let _ = manager.persist(managed.clone()).await;
            }
            let mut runtime = managed.runtime.lock().expect("runtime poisoned");
            *runtime = None;
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
        let current_manifest = { managed.manifest.lock().expect("manifest poisoned").clone() };
        let metadata = self.probe(&current_manifest.url).await?;

        let mut reset_progress = false;
        let mut force_single_stream_restart = false;
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            if validators_changed(&manifest, &metadata) {
                manifest.downloaded_bytes = 0;
                manifest.segments = plan_segments(
                    metadata.total_bytes,
                    metadata.supports_ranges,
                    manifest.connection_count,
                );
                manifest.checksum = None;
                reset_progress = true;
            } else if !metadata.supports_ranges && has_partial_segment_progress(&manifest) {
                manifest.downloaded_bytes = 0;
                manifest.connection_count = 1;
                manifest.supports_ranges = false;
                manifest.segments.clear();
                manifest.checksum = None;
                reset_progress = true;
                force_single_stream_restart = true;
            }
            manifest.final_url = metadata.final_url.clone();
            manifest.supports_ranges = metadata.supports_ranges;
            manifest.total_bytes = metadata.total_bytes;
            manifest.etag = metadata.etag.clone();
            manifest.last_modified = metadata.last_modified.clone();
            manifest.file_name = manifest.file_name.clone();
            manifest.updated_at_ms = now_ms();
            manifest.error = None;
        }
        if reset_progress {
            self.prepare_fresh_temp_file(&managed)?;
            if force_single_stream_restart {
                self.reset_progress(&managed, true);
            }
        }

        let mode_segmented = {
            let manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.supports_ranges
                && manifest.total_bytes.is_some()
                && manifest.connection_count > 1
        };

        let outcome = if mode_segmented {
            self.download_segmented(managed.clone(), client.clone(), token.clone(), max_retries)
                .await?
        } else {
            self.download_single(managed.clone(), client.clone(), token.clone(), max_retries)
                .await?
        };

        match outcome {
            RunOutcome::Finished => {
                self.finalize_download(managed.clone()).await?;
            }
            RunOutcome::Paused => {
                let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                snapshot.state = DownloadState::Paused;
                snapshot.updated_at_ms = now_ms();
                drop(snapshot);

                let mut manifest = managed.manifest.lock().expect("manifest poisoned");
                manifest.state = DownloadState::Paused;
                manifest.updated_at_ms = now_ms();
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
            managed
                .manifest
                .lock()
                .expect("manifest poisoned")
                .temp_path
                .clone(),
        );
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = open_download_file(
            &file_path,
            managed
                .manifest
                .lock()
                .expect("manifest poisoned")
                .total_bytes,
        )?;

        let mut last_persist = Instant::now();

        loop {
            let (url, validator, state) = {
                let manifest = managed.manifest.lock().expect("manifest poisoned");
                (
                    manifest.final_url.clone(),
                    if_range_header(&manifest),
                    manifest.state,
                )
            };
            if state == DownloadState::Canceled
                || token.is_cancelled() && state == DownloadState::Canceled
            {
                return Ok(RunOutcome::Canceled);
            }
            if token.is_cancelled() {
                return Ok(RunOutcome::Paused);
            }

            let start_offset = {
                let manifest = managed.manifest.lock().expect("manifest poisoned");
                contiguous_prefix_end(&manifest)
            };

            let response = request_with_retry(
                || {
                    let client = client.clone();
                    let url = url.clone();
                    let validator = validator.clone();
                    async move {
                        let mut builder = client.get(url);
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
                    let total_bytes = managed
                        .manifest
                        .lock()
                        .expect("manifest poisoned")
                        .total_bytes;
                    reset_download_file(&file, total_bytes)?;
                    self.reset_progress(&managed, false);
                }
                0
            };

            {
                let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                snapshot.state = DownloadState::Downloading;
                snapshot.updated_at_ms = now_ms();
                let mut manifest = managed.manifest.lock().expect("manifest poisoned");
                manifest.state = DownloadState::Downloading;
                manifest.updated_at_ms = now_ms();
            }

            loop {
                let next_chunk = tokio::select! {
                    _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                    chunk = stream.next() => chunk,
                };
                let Some(chunk) = next_chunk else {
                    break;
                };
                let chunk = chunk?;
                write_all_at(&file, &chunk, absolute_offset)?;
                absolute_offset += chunk.len() as u64;
                self.record_progress(&managed, None, chunk.len() as u64);
                if last_persist.elapsed() >= PERSIST_INTERVAL {
                    self.persist(managed.clone()).await?;
                    last_persist = Instant::now();
                }
            }

            self.persist(managed.clone()).await?;
            return Ok(RunOutcome::Finished);
        }
    }

    async fn download_segmented(
        &self,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<RunOutcome> {
        let (file_path, total_size, segments) = {
            let manifest = managed.manifest.lock().expect("manifest poisoned");
            (
                PathBuf::from(manifest.temp_path.clone()),
                manifest.total_bytes,
                manifest.segments.clone(),
            )
        };
        let file = Arc::new(open_download_file(&file_path, total_size)?);

        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.state = DownloadState::Downloading;
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.state = DownloadState::Downloading;
            manifest.updated_at_ms = now_ms();
        }

        let mut joins = Vec::new();
        for segment in segments.into_iter().filter(|segment| !segment.completed) {
            let managed = managed.clone();
            let client = client.clone();
            let token = token.clone();
            let file = file.clone();
            joins.push(tokio::spawn(async move {
                download_segment(managed, client, token, file, segment, max_retries).await
            }));
        }

        let mut restart_single = false;
        for join in joins {
            let outcome = join
                .await
                .map_err(|error| DownloadError::InvalidResponse(error.to_string()))??;
            match outcome {
                SegmentOutcome::Finished => {}
                SegmentOutcome::RestartSingle => {
                    restart_single = true;
                }
                SegmentOutcome::Paused => return Ok(RunOutcome::Paused),
                SegmentOutcome::Canceled => return Ok(RunOutcome::Canceled),
            }
        }

        if restart_single {
            self.reset_progress(&managed, true);
            return self
                .download_single(managed, client, token, max_retries)
                .await;
        }

        Ok(RunOutcome::Finished)
    }

    async fn finalize_download(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.state = DownloadState::Verifying;
            snapshot.updated_at_ms = now_ms();
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.state = DownloadState::Verifying;
            manifest.updated_at_ms = now_ms();
        }
        self.persist(managed.clone()).await?;

        let (temp_path, destination_path, checksum_mode) = {
            let manifest = managed.manifest.lock().expect("manifest poisoned");
            (
                PathBuf::from(manifest.temp_path.clone()),
                PathBuf::from(manifest.destination_path.clone()),
                manifest.checksum_mode.clone(),
            )
        };

        let checksum = if matches!(checksum_mode, ChecksumMode::Blake3) {
            Some(calculate_blake3(temp_path.clone()).await?)
        } else {
            None
        };

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)?;
        }
        finalize_temp_file(&temp_path, &destination_path)?;

        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.state = DownloadState::Completed;
            snapshot.downloaded_bytes = snapshot.total_bytes.unwrap_or(snapshot.downloaded_bytes);
            snapshot.checksum = checksum.clone();
            snapshot.destination_path = destination_path.to_string_lossy().to_string();
            snapshot.updated_at_ms = now_ms();
            snapshot.error = None;
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.state = DownloadState::Completed;
            manifest.downloaded_bytes = manifest.total_bytes.unwrap_or(manifest.downloaded_bytes);
            manifest.checksum = checksum;
            manifest.destination_path = destination_path.to_string_lossy().to_string();
            manifest.updated_at_ms = now_ms();
            manifest.error = None;
            for segment in &mut manifest.segments {
                segment.completed = true;
                segment.downloaded = segment.end.saturating_sub(segment.start) + 1;
            }
        }
        self.persist(managed).await?;
        Ok(())
    }

    async fn persist(&self, managed: Arc<ManagedDownload>) -> Result<()> {
        let _guard = self.persist_lock.lock().await;
        let manifest = managed.manifest.lock().expect("manifest poisoned").clone();
        let manifest_path = PathBuf::from(&manifest.manifest_path);
        if let Some(parent) = manifest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let temp_path = manifest_path.with_extension("json.tmp");
        tokio::fs::write(&temp_path, serde_json::to_vec_pretty(&manifest)?).await?;
        tokio::fs::rename(&temp_path, &manifest_path).await?;
        Ok(())
    }

    fn build_snapshot(&self, managed: Arc<ManagedDownload>) -> DownloadSnapshot {
        let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned").clone();
        let elapsed = (snapshot
            .updated_at_ms
            .saturating_sub(snapshot.created_at_ms))
        .max(1) as f64
            / 1000.0;
        let speed = if snapshot.downloaded_bytes == 0 {
            None
        } else {
            Some(snapshot.downloaded_bytes as f64 / elapsed)
        };
        let eta = match (snapshot.total_bytes, speed) {
            (Some(total), Some(speed)) if speed > 0.0 && total >= snapshot.downloaded_bytes => {
                Some(((total - snapshot.downloaded_bytes) as f64 / speed).ceil() as u64)
            }
            _ => None,
        };
        snapshot.speed_bytes_per_second = speed;
        snapshot.eta_seconds = eta;
        snapshot
    }

    fn record_progress(
        &self,
        managed: &Arc<ManagedDownload>,
        segment_index: Option<usize>,
        bytes: u64,
    ) {
        let now = now_ms();
        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.downloaded_bytes = snapshot.downloaded_bytes.saturating_add(bytes);
            snapshot.updated_at_ms = now;
            snapshot.error = None;
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.downloaded_bytes = manifest.downloaded_bytes.saturating_add(bytes);
            manifest.updated_at_ms = now;
            if let Some(index) = segment_index {
                if let Some(segment) = manifest
                    .segments
                    .iter_mut()
                    .find(|segment| segment.index == index)
                {
                    segment.downloaded = segment.downloaded.saturating_add(bytes);
                    if segment.downloaded >= segment.end.saturating_sub(segment.start) + 1 {
                        segment.completed = true;
                    }
                }
            }
        }
    }

    fn reset_progress(&self, managed: &Arc<ManagedDownload>, force_single_stream: bool) {
        let now = now_ms();
        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.downloaded_bytes = 0;
            snapshot.updated_at_ms = now;
            if force_single_stream {
                snapshot.connection_count = 1;
                snapshot.supports_ranges = false;
            }
        }
        {
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.downloaded_bytes = 0;
            manifest.updated_at_ms = now;
            for segment in &mut manifest.segments {
                segment.downloaded = 0;
                segment.completed = false;
            }
            if force_single_stream {
                manifest.connection_count = 1;
                manifest.supports_ranges = false;
                manifest.segments = vec![];
            }
        }
    }

    fn cleanup_files(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.manifest.lock().expect("manifest poisoned").clone();
        let temp_path = PathBuf::from(manifest.temp_path);
        let manifest_path = PathBuf::from(manifest.manifest_path);
        if temp_path.exists() {
            let _ = fs::remove_file(temp_path);
        }
        if manifest_path.exists() {
            let _ = fs::remove_file(manifest_path);
        }
        Ok(())
    }

    fn prepare_fresh_temp_file(&self, managed: &Arc<ManagedDownload>) -> Result<()> {
        let manifest = managed.manifest.lock().expect("manifest poisoned").clone();
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
        for _ in 0..200 {
            if managed.runtime.lock().expect("runtime poisoned").is_none() {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
    }

    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            client: self.client.clone(),
            state_dir: self.state_dir.clone(),
            settings_path: self.settings_path.clone(),
            proxy_settings: self.proxy_settings.clone(),
            downloads: self.downloads.clone(),
            persist_lock: self.persist_lock.clone(),
        })
    }
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

fn build_http_client(settings: &ProxySettings) -> Result<Client> {
    let mut builder = Client::builder()
        .redirect(Policy::limited(10))
        .tcp_nodelay(true)
        .read_timeout(Duration::from_secs(15))
        .user_agent("downloader/0.1");

    match settings.mode {
        ProxyMode::Disabled => {
            builder = builder.no_proxy();
        }
        ProxyMode::System => {}
        ProxyMode::Manual => {
            let proxy = Proxy::all(&settings.manual_url)
                .map_err(|error| DownloadError::InvalidProxy(error.to_string()))?;
            builder = builder.proxy(proxy);
        }
    }

    builder.build().map_err(DownloadError::from)
}

fn load_proxy_settings(settings_path: &Path) -> Result<ProxySettings> {
    let content = match fs::read_to_string(settings_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProxySettings::default())
        }
        Err(error) => return Err(error.into()),
    };

    let parsed = serde_json::from_str::<ProxySettings>(&content)?;
    normalize_proxy_settings(parsed)
}

async fn persist_proxy_settings(settings_path: &Path, settings: &ProxySettings) -> Result<()> {
    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let temp_path = settings_path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, serde_json::to_vec_pretty(settings)?).await?;
    tokio::fs::rename(&temp_path, settings_path).await?;
    Ok(())
}

async fn download_segment(
    managed: Arc<ManagedDownload>,
    client: Client,
    token: CancellationToken,
    file: Arc<std::fs::File>,
    segment: SegmentManifest,
    max_retries: u32,
) -> Result<SegmentOutcome> {
    let mut current = segment.start + segment.downloaded;
    let end = segment.end;
    if current > end {
        return Ok(SegmentOutcome::Finished);
    }

    let mut last_persist = Instant::now();
    let mut attempts = 0;

    while current <= end {
        if token.is_cancelled() {
            return Ok(
                match managed.snapshot.lock().expect("snapshot poisoned").state {
                    DownloadState::Canceled => SegmentOutcome::Canceled,
                    _ => SegmentOutcome::Paused,
                },
            );
        }

        let (url, validator) = {
            let manifest = managed.manifest.lock().expect("manifest poisoned");
            (manifest.final_url.clone(), if_range_header(&manifest))
        };

        let response = {
            loop {
                if token.is_cancelled() {
                    return Ok(cancellation_segment_outcome(&managed));
                }
                let send_result = tokio::select! {
                    _ = token.cancelled() => return Ok(cancellation_segment_outcome(&managed)),
                    response = build_segment_request(&client, &url, current, end, validator.clone()).send() => response,
                };
                match send_result {
                    Ok(response) => break response,
                    Err(error) => {
                        if attempts >= max_retries {
                            return Err(error.into());
                        }
                        attempts += 1;
                        {
                            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                            snapshot.state = DownloadState::Retrying;
                            snapshot.updated_at_ms = now_ms();
                            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
                            manifest.state = DownloadState::Retrying;
                            manifest.updated_at_ms = now_ms();
                        }
                        tokio::select! {
                            _ = token.cancelled() => return Ok(cancellation_segment_outcome(&managed)),
                            _ = sleep(backoff_delay(attempts)) => {}
                        }
                    }
                }
            }
        };

        if response.status() != StatusCode::PARTIAL_CONTENT {
            return Ok(SegmentOutcome::RestartSingle);
        }
        validate_segment_response(&response, current, end)?;

        {
            let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
            snapshot.state = DownloadState::Downloading;
            snapshot.updated_at_ms = now_ms();
            let mut manifest = managed.manifest.lock().expect("manifest poisoned");
            manifest.state = DownloadState::Downloading;
            manifest.updated_at_ms = now_ms();
        }

        let mut stream = response.bytes_stream();
        loop {
            let next_chunk = tokio::select! {
                _ = token.cancelled() => return Ok(cancellation_segment_outcome(&managed)),
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = next_chunk else {
                break;
            };
            let chunk = chunk?;
            let remaining = end.saturating_sub(current) + 1;
            if chunk.len() as u64 > remaining {
                return Err(DownloadError::InvalidResponse(String::from(
                    "segment body exceeded requested range",
                )));
            }
            write_all_at(&file, &chunk, current)?;
            current += chunk.len() as u64;
            {
                let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                snapshot.downloaded_bytes =
                    snapshot.downloaded_bytes.saturating_add(chunk.len() as u64);
                snapshot.updated_at_ms = now_ms();
                let mut manifest = managed.manifest.lock().expect("manifest poisoned");
                manifest.downloaded_bytes =
                    manifest.downloaded_bytes.saturating_add(chunk.len() as u64);
                manifest.updated_at_ms = now_ms();
                if let Some(target_segment) = manifest
                    .segments
                    .iter_mut()
                    .find(|candidate| candidate.index == segment.index)
                {
                    target_segment.downloaded =
                        target_segment.downloaded.saturating_add(chunk.len() as u64);
                    target_segment.completed = target_segment.downloaded
                        >= target_segment.end.saturating_sub(target_segment.start) + 1;
                }
            }
            if last_persist.elapsed() >= PERSIST_INTERVAL {
                persist_manifest_snapshot(&managed).await?;
                last_persist = Instant::now();
            }
        }
    }

    {
        let mut manifest = managed.manifest.lock().expect("manifest poisoned");
        if let Some(target_segment) = manifest
            .segments
            .iter_mut()
            .find(|candidate| candidate.index == segment.index)
        {
            target_segment.completed = true;
            target_segment.downloaded = target_segment.end.saturating_sub(target_segment.start) + 1;
        }
        manifest.updated_at_ms = now_ms();
    }
    Ok(SegmentOutcome::Finished)
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
                    {
                        let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                        snapshot.state = DownloadState::Retrying;
                        snapshot.error = Some(format!("http status {status}"));
                        snapshot.updated_at_ms = now_ms();
                        let mut manifest = managed.manifest.lock().expect("manifest poisoned");
                        manifest.state = DownloadState::Retrying;
                        manifest.error = Some(format!("http status {status}"));
                        manifest.updated_at_ms = now_ms();
                    }
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
                {
                    let mut snapshot = managed.snapshot.lock().expect("snapshot poisoned");
                    snapshot.state = DownloadState::Retrying;
                    snapshot.error = Some(error.to_string());
                    snapshot.updated_at_ms = now_ms();
                    let mut manifest = managed.manifest.lock().expect("manifest poisoned");
                    manifest.state = DownloadState::Retrying;
                    manifest.error = Some(error.to_string());
                    manifest.updated_at_ms = now_ms();
                }
                tokio::select! {
                    _ = token.cancelled() => return Err(DownloadError::Interrupted),
                    _ = sleep(backoff_delay(attempt)) => {}
                }
            }
        }
    }
}

async fn persist_manifest_snapshot(managed: &Arc<ManagedDownload>) -> Result<()> {
    let _guard = managed.persist_lock.lock().await;
    let manifest = managed.manifest.lock().expect("manifest poisoned").clone();
    let manifest_path = PathBuf::from(&manifest.manifest_path);
    if let Some(parent) = manifest_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temp_path = manifest_path.with_extension("json.tmp");
    tokio::fs::write(&temp_path, serde_json::to_vec_pretty(&manifest)?).await?;
    tokio::fs::rename(&temp_path, &manifest_path).await?;
    Ok(())
}

fn cancellation_outcome(managed: &Arc<ManagedDownload>) -> RunOutcome {
    match managed.snapshot.lock().expect("snapshot poisoned").state {
        DownloadState::Canceled => RunOutcome::Canceled,
        _ => RunOutcome::Paused,
    }
}

fn cancellation_segment_outcome(managed: &Arc<ManagedDownload>) -> SegmentOutcome {
    match managed.snapshot.lock().expect("snapshot poisoned").state {
        DownloadState::Canceled => SegmentOutcome::Canceled,
        _ => SegmentOutcome::Paused,
    }
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis((250_u64).saturating_mul(2_u64.saturating_pow(attempt.min(4))))
}

async fn calculate_blake3(path: PathBuf) -> Result<String> {
    tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;

        let mut file = std::fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = vec![0_u8; 1024 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hasher.finalize().to_hex().to_string())
    })
    .await
    .map_err(|error| DownloadError::InvalidResponse(error.to_string()))?
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode},
        response::IntoResponse,
        routing::get,
        Router,
    };
    use tempfile::tempdir;

    use super::*;

    #[derive(Clone)]
    struct TestState {
        bytes: Arc<Vec<u8>>,
        etag: String,
    }

    #[tokio::test]
    async fn resumes_and_derives_filename() {
        let payload = Arc::new(vec![42_u8; 2 * 1024 * 1024]);
        let state = TestState {
            bytes: payload.clone(),
            etag: String::from("\"test-etag\""),
        };

        let app = Router::new()
            .route("/file.bin", get(file_get).head(file_head))
            .route("/slow.bin", get(slow_file_get).head(file_head))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let temp = tempdir().unwrap();
        let manager = DownloadManager::new(temp.path().join("state")).unwrap();
        let request = StartDownloadRequest {
            url: format!("http://{address}/file.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            max_connections: Some(4),
            max_retries: Some(2),
            checksum: Some(ChecksumMode::Blake3),
        };

        let id = manager.start(request).await.unwrap();

        loop {
            let snapshot = manager.status(&id).await.unwrap();
            if matches!(snapshot.state, DownloadState::Completed) {
                assert!(snapshot.file_name.contains("server-name.bin"));
                assert_eq!(snapshot.total_bytes, Some(payload.len() as u64));
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }

        let completed = manager.status(&id).await.unwrap();
        assert!(PathBuf::from(completed.destination_path).exists());
        assert!(completed.checksum.is_some());
    }

    #[tokio::test]
    async fn pauses_and_resumes_download() {
        let payload = Arc::new(vec![7_u8; 1024 * 1024]);
        let state = TestState {
            bytes: payload.clone(),
            etag: String::from("\"pause-etag\""),
        };

        let app = Router::new()
            .route("/slow.bin", get(slow_file_get).head(file_head))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let temp = tempdir().unwrap();
        let manager = DownloadManager::new(temp.path().join("state")).unwrap();
        let request = StartDownloadRequest {
            url: format!("http://{address}/slow.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("paused.bin")),
            max_connections: Some(1),
            max_retries: Some(2),
            checksum: Some(ChecksumMode::Blake3),
        };

        let id = manager.start(request).await.unwrap();
        sleep(Duration::from_millis(20)).await;
        let paused = manager.pause(&id).await.unwrap();
        assert_eq!(paused.state, DownloadState::Paused);

        let resumed = manager.resume(&id).await.unwrap();
        assert!(matches!(
            resumed.state,
            DownloadState::Queued | DownloadState::Downloading
        ));

        loop {
            let snapshot = manager.status(&id).await.unwrap();
            if matches!(snapshot.state, DownloadState::Completed) {
                assert_eq!(snapshot.total_bytes, Some(payload.len() as u64));
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn rejects_http_error_status() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let temp = tempdir().unwrap();
        let manager = DownloadManager::new(temp.path().join("state")).unwrap();
        let request = StartDownloadRequest {
            url: format!("http://{address}/missing.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: None,
            max_connections: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
        };

        let error = manager.start(request).await.unwrap_err();
        assert!(matches!(error, DownloadError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn cancel_removes_manifest_and_blocks_resume() {
        let payload = Arc::new(vec![3_u8; 1024 * 1024]);
        let state = TestState {
            bytes: payload,
            etag: String::from("\"cancel-etag\""),
        };

        let app = Router::new()
            .route("/slow.bin", get(slow_file_get).head(file_head))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let temp = tempdir().unwrap();
        let manager = DownloadManager::new(temp.path().join("state")).unwrap();
        let request = StartDownloadRequest {
            url: format!("http://{address}/slow.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("cancel.bin")),
            max_connections: Some(1),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
        };

        let id = manager.start(request).await.unwrap();
        sleep(Duration::from_millis(20)).await;
        let canceled = manager.cancel(&id).await.unwrap();
        assert_eq!(canceled.state, DownloadState::Canceled);
        assert!(manager.status(&id).await.is_err());
        assert!(matches!(
            manager.resume(&id).await,
            Err(DownloadError::NotFound)
        ));
    }

    #[tokio::test]
    async fn rejects_invalid_segment_content_range() {
        let payload = Arc::new(vec![9_u8; 10 * 1024 * 1024]);
        let state = TestState {
            bytes: payload,
            etag: String::from("\"bad-range\""),
        };

        let app = Router::new()
            .route("/bad.bin", get(bad_segment_file_get).head(file_head))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let temp = tempdir().unwrap();
        let manager = DownloadManager::new(temp.path().join("state")).unwrap();
        let request = StartDownloadRequest {
            url: format!("http://{address}/bad.bin"),
            destination_dir: temp.path().join("out").to_string_lossy().to_string(),
            file_name: Some(String::from("bad.bin")),
            max_connections: Some(2),
            max_retries: Some(1),
            checksum: Some(ChecksumMode::Blake3),
        };

        let id = manager.start(request).await.unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let snapshot = manager.status(&id).await.unwrap();
                if matches!(snapshot.state, DownloadState::Failed) {
                    return snapshot;
                }
                sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("download should fail quickly on invalid content-range");

        assert!(result.error.unwrap_or_default().contains("content-range"));
    }

    async fn file_head(State(state): State<TestState>) -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
        );
        headers.insert(header::ETAG, HeaderValue::from_str(&state.etag).unwrap());
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename*=UTF-8''server-name.bin"),
        );
        (StatusCode::OK, headers)
    }

    async fn file_get(State(state): State<TestState>, headers: HeaderMap) -> impl IntoResponse {
        let mut response_headers = HeaderMap::new();
        response_headers.insert(header::ETAG, HeaderValue::from_str(&state.etag).unwrap());
        response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        response_headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename*=UTF-8''server-name.bin"),
        );

        let requested = headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok());
        if let Some(requested) = requested {
            let range = requested.strip_prefix("bytes=").unwrap();
            let mut pieces = range.split('-');
            let start = pieces.next().unwrap().parse::<usize>().unwrap();
            let end = pieces
                .next()
                .and_then(|value| {
                    if value.is_empty() {
                        None
                    } else {
                        value.parse::<usize>().ok()
                    }
                })
                .unwrap_or(state.bytes.len() - 1);
            let body = state.bytes[start..=end].to_vec();
            response_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{}", state.bytes.len()))
                    .unwrap(),
            );
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&body.len().to_string()).unwrap(),
            );
            return (StatusCode::PARTIAL_CONTENT, response_headers, body).into_response();
        }

        response_headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&state.bytes.len().to_string()).unwrap(),
        );
        (
            StatusCode::OK,
            response_headers,
            state.bytes.as_ref().clone(),
        )
            .into_response()
    }

    async fn slow_file_get(
        State(state): State<TestState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        sleep(Duration::from_millis(150)).await;
        file_get(State(state), headers).await
    }

    async fn bad_segment_file_get(
        State(state): State<TestState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        let mut response = file_get(State(state), headers).await.into_response();
        response.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str("bytes 1-10/10485760").unwrap(),
        );
        response
    }
}
