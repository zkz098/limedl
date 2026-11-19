//! Reusable test harness for download integration tests.
//! Provides a local HTTP server serving deterministic random content
//! with known checksums, range request support, bandwidth simulation,
//! and failure injection endpoints.
//!
//! # Example
//!
//! ```ignore
//! use limedl_testkit::TestServer;
//!
//! let server = TestServer::new(1024 * 1024).await;
//! let url = server.file_url();
//! // use url in a download request ...
//! assert_eq!(downloaded.len() as u64, server.file_size);
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use axum::{Router, routing::get, serve};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use limedl_disk::{buffer_pool, io_scheduler};
/// The layers this harness drives, imported privately so the moved module kept
/// its `super::checksum::…` spellings.
use limedl_types::{checksum, types};

pub mod bench;
mod faults;
mod handlers;

use faults::*;
use handlers::*;

pub use bench::BenchHarness;

use crate::checksum::hash_slices;
use crate::types::ChecksumMode;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Test server that serves deterministic random bytes with known checksums.
///
/// Content is generated from a fixed seed (42), so every instance with the
/// same `size` produces identical bytes — useful for reproducible tests.
pub struct TestServer {
    /// The listen address (e.g. `"http://127.0.0.1:PORT"`).
    pub addr: String,
    /// Total file size in bytes.
    pub file_size: u64,
    /// Pre-computed Blake3 hex checksum of the full content.
    pub blake3_hash: String,
    /// Pre-computed SHA-256 hex checksum of the full content.
    pub sha256_hash: String,
    /// Pre-computed SHA-512 hex checksum of the full content.
    pub sha512_hash: String,
    /// The deterministic bytes every non-fault endpoint serves.
    content: Arc<Vec<u8>>,
    /// Dropping this triggers graceful server shutdown.
    _shutdown: tokio::sync::oneshot::Sender<()>,
    /// How many responses the encoded endpoints actually compressed. Lets a
    /// test prove compression was exercised (or, for range downloads, that the
    /// engine never asked for it).
    pub(crate) encoded_responses: Arc<AtomicUsize>,
}

impl TestServer {
    /// Start a server serving `size` bytes of deterministic random content.
    ///
    /// Uses a seeded PRNG (seed = 42) so content is fully reproducible.
    /// The server binds to `127.0.0.1:0` (OS-assigned port) and listens
    /// until it is dropped, which triggers a graceful shutdown.
    pub async fn new(size: u64) -> Self {
        Self::from_bytes(generate_content(size)).await
    }

