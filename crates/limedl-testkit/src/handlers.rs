use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::time::sleep;

use crate::ServerState;

// ---------------------------------------------------------------------------
// Helper: attach checksum headers
// ---------------------------------------------------------------------------

/// Add every pre‑computed checksum header to `headers`.
pub(crate) fn add_checksum_headers(headers: &mut HeaderMap, state: &ServerState) {
    if let Ok(val) = HeaderValue::from_str(&state.blake3_hash) {
        headers.insert("x-checksum-blake3", val);
    }
    if let Ok(val) = HeaderValue::from_str(&state.sha256_hash) {
        headers.insert("x-checksum-sha256", val);
    }
    if let Ok(val) = HeaderValue::from_str(&state.sha512_hash) {
        headers.insert("x-checksum-sha512", val);
    }
}

// ---------------------------------------------------------------------------
// Helper: numeric header values
// ---------------------------------------------------------------------------

pub(crate) fn usize_header_value(n: usize) -> HeaderValue {
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
pub(crate) async fn serve_file(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
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
pub(crate) async fn serve_file_range(
    State(state): State<Arc<ServerState>>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    serve_range_identity(&state, &req_headers)
}

/// Range response delayed by `ms` before the first byte is produced.
pub(crate) async fn serve_file_range_slow(
    State(state): State<Arc<ServerState>>,
    Path(delay_ms): Path<u64>,
    req_headers: HeaderMap,
) -> Response {
    sleep(Duration::from_millis(delay_ms)).await;
    serve_range_identity(&state, &req_headers)
}

/// Range response whose first request starting at or beyond `after` fails.
///
/// The failure is one-shot: once the `500` has been served the route behaves
/// exactly like `/file/range`, so a retry can finish the download. Requests
/// without a `Range` header (the probe's HEAD) are always served.
pub(crate) async fn serve_file_range_fail_once(
    State(state): State<Arc<ServerState>>,
    Path(after): Path<u64>,
    req_headers: HeaderMap,
) -> Response {
    let starts_at_or_after = req_headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes="))
        .and_then(|value| value.split('-').next())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|start| start >= after);
    if starts_at_or_after && !state.range_failure_served.swap(true, Ordering::SeqCst) {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    serve_range_identity(&state, &req_headers)
}

/// Identity `Range` response shared by `/file/range` and the encoded-range
/// endpoint's fallback (a request that did not ask for compression).
fn serve_range_identity(state: &ServerState, req_headers: &HeaderMap) -> Response {
    let data_len = state.data.len();
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    resp_headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename*=UTF-8''test-file.bin"),
    );
    add_checksum_headers(&mut resp_headers, state);

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

/// The `Content-Encoding` token a client must ask for.
fn encoding_token(encoding: &str) -> &'static str {
    match encoding {
        "brotli" => "br",
        "gzip" => "gzip",
        "zstd" => "zstd",
        _ => "identity",
    }
}

/// `true` when the request's `Accept-Encoding` explicitly allows `encoding`.
fn accepts_encoding(req_headers: &HeaderMap, encoding: &str) -> bool {
    let token = encoding_token(encoding);
    let Some(value) = req_headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    value.split(',').any(|part| {
        part.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case(token)
    })
}

/// Compress `data` with the named encoding. `None` for an unknown name.
fn encode_body(data: &[u8], encoding: &str) -> Option<Vec<u8>> {
    use std::io::Write;
    match encoding {
        "gzip" => {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(data).ok()?;
            encoder.finish().ok()
        }
        "brotli" => {
            let mut out = Vec::new();
            {
                let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 5, 22);
                writer.write_all(data).ok()?;
            }
            Some(out)
        }
        "zstd" => zstd::stream::encode_all(data, 3).ok(),
        _ => None,
    }
}

