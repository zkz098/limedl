use reqwest::{
    Client, Response, StatusCode, Url,
    header::{self, HeaderMap, HeaderValue},
};
use percent_encoding::percent_decode_str;

use super::{
    error::{DownloadError, Result},
    manifest::Manifest,
};

pub enum ResponseDisposition {
    Use(Response),
    Retryable(StatusCode),
    Invalid(StatusCode),
}

pub fn classify_download_response(response: Response) -> ResponseDisposition {
    let status = response.status();
    if status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT {
        return ResponseDisposition::Use(response);
    }
    if status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
    {
        return ResponseDisposition::Retryable(status);
    }
    ResponseDisposition::Invalid(status)
}

/// Maximum number of bytes read from a 403 body when looking for anti-abuse
/// markers. Denial pages are small; the cap keeps the sniff bounded.
pub const ANTI_ABUSE_SNIFF_LIMIT: usize = 16 * 1024;

/// Lower-case markers that identify a WAF / mirror anti-abuse denial page.
///
/// These pages answer 403 like anti-hotlink protection does, but no `Referer`
/// can fix them — the edge has rejected the client itself.
const ANTI_ABUSE_MARKERS: &[&str] = &[
    // TUNA (mirrors.tuna.tsinghua.edu.cn)
    "uncommon characteristics",
    "非常用软件的特征",
    "you have been denied access",
    "无法访问此页面",
    // Cloudflare / generic WAF interstitials
    "cf-error-details",
    "cf-chl-",
    "attention required! | cloudflare",
    "sorry, you have been blocked",
    // Generic Chinese WAF denials
    "访问被拒绝",
    "请求被拦截",
];

/// Returns `true` when `body` looks like a WAF / anti-abuse denial page.
pub fn looks_like_anti_abuse_page(body: &[u8]) -> bool {
    if body.is_empty() {
        return false;
    }
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    ANTI_ABUSE_MARKERS.iter().any(|marker| text.contains(marker))
}

/// Read at most `limit` bytes from the start of `response`'s body.
///
/// Used for best-effort 403 classification only: EOF and transport errors end
/// the read early and are not propagated.
pub async fn read_body_prefix(response: &mut Response, limit: usize) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(limit.min(8 * 1024));
    while buffer.len() < limit {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let take = chunk.len().min(limit - buffer.len());
                buffer.extend_from_slice(&chunk[..take]);
                if take < chunk.len() {
                    break;
                }
            }
            Ok(None) | Err(_) => break,
        }
    }
    buffer
}

/// Error for a 403 whose body indicates an anti-abuse/WAF block.
///
/// Keeps the historical `http status …` prefix so existing error handling and
/// status parsing continue to work, and appends an actionable hint.
pub fn anti_abuse_forbidden_error() -> DownloadError {
    DownloadError::InvalidResponse(String::from(
        "http status 403 Forbidden (server anti-abuse check rejected this client; \
         update the default User-Agent in Settings or try another mirror)",
    ))
}

pub fn validate_probe_response(response: &Response) -> Result<()> {
    let status = response.status();
    if status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT {
        return Ok(());
    }

    Err(DownloadError::InvalidResponse(format!(
        "probe returned http status {status}"
    )))
}

pub fn extract_total_bytes(status: StatusCode, headers: &HeaderMap) -> Option<u64> {
    if status == StatusCode::PARTIAL_CONTENT
        && let Some(content_range) = header_string(headers, header::CONTENT_RANGE)
    {
        return content_range
            .rsplit('/')
            .next()
            .and_then(|value| value.parse::<u64>().ok());
    }
    header_string(headers, header::CONTENT_LENGTH).and_then(|value| value.parse::<u64>().ok())
}

pub fn supports_ranges(status: StatusCode, headers: &HeaderMap) -> bool {
    if status == StatusCode::PARTIAL_CONTENT || headers.contains_key(header::CONTENT_RANGE) {
        return true;
    }
    header_string(headers, header::ACCEPT_RANGES)
        .map(|value| value.eq_ignore_ascii_case("bytes"))
        .unwrap_or(false)
}

