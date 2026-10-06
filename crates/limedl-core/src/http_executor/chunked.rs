//! Chunked (parallel) download path, tail sprint and worker supervision.

use super::worker::{ChunkWorkerCtx, all_chunks_completed, claim_or_steal_chunk, current_allocation, download_chunk, shutdown_chunk_workers};
use super::{Arc, CancellationToken, ChunkWorkerOutcome, Client, DiskType, DownloadBuffer, DownloadError, DownloadManager, DownloadState, Duration, HttpExecutor, Instant, JoinSet, ManagedDownload, Path, PathBuf, Result, RunOutcome, TAIL_SPRINT_MIN_SPLIT_SIZE, TAIL_SPRINT_STALL_WINDOW_SECS, ThreadMode, build_write_buffer, cancellation_outcome, check_disk_space_periodically, finish_buffer_flush, flush_write_buffer, identity_encoding, now_ms, open_download_file, persist_manifest_snapshot, sleep, wait_or_stop};

impl HttpExecutor {
    pub(super) async fn download_chunked(
        &self,
        dm: Arc<DownloadManager>,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<RunOutcome> {
        let (file, disk_type, write_buffer, tail_sprint_enabled) =
            open_chunked_target(&dm, &managed, &client).await?;
        let write_buffer = Some(write_buffer);

        let pool = WorkerPool {
            dm: &dm,
            managed: &managed,
            client: &client,
            token: &token,
            file: &file,
            write_buffer: &write_buffer,
            disk_type,
            max_retries,
        };

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
                flush_write_buffer(&write_buffer, "cancel").await;
                return Ok(cancellation_outcome(&managed));
            }

            check_disk_space_periodically(&dm, &managed, &mut last_disk_check).await?;

            if all_chunks_completed(&managed) {
                shutdown_chunk_workers(&managed, &mut workers).await;
                finish_buffer_flush(&dm, &managed, &write_buffer).await?;
                return Ok(RunOutcome::Finished);
            }

            if current_allocation(&managed) == 0
                && workers.is_empty()
                && let Some(outcome) = wait_or_stop(&dm, &managed, &token, &write_buffer).await
            {
                return Ok(outcome);
            }

            let target_workers = {
                let mut target = current_allocation(&managed);
                let chunk_count = {
                    let core = managed.lock_core();
                    core.manifest.chunks.len()
                };
                if chunk_count > 0 {
                    target = target.min(chunk_count);
                }
                target
            };

            // ── Tail Sprint ──────────────────────────────────────
            // Stage 1: Fresh-connection retry for stalled tail chunks.
            // Stage 2: Split the last unclaimed chunk into two sub-chunks.
            if tail_sprint_enabled && tail_sprint_step(&managed, &chunk_claim_times) {
                continue;
            }

            grow_worker_pool(
                &mut workers,
                &pool,
                target_workers,
                &mut next_worker_id,
                &mut chunk_claim_times,
            );

            if workers.is_empty() {
                tokio::select! {
                    _ = token.cancelled() => return Ok(cancellation_outcome(&managed)),
                    _ = dm.controls.rebalance_notify.notified() => {}
                    _ = sleep(Duration::from_millis(120)) => {}
                }
                continue;
            }

            match join_one_worker(&managed, &mut workers, &token, &write_buffer).await? {
                WorkerJoin::Recheck => continue,
                WorkerJoin::Cancelled(outcome) => return Ok(outcome),
                WorkerJoin::Outcome(outcome) => {
                    match handle_worker_outcome(
                        &dm,
                        &managed,
                        &token,
                        &mut workers,
                        &write_buffer,
                        outcome,
                    )
                    .await?
                    {
                        SupervisorStep::Continue => continue,
                        SupervisorStep::Return(outcome) => return Ok(outcome),
                        SupervisorStep::RestartSingle => {
                            // Data is deliberately discarded (fresh temp file),
                            // so release our file handle before recreating it.
                            drop(file);
                            dm.task_lifecycle.prepare_fresh_temp_file(&dm, &managed)?;
                            dm.task_lifecycle.reset_progress(&dm, &managed, true);
                            return self
                                .download_single(dm, managed, client, token, max_retries)
                                .await;
                        }
                    }
                }
            }
        }
    }
}

