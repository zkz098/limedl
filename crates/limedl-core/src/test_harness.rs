//! Reusable test harness for download integration tests.
//! Provides a local HTTP server serving deterministic random content
//! with known checksums, range request support, bandwidth simulation,
//! and failure injection endpoints.
//!
//! # Example
//!
//! ```ignore
//! use crate::test_harness::TestServer;
//!
//! let server = TestServer::new(1024 * 1024).await;
//! let url = server.file_url();
//! // use url in a download request ...
//! assert_eq!(downloaded.len() as u64, server.file_size);
//! ```

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
    routing::get,
    serve,
};
use bytes::Bytes;
use futures_util::stream;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use tokio::time::sleep;

use super::checksum::hash_slices;
use super::types::ChecksumMode;

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
    /// Pre-computed XXH3-128 hex checksum of the full content.
    pub xxh3_hash: String,
    /// Dropping this triggers graceful server shutdown.
    _shutdown: tokio::sync::oneshot::Sender<()>,
}

impl TestServer {
    /// Start a server serving `size` bytes of deterministic random content.
    ///
    /// Uses a seeded PRNG (seed = 42) so content is fully reproducible.
    /// The server binds to `127.0.0.1:0` (OS-assigned port) and listens
    /// until dropped or [`Self::close`] is called.
    pub async fn new(size: u64) -> Self {
        let data = generate_content(size);

        // Pre‑compute all three checksums from the in‑memory buffer.
        let data_ref: &[&[u8]] = &[&data];
        let blake3_hash = hash_slices(ChecksumMode::Blake3, data_ref);
        let sha256_hash = hash_slices(ChecksumMode::Sha256, data_ref);
        let xxh3_hash = hash_slices(ChecksumMode::Xxh3128, data_ref);

        let state = Arc::new(ServerState {
            data: Arc::new(data),
            blake3_hash: blake3_hash.clone(),
            sha256_hash: sha256_hash.clone(),
            xxh3_hash: xxh3_hash.clone(),
        });

        let app = Router::new()
            .route("/file", get(serve_file))
            .route("/file/range", get(serve_file_range))
            .route("/file/delayed/{ms}", get(serve_file_delayed))
            .route("/file/truncated/{bytes}", get(serve_file_truncated))
            .route("/file/slow/{ms}", get(serve_file_slow))
            .route("/file/bandwidth/{bps}", get(serve_file_bandwidth))
            .route("/file/redirect/{code}", get(serve_file_redirect))
            .route("/file/range-416", get(serve_file_range_416))
            .route("/file/no-length", get(serve_file_no_length))
            .route("/file/wrong-length", get(serve_file_wrong_length))
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
            file_size: size,
            blake3_hash,
            sha256_hash,
            xxh3_hash,
            _shutdown: tx,
        }
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
    /// The server includes all three checksum values as response headers
    /// (`x-checksum-blake3`, `x-checksum-sha256`, `x-checksum-xxh3`).
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

    /// URL that returns `416 Range Not Satisfiable` for any request with a `Range` header.
    ///
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
}

// ---------------------------------------------------------------------------
// Internal state & handlers
// ---------------------------------------------------------------------------

/// Shared server state available to all route handlers.
struct ServerState {
    data: Arc<Vec<u8>>,
    blake3_hash: String,
    sha256_hash: String,
    xxh3_hash: String,
}

// ---------------------------------------------------------------------------
// Helper: attach checksum headers
// ---------------------------------------------------------------------------

/// Add all three pre‑computed checksum headers to `headers`.
fn add_checksum_headers(headers: &mut HeaderMap, state: &ServerState) {
    if let Ok(val) = HeaderValue::from_str(&state.blake3_hash) {
        headers.insert("x-checksum-blake3", val);
    }
    if let Ok(val) = HeaderValue::from_str(&state.sha256_hash) {
        headers.insert("x-checksum-sha256", val);
    }
    if let Ok(val) = HeaderValue::from_str(&state.xxh3_hash) {
        headers.insert("x-checksum-xxh3", val);
    }
}