pub fn infer_file_name(final_url: &str, headers: &HeaderMap) -> Option<String> {
    if let Some(header) = headers.get(header::CONTENT_DISPOSITION)
        && let Ok(value) = header.to_str()
        && let Some(decoded) = parse_content_disposition(value)
    {
        let clean = sanitize_filename::sanitize(decoded);
        if !clean.is_empty() {
            return Some(clean);
        }
    }

    Url::parse(final_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(ToOwned::to_owned))
        })
        .map(sanitize_filename::sanitize)
        .filter(|value| !value.is_empty())
        .or_else(|| Some(String::from("download")))
}

pub fn validate_segment_response(
    response: &Response,
    expected_start: u64,
    expected_end: u64,
) -> Result<()> {
    let Some(content_range) = header_string(response.headers(), header::CONTENT_RANGE) else {
        return Err(DownloadError::InvalidResponse(String::from(
            "segment response missing content-range",
        )));
    };

    let Some(range_part) = content_range.strip_prefix("bytes ") else {
        return Err(DownloadError::InvalidResponse(String::from(
            "invalid content-range format",
        )));
    };
    let Some((range, _total)) = range_part.split_once('/') else {
        return Err(DownloadError::InvalidResponse(String::from(
            "invalid content-range payload",
        )));
    };
    let Some((start, end)) = range.split_once('-') else {
        return Err(DownloadError::InvalidResponse(String::from(
            "invalid content-range bounds",
        )));
    };

    let parsed_start = start
        .parse::<u64>()
        .map_err(|_| DownloadError::InvalidResponse(String::from("invalid content-range start")))?;
    let parsed_end = end
        .parse::<u64>()
        .map_err(|_| DownloadError::InvalidResponse(String::from("invalid content-range end")))?;

    if parsed_start != expected_start || parsed_end > expected_end {
        return Err(DownloadError::InvalidResponse(format!(
            "unexpected content-range {parsed_start}-{parsed_end}, expected {expected_start}-{expected_end}"
        )));
    }

    Ok(())
}

pub fn if_range_header(manifest: &Manifest) -> Option<(header::HeaderName, HeaderValue)> {
    manifest
        .etag
        .as_deref()
        .or(manifest.last_modified.as_deref())
        .and_then(|value| HeaderValue::from_str(value).ok())
        .map(|value| (header::IF_RANGE, value))
}

pub fn build_segment_request(
    client: &Client,
    url: &str,
    user_agent: &str,
    extra_headers: &[String],
    start: u64,
    end: u64,
    validator: Option<(header::HeaderName, HeaderValue)>,
) -> reqwest::RequestBuilder {
    let mut builder = client
        .get(url)
        .header(header::USER_AGENT, user_agent)
        .header(header::RANGE, format!("bytes={start}-{end}"));
    if let Some((name, value)) = validator {
        builder = builder.header(name, value);
    }
    // A segment always carries `Range`; a compressed `206` would break offsets.
    identity_encoding(apply_extra_headers(builder, extra_headers))
}

/// Force `Accept-Encoding: identity` on a request builder.
///
/// reqwest's gzip/brotli/zstd features make it advertise `Accept-Encoding` on
/// every request without one and transparently decompress any `Content-Encoding`
/// response. That is only correct for the plain single-stream GET:
///
/// - on a probe, a compressed body makes `Content-Length` and `Accept-Ranges`
///   describe the compressed representation, which would mis-plan the download;
/// - on a `Range` request, decompressing a `206` destroys the byte offsets, the
///   per-chunk `Content-Length` and the final checksum.
///
/// Probes and segment requests therefore opt out. Applied *after*
/// [`apply_extra_headers`] so a user-supplied `Accept-Encoding` cannot re-enable
/// compression on a request where it would corrupt the file.
pub fn identity_encoding(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    builder.header(header::ACCEPT_ENCODING, "identity")
}

