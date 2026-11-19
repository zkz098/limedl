use base64::Engine;
use serde_json::Value;

use crate::http::has_header;
use crate::jsonrpc::{ERR_INVALID_PARAMS, JsonRpcError, make_error};
use crate::types::ChecksumMode;

pub fn extract_option_str(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<String> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

pub fn extract_option_usize(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<usize> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str().and_then(|s| s.parse::<usize>().ok()))
}

pub fn extract_option_u32(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Option<u32> {
    options
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_str().and_then(|s| s.parse::<u32>().ok()))
}

/// Collect the aria2 `header` option into `"Name: value"` strings.
///
/// aria2 documents `header` as a repeatable option; over JSON-RPC it is an
/// array of strings (AriaNg, Motrix), but some clients send a single string.
/// Malformed entries without a colon are dropped.
pub fn extract_option_headers(
    options: Option<&serde_json::Map<String, Value>>,
    key: &str,
) -> Vec<String> {
    let mut headers = Vec::new();
    match options.and_then(|o| o.get(key)) {
        Some(Value::Array(items)) => {
            for item in items {
                if let Some(raw) = item.as_str() {
                    push_header(&mut headers, raw);
                }
            }
        }
        Some(Value::String(raw)) => push_header(&mut headers, raw),
        _ => {}
    }
    headers
}

pub(crate) fn push_header(headers: &mut Vec<String>, raw: &str) {
    let trimmed = raw.trim();
    if !trimmed.is_empty() && trimmed.contains(':') {
        headers.push(trimmed.to_string());
    }
}

/// aria2 serialises every boolean option as a string over JSON-RPC
/// (`"pause": "true"`); accept raw booleans too.
pub(crate) fn aria2_bool(value: &Value) -> bool {
    value.as_bool().unwrap_or_else(|| {
        value
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("true") || s == "1")
    })
}

/// Read a boolean option from an aria2 options object.
pub(crate) fn option_is_true(options: Option<&serde_json::Map<String, Value>>, key: &str) -> bool {
    options.and_then(|o| o.get(key)).is_some_and(aria2_bool)
}

/// The optional trailing arguments of an aria2 RPC call: `(uris, options,
/// position)`, each absent unless the caller sent it.
pub type Aria2Tail = (
    Option<Vec<String>>,
    Option<serde_json::Map<String, Value>>,
    Option<i64>,
);

/// Split the trailing arguments of an aria2 RPC call by JSON type.
///
/// aria2's method signatures are type-driven: `addTorrent` is
/// `(torrent[, uris[, options[, position]]])`, so `uris` may be present or
/// omitted before `options`. AriaNg sends `addTorrent` as
/// `[torrent, [], options]`, which a naive `params.get(1)` read mistakes for
/// the options object and silently drops `dir`/`out`/`pause`/`select-file`;
/// the existing tests only exercised `[torrent, options]`, so the bug stayed
/// invisible. Scanning by type accepts both shapes and any future mix.
///
/// `from` is the index of the first optional argument (1 for both `addUri`
/// and `addTorrent`, whose first element is the URI list / torrent payload).
/// Returns the first URI array, the first options object and the first
/// integer position, each optional.
pub fn split_aria2_tail(params: &[Value], from: usize) -> Aria2Tail {
    let mut uris = None;
    let mut options = None;
    let mut position = None;
    for value in params.iter().skip(from) {
        match value {
            Value::Array(items) if uris.is_none() => {
                uris = Some(
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                );
            }
            Value::Object(map) if options.is_none() => options = Some(map.clone()),
            Value::Number(n) if position.is_none() => position = n.as_i64(),
            _ => {}
        }
    }
    (uris, options, position)
}