/// `GET /file/encoded/{encoding}` — single-stream (no `Accept-Ranges`).
///
/// Compresses only when the client asks for the encoding; the probe's
/// `Accept-Encoding: identity` therefore sees the true length and no ranges.
pub(crate) async fn serve_file_encoded(
    State(state): State<Arc<ServerState>>,
    Path(encoding): Path<String>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    if accepts_encoding(&req_headers, &encoding)
        && let Some(body) = encode_body(&state.data, &encoding)
    {
        state.encoded_responses.fetch_add(1, Ordering::Relaxed);
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(encoding_token(&encoding)) {
            headers.insert(header::CONTENT_ENCODING, value);
        }
        headers.insert(header::CONTENT_LENGTH, usize_header_value(body.len()));
        return (StatusCode::OK, headers, body).into_response();
    }
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_LENGTH, usize_header_value(state.data.len()));
    (StatusCode::OK, headers, state.data.to_vec()).into_response()
}

/// `GET /file/encoded-range/{encoding}` — range-capable, but a request that
/// asks for compression gets the **whole** compressed file as a `200` (the
/// endpoint ignores `Range`, as a compressing server may). Segment requests
/// must therefore force `identity`.
pub(crate) async fn serve_file_encoded_range(
    State(state): State<Arc<ServerState>>,
    Path(encoding): Path<String>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    if accepts_encoding(&req_headers, &encoding)
        && let Some(body) = encode_body(&state.data, &encoding)
    {
        state.encoded_responses.fetch_add(1, Ordering::Relaxed);
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        if let Ok(value) = HeaderValue::from_str(encoding_token(&encoding)) {
            headers.insert(header::CONTENT_ENCODING, value);
        }
        headers.insert(header::CONTENT_LENGTH, usize_header_value(body.len()));
        return (StatusCode::OK, headers, body).into_response();
    }
    serve_range_identity(&state, &req_headers)
}

/// Parse a `"start-end"` or `"start-"` byte range string.
///
/// Returns `(start, end)` **inclusive** bounds if valid, `None` otherwise.
pub(crate) fn parse_byte_range(range_str: &str, data_len: usize) -> Option<(usize, usize)> {
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
// GET /file/github-asset
// ---------------------------------------------------------------------------

/// GitHub-style release asset endpoint.
///
/// Mimics `GET https://api.github.com/repos/{owner}/{repo}/releases/assets/<id>`:
/// - with `Accept: application/octet-stream` → `302 Found` → `/file` (real bytes)
/// - otherwise → `200 OK` with the asset's JSON metadata body
///
/// This is the exact behavior that requires the self-update download to send the
/// `Accept: application/octet-stream` header, otherwise the "installer" is the
/// metadata JSON and signature verification fails.
pub(crate) async fn serve_github_asset(
    State(_state): State<Arc<ServerState>>,
    req_headers: HeaderMap,
) -> impl IntoResponse {
    let accepts_octet_stream = req_headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("application/octet-stream"));

    if accepts_octet_stream {
        let mut headers = HeaderMap::new();
        headers.insert(header::LOCATION, HeaderValue::from_static("/file"));
        return (StatusCode::FOUND, headers, Vec::<u8>::new()).into_response();
    }

    // JSON metadata that loosely mirrors GitHub's asset object.
    let metadata = format!(
        r#"{{"name":"test-file.bin","size":{},"content_type":"application/json"}}"#,
        0
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(header::CONTENT_LENGTH, usize_header_value(metadata.len()));
    (StatusCode::OK, headers, metadata.into_bytes()).into_response()
}

// ---------------------------------------------------------------------------
// GET /file.sha256 and /SHA256SUMS
// ---------------------------------------------------------------------------

pub(crate) async fn serve_sha256(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    (StatusCode::OK, headers, state.sha256_hash.clone()).into_response()
}

pub(crate) async fn serve_sha256sums(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let manifest = format!(
        "{}  file\n{}  test-file.bin\n",
        state.sha256_hash, state.sha256_hash
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    (StatusCode::OK, headers, manifest).into_response()
}

// ---------------------------------------------------------------------------
// Content generation
// ---------------------------------------------------------------------------
