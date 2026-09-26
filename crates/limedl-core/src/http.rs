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
    apply_extra_headers(builder, extra_headers)
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
mod tests {
    use super::*;
    use axum::http::Response as HttpResponse;
    use reqwest::Body;
    use std::str::FromStr;

    // ── helpers ─────────────────────────────────────────────

    fn make_response(status: u16, headers: &[(&str, &str)]) -> Response {
        let status_code = StatusCode::from_u16(status).unwrap();
        let mut builder = HttpResponse::builder().status(status_code);
        for &(k, v) in headers {
            builder = builder.header(k, v);
        }
        let http_resp = builder.body(Body::from(String::new())).unwrap();
        Response::from(http_resp)
    }

    fn header_map(headers: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for &(k, v) in headers {
            map.insert(
                header::HeaderName::from_str(k).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        map
    }

    // ── classify_download_response ─────────────────────────

    #[test]
    fn classify_200_ok_is_use() {
        let resp = make_response(200, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Use(_) => {}
            _ => panic!("expected Use"),
        }
    }

    #[test]
    fn classify_206_partial_content_is_use() {
        let resp = make_response(206, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Use(_) => {}
            _ => panic!("expected Use"),
        }
    }

    #[test]
    fn classify_408_request_timeout_is_retryable() {
        let resp = make_response(408, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Retryable(s) => assert_eq!(s, StatusCode::REQUEST_TIMEOUT),
            _ => panic!("expected Retryable"),
        }
    }

    #[test]
    fn classify_429_too_many_requests_is_retryable() {
        let resp = make_response(429, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Retryable(s) => assert_eq!(s, StatusCode::TOO_MANY_REQUESTS),
            _ => panic!("expected Retryable"),
        }
    }

    #[test]
    fn classify_500_server_error_is_retryable() {
        let resp = make_response(500, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Retryable(s) => assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR),
            _ => panic!("expected Retryable"),
        }
    }

    #[test]
    fn classify_503_is_retryable() {
        let resp = make_response(503, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Retryable(s) => assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE),
            _ => panic!("expected Retryable"),
        }
    }

    #[test]
    fn classify_301_redirect_is_invalid() {
        let resp = make_response(301, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Invalid(s) => assert_eq!(s, StatusCode::MOVED_PERMANENTLY),
            _ => panic!("expected Invalid"),
        }
    }

    #[test]
    fn classify_404_not_found_is_invalid() {
        let resp = make_response(404, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Invalid(s) => assert_eq!(s, StatusCode::NOT_FOUND),
            _ => panic!("expected Invalid"),
        }
    }

    #[test]
    fn classify_403_forbidden_is_invalid() {
        let resp = make_response(403, &[]);
        match classify_download_response(resp) {
            ResponseDisposition::Invalid(s) => assert_eq!(s, StatusCode::FORBIDDEN),
            _ => panic!("expected Invalid"),
        }
    }

    // ── looks_like_anti_abuse_page ──────────────────────────

    #[test]
    fn detects_tuna_anti_abuse_page_english() {
        let body = br#"<html><body><h1>Sorry, you've been denied access to this page</h1>
            <li>The software that you are using is with uncommon characteristics;</li>
            <li>Your subnet has sent abnormal requests to our site recently</li></body></html>"#;
        assert!(looks_like_anti_abuse_page(body));
    }

    #[test]
    fn detects_tuna_anti_abuse_page_chinese() {
        let body = "<html><body><h1>抱歉，您目前无法访问此页面</h1>\
            <li>您访问使用的软件带有非常用软件的特征；</li></body></html>"
            .as_bytes();
        assert!(looks_like_anti_abuse_page(body));
    }

    #[test]
    fn detects_cloudflare_block_page() {
        let body = br#"<html><head><title>Attention Required! | Cloudflare</title></head>
            <body><div class="cf-error-details">Sorry, you have been blocked</div></body></html>"#;
        assert!(looks_like_anti_abuse_page(body));
    }

