use std::sync::Arc;
use std::time::Duration;

use ntest::timeout;
use tempfile::tempdir;
use tokio::time::sleep;

use crate::DownloadManager;
use crate::event_bus::EventBus;
use crate::rate_limiter::RateLimiter;
use crate::test_harness::TestServer;
use crate::types::{
    AppSettings, ChecksumMode, DownloadState, SchedulerSettings, StartDownloadRequest, ThreadMode,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

async fn wait_for_terminal(manager: &DownloadManager, id: &str) -> crate::types::DownloadSnapshot {
    loop {
        let status = manager.status(id).await.unwrap();
        if matches!(
            status.state,
            DownloadState::Completed | DownloadState::Failed | DownloadState::Canceled
        ) {
            return status;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

fn generate_test_content(size: u64) -> Vec<u8> {
    use rand::Rng;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    let mut rng = StdRng::seed_from_u64(42);
    let mut data = vec![0u8; size as usize];
    rng.fill_bytes(&mut data);
    data
}

// ==========================================================================
// Single-stream download tests (using /file - no Accept-Ranges)
// ==========================================================================

// ==========================================================================
// Multi-stream (range-based) download tests (using /file/range)
// ==========================================================================

// ==========================================================================
// Edge-case tests
// ==========================================================================

// ── Redirect handling ──────────────────────────────────────────────────────

// ── HTTP 416 Range Not Satisfiable ────────────────────────────────────────

// ── Connection refused (fast failure) ─────────────────────────────────────

// ── No Content-Length (chunked transfer encoding) ─────────────────────────

// ── Content-Length mismatch ───────────────────────────────────────────────

// ── Tests / features not covered ──────────────────────────────────────────
//
// 1. gzip / deflate content-encoding
//    reqwest is built with `default-features = false` (gzip/brotli/deflate
//    features NOT enabled).  Responses with `Content-Encoding: gzip` would
//    NOT be decompressed transparently — the downloader would save the raw
//    compressed bytes to disk.  A test for this requires either enabling
//    the `gzip` feature on reqwest or manually decompressing.
//    TODO: Enable reqwest gzip decompression in `configure_client_builder`
//          and add a gzip endpoint + test.
//
// 2. Connection / read timeout
//    The HTTP client has a 15-second read timeout.  Testing it requires a
//    delayed endpoint (>15 s), making the test too slow for routine runs.
//    TODO: Add a timeout test with a custom short-lived client if the
//    timeout configuration becomes user-adjustable.
//
// 3. Proxy support
//    The codebase has proxy support via `AppSettings.proxy` but testing
//    requires a mock proxy server (additional infrastructure).  This is
//    better tested at the `http_client_factory` or integration level.
//    TODO: Add proxy tests when a mock proxy fixture is available.

// ==========================================================================
// GitHub release-asset endpoint (self-update download path)
//
// Regression tests for the self-update signature failure:
// `https://api.github.com/.../releases/assets/<id>` only redirects to the real
// artifact when the request carries `Accept: application/octet-stream`; without
// it GitHub returns the asset's JSON metadata, which the updater would save as
// the "installer" and then reject at minisign verification.
// ==========================================================================

mod chunk_workers;
mod http_errors;
mod mirrors_and_abuse;
mod multi_stream;
mod rate_limit;
mod single_stream;
