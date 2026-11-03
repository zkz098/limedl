#![no_main]

use http::header::{HeaderMap, HeaderValue, LINK};
use libfuzzer_sys::fuzz_target;

/// Header values are visible ASCII; map arbitrary bytes into that range so the
/// fuzzer spends its budget on the parser instead of `HeaderValue` rejections.
fn printable(bytes: &[u8]) -> Vec<u8> {
    bytes.iter().map(|byte| 0x20 + (*byte % 0x5f)).collect()
}

fuzz_target!(|data: &[u8]| {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_bytes(&printable(data)) {
        headers.append(LINK, value);
    }
    if let Ok(value) = HeaderValue::from_bytes(&printable(&data[data.len() / 2..])) {
        headers.append("digest", value);
    }
    let _ = limedl_engine::metalink::parse_metalink_headers(&headers);
});
