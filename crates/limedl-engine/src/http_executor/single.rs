//! Single-stream download path.

use bytes::Bytes;
use futures_util::StreamExt;

use super::{
    BatchLimiter, HttpExecutor, MICRO_BATCH_MAX_INTERVAL, ProgressThrottle, RequestBudget,
    build_write_buffer, check_disk_space_periodically, finish_buffer_flush, flush_write_buffer,
    wait_or_stop,
};
use crate::buffer_pool::DownloadBuffer;
use crate::download::{ManagedDownload, RunOutcome, cancellation_outcome, record_durable_bytes};
use crate::error::{DownloadError, Result, io_error_with_path};
use crate::file_ops::{open_download_file, reset_download_file, write_all_at};
use crate::http::{apply_extra_headers, identity_encoding, if_range_header};
use crate::manager_state::ManagerState;
use crate::manifest::contiguous_prefix_end;
use crate::now_ms;
use crate::retry::{jittered_backoff_delay, register_retry_penalty, request_with_retry};
use crate::types::{DiskType, DownloadState};
use reqwest::{Client, StatusCode, header};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

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
        dm: Arc<dyn ManagerState>,
        managed: Arc<ManagedDownload>,
        client: Client,
        token: CancellationToken,
        max_retries: u32,
    ) -> Result<RunOutcome> {
        let (file, total_bytes, disk_type, write_buffer) =
            open_single_target(&dm, &managed).await?;
        let write_buffer = Some(write_buffer);

        let checksum_mode = managed.lock_core().manifest.checksum_mode;
        let initial_offset = {
            let core = managed.lock_core();
            contiguous_prefix_end(&core.manifest)
        };
        let mut hasher = if initial_offset == 0 && checksum_mode != crate::types::ChecksumMode::None
        {
            crate::checksum::ChecksumHasher::new(checksum_mode).ok()
        } else {
            None
        };

        let mut budget = RequestBudget::new(MAX_SINGLE_STREAM_REQUESTS);
        let mut batch = BatchLimiter::new();
        let mut throttle = ProgressThrottle::new();
        let mut last_disk_check = Instant::now();
        let mut hasher_expected_offset: u64 = 0;
        let mut consecutive_stream_errors: u32 = 0;
        let mut last_progress_offset: u64 = 0;

        loop {
            let mut run = SingleRun {
                dm: &dm,
                managed: &managed,
                file: &file,
                write_buffer: &write_buffer,
                disk_type,
                token: &token,
                client: &client,
                max_retries,
                budget: &mut budget,
                batch: &mut batch,
                throttle: &mut throttle,
                last_disk_check: &mut last_disk_check,
                hasher: &mut hasher,
                hasher_expected_offset: &mut hasher_expected_offset,
                consecutive_stream_errors: &mut consecutive_stream_errors,
                last_progress_offset: &mut last_progress_offset,
            };
            if let Some(outcome) = run.attempt(total_bytes).await? {
                return Ok(outcome);
            }
        }
    }
}

/// HTTP/3 is given this long before the request is abandoned and the circuit
/// breaker trips in favour of TCP.
const HTTP3_ATTEMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// The per-request inputs read from the manifest under one lock.
struct RequestHead {
    url: String,
    user_agent: String,
    extra_headers: Vec<String>,
    validator: Option<(header::HeaderName, header::HeaderValue)>,
    state: DownloadState,
}

/// How draining one response body ended.
enum DrainEnd {
    /// The body ended; `Some` carries the stream error that cut it short.
    Ended(Option<reqwest::Error>),
    /// The download was cancelled while draining.
    Cancelled,
}

/// The state one single-stream download threads through every request cycle.
///
/// `download_single` is a chain of small steps over the same handful of
/// mutable values; holding them here lets each step be its own function instead
/// of one deeply nested loop.
struct SingleRun<'a> {
    dm: &'a Arc<dyn ManagerState>,
    managed: &'a Arc<ManagedDownload>,
    file: &'a Arc<fs::File>,
    write_buffer: &'a Option<Arc<DownloadBuffer>>,
    disk_type: DiskType,
    token: &'a CancellationToken,
    client: &'a Client,
    max_retries: u32,
    budget: &'a mut RequestBudget,
    batch: &'a mut BatchLimiter,
    throttle: &'a mut ProgressThrottle,
    last_disk_check: &'a mut Instant,
    hasher: &'a mut Option<crate::checksum::ChecksumHasher>,
    hasher_expected_offset: &'a mut u64,
    consecutive_stream_errors: &'a mut u32,
    last_progress_offset: &'a mut u64,
}

