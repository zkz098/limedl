//! Integration tests for retry.rs (retry/backoff logic) and CDN fallback scenarios.
//!
//! # Test structure
//!
//! - [`backoff_delay`] unit tests (synchronous) — verify the pure delay function.
//! - [`request_with_retry`] integration tests (async) — verify retry flow using
//!   in-memory `reqwest::Response` objects (no real network I/O).
//!
//! # CDN fallback note
//!
//! CDN fallback testing (switching to the next fastest node when a CDN node fails)
//! requires mocking the entire probing/speed-test pipeline in `CdnAccelerator::start_test()`,
//! which currently calls real Cloudflare IP probing, DNS resolution, and uses
//! `tokio::spawn`.  There is no injection point for test doubles.
//! These scenarios are marked as E2E-only — see the comments at the end of this file.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use ntest::timeout;
use parking_lot::Mutex as ParkingMutex;
use reqwest::Body;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::aimd::AimdState;
use crate::error::DownloadError;
use crate::manager::DownloadCore;
use crate::manifest::Manifest;
use crate::retry::{backoff_delay, request_with_retry};
use crate::types::{ChecksumMode, DownloadSnapshot, DownloadState, TaskKind, ThreadMode};

// ---------------------------------------------------------------------------
// Type alias
// ---------------------------------------------------------------------------

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a [`reqwest::Response`] with the given HTTP status and no body.
///
/// This is the same technique used in `http.rs` tests — constructing an
/// `http::Response<Body>` and converting it into a `reqwest::Response`.
fn make_response(status: u16) -> reqwest::Response {
    let status_code = StatusCode::from_u16(status).unwrap();
    let http_resp = axum::http::Response::builder()
        .status(status_code)
        .body(Body::from(String::new()))
        .unwrap();
    reqwest::Response::from(http_resp)
}

/// Create a minimal [`ManagedDownload`] for retry tests.
///
/// All fields are initialised to sensible zero / default values.  The download
/// starts in [`DownloadState::Downloading`] with zero progress.
fn make_managed() -> Arc<crate::manager::ManagedDownload> {
    Arc::new(crate::manager::ManagedDownload {
        core: ParkingMutex::new(DownloadCore {
            snapshot: DownloadSnapshot {
                id: String::new(),
                kind: TaskKind::Http,
                state: DownloadState::Downloading,
                url: String::new(),
                final_url: String::new(),
                file_name: String::new(),
                destination_path: String::new(),
                temp_path: String::new(),
                total_bytes: None,
                downloaded_bytes: 0,
                supports_ranges: false,
                connection_count: 0,
                thread_mode: ThreadMode::Adaptive,
                requested_thread_count: None,
                desired_thread_count: None,
                allocated_thread_count: None,
                adaptive_profile: None,
                thread_note: None,
                checksum: None,
                checksum_mode: ChecksumMode::None,
                etag: None,
                last_modified: None,
                error: None,
                speed_bytes_per_second: None,
                eta_seconds: None,
                uploaded_bytes: None,
                upload_speed_bytes_per_second: None,
                peer_count: None,
                upload_status: None,
                info_hash: None,
                created_at_ms: 0,
                updated_at_ms: 0,
                cdn_accelerated: false,
                chunks: vec![],
                seed_count: None,
                leech_count: None,
                download_limit_bps: None,
                upload_limit_bps: None,
                mirror_url: None,
                degraded: false,
                disk_type: None,
                flushing: false,
            },
            manifest: Manifest {
                id: String::new(),
                url: String::new(),
                final_url: String::new(),
                user_agent: "test".into(),
                destination_dir: String::new(),
                file_name: String::new(),
                file_name_locked: false,
                destination_path: String::new(),
                temp_path: String::new(),
                total_bytes: None,
                downloaded_bytes: 0,
                supports_ranges: false,
                chunk_size: 4_194_304,
                connection_count: 0,
                thread_mode: ThreadMode::Adaptive,
                requested_thread_count: None,
                desired_thread_count: None,
                allocated_thread_count: None,
                adaptive_profile_snapshot: None,
                thread_note: None,
                etag: None,
                last_modified: None,
                state: DownloadState::Downloading,
                cdn_accelerated: false,
                checksum_mode: ChecksumMode::None,
                checksum: None,
                expected_checksum: None,
                error: None,
                created_at_ms: 0,
                updated_at_ms: 0,
                mirror_url: None,
                mirror_urls: vec![],
                current_mirror_index: 0,
                chunks: vec![],
            },
        }),
        runtime: ParkingMutex::new(None),
        aimd: ParkingMutex::new(AimdState::default()),
        stop_notify: Notify::new(),
    })
}

