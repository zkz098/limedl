//! Property-based tests for parsers handling untrusted external inputs in `limedl-types`:
//! checksum manifests, `aria2c` CLI snippets, batch URL lists, and percent-decoding.

use http::{HeaderMap, HeaderName, HeaderValue};
use limedl_types::aria2_cli;
use limedl_types::checksum::{detect_checksum_from_headers, parse_sha256_file, parse_sha256sums};
use limedl_types::url_utils::{extract_batch_file_name, parse_batch_entries, percent_decode};
use proptest::prelude::*;

fn sha256_hex_strategy() -> impl Strategy<Value = String> {
    "[0-9a-fA-F]{64}"
}

fn safe_filename_strategy() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9._-]{1,32}"
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn parse_sha256_file_never_panics_on_arbitrary_utf8(
        content in "\\PC{0,512}",
        target in proptest::option::of("\\PC{0,64}"),
    ) {
        let parsed = parse_sha256_file(&content, target.as_deref());
        if let Some(hash) = parsed {
            prop_assert_eq!(hash.len(), 64);
            prop_assert!(hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        }
    }

    #[test]
    fn parse_sha256_file_survives_multibyte_chars_near_byte_64(
        prefix in "[\\u{4e00}-\\u{9fff}\\u{1f600}-\\u{1f64f}a-f0-9]{10,40}",
        suffix in "\\PC{0,64}",
        target in proptest::option::of(safe_filename_strategy()),
    ) {
        let line = format!("{prefix}  {suffix}");
        let parsed = parse_sha256_file(&line, target.as_deref());
        if let Some(hash) = parsed {
            prop_assert_eq!(hash.len(), 64);
            prop_assert!(hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        }
    }

    #[test]
    fn parse_sha256sums_roundtrips_gnu_and_bsd_lines(
        hash in sha256_hex_strategy(),
        filename in safe_filename_strategy(),
        dir_prefix in proptest::option::of("[a-zA-Z0-9_-]{1,12}/"),
        binary_flag in prop::bool::ANY,
        use_bsd in prop::bool::ANY,
    ) {
        let path = match dir_prefix {
            Some(p) => format!("{p}{filename}"),
            None => filename.clone(),
        };
        let line = if use_bsd {
            format!("SHA256 ({path}) = {hash}")
        } else if binary_flag {
            format!("{hash} *{path}")
        } else {
            format!("{hash}  {path}")
        };
        let doc = format!("# comment\n\n{line}\n");
        let parsed = parse_sha256sums(&doc, &filename);
        prop_assert_eq!(parsed, Some(hash.to_ascii_lowercase()));
    }

    #[test]
    fn detect_checksum_from_headers_never_panics_on_arbitrary_digest_headers(
        digest_val in "[\\x20-\\x7e]{0,256}",
    ) {
        let mut headers = HeaderMap::new();
        if let Ok(val) = HeaderValue::from_str(&digest_val) {
            headers.insert(HeaderName::from_static("digest"), val);
        }
        let _ = detect_checksum_from_headers(&headers);
    }

    #[test]
    fn aria2_cli_parse_never_panics_on_arbitrary_input(raw in "\\PC{0,512}") {
        let _ = aria2_cli::parse(&raw);
        let with_prefix = format!("aria2c {raw}");
        let _ = aria2_cli::parse(&with_prefix);
    }

    #[test]
    fn aria2_cli_parse_preserves_valid_urls_with_arbitrary_extra_flags(
        url in "https://[a-z0-9.-]{1,20}\\.example/[a-z0-9._-]{1,20}",
        out_name in safe_filename_strategy(),
        noise in "[a-zA-Z0-9 ='\"\\\\^\\n-]{0,80}",
    ) {
        let cmd = format!("aria2c --out \"{out_name}\" {noise} {url}");
        if let Some(parsed) = aria2_cli::parse(&cmd) {
            prop_assert!(parsed.urls.iter().any(|u| u == &url) || !parsed.urls.is_empty());
        }
    }

    #[test]
    fn batch_entries_and_percent_decode_never_panic(raw in "\\PC{0,512}") {
        let entries = parse_batch_entries(&raw);
        for entry in entries {
            prop_assert!(!entry.url.trim().is_empty());
            let _ = extract_batch_file_name(&entry.url);
        }
        let _ = percent_decode(&raw);
    }
}

#[test]
fn aria2_cli_parse_handles_multibyte_short_flag_without_panicking() {
    assert!(aria2_cli::parse("-®").is_none());
    assert!(aria2_cli::parse("aria2c -𑿏 https://example.com/a.bin").is_some());
}
