use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use reqwest::header::HeaderMap;

use super::types::{ChecksumEntry, MirrorResource};
use crate::types::ChecksumMode;

/// Parsed Metalink information extracted from HTTP response headers (RFC 6249 & RFC 3230/5843).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MetalinkHttpMetadata {
    /// Mirror URLs declared with `rel=duplicate`.
    pub mirrors: Vec<MirrorResource>,
    /// Checksums declared with `Digest: <algo>=<base64>`.
    pub hashes: Vec<ChecksumEntry>,
    /// External metalink document link declared with `rel=describedby`.
    pub metalink_document_url: Option<String>,
}

/// Parse Metalink mirrors and digests from HTTP response headers.
pub fn parse_metalink_headers(headers: &HeaderMap) -> MetalinkHttpMetadata {
    let mut meta = MetalinkHttpMetadata::default();

    // 1. Parse `Link` headers (RFC 6249 & RFC 5988)
    for link_val in headers.get_all(reqwest::header::LINK) {
        let Ok(link_str) = link_val.to_str() else {
            continue;
        };
        for part in link_str.split(',') {
            parse_single_link_header(part.trim(), &mut meta);
        }
    }

    // 2. Parse `Digest` header (RFC 3230 & RFC 5843)
    for digest_val in headers.get_all("digest") {
        let Ok(digest_str) = digest_val.to_str() else {
            continue;
        };
        for part in digest_str.split(',') {
            if let Some(entry) = parse_digest_item(part.trim()) {
                meta.hashes.push(entry);
            }
        }
    }

    meta
}

fn parse_single_link_header(link_text: &str, meta: &mut MetalinkHttpMetadata) {
    // Expected format: <url>; rel="duplicate"; pri=1; geo=de
    let Some(start_idx) = link_text.find('<') else {
        return;
    };
    let Some(end_idx) = link_text[start_idx + 1..].find('>') else {
        return;
    };
    let url = link_text[start_idx + 1..start_idx + 1 + end_idx].trim().to_string();
    if url.is_empty() {
        return;
    }

    let params_str = &link_text[start_idx + 1 + end_idx + 1..];
    let mut rel = None;
    let mut pri = None;
    let mut pref = false;
    let mut geo = None;
    let mut link_type = None;

    for param in params_str.split(';') {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }
        if param.eq_ignore_ascii_case("pref") {
            pref = true;
            continue;
        }
        if let Some((k, v)) = param.split_once('=') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
            match key.as_str() {
                "rel" => rel = Some(val.to_ascii_lowercase()),
                "pri" => pri = val.parse::<u32>().ok(),
                "geo" => geo = Some(val.to_ascii_lowercase()),
                "type" => link_type = Some(val.to_ascii_lowercase()),
                _ => {}
            }
        }
    }

    match rel.as_deref() {
        Some("duplicate") => {
            let mut priority = pri.unwrap_or(100).max(1);
            if pref {
                priority = priority.min(10);
            }
            meta.mirrors.push(MirrorResource {
                url,
                priority,
                location: geo,
                max_connections: None,
            });
        }
        Some("describedby")
            if link_type.as_deref() == Some("application/metalink4+xml")
                || link_type.as_deref() == Some("application/metalink+xml")
                || url.ends_with(".meta4")
                || url.ends_with(".metalink") =>
        {
            meta.metalink_document_url = Some(url);
        }
        _ => {}
    }
}

fn parse_digest_item(item: &str) -> Option<ChecksumEntry> {
    // Format: SHA-256=base64_encoded_digest or SHA-512=...
    let (algo_str, b64_str) = item.split_once('=')?;
    let algo_raw = algo_str.trim().to_ascii_lowercase();
    let (algo, raw_type) = match algo_raw.as_str() {
        "sha-256" | "sha256" => (ChecksumMode::Sha256, "sha-256".to_string()),
        "sha-512" | "sha512" => (ChecksumMode::Sha512, "sha-512".to_string()),
        _ => return None,
    };

    let b64_clean = b64_str.trim().trim_matches('"');
    let decoded = BASE64_STANDARD.decode(b64_clean).ok()?;
    let hex_hash = decoded.iter().map(|b| format!("{:02x}", b)).collect();

    Some(ChecksumEntry {
        algorithm: algo,
        raw_type,
        hash: hex_hash,
    })
}
