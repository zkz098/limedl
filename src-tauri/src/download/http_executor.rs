//! HTTP download execution — extracted from manager.rs to reduce the god object.
//!
//! Contains the HTTP-specific download flow: probing, single-stream and chunked
//! parallel downloads, chunk worker, and finalization with checksum verification.

use super::*;
use crate::download::calculate_checksum;
use crate::download::persistence::persist_manifest_snapshot;
use crate::download::retry::request_with_retry;

impl super::DownloadManager {
    async fn probe(&self, url: &str, user_agent: &str) -> Result<RemoteMetadata> {
        let (client, _) = self.resolve_client(url).await;
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

    pub(super) async fn run_download(
        &self,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<()> {
        let current_manifest = { managed.lock_core().manifest.clone() };
        let metadata = self
            .probe(&current_manifest.url, &current_manifest.user_agent)
            .await?;

        // Check available disk space before starting the download
        // Account for already-downloaded bytes to avoid false rejections on resume.
        if let Some(total_bytes) = metadata.total_bytes {
            let already_downloaded = current_manifest.downloaded_bytes;
            let needed = total_bytes.saturating_sub(already_downloaded);
            check_disk_space(Path::new(&current_manifest.destination_dir), needed)?;
        }

        let settings = self.settings.read().await.clone();
        let chunk_size =
            resolve_chunk_size(settings.scheduler.chunk_size_strategy, metadata.total_bytes);
        let supports_parallel =
            supports_parallelism(metadata.total_bytes, metadata.supports_ranges, chunk_size);
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
            selected_file_indices: None,
            start_paused: false,
        };
        let (thread_mode, requested_thread_count, desired_thread_count, adaptive_profile) =
            resolve_thread_settings(&settings, &request, supports_parallel);
        let mut reset_progress = false;
        let mut force_single_stream_restart = false;
        let mut refresh_aimd = false;
        {
            let mut core = managed.lock_core();
            let manifest = &mut core.manifest;
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
            if validators_changed(manifest, &metadata)
                || manifest.total_bytes != metadata.total_bytes
                || manifest.supports_ranges != supports_parallel
                || (supports_parallel && manifest.chunks.is_empty())
            {
                manifest.downloaded_bytes = 0;
                manifest.chunks = plan_chunks(metadata.total_bytes, supports_parallel, chunk_size);
                manifest.chunk_size = chunk_size;
                manifest.checksum = None;
                manifest.supports_ranges = supports_parallel;
                reset_progress = true;
            } else if !supports_parallel && has_partial_chunk_progress(manifest) {
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
            sync_snapshot_with_manifest(&mut core);
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
                match self
                    .finalize_download(managed.clone(), token.clone())
                    .await?
                {
                    RunOutcome::Finished => {
                        self.emit_single_summary(&managed);
                        if let Err(error) = self.learn_from_download(managed.clone()).await {
                            log_background_error("learn from completed download", &error);
                        }
                    }
                    RunOutcome::Canceled => {
                        return Ok(());
                    }
                    RunOutcome::Paused => {
                        return Ok(());
                    }
                }
            }
            RunOutcome::Paused => {
                {
                    let mut core = managed.lock_core();
                    core.snapshot.state = DownloadState::Paused;
                    core.snapshot.connection_count = 0;
                    core.snapshot.allocated_thread_count = Some(0);
                    core.snapshot.updated_at_ms = now_ms();
                    core.manifest.state = DownloadState::Paused;
                    core.manifest.connection_count = 0;
                    core.manifest.allocated_thread_count = Some(0);
                    core.manifest.updated_at_ms = now_ms();
                }
                self.emit_single_summary(&managed);
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
        let file_path = PathBuf::from(managed.lock_core().manifest.temp_path.clone());
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = open_download_file(&file_path, managed.lock_core().manifest.total_bytes)?;

        let mut last_persist = Instant::now();

        loop {
            match self.wait_until_active(&managed, &token).await {
                WaitState::Running => {}
                WaitState::Paused => return Ok(RunOutcome::Paused),
                WaitState::Canceled => return Ok(RunOutcome::Canceled),
            }

            let (url, user_agent, validator, state) = {
                let core = managed.lock_core();
                (
                    core.manifest.final_url.clone(),
                    core.manifest.user_agent.clone(),
                    if_range_header(&core.manifest),
                    core.manifest.state,
                )
            };
            if state == DownloadState::Canceled {
                return Ok(RunOutcome::Canceled);
            }
            if token.is_cancelled() {
                return Ok(cancellation_outcome(&managed));
            }

            let start_offset = {
                let core = managed.lock_core();
                contiguous_prefix_end(&core.manifest)
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
                    reset_download_file(&file, managed.lock_core().manifest.total_bytes)?;
                    self.reset_progress(&managed, true);
                }
                0
            };

            {
                let mut core = managed.lock_core();
                core.snapshot.state = DownloadState::Downloading;
                core.snapshot.connection_count = 1;
                core.snapshot.updated_at_ms = now_ms();
                core.manifest.state = DownloadState::Downloading;
                core.manifest.connection_count = 1;
                core.manifest.updated_at_ms = now_ms();
            }

            while let Some(chunk) = tokio::select! {
                _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                chunk = stream.next() => chunk,
            } {
                let chunk = chunk?;
                self.rate_limiter.consume(chunk.len()).await;
                write_all_at(&file, &chunk, absolute_offset)?;
                absolute_offset += chunk.len() as u64;
                self.record_progress(&managed, None, chunk.len() as u64);
                if last_persist.elapsed() >= PERSIST_INTERVAL {
                    persist_manifest_snapshot(&self.db, &managed).await?;
                    last_persist = Instant::now();
                }
            }

            let finished = {
                let core = managed.lock_core();
                match core.manifest.total_bytes {
                    Some(total) => core.manifest.downloaded_bytes >= total,
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
            let core = managed.lock_core();
            (
                PathBuf::from(core.manifest.temp_path.clone()),
                core.manifest.total_bytes,
            )
        };
        let file = Arc::new(open_download_file(&file_path, total_size)?);
        let mut workers = JoinSet::new();
        let mut next_worker_id = 0usize;

        loop {
            if token.is_cancelled() {
                shutdown_chunk_workers(&managed, &mut workers).await;
                return Ok(cancellation_outcome(&managed));
            }

            if all_chunks_completed(&managed) {
                shutdown_chunk_workers(&managed, &mut workers).await;
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

            let mut target_workers = current_allocation(&managed);
            let chunk_count = {
                let core = managed.lock_core();
                core.manifest.chunks.len()
            };
            if chunk_count > 0 {
                target_workers = target_workers.min((chunk_count / 2).max(1));
            }
            while workers.len() < target_workers {
                let worker_id = next_worker_id;
                let chunk = {
                    let mut core = managed.lock_core();
                    claim_next_chunk(&mut core.manifest, worker_id)
                };
                let Some(chunk) = chunk else {
                    break;
                };

                {
                    let mut core = managed.lock_core();
                    core.snapshot.state = DownloadState::Downloading;
                    core.snapshot.connection_count = target_workers;
                    core.snapshot.updated_at_ms = now_ms();
                    core.manifest.state = DownloadState::Downloading;
                    core.manifest.connection_count = target_workers;
                    core.manifest.updated_at_ms = now_ms();
                }

                let db = self.db.clone();
                let rate_limiter = self.rate_limiter.clone();
                let managed = managed.clone();
                let client = client.clone();
                let token = token.clone();
                let file = file.clone();
                workers.spawn(async move {
                    download_chunk(
                        managed,
                        client,
                        token,
                        file,
                        chunk,
                        max_retries,
                        db,
                        rate_limiter,
                    )
                    .await
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
                _ = token.cancelled() => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    return Ok(cancellation_outcome(&managed));
                }
                joined = workers.join_next() => joined,
            };

            let Some(join_result) = join_result else {
                continue;
            };
            let worker_outcome = match join_result {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(error)) => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    return Err(error);
                }
                Err(error) => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    return Err(DownloadError::InvalidResponse(error.to_string()));
                }
            };

            match worker_outcome {
                ChunkWorkerOutcome::Finished => {}
                ChunkWorkerOutcome::RestartSingle => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    drop(file);
                    self.prepare_fresh_temp_file(&managed)?;
                    self.reset_progress(&managed, true);
                    return self
                        .download_single(managed, client, token, max_retries)
                        .await;
                }
                ChunkWorkerOutcome::Paused => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    return Ok(RunOutcome::Paused);
                }
                ChunkWorkerOutcome::Canceled => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    return Ok(RunOutcome::Canceled);
                }
            }
        }
    }

