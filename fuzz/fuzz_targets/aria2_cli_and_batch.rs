#![no_main]

use libfuzzer_sys::fuzz_target;
use limedl_types::{aria2_cli, url_utils};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = aria2_cli::looks_like_aria2_command(text);
    let _ = aria2_cli::parse(text);
    let _ = url_utils::parse_batch_entries(text);
    let _ = url_utils::expand_url_ranges(text);
    let _ = url_utils::extract_batch_file_name(text);
    let _ = url_utils::percent_decode(text);
});