    /// Start a server over caller-provided bytes (checksums are derived).
    ///
    /// Useful for artifacts whose content is fixed by another component,
    /// such as the native updater's download-and-verify tests.
    pub async fn from_bytes(data: Vec<u8>) -> Self {
        let data = Arc::new(data);
        let file_size = data.len() as u64;

        // Pre‑compute every supported checksum from the in‑memory buffer.
        let data_ref: &[&[u8]] = &[data.as_slice()];
        let blake3_hash = hash_slices(ChecksumMode::Blake3, data_ref);
        let sha256_hash = hash_slices(ChecksumMode::Sha256, data_ref);
        let sha512_hash = hash_slices(ChecksumMode::Sha512, data_ref);

        let state = Arc::new(ServerState {
            data: data.clone(),
            blake3_hash: blake3_hash.clone(),
            sha256_hash: sha256_hash.clone(),
            sha512_hash: sha512_hash.clone(),
            encoded_responses: Arc::new(AtomicUsize::new(0)),
            range_failure_served: AtomicBool::new(false),
        });
        let encoded_responses = state.encoded_responses.clone();

        let app = Router::new()
            .route("/file", get(serve_file))
            .route("/file/range", get(serve_file_range))
            .route("/file/range/slow/{ms}", get(serve_file_range_slow))
            .route(
                "/file/range/fail-once/{after}",
                get(serve_file_range_fail_once),
            )
            .route(
                "/file/range/bandwidth/{bps}",
                get(serve_file_range_bandwidth),
            )
            .route("/file/delayed/{ms}", get(serve_file_delayed))
            .route("/file/truncated/{bytes}", get(serve_file_truncated))
            .route("/file/stream-reset/{bytes}", get(serve_file_stream_reset))
            .route("/file/slow/{ms}", get(serve_file_slow))
            .route("/file/bandwidth/{bps}", get(serve_file_bandwidth))
            .route("/file/redirect/{code}", get(serve_file_redirect))
            .route("/file/range-416", get(serve_file_range_416))
            .route("/file/no-length", get(serve_file_no_length))
            .route("/file/wrong-length", get(serve_file_wrong_length))
            .route("/file/status/{code}", get(serve_file_status))
            .route("/file/github-asset", get(serve_github_asset))
            .route("/file/range-shifted/{shift}", get(serve_file_range_shifted))
            .route("/file/range-bitflip", get(serve_file_range_bitflip))
            .route("/file/encoded/{encoding}", get(serve_file_encoded))
            .route(
                "/file/encoded-range/{encoding}",
                get(serve_file_encoded_range),
            )
            .route("/file.sha256", get(serve_sha256))
            .route("/file.sha256sum", get(serve_sha256))
            .route("/SHA256SUMS", get(serve_sha256sums))
            .route("/sha256sums.txt", get(serve_sha256sums))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("TestServer: failed to bind TCP listener");
        let addr = format!("http://{}", listener.local_addr().unwrap());

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            if let Err(e) = serve::serve(listener, app)
                .with_graceful_shutdown(async {
                    // Wait for shutdown signal or cancellation.
                    let _ = rx.await;
                })
                .await
            {
                // server errors are benign in tests; just trace them
                eprintln!("[test_harness] server exited: {e}");
            }
        });

        Self {
            addr,
            file_size,
            blake3_hash,
            sha256_hash,
            sha512_hash,
            content: data,
            _shutdown: tx,
            encoded_responses,
        }
    }

    /// The deterministic bytes every non-fault endpoint serves.
    ///
    /// Lets a test compute per-piece hashes without downloading the file first.
    pub fn content(&self) -> &[u8] {
        &self.content
    }

    /// URL for the full file download (`GET /file`).
    ///
    /// The `/file` endpoint does **not** set `Accept-Ranges` so the downloader
    /// falls back to single-stream mode.  For range‑aware downloads use
    /// [`Self::file_url_range`].
    pub fn file_url(&self) -> String {
        format!("{}/file", self.addr)
    }

    /// URL for the file download with full `Range` request support (`GET /file/range`).
    ///
    /// Returns `206 Partial Content` for range requests and includes the
    /// `Accept-Ranges: bytes` header, so the downloader will use multi‑threaded
    /// streaming with parallel byte‑range requests.
    pub fn file_url_range(&self) -> String {
        format!("{}/file/range", self.addr)
    }

    /// URL for a throttled, range‑aware download.
    ///
    /// Like [`Self::file_url_range`] the server advertises `Accept-Ranges: bytes`
    /// and answers range requests with `206 Partial Content`, but every
    /// connection is throttled to `bps` bytes/second. A parallel range download
    /// therefore stays in flight long enough for tests that pause/resume
    /// mid‑chunk to be deterministic instead of racing a fast localhost
    /// transfer. The limit is per connection, so N chunk workers share N×`bps`.
    pub fn file_url_range_bandwidth(&self, bps: u64) -> String {
        format!("{}/file/range/bandwidth/{bps}", self.addr)
    }

    /// URL for a range-capable file whose first request starting at or beyond
    /// `after` bytes fails with `500`; every later request is served normally.
    ///
    /// A chunked download already mid-transfer has data in its write buffer and
    /// possibly received-complete chunks behind it when the failure lands, which
    /// is the state that used to leave the buffered bytes counted as progress
    /// but absent from the file.
    pub fn file_url_range_fail_once(&self, after: u64) -> String {
        format!("{}/file/range/fail-once/{after}", self.addr)
    }

    /// URL for a range-capable file whose first response is delayed by
    /// `delay_ms` milliseconds.
    ///
    /// Useful when a test has to attach state (for example Metalink piece
    /// hashes) after `start()` but before the executor plans its chunks.
    pub fn file_url_range_slow(&self, delay_ms: u64) -> String {
        format!("{}/file/range/slow/{delay_ms}", self.addr)
    }

    /// URL for a **single-stream** file (no `Accept-Ranges`) that is served
    /// gzip/brotli/zstd-compressed when the request's `Accept-Encoding` allows
    /// it, and identity otherwise. The probe sends `identity`, so the engine
    /// sees the true size and falls back to a single stream.
    pub fn file_url_encoded(&self, encoding: &str) -> String {
        format!("{}/file/encoded/{encoding}", self.addr)
    }

    /// URL for a **range-capable** file whose endpoint compresses the whole body
    /// (ignoring `Range`) whenever the request asks for the encoding. This is
    /// the hazard `identity_encoding` prevents on segment requests: if the
    /// engine advertised compression here, every `206` would be a compressed
    /// full-file `200` and the download would corrupt.
    pub fn file_url_encoded_range(&self, encoding: &str) -> String {
        format!("{}/file/encoded-range/{encoding}", self.addr)
    }

    /// How many responses the encoded endpoints actually compressed.
    pub fn encoded_responses(&self) -> usize {
        self.encoded_responses.load(Ordering::Relaxed)
    }

    /// URL for a slow download at the given bytes‑per‑second rate.
    ///
    /// The server streams the file body in 64 KB chunks with intra‑chunk delays
    /// to emulate the target bandwidth.  This endpoint does **not** advertise
    /// `Accept-Ranges`, so downloads proceed in single‑stream mode.
    pub fn file_url_bandwidth(&self, bps: u64) -> String {
        format!("{}/file/bandwidth/{bps}", self.addr)
    }

    /// URL for a download with a startup delay.
    ///
    /// The server waits for `delay_ms` milliseconds before beginning to respond.
    /// After the delay the full file body is sent in one shot.  No `Accept-Ranges`
    /// header is set.
    pub fn file_url_slow(&self, delay_ms: u64) -> String {
        format!("{}/file/slow/{delay_ms}", self.addr)
    }

    /// URL for the file download.
    ///
    /// The server includes every supported checksum value as response headers
    /// (`x-checksum-blake3`, `x-checksum-sha256`, `x-checksum-sha512`).
    /// The `_mode` parameter indicates which checksum the caller intends
    /// to validate and is provided for documentation / future use.
    #[allow(dead_code, unused_variables)]
    pub fn file_url_with_checksum(&self, _mode: &str) -> String {
        self.file_url()
    }

    /// URL for a redirect endpoint that redirects to `/file` with the given `status_code`
    /// (e.g. 301, 302, 307, 308).
    ///
    /// reqwest follows the redirect automatically (up to 10 hops), so the download
    /// should complete at the final destination.
    pub fn file_url_redirect(&self, status_code: u16) -> String {
        format!("{}/file/redirect/{status_code}", self.addr)
    }

    /// URL that serves a `206` slice of the file whose *content* is shifted by
    /// `shift` bytes relative to the advertised `Content-Range`, while keeping the
    /// byte count identical.
    ///
    /// A well-behaved server returns `data[start..=end]` for `bytes=start-end`;
    /// this endpoint returns `data[start+shift ..= end+shift]` but still claims the
    /// range `start-end`. This simulates a misbehaving CDN/proxy that serves the
    /// wrong bytes for each range (the classic source of "SHA doesn't match" on
    /// multi-threaded range downloads). Ranges whose shifted span would fall out of
    /// bounds are served correctly, so every requested range still succeeds and the
    /// whole download completes — leaving only the checksum to catch the corruption.
    pub fn file_url_range_shifted(&self, shift: u64) -> String {
        format!("{}/file/range-shifted/{shift}", self.addr)
    }

    /// URL that serves `206` range slices with content that drifts from the source.
    ///
    /// Every served range has its first byte bit-flipped relative to the true
    /// content, simulating a flaky origin that returns different bytes on retry.
    /// Byte counts stay identical, so the downloader completes every chunk and only
    /// a checksum comparison can detect the corruption.
    pub fn file_url_range_bitflip(&self) -> String {
        format!("{}/file/range-bitflip", self.addr)
    }

    /// URL that returns `416 Range Not Satisfiable` for any request with a `Range` header.    ///
    /// Non-range requests receive the full file with `Accept-Ranges: bytes`, which
    /// tricks the executor into attempting parallel chunked downloads — each chunk
    /// worker then gets 416 and fails.
    pub fn file_url_range_416(&self) -> String {
        format!("{}/file/range-416", self.addr)
    }

    /// URL that serves the full file body **without** a `Content-Length` header.
    ///
    /// The HTTP layer uses chunked transfer encoding. Because no Content-Length is
    /// advertised, the executor falls back to single-stream mode and finishes when
    /// the stream ends.
    pub fn file_url_no_length(&self) -> String {
        format!("{}/file/no-length", self.addr)
    }

    /// URL that serves the full file body but with a `Content-Length` header set to
    /// `file_size - 1` (one byte less than the actual body).
    ///
    /// reqwest respects the declared Content-Length, so the downloader receives
    /// one fewer byte than the true file content and marks the task as Completed
    /// prematurely.  This tests how the executor handles a Content-Length mismatch.
    pub fn file_url_wrong_length(&self) -> String {
        format!("{}/file/wrong-length", self.addr)
    }

    /// URL that advertises the full `Content-Length`, streams the first
    /// `bytes_before_reset` bytes, and then aborts the HTTP response body stream
    /// with a simulated `ConnectionReset` error.
    pub fn file_url_stream_reset(&self, bytes_before_reset: usize) -> String {
        format!("{}/file/stream-reset/{bytes_before_reset}", self.addr)
    }

    /// URL that returns the given HTTP status code with an empty body.
    ///
    /// Useful for testing retry logic with arbitrary server error status codes.
    pub fn file_url_status(&self, code: u16) -> String {
        format!("{}/file/status/{code}", self.addr)
    }

    /// URL mimicking GitHub's release-asset download endpoint
    /// (`https://api.github.com/.../releases/assets/<id>`).
    ///
    /// Like GitHub, this endpoint only redirects to the real artifact when the
    /// request carries `Accept: application/octet-stream`; without that header it
    /// responds `200 OK` with the asset's JSON metadata. Downloaders that omit the
    /// header (e.g. the self-update flow before the fix) would silently save the
    /// metadata JSON instead of the installer, then fail signature verification.
    pub fn file_url_github_asset(&self) -> String {
        format!("{}/file/github-asset", self.addr)
    }

    /// URL for the direct .sha256 checksum file of the main file.
    pub fn file_sha256_url(&self) -> String {
        format!("{}/file.sha256", self.addr)
    }

    /// URL for the SHA256SUMS manifest file.
    pub fn sha256sums_url(&self) -> String {
        format!("{}/SHA256SUMS", self.addr)
    }
}