// ---------------------------------------------------------------------------
// Helper: numeric header values
// ---------------------------------------------------------------------------

fn usize_header_value(n: usize) -> HeaderValue {
    HeaderValue::from_str(&n.to_string()).unwrap()
}

// ---------------------------------------------------------------------------
// GET /file
// ---------------------------------------------------------------------------

/// Serve the full file content with checksum headers.
///
/// This endpoint does **not** set `Accept-Ranges`, so the downloader will
/// fall back to single‑stream mode.  Use [`serve_file_range`] for range‑aware
/// responses.
async fn serve_file(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_LENGTH, usize_header_value(state.data.len()));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);
    (StatusCode::OK, headers, state.data.to_vec()).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/range
// ---------------------------------------------------------------------------

/// Serve the file with full `Range` request support.
///
/// Returns the requested byte range with `206 Partial Content`.
/// If no `Range` header is present the full file is returned.
async fn serve_file_range(
    State(state): State<Arc<ServerState>>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    let data_len = state.data.len();
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    resp_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut resp_headers, &state);

    // Try to extract a Range header
    let Some(range_value) = req_headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
        // No range → full file
        resp_headers.insert(header::CONTENT_LENGTH, usize_header_value(data_len));
        return (StatusCode::OK, resp_headers, state.data.to_vec()).into_response();
    };

    let Some(range_str) = range_value.strip_prefix("bytes=") else {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    };

    let Some((start, end)) = parse_byte_range(range_str, data_len) else {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    };

    let body = &state.data[start..=end];
    let content_range = format!("bytes {start}-{end}/{data_len}");
    resp_headers.insert(
        header::CONTENT_RANGE,
        HeaderValue::from_str(&content_range).unwrap(),
    );
    resp_headers.insert(header::CONTENT_LENGTH, usize_header_value(body.len()));
    (StatusCode::PARTIAL_CONTENT, resp_headers, body.to_vec()).into_response()
}

/// Parse a `"start-end"` or `"start-"` byte range string.
///
/// Returns `(start, end)` **inclusive** bounds if valid, `None` otherwise.
fn parse_byte_range(range_str: &str, data_len: usize) -> Option<(usize, usize)> {
    let mut pieces = range_str.split('-');
    let start = pieces.next()?.parse::<usize>().ok()?;
    let end = pieces
        .next()
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                s.parse::<usize>().ok()
            }
        })
        .unwrap_or(data_len.saturating_sub(1));
    if start >= data_len {
        return None;
    }
    let end = end.min(data_len.saturating_sub(1));
    if start > end {
        return None;
    }
    Some((start, end))
}

// ---------------------------------------------------------------------------
// GET /file/delayed/{ms}
// ---------------------------------------------------------------------------

/// Serve the full file after an initial delay of `ms` milliseconds.
///
/// The delay is applied once before the response starts — useful for
/// simulating slow servers or testing timeout behaviour. Content and
/// checksums are identical to `/file`.
async fn serve_file_delayed(
    State(state): State<Arc<ServerState>>,
    Path(delay_ms): Path<u64>,
) -> impl IntoResponse {
    sleep(Duration::from_millis(delay_ms)).await;
    serve_file(State(state)).await
}

// ---------------------------------------------------------------------------
// GET /file/truncated/{bytes}
// ---------------------------------------------------------------------------

/// Serve only the first `bytes` of the file.
///
/// The `Content-Length` header is set to `bytes` (not the full file size),
/// simulating a partial / truncated response. Useful for testing how the
/// downloader handles unexpectedly short responses.
async fn serve_file_truncated(
    State(state): State<Arc<ServerState>>,
    Path(bytes): Path<u64>,
) -> impl IntoResponse {
    let truncate_at = (bytes as usize).min(state.data.len());
    let truncated = &state.data[..truncate_at];

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_LENGTH, usize_header_value(truncated.len()));
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    add_checksum_headers(&mut headers, &state);
    (StatusCode::OK, headers, truncated.to_vec()).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/slow/{ms}