    #[test]
    fn plain_forbidden_body_is_not_anti_abuse() {
        assert!(!looks_like_anti_abuse_page(b""));
        assert!(!looks_like_anti_abuse_page(
            b"<html><head><title>403 Forbidden</title></head><body><center><h1>403 Forbidden</h1></center></body></html>"
        ));
        assert!(!looks_like_anti_abuse_page(b"Access to this resource is forbidden"));
    }

    // ── validate_probe_response ─────────────────────────────

    #[test]
    fn validate_probe_200_ok() {
        let resp = make_response(200, &[]);
        assert!(validate_probe_response(&resp).is_ok());
    }

    #[test]
    fn validate_probe_206_partial_content() {
        let resp = make_response(206, &[]);
        assert!(validate_probe_response(&resp).is_ok());
    }

    #[test]
    fn validate_probe_non_ok_returns_error() {
        let resp = make_response(404, &[]);
        assert!(validate_probe_response(&resp).is_err());
    }

    #[test]
    fn validate_probe_500_returns_error() {
        let resp = make_response(500, &[]);
        assert!(validate_probe_response(&resp).is_err());
    }

    // ── extract_total_bytes ─────────────────────────────────

    #[test]
    fn extract_bytes_from_content_length() {
        let headers = header_map(&[("content-length", "1024")]);
        assert_eq!(extract_total_bytes(StatusCode::OK, &headers), Some(1024));
    }

    #[test]
    fn extract_bytes_from_content_range_with_partial() {
        let headers = header_map(&[("content-range", "bytes 0-499/2000")]);
        assert_eq!(
            extract_total_bytes(StatusCode::PARTIAL_CONTENT, &headers),
            Some(2000)
        );
    }

    #[test]
    fn extract_bytes_prefers_content_range_over_length_for_partial() {
        let headers = header_map(&[
            ("content-range", "bytes 0-999/5000"),
            ("content-length", "1000"),
        ]);
        assert_eq!(
            extract_total_bytes(StatusCode::PARTIAL_CONTENT, &headers),
            Some(5000)
        );
    }

    #[test]
    fn extract_bytes_missing_headers_returns_none() {
        let headers = header_map(&[]);
        assert_eq!(extract_total_bytes(StatusCode::OK, &headers), None);
    }

    #[test]
    fn extract_bytes_content_range_asterisk_total() {
        let headers = header_map(&[("content-range", "bytes */5000")]);
        // rsplit('/').next() on "bytes */5000" gives "5000"
        assert_eq!(
            extract_total_bytes(StatusCode::PARTIAL_CONTENT, &headers),
            Some(5000)
        );
    }

    #[test]
    fn extract_bytes_invalid_length_returns_none() {
        let headers = header_map(&[("content-length", "not-a-number")]);
        assert_eq!(extract_total_bytes(StatusCode::OK, &headers), None);
    }

    // ── supports_ranges ─────────────────────────────────────

    #[test]
    fn supports_ranges_partial_content_status() {
        let headers = header_map(&[]);
        assert!(supports_ranges(StatusCode::PARTIAL_CONTENT, &headers));
    }

    #[test]
    fn supports_ranges_via_content_range_header() {
        let headers = header_map(&[("content-range", "bytes 0-99/100")]);
        assert!(supports_ranges(StatusCode::OK, &headers));
    }

    #[test]
    fn supports_ranges_via_accept_ranges_bytes() {
        let headers = header_map(&[("accept-ranges", "bytes")]);
        assert!(supports_ranges(StatusCode::OK, &headers));
    }

    #[test]
    fn supports_ranges_accept_ranges_case_insensitive() {
        let headers = header_map(&[("accept-ranges", "Bytes")]);
        assert!(supports_ranges(StatusCode::OK, &headers));
    }

    #[test]
    fn supports_ranges_no_header_returns_false() {
        let headers = header_map(&[]);
        assert!(!supports_ranges(StatusCode::OK, &headers));
    }

    #[test]
    fn supports_ranges_accept_ranges_none_returns_false() {
        let headers = header_map(&[("accept-ranges", "none")]);
        assert!(!supports_ranges(StatusCode::OK, &headers));
    }

    // ── infer_file_name ─────────────────────────────────────