impl SingleRun<'_> {
    /// One request → drain → handle cycle. `Some(outcome)` means the caller must
    /// return it; `None` means loop again.
    async fn attempt(&mut self, total_bytes: Option<u64>) -> Result<Option<RunOutcome>> {
        if let Some(outcome) = wait_or_stop(
            self.dm.as_ref(),
            self.managed,
            self.token,
            self.write_buffer,
        )
        .await
        {
            return Ok(Some(outcome));
        }

        // The stream ends without `single_finished` whenever the server cuts the
        // body short, and the loop resumes from the new offset. A server that
        // always ends it immediately (zero bytes, so the offset never moves)
        // would otherwise spin here forever, one request at a time.
        self.budget.charge("single-stream download")?;

        let head = self.read_request_head();
        if head.state == DownloadState::Canceled {
            flush_write_buffer(self.write_buffer, "cancel").await;
            return Ok(Some(RunOutcome::Canceled));
        }
        if self.token.is_cancelled() {
            return Ok(Some(cancellation_outcome(self.managed)));
        }

        let start_offset = self.contiguous_prefix();
        let response = head.acquire_response(self, start_offset).await?;
        self.dm
            .alt_svc()
            .record_from_response(&head.url, response.headers(), now_ms());

        let status = response.status();
        let mut absolute_offset = single_start_offset(
            self.dm,
            self.managed,
            self.file,
            self.write_buffer,
            status,
            start_offset,
        )
        .await?;
        if absolute_offset < *self.last_progress_offset {
            *self.last_progress_offset = absolute_offset;
        }

        self.mark_downloading();

        match self
            .drain_body(response, total_bytes, &mut absolute_offset)
            .await?
        {
            DrainEnd::Cancelled => Ok(Some(cancellation_outcome(self.managed))),
            DrainEnd::Ended(Some(err)) => self.handle_stream_error(err, absolute_offset).await,
            DrainEnd::Ended(None) => self.finish_stream(total_bytes, absolute_offset).await,
        }
    }

    /// The per-request inputs, taken under one lock.
    fn read_request_head(&self) -> RequestHead {
        let core = self.managed.lock_core();
        RequestHead {
            url: core.manifest.final_url.clone(),
            user_agent: core.manifest.user_agent.clone(),
            extra_headers: core.manifest.extra_headers.clone(),
            validator: if_range_header(&core.manifest),
            state: core.manifest.state,
        }
    }

    /// Where the next request must resume from: the durable contiguous prefix.
    fn contiguous_prefix(&self) -> u64 {
        let core = self.managed.lock_core();
        contiguous_prefix_end(&core.manifest)
    }

    fn plain_request(
        &self,
        head: &RequestHead,
        start_offset: u64,
        use_http3: bool,
    ) -> impl std::future::Future<Output = std::result::Result<reqwest::Response, reqwest::Error>>
    {
        single_request(
            self.client.clone(),
            head.url.clone(),
            head.user_agent.clone(),
            head.extra_headers.clone(),
            start_offset,
            head.validator.clone(),
            use_http3,
        )
    }

    /// This attempt is now downloading: clear whatever stale marker the previous
    /// failure left on the task.
    fn mark_downloading(&self) {
        let mut core = self.managed.lock_core();
        core.snapshot.state = DownloadState::Downloading;
        core.snapshot.connection_count = 1;
        core.snapshot.updated_at_ms = now_ms();
        core.manifest.state = DownloadState::Downloading;
        core.manifest.connection_count = 1;
        core.manifest.updated_at_ms = now_ms();
    }

    /// Drain one response body into the write buffer.
    async fn drain_body(
        &mut self,
        response: reqwest::Response,
        total_bytes: Option<u64>,
        absolute_offset: &mut u64,
    ) -> Result<DrainEnd> {
        const SINGLE_MICRO_BATCH_SIZE: usize = 128 * 1024;
        let mut stream = response.bytes_stream();
        let mut local_buf = Vec::with_capacity(SINGLE_MICRO_BATCH_SIZE);
        let mut batch_start_offset = *absolute_offset;
        let mut last_flush = Instant::now();
        let mut stream_error = None;
        while let Some(chunk) = tokio::select! {
            _ = self.token.cancelled() => {
                if !local_buf.is_empty() {
                    let bytes = Bytes::from(std::mem::take(&mut local_buf));
                    self.write_run_lossy(batch_start_offset, &bytes).await;
                }
                // Flush remaining rate limiter bytes before exiting
                self.batch.flush(self.dm.rate_limiter()).await;
                // Persist buffered data before exit: `downloaded_bytes` was
                // already credited for these chunks, so discarding them here
                // would leave a hole on resume and misfire the Drop canary.
                flush_write_buffer(self.write_buffer, "cancel").await;
                return Ok(DrainEnd::Cancelled);
            }
            chunk = stream.next() => chunk,
        } {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(err) => {
                    stream_error = Some(err);
                    break;
                }
            };
            self.batch
                .account(self.dm.rate_limiter(), chunk.len())
                .await;
            guard_content_length(total_bytes, *absolute_offset, chunk.len())?;
            self.note_hasher(*absolute_offset, &chunk);

            local_buf.extend_from_slice(&chunk);
            *absolute_offset += chunk.len() as u64;

            if local_buf.len() >= SINGLE_MICRO_BATCH_SIZE
                || (!local_buf.is_empty() && last_flush.elapsed() >= MICRO_BATCH_MAX_INTERVAL)
            {
                let bytes = Bytes::from(std::mem::replace(
                    &mut local_buf,
                    Vec::with_capacity(SINGLE_MICRO_BATCH_SIZE),
                ));
                self.flush_run(batch_start_offset, &bytes).await?;
                batch_start_offset = *absolute_offset;
                last_flush = Instant::now();
            }
        }

        if !local_buf.is_empty() {
            let bytes = Bytes::from(std::mem::take(&mut local_buf));
            self.flush_run(batch_start_offset, &bytes).await?;
        }

        // Flush remaining rate limiter bytes after stream ends
        self.batch.flush(self.dm.rate_limiter()).await;

        Ok(DrainEnd::Ended(stream_error))
    }

    /// Feed the streaming hasher. A gap makes it unusable, so it is dropped.
    fn note_hasher(&mut self, offset: u64, chunk: &[u8]) {
        if self.hasher.is_none() {
            return;
        }
        if offset != *self.hasher_expected_offset {
            *self.hasher = None;
            return;
        }
        if let Some(hasher) = self.hasher.as_mut() {
            hasher.update(chunk);
            *self.hasher_expected_offset += chunk.len() as u64;
        }
    }

    /// Write one buffered run and credit its bytes, then run the throttled
    /// progress persist and free-space check.
    async fn flush_run(&mut self, offset: u64, bytes: &Bytes) -> Result<()> {
        write_chunk_bytes(
            self.managed,
            self.file,
            self.write_buffer,
            self.disk_type,
            offset,
            bytes,
        )
        .await?;
        self.dm
            .record_progress(self.managed, None, bytes.len() as u64);
        self.throttle
            .tick(self.dm.db(), self.dm.as_ref(), self.managed)
            .await?;
        check_disk_space_periodically(self.dm.as_ref(), self.managed, self.last_disk_check).await?;
        Ok(())
    }

    /// The cancellation path: a failed write is deliberately swallowed, but the
    /// bytes are still credited — they were already counted as received.
    async fn write_run_lossy(&self, offset: u64, bytes: &Bytes) {
        let _ = write_chunk_bytes(
            self.managed,
            self.file,
            self.write_buffer,
            self.disk_type,
            offset,
            bytes,
        )
        .await;
        self.dm
            .record_progress(self.managed, None, bytes.len() as u64);
    }

    /// A body the server cut short: back off and retry, or give up once the retry
    /// budget is exhausted.
    async fn handle_stream_error(
        &mut self,
        err: reqwest::Error,
        absolute_offset: u64,
    ) -> Result<Option<RunOutcome>> {
        tracing::warn!("Single-stream download interrupted at offset {absolute_offset}: {err}");
        register_retry_penalty(self.managed, err.to_string());
        self.note_interrupted(absolute_offset);
        if *self.consecutive_stream_errors > self.max_retries {
            flush_write_buffer(self.write_buffer, "error").await;
            return Err(DownloadError::from(err));
        }
        let cancelled = tokio::select! {
            _ = self.token.cancelled() => true,
            _ = sleep(jittered_backoff_delay(*self.consecutive_stream_errors)) => false,
        };
        if cancelled {
            flush_write_buffer(self.write_buffer, "cancel").await;
            return Ok(Some(cancellation_outcome(self.managed)));
        }
        Ok(None)
    }

    /// Stream-retry bookkeeping: forward progress restarts the streak at 1, a
    /// stalled stream grows it.
    fn note_interrupted(&mut self, absolute_offset: u64) {
        if absolute_offset > *self.last_progress_offset {
            *self.last_progress_offset = absolute_offset;
            *self.consecutive_stream_errors = 1;
        } else {
            *self.consecutive_stream_errors = self.consecutive_stream_errors.saturating_add(1);
        }
    }

    /// The body ended cleanly: finish when the download is complete, otherwise
    /// loop for the next range.
    async fn finish_stream(
        &mut self,
        total_bytes: Option<u64>,
        absolute_offset: u64,
    ) -> Result<Option<RunOutcome>> {
        *self.consecutive_stream_errors = 0;
        *self.last_progress_offset = absolute_offset;
        if !single_finished(self.managed) {
            return Ok(None);
        }
        finish_buffer_flush(self.dm.as_ref(), self.managed, self.write_buffer).await?;
        self.finalize_checksum(total_bytes);
        Ok(Some(RunOutcome::Finished))
    }

    /// Publish the on-the-fly checksum when the hasher saw every byte.
    fn finalize_checksum(&mut self, total_bytes: Option<u64>) {
        let Some(hasher) = self.hasher.take() else {
            return;
        };
        let is_complete = match total_bytes {
            Some(total) => *self.hasher_expected_offset == total,
            None => true,
        };
        if !is_complete {
            return;
        }
        let computed = hasher.finalize();
        tracing::info!("Single-stream on-the-fly checksum computed: {computed}");
        let mut core = self.managed.lock_core();
        core.manifest.checksum = Some(computed.clone());
        core.snapshot.checksum = Some(computed);
    }
}

