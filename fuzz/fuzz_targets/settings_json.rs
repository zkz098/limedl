#![no_main]

use libfuzzer_sys::fuzz_target;
use limedl_types::settings::normalize_settings;
use limedl_types::types::AppSettings;

fuzz_target!(|data: &[u8]| {
    let Ok(contents) = std::str::from_utf8(data) else {
        return;
    };
    if let Ok(settings) = serde_json::from_str::<AppSettings>(contents) {
        let _ = normalize_settings(settings);
    }
});