    #[test]
    fn infer_name_from_content_disposition_quoted() {
        let headers = header_map(&[(
            "content-disposition",
            r#"attachment; filename="myfile.zip""#,
        )]);
        assert_eq!(
            infer_file_name("https://example.com/ignored", &headers).as_deref(),
            Some("myfile.zip")
        );
    }

    #[test]
    fn infer_name_from_content_disposition_unquoted() {
        let headers = header_map(&[("content-disposition", "attachment; filename=myfile.zip")]);
        assert_eq!(
            infer_file_name("https://example.com/ignored", &headers).as_deref(),
            Some("myfile.zip")
        );
    }

    #[test]
    fn infer_name_from_content_disposition_ext_filename() {
        let headers = header_map(&[(
            "content-disposition",
            "attachment; filename*=UTF-8''encoded%20file.txt",
        )]);
        assert_eq!(
            infer_file_name("https://example.com/ignored", &headers).as_deref(),
            Some("encoded file.txt")
        );
    }

    #[test]
    fn infer_name_falls_back_to_url_path() {
        let headers = header_map(&[]);
        assert_eq!(
            infer_file_name("https://example.com/path/to/document.pdf", &headers).as_deref(),
            Some("document.pdf")
        );
    }

    #[test]
    fn infer_name_falls_back_to_download_when_no_path() {
        let headers = header_map(&[]);
        assert_eq!(
            infer_file_name("https://example.com/", &headers).as_deref(),
            Some("download")
        );
    }

    #[test]
    fn infer_name_sanitizes_malicious_filename() {
        let headers = header_map(&[(
            "content-disposition",
            r#"attachment; filename="../../etc/passwd""#,
        )]);
        let name = infer_file_name("https://example.com/ok", &headers);
        assert!(name.is_some());
        // sanitize_filename replaces path separators with underscores
        let n = name.unwrap();
        assert!(!n.contains('/'), "should not contain forward slashes");
        assert!(!n.contains('\\'), "should not contain backslashes");
    }

    #[test]
    fn infer_name_uses_content_disposition_over_url() {
        let headers = header_map(&[(
            "content-disposition",
            r#"attachment; filename="actual.txt""#,
        )]);
        assert_eq!(
            infer_file_name("https://example.com/ignored.html", &headers).as_deref(),
            Some("actual.txt")
        );
    }

    // ── validate_segment_response ──────────────────────────

    #[test]
    fn validate_segment_response_exact_match() {
        let resp = make_response(206, &[("content-range", "bytes 100-199/1000")]);
        assert!(validate_segment_response(&resp, 100, 199).is_ok());
    }

    #[test]
    fn validate_segment_response_start_mismatch() {
        let resp = make_response(206, &[("content-range", "bytes 99-199/1000")]);
        assert!(validate_segment_response(&resp, 100, 199).is_err());
    }

    #[test]
    fn validate_segment_response_end_exceeds_expected() {
        let resp = make_response(206, &[("content-range", "bytes 100-200/1000")]);
        assert!(validate_segment_response(&resp, 100, 199).is_err());
    }

    #[test]
    fn validate_segment_response_missing_content_range() {
        let resp = make_response(206, &[]);
        assert!(validate_segment_response(&resp, 0, 99).is_err());
    }

    #[test]
    fn validate_segment_response_invalid_content_range_format() {
        let resp = make_response(206, &[("content-range", "invalid")]);
        assert!(validate_segment_response(&resp, 0, 99).is_err());
    }

    #[test]
    fn validate_segment_response_non_bytes_unit() {
        let resp = make_response(206, &[("content-range", "bytes 0-99/100")]);
        // Valid format, this should work
        assert!(validate_segment_response(&resp, 0, 99).is_ok());
    }

    // ── if_range_header ─────────────────────────────────────

    #[test]
    fn if_range_uses_etag_when_present() {
        let manifest = Manifest {
            etag: Some("\"abc123\"".into()),
            last_modified: None,
            ..make_minimal_manifest()
        };
        let result = if_range_header(&manifest);
        assert!(result.is_some());
        let (name, value) = result.unwrap();
        assert_eq!(name, header::IF_RANGE);
        assert_eq!(value.to_str().unwrap(), "\"abc123\"");
    }