// ---------------------------------------------------------------------------

/// Serve the full file after an initial delay of `ms` milliseconds.
///
/// Unlike [`serve_file_delayed`], this endpoint does **not** set the
/// `Accept-Ranges` header so that the downloader does not attempt
/// multi‑threaded range requests — the download proceeds as a single
/// sequential stream.  This is useful for scheduler tests where you
/// need predictable, sequential download timing.
async fn serve_file_slow(
    State(state): State<Arc<ServerState>>,
    Path(delay_ms): Path<u64>,
) -> impl IntoResponse {
    sleep(Duration::from_millis(delay_ms)).await;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_LENGTH, usize_header_value(state.data.len()));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);
    (StatusCode::OK, headers, state.data.to_vec()).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/bandwidth/{bps}
// ---------------------------------------------------------------------------

/// Serve the full file at the given bytes‑per‑second rate.
///
/// The body is streamed in 64 KB chunks with per‑chunk delays to hit the
/// target bandwidth.  No `Accept-Ranges` header is set, so the downloader
/// uses single‑stream mode and the bandwidth limit applies globally.
async fn serve_file_bandwidth(
    State(state): State<Arc<ServerState>>,
    Path(bps): Path<u64>,
) -> impl IntoResponse {
    const CHUNK_SIZE: usize = 64 * 1024; // 64 KB

    let delay_per_chunk = if bps > 0 {
        Duration::from_nanos((CHUNK_SIZE as f64 / bps as f64 * 1_000_000_000.0) as u64)
    } else {
        Duration::ZERO
    };

    let data = state.data.clone();
    let total_len = data.len();

    // Build a stream that yields 64 KB chunks with delays.
    let stream = stream::unfold((data, 0usize), move |(data, offset)| {
        let delay = delay_per_chunk;
        async move {
            if offset >= data.len() {
                return None;
            }
            let end = (offset + CHUNK_SIZE).min(data.len());
            let chunk = Bytes::copy_from_slice(&data[offset..end]);
            if delay > Duration::ZERO {
                tokio::time::sleep(delay).await;
            }
            Some((Ok::<Bytes, Infallible>(chunk), (data, end)))
        }
    });

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_LENGTH, usize_header_value(total_len));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);

    let body = Body::from_stream(stream);
    (StatusCode::OK, headers, body).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/redirect/{code}
// ---------------------------------------------------------------------------

/// Redirect to `/file` with the given HTTP status code (301, 302, 307, 308).
///
/// reqwest follows the redirect automatically (up to 10 hops), so the
/// downloader never sees the redirect status — it goes directly to `/file`.
async fn serve_file_redirect(
    State(_state): State<Arc<ServerState>>,
    Path(code): Path<u16>,
) -> impl IntoResponse {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::FOUND);
    let mut headers = HeaderMap::new();
    headers.insert(header::LOCATION, HeaderValue::from_static("/file"));
    (status, headers, Vec::<u8>::new()).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/range-416
// ---------------------------------------------------------------------------

/// Return `200 OK` with full body and `Accept-Ranges: bytes` for non-range
/// requests, but `416 Range Not Satisfiable` for any request with a `Range`
/// header.
///
/// The `Accept-Ranges` header causes the executor to attempt parallel
/// chunked downloads; each chunk worker gets 416 and fails.
async fn serve_file_range_416(
    State(state): State<Arc<ServerState>>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    if req_headers.contains_key(header::RANGE) {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    }
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(header::CONTENT_LENGTH, usize_header_value(state.data.len()));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);
    (StatusCode::OK, headers, state.data.to_vec()).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/no-length
// ---------------------------------------------------------------------------

/// Serve the full file body **without** a `Content-Length` header.
///
/// The body is streamed so the HTTP layer uses chunked transfer encoding.
/// Because no Content-Length is advertised, the executor falls back to
/// single-stream mode and finishes when the TCP stream ends.
async fn serve_file_no_length(
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);

    let data = state.data.clone();
    let stream = stream::once(async move {
        Ok::<Bytes, Infallible>(Bytes::copy_from_slice(&data))
    });
    let body = Body::from_stream(stream);
    (StatusCode::OK, headers, body).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/wrong-length
