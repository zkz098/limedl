#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use serde_json::{Map, Value};

#[derive(Arbitrary, Debug)]
enum OptVal {
    Str(String),
    StrList(Vec<String>),
    Bool(bool),
    Int(i64),
    Null,
}

impl OptVal {
    fn into_value(self) -> Value {
        match self {
            Self::Str(s) => Value::String(s),
            Self::StrList(list) => Value::Array(list.into_iter().map(Value::String).collect()),
            Self::Bool(b) => Value::Bool(b),
            Self::Int(n) => serde_json::json!(n),
            Self::Null => Value::Null,
        }
    }
}

#[derive(Arbitrary, Debug)]
struct StructuredOptions {
    checksum: Option<OptVal>,
    referer: Option<OptVal>,
    http_user: Option<OptVal>,
    http_passwd: Option<OptVal>,
    header: Option<OptVal>,
    select_file: OptVal,
    url: String,
    tail_from: usize,
    extra: Vec<(String, OptVal)>,
}

#[derive(Arbitrary, Debug)]
enum FuzzInput {
    Structured(StructuredOptions),
    Raw(Vec<u8>),
}

fuzz_target!(|input: FuzzInput| {
    match input {
        FuzzInput::Structured(s) => {
            let mut map = Map::new();
            if let Some(v) = s.checksum {
                map.insert("checksum".into(), v.into_value());
            }
            if let Some(v) = s.referer {
                map.insert("referer".into(), v.into_value());
            }
            if let Some(v) = s.http_user {
                map.insert("http-user".into(), v.into_value());
            }
            if let Some(v) = s.http_passwd {
                map.insert("http-passwd".into(), v.into_value());
            }
            if let Some(v) = s.header {
                map.insert("header".into(), v.into_value());
            }
            for (k, v) in s.extra.into_iter().take(16) {
                map.insert(k, v.into_value());
            }

            let headers = limedl_rpc::fuzz_api::collect_request_headers(Some(&map), &s.url);
            let _ = limedl_rpc::fuzz_api::extract_option_headers(Some(&map), "header");
            let _ = limedl_rpc::fuzz_api::parse_checksum_option(Some(&map));
            let _ = limedl_rpc::fuzz_api::find_header_value(&headers, "referer");

            let select_val = s.select_file.into_value();
            let _ = limedl_rpc::fuzz_api::parse_select_file(&select_val);

            let params = vec![
                Value::Array(vec![Value::String(s.url)]),
                Value::Object(map),
                select_val,
            ];
            let _ = limedl_rpc::fuzz_api::split_aria2_tail(&params, s.tail_from % 4);
        }
        FuzzInput::Raw(data) => {
            if let Ok(value) = serde_json::from_slice::<Value>(&data) {
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
        }
    }
});