    async fn finalize_download(
        &self,
        managed: Arc<ManagedDownload>,
        token: CancellationToken,
    ) -> Result<RunOutcome> {
        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }
        {
            let mut core = managed.lock_core();
            if core.snapshot.state == DownloadState::Canceled || token.is_cancelled() {
                return Ok(RunOutcome::Canceled);
            }
            core.snapshot.state = DownloadState::Verifying;
            core.snapshot.connection_count = 0;
            core.snapshot.allocated_thread_count = Some(0);
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Verifying;
            core.manifest.connection_count = 0;
            core.manifest.allocated_thread_count = Some(0);
            core.manifest.updated_at_ms = now_ms();
        }
        self.persist(managed.clone()).await?;

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        let (temp_path, destination_path, checksum_mode) = {
            let core = managed.lock_core();
            (
                PathBuf::from(core.manifest.temp_path.clone()),
                PathBuf::from(core.manifest.destination_path.clone()),
                core.manifest.checksum_mode,
            )
        };

        let checksum = match checksum_mode {
            ChecksumMode::None => None,
            mode => Some(calculate_checksum(temp_path.clone(), mode).await?),
        };

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        {
            let mut core = managed.lock_core();
            if core.manifest.state == DownloadState::Canceled || token.is_cancelled() {
                return Ok(RunOutcome::Canceled);
            }
            if core.snapshot.state == DownloadState::Canceled || token.is_cancelled() {
                return Ok(RunOutcome::Canceled);
            }

            finalize_temp_file(&temp_path, &destination_path)?;

            core.snapshot.state = DownloadState::Completed;
            core.snapshot.downloaded_bytes = core
                .snapshot
                .total_bytes
                .unwrap_or(core.snapshot.downloaded_bytes);
            core.snapshot.checksum = checksum.clone();
            core.snapshot.destination_path = destination_path.to_string_lossy().to_string();
            core.snapshot.error = None;
            core.snapshot.updated_at_ms = now_ms();

            core.manifest.state = DownloadState::Completed;
            core.manifest.downloaded_bytes = core
                .manifest
                .total_bytes
                .unwrap_or(core.manifest.downloaded_bytes);
            core.manifest.checksum = checksum;
            core.manifest.destination_path = destination_path.to_string_lossy().to_string();
            core.manifest.error = None;
            core.manifest.updated_at_ms = now_ms();
            for chunk in &mut core.manifest.chunks {
                chunk.completed = true;
                chunk.downloaded = chunk.end.saturating_sub(chunk.start) + 1;
                chunk.claimed_by = None;
                chunk.dirty = true;
            }
        }
        self.persist(managed.clone()).await?;