// ---------------------------------------------------------------------------
// backoff_delay  —  pure function unit tests
// ---------------------------------------------------------------------------

#[test]
fn backoff_delay_attempt_1_is_500ms() {
    assert_eq!(backoff_delay(1), Duration::from_millis(500));
}

#[test]
fn backoff_delay_attempt_2_is_1s() {
    assert_eq!(backoff_delay(2), Duration::from_millis(1000));
}

#[test]
fn backoff_delay_attempt_3_is_2s() {
    assert_eq!(backoff_delay(3), Duration::from_millis(2000));
}

#[test]
fn backoff_delay_attempt_4_is_4s() {
    assert_eq!(backoff_delay(4), Duration::from_millis(4000));
}

#[test]
fn backoff_delay_capped_at_4s() {
    // Attempts >= 4 are all capped at 4 seconds.
    assert_eq!(backoff_delay(5), Duration::from_millis(4000));
    assert_eq!(backoff_delay(10), Duration::from_millis(4000));
    assert_eq!(backoff_delay(100), Duration::from_millis(4000));
}

#[test]
fn backoff_delay_attempt_0_is_250ms() {
    // attempt=0 means "first request", but the delay function still computes.
    // 250 * 2^0 = 250ms.
    assert_eq!(backoff_delay(0), Duration::from_millis(250));
}

// ---------------------------------------------------------------------------
// request_with_retry  —  async integration tests (in-memory responses)
// ---------------------------------------------------------------------------

/// Verify that backoff intervals follow the expected exponential pattern
/// (500ms → 1s → 2s → 4s) when a retryable status (503) is returned.
///
/// Uses real wall-clock time with a generous tolerance to account for CI
/// variability.  Each sleep is small (max ~2 s) so the whole test completes
/// within a few seconds.
#[tokio::test]
#[timeout(120_000)]
async fn retry_exponential_backoff_timing() -> TestResult {
    let call_times = Arc::new(Mutex::new(Vec::<Instant>::new()));
    let ct = call_times.clone();

    let factory = move || {
        ct.lock().unwrap().push(Instant::now());
        async { Ok(make_response(503)) }
    };

    let managed = make_managed();
    let token = CancellationToken::new();

    // max_retries = 3 → 1 initial attempt + 3 retries = 4 factory calls
    let result = request_with_retry(factory, token, 3, managed).await;
    assert!(result.is_err(), "request_with_retry should exhaust retries");

    let times = call_times.lock().unwrap();
    assert_eq!(times.len(), 4, "expected 1 initial + 3 retries = 4 calls");

    // Intervals follow: ~500ms, ~1000ms, ~2000ms
    let intervals: Vec<Duration> = times
        .windows(2)
        .map(|w| {
            if w[1] > w[0] {
                w[1] - w[0]
            } else {
                Duration::ZERO
            }
        })
        .collect();

    let tol = Duration::from_millis(400);

    let i0 = intervals[0].as_millis() as i64;
    assert!(
        (i0 - 500).abs() <= tol.as_millis() as i64,
        "interval 0 expected ~500ms, got {:?}",
        intervals[0]
    );

    let i1 = intervals[1].as_millis() as i64;
    assert!(
        (i1 - 1000).abs() <= tol.as_millis() as i64,
        "interval 1 expected ~1000ms, got {:?}",
        intervals[1]
    );

    let i2 = intervals[2].as_millis() as i64;
    assert!(
        (i2 - 2000).abs() <= tol.as_millis() as i64,
        "interval 2 expected ~2000ms, got {:?}",
        intervals[2]
    );

    Ok(())
}

