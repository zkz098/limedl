//! Retry/backoff logic for HTTP downloads.
//!
//! Extracted from `manager.rs` to reduce the god object. Contains:
//! - `request_with_retry()` — wraps HTTP requests with retry logic and exponential backoff
//! - `register_retry_penalty()` — records a penalty in the AIMD state after a retry failure
//! - `backoff_delay()` — computes the base delay for retry attempts
//! - `jittered_backoff_delay()` — the delay actually slept, spread by up to 25%

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Response, StatusCode};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use super::error::{DownloadError, Result};
use super::http::{
    ANTI_ABUSE_SNIFF_LIMIT, ResponseDisposition, anti_abuse_forbidden_error,
    classify_download_response, looks_like_anti_abuse_page, read_body_prefix,
};
use super::download::ManagedDownload;
use super::now_ms;
use super::types::DownloadState;

/// Wraps an HTTP request factory with retry logic and exponential backoff.
///
/// Calls `factory()` to produce each request attempt. On retryable responses
/// (timeout, rate-limit, server errors) or transport errors, sleeps with
/// exponential backoff and retries up to `max_retries` attempts.
/// On cancellation via `token`, returns immediately with `DownloadError::Interrupted`.
pub async fn request_with_retry<F, Fut>(
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
            Ok(response) => {
                if let Some(response) =
                    process_response_attempt(response, &token, max_retries, &managed, &mut attempt)
                        .await?
                {
                    return Ok(response);
                }
            }
            Err(error) => {
                retry_transport_error(&token, max_retries, &managed, &mut attempt, error).await?;
            }
        }
    }
}

/// Classify one successful request exchange.
///
/// Returns `Some(response)` when the caller should hand it back, `None` after
/// a retryable response was penalised and backed off, or `Err` for terminal
/// failures (anti-abuse 403, invalid status, exhausted retries, cancellation).
async fn process_response_attempt(
    mut response: Response,
    token: &CancellationToken,
    max_retries: u32,
    managed: &Arc<ManagedDownload>,
    attempt: &mut u32,
) -> Result<Option<Response>> {
    // A 403 from a WAF / mirror anti-abuse page is terminal and no
    // Referer can fix it. Sniff the (small) body once so the user
    // gets an actionable error instead of a bare status code.
    if response.status() == StatusCode::FORBIDDEN {
        let prefix = read_body_prefix(&mut response, ANTI_ABUSE_SNIFF_LIMIT).await;
        if looks_like_anti_abuse_page(&prefix) {
            return Err(anti_abuse_forbidden_error());
        }
    }

    match classify_download_response(response) {
        ResponseDisposition::Use(response) => Ok(Some(response)),
        ResponseDisposition::Retryable(status) => {
            if rate_limit_aborts(managed, Some(status)) {
                return Err(DownloadError::InvalidResponse(format!(
                    "http status {status}"
                )));
            }
            if *attempt >= max_retries {
                return Err(DownloadError::InvalidResponse(format!(
                    "http status {status}"
                )));
            }
            *attempt += 1;
            register_retry_penalty(managed, format!("http status {status}"));
            backoff_or_cancel(token, *attempt).await?;
            Ok(None)
        }
        ResponseDisposition::Invalid(status) => {
            Err(DownloadError::InvalidResponse(format!(
                "http status {status}"
            )))
        }
    }
}

/// Register a transport error's penalty and back off, or fail terminally.
async fn retry_transport_error(
    token: &CancellationToken,
    max_retries: u32,
    managed: &Arc<ManagedDownload>,
    attempt: &mut u32,
    error: reqwest::Error,
) -> Result<()> {
    if rate_limit_aborts(managed, error.status()) {
        return Err(error.into());
    }
    if *attempt >= max_retries {
        return Err(error.into());
    }
    *attempt += 1;
    register_retry_penalty(managed, error.to_string());
    backoff_or_cancel(token, *attempt).await
}

/// `true` when a 429 must abort instead of retrying: concurrency above one
/// triggers the chunked path's downgrade-to-single-thread instead.
fn rate_limit_aborts(managed: &Arc<ManagedDownload>, status: Option<StatusCode>) -> bool {
    status == Some(StatusCode::TOO_MANY_REQUESTS)
        && managed
            .lock_core()
            .manifest
            .allocated_thread_count
            .unwrap_or(1)
            > 1
}

/// Sleep the exponential backoff for `attempt`, aborting on cancellation.
async fn backoff_or_cancel(token: &CancellationToken, attempt: u32) -> Result<()> {
    tokio::select! {
        _ = token.cancelled() => Err(DownloadError::Interrupted),
        _ = sleep(jittered_backoff_delay(attempt)) => Ok(()),
    }
}

/// Records a retry penalty on a managed download.
///
/// Sets the snapshot and manifest state to `Retrying`, records the error
/// message, and marks a penalty on the AIMD state for backpressure on
/// connection concurrency.
fn register_retry_penalty(managed: &Arc<ManagedDownload>, error: String) {
    {
        let mut core = managed.lock_core();
        core.snapshot.state = DownloadState::Retrying;
        core.snapshot.error = Some(error.clone());
        core.snapshot.updated_at_ms = now_ms();
        core.manifest.state = DownloadState::Retrying;
        core.manifest.error = Some(error);
        core.manifest.updated_at_ms = now_ms();
    }
    let mut aimd = managed.lock_aimd();
    aimd.recent_penalty = true;
    aimd.penalty_count = aimd.penalty_count.saturating_add(1);
}

/// Computes an exponential backoff delay for the given retry attempt.
///
/// Formula: `250ms * 2^min(attempt, 4)`, capped at a 4-second delay
/// (attempt 4+). This gives: 500ms, 1s, 2s, 4s, 4s, ...
///
/// Deterministic on purpose: it is the base [`jittered_backoff_delay`] spreads,
/// and it is what the unit tests pin.
pub fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis((250_u64).saturating_mul(2_u64.saturating_pow(attempt.min(4))))
}

/// Upper bound of the random jitter added on top of [`backoff_delay`], as a
/// percentage of the base delay.
const BACKOFF_JITTER_PERCENT: u64 = 25;

/// [`backoff_delay`] plus uniform jitter in `[base, base * 1.25]`.
///
/// Every chunk of every task retries against the same host, so an unjittered
/// schedule puts them all back on the wire at the same instant — exactly in the
/// 429/5xx case where the extra load is least welcome. The delay never drops
/// below the base value, so the retry policy stays as gentle as before.
pub fn jittered_backoff_delay(attempt: u32) -> Duration {
    let base = backoff_delay(attempt).as_millis() as u64;
    let span = base * BACKOFF_JITTER_PERCENT / 100;
    if span == 0 {
        return Duration::from_millis(base);
    }
    Duration::from_millis(base + rand::random_range(0..=span))
}
