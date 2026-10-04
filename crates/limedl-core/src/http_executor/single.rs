//! Single-stream download path.

use bytes::Bytes;
use futures_util::StreamExt;

use super::{Arc, BatchLimiter, CancellationToken, Client, DiskType, DownloadBuffer, DownloadError, DownloadManager, DownloadState, HttpExecutor, Instant, ManagedDownload, Path, PathBuf, ProgressThrottle, RequestBudget, Result, RunOutcome, StatusCode, apply_extra_headers, build_write_buffer, cancellation_outcome, check_disk_space_periodically, contiguous_prefix_end, finish_buffer_flush, flush_write_buffer, fs, header, if_range_header, io_error_with_path, now_ms, open_download_file, record_durable_bytes, request_with_retry, reset_download_file, wait_or_stop, write_all_at};

/// Upper bound on the number of responses one single-stream download may consume.
///
/// Each response may itself retry up to the configured `max_retries`, so this is
/// what keeps a server that always ends the body early from producing an
/// unbounded request loop. A response that makes no progress at all is exactly
/// the case the limit exists for; a legitimate resumption needs at most a few.
const MAX_SINGLE_STREAM_REQUESTS: u32 = 64;

impl HttpExecutor {
    pub(super) async fn download_single(
        &self,
        dm: Arc<DownloadManager>,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<RunOutcome> {
        let (file, total_bytes, disk_type, write_buffer) = open_single_target(&dm, &managed).await?;
        let write_buffer = Some(write_buffer);

        let mut last_disk_check = Instant::now();
        let mut throttle = ProgressThrottle::new();
        let mut batch = BatchLimiter::new();
        let mut budget = RequestBudget::new(MAX_SINGLE_STREAM_REQUESTS);

        loop {
            if let Some(outcome) = wait_or_stop(&dm, &managed, &token, &write_buffer).await {
                return Ok(outcome);
            }

            // The stream ends without `single_finished` whenever the server cuts
            // the body short, and the loop resumes from the new offset. A server
            // that always ends it immediately (zero bytes, so the offset never
            // moves) would otherwise spin here forever, one request at a time.
            budget.charge("single-stream download")?;

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
                    single_request(
                        client.clone(),
                        url.clone(),
                        user_agent.clone(),
                        extra_headers.clone(),
                        start_offset,
                        validator.clone(),
                    )
                },
                token.clone(),
                max_retries,
                managed.clone(),
            )
            .await?;

            let status = response.status();
            let mut stream = response.bytes_stream();
            let mut absolute_offset =
                single_start_offset(&dm, &managed, &file, &write_buffer, status, start_offset)
                    .await?;

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
                guard_content_length(total_bytes, absolute_offset, chunk.len())?;
                write_chunk_bytes(
                    &managed,
                    &file,
                    &write_buffer,
                    disk_type,
                    absolute_offset,
                    &chunk,
                )
                .await?;
                dm.task_lifecycle
                    .record_progress(&dm, &managed, None, chunk.len() as u64);
                absolute_offset += chunk.len() as u64;
                throttle.tick(&dm.db, &dm, &managed).await?;
                check_disk_space_periodically(&dm, &managed, &mut last_disk_check).await?;
            }

            // Flush remaining rate limiter bytes after stream ends
            batch.flush(&dm.rate_limiter).await;

            if single_finished(&managed) {
                finish_buffer_flush(&dm, &managed, &write_buffer).await?;
                return Ok(RunOutcome::Finished);
            }
        }
    }
}

/// Open the temp file and build the write buffer for the single-stream path.
///
/// Also publishes the detected disk type on the snapshot for the UI badge.
async fn open_single_target(
    dm: &Arc<DownloadManager>,
    managed: &Arc<ManagedDownload>,
) -> Result<(Arc<fs::File>, Option<u64>, DiskType, Arc<DownloadBuffer>)> {
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
    let write_buffer =
        build_write_buffer(dm, managed, &file, disk_type, hdd_buffering, ssd_write_combine_mb)
            .await;

    // Set disk_type on snapshot for frontend badge display
    {
        let mut core = managed.lock_core();
        core.snapshot.disk_type = Some(disk_type);
    }

    Ok((file, total_bytes, disk_type, write_buffer))
}

/// Build one single-stream GET (with a resume Range when starting mid-file).
async fn single_request(
    client: Client,
    url: String,
    user_agent: String,
    extra_headers: Vec<String>,
    start_offset: u64,
    validator: Option<(header::HeaderName, header::HeaderValue)>,
) -> std::result::Result<reqwest::Response, reqwest::Error> {
    let mut builder = apply_extra_headers(
        client.get(url).header(header::USER_AGENT, user_agent),
        &extra_headers,
    );
    if start_offset > 0 {
        builder = builder.header(header::RANGE, format!("bytes={start_offset}-"));
        if let Some((name, value)) = validator {
            builder = builder.header(name, value);
        }
    }
    builder.send().await
}

/// Decide the starting offset for this attempt; a non-range response to a
/// resume request resets the file and progress.
async fn single_start_offset(
    dm: &Arc<DownloadManager>,
    managed: &Arc<ManagedDownload>,
    file: &Arc<fs::File>,
    write_buffer: &Option<Arc<DownloadBuffer>>,
    status: StatusCode,
    start_offset: u64,
) -> Result<u64> {
    if status == StatusCode::PARTIAL_CONTENT && start_offset > 0 {
        return Ok(start_offset);
    }
    if start_offset > 0 {
        reset_download_file(file, managed.lock_core().manifest.total_bytes)?;
        dm.task_lifecycle.reset_progress(dm, managed, true);
        if let Some(buf) = write_buffer {
            buf.clear();
        }
    }
    Ok(0)
}

/// Guard against the server sending more data than Content-Length.
fn guard_content_length(total_bytes: Option<u64>, offset: u64, len: usize) -> Result<()> {
    if let Some(total) = total_bytes {
        let len = len as u64;
        if offset + len > total {
            return Err(DownloadError::InvalidResponse(format!(
                "server sent more data than Content-Length ({} bytes received, expected {total})",
                offset + len
            )));
        }
    }
    Ok(())
}

/// Write one stream chunk through the buffer (with direct-write fallback).
async fn write_chunk_bytes(
    managed: &Arc<ManagedDownload>,
    file: &Arc<fs::File>,
    write_buffer: &Option<Arc<DownloadBuffer>>,
    disk_type: DiskType,
    offset: u64,
    chunk: &Bytes,
) -> Result<()> {
    if let Some(buf) = write_buffer {
        if buf.buffer_chunk(offset, chunk.clone()).await.is_err() {
            // Background flush failed — fall back to direct write. The bytes are
            // on the file as soon as `write_all_at` returns, so they are durable
            // immediately; the buffer never got to report them.
            write_all_at(file, chunk, offset)?;
            record_durable_bytes(managed, &[(offset, chunk.len() as u64)]);
            if disk_type == DiskType::Hdd {
                let mut core = managed.lock_core();
                core.snapshot.degraded = true;
            }
        }
    } else {
        write_all_at(file, chunk, offset)?;
        record_durable_bytes(managed, &[(offset, chunk.len() as u64)]);
    }
    Ok(())
}

/// Whether the stream is complete for the manifest's known total size.
fn single_finished(managed: &Arc<ManagedDownload>) -> bool {
    let core = managed.lock_core();
    match core.manifest.total_bytes {
        Some(total) => core.manifest.downloaded_bytes >= total,
        None => true,
    }
}
