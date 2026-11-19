use std::ffi::OsStr;
use std::path::Path;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use http::header::{self, HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use url::Url;

use crate::types::ChecksumMode;

const MAX_CHECKSUM_FILE_SIZE: usize = 512 * 1024; // 512 KiB limit

/// Validate if a string is a valid 64-character hexadecimal SHA-256 digest.
pub fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Extract the file basename from a path or relative path string.
fn extract_basename(path_str: &str) -> &str {
    let s = path_str
        .trim()
        .trim_start_matches("./")
        .trim_start_matches(".\\");
    let after_slash = s.rsplit(['/', '\\']).next().unwrap_or(s);
    after_slash.trim()
}

/// Parse a single-file checksum document (e.g. `.sha256` or `.sha256sum`).
///
/// Handles:
/// - Bare 64-hex hash (with optional whitespace/newlines)
/// - Standard GNU format: `<hash>  <filename>` or `<hash> *<filename>`
/// - BSD format: `SHA256 (<filename>) = <hash>`
pub fn parse_sha256_file(content: &str, target_file_name: Option<&str>) -> Option<String> {
    let trimmed = content.trim();
    if is_valid_sha256_hex(trimmed) {
        return Some(trimmed.to_ascii_lowercase());
    }

    let target_base = target_file_name.map(extract_basename);

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("-----")
            || line.starts_with("Hash:")
        {
            continue;
        }

        // Check if line is just 64 hex chars
        if is_valid_sha256_hex(line) {
            return Some(line.to_ascii_lowercase());
        }

        // Format: SHA256 (<file>) = <hash> or SHA256(<file>)= <hash>
        if let Some(hash) = parse_bsd_line(line, target_base) {
            return Some(hash);
        }

        // Format: <hash> [* ]<filename>
        if let Some(hash) = parse_gnu_line(line, target_base) {
            return Some(hash);
        }
    }

    // Fallback: If target_file_name is provided or not, but there's a line with a 64-hex word at start
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("-----") {
            continue;
        }
        if let Some((candidate, remainder)) = line.split_at_checked(64)
            && is_valid_sha256_hex(candidate)
        {
            if target_base.is_none() {
                return Some(candidate.to_ascii_lowercase());
            }
            let rest = remainder.trim_start_matches([' ', '*', '\t']);
            if rest.is_empty() || extract_basename(rest).eq_ignore_ascii_case(target_base.unwrap())
            {
                return Some(candidate.to_ascii_lowercase());
            }
        }
    }

    None
}

/// Parse a multi-file checksum manifest document (e.g. `SHA256SUMS`, `sha256sums.txt`).
///
/// Matches against `target_file_name`.
pub fn parse_sha256sums(content: &str, target_file_name: &str) -> Option<String> {
    let target_base = extract_basename(target_file_name);

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("-----")
            || line.starts_with("Hash:")
        {
            continue;
        }

        // Format: SHA256 (<file>) = <hash>
        if let Some(hash) = parse_bsd_line(line, Some(target_base)) {
            return Some(hash);
        }

        // Format: <hash> [* ]<filename>
        if let Some(hash) = parse_gnu_line(line, Some(target_base)) {
            return Some(hash);
        }
    }

    None
}

fn parse_bsd_line(line: &str, target_base: Option<&str>) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    if !lower.starts_with("sha256") && !lower.starts_with("sha-256") {
        return None;
    }

    let paren_start = line.find('(')?;
    let paren_end = line.find(')')?;
    if paren_start >= paren_end {
        return None;
    }

    let file_in_paren = &line[paren_start + 1..paren_end];
    let eq_pos = line[paren_end..].find('=')? + paren_end;
    let hash_part = line[eq_pos + 1..].trim();

    if is_valid_sha256_hex(hash_part) {
        if let Some(target) = target_base {
            if extract_basename(file_in_paren).eq_ignore_ascii_case(target) {
                return Some(hash_part.to_ascii_lowercase());
            }
        } else {
            return Some(hash_part.to_ascii_lowercase());
        }
    }

    None
}

fn parse_gnu_line(line: &str, target_base: Option<&str>) -> Option<String> {
    // A byte length check is not enough: byte 64 can fall inside a multi-byte
    // character, and slicing there panics. `split_at_checked` returns `None`
    // unless both halves land on UTF-8 boundaries.
    let (hash_part, remainder) = line.split_at_checked(64)?;
    if !is_valid_sha256_hex(hash_part) {
        return None;
    }

    if remainder.is_empty() {
        if target_base.is_none() {
            return Some(hash_part.to_ascii_lowercase());
        }
        return None;
    }

    // Next character must be space or tab
    if !remainder.starts_with(' ') && !remainder.starts_with('\t') {
        return None;
    }

    let filename_part = remainder.trim_start_matches([' ', '*', '\t']);
    if let Some(target) = target_base {
        if extract_basename(filename_part).eq_ignore_ascii_case(target) {
            return Some(hash_part.to_ascii_lowercase());
        }
    } else {
        return Some(hash_part.to_ascii_lowercase());
    }

    None
}

