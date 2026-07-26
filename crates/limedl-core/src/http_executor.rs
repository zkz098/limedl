//! HTTP download execution — extracted from manager.rs to reduce the god object.
//!
//! Contains the HTTP-specific download flow: probing, single-stream and chunked
//! parallel downloads, chunk worker, and finalization with checksum verification.
//!
//! `HttpExecutor` is an independent actor type.  All its methods receive a
//! `&DownloadManager` or `Arc<DownloadManager>` parameter to access shared
//! state, avoiding any ownership cycle with `DownloadManager` (which holds
//! `Arc<HttpExecutor>`).

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::StreamExt;
use reqwest::{Client, StatusCode, header};
use tokio::{task::JoinSet, time::sleep};
use tokio_util::sync::CancellationToken;

use crate::{
    aimd::AimdState,
    buffer_pool::DownloadBuffer,
    calculate_checksum,
    database::Database,
    error::{DownloadError, Result, io_error_with_path},
    event_bus::DownloadEvent,
    file_ops::{
        check_disk_space, finalize_temp_file, open_download_file,
        reset_download_file, write_all_at,
    },
    http::{
        build_segment_request, extract_total_bytes, header_string, if_range_header, infer_file_name,
        supports_ranges, validate_probe_response, validate_segment_response,
    },
    manager::{
        self, cancellation_chunk_outcome, cancellation_outcome,
        ChunkWorkerOutcome, DownloadManager, ManagedDownload,
        PERSIST_INTERVAL, RunOutcome, record_progress_on_managed, supports_parallelism,
    },
    manifest::{
        ChunkManifest, RemoteMetadata, contiguous_prefix_end, has_partial_chunk_progress, plan_chunks, resolve_chunk_size,
        validators_changed,
    },
    now_ms,
    persistence::persist_manifest_snapshot,
    rate_limiter::RateLimiter,
    retry::request_with_retry,
    types::{
        ChecksumMode, DiskType, DownloadState, StartDownloadRequest,
        TaskKind,
    },
};

/// Zero-sized actor type for HTTP download execution.
///
/// All methods receive `&DownloadManager` or `Arc<DownloadManager>` to access
/// shared state.  `DownloadManager` holds `Arc<HttpExecutor>` for delegation.
pub struct HttpExecutor;

/// Tail Sprint: stall window for detecting a slow last-chunk connection.
const TAIL_SPRINT_STALL_WINDOW_SECS: u64 = 8;
/// Tail Sprint: minimum remaining bytes to qualify for chunk splitting (1 MiB).
const TAIL_SPRINT_MIN_SPLIT_SIZE: u64 = 1024 * 1024;