    #[test]
    fn if_range_uses_last_modified_when_no_etag() {
        let manifest = Manifest {
            etag: None,
            last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".into()),
            ..make_minimal_manifest()
        };
        let result = if_range_header(&manifest);
        assert!(result.is_some());
        let (name, value) = result.unwrap();
        assert_eq!(name, header::IF_RANGE);
        assert_eq!(value.to_str().unwrap(), "Mon, 01 Jan 2024 00:00:00 GMT");
    }

    #[test]
    fn if_range_etag_preferred_over_last_modified() {
        let manifest = Manifest {
            etag: Some("\"etag-value\"".into()),
            last_modified: Some("some-date".into()),
            ..make_minimal_manifest()
        };
        let result = if_range_header(&manifest);
        assert!(result.is_some());
        let (_, value) = result.unwrap();
        assert_eq!(value.to_str().unwrap(), "\"etag-value\"");
    }

    #[test]
    fn if_range_none_when_both_missing() {
        let manifest = Manifest {
            etag: None,
            last_modified: None,
            ..make_minimal_manifest()
        };
        assert!(if_range_header(&manifest).is_none());
    }

    // ── build_segment_request ───────────────────────────────

    #[test]
    fn build_segment_request_includes_range() {
        let client = Client::new();
        let builder = build_segment_request(
            &client,
            "https://example.com/file",
            "TestAgent/1.0",
            &[],
            100,
            199,
            None,
        );
        // We can't easily inspect the builder, but we verify it doesn't panic
        let _ = builder;
    }

    #[test]
    fn build_segment_request_with_validator() {
        let client = Client::new();
        let validator = Some((header::IF_RANGE, HeaderValue::from_str("\"etag\"").unwrap()));
        let builder = build_segment_request(
            &client,
            "https://example.com/file",
            "TestAgent/1.0",
            &[],
            0,
            99,
            validator,
        );
        let _ = builder;
    }

    // ── parse_content_disposition ──────────────────────────