/// Generate candidate checksum URLs to probe for a given target URL.
pub fn generate_candidate_urls(target_url: &str) -> Vec<CandidateUrl> {
    let mut candidates = Vec::new();
    let Ok(parsed) = Url::parse(target_url) else {
        return candidates;
    };

    let path = parsed.path();
    let path_without_trailing_slash = path.trim_end_matches('/');
    let filename = Path::new(path_without_trailing_slash)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("");

    if filename.is_empty() {
        return candidates;
    }

    // 1. Direct single-file candidates: {url}.sha256, {url}.sha256sum
    let mut direct_sha256 = parsed.clone();
    direct_sha256.set_path(&format!("{path}.sha256"));
    candidates.push(CandidateUrl {
        url: direct_sha256.to_string(),
        is_manifest: false,
    });

    let mut direct_sha256sum = parsed.clone();
    direct_sha256sum.set_path(&format!("{path}.sha256sum"));
    candidates.push(CandidateUrl {
        url: direct_sha256sum.to_string(),
        is_manifest: false,
    });

    // If target URL has query parameters, also try stripping query params for direct checksum
    if parsed.query().is_some() {
        let mut clean_sha256 = parsed.clone();
        clean_sha256.set_query(None);
        clean_sha256.set_path(&format!("{path_without_trailing_slash}.sha256"));
        candidates.push(CandidateUrl {
            url: clean_sha256.to_string(),
            is_manifest: false,
        });
    }

    // 2. Directory manifest candidates: {parent_dir}/SHA256SUMS, etc.
    let parent_path = match path_without_trailing_slash.rfind('/') {
        Some(pos) => &path_without_trailing_slash[..=pos],
        None => "/",
    };

    let manifest_names = [
        "SHA256SUMS",
        "sha256sums.txt",
        "SHA256SUMS.txt",
        "sha256sums",
        "checksums.txt",
        "checksums.sha256",
    ];

    for name in manifest_names {
        let mut manifest_url = parsed.clone();
        manifest_url.set_query(None);
        manifest_url.set_path(&format!("{parent_path}{name}"));
        candidates.push(CandidateUrl {
            url: manifest_url.to_string(),
            is_manifest: true,
        });
    }

    candidates
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateUrl {
    pub url: String,
    pub is_manifest: bool,
}

/// Apply extra headers string array (`"Key: Value"`) to an HTTP header map.
fn build_headers(user_agent: &str, extra_headers: &[String]) -> HeaderMap {
    let mut map = HeaderMap::new();
    if let Ok(val) = HeaderValue::from_str(user_agent) {
        map.insert(header::USER_AGENT, val);
    }

    for header_str in extra_headers {
        if let Some((name, val)) = header_str.split_once(':') {
            let name = name.trim();
            let val = val.trim();
            if let (Ok(header_name), Ok(header_val)) = (
                HeaderName::from_bytes(name.as_bytes()),
                HeaderValue::from_str(val),
            ) {
                map.insert(header_name, header_val);
            }
        }
    }

    map
}

/// Proactively probe candidate checksum files and return the matched SHA-256 hash if found.
pub async fn detect_sha256(
    client: &Client,
    target_url: &str,
    file_name: &str,
    user_agent: &str,
    extra_headers: &[String],
) -> Option<String> {
    let candidates = generate_candidate_urls(target_url);
    if candidates.is_empty() {
        return None;
    }

    let headers = build_headers(user_agent, extra_headers);

    for candidate in candidates {
        let resp = match client
            .get(&candidate.url)
            .headers(headers.clone())
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };

        // If response is excessively large, skip
        if let Some(len) = resp.content_length()
            && len > MAX_CHECKSUM_FILE_SIZE as u64
        {
            continue;
        }

        let content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        let Ok(text) = resp.text().await else {
            continue;
        };

        if text.len() > MAX_CHECKSUM_FILE_SIZE {
            continue;
        }

        // Avoid false positives on HTML 404/login pages
        if content_type.contains("text/html") {
            let trimmed = text.trim_start();
            if trimmed.starts_with("<!DOCTYPE")
                || trimmed.starts_with("<!doctype")
                || trimmed.starts_with("<html")
                || trimmed.starts_with("<HTML")
            {
                continue;
            }
        }

        if candidate.is_manifest {
            if let Some(hash) = parse_sha256sums(&text, file_name) {
                return Some(hash);
            }
        } else if let Some(hash) = parse_sha256_file(&text, Some(file_name)) {
            return Some(hash);
        }
    }

    None
}

