#![no_main]

use libfuzzer_sys::fuzz_target;
use limedl_types::checksum::{parse_sha256_file, parse_sha256sums};

fuzz_target!(|data: &[u8]| {
    let Ok(contents) = std::str::from_utf8(data) else {
        return;
    };
    let _ = parse_sha256_file(contents, None);
    let _ = parse_sha256_file(contents, Some("test-file.bin"));
    let _ = parse_sha256sums(contents, "test-file.bin");
});