/// Parse aria2's `select-file`: a 1-based comma-separated index list, into the
/// engine's 0-based file indices.
pub fn parse_select_file(value: &Value) -> std::result::Result<Vec<usize>, JsonRpcError> {
    let raw = value.as_str().ok_or_else(|| {
        make_error(
            ERR_INVALID_PARAMS,
            "select-file must be a comma-separated string",
        )
    })?;

    let mut indices = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let index: usize = part.parse().map_err(|_| {
            make_error(
                ERR_INVALID_PARAMS,
                format!("invalid select-file index: {part}"),
            )
        })?;
        if index == 0 {
            return Err(make_error(
                ERR_INVALID_PARAMS,
                "select-file indices are 1-based",
            ));
        }
        indices.push(index - 1);
    }
    Ok(indices)
}

/// Parse an aria2 byte-rate limit. `""` and `"0"` mean "unlimited".
pub fn parse_aria2_limit(value: &Value) -> std::result::Result<Option<u64>, JsonRpcError> {
    let raw = match value {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => {
            return Err(make_error(
                ERR_INVALID_PARAMS,
                "limit must be a number or a string",
            ));
        }
    };
    if raw.is_empty() || raw == "0" {
        return Ok(None);
    }
    raw.parse::<u64>()
        .map(Some)
        .map_err(|_| make_error(ERR_INVALID_PARAMS, format!("invalid limit: {raw}")))
}

/// Build the effective extra-header list from aria2 request options.
///
/// Maps `header` (array/string), `referer` (`*` means the download URL
/// itself) and `http-user`/`http-passwd` (Basic auth) onto the header list
/// consumed by `StartDownloadRequest::headers`.
pub fn collect_request_headers(
    options: Option<&serde_json::Map<String, Value>>,
    url: &str,
) -> Vec<String> {
    let mut headers = extract_option_headers(options, "header");

    if let Some(referer) = extract_option_str(options, "referer") {
        let referer = referer.trim();
        if !referer.is_empty() && !has_header(&headers, "referer") {
            let value = if referer == "*" { url } else { referer };
            headers.push(format!("Referer: {value}"));
        }
    }

    if let Some(user) = extract_option_str(options, "http-user") {
        let user = user.trim();
        if !user.is_empty() && !has_header(&headers, "authorization") {
            let password = extract_option_str(options, "http-passwd").unwrap_or_default();
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
            headers.push(format!("Authorization: Basic {encoded}"));
        }
    }

    headers
}

/// Parse the aria2 `checksum` option (`"TYPE=DIGEST"`, e.g.
/// `sha-256=abcdef…`) into limedl checksum fields.
///
/// A well-formed request for an algorithm limedl cannot verify (SHA-1 and MD5
/// are the ones aria2 clients actually send) is rejected instead of silently
/// starting an unverified download: reporting success while skipping the check
/// is worse than failing the call. Malformed values are still ignored, matching
/// aria2's tolerance for optional metadata.
pub fn parse_checksum_option(
    options: Option<&serde_json::Map<String, Value>>,
) -> Result<(Option<ChecksumMode>, Option<String>), String> {
    let Some(raw) = extract_option_str(options, "checksum") else {
        return Ok((None, None));
    };
    let Some((hash_type, digest)) = raw.trim().split_once('=') else {
        return Ok((None, None));
    };
    let hash_type = hash_type.trim().to_ascii_lowercase();
    let mode = match hash_type.as_str() {
        "sha-256" | "sha256" => ChecksumMode::Sha256,
        "sha-512" | "sha512" => ChecksumMode::Sha512,
        "blake3" => ChecksumMode::Blake3,
        other => {
            return Err(format!(
                "unsupported checksum type '{other}': limedl verifies blake3, sha-256 and sha-512"
            ));
        }
    };
    let digest = digest.trim().to_ascii_lowercase();
    if digest.is_empty() {
        return Ok((None, None));
    }
    Ok((Some(mode), Some(digest)))
}

/// Case-insensitive lookup of a `"Name: value"` entry's value.
pub fn find_header_value(headers: &[String], name: &str) -> Option<String> {
    headers.iter().find_map(|header| {
        let (header_name, value) = header.split_once(':')?;
        header_name
            .trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}