/// Apply `"Name: Value"` extra headers to a request builder, skipping any
/// malformed entries.
pub fn apply_extra_headers(
    mut builder: reqwest::RequestBuilder,
    headers: &[String],
) -> reqwest::RequestBuilder {
    for h in headers {
        if let Some((name, value)) = h.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                continue;
            }
            if let (Ok(n), Ok(v)) = (
                header::HeaderName::from_bytes(name.as_bytes()),
                header::HeaderValue::from_str(value),
            ) {
                builder = builder.header(n, v);
            }
        }
    }
    builder
}

fn parse_content_disposition(value: &str) -> Option<String> {
    for part in value.split(';').map(str::trim) {
        if let Some(rest) = part.strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            let encoded = rest.split("''").nth(1).unwrap_or(rest);
            if let Ok(decoded) = percent_decode_str(encoded).decode_utf8() {
                return Some(decoded.into_owned());
            }
        }
    }

    for part in value.split(';').map(str::trim) {
        if let Some(rest) = part.strip_prefix("filename=") {
            return Some(rest.trim_matches('"').to_string());
        }
    }
    None
}

pub fn header_string(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

/// Check if a header named `header_name` exists in `headers` (case-insensitive).
pub fn has_header(headers: &[String], header_name: &str) -> bool {
    headers.iter().any(|h| {
        h.split_once(':')
            .map(|(name, _)| name.trim().eq_ignore_ascii_case(header_name))
            .unwrap_or(false)
    })
}

/// Infer candidate Referer URLs for anti-hotlink protection when a server
/// returns HTTP 403 Forbidden.
pub fn infer_candidate_referers(url: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let Ok(parsed) = Url::parse(url) else {
        return candidates;
    };
    let Some(host) = parsed.host_str() else {
        return candidates;
    };

    let host_lower = host.to_ascii_lowercase();

    // Specific domain rules for known anti-hotlink servers (e.g. NVIDIA Zen CDN)
    if host_lower.contains("nvidia.com") {
        candidates.push("https://www.nvidia.com/".to_string());
        candidates.push("https://www.nvidia.cn/".to_string());
    } else if host_lower.contains("nvidia.cn") {
        candidates.push("https://www.nvidia.cn/".to_string());
        candidates.push("https://www.nvidia.com/".to_string());
    }

    // General domain rules:
    // 1. Same origin: scheme + host + "/"
    let scheme = parsed.scheme();
    let origin_referer = format!("{scheme}://{host}/");
    if !candidates.contains(&origin_referer) {
        candidates.push(origin_referer);
    }

    // 2. Base / apex domain (e.g. cn.download.nvidia.com -> www.nvidia.com, nvidia.com)
    let parts: Vec<&str> = host_lower.split('.').collect();
    if parts.len() >= 3 {
        let is_second_level = parts.len() >= 4
            && (parts[parts.len() - 2] == "com"
                || parts[parts.len() - 2] == "co"
                || parts[parts.len() - 2] == "org"
                || parts[parts.len() - 2] == "net"
                || parts[parts.len() - 2] == "edu"
                || parts[parts.len() - 2] == "gov");

        let base_domain = if is_second_level {
            parts[parts.len() - 3..].join(".")
        } else {
            parts[parts.len() - 2..].join(".")
        };

        let www_referer = format!("{scheme}://www.{base_domain}/");
        if !candidates.contains(&www_referer) {
            candidates.push(www_referer);
        }

        let base_referer = format!("{scheme}://{base_domain}/");
        if !candidates.contains(&base_referer) {
            candidates.push(base_referer);
        }
    }

    candidates
}

/// Check if an error was caused by an HTTP 429 Too Many Requests response.
pub fn is_too_many_requests_error(error: &DownloadError) -> bool {
    match error {
        DownloadError::InvalidResponse(msg) => msg.contains("429"),
        DownloadError::Http(err) => err.status() == Some(StatusCode::TOO_MANY_REQUESTS),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