impl RequestHead {
    /// Prefer HTTP/3 when the Alt-Svc cache says the origin supports it, then fall
    /// back to the retrying TCP request.
    async fn acquire_response(
        &self,
        run: &SingleRun<'_>,
        start_offset: u64,
    ) -> Result<reqwest::Response> {
        if let Some(response) = self.try_http3(run, start_offset).await {
            return Ok(response);
        }
        request_with_retry(
            || run.plain_request(self, start_offset, false),
            run.token.clone(),
            run.max_retries,
            run.managed.clone(),
        )
        .await
    }

    /// One attempt over HTTP/3. A failure trips the circuit breaker so a later
    /// attempt uses TCP.
    async fn try_http3(&self, run: &SingleRun<'_>, start_offset: u64) -> Option<reqwest::Response> {
        let (proxy_mode, alt_svc_enabled) = {
            let settings = run.dm.settings_service().get_blocking();
            (
                settings.proxy.mode,
                settings.scheduler.http3_alt_svc_enabled,
            )
        };
        let (host, port) = run.dm.alt_svc().should_attempt_http3(
            &self.url,
            proxy_mode,
            alt_svc_enabled,
            now_ms(),
        )?;
        match tokio::time::timeout(
            HTTP3_ATTEMPT_TIMEOUT,
            run.plain_request(self, start_offset, true),
        )
        .await
        {
            Ok(Ok(response)) => {
                run.dm.alt_svc().mark_success(&host, port);
                Some(response)
            }
            Ok(Err(err)) => {
                tracing::warn!(
                    "HTTP/3 single request to {} failed: {err}; tripping circuit breaker and falling back to TCP",
                    self.url
                );
                run.dm.alt_svc().mark_broken(&host, port, now_ms());
                None
            }
            Err(_) => {
                tracing::warn!(
                    "HTTP/3 single request to {} timed out; tripping circuit breaker and falling back to TCP",
                    self.url
                );
                run.dm.alt_svc().mark_broken(&host, port, now_ms());
                None
            }
        }
    }
}