/// With `max_retries = 2`, all requests return 503.  After the initial attempt
/// and 2 retries, the function should return an `InvalidResponse` error.
#[tokio::test]
#[timeout(120_000)]
async fn retry_max_attempts_exhausted() -> TestResult {
    let call_count = Arc::new(Mutex::new(0u32));
    let cc = call_count.clone();

    let factory = move || {
        *cc.lock().unwrap() += 1;
        async { Ok(make_response(503)) }
    };

    let managed = make_managed();
    let token = CancellationToken::new();

    let result = request_with_retry(factory, token, 2, managed).await;

    assert!(result.is_err(), "expected error after exhausting retries");
    match result.unwrap_err() {
        DownloadError::InvalidResponse(msg) => {
            assert!(msg.contains("503"), "error should mention status code, got: {msg}");
        }
        other => panic!("expected InvalidResponse, got: {other}"),
    }

    // 1 initial attempt + 2 retries = 3 total calls
    assert_eq!(*call_count.lock().unwrap(), 3);

    Ok(())
}

/// The first two requests return 503 (retryable), the third returns 200.
/// The function should succeed after 2 retries.
#[tokio::test]
#[timeout(120_000)]
async fn retry_succeeds_on_third_attempt() -> TestResult {
    let call_count = Arc::new(Mutex::new(0u32));
    let cc = call_count.clone();

    let factory = move || {
        let mut count = cc.lock().unwrap();
        *count += 1;
        // Fail first 2 attempts, succeed on 3rd.
        let status = if *count <= 2 { 503 } else { 200 };
        async move { Ok(make_response(status)) }
    };

    let managed = make_managed();
    let token = CancellationToken::new();

    let result = request_with_retry(factory, token, 5, managed).await;

    assert!(result.is_ok(), "expected success on third attempt");
    let response = result?;
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    assert_eq!(*call_count.lock().unwrap(), 3);

    Ok(())
}

/// Client-error status codes (4xx) must NOT be retried — they are classified as
/// [`ResponseDisposition::Invalid`] and cause an immediate error return.
///
/// Server-error codes (5xx) ARE retryable.
#[tokio::test]
#[timeout(60_000)]
async fn retry_respects_transient_vs_permanent_errors() -> TestResult {
    // ── 4xx is NOT retried ───────────────────────────────────
    let call_count = Arc::new(Mutex::new(0u32));
    let cc = call_count.clone();

    let factory = move || {
        *cc.lock().unwrap() += 1;
        async { Ok(make_response(404)) }
    };

    let managed = make_managed();
    let token = CancellationToken::new();

    let result = request_with_retry(factory, token, 5, managed).await;

    assert!(result.is_err(), "expected error for 404");
    match result.unwrap_err() {
        DownloadError::InvalidResponse(msg) => {
            assert!(msg.contains("404"), "error should mention 404, got: {msg}");
        }
        other => panic!("expected InvalidResponse, got: {other}"),
    }

    // Factory called only once — no retries for client errors
    assert_eq!(*call_count.lock().unwrap(), 1);

    Ok(())
}

// ---------------------------------------------------------------------------
// CDN fallback documentation note
// ---------------------------------------------------------------------------
//
// CDN fallback scenarios that cannot be unit-tested in the current architecture:
//
// 1. `cdn_fallback_to_next_node_on_failure` — Verify that when the active CDN
//    node returns a truncated/malformed response, the system falls back to the
//    next fastest candidate.
//
// 2. `cdn_delayed_node_deprioritized` — Verify that a node with high latency
//    is not selected when a faster node is available.
//
// Why these are E2E-only:
//
// - `CdnAccelerator::start_test()` calls `get_ip_ranges()` (fetches Cloudflare
//   IP ranges from the network), `run_speed_test()` (probes candidate IPs), and
//   `measure_default_node()` (measures direct download speed).  All three
//   perform real network I/O.
// - The background task is spawned via `tokio::spawn`, which
//   requires a running Tauri runtime.
// - There is no trait / dependency-injection point for substituting fake
//   implementations of IP-range fetching or speed testing.
//
// To enable unit testing, the speed-test and IP-range modules would need to
// accept injectable strategies (e.g., a `SpeedTestStrategy` trait and an
// `IpRangeProvider` trait) so that tests can provide controlled results without
// touching the network.
//
// Existing unit tests in `accelerator.rs` already cover the lifecycle (new,
// apply_ip, clear, cancel, candidate storage) which is the most we can test
// without mocking infrastructure.
