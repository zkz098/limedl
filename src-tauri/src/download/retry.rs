//! Retry/backoff logic for HTTP downloads.
//!
//! Extracted from `manager.rs` to reduce the god object. Contains:
//! - `request_with_retry()` — wraps HTTP requests with retry logic and exponential backoff
//! - `register_retry_penalty()` — records a penalty in the AIMD state after a retry failure
//! - `backoff_delay()` — computes the delay duration for retry attempts

use std::sync::Arc;
use std::time::Duration;

use reqwest::Response;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use super::error::{DownloadError, Result};
use super::http::{ResponseDisposition, classify_download_response};
use super::manager::ManagedDownload;
use super::now_ms;
use super::types::DownloadState;

/// Wraps an HTTP request factory with retry logic and exponential backoff.
///
/// Calls `factory()` to produce each request attempt. On retryable responses
/// (timeout, rate-limit, server errors) or transport errors, sleeps with
/// exponential backoff and retries up to `max_retries` attempts.
/// On cancellation via `token`, returns immediately with `DownloadError::Interrupted`.
pub(crate) async fn request_with_retry<F, Fut>(
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
                    register_retry_penalty(&managed, format!("http status {status}"));
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
                register_retry_penalty(&managed, error.to_string());
                tokio::select! {
                    _ = token.cancelled() => return Err(DownloadError::Interrupted),
                    _ = sleep(backoff_delay(attempt)) => {}
                }
            }
        }
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
fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis((250_u64).saturating_mul(2_u64.saturating_pow(attempt.min(4))))
}
