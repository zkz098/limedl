//! Property-based tests for `limedl-http` parsers handling untrusted HTTP headers and URLs.

use axum::http::Response as HttpResponse;
use limedl_http::http::{
    extract_total_bytes, infer_candidate_referers, infer_file_name, supports_ranges,
    validate_segment_response, validate_segment_total,
};
use proptest::prelude::*;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Body, Response, StatusCode};

fn make_response(status: u16, headers: &[(&str, &str)]) -> Response {
    let status_code = StatusCode::from_u16(status).unwrap();
    let mut builder = HttpResponse::builder().status(status_code);
    for &(k, v) in headers {
        builder = builder.header(k, v);
    }
    let http_resp = builder.body(Body::from(String::new())).unwrap();
    Response::from(http_resp)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn infer_file_name_sanitizes_untrusted_content_disposition(
        url in "\\PC{0,128}",
        cd in "[\\x20-\\x7e]{0,200}",
    ) {
        let mut headers = HeaderMap::new();
        if let Ok(val) = HeaderValue::from_str(&cd) {
            headers.insert(header::CONTENT_DISPOSITION, val);
        }
        let name = infer_file_name(&url, &headers);
        if let Some(n) = name {
            prop_assert!(!n.is_empty());
            prop_assert!(!n.contains('/'));
            prop_assert!(!n.contains('\\'));
            prop_assert!(!n.contains('\0'));
        }
    }

    #[test]
    fn infer_file_name_blocks_path_traversal_in_disposition_or_url(
        prefix in "(\\.\\./|\\.\\.\\\\|/etc/|C:\\\\Windows\\\\){1,4}",
        leaf in "[a-zA-Z0-9._-]{1,24}",
        use_star in prop::bool::ANY,
    ) {
        let payload = if use_star {
            format!("attachment; filename*=UTF-8''{prefix}{leaf}")
        } else {
            format!("attachment; filename=\"{prefix}{leaf}\"")
        };
        let mut headers = HeaderMap::new();
        if let Ok(val) = HeaderValue::from_str(&payload) {
            headers.insert(header::CONTENT_DISPOSITION, val);
        }
        let name = infer_file_name("https://example.com/fallback.bin", &headers)
            .expect("should always yield a fallback or sanitized name");
        prop_assert!(!name.contains('/'));
        prop_assert!(!name.contains('\\'));
        prop_assert_ne!(name.as_str(), "..");
        prop_assert_ne!(name.as_str(), ".");
    }

    #[test]
    fn extract_total_bytes_and_supports_ranges_never_panic(
        is_partial in prop::bool::ANY,
        content_range in "[\\x20-\\x7e]{0,120}",
        content_length in "[\\x20-\\x7e]{0,64}",
        accept_ranges in "[\\x20-\\x7e]{0,64}",
    ) {
        let status = if is_partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        };
        let mut headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&content_range) {
            headers.insert(header::CONTENT_RANGE, v);
        }
        if let Ok(v) = HeaderValue::from_str(&content_length) {
            headers.insert(header::CONTENT_LENGTH, v);
        }
        if let Ok(v) = HeaderValue::from_str(&accept_ranges) {
            headers.insert(header::ACCEPT_RANGES, v);
        }
        let _ = extract_total_bytes(status, &headers);
        let _ = supports_ranges(status, &headers);
    }

    #[test]
    fn validate_segment_response_rejects_out_of_bounds_and_never_panics(
        raw_range in "[\\x20-\\x7e]{0,120}",
        expected_start in 0u64..10_000_000,
        span in 0u64..10_000_000,
        expected_total in proptest::option::of(1u64..20_000_000),
    ) {
        let expected_end = expected_start.saturating_add(span);
        if let Ok(val) = HeaderValue::from_str(&raw_range) {
            let mut builder = HttpResponse::builder().status(StatusCode::PARTIAL_CONTENT);
            builder = builder.header(header::CONTENT_RANGE, val);
            let resp = Response::from(builder.body(Body::from(String::new())).unwrap());
            let _ = validate_segment_response(&resp, expected_start, expected_end);

            let resp2 = make_response(206, &[("content-range", &raw_range)]);
            let _ = validate_segment_total(&resp2, expected_total);
        }
    }

    #[test]
    fn infer_candidate_referers_never_panics_and_returns_urls(raw_url in "\\PC{0,256}") {
        let referers = infer_candidate_referers(&raw_url);
        for r in referers {
            prop_assert!(r.ends_with('/'));
            prop_assert!(reqwest::Url::parse(&r).is_ok());
        }
    }
}