        // Broadcast aria2.onDownloadComplete via RPC event channel
        let event_tx = self.event_tx.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("event_tx lock poisoned in finalize_download");
            poisoned.into_inner()
        });
        if let Some(ref tx) = *event_tx {
            let download_id = managed.lock_core().snapshot.id.clone();
            let gid = format!(
                "{:016x}",
                xxhash_rust::xxh3::xxh3_64(download_id.as_bytes())
            );
            let _ = tx.send(build_rpc_notification("aria2.onDownloadComplete", &gid));
        }

        Ok(RunOutcome::Finished)
    }
}

fn current_allocation(managed: &Arc<ManagedDownload>) -> usize {
    managed
        .lock_core()
        .manifest
        .allocated_thread_count
        .unwrap_or(0)
}

fn all_chunks_completed(managed: &Arc<ManagedDownload>) -> bool {
    managed
        .lock_core()
        .manifest
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
    chunk.dirty = true;
    Some(chunk.clone())
}

fn mark_chunk_released(managed: &Arc<ManagedDownload>, chunk_index: usize) {
    let mut core = managed.lock_core();
    if let Some(chunk) = core
        .manifest
        .chunks
        .iter_mut()
        .find(|chunk| chunk.index == chunk_index)
    {
        chunk.claimed_by = None;
        chunk.dirty = true;
    }
}

async fn shutdown_chunk_workers(
    managed: &Arc<ManagedDownload>,
    workers: &mut JoinSet<Result<ChunkWorkerOutcome>>,
) {
    workers.abort_all();
    while workers.join_next().await.is_some() {}
    release_all_chunk_claims(managed);
}

fn release_all_chunk_claims(managed: &Arc<ManagedDownload>) {
    let mut core = managed.lock_core();
    for chunk in &mut core.manifest.chunks {
        chunk.claimed_by = None;
        chunk.dirty = true;
    }
    core.manifest.connection_count = 0;
    core.manifest.updated_at_ms = now_ms();
}

fn finalize_was_canceled(managed: &Arc<ManagedDownload>, token: &CancellationToken) -> bool {
    if token.is_cancelled() {
        return true;
    }
    let core = managed.lock_core();
    core.snapshot.state == DownloadState::Canceled || core.manifest.state == DownloadState::Canceled
}

#[allow(clippy::too_many_arguments)]
async fn download_chunk(
    managed: Arc<ManagedDownload>,
    client: Client,
    token: CancellationToken,
    file: Arc<std::fs::File>,
    chunk: ChunkManifest,
    max_retries: u32,
    db: Arc<Database>,
    rate_limiter: Arc<RateLimiter>,
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
            return Ok(match managed.lock_core().snapshot.state {
                DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
                _ => ChunkWorkerOutcome::Paused,
            });
        }

        let (url, user_agent, validator) = {
            let core = managed.lock_core();
            (
                core.manifest.final_url.clone(),
                core.manifest.user_agent.clone(),
                if_range_header(&core.manifest),
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

        if response.status() == StatusCode::OK {
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
            rate_limiter.consume(bytes.len()).await;
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
        let mut core = managed.lock_core();
        if let Some(target) = core
            .manifest
            .chunks
            .iter_mut()
            .find(|candidate| candidate.index == chunk.index)
        {
            target.completed = true;
            target.downloaded = target.end.saturating_sub(target.start) + 1;
            target.claimed_by = None;
            target.dirty = true;
        }
        core.manifest.updated_at_ms = now_ms();
    }
    Ok(ChunkWorkerOutcome::Finished)
}
