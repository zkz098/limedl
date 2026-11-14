#![no_main]

use arbitrary::Arbitrary;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use libfuzzer_sys::fuzz_target;
use limedl_http::http::{
    extract_total_bytes, has_header, infer_candidate_referers, infer_file_name, supports_ranges,
};

#[derive(Arbitrary, Debug)]
enum DispositionMode {
    None,
    FilenameQuoted,
    FilenameUnquoted,
    FilenameStarUtf8,
    Raw(String),
}

#[derive(Arbitrary, Debug)]
enum ContentRangeMode {
    None,
    Valid {
        start: u64,
        end: u64,
        total: Option<u64>,
    },
    Raw(String),
}

#[derive(Arbitrary, Debug)]
struct StructuredHttpInput {
    disposition: DispositionMode,
    filename: String,
    content_range: ContentRangeMode,
    content_length: Option<String>,
    accept_ranges: Option<String>,
    url: String,
    extra_headers: Vec<String>,
    lookup_header: String,
}

#[derive(Arbitrary, Debug)]
enum FuzzInput {
    Structured(StructuredHttpInput),
    Raw(Vec<u8>),
}

fuzz_target!(|input: FuzzInput| {
    match input {
        FuzzInput::Structured(s) => {
            let mut headers = HeaderMap::new();
            let cd = match s.disposition {
                DispositionMode::None => None,
                DispositionMode::FilenameQuoted => {
                    Some(format!("attachment; filename=\"{}\"", s.filename))
                }
                DispositionMode::FilenameUnquoted => {
                    Some(format!("attachment; filename={}", s.filename))
                }
                DispositionMode::FilenameStarUtf8 => {
                    Some(format!("attachment; filename*=UTF-8''{}", s.filename))
                }
                DispositionMode::Raw(raw) => Some(raw),
            };
            if let Some(cd_str) = cd
                && let Ok(val) = HeaderValue::from_str(&cd_str)
            {
                headers.insert(header::CONTENT_DISPOSITION, val);
            }

            let cr = match s.content_range {
                ContentRangeMode::None => None,
                ContentRangeMode::Valid { start, end, total } => {
                    let total_str = total.map_or_else(|| "*".to_string(), |t| t.to_string());
                    Some(format!("bytes {start}-{end}/{total_str}"))
                }
                ContentRangeMode::Raw(raw) => Some(raw),
            };
            if let Some(cr_str) = cr
                && let Ok(val) = HeaderValue::from_str(&cr_str)
            {
                headers.insert(header::CONTENT_RANGE, val);
            }

            if let Some(cl) = s.content_length
                && let Ok(val) = HeaderValue::from_str(&cl)
            {
                headers.insert(header::CONTENT_LENGTH, val);
            }
            if let Some(ar) = s.accept_ranges
                && let Ok(val) = HeaderValue::from_str(&ar)
            {
                headers.insert(header::ACCEPT_RANGES, val);
            }

            if let Some(name) = infer_file_name(&s.url, &headers) {
                assert!(!name.is_empty());
                assert!(!name.contains('/'));
                assert!(!name.contains('\\'));
                assert!(!name.contains('\0'));
            }

            for status in [StatusCode::OK, StatusCode::PARTIAL_CONTENT] {
                let _ = extract_total_bytes(status, &headers);
                let _ = supports_ranges(status, &headers);
            }

            let _ = infer_candidate_referers(&s.url);
            let _ = has_header(&s.extra_headers, &s.lookup_header);
        }
        FuzzInput::Raw(data) => {
            let Ok(input) = std::str::from_utf8(&data) else {
                return;
            };
            let (a, b) = input.split_once('\0').unwrap_or((input, input));
            let mut headers = HeaderMap::new();
            if let Ok(val) = HeaderValue::from_str(a) {
                headers.insert(header::CONTENT_DISPOSITION, val.clone());
                headers.insert(header::CONTENT_RANGE, val.clone());
                headers.insert(header::CONTENT_LENGTH, val.clone());
                headers.insert(header::ACCEPT_RANGES, val);
            }
            if let Some(name) = infer_file_name(b, &headers) {
                assert!(!name.is_empty());
                assert!(!name.contains('/'));
                assert!(!name.contains('\\'));
                assert!(!name.contains('\0'));
            }
            for status in [StatusCode::OK, StatusCode::PARTIAL_CONTENT] {
                let _ = extract_total_bytes(status, &headers);
                let _ = supports_ranges(status, &headers);
            }
            let _ = infer_candidate_referers(b);
        }
    }
});
