//! Single-stream download path.

use futures_util::StreamExt;

use super::{Arc, BatchLimiter, CancellationToken, Client, DiskType, DownloadError, DownloadManager, DownloadState, HttpExecutor, Instant, ManagedDownload, Path, PathBuf, ProgressThrottle, Result, RunOutcome, StatusCode, apply_extra_headers, build_write_buffer, cancellation_outcome, check_disk_space_periodically, contiguous_prefix_end, flush_write_buffer, fs, header, if_range_header, io_error_with_path, now_ms, open_download_file, request_with_retry, reset_download_file, write_all_at};

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
        let write_buffer = Some(
            build_write_buffer(
                &dm,
                &managed,
                &file,
                disk_type,
                hdd_buffering,
                ssd_write_combine_mb,
            )
            .await,
        ); // always Some — SSD uses ping-pong buffer for write combining

        // Set disk_type on snapshot for frontend badge display
        {
            let mut core = managed.lock_core();
            core.snapshot.disk_type = Some(disk_type);
        }

        let mut last_disk_check = Instant::now();
        let mut throttle = ProgressThrottle::new();
        let mut batch = BatchLimiter::new();

        loop {
            match dm
                .task_lifecycle
                .wait_until_active(&dm, &managed, &token)
                .await
            {
                crate::download::WaitState::Running => {}
                crate::download::WaitState::Paused => {
                    flush_write_buffer(&write_buffer, "pause").await;
                    return Ok(RunOutcome::Paused);
                }
                crate::download::WaitState::Canceled => {
                    flush_write_buffer(&write_buffer, "cancel").await;
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
                flush_write_buffer(&write_buffer, "cancel").await;
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
                    batch.flush(&dm.rate_limiter).await;
                    // Persist buffered data before exit: `downloaded_bytes` was
                    // already credited for these chunks, so discarding them here
                    // would leave a hole on resume and misfire the Drop canary.
                    flush_write_buffer(&write_buffer, "cancel").await;
                    return Ok(cancellation_outcome(&managed));
                }
                chunk = stream.next() => chunk,
            } {
                let chunk = chunk?;
                batch.account(&dm.rate_limiter, chunk.len()).await;
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
                throttle.tick(&dm.db, &dm, &managed).await?;
                check_disk_space_periodically(&dm, &managed, &mut last_disk_check).await?;
            }

            // Flush remaining rate limiter bytes after stream ends
            batch.flush(&dm.rate_limiter).await;

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