// ---------------------------------------------------------------------------
// Internal state & handlers
// ---------------------------------------------------------------------------

/// Shared server state available to all route handlers.
pub(crate) struct ServerState {
    pub(crate) data: Arc<Vec<u8>>,
    pub(crate) blake3_hash: String,
    pub(crate) sha256_hash: String,
    pub(crate) sha512_hash: String,
    pub(crate) encoded_responses: Arc<AtomicUsize>,
    /// Set once `/file/range/fail-once/{after}` has served its one 500.
    pub(crate) range_failure_served: AtomicBool,
}

/// Generate `size` bytes of deterministic random content.
///
/// Uses a [`StdRng`] seeded with a fixed value (`42`), guaranteeing the
/// same output for the same `size` across runs and platforms.
fn generate_content(size: u64) -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut data = vec![0u8; size as usize];
    rng.fill_bytes(&mut data);
    data
}

// ---------------------------------------------------------------------------
// Self‑tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;

/// An [`IoWorker`](crate::buffer_pool::IoWorker) backed by a real device queue.
///
/// Benches and tests that want the production flush path without building a
/// whole `SystemContext` use this; `io_scheduler` is the only producer of
/// sinks, and `buffer_pool` deliberately does not name it.
pub fn io_worker() -> crate::buffer_pool::IoWorker {
    crate::buffer_pool::IoWorker::new(std::sync::Arc::new(
        crate::io_scheduler::DiskDeviceManager::new(),
    ))
}
