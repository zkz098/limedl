use std::path::Path;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Url};

const MAX_CHECKSUM_FILE_SIZE: usize = 512 * 1024; // 512 KiB limit

/// Validate if a string is a valid 64-character hexadecimal SHA-256 digest.
pub fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Extract the file basename from a path or relative path string.
fn extract_basename(path_str: &str) -> &str {
    let s = path_str.trim().trim_start_matches("./").trim_start_matches(".\\");
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
        if line.is_empty() || line.starts_with('#') || line.starts_with("-----") || line.starts_with("Hash:") {
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
        if line.len() >= 64 {
            let candidate = &line[0..64];
            if is_valid_sha256_hex(candidate) {
                if target_base.is_none() {
                    return Some(candidate.to_ascii_lowercase());
                }
                let rest = line[64..].trim_start_matches([' ', '*', '\t']);
                if rest.is_empty() || extract_basename(rest).eq_ignore_ascii_case(target_base.unwrap()) {
                    return Some(candidate.to_ascii_lowercase());
                }
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
        if line.is_empty() || line.starts_with('#') || line.starts_with("-----") || line.starts_with("Hash:") {
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
    if line.len() < 64 {
        return None;
    }

    let hash_part = &line[0..64];
    if !is_valid_sha256_hex(hash_part) {
        return None;
    }

    let remainder = &line[64..];
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
        .and_then(|n| n.to_str())
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

/// Apply extra headers string array (`"Key: Value"`) to a reqwest HeaderMap.
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const SAMPLE_HASH_UPPER: &str = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
    const OTHER_HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn test_is_valid_sha256_hex() {
        assert!(is_valid_sha256_hex(SAMPLE_HASH));
        assert!(is_valid_sha256_hex(SAMPLE_HASH_UPPER));
        assert!(!is_valid_sha256_hex("short"));
        assert!(!is_valid_sha256_hex(&format!("{SAMPLE_HASH}extra")));
        assert!(!is_valid_sha256_hex("g".repeat(64).as_str()));
    }

    #[test]
    fn test_parse_single_raw_hash() {
        assert_eq!(
            parse_sha256_file(SAMPLE_HASH, Some("file.iso")),
            Some(SAMPLE_HASH.to_string())
        );
        assert_eq!(
            parse_sha256_file(&format!("  {SAMPLE_HASH_UPPER}\n"), Some("file.iso")),
            Some(SAMPLE_HASH.to_string())
        );
    }

    #[test]
    fn test_parse_gnu_sha256sum() {
        let content = format!("{SAMPLE_HASH}  ubuntu-24.04.iso\n{OTHER_HASH}  debian.iso\n");
        assert_eq!(
            parse_sha256_file(&content, Some("ubuntu-24.04.iso")),
            Some(SAMPLE_HASH.to_string())
        );
        assert_eq!(
            parse_sha256_file(&content, Some("debian.iso")),
            Some(OTHER_HASH.to_string())
        );
        assert_eq!(
            parse_sha256_file(&content, Some("nonexistent.iso")),
            None
        );
    }

    #[test]
    fn test_parse_gnu_binary_mode() {
        let content = format!("{SAMPLE_HASH} *ubuntu-24.04.iso\n");
        assert_eq!(
            parse_sha256_file(&content, Some("ubuntu-24.04.iso")),
            Some(SAMPLE_HASH.to_string())
        );
    }

    #[test]
    fn test_parse_bsd_format() {
        let content = format!("SHA256 (ubuntu-24.04.iso) = {SAMPLE_HASH}\n");
        assert_eq!(
            parse_sha256_file(&content, Some("ubuntu-24.04.iso")),
            Some(SAMPLE_HASH.to_string())
        );

        let content2 = format!("SHA256(./dist/app.zip) = {OTHER_HASH}\n");
        assert_eq!(
            parse_sha256_file(&content2, Some("app.zip")),
            Some(OTHER_HASH.to_string())
        );
    }

    #[test]
    fn test_parse_pgp_signed_sha256sums() {
        let content = format!(
            "-----BEGIN PGP SIGNED MESSAGE-----\n\
             Hash: SHA512\n\n\
             {SAMPLE_HASH}  ubuntu-24.04.iso\n\
             {OTHER_HASH}  ubuntu-24.04-server.iso\n\
             -----BEGIN PGP SIGNATURE-----\n\
             Version: GnuPG v1\n\
             ...\n\
             -----END PGP SIGNATURE-----\n"
        );
        assert_eq!(
            parse_sha256sums(&content, "ubuntu-24.04.iso"),
            Some(SAMPLE_HASH.to_string())
        );
        assert_eq!(
            parse_sha256sums(&content, "ubuntu-24.04-server.iso"),
            Some(OTHER_HASH.to_string())
        );
        assert_eq!(parse_sha256sums(&content, "unknown.iso"), None);
    }

    #[test]
    fn test_generate_candidate_urls() {
        let url = "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso";
        let candidates = generate_candidate_urls(url);
        let urls: Vec<_> = candidates.into_iter().map(|c| c.url).collect();

        assert!(urls.contains(&"https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso.sha256".to_string()));
        assert!(urls.contains(&"https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso.sha256sum".to_string()));
        assert!(urls.contains(&"https://releases.ubuntu.com/24.04/SHA256SUMS".to_string()));
        assert!(urls.contains(&"https://releases.ubuntu.com/24.04/sha256sums.txt".to_string()));
    }

    #[test]
    fn test_generate_candidate_urls_with_query() {
        let url = "https://example.com/downloads/package.tar.gz?token=secret123";
        let candidates = generate_candidate_urls(url);
        let urls: Vec<_> = candidates.into_iter().map(|c| c.url).collect();

        assert!(urls.contains(&"https://example.com/downloads/package.tar.gz.sha256?token=secret123".to_string()));
        assert!(urls.contains(&"https://example.com/downloads/package.tar.gz.sha256".to_string()));
        assert!(urls.contains(&"https://example.com/downloads/SHA256SUMS".to_string()));
    }
}
