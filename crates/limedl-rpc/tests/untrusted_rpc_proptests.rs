//! Property-based tests for untrusted Aria2 JSON-RPC option and header parsers.

use limedl_rpc::fuzz_api::{
    collect_request_headers, extract_option_headers, find_header_value, parse_checksum_option,
    parse_select_file, split_aria2_tail,
};
use proptest::prelude::*;
use serde_json::{Map, Value};

fn json_value_strategy() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| Value::Number(n.into())),
        "\\PC{0,64}".prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 32, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec(("[a-zA-Z0-9_-]{0,16}", inner), 0..6)
                .prop_map(|entries| Value::Object(entries.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

fn json_map_strategy() -> impl Strategy<Value = Map<String, Value>> {
    let key = prop_oneof![
        Just("header".to_string()),
        Just("referer".to_string()),
        Just("http-user".to_string()),
        Just("http-passwd".to_string()),
        Just("checksum".to_string()),
        "[a-zA-Z0-9_-]{0,16}",
    ];
    prop::collection::vec((key, json_value_strategy()), 0..8)
        .prop_map(|entries| entries.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn parse_select_file_never_panics_on_arbitrary_json(val in json_value_strategy()) {
        let _ = parse_select_file(&val);
    }

    #[test]
    fn parse_checksum_option_never_panics_on_arbitrary_options(map in proptest::option::of(json_map_strategy())) {
        if let Ok((Some(_mode), Some(hash))) = parse_checksum_option(map.as_ref()) {
            prop_assert!(!hash.is_empty());
        }
    }

    #[test]
    fn header_and_tail_helpers_never_panic_on_arbitrary_json(
        map in proptest::option::of(json_map_strategy()),
        params in prop::collection::vec(json_value_strategy(), 0..6),
        from in 0usize..4,
        url in "\\PC{0,64}",
        key in "\\PC{0,24}",
        target_name in "\\PC{0,24}",
    ) {
        let extracted = extract_option_headers(map.as_ref(), &key);
        for h in &extracted {
            prop_assert!(h.contains(':'));
        }
        let collected = collect_request_headers(map.as_ref(), &url);
        for h in &collected {
            prop_assert!(h.contains(':'));
        }
        let _ = find_header_value(&extracted, &target_name);
        let _ = find_header_value(&collected, &target_name);
        let _ = split_aria2_tail(&params, from);
    }
}
