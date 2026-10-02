//! Chunked (parallel) download path, tail sprint and worker supervision.

use super::worker::{ChunkWorkerCtx, all_chunks_completed, claim_or_steal_chunk, current_allocation, download_chunk, shutdown_chunk_workers};
use super::{Arc, CancellationToken, ChunkWorkerOutcome, Client, DiskType, DownloadBuffer, DownloadError, DownloadEvent, DownloadManager, DownloadState, Duration, HttpExecutor, Instant, JoinSet, ManagedDownload, Path, PathBuf, Result, RunOutcome, TAIL_SPRINT_MIN_SPLIT_SIZE, TAIL_SPRINT_STALL_WINDOW_SECS, ThreadMode, cancellation_outcome, check_disk_space, now_ms, open_download_file, persist_manifest_snapshot, sleep};

impl HttpExecutor {
    pub(super) async fn download_chunked(
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
        // Test-only: remember this download's temp file (by id) so a pipeline
        // fault-injection test can arm a targeted write-failure. Inert in prod.
        #[cfg(any(test, feature = "test-utils"))]
        crate::buffer_pool::fault::register_file(&managed.lock_core().manifest.id, &file);

        // HDD/SSD optimization: set up buffered writing
        let settings = dm.settings().await?;
        let tail_sprint_enabled = settings.scheduler.tail_sprint_enabled;
        let warmup_enabled = settings.scheduler.connection_warmup_enabled;
        let hdd_buffering = settings.io_baseline.hdd_buffer_enabled;
        let ssd_write_combine_mb = settings.io_baseline.ssd_write_combine_mb;
        drop(settings);
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
        let write_buffer: Option<Arc<DownloadBuffer>> =
            if disk_type == DiskType::Hdd && hdd_buffering {
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
                    chunk_size // auto: use the download's chunk size
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
                shutdown_chunk_workers(&managed, &mut workers).await;
                // Persist buffered chunks before exit so resume continues from a
                // consistent point and the buffer is never dropped with data.
                if let Some(ref buf) = write_buffer
                    && let Err(e) = buf.flush_all().await
                {
                    tracing::warn!("flush on cancel failed: {e}");
                }
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
                return Ok(RunOutcome::Finished);
            }

            let allocation = current_allocation(&managed);
            if allocation == 0 && workers.is_empty() {
                match dm
                    .task_lifecycle
                    .wait_until_active(&dm, &managed, &token)
                    .await
                {
                    crate::download::WaitState::Running => {}
                    crate::download::WaitState::Paused => {
                        if let Some(ref buf) = write_buffer
                            && let Err(e) = buf.flush_all().await
                        {
                            tracing::warn!("flush on pause failed: {e}");
                        }
                        return Ok(RunOutcome::Paused);
                    }
                    crate::download::WaitState::Canceled => {
                        if let Some(ref buf) = write_buffer
                            && let Err(e) = buf.flush_all().await
                        {
                            tracing::warn!("flush on cancel failed: {e}");
                        }
                        return Ok(RunOutcome::Canceled);
                    }
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
                    let all_claimed = !uncompleted.is_empty()
                        && uncompleted.iter().all(|c| c.claimed_by.is_some());
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
                        && !chunk.completed
                        && chunk.claimed_by.is_none()
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
                    claim_or_steal_chunk(&mut core.manifest, worker_id, target_workers)
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
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    if let Some(ref buf) = write_buffer
                        && let Err(e) = buf.flush_all().await
                    {
                        tracing::warn!("flush on cancel failed: {e}");
                    }
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
                ChunkWorkerOutcome::DowngradeSingleThread => {
                    tracing::info!(
                        "Received HTTP 429 Too Many Requests; downgrading to single-thread mode"
                    );
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    if let Some(ref buf) = write_buffer
                        && let Err(e) = buf.flush_all().await
                    {
                        tracing::warn!("flush on downgrade failed: {e}");
                    }
                    {
                        let mut core = managed.lock_core();
                        core.manifest.thread_mode = ThreadMode::Fixed;
                        core.manifest.requested_thread_count = Some(1);
                        core.manifest.desired_thread_count = Some(1);
                        core.manifest.allocated_thread_count = Some(1);
                        core.manifest.connection_count = 1;
                        core.manifest.thread_note = Some(String::from("单线程（429 限流降级）"));
                        core.manifest.error = None;
                        core.manifest.updated_at_ms = now_ms();
                        core.sync_snapshot_from_manifest();
                    }
                    dm.task_lifecycle.emit_progress(&dm, &managed);
                    persist_manifest_snapshot(&dm.db, &managed).await?;
                    dm.controls.rebalance_notify.notify_waiters();
                    tokio::select! {
                        _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                        _ = sleep(Duration::from_millis(1500)) => {}
                    }
                    continue;
                }
                ChunkWorkerOutcome::RestartSingle => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    // Data is deliberately discarded (fresh temp file) — clear
                    // the buffer so the Drop canary doesn't misfire a false
                    // "data lost" warning.
                    if let Some(ref buf) = write_buffer {
                        buf.clear();
                    }
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
                    return Ok(RunOutcome::Paused);
                }
                ChunkWorkerOutcome::Canceled => {
                    shutdown_chunk_workers(&managed, &mut workers).await;
                    if let Some(ref buf) = write_buffer
                        && let Err(e) = buf.flush_all().await
                    {
                        tracing::warn!("flush on cancel failed: {e}");
                    }
                    return Ok(RunOutcome::Canceled);
                }
            }
        }
    }
}