/// Open the temp file, warm up the connection and build the write buffer.
///
/// Also publishes the detected disk type on the snapshot for the UI badge.
async fn open_chunked_target(
    dm: &Arc<DownloadManager>,
    managed: &Arc<ManagedDownload>,
    client: &Client,
) -> Result<(Arc<std::fs::File>, DiskType, Arc<DownloadBuffer>, bool)> {
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
        let _ = identity_encoding(
            client
                .get(&final_url)
                .header(reqwest::header::RANGE, "bytes=0-0"),
        )
        .send()
        .await;
    }
    let disk_type = {
        let destination_dir = managed.lock_core().manifest.destination_dir.clone();
        dm.resolve_disk_type(Path::new(&destination_dir)).await
    };
    let write_buffer =
        build_write_buffer(dm, managed, &file, disk_type, hdd_buffering, ssd_write_combine_mb)
            .await;

    // Set disk_type on snapshot for frontend badge display
    {
        let mut core = managed.lock_core();
        core.snapshot.disk_type = Some(disk_type);
    }

    Ok((file, disk_type, write_buffer, tail_sprint_enabled))
}

/// Everything a spawned chunk worker needs, minus the claimed chunk itself.
struct WorkerPool<'a> {
    dm: &'a Arc<DownloadManager>,
    managed: &'a Arc<ManagedDownload>,
    client: &'a Client,
    token: &'a CancellationToken,
    file: &'a Arc<std::fs::File>,
    write_buffer: &'a Option<Arc<DownloadBuffer>>,
    disk_type: DiskType,
    max_retries: u32,
}

/// Spawn workers until `target_workers` or no claimable chunk is left.
fn grow_worker_pool(
    workers: &mut JoinSet<Result<ChunkWorkerOutcome>>,
    pool: &WorkerPool<'_>,
    target_workers: usize,
    next_worker_id: &mut usize,
    chunk_claim_times: &mut std::collections::HashMap<usize, std::time::Instant>,
) {
    while workers.len() < target_workers {
        let worker_id = *next_worker_id;
        let chunk = {
            let mut core = pool.managed.lock_core();
            claim_or_steal_chunk(&mut core.manifest, worker_id, target_workers)
        };
        let Some(chunk) = chunk else {
            break;
        };
        chunk_claim_times.insert(chunk.index, std::time::Instant::now());

        {
            let mut core = pool.managed.lock_core();
            core.snapshot.state = DownloadState::Downloading;
            core.snapshot.connection_count = target_workers;
            core.snapshot.updated_at_ms = now_ms();
            core.manifest.state = DownloadState::Downloading;
            core.manifest.connection_count = target_workers;
            core.manifest.updated_at_ms = now_ms();
        }

        let db = pool.dm.db.clone();
        let rate_limiter = pool.dm.rate_limiter.clone();
        let manager_for_worker = pool.dm.clone();
        let managed = pool.managed.clone();
        let client = pool.client.clone();
        let token = pool.token.clone();
        let file = pool.file.clone();
        let wbuf = pool.write_buffer.clone();
        let dtyp = pool.disk_type;
        let max_retries = pool.max_retries;
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
        *next_worker_id = next_worker_id.saturating_add(1);
    }
}

/// Snapshot of the tail state used by the two Tail Sprint stages.
struct TailState {
    count: usize,
    all_claimed: bool,
    /// Index of the single unclaimed chunk, when exactly one remains.
    unclaimed_idx: Option<usize>,
}

fn tail_state(managed: &Arc<ManagedDownload>) -> TailState {
    let core = managed.lock_core();
    let uncompleted: Vec<&crate::manifest::ChunkManifest> = core
        .manifest
        .chunks
        .iter()
        .filter(|c| !c.completed)
        .collect();
    let count = uncompleted.len();
    let all_claimed = !uncompleted.is_empty() && uncompleted.iter().all(|c| c.claimed_by.is_some());
    // For Stage 2: find the unclaimed chunk (if exactly one)
    let unclaimed_idx = if count == 1 && !all_claimed {
        uncompleted.first().map(|c| c.index)
    } else {
        None
    };
    TailState {
        count,
        all_claimed,
        unclaimed_idx,
    }
}

/// Stage 1: release slow claimed tail chunks; `true` means "retry the loop".
fn release_stalled_tail_chunks(
    managed: &Arc<ManagedDownload>,
    chunk_claim_times: &std::collections::HashMap<usize, std::time::Instant>,
) -> bool {
    let now = std::time::Instant::now();
    let stall_limit = std::time::Duration::from_secs(TAIL_SPRINT_STALL_WINDOW_SECS);
    let stalled: Vec<usize> = chunk_claim_times
        .iter()
        .filter(|(_, t)| now.duration_since(**t) > stall_limit)
        .map(|(idx, _)| *idx)
        .collect();

    if stalled.is_empty() {
        return false;
    }

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
    true
}

/// Stage 2: split the last unclaimed chunk into two sub-chunks.
fn split_tail_chunk(managed: &Arc<ManagedDownload>, last_idx: usize) {
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
                durable_downloaded: 0,
                claimed_by: None,
                dirty: true,
            });
        }
    }
}