// ---------------------------------------------------------------------------

/// Serve the full file body but with a `Content-Length` header set to
/// `file_size - 1` (one byte less than the actual body).
///
/// The response body is truncated to match the declared Content-Length
/// so the HTTP protocol layer accepts it.  This tests how the executor
/// handles a server that advertises fewer bytes than the true content —
/// hyper's protocol validation would reject a body longer than
/// Content-Length.
async fn serve_file_wrong_length(
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    let wrong_len = state.data.len().saturating_sub(1).max(1);
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_LENGTH, usize_header_value(wrong_len));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);
    (StatusCode::OK, headers, state.data[..wrong_len].to_vec()).into_response()
}

// ---------------------------------------------------------------------------
// Content generation
// ---------------------------------------------------------------------------

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
mod tests {
    use super::*;
    use ntest::timeout;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

    #[tokio::test]
    #[timeout(30_000)]
    async fn downloads_full_file() -> TestResult {
        let server = TestServer::new(64 * 1024).await;
        let body = reqwest::get(server.file_url()).await?.bytes().await?;
        assert_eq!(body.len() as u64, server.file_size);
        Ok(())
    }

    #[tokio::test]
    #[timeout(30_000)]
    async fn checksums_match_computed_content() -> TestResult {
        let server = TestServer::new(64 * 1024).await;
        let body = reqwest::get(server.file_url()).await?.bytes().await?;

        let slices = &[&body[..]];
        assert_eq!(
            hash_slices(ChecksumMode::Blake3, slices),
            server.blake3_hash,
            "Blake3 checksum mismatch",
        );
        assert_eq!(
            hash_slices(ChecksumMode::Sha256, slices),
            server.sha256_hash,
            "SHA-256 checksum mismatch",
        );
        assert_eq!(
            hash_slices(ChecksumMode::Xxh3128, slices),
            server.xxh3_hash,
            "XXH3-128 checksum mismatch",
        );
        Ok(())
    }

    #[tokio::test]
    #[timeout(30_000)]
    async fn range_request_returns_correct_bytes() -> TestResult {
        let server = TestServer::new(64 * 1024).await;
        let range_url = format!("{}/file/range", server.addr);

        let client = reqwest::Client::new();
        let response = client
            .get(&range_url)
            .header(header::RANGE, "bytes=1000-1999")
            .send()
            .await?;

        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let body = response.bytes().await?;
        assert_eq!(body.len(), 1000);

        // Must match the deterministic random content at that offset
        let expected = generate_content(64 * 1024);
        assert_eq!(&body[..], &expected[1000..2000]);
        Ok(())
    }

    #[tokio::test]
    #[timeout(30_000)]
    async fn delayed_endpoint_content_matches() -> TestResult {
        let server = TestServer::new(16 * 1024).await;
        let delayed_url = format!("{}/file/delayed/250", server.addr);

        let start = std::time::Instant::now();
        let body = reqwest::get(&delayed_url).await?.bytes().await?;
        let elapsed = start.elapsed();

        assert_eq!(body.len() as u64, server.file_size);
        assert!(
            elapsed >= Duration::from_millis(250),
            "expected at least 250ms delay but got {elapsed:?}",
        );

        // Content must still be identical
        let expected = generate_content(16 * 1024);
        assert_eq!(&body[..], &expected);
        Ok(())
    }

    #[tokio::test]
    #[timeout(30_000)]
    async fn truncated_endpoint_serves_partial_content() -> TestResult {
        let server = TestServer::new(64 * 1024).await;
        let truncated_url = format!("{}/file/truncated/512", server.addr);

        let body = reqwest::get(&truncated_url).await?.bytes().await?;
        assert_eq!(body.len(), 512);

        // Content must be the first 512 bytes of the deterministic file
        let expected = generate_content(64 * 1024);
        assert_eq!(&body[..], &expected[..512]);
        Ok(())
    }
}