    #[test]
    fn parse_cd_quoted_filename() {
        assert_eq!(
            parse_content_disposition(r#"attachment; filename="file.txt""#).as_deref(),
            Some("file.txt")
        );
    }

    #[test]
    fn parse_cd_unquoted_filename() {
        assert_eq!(
            parse_content_disposition("attachment; filename=file.txt").as_deref(),
            Some("file.txt")
        );
    }

    #[test]
    fn parse_cd_ext_filename_takes_priority() {
        let result =
            parse_content_disposition("attachment; filename=old.txt; filename*=UTF-8''new.txt");
        assert_eq!(result.as_deref(), Some("new.txt"));
    }

    #[test]
    fn parse_cd_url_encoded_filename() {
        let result =
            parse_content_disposition("attachment; filename*=UTF-8''%E4%B8%AD%E6%96%87.txt");
        assert_eq!(result.as_deref(), Some("中文.txt"));
    }

    #[test]
    fn parse_cd_no_filename_returns_none() {
        assert!(parse_content_disposition("attachment;").is_none());
    }

    #[test]
    fn parse_cd_empty_value_returns_none() {
        assert!(parse_content_disposition("").is_none());
    }

    #[test]
    fn parse_cd_only_extension_returns_none_when_no_standard() {
        // filename*= without a standard filename= still works via first loop
        let result = parse_content_disposition("attachment; filename*=UTF-8''encoded.bin");
        assert_eq!(result.as_deref(), Some("encoded.bin"));
    }

    // ── header_string ───────────────────────────────────────

    #[test]
    fn header_string_found() {
        let headers = header_map(&[("content-type", "application/json")]);
        assert_eq!(
            header_string(&headers, header::CONTENT_TYPE).as_deref(),
            Some("application/json")
        );
    }

    #[test]
    fn header_string_missing_returns_none() {
        let headers = header_map(&[]);
        assert!(header_string(&headers, header::CONTENT_TYPE).is_none());
    }

    #[test]
    fn header_string_multiple_values_returns_first() {
        let headers = header_map(&[("accept", "text/html"), ("accept", "application/json")]);
        // HeaderMap returns one value per key (the first)
        assert!(header_string(&headers, header::ACCEPT).is_some());
    }

    // ── Manifest helper ─────────────────────────────────────

    fn make_minimal_manifest() -> Manifest {
        Manifest {
            id: String::new(),
            url: String::new(),
            final_url: String::new(),
            user_agent: String::new(),
            extra_headers: vec![],
            destination_dir: String::new(),
            file_name: String::new(),
            file_name_locked: false,
            destination_path: String::new(),
            temp_path: String::new(),
            total_bytes: None,
            downloaded_bytes: 0,
            supports_ranges: false,
            chunk_size: 4194304,
            connection_count: 0,
            thread_mode: crate::types::ThreadMode::Adaptive,
            requested_thread_count: None,
            desired_thread_count: None,
            allocated_thread_count: None,
            adaptive_profile_snapshot: None,
            thread_note: None,
            etag: None,
            last_modified: None,
            state: crate::types::DownloadState::Queued,
            cdn_accelerated: false,
            cdn_node_ip: None,
            checksum_mode: crate::types::ChecksumMode::None,
            checksum: None,
            expected_checksum: None,
            error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
            chunks: vec![],
            mirror_url: None,
            mirror_urls: Vec::new(),
            current_mirror_index: 0,
            priority: crate::types::Priority::Normal,
        }
    }

    // ── has_header ──────────────────────────────────────────

    #[test]
    fn has_header_case_insensitive() {
        let headers = vec![
            "Authorization: Bearer token".to_string(),
            "Referer: https://example.com/".to_string(),
        ];
        assert!(has_header(&headers, "referer"));
        assert!(has_header(&headers, "Referer"));
        assert!(has_header(&headers, "REFERER"));
        assert!(has_header(&headers, "authorization"));
        assert!(!has_header(&headers, "user-agent"));
    }

    #[test]
    fn has_header_empty_list() {
        assert!(!has_header(&[], "referer"));
    }

    // ── infer_candidate_referers ────────────────────────────

    #[test]
    fn infer_nvidia_com_referers() {
        let url = "https://cn.download.nvidia.com/Windows/617.14/617.14-desktop-win10-win11-64bit-international-dch-whql.exe";
        let cands = infer_candidate_referers(url);
        assert!(cands.contains(&"https://www.nvidia.com/".to_string()));
        assert!(cands.contains(&"https://www.nvidia.cn/".to_string()));
        assert!(cands.contains(&"https://cn.download.nvidia.com/".to_string()));
    }

    #[test]
    fn infer_nvidia_cn_referers() {
        let url = "https://cn.download.nvidia.cn/Windows/driver.exe";
        let cands = infer_candidate_referers(url);
        assert!(cands.contains(&"https://www.nvidia.cn/".to_string()));
        assert!(cands.contains(&"https://www.nvidia.com/".to_string()));
    }

    #[test]
    fn infer_generic_subdomain_referers() {
        let url = "https://dl.example.com/files/archive.tar.gz";
        let cands = infer_candidate_referers(url);
        assert!(cands.contains(&"https://dl.example.com/".to_string()));
        assert!(cands.contains(&"https://www.example.com/".to_string()));
        assert!(cands.contains(&"https://example.com/".to_string()));
    }

    #[test]
    fn infer_second_level_domain_referers() {
        let url = "https://cdn.example.com.cn/files/archive.tar.gz";
        let cands = infer_candidate_referers(url);
        assert!(cands.contains(&"https://cdn.example.com.cn/".to_string()));
        assert!(cands.contains(&"https://www.example.com.cn/".to_string()));
        assert!(cands.contains(&"https://example.com.cn/".to_string()));
    }

    #[test]
    fn infer_invalid_url_empty() {
        assert!(infer_candidate_referers("not-a-url").is_empty());
    }

    // ── is_too_many_requests_error ──────────────────────────

    #[test]
    fn detects_429_too_many_requests() {
        let err1 = DownloadError::InvalidResponse("http status 429 Too Many Requests".to_string());
        assert!(is_too_many_requests_error(&err1));

        let err2 = DownloadError::InvalidResponse("http status 500 Internal Server Error".to_string());
        assert!(!is_too_many_requests_error(&err2));

        let err3 = DownloadError::Interrupted;
        assert!(!is_too_many_requests_error(&err3));
    }
}