/// Run both Tail Sprint stages; `true` means the supervisor should retry.
fn tail_sprint_step(
    managed: &Arc<ManagedDownload>,
    chunk_claim_times: &std::collections::HashMap<usize, std::time::Instant>,
) -> bool {
    let state = tail_state(managed);

    // Stage 1: stalled chunk detection — release slow claimed tail chunks
    if state.count <= 2
        && state.all_claimed
        && !chunk_claim_times.is_empty()
        && release_stalled_tail_chunks(managed, chunk_claim_times)
    {
        return true;
    }

    // Stage 2: split the last unclaimed chunk into two sub-chunks
    if let Some(last_idx) = state.unclaimed_idx {
        split_tail_chunk(managed, last_idx);
    }
    false
}

/// Result of waiting for one worker to finish.
enum WorkerJoin {
    /// A worker finished with this outcome.
    Outcome(ChunkWorkerOutcome),
    /// Cancellation won the race; the run must stop.
    Cancelled(RunOutcome),
    /// No worker was running; the supervisor should re-evaluate its state.
    Recheck,
}

/// Await the next finished worker (or cancellation).
async fn join_one_worker(
    managed: &Arc<ManagedDownload>,
    workers: &mut JoinSet<Result<ChunkWorkerOutcome>>,
    token: &CancellationToken,
    write_buffer: &Option<Arc<DownloadBuffer>>,
) -> Result<WorkerJoin> {
    let join_result = tokio::select! {
        _ = token.cancelled() => {
            shutdown_chunk_workers(managed, workers).await;
            flush_write_buffer(write_buffer, "cancel").await;
            return Ok(WorkerJoin::Cancelled(cancellation_outcome(managed)));
        }
        joined = workers.join_next() => joined,
    };

    let Some(join_result) = join_result else {
        return Ok(WorkerJoin::Recheck);
    };
    let worker_outcome = match join_result {
        Ok(Ok(outcome)) => outcome,
        Ok(Err(error)) => {
            shutdown_chunk_workers(managed, workers).await;
            return Err(error);
        }
        Err(error) => {
            shutdown_chunk_workers(managed, workers).await;
            return Err(DownloadError::InvalidResponse(error.to_string()));
        }
    };
    Ok(WorkerJoin::Outcome(worker_outcome))
}

/// What the supervisor should do after a worker outcome.
enum SupervisorStep {
    Continue,
    Return(RunOutcome),
    RestartSingle,
}

/// Fold one worker outcome into the supervisor decision.
async fn handle_worker_outcome(
    dm: &Arc<DownloadManager>,
    managed: &Arc<ManagedDownload>,
    token: &CancellationToken,
    workers: &mut JoinSet<Result<ChunkWorkerOutcome>>,
    write_buffer: &Option<Arc<DownloadBuffer>>,
    outcome: ChunkWorkerOutcome,
) -> Result<SupervisorStep> {
    match outcome {
        ChunkWorkerOutcome::Finished => Ok(SupervisorStep::Continue),
        ChunkWorkerOutcome::DowngradeSingleThread => {
            tracing::info!(
                "Received HTTP 429 Too Many Requests; downgrading to single-thread mode"
            );
            shutdown_chunk_workers(managed, workers).await;
            flush_write_buffer(write_buffer, "downgrade").await;
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
            dm.task_lifecycle.emit_progress(dm, managed);
            persist_manifest_snapshot(&dm.db, managed).await?;
            dm.controls.rebalance_notify.notify_waiters();
            tokio::select! {
                _ = token.cancelled() => {
                    return Ok(SupervisorStep::Return(cancellation_outcome(managed)));
                }
                _ = sleep(Duration::from_millis(1500)) => {}
            }
            Ok(SupervisorStep::Continue)
        }
        ChunkWorkerOutcome::RestartSingle => {
            shutdown_chunk_workers(managed, workers).await;
            // Data is deliberately discarded (fresh temp file) — clear
            // the buffer so the Drop canary doesn't misfire a false
            // "data lost" warning.
            if let Some(buf) = write_buffer {
                buf.clear();
            }
            Ok(SupervisorStep::RestartSingle)
        }
        ChunkWorkerOutcome::Paused => {
            shutdown_chunk_workers(managed, workers).await;
            flush_write_buffer(write_buffer, "pause").await;
            Ok(SupervisorStep::Return(RunOutcome::Paused))
        }
        ChunkWorkerOutcome::Canceled => {
            shutdown_chunk_workers(managed, workers).await;
            flush_write_buffer(write_buffer, "cancel").await;
            Ok(SupervisorStep::Return(RunOutcome::Canceled))
        }
    }
}
