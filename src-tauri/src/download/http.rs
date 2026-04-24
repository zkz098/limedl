use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client, Response, StatusCode, Url,
};

use super::{
    error::{DownloadError, Result},
    manifest::Manifest,
};

pub(super) enum ResponseDisposition {
    Use(Response),
    Retryable(StatusCode),
    Invalid(StatusCode),
}

pub(super) fn classify_download_response(response: Response) -> ResponseDisposition {
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

pub(super) fn validate_probe_response(response: &Response) -> Result<()> {
    let status = response.status();
    if status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT {
        return Ok(());
    }

    Err(DownloadError::InvalidResponse(format!(
        "probe returned http status {status}"
    )))
}

pub(super) fn extract_total_bytes(status: StatusCode, headers: &HeaderMap) -> Option<u64> {
    if status == StatusCode::PARTIAL_CONTENT {
        if let Some(content_range) = header_string(headers, header::CONTENT_RANGE) {
            return content_range
                .rsplit('/')
                .next()
                .and_then(|value| value.parse::<u64>().ok());
        }
    }
    header_string(headers, header::CONTENT_LENGTH).and_then(|value| value.parse::<u64>().ok())
}

pub(super) fn supports_ranges(status: StatusCode, headers: &HeaderMap) -> bool {
    if status == StatusCode::PARTIAL_CONTENT || headers.contains_key(header::CONTENT_RANGE) {
        return true;
    }
    header_string(headers, header::ACCEPT_RANGES)
        .map(|value| value.eq_ignore_ascii_case("bytes"))
        .unwrap_or(false)
}

pub(super) fn infer_file_name(final_url: &str, headers: &HeaderMap) -> Option<String> {
    if let Some(header) = headers.get(header::CONTENT_DISPOSITION) {
        if let Ok(value) = header.to_str() {
            if let Some(decoded) = parse_content_disposition(value) {
                let clean = sanitize_filename::sanitize(decoded);
                if !clean.is_empty() {
                    return Some(clean);
                }
            }
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

pub(super) fn validate_segment_response(
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

pub(super) fn if_range_header(manifest: &Manifest) -> Option<(header::HeaderName, HeaderValue)> {
    manifest
        .etag
        .as_deref()
        .or(manifest.last_modified.as_deref())
        .and_then(|value| HeaderValue::from_str(value).ok())
        .map(|value| (header::IF_RANGE, value))
}

pub(super) fn build_segment_request(
    client: &Client,
    url: &str,
    start: u64,
    end: u64,
    validator: Option<(header::HeaderName, HeaderValue)>,
) -> reqwest::RequestBuilder {
    let mut builder = client
        .get(url)
        .header(header::RANGE, format!("bytes={start}-{end}"));
    if let Some((name, value)) = validator {
        builder = builder.header(name, value);
    }
    builder
}

fn parse_content_disposition(value: &str) -> Option<String> {
    for part in value.split(';').map(str::trim) {
        if let Some(rest) = part.strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            let encoded = rest.split("''").nth(1).unwrap_or(rest);
            if let Ok(decoded) = urlencoding::decode(encoded) {
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

pub(super) fn header_string(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}