/// Parse a checksum declared directly in HTTP response headers.
///
/// Supported headers:
/// - `Digest` (RFC 3230 & RFC 5843): `SHA-256=<base64>`, `SHA-512=<base64>`, etc.
/// - `Repr-Digest` (RFC 9530): `sha-256=:<base64>:`, `sha-512=:<base64>:`, etc.
/// - `x-amz-checksum-sha256` / `x-amz-checksum-sha512` (AWS S3)
/// - `x-checksum-sha256` / `x-checksum-sha512` / `x-checksum-blake3`
/// - `x-goog-hash`: `sha256=<base64>` (Google Cloud Storage)
pub fn detect_checksum_from_headers(headers: &HeaderMap) -> Option<(ChecksumMode, String)> {
    let mut candidates: Vec<(ChecksumMode, String)> = Vec::new();

    // 1. Digest & Repr-Digest (RFC 3230, RFC 5843, RFC 9530)
    for header_name in ["digest", "repr-digest"] {
        for header_val in headers.get_all(header_name) {
            let Ok(header_str) = header_val.to_str() else {
                continue;
            };
            for part in header_str.split(',') {
                if let Some(entry) = parse_digest_item(part.trim()) {
                    candidates.push(entry);
                }
            }
        }
    }

    // 2. AWS S3 headers (base64 encoded)
    if let Some(v) = headers
        .get("x-amz-checksum-sha512")
        .and_then(|h| h.to_str().ok())
        && let Some(hex) = decode_b64_to_hex(v, 64)
    {
        candidates.push((ChecksumMode::Sha512, hex));
    }
    if let Some(v) = headers
        .get("x-amz-checksum-sha256")
        .and_then(|h| h.to_str().ok())
        && let Some(hex) = decode_b64_to_hex(v, 32)
    {
        candidates.push((ChecksumMode::Sha256, hex));
    }

    // 3. Google Cloud Storage x-goog-hash header (can be multiple entries, comma or separate headers)
    for header_val in headers.get_all("x-goog-hash") {
        let Ok(header_str) = header_val.to_str() else {
            continue;
        };
        for part in header_str.split(',') {
            let part = part.trim();
            if let Some((k, v)) = part.split_once('=')
                && k.trim().eq_ignore_ascii_case("sha256")
                && let Some(hex) = decode_b64_to_hex(v.trim(), 32)
            {
                candidates.push((ChecksumMode::Sha256, hex));
            }
        }
    }

    // Priority order: SHA-256 > SHA-512 > BLAKE3
    candidates.sort_by_key(|(mode, _)| match mode {
        ChecksumMode::Sha256 => 0,
        ChecksumMode::Sha512 => 1,
        ChecksumMode::Blake3 => 2,
        ChecksumMode::None => 3,
    });

    candidates.into_iter().next()
}

fn decode_b64_to_hex(b64_str: &str, expected_bytes: usize) -> Option<String> {
    let clean = b64_str.trim().trim_matches('"').trim_matches(':');
    let decoded = BASE64_STANDARD.decode(clean).ok()?;
    if decoded.len() == expected_bytes {
        Some(decoded.iter().map(|b| format!("{:02x}", b)).collect())
    } else {
        None
    }
}

fn normalize_hex_hash(s: &str, expected_hex_len: usize) -> Option<String> {
    let clean = s.trim().trim_matches('"').to_ascii_lowercase();
    if clean.len() == expected_hex_len && clean.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(clean)
    } else {
        None
    }
}

fn parse_digest_item(item: &str) -> Option<(ChecksumMode, String)> {
    let (algo_str, raw_val) = item.split_once('=')?;
    let algo_raw = algo_str.trim().to_ascii_lowercase();
    let (algo, expected_bytes) = match algo_raw.as_str() {
        "sha-256" | "sha256" => (ChecksumMode::Sha256, 32),
        "sha-512" | "sha512" => (ChecksumMode::Sha512, 64),
        "blake3" | "b3" => (ChecksumMode::Blake3, 32),
        _ => return None,
    };

    let clean = raw_val.trim().trim_matches('"').trim_matches(':');
    // First try base64
    if let Some(hex) = decode_b64_to_hex(clean, expected_bytes) {
        return Some((algo, hex));
    }
    // Fallback: server may pass raw hex string
    if let Some(hex) = normalize_hex_hash(clean, expected_bytes * 2) {
        return Some((algo, hex));
    }

    None
}

#[cfg(test)]
mod tests;
