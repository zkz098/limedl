use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use bytes::Bytes;
use futures_util::stream;
use tokio::time::sleep;

use crate::ServerState;
use crate::handlers::{add_checksum_headers, parse_byte_range, serve_file, usize_header_value};

// ---------------------------------------------------------------------------
// GET /file/delayed/{ms}
// ---------------------------------------------------------------------------

/// Serve the full file after an initial delay of `ms` milliseconds.
///
/// The delay is applied once before the response starts — useful for
/// simulating slow servers or testing timeout behaviour. Content and
/// checksums are identical to `/file`.
pub(crate) async fn serve_file_delayed(
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
pub(crate) async fn serve_file_truncated(
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
// GET /file/stream-reset/{bytes}
// ---------------------------------------------------------------------------

/// Advertise the full `Content-Length`, stream `bytes_before_reset` bytes, and
/// then abort the response body stream with a simulated `ConnectionReset`.
pub(crate) async fn serve_file_stream_reset(
    State(state): State<Arc<ServerState>>,
    Path(bytes_before_reset): Path<usize>,
) -> impl IntoResponse {
    let total_len = state.data.len();
    let limit = bytes_before_reset.min(total_len);
    let prefix = Bytes::copy_from_slice(&state.data[..limit]);

    let stream = stream::unfold(0u8, move |step| {
        let prefix = prefix.clone();
        async move {
            match step {
                0 => Some((Ok::<Bytes, std::io::Error>(prefix), 1)),
                1 => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Some((
                        Err(std::io::Error::new(
                            std::io::ErrorKind::ConnectionReset,
                            "simulated mid-stream connection reset",
                        )),
                        2,
                    ))
                }
                _ => None,
            }
        }
    });

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_LENGTH, usize_header_value(total_len));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);

    (StatusCode::OK, headers, Body::from_stream(stream)).into_response()
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
pub(crate) async fn serve_file_slow(
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
pub(crate) async fn serve_file_bandwidth(
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
// GET /file/range/bandwidth/{bps}
// ---------------------------------------------------------------------------

/// Serve a byte range at the given bytes‑per‑second rate.
///
/// Combines [`serve_file_range`]'s `206 Partial Content` handling with
/// [`serve_file_bandwidth`]'s per‑connection throttling: the range is streamed
/// in 64 KB chunks with delays, so a multi‑threaded range download remains
/// mid‑flight long enough for timing‑sensitive tests (pause/resume chunk
/// teardown) to be deterministic. Without a `Range` header the full file is
/// streamed at the same rate.
pub(crate) async fn serve_file_range_bandwidth(
    State(state): State<Arc<ServerState>>,
    Path(bps): Path<u64>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    const CHUNK_SIZE: usize = 64 * 1024; // 64 KB

    let data_len = state.data.len();
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    resp_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut resp_headers, &state);

    let (start, end, status) = match req_headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        None => (0, data_len.saturating_sub(1), StatusCode::OK),
        Some(range_value) => {
            let Some(range_str) = range_value.strip_prefix("bytes=") else {
                return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            };
            let Some((start, end)) = parse_byte_range(range_str, data_len) else {
                return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            };
            let content_range = format!("bytes {start}-{end}/{data_len}");
            resp_headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&content_range).unwrap(),
            );
            (start, end, StatusCode::PARTIAL_CONTENT)
        }
    };

    let body = if data_len == 0 {
        Vec::new()
    } else {
        state.data[start..=end].to_vec()
    };
    resp_headers.insert(header::CONTENT_LENGTH, usize_header_value(body.len()));

    let delay_per_chunk = if bps > 0 {
        Duration::from_nanos((CHUNK_SIZE as f64 / bps as f64 * 1_000_000_000.0) as u64)
    } else {
        Duration::ZERO
    };

    let stream = stream::unfold((body, 0usize), move |(data, offset)| {
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

    (status, resp_headers, Body::from_stream(stream)).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/redirect/{code}
// ---------------------------------------------------------------------------

/// Redirect to `/file` with the given HTTP status code (301, 302, 307, 308).
///
/// reqwest follows the redirect automatically (up to 10 hops), so the
/// downloader never sees the redirect status — it goes directly to `/file`.
pub(crate) async fn serve_file_redirect(
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
pub(crate) async fn serve_file_range_416(
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
pub(crate) async fn serve_file_no_length(
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut headers, &state);

    let data = state.data.clone();
    let stream =
        stream::once(async move { Ok::<Bytes, Infallible>(Bytes::copy_from_slice(&data)) });
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
pub(crate) async fn serve_file_wrong_length(
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
// GET /file/status/{code}
// ---------------------------------------------------------------------------

/// Serve the given HTTP status code with an empty body.
///
/// Useful for testing retry logic with arbitrary status codes.
pub(crate) async fn serve_file_status(Path(code): Path<u16>) -> impl IntoResponse {
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Vec::<u8>::new()).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/range-shifted/{shift}
// ---------------------------------------------------------------------------

/// Serve a byte range whose *content* is shifted by `shift` bytes from the
/// advertised `Content-Range`, with identical length.
///
/// `bytes=start-end` returns `data[start+shift ..= end+shift]` but declares
/// `Content-Range: bytes start-end/{len}`. Spans that would fall out of bounds are
/// served correctly so every range still succeeds. This reproduces the real-world
/// failure where a multi-threaded download assembles a silently-shifted file from
/// a misbehaving range server.
pub(crate) async fn serve_file_range_shifted(
    State(state): State<Arc<ServerState>>,
    Path(shift): Path<u64>,
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

    let Some(range_value) = req_headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
        resp_headers.insert(header::CONTENT_LENGTH, usize_header_value(data_len));
        return (StatusCode::OK, resp_headers, state.data.to_vec()).into_response();
    };
    let Some(range_str) = range_value.strip_prefix("bytes=") else {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    };
    let Some((start, end)) = parse_byte_range(range_str, data_len) else {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    };

    let shifted_start = start.saturating_add(shift as usize);
    let shifted_end = end.saturating_add(shift as usize);
    let body = if shifted_end < data_len && shifted_start < data_len {
        state.data[shifted_start..=shifted_end].to_vec()
    } else {
        // Out-of-bounds range — serve the true bytes so the request still succeeds.
        state.data[start..=end].to_vec()
    };

    let content_range = format!("bytes {start}-{end}/{data_len}");
    resp_headers.insert(
        header::CONTENT_RANGE,
        HeaderValue::from_str(&content_range).unwrap(),
    );
    resp_headers.insert(header::CONTENT_LENGTH, usize_header_value(body.len()));
    (StatusCode::PARTIAL_CONTENT, resp_headers, body).into_response()
}

// ---------------------------------------------------------------------------
// GET /file/range-bitflip
// ---------------------------------------------------------------------------

/// Serve `206` ranges whose first byte is bit-flipped vs. the true content.
///
/// Byte counts are unchanged, so every chunk completes normally; only a checksum
/// comparison can reveal that the assembled file differs from the source.
pub(crate) async fn serve_file_range_bitflip(
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

    let Some(range_value) = req_headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
        resp_headers.insert(header::CONTENT_LENGTH, usize_header_value(data_len));
        return (StatusCode::OK, resp_headers, state.data.to_vec()).into_response();
    };
    let Some(range_str) = range_value.strip_prefix("bytes=") else {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    };
    let Some((start, end)) = parse_byte_range(range_str, data_len) else {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    };

    let mut body = state.data[start..=end].to_vec();
    if let Some(first) = body.first_mut() {
        *first ^= 0xFF;
    }
    let content_range = format!("bytes {start}-{end}/{data_len}");
    resp_headers.insert(
        header::CONTENT_RANGE,
        HeaderValue::from_str(&content_range).unwrap(),
    );
    resp_headers.insert(header::CONTENT_LENGTH, usize_header_value(body.len()));
    (StatusCode::PARTIAL_CONTENT, resp_headers, body).into_response()
}