impl HttpExecutor {
    /// Probe a remote URL to obtain file metadata (final URL, file name,
    /// content length, ETag, Last-Modified, range support).
    pub(crate) async fn probe(
        &self,
        dm: &DownloadManager,
        url: &str,
        user_agent: &str,
    ) -> Result<RemoteMetadata> {
        let (client, _) = dm.resolve_client(url).await;
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

    /// Main download run loop.  Decides between single-stream and chunked
    /// (parallel) download based on server capabilities.
    pub(crate) async fn run_download(
        &self,
        dm: Arc<DownloadManager>,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<()> {
        let current_manifest = { managed.lock_core().manifest.clone() };
        let metadata = self
            .probe(&dm, &current_manifest.final_url, &current_manifest.user_agent)
            .await?;

        // Check available disk space before starting the download
        if let Some(total_bytes) = metadata.total_bytes {
            let already_downloaded = current_manifest.downloaded_bytes;
            let needed = total_bytes.saturating_sub(already_downloaded);
            check_disk_space(Path::new(&current_manifest.destination_dir), needed)?;
        }

        let settings = dm.settings.read().await.clone();
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
            expected_checksum: None,
            selected_file_indices: None,
            start_paused: false,
            mirror_urls: None,
            priority: None,
        };
        let (thread_mode, requested_thread_count, desired_thread_count, adaptive_profile) =
            manager::resolve_thread_settings(&settings, &request, supports_parallel);
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
                        manager::unique_destination_path(&destination_dir, &safe_name)
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
            manifest.thread_note = manager::thread_note(supports_parallel, thread_mode, adaptive_profile);
            manifest.updated_at_ms = now_ms();
            manifest.error = None;
            if !supports_parallel {
                manifest.thread_note = Some(String::from("单线程（服务器不支持分段）"));
                manifest.desired_thread_count = Some(1);
            }
            core.sync_snapshot_from_manifest();
        }

        // Emit a frontend-visible warning for files exceeding 4 GB (FAT32 limitation).
        if let Some(total) = metadata.total_bytes
            && total > 4_294_967_295
        {
            let msg = String::from(
                "Download exceeds 4 GB. FAT32 and some older filesystems cannot store files larger than 4 GB. \
                 Ensure the destination drive is formatted as NTFS, exFAT, ext4, or APFS.",
            );
            tracing::warn!("{msg}");
            dm.event_bus.publish(DownloadEvent::Warning {
                id: managed.lock_core().manifest.id.clone(),
                message: msg,
            });
        }

        if refresh_aimd {
            let mut aimd = managed.lock_aimd();
            *aimd = AimdState::initial(adaptive_profile, desired_thread_count);
        }
        dm.scheduler.rebalance_allocations(&dm).await?;
        dm.controls.rebalance_notify.notify_waiters();
        if reset_progress {
            dm.task_lifecycle.prepare_fresh_temp_file(&dm, &managed)?;
            if force_single_stream_restart {
                dm.task_lifecycle.reset_progress(&dm, &managed, true);
            }
        }

        let outcome = if supports_parallel {
            self.download_chunked(dm.clone(), managed.clone(), client.clone(), token.clone(), max_retries)
                .await?
        } else {
            self.download_single(dm.clone(), managed.clone(), client.clone(), token.clone(), max_retries)
                .await?
        };

        match outcome {
            RunOutcome::Finished => {
                match self.finalize_download(dm.clone(), managed.clone(), token.clone()).await? {
                    RunOutcome::Finished => {
                        dm.task_lifecycle.emit_single_summary(&dm, &managed);
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
                dm.task_lifecycle.emit_single_summary(&dm, &managed);
            }
            RunOutcome::Canceled => {
                dm.task_lifecycle.cleanup_files(&dm, &managed)?;
            }
        }

        Ok(())
    }

    async fn download_single(
        &self,
        dm: Arc<DownloadManager>,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<RunOutcome> {
        let (temp_path, total_bytes, destination_dir) = {
            let core = managed.lock_core();
            (
                core.manifest.temp_path.clone(),
                core.manifest.total_bytes,
                core.manifest.destination_dir.clone(),
            )
        };
        let file_path = PathBuf::from(temp_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
        }
        let file = Arc::new(open_download_file(&file_path, total_bytes)?);

        // HDD/SSD optimization: set up buffered writing
        let settings = dm.settings().await?;
        let hdd_buffering = settings.io_baseline.hdd_buffer_enabled;
        let ssd_write_combine_mb = settings.io_baseline.ssd_write_combine_mb;
        drop(settings);
        let disk_type = dm.resolve_disk_type(Path::new(&destination_dir)).await;
        let write_buffer: Option<Arc<DownloadBuffer>> = if disk_type == DiskType::Hdd && hdd_buffering {
            let slot = dm.buffer_pool.acquire_slot().await;
            Some(Arc::new(DownloadBuffer::new_with_worker(
                dm.buffer_pool.clone(),
                slot,
                file.clone(),
                dm.io_worker.clone(),
            )))
        } else {
            let chunk_size = {
                let core = managed.lock_core();
                core.manifest.chunk_size
            };
            let ssd_limit_bytes = if ssd_write_combine_mb == 0 {
                chunk_size  // auto: use the download's chunk size
            } else {
                ssd_write_combine_mb * 1024 * 1024
            };
            // half_size = ssd_limit_bytes / 2, minimum 64 KiB, capped at 8 MiB.
            // Note: for large auto-sized buffers (chunk_size up to 128 MiB),
            // this means at most 8 MiB per download — the ping-pong double-buffer
            // caps peak untracked memory at 16 MiB per download regardless of N.
            let ssd_half_size = (ssd_limit_bytes / 2).clamp(64 * 1024, 8 * 1024 * 1024);
            Some(Arc::new(DownloadBuffer::new_local_pingpong_with_worker(
                ssd_half_size,
                file.clone(),
                dm.io_worker.clone(),
            )))
        }; // always Some — SSD uses ping-pong buffer for write combining

        // Set disk_type on snapshot for frontend badge display
        {
            let mut core = managed.lock_core();
            core.snapshot.disk_type = Some(disk_type);
        }

        let mut last_persist = Instant::now();
        let mut last_disk_check = Instant::now();
        // ── progress throttling ──
        let mut last_progress_emit = Instant::now();
        // ── rate limiter batch consume ──
        let mut bytes_since_consume: usize = 0;
        let mut chunks_since_consume: usize = 0;

        loop {
            match dm.task_lifecycle.wait_until_active(&dm, &managed, &token).await {
                manager::WaitState::Running => {}
                manager::WaitState::Paused => {
                    if let Some(ref buf) = write_buffer
                        && let Err(e) = buf.flush_all().await
                    {
                        tracing::warn!("flush on pause failed: {e}");
                    }
                    return Ok(RunOutcome::Paused);
                }
                manager::WaitState::Canceled => return Ok(RunOutcome::Canceled),
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
                    dm.task_lifecycle.reset_progress(&dm, &managed, true);
                    if let Some(ref buf) = write_buffer {
                        buf.clear();
                    }
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
                _ = token.cancelled() => {
                    // Flush remaining rate limiter bytes before exiting
                    if bytes_since_consume > 0 {
                        dm.rate_limiter.consume(bytes_since_consume).await;
                    }
                    if let Some(ref buf) = write_buffer {
                        buf.drain_background().await;
                    }
                    return Ok(cancellation_outcome(&managed));
                }
                chunk = stream.next() => chunk,
            } {
                let chunk = chunk?;
                // ── batch rate limiter consume ──
                const BATCH_BYTES: usize = 256 * 1024; // 256 KB
                const BATCH_CHUNKS: usize = 8;
                bytes_since_consume += chunk.len();
                chunks_since_consume += 1;
                if bytes_since_consume >= BATCH_BYTES || chunks_since_consume >= BATCH_CHUNKS {
                    dm.rate_limiter.consume(bytes_since_consume).await;
                    bytes_since_consume = 0;
                    chunks_since_consume = 0;
                }
                // Guard against server sending more data than Content-Length
                if let Some(total) = total_bytes {
                    let len = chunk.len() as u64;
                    if absolute_offset + len > total {
                        return Err(DownloadError::InvalidResponse(format!(
                            "server sent more data than Content-Length ({} bytes received, expected {total})",
                            absolute_offset + len
                        )));
                    }
                }
                if let Some(ref buf) = write_buffer {
                    if buf
                        .buffer_chunk(absolute_offset, chunk.clone())
                        .await
                        .is_err()
                    {
                        // Background flush failed — fall back to direct write.
                        write_all_at(&file, &chunk, absolute_offset)?;
                        if disk_type == DiskType::Hdd {
                            let mut core = managed.lock_core();
                            core.snapshot.degraded = true;
                        }
                    }
                } else {
                    write_all_at(&file, &chunk, absolute_offset)?;
                }
                absolute_offset += chunk.len() as u64;
                dm.task_lifecycle.record_progress(&dm, &managed, None, chunk.len() as u64);
                if last_persist.elapsed() >= PERSIST_INTERVAL {
                    persist_manifest_snapshot(&dm.db, &managed).await?;
                    last_persist = Instant::now();
                    // Throttle progress events: at most once per 500ms
                    if last_progress_emit.elapsed() >= Duration::from_millis(500) {
                        dm.task_lifecycle.emit_progress(&dm, &managed);
                        last_progress_emit = Instant::now();
                    }
                }
                if last_disk_check.elapsed() >= Duration::from_secs(30) {
                    let (total_bytes, downloaded_bytes, destination_dir) = {
                        let core = managed.lock_core();
                        (
                            core.manifest.total_bytes,
                            core.manifest.downloaded_bytes,
                            core.manifest.destination_dir.clone(),
                        )
                    };
                    if let Some(total) = total_bytes {
                        let remaining = total.saturating_sub(downloaded_bytes);
                        if remaining > 0
                            && check_disk_space(Path::new(&destination_dir), remaining).is_err()
                        {
                            let msg =
                                format!("Insufficient disk space: {remaining} bytes remaining");
                            {
                                let mut core = managed.lock_core();
                                core.snapshot.state = DownloadState::Failed;
                                core.snapshot.error = Some(msg.clone());
                                core.snapshot.connection_count = 0;
                                core.snapshot.updated_at_ms = now_ms();
                                core.manifest.state = DownloadState::Failed;
                                core.manifest.error = Some(msg);
                                core.manifest.connection_count = 0;
                                core.manifest.updated_at_ms = now_ms();
                            }
                            dm.event_bus.publish(DownloadEvent::Warning {
                                id: managed.lock_core().manifest.id.clone(),
                                message: "disk full".into(),
                            });
                            return Err(DownloadError::InsufficientDiskSpace {
                                available: 0,
                                required: remaining,
                            });
                        }
                    }
                    last_disk_check = Instant::now();
                }
            }

            // Flush remaining rate limiter bytes after stream ends
            if bytes_since_consume > 0 {
                dm.rate_limiter.consume(bytes_since_consume).await;
            }

            let finished = {
                let core = managed.lock_core();
                match core.manifest.total_bytes {
                    Some(total) => core.manifest.downloaded_bytes >= total,
                    None => true,
                }
            };
            if finished {
                if let Some(ref buf) = write_buffer {
                    // Signal frontend that we're flushing to disk
                    {
                        let mut core = managed.lock_core();
                        core.snapshot.flushing = true;
                    }
                    dm.task_lifecycle.emit_progress(&dm, &managed);

                    let flush_result = buf.flush_all().await;

                    // Always clear the flag, even on error
                    {
                        let mut core = managed.lock_core();
                        core.snapshot.flushing = false;
                    }
                    dm.task_lifecycle.emit_progress(&dm, &managed);
                    flush_result?;
                }
                return Ok(RunOutcome::Finished);
            }
        }
    }

    async fn download_chunked(
        &self,
        dm: Arc<DownloadManager>,
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

        // HDD/SSD optimization: set up buffered writing
        let settings = dm.settings().await?;
        let tail_sprint_enabled = settings.scheduler.tail_sprint_enabled;
        let warmup_enabled = settings.scheduler.connection_warmup_enabled;
        let hdd_buffering = settings.io_baseline.hdd_buffer_enabled;
        let ssd_write_combine_mb = settings.io_baseline.ssd_write_combine_mb;
        drop(settings);
        let checksum_mode = {
            let core = managed.lock_core();
            core.manifest.checksum_mode
        };
        // Incremental Blake3 hasher — avoids file re-read during finalize
        let incremental_hasher: Option<Arc<parking_lot::Mutex<blake3::Hasher>>> =
            if checksum_mode == ChecksumMode::Blake3 {
                Some(Arc::new(parking_lot::Mutex::new(blake3::Hasher::new())))
            } else {
                None
            };
        // ── Connection warmup: pre-establish TCP+TLS before workers start ──
        if warmup_enabled {
            let final_url = managed.lock_core().manifest.final_url.clone();
            let _ = client
                .get(&final_url)
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await;
        }
        let disk_type = {
            let destination_dir = managed.lock_core().manifest.destination_dir.clone();
            dm.resolve_disk_type(Path::new(&destination_dir)).await
        };
        let write_buffer: Option<Arc<DownloadBuffer>> = if disk_type == DiskType::Hdd && hdd_buffering {
            let slot = dm.buffer_pool.acquire_slot().await;
            Some(Arc::new(DownloadBuffer::new_with_worker(
                dm.buffer_pool.clone(),
                slot,
                file.clone(),
                dm.io_worker.clone(),
            )))
        } else {
            let chunk_size = managed.lock_core().manifest.chunk_size;
            let ssd_limit_bytes = if ssd_write_combine_mb == 0 {
                chunk_size  // auto: use the download's chunk size
            } else {
                ssd_write_combine_mb * 1024 * 1024
            };
            // half_size = ssd_limit_bytes / 2, minimum 64 KiB, capped at 8 MiB.
            // Note: for large auto-sized buffers (chunk_size up to 128 MiB),
            // this means at most 8 MiB per download — the ping-pong double-buffer
            // caps peak untracked memory at 16 MiB per download regardless of N.
            let ssd_half_size = (ssd_limit_bytes / 2).clamp(64 * 1024, 8 * 1024 * 1024);
            Some(Arc::new(DownloadBuffer::new_local_pingpong_with_worker(
                ssd_half_size,
                file.clone(),
                dm.io_worker.clone(),
            )))
        };

        // Set disk_type on snapshot for frontend badge display
        {
            let mut core = managed.lock_core();
            core.snapshot.disk_type = Some(disk_type);
        }

        let mut workers = JoinSet::new();
        let mut next_worker_id = 0usize;
        let mut chunk_claim_times: std::collections::HashMap<usize, std::time::Instant> =
            std::collections::HashMap::new();
        let mut last_disk_check = Instant::now();

        loop {
            chunk_claim_times.clear();
            if token.is_cancelled() {
                if let Some(ref buf) = write_buffer {
                    buf.drain_background().await;
                }
                shutdown_chunk_workers(&managed, &mut workers).await;
                return Ok(cancellation_outcome(&managed));
            }

            if last_disk_check.elapsed() >= Duration::from_secs(30) {
                let (total_bytes, downloaded_bytes, destination_dir) = {
                    let core = managed.lock_core();
                    (
                        core.manifest.total_bytes,
                        core.manifest.downloaded_bytes,
                        core.manifest.destination_dir.clone(),
                    )
                };
                if let Some(total) = total_bytes {
                    let remaining = total.saturating_sub(downloaded_bytes);
                    if remaining > 0
                        && check_disk_space(Path::new(&destination_dir), remaining).is_err()
                    {
                        let msg = format!("Insufficient disk space: {remaining} bytes remaining");
                        {
                            let mut core = managed.lock_core();
                            core.snapshot.state = DownloadState::Failed;
                            core.snapshot.error = Some(msg.clone());
                            core.snapshot.connection_count = 0;
                            core.snapshot.updated_at_ms = now_ms();
                            core.manifest.state = DownloadState::Failed;
                            core.manifest.error = Some(msg);
                            core.manifest.connection_count = 0;
                            core.manifest.updated_at_ms = now_ms();
                        }
                        dm.event_bus.publish(DownloadEvent::Warning {
                            id: managed.lock_core().manifest.id.clone(),
                            message: "disk full".into(),
                        });
                        return Err(DownloadError::InsufficientDiskSpace {
                            available: 0,
                            required: remaining,
                        });
                    }
                }
                last_disk_check = Instant::now();
            }

            if all_chunks_completed(&managed) {
                shutdown_chunk_workers(&managed, &mut workers).await;
                if let Some(ref buf) = write_buffer {
                    // Signal frontend that we're flushing to disk
                    {
                        let mut core = managed.lock_core();
                        core.snapshot.flushing = true;
                    }
                    dm.task_lifecycle.emit_progress(&dm, &managed);

                    let flush_result = buf.flush_all().await;

                    // Always clear the flag, even on error
                    {
                        let mut core = managed.lock_core();
                        core.snapshot.flushing = false;
                    }
                    dm.task_lifecycle.emit_progress(&dm, &managed);
                    flush_result?;
                }
                // Store incremental checksum if computed
                if let Some(ref hasher) = incremental_hasher {
                    let guard = hasher.lock();
                    let hash = guard.finalize();
                    let mut core = managed.lock_core();
                    let hex = hash.to_hex().to_string();
                    core.manifest.checksum = Some(hex);
                    core.snapshot.checksum = core.manifest.checksum.clone();
                }
                return Ok(RunOutcome::Finished);
            }

            let allocation = current_allocation(&managed);
            if allocation == 0 && workers.is_empty() {
                match dm.task_lifecycle.wait_until_active(&dm, &managed, &token).await {
                    manager::WaitState::Running => {}
                    manager::WaitState::Paused => {
                        if let Some(ref buf) = write_buffer
                            && let Err(e) = buf.flush_all().await
                        {
                            tracing::warn!("flush on pause failed: {e}");
                        }
                        // Clear incremental checksum — fresh hasher on resume misses pre-pause bytes
                        managed.lock_core().manifest.checksum = None;
                        return Ok(RunOutcome::Paused);
                    }
                    manager::WaitState::Canceled => return Ok(RunOutcome::Canceled),
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

            // ── Tail Sprint ──────────────────────────────────────
            // Stage 1: Fresh-connection retry for stalled tail chunks.
            // Stage 2: Split the last unclaimed chunk into two sub-chunks.
            if tail_sprint_enabled {
                let tail_state = {
                    let core = managed.lock_core();
                    let uncompleted: Vec<&crate::manifest::ChunkManifest> = core
                        .manifest
                        .chunks
                        .iter()
                        .filter(|c| !c.completed)
                        .collect();
                    let count = uncompleted.len();
                    let all_claimed =
                        !uncompleted.is_empty() && uncompleted.iter().all(|c| c.claimed_by.is_some());
                    // For Stage 2: find the unclaimed chunk (if exactly one)
                    let unclaimed_idx = if count == 1 && !all_claimed {
                        uncompleted.first().map(|c| c.index)
                    } else {
                        None
                    };
                    (count, all_claimed, unclaimed_idx)
                };

                // Stage 1: stalled chunk detection — release slow claimed tail chunks
                if tail_state.0 <= 2 && tail_state.1 && !chunk_claim_times.is_empty() {
                    let now = std::time::Instant::now();
                    let stall_limit = std::time::Duration::from_secs(TAIL_SPRINT_STALL_WINDOW_SECS);
                    let stalled: Vec<usize> = chunk_claim_times
                        .iter()
                        .filter(|(_, t)| now.duration_since(**t) > stall_limit)
                        .map(|(idx, _)| *idx)
                        .collect();

                    if !stalled.is_empty() {
                        // Release stalled chunks for fresh re-claim next iteration.
                        // Non-stalled workers continue unaffected; new workers spawned next
                        // iteration will pick up the released chunks with fresh connections.
                        {
                            let mut core = managed.lock_core();
                            for idx in &stalled {
                                // Direct index lookup (chunks are stored in Vec at index position)
                                if let Some(chunk) = core.manifest.chunks.get_mut(*idx) {
                                    chunk.claimed_by = None;
                                    chunk.dirty = true;
                                }
                            }
                        }
                        continue;
                    }
                }

                // Stage 2: split the last unclaimed chunk into two sub-chunks
                if let Some(last_idx) = tail_state.2 {
                    let new_chunk_idx = {
                        let core = managed.lock_core();
                        core.manifest.chunks.len()
                    };
                    let mut core = managed.lock_core();
                    if let Some(chunk) = core.manifest.chunks.get_mut(last_idx)
                        && !chunk.completed && chunk.claimed_by.is_none()
                    {
                        let remaining = chunk
                            .end
                            .saturating_sub(chunk.start)
                            .saturating_add(1)
                            .saturating_sub(chunk.downloaded);
                        if remaining > TAIL_SPRINT_MIN_SPLIT_SIZE {
                            let mid = chunk.start + chunk.downloaded + remaining / 2;
                            let old_end = chunk.end;
                            chunk.end = mid.saturating_sub(1);
                            chunk.dirty = true;
                            core.manifest.chunks.push(crate::manifest::ChunkManifest {
                                index: new_chunk_idx,
                                start: mid,
                                end: old_end,
                                downloaded: 0,
                                completed: false,
                                claimed_by: None,
                                dirty: true,
                            });
                        }
                    }
                }
            }

            while workers.len() < target_workers {
                let worker_id = next_worker_id;
                let chunk = {
                    let mut core = managed.lock_core();
                    claim_next_chunk(&mut core.manifest, worker_id, target_workers)
                };
                let Some(chunk) = chunk else {
                    break;
                };
                chunk_claim_times.insert(chunk.index, std::time::Instant::now());

                {
                    let mut core = managed.lock_core();
                    core.snapshot.state = DownloadState::Downloading;
                    core.snapshot.connection_count = target_workers;
                    core.snapshot.updated_at_ms = now_ms();
                    core.manifest.state = DownloadState::Downloading;
                    core.manifest.connection_count = target_workers;
                    core.manifest.updated_at_ms = now_ms();
                }

                let db = dm.db.clone();
                let rate_limiter = dm.rate_limiter.clone();
                let manager_for_worker = dm.clone();
                let managed = managed.clone();
                let client = client.clone();
                let token = token.clone();
                let file = file.clone();
                let wbuf = write_buffer.clone();
                let dtyp = disk_type;
                let inc_hasher = incremental_hasher.clone();
                workers.spawn(async move {
                    download_chunk(ChunkWorkerCtx {
                        managed,
                        client,
                        token,
                        file,
                        chunk,
                        max_retries,
                        db,
                        rate_limiter,
                        manager: manager_for_worker,
                        write_buffer: wbuf,
                        disk_type: dtyp,
                        incremental_hasher: inc_hasher,
                        worker_id,
                    })
                    .await
                });
                next_worker_id = next_worker_id.saturating_add(1);
            }

            if workers.is_empty() {
                tokio::select! {
                    _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                    _ = dm.controls.rebalance_notify.notified() => {}
                    _ = sleep(Duration::from_millis(120)) => {}
                }
                continue;
            }

            let join_result = tokio::select! {
                _ = token.cancelled() => {
                    if let Some(ref buf) = write_buffer {
                        buf.drain_background().await;
                    }
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
                    dm.task_lifecycle.prepare_fresh_temp_file(&dm, &managed)?;
                    dm.task_lifecycle.reset_progress(&dm, &managed, true);
                    return self
                        .download_single(dm, managed, client, token, max_retries)
                        .await;
                }
                ChunkWorkerOutcome::Paused => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    if let Some(ref buf) = write_buffer
                        && let Err(e) = buf.flush_all().await
                    {
                        tracing::warn!("flush on pause failed: {e}");
                    }
                    // Clear incremental checksum — fresh hasher on resume misses pre-pause bytes
                    managed.lock_core().manifest.checksum = None;
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
        dm: Arc<DownloadManager>,
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
        dm.persist(managed.clone()).await?;

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        let (temp_path, destination_path, checksum_mode, expected_checksum, precomputed) = {
            let core = managed.lock_core();
            (
                PathBuf::from(core.manifest.temp_path.clone()),
                PathBuf::from(core.manifest.destination_path.clone()),
                core.manifest.checksum_mode,
                core.manifest.expected_checksum.clone(),
                core.manifest.checksum.clone(),
            )
        };

        let checksum = match checksum_mode {
            ChecksumMode::None => None,
            _ if precomputed.is_some() => precomputed,
            mode => Some(calculate_checksum(temp_path.clone(), mode).await?),
        };

        // Verify expected checksum if one was provided
        if let (Some(expected), Some(computed)) = (&expected_checksum, &checksum)
            && !expected.eq_ignore_ascii_case(computed)
        {
            let error_msg = format!("Checksum mismatch: expected {expected}, got {computed}");
            {
                let mut core = managed.lock_core();
                core.snapshot.state = DownloadState::Failed;
                core.snapshot.error = Some(error_msg.clone());
                core.snapshot.connection_count = 0;
                core.snapshot.allocated_thread_count = Some(0);
                core.snapshot.updated_at_ms = now_ms();
                core.manifest.state = DownloadState::Failed;
                core.manifest.error = Some(error_msg);
                core.manifest.connection_count = 0;
                core.manifest.allocated_thread_count = Some(0);
                core.manifest.updated_at_ms = now_ms();
            }
            dm.persist(managed.clone()).await?;
            return Ok(RunOutcome::Finished);
        }

        // Log success when checksum was computed and either matched or not checked
        if checksum_mode != ChecksumMode::None {
            tracing::info!("checksum verified");
        }

        if finalize_was_canceled(&managed, &token) {
            return Ok(RunOutcome::Canceled);
        }

        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
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
        dm.persist(managed.clone()).await?;

        // Broadcast aria2.onDownloadComplete via EventBus
        let download_id = managed.lock_core().snapshot.id.clone();
        let gid = format!(
            "{:016x}",
            xxhash_rust::xxh3::xxh3_64(download_id.as_bytes())
        );
        dm.event_bus.publish(DownloadEvent::Aria2Notification {
            event_name: "aria2.onDownloadComplete".into(),
            gid,
        });

        Ok(RunOutcome::Finished)
    }
}

// ── Free helper functions ─────────────────────────────────────────────────────

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

fn claim_next_chunk(manifest: &mut crate::manifest::Manifest, worker_id: usize, total_workers: usize) -> Option<ChunkManifest> {
    let stripe_count = total_workers.max(1);
    let stripe = worker_id % stripe_count;
    // Try interleaved: chunks whose index matches the worker's stripe
    if let Some(chunk) = manifest.chunks.iter_mut()
        .find(|c| !c.completed && c.claimed_by.is_none() && c.index % stripe_count == stripe)
    {
        chunk.claimed_by = Some(worker_id);
        chunk.dirty = true;
        return Some(chunk.clone());
    }
    // Fallback: any unclaimed chunk (handles cases where stripe is exhausted)
    let chunk = manifest.chunks.iter_mut()
        .find(|chunk| !chunk.completed && chunk.claimed_by.is_none())?;
    chunk.claimed_by = Some(worker_id);
    chunk.dirty = true;
    Some(chunk.clone())
}

pub(crate) fn mark_chunk_released(managed: &Arc<ManagedDownload>, chunk_index: usize, worker_id: usize) {
    let mut core = managed.lock_core();
    if let Some(chunk) = core.manifest.chunks.get_mut(chunk_index) {
        // Only clear the claim if it still belongs to this worker.
        // If tail sprint released it and a new worker claimed it,
        // we must not clear the new worker's claim.
        if chunk.claimed_by == Some(worker_id) {
            chunk.claimed_by = None;
            chunk.dirty = true;
        }
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

struct ChunkWorkerCtx {
    managed: Arc<ManagedDownload>,
    client: Client,
    token: CancellationToken,
    file: Arc<std::fs::File>,
    chunk: ChunkManifest,
    max_retries: u32,
    db: Arc<Database>,
    rate_limiter: Arc<RateLimiter>,
    manager: Arc<DownloadManager>,
    write_buffer: Option<Arc<DownloadBuffer>>,
    disk_type: DiskType,
    incremental_hasher: Option<Arc<parking_lot::Mutex<blake3::Hasher>>>,
    worker_id: usize,
}

async fn download_chunk(ctx: ChunkWorkerCtx) -> Result<ChunkWorkerOutcome> {
    let mut current = ctx.chunk.start + ctx.chunk.downloaded;
    let end = ctx.chunk.end;
    if current > end {
        mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
        return Ok(ChunkWorkerOutcome::Finished);
    }

    let mut last_persist = Instant::now();
    // ── progress throttling ──
    let mut last_progress_emit = Instant::now();
    // ── rate limiter batch consume ──
    let mut bytes_since_consume: usize = 0;
    let mut chunks_since_consume: usize = 0;
    while current <= end {
        if ctx.token.is_cancelled() {
            mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
            return Ok(match ctx.managed.lock_core().snapshot.state {
                DownloadState::Canceled => ChunkWorkerOutcome::Canceled,
                _ => ChunkWorkerOutcome::Paused,
            });
        }

        // Check if tail sprint released our chunk claim
        {
            let core = ctx.managed.lock_core();
            if let Some(chunk) = core.manifest.chunks.get(ctx.chunk.index)
                && chunk.claimed_by != Some(ctx.worker_id)
            {
                return Ok(ChunkWorkerOutcome::Finished);
            }
        }

        let (url, user_agent, validator) = {
            let core = ctx.managed.lock_core();
            (
                core.manifest.final_url.clone(),
                core.manifest.user_agent.clone(),
                if_range_header(&core.manifest),
            )
        };

        let response = request_with_retry(
            || {
                let client = ctx.client.clone();
                let url = url.clone();
                let user_agent = user_agent.clone();
                let validator = validator.clone();
                async move {
                    build_segment_request(&client, &url, &user_agent, current, end, validator)
                        .send()
                        .await
                }
            },
            ctx.token.clone(),
            ctx.max_retries,
            ctx.managed.clone(),
        )
        .await?;

        if response.status() == StatusCode::OK {
            mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
            return Ok(ChunkWorkerOutcome::RestartSingle);
        }

        validate_segment_response(&response, current, end)?;

        let mut stream = response.bytes_stream();
        while let Some(bytes) = tokio::select! {
            _ = ctx.token.cancelled() => {
                // Flush remaining rate limiter bytes before exiting
                if bytes_since_consume > 0 {
                    ctx.rate_limiter.consume(bytes_since_consume).await;
                }
                mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
                return Ok(cancellation_chunk_outcome(&ctx.managed));
            }
            next = stream.next() => next,
        } {
            let bytes = bytes?;
            // ── batch rate limiter consume ──
            const BATCH_BYTES: usize = 256 * 1024; // 256 KB
            const BATCH_CHUNKS: usize = 8;
            bytes_since_consume += bytes.len();
            chunks_since_consume += 1;
            if bytes_since_consume >= BATCH_BYTES || chunks_since_consume >= BATCH_CHUNKS {
                ctx.rate_limiter.consume(bytes_since_consume).await;
                bytes_since_consume = 0;
                chunks_since_consume = 0;
            }
            if current + bytes.len() as u64 - 1 > end {
                mark_chunk_released(&ctx.managed, ctx.chunk.index, ctx.worker_id);
                return Err(DownloadError::InvalidResponse(String::from(
                    "segment body exceeded requested range",
                )));
            }

            if let Some(ref buf) = ctx.write_buffer {
                if buf.buffer_chunk(current, bytes.clone()).await.is_err() {
                    // Background flush failed — fall back to direct write.
                    write_all_at(&ctx.file, &bytes, current)?;
                    if ctx.disk_type == DiskType::Hdd {
                        let mut core = ctx.managed.lock_core();
                        core.snapshot.degraded = true;
                    }
                }
            } else {
                write_all_at(&ctx.file, &bytes, current)?;
            }
            // Feed downloaded bytes through incremental hasher (Blake3 only)
            if let Some(ref hasher) = ctx.incremental_hasher {
                let mut guard = hasher.lock();
                guard.update(&bytes);
            }
            current += bytes.len() as u64;
            {
                record_progress_on_managed(&ctx.managed, Some(ctx.chunk.index), bytes.len() as u64);
            }
            // Check if tail sprint released our chunk claim — exit early to avoid
            // wasting bandwidth competing with a new worker on the same chunk.
            {
                let claimed_by = {
                    let core = ctx.managed.lock_core();
                    core.manifest.chunks.get(ctx.chunk.index).and_then(|c| c.claimed_by)
                };
                if claimed_by != Some(ctx.worker_id) {
                    // Flush remaining rate limiter bytes before exiting
                    if bytes_since_consume > 0 {
                        ctx.rate_limiter.consume(bytes_since_consume).await;
                    }
                    return Ok(ChunkWorkerOutcome::Finished);
                }
            }
            if last_persist.elapsed() >= PERSIST_INTERVAL {
                persist_manifest_snapshot(&ctx.db, &ctx.managed).await?;
                last_persist = Instant::now();
                // Throttle progress events: at most once per 500ms
                if last_progress_emit.elapsed() >= Duration::from_millis(500) {
                    ctx.manager.task_lifecycle.emit_progress(&ctx.manager, &ctx.managed);
                    last_progress_emit = Instant::now();
                }
            }
        }
    }

    // Flush remaining rate limiter bytes after chunk completes
    if bytes_since_consume > 0 {
        ctx.rate_limiter.consume(bytes_since_consume).await;
    }

    {
        let mut core = ctx.managed.lock_core();
        if let Some(target) = core.manifest.chunks.get_mut(ctx.chunk.index) {
            // Only mark completed if we still own the claim
            if target.claimed_by == Some(ctx.worker_id) {
                target.completed = true;
                target.downloaded = target.end.saturating_sub(target.start) + 1;
                target.claimed_by = None;
                target.dirty = true;
            }
        }
        core.manifest.updated_at_ms = now_ms();
    }
    Ok(ChunkWorkerOutcome::Finished)
}
