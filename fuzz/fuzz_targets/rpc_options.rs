#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    if let Ok(value) = serde_json::from_slice::<Value>(data) {
        if let Value::Array(params) = &value {
            let _ = limedl_rpc::fuzz_api::split_aria2_tail(params, 0);
            let _ = limedl_rpc::fuzz_api::split_aria2_tail(params, 1);
        }
        if let Value::Object(options) = &value {
            let _ = limedl_rpc::fuzz_api::collect_request_headers(
                Some(options),
                "https://example.com/file.bin",
            );
            let _ = limedl_rpc::fuzz_api::parse_checksum_option(Some(options));
            let _ = limedl_rpc::fuzz_api::extract_option_headers(Some(options), "header");
        }
        let _ = limedl_rpc::fuzz_api::parse_select_file(&value);
    }

    if let Ok(text) = std::str::from_utf8(data) {
        let _ = limedl_rpc::fuzz_api::parse_select_file(&Value::String(text.to_string()));

        let mut options = serde_json::Map::new();
        for key in ["checksum", "referer", "http-user", "http-passwd", "header"] {
            options.insert(key.into(), Value::String(text.to_string()));
        }
        let _ = limedl_rpc::fuzz_api::parse_checksum_option(Some(&options));
        let _ = limedl_rpc::fuzz_api::collect_request_headers(
            Some(&options),
            "https://example.com/file.bin",
        );
        let _ = limedl_rpc::fuzz_api::find_header_value(
            &[text.to_string(), format!("Referer: {text}")],
            "referer",
        );
    }
});
