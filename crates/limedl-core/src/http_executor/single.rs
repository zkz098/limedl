//! Single-stream download path.

use futures_util::StreamExt;

use super::{Arc, CancellationToken, Client, DiskType, DownloadBuffer, DownloadError, DownloadEvent, DownloadManager, DownloadState, Duration, HttpExecutor, Instant, ManagedDownload, PERSIST_INTERVAL, Path, PathBuf, Result, RunOutcome, StatusCode, apply_extra_headers, cancellation_outcome, check_disk_space, contiguous_prefix_end, fs, header, if_range_header, io_error_with_path, now_ms, open_download_file, persist_manifest_snapshot, request_with_retry, reset_download_file, write_all_at};

impl HttpExecutor {
    pub(super) async fn download_single(
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
        // Test-only: remember this download's temp file (by id) so a pipeline
        // fault-injection test can arm a targeted write-failure. Inert in prod.
        #[cfg(any(test, feature = "test-utils"))]
        crate::buffer_pool::fault::register_file(&managed.lock_core().manifest.id, &file);

        // HDD/SSD optimization: set up buffered writing
        let settings = dm.settings().await?;
        let hdd_buffering = settings.io_baseline.hdd_buffer_enabled;
        let ssd_write_combine_mb = settings.io_baseline.ssd_write_combine_mb;
        drop(settings);
        let disk_type = dm.resolve_disk_type(Path::new(&destination_dir)).await;
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
                let chunk_size = {
                    let core = managed.lock_core();
                    core.manifest.chunk_size
                };
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

            let (url, user_agent, extra_headers, validator, state) = {
                let core = managed.lock_core();
                (
                    core.manifest.final_url.clone(),
                    core.manifest.user_agent.clone(),
                    core.manifest.extra_headers.clone(),
                    if_range_header(&core.manifest),
                    core.manifest.state,
                )
            };
            if state == DownloadState::Canceled {
                if let Some(ref buf) = write_buffer
                    && let Err(e) = buf.flush_all().await
                {
                    tracing::warn!("flush on cancel failed: {e}");
                }
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
                    let extra_headers = extra_headers.clone();
                    let validator = validator.clone();
                    async move {
                        let mut builder = apply_extra_headers(
                            client.get(url).header(header::USER_AGENT, user_agent),
                            &extra_headers,
                        );
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
                    // Persist buffered data before exit: `downloaded_bytes` was
                    // already credited for these chunks, so discarding them here
                    // would leave a hole on resume and misfire the Drop canary.
                    if let Some(ref buf) = write_buffer
                        && let Err(e) = buf.flush_all().await
                    {
                        tracing::warn!("flush on cancel failed: {e}");
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
                dm.task_lifecycle
                    .record_progress(&dm, &managed, None, chunk.len() as u64);
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
}