/// Open the temp file and build the write buffer for the single-stream path.
///
/// Also publishes the detected disk type on the snapshot for the UI badge.
async fn open_single_target(
    dm: &Arc<dyn ManagerState>,
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
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| io_error_with_path(e, parent.to_string_lossy()))?;
    }
    let file = Arc::new(open_download_file(&file_path, total_bytes)?);
    // Test-only: remember this download's temp file (by id) so a pipeline
    // fault-injection test can arm a targeted write-failure. Inert in prod.
    #[cfg(any(test, feature = "test-utils"))]
    crate::file_ops::fault::register_file(&managed.lock_core().manifest.id, &file);

    // HDD/SSD optimization: set up buffered writing
    let settings = dm.settings().await?;
    let hdd_buffering = settings.io_baseline.hdd_buffer_enabled;
    let ssd_write_combine_mb = settings.io_baseline.ssd_write_combine_mb;
    drop(settings);
    let disk_type = dm.resolve_disk_type(Path::new(&destination_dir)).await;
    let write_buffer = build_write_buffer(
        dm.as_ref(),
        managed,
        &file,
        disk_type,
        hdd_buffering,
        ssd_write_combine_mb,
    )
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
    use_http3: bool,
) -> std::result::Result<reqwest::Response, reqwest::Error> {
    let mut builder = apply_extra_headers(
        client.get(url).header(header::USER_AGENT, user_agent),
        &extra_headers,
    );
    if use_http3 {
        builder = builder.version(reqwest::Version::HTTP_3);
    }
    if start_offset > 0 {
        // Resuming sends `Range`: a compressed 206 would shift byte offsets, so
        // this request must not negotiate compression even though the fresh
        // (offset 0) GET does.
        builder =
            identity_encoding(builder.header(header::RANGE, format!("bytes={start_offset}-")));
        if let Some((name, value)) = validator {
            builder = builder.header(name, value);
        }
    }
    builder.send().await
}

/// Decide the starting offset for this attempt; a non-range response to a
/// resume request resets the file and progress.
async fn single_start_offset(
    dm: &Arc<dyn ManagerState>,
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
        dm.reset_progress(managed, true);
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
