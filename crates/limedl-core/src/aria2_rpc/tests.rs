//! Unit tests for aria2_rpc.rs — JSON-RPC gateway.
//!
//! Coverage:
//!   - JSON-RPC protocol types: JsonRpcResponse, JsonRpcError serialization
//!   - Factory functions: error_response, success_response, make_error
//!   - Stateless handlers: handle_version, handle_list_methods, handle_list_notifications
//!   - ID conversion: internal_id_to_gid
//!   - Request deserialization: JsonRpcRequest
//!   - process_jsonrpc_message (the WebSocket transport path) and RpcContext
//!     GID resolution / cache eviction
//!
//! What requires E2E (see `e2e_tests.rs`): dispatch over a live server, the
//! handler matrix, `system.multicall` and secret-token auth.

use super::*;
use ntest::timeout;
use serde_json::{Value, json};

// ── JSON-RPC response serialization ───────────────────────────────────────

#[test]
#[timeout(10_000)]
fn success_response_serializes_correctly() {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0",
        id: Some(Value::Number(1.into())),
        result: Some(Value::String("ok".into())),
        error: None,
    };
    let json_str = serde_json::to_string(&resp).expect("serialize success response");
    let parsed: Value = serde_json::from_str(&json_str).expect("valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"], "ok");
    assert!(
        parsed.get("error").is_none(),
        "success response must not have error field"
    );
}

#[test]
#[timeout(10_000)]
fn error_response_serializes_correctly() {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0",
        id: Some(Value::Number(1.into())),
        result: None,
        error: Some(JsonRpcError {
            code: 1,
            message: "Not found".into(),
        }),
    };
    let json_str = serde_json::to_string(&resp).expect("serialize error response");
    let parsed: Value = serde_json::from_str(&json_str).expect("valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert!(
        parsed.get("result").is_none(),
        "error response must not have result field"
    );
    assert_eq!(parsed["error"]["code"], 1);
    assert_eq!(parsed["error"]["message"], "Not found");
}

#[test]
#[timeout(10_000)]
fn notification_response_omits_id_and_result_and_error() {
    // A notification is a response without an id — valid in JSON-RPC batch context.
    let resp = JsonRpcResponse {
        jsonrpc: "2.0",
        id: None,
        result: None,
        error: None,
    };
    let json_str = serde_json::to_string(&resp).expect("serialize notification response");
    let parsed: Value = serde_json::from_str(&json_str).expect("valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert!(
        parsed.get("id").is_none(),
        "notification must not have id field"
    );
    assert!(
        parsed.get("result").is_none(),
        "notification must not have result field"
    );
    assert!(
        parsed.get("error").is_none(),
        "notification must not have error field"
    );
}

// ── Error codes and factory functions ─────────────────────────────────────

#[test]
#[timeout(10_000)]
fn make_error_creates_correct_struct() {
    let err = make_error(-32700, "Parse error");
    assert_eq!(err.code, -32700);
    assert_eq!(err.message, "Parse error");
}

#[test]
#[timeout(10_000)]
fn make_error_accepts_string_owning_types() {
    let err = make_error(-1, String::from("custom error"));
    assert_eq!(err.code, -1);
    assert_eq!(err.message, "custom error");
}

#[test]
#[timeout(10_000)]
fn error_response_parse_error() {
    let resp = error_response(Some(Value::Null), ERR_PARSE, "Parse error");
    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(Value::Null));
    assert!(resp.result.is_none());
    let err = resp.error.expect("error field should be Some");
    assert_eq!(err.code, -32700);
    assert_eq!(err.message, "Parse error");
}

#[test]
#[timeout(10_000)]
fn error_response_invalid_request() {
    let resp = error_response(None, ERR_INVALID_REQUEST, "Invalid Request");
    assert_eq!(resp.error.as_ref().unwrap().code, -32600);
    assert_eq!(resp.error.as_ref().unwrap().message, "Invalid Request");
}

#[test]
#[timeout(10_000)]
fn error_response_method_not_found() {
    let resp = error_response(None, ERR_METHOD_NOT_FOUND, "Method not found: foo");
    assert_eq!(resp.error.as_ref().unwrap().code, 1);
}

#[test]
#[timeout(10_000)]
fn error_response_invalid_params() {
    let resp = error_response(None, ERR_INVALID_PARAMS, "Missing GID parameter");
    assert_eq!(resp.error.as_ref().unwrap().code, 1);
}

#[test]
#[timeout(10_000)]
fn error_response_internal_error() {
    let resp = error_response(None, ERR_INTERNAL, "Something went wrong");
    assert_eq!(resp.error.as_ref().unwrap().code, 1);
}

#[test]
#[timeout(10_000)]
fn error_response_serializes_to_valid_json() {
    let resp = error_response(Some(json!(42)), ERR_METHOD_NOT_FOUND, "Not found");
    let json_str = serde_json::to_string(&resp).expect("serialize");
    let parsed: Value = serde_json::from_str(&json_str).expect("valid JSON");

    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["error"]["code"], 1);
    assert_eq!(parsed["error"]["message"], "Not found");
    assert!(parsed.get("result").is_none());
}

// ── success_response factory ──────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn success_response_wraps_result_correctly() {
    let result = json!({"gid": "abc123"});
    let resp = success_response(Some(json!(1)), result.clone());
    assert_eq!(resp.jsonrpc, "2.0");
    assert_eq!(resp.id, Some(json!(1)));
    assert_eq!(resp.result, Some(result));
    assert!(resp.error.is_none());
}

#[test]
#[timeout(10_000)]
fn success_response_with_null_id() {
    let resp = success_response(None, json!("ok"));
    assert!(resp.id.is_none());
    assert_eq!(resp.result, Some(json!("ok")));
}

#[test]
#[timeout(10_000)]
fn success_response_factory_serialization() {
    let resp = success_response(Some(json!(1)), json!("OK"));
    let json_str = serde_json::to_string(&resp).expect("serialize");
    let parsed: Value = serde_json::from_str(&json_str).expect("valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"], "OK");
    assert!(parsed.get("error").is_none());
}

// ── Stateless handlers ────────────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn handle_version_returns_version_info() {
    let result = handle_version();
    assert_eq!(result["version"], env!("CARGO_PKG_VERSION"));
    let features = result["enabledFeatures"]
        .as_array()
        .expect("enabledFeatures should be an array");
    assert!(features.contains(&json!("BitTorrent")));
    assert!(features.contains(&json!("HTTPS")));
    assert!(features.contains(&json!("Async DNS")));
    assert!(features.contains(&json!("Message Digest")));
    assert!(features.contains(&json!("GZip")));
    assert!(features.contains(&json!("Brotli")));
    assert!(features.contains(&json!("Zstd")));
    // Capabilities limedl does not have must not be advertised: XML-RPC has no
    // `/rpc` endpoint, Firefox3 Cookie is not implemented, and Metalink/SFTP are
    // out of scope.
    for untrue in ["XML-RPC", "Firefox3 Cookie", "Metalink", "SFTP"] {
        assert!(
            !features.contains(&json!(untrue)),
            "getVersion must not advertise unsupported feature {untrue}: {result}"
        );
    }
    assert_eq!(features.len(), 7);
}

#[test]
#[timeout(10_000)]
fn handle_list_methods_returns_array() {
    let result = handle_list_methods();
    let methods = result.as_array().expect("should be an array");

    // Spot-check essential methods
    assert!(methods.contains(&json!("aria2.addUri")));
    assert!(methods.contains(&json!("aria2.addTorrent")));
    assert!(methods.contains(&json!("aria2.pause")));
    assert!(methods.contains(&json!("aria2.forcePause")));
    assert!(methods.contains(&json!("aria2.unpause")));
    assert!(methods.contains(&json!("aria2.pauseAll")));
    assert!(methods.contains(&json!("aria2.forcePauseAll")));
    assert!(methods.contains(&json!("aria2.unpauseAll")));
    assert!(methods.contains(&json!("aria2.remove")));
    assert!(methods.contains(&json!("aria2.forceRemove")));
    assert!(methods.contains(&json!("aria2.tellStatus")));
    assert!(methods.contains(&json!("aria2.tellActive")));
    assert!(methods.contains(&json!("aria2.tellWaiting")));
    assert!(methods.contains(&json!("aria2.tellStopped")));
    assert!(methods.contains(&json!("aria2.getGlobalStat")));
    assert!(methods.contains(&json!("aria2.getGlobalOption")));
    assert!(methods.contains(&json!("aria2.changeGlobalOption")));
    assert!(methods.contains(&json!("aria2.changeOption")));
    assert!(methods.contains(&json!("aria2.removeDownloadResult")));
    assert!(methods.contains(&json!("aria2.getVersion")));
    assert!(methods.contains(&json!("aria2.getFiles")));
    assert!(methods.contains(&json!("aria2.getUris")));
    assert!(methods.contains(&json!("aria2.getPeers")));
    assert!(methods.contains(&json!("aria2.shutdown")));
    assert!(methods.contains(&json!("system.listMethods")));
    assert!(methods.contains(&json!("system.listNotifications")));

    // Verify the exact count
    assert_eq!(methods.len(), 36);
}

#[test]
#[timeout(10_000)]
fn handle_list_notifications_returns_array() {
    let result = handle_list_notifications();
    let notifications = result.as_array().expect("should be an array");

    assert!(notifications.contains(&json!("aria2.onDownloadStart")));
    assert!(notifications.contains(&json!("aria2.onDownloadPause")));
    assert!(notifications.contains(&json!("aria2.onDownloadStop")));
    assert!(notifications.contains(&json!("aria2.onDownloadComplete")));
    assert!(notifications.contains(&json!("aria2.onBtDownloadComplete")));
    assert!(notifications.contains(&json!("aria2.onDownloadError")));

    assert_eq!(notifications.len(), 6);
}

// ── internal_id_to_gid ────────────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn internal_id_to_gid_deterministic() {
    let id = "http:abc-123-def";
    let gid1 = internal_id_to_gid(id);
    let gid2 = internal_id_to_gid(id);
    assert_eq!(gid1, gid2, "same input must produce same GID");
}

#[test]
#[timeout(10_000)]
fn internal_id_to_gid_format() {
    let gid = internal_id_to_gid("http:abc");
    assert_eq!(gid.len(), 16, "GID must be exactly 16 hex characters");
    assert!(
        gid.chars().all(|c| c.is_ascii_hexdigit()),
        "GID must contain only hex characters: {gid}"
    );
}

#[test]
#[timeout(10_000)]
fn internal_id_to_gid_different_inputs() {
    let gid_a = internal_id_to_gid("http:task-A");
    let gid_b = internal_id_to_gid("http:task-B");
    assert_ne!(gid_a, gid_b, "different inputs must produce different GIDs");
}

#[test]
#[timeout(10_000)]
fn internal_id_to_gid_bt_prefix() {
    let gid = internal_id_to_gid("bt:some-torrent-hash");
    assert_eq!(gid.len(), 16);
    assert!(gid.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
#[timeout(10_000)]
fn internal_id_to_gid_empty_string() {
    // xxh3_64 of empty bytes still produces 16 hex chars
    let gid = internal_id_to_gid("");
    assert_eq!(gid.len(), 16);
    assert!(gid.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
#[timeout(10_000)]
fn internal_id_to_gid_unicode() {
    // Unicode inputs should hash deterministically
    let gid = internal_id_to_gid("http:文件下载");
    assert_eq!(gid.len(), 16);
    assert!(gid.chars().all(|c| c.is_ascii_hexdigit()));
}

// ── JSON-RPC request deserialization ──────────────────────────────────────

#[test]
#[timeout(10_000)]
fn deserialize_valid_request() {
    let json_str = r#"{"jsonrpc":"2.0","id":1,"method":"aria2.addUri","params":[["http://example.com/file.zip"]]}"#;
    let req: JsonRpcRequest = serde_json::from_str(json_str).expect("deserialize valid request");

    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, Some(json!(1)));
    assert_eq!(req.method, "aria2.addUri");
    let params = req.params.expect("params should be Some");
    assert_eq!(params.len(), 1);
    let uris = params[0].as_array().expect("first param should be array");
    assert_eq!(uris[0], json!("http://example.com/file.zip"));
}

#[test]
#[timeout(10_000)]
fn deserialize_request_without_id() {
    // Notifications omit the id field
    let json_str =
        r#"{"jsonrpc":"2.0","method":"aria2.addUri","params":[["http://example.com/file.zip"]]}"#;
    let req: JsonRpcRequest = serde_json::from_str(json_str).expect("deserialize notification");

    assert_eq!(req.jsonrpc, "2.0");
    assert!(req.id.is_none(), "notification must not have id");
    assert_eq!(req.method, "aria2.addUri");
}

#[test]
#[timeout(10_000)]
fn deserialize_request_without_params() {
    // Some methods (e.g. system.listMethods) have no params
    let json_str = r#"{"jsonrpc":"2.0","id":2,"method":"system.listMethods"}"#;
    let req: JsonRpcRequest = serde_json::from_str(json_str).expect("deserialize without params");

    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, Some(json!(2)));
    assert_eq!(req.method, "system.listMethods");
    assert!(req.params.is_none(), "params should be None when absent");
}

#[test]
#[timeout(10_000)]
fn deserialize_request_with_string_id() {
    // JSON-RPC allows string ids
    let json_str = r#"{"jsonrpc":"2.0","id":"req-001","method":"aria2.getVersion"}"#;
    let req: JsonRpcRequest = serde_json::from_str(json_str).expect("deserialize with string id");

    assert_eq!(req.id, Some(json!("req-001")));
    assert_eq!(req.method, "aria2.getVersion");
}

#[test]
#[timeout(10_000)]
fn deserialize_request_with_null_id() {
    let json_str = r#"{"jsonrpc":"2.0","id":null,"method":"aria2.getVersion"}"#;
    let req: JsonRpcRequest = serde_json::from_str(json_str).expect("deserialize with null id");

    assert_eq!(
        req.id, None,
        "JSON null should deserialize as None for Option<Value>"
    );
}

#[test]
#[timeout(10_000)]
fn deserialize_invalid_json_returns_error() {
    let json_str = r#"{bad json}"#;
    let result: Result<JsonRpcRequest, _> = serde_json::from_str(json_str);
    assert!(
        result.is_err(),
        "malformed JSON should fail deserialization"
    );
}

#[test]
#[timeout(10_000)]
fn deserialize_non_string_method_returns_error() {
    let json_str = r#"{"jsonrpc":"2.0","id":1,"method":123}"#;
    let result: Result<JsonRpcRequest, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "non-string method should fail");
}

#[test]
#[timeout(10_000)]
fn deserialize_missing_method_returns_error() {
    let json_str = r#"{"jsonrpc":"2.0","id":1}"#;
    let result: Result<JsonRpcRequest, _> = serde_json::from_str(json_str);
    assert!(result.is_err(), "missing method should fail");
}

// ── state_to_aria2 mapping ────────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn state_to_aria2_maps_all_states() {
    use super::DownloadState::*;
    let cases = [
        (Queued, "waiting"),
        (Downloading, "active"),
        (Paused, "paused"),
        (Retrying, "active"),
        (Verifying, "active"),
        (Completed, "complete"),
        (Failed, "error"),
        (Canceled, "removed"),
    ];
    for (state, expected) in &cases {
        assert_eq!(state_to_aria2(state), *expected, "mismatch for {state:?}");
    }
}

// ── extract_option_* helpers ──────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn extract_option_str_found() {
    let mut map = serde_json::Map::new();
    map.insert("out".to_string(), json!("myfile.zip"));
    let result = extract_option_str(Some(&map), "out");
    assert_eq!(result, Some("myfile.zip".to_string()));
}

#[test]
#[timeout(10_000)]
fn extract_option_str_missing() {
    let map = serde_json::Map::new();
    let result = extract_option_str(Some(&map), "out");
    assert_eq!(result, None);
}

#[test]
#[timeout(10_000)]
fn extract_option_str_none_options() {
    let result = extract_option_str(None, "out");
    assert_eq!(result, None);
}

#[test]
#[timeout(10_000)]
fn extract_option_usize_found() {
    let mut map = serde_json::Map::new();
    map.insert("split".to_string(), json!("5"));
    let result = extract_option_usize(Some(&map), "split");
    assert_eq!(result, Some(5));
}

#[test]
#[timeout(10_000)]
fn extract_option_usize_invalid_value() {
    let mut map = serde_json::Map::new();
    map.insert("split".to_string(), json!("not-a-number"));
    let result = extract_option_usize(Some(&map), "split");
    assert_eq!(result, None);
}

#[test]
#[timeout(10_000)]
fn extract_option_usize_non_string_value() {
    let mut map = serde_json::Map::new();
    map.insert("split".to_string(), json!(42));
    let result = extract_option_usize(Some(&map), "split");
    // as_str() on a number returns None
    assert_eq!(result, None);
}

#[test]
#[timeout(10_000)]
fn extract_option_u32_found() {
    let mut map = serde_json::Map::new();
    map.insert("max-tries".to_string(), json!("3"));
    let result = extract_option_u32(Some(&map), "max-tries");
    assert_eq!(result, Some(3));
}

#[test]
#[timeout(10_000)]
fn extract_option_u32_invalid_value() {
    let mut map = serde_json::Map::new();
    map.insert("max-tries".to_string(), json!("999999999999")); // overflow u32
    let result = extract_option_u32(Some(&map), "max-tries");
    assert_eq!(result, None);
}

// ── aria2 request-option passthrough ─────────────────────────────────────

#[test]
#[timeout(10_000)]
fn extract_option_headers_array_keeps_valid_and_drops_malformed() {
    let mut map = serde_json::Map::new();
    map.insert(
        "header".to_string(),
        json!(["X-A: 1", "no-colon", "  X-B:  two  ", ""]),
    );
    let headers = extract_option_headers(Some(&map), "header");
    assert_eq!(headers, vec!["X-A: 1", "X-B:  two"]);
}

#[test]
#[timeout(10_000)]
fn extract_option_headers_single_string() {
    let mut map = serde_json::Map::new();
    map.insert("header".to_string(), json!("Cookie: a=b"));
    assert_eq!(
        extract_option_headers(Some(&map), "header"),
        vec!["Cookie: a=b"]
    );
}

#[test]
#[timeout(10_000)]
fn extract_option_headers_missing_or_wrong_type() {
    let map = serde_json::Map::new();
    assert!(extract_option_headers(Some(&map), "header").is_empty());

    let mut map = serde_json::Map::new();
    map.insert("header".to_string(), json!(42));
    assert!(extract_option_headers(Some(&map), "header").is_empty());
}

#[test]
#[timeout(10_000)]
fn collect_request_headers_appends_referer() {
    let mut map = serde_json::Map::new();
    map.insert("referer".to_string(), json!("https://example.com/page"));
    let headers = collect_request_headers(Some(&map), "https://example.com/file.bin");
    assert_eq!(headers, vec!["Referer: https://example.com/page"]);
}

#[test]
#[timeout(10_000)]
fn collect_request_headers_referer_star_uses_download_url() {
    let mut map = serde_json::Map::new();
    map.insert("referer".to_string(), json!("*"));
    let url = "https://example.com/file.bin";
    let headers = collect_request_headers(Some(&map), url);
    assert_eq!(headers, vec![format!("Referer: {url}")]);
}

#[test]
#[timeout(10_000)]
fn collect_request_headers_explicit_referer_wins() {
    let mut map = serde_json::Map::new();
    map.insert(
        "header".to_string(),
        json!(["Referer: https://explicit.example/"]),
    );
    map.insert("referer".to_string(), json!("https://option.example/"));
    let headers = collect_request_headers(Some(&map), "https://example.com/file.bin");
    assert_eq!(headers, vec!["Referer: https://explicit.example/"]);
}

#[test]
#[timeout(10_000)]
fn collect_request_headers_basic_auth() {
    let mut map = serde_json::Map::new();
    map.insert("http-user".to_string(), json!("user"));
    map.insert("http-passwd".to_string(), json!("pass"));
    let headers = collect_request_headers(Some(&map), "https://example.com/file.bin");
    assert_eq!(headers, vec!["Authorization: Basic dXNlcjpwYXNz"]);
}

#[test]
#[timeout(10_000)]
fn collect_request_headers_explicit_authorization_wins() {
    let mut map = serde_json::Map::new();
    map.insert("header".to_string(), json!(["Authorization: Bearer tok"]));
    map.insert("http-user".to_string(), json!("user"));
    map.insert("http-passwd".to_string(), json!("pass"));
    let headers = collect_request_headers(Some(&map), "https://example.com/file.bin");
    assert_eq!(headers, vec!["Authorization: Bearer tok"]);
}

#[test]
#[timeout(10_000)]
fn collect_request_headers_empty_when_no_options() {
    assert!(collect_request_headers(None, "https://example.com/file.bin").is_empty());
}

#[test]
#[timeout(10_000)]
fn parse_checksum_option_supported_types() {
    for (raw, expected_mode) in [
        ("sha-256=ABCDEF", ChecksumMode::Sha256),
        ("sha256=ABCDEF", ChecksumMode::Sha256),
        ("sha-512=ABCDEF", ChecksumMode::Sha512),
        ("blake3=ABCDEF", ChecksumMode::Blake3),
    ] {
        let mut map = serde_json::Map::new();
        map.insert("checksum".to_string(), json!(raw));
        let (mode, digest) = parse_checksum_option(Some(&map));
        assert_eq!(mode, Some(expected_mode), "type mismatch for {raw}");
        assert_eq!(
            digest,
            Some("abcdef".to_string()),
            "digest mismatch for {raw}"
        );
    }
}

#[test]
#[timeout(10_000)]
fn parse_checksum_option_ignores_unsupported_and_malformed() {
    for raw in [
        "md5=abc",
        "sha-1=abc",
        "adler32=abc",
        "no-equals",
        "sha-256=",
    ] {
        let mut map = serde_json::Map::new();
        map.insert("checksum".to_string(), json!(raw));
        assert_eq!(parse_checksum_option(Some(&map)), (None, None), "raw={raw}");
    }
    assert_eq!(parse_checksum_option(None), (None, None));
}

#[test]
#[timeout(10_000)]
fn find_header_value_is_case_insensitive() {
    let headers = vec![
        "X-A: 1".to_string(),
        "REFERER: https://example.com/".to_string(),
    ];
    assert_eq!(
        find_header_value(&headers, "referer"),
        Some("https://example.com/".to_string())
    );
    assert_eq!(find_header_value(&headers, "missing"), None);
}

// ── strip_token helper ────────────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn strip_token_removes_token_prefix() {
    let params = vec![json!("token:sekret"), json!("arg1"), json!("arg2")];
    let stripped = strip_token(params);
    assert_eq!(stripped.len(), 2);
    assert_eq!(stripped[0], json!("arg1"));
    assert_eq!(stripped[1], json!("arg2"));
}

#[test]
#[timeout(10_000)]
fn strip_token_no_token() {
    let params = vec![json!("arg1"), json!("arg2")];
    let stripped = strip_token(params);
    assert_eq!(stripped.len(), 2);
    assert_eq!(stripped[0], json!("arg1"));
    assert_eq!(stripped[1], json!("arg2"));
}

#[test]
#[timeout(10_000)]
fn strip_token_empty_params() {
    let params: Vec<Value> = vec![];
    let stripped = strip_token(params);
    assert!(stripped.is_empty());
}

#[test]
#[timeout(10_000)]
fn strip_token_non_string_first_param() {
    let params = vec![json!(42), json!("arg1")];
    let stripped = strip_token(params);
    assert_eq!(stripped.len(), 2);
    assert_eq!(stripped[0], json!(42));
}

#[test]
#[timeout(10_000)]
fn strip_token_token_not_at_start() {
    // The function only checks the first param for "token:" prefix
    let params = vec![json!("arg1"), json!("token:sekret")];
    let stripped = strip_token(params);
    assert_eq!(stripped.len(), 2);
    assert_eq!(stripped[0], json!("arg1"));
}

// ── parse_int_param helper ────────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn parse_int_param_string_number() {
    let params = vec![json!("42")];
    assert_eq!(parse_int_param(&params, 0), Some(42));
}

#[test]
#[timeout(10_000)]
fn parse_int_param_number() {
    let params = vec![json!(42)];
    assert_eq!(parse_int_param(&params, 0), Some(42));
}

#[test]
#[timeout(10_000)]
fn parse_int_param_out_of_range() {
    let params = vec![json!("hello")];
    assert_eq!(parse_int_param(&params, 0), None);
}

#[test]
#[timeout(10_000)]
fn parse_int_param_missing_index() {
    let params: Vec<Value> = vec![];
    assert_eq!(parse_int_param(&params, 0), None);
}

// ── peer_info_to_aria2_peer ───────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn peer_info_to_aria2_peer_with_port() {
    let peer = BtPeerInfo {
        address: "192.168.1.1:6881".to_string(),
        client: "-IT-".to_string(),
        flags: "c".to_string(),
        progress: 0.5,
        download_speed: 1024.0,
        upload_speed: 512.0,
    };
    let result = peer_info_to_aria2_peer(&peer);
    assert_eq!(result["ip"], "192.168.1.1");
    assert_eq!(result["port"], 6881);
    assert_eq!(result["amChoking"], "true");
    assert_eq!(result["seeder"], "false");
    assert_eq!(result["downloadSpeed"], "1024");
    assert_eq!(result["uploadSpeed"], "512");
}

#[test]
#[timeout(10_000)]
fn peer_info_to_aria2_peer_seeder() {
    let peer = BtPeerInfo {
        address: "10.0.0.1:51413".to_string(),
        client: "-IT-".to_string(),
        flags: "".to_string(),
        progress: 1.0, // 100% = seeder
        download_speed: 0.0,
        upload_speed: 2000.0,
    };
    let result = peer_info_to_aria2_peer(&peer);
    assert_eq!(result["seeder"], "true");
    assert_eq!(result["amChoking"], "false");
}

#[test]
#[timeout(10_000)]
fn peer_info_to_aria2_peer_no_port() {
    let peer = BtPeerInfo {
        address: "192.168.1.1".to_string(),
        client: "-IT-".to_string(),
        flags: "".to_string(),
        progress: 0.0,
        download_speed: 0.0,
        upload_speed: 0.0,
    };
    let result = peer_info_to_aria2_peer(&peer);
    assert_eq!(result["ip"], "192.168.1.1");
    assert_eq!(result["port"], 0);
}

// ── RpcContext-dependent units ───────────────────────────────────────

/// A context over the given registry/event bus.
fn make_ctx_with(registry: Arc<BackendRegistry>, event_bus: Arc<EventBus>) -> RpcContext {
    let dispatcher = Dispatcher::new(registry.clone(), event_bus.clone());
    RpcContext {
        registry,
        dispatcher,
        auth: AuthConfig::Disabled,
        event_bus,
        gid_cache: Mutex::new(HashMap::default()),
        session_id: "test-session".to_string(),
        exit_on_shutdown: false,
        shutdown_notify: Arc::new(Notify::new()),
    }
}

/// A minimal context with an empty registry — enough for the WebSocket message
/// transport and for cache-hit GID resolution.
fn make_ctx() -> RpcContext {
    make_ctx_with(Arc::new(BackendRegistry::new()), Arc::new(EventBus::new(64)))
}

/// `aria2.shutdown` is a no-op handshake for the desktop (a managed subsystem)
/// but must wake the headless daemon when `exit_on_shutdown` is set.
#[tokio::test]
#[timeout(10_000)]
async fn shutdown_notifies_only_when_exit_on_shutdown_is_set() {
    let mut ctx = make_ctx();
    let notify = ctx.shutdown_notify.clone();

    handle_shutdown(&ctx).await.expect("the handshake must answer");
    assert!(
        tokio::time::timeout(Duration::from_millis(50), notify.notified())
            .await
            .is_err(),
        "the desktop path must not request an exit"
    );

    ctx.exit_on_shutdown = true;
    handle_shutdown(&ctx).await.expect("the handshake must still answer");
    tokio::time::timeout(Duration::from_millis(50), notify.notified())
        .await
        .expect("exit_on_shutdown must wake the waiter");
}

/// `process_jsonrpc_message` is the WebSocket transport path: parse errors,
/// wrong protocol versions, unknown methods and successes must all be wrapped
/// in a JSON-RPC response rather than dropped.
#[tokio::test]
#[timeout(10_000)]
async fn process_jsonrpc_message_covers_errors_and_success() {
    let ctx = make_ctx();

    let resp: Value = serde_json::from_str(&process_jsonrpc_message(&ctx, "{ not json").await)
        .expect("transport must always answer valid JSON");
    assert_eq!(resp["error"]["code"], -32700, "malformed JSON: {resp}");

    let body = json!({"jsonrpc": "1.0", "id": 7, "method": "aria2.getVersion", "params": []});
    let resp: Value = serde_json::from_str(&process_jsonrpc_message(&ctx, &body.to_string()).await)
        .expect("valid JSON response");
    assert_eq!(resp["error"]["code"], -32600, "wrong version: {resp}");
    assert_eq!(resp["id"], 7, "the request id must be echoed: {resp}");

    let body = json!({"jsonrpc": "2.0", "id": 8, "method": "no.such.method", "params": []});
    let resp: Value = serde_json::from_str(&process_jsonrpc_message(&ctx, &body.to_string()).await)
        .expect("valid JSON response");
    assert_eq!(resp["error"]["code"], 1, "unknown method: {resp}");

    let body = json!({"jsonrpc": "2.0", "id": 9, "method": "aria2.getVersion", "params": []});
    let resp: Value = serde_json::from_str(&process_jsonrpc_message(&ctx, &body.to_string()).await)
        .expect("valid JSON response");
    assert_eq!(resp["result"]["version"], env!("CARGO_PKG_VERSION"), "success: {resp}");
    assert_eq!(resp["id"], 9);
}

/// A top-level JSON-RPC array is a batch: one response per request element.
#[tokio::test]
#[timeout(10_000)]
async fn batch_request_returns_one_response_per_element() {
    let ctx = make_ctx();
    let body = json!([
        {"jsonrpc": "2.0", "id": 1, "method": "aria2.getVersion", "params": []},
        {"jsonrpc": "2.0", "id": 2, "method": "no.such.method", "params": []}
    ])
    .to_string();

    let resp: Value = serde_json::from_str(&process_jsonrpc_message(&ctx, &body).await)
        .expect("batch must answer valid JSON");
    let responses = resp.as_array().expect("a batch must return an array");
    assert_eq!(responses.len(), 2, "{resp}");
    assert_eq!(responses[0]["id"], 1);
    assert!(responses[0]["result"]["version"].is_string(), "{resp}");
    assert_eq!(responses[1]["error"]["code"], 1, "{resp}");
}

/// A batch of notifications executes but produces no response at all.
#[tokio::test]
#[timeout(10_000)]
async fn batch_of_notifications_produces_no_response() {
    let ctx = make_ctx();
    let body = json!([{"jsonrpc": "2.0", "method": "aria2.pauseAll", "params": []}]).to_string();
    assert!(
        process_jsonrpc_message(&ctx, &body).await.is_empty(),
        "an all-notification batch must not be answered"
    );
}

/// An empty array is a single Invalid Request error per the JSON-RPC 2.0 spec.
#[tokio::test]
#[timeout(10_000)]
async fn empty_batch_is_an_invalid_request() {
    let ctx = make_ctx();
    let resp: Value = serde_json::from_str(&process_jsonrpc_message(&ctx, "[]").await)
        .expect("valid JSON response");
    assert_eq!(resp["error"]["code"], -32600, "{resp}");
}

/// `resolve_gid` scans the registered backends on a cache miss and caches the
/// result; `aria2.remove` must evict that entry so the cache cannot grow for
/// the lifetime of the RPC server.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn resolve_gid_scans_backends_and_remove_clears_the_cache() {
    let (_tmp, dm) = crate::tests::dispatcher_tests::make_manager();
    let uuid = uuid::Uuid::from_u128(0xC0FFEE);
    let id = uuid.to_string();
    crate::tests::dispatcher_tests::inject_download(&dm, &id, DownloadState::Completed).await;

    let mut registry = BackendRegistry::new();
    registry.register_arc(TaskKind::Http, dm.clone());
    let registry = Arc::new(registry);
    let event_bus = Arc::new(EventBus::new(64));
    let dispatcher = Dispatcher::new(registry.clone(), event_bus.clone());
    let ctx = RpcContext {
        registry,
        dispatcher,
        auth: AuthConfig::Disabled,
        event_bus,
        gid_cache: Mutex::new(HashMap::default()),
        session_id: "test-session".to_string(),
        exit_on_shutdown: false,
        shutdown_notify: Arc::new(Notify::new()),
    };

    let gid = internal_id_to_gid(&id);
    // Cache miss → backend scan → hit cached for the next call.
    assert_eq!(resolve_gid(&ctx, &gid).await, Some(TaskId::Http(uuid)));
    assert!(
        ctx.gid_cache.lock().await.contains_key(&gid),
        "the scan result must be cached"
    );
    assert_eq!(resolve_gid(&ctx, &gid).await, Some(TaskId::Http(uuid)));

    // Unknown GIDs resolve to None instead of panicking.
    assert!(resolve_gid(&ctx, "0000000000000000").await.is_none());

    handle_remove(&ctx, vec![json!(gid.clone())])
        .await
        .expect("remove");
    assert!(
        !ctx.gid_cache.lock().await.contains_key(&gid),
        "aria2.remove must evict the cached gid"
    );
    assert!(
        resolve_gid(&ctx, &gid).await.is_none(),
        "a removed task must not resolve any more"
    );
}

/// aria2 accepts an abbreviated GID when the prefix is unambiguous; limedl must
/// refuse an ambiguous one rather than guess which task the client meant.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn resolve_gid_accepts_a_unique_prefix_and_refuses_an_ambiguous_one() {
    let (_tmp, dm) = crate::tests::dispatcher_tests::make_manager();
    // 17 GIDs over 16 leading hex digits guarantees two share the first digit.
    let mut gids = Vec::new();
    for n in 0u128..17 {
        let id = uuid::Uuid::from_u128(0x1000 + n).to_string();
        crate::tests::dispatcher_tests::inject_download(&dm, &id, DownloadState::Completed).await;
        gids.push(internal_id_to_gid(&id));
    }

    let mut registry = BackendRegistry::new();
    registry.register_arc(TaskKind::Http, dm.clone());
    let ctx = make_ctx_with(Arc::new(registry), Arc::new(EventBus::new(64)));

    // A unique prefix resolves to the full task.
    assert_eq!(
        resolve_gid(&ctx, &gids[0][..8]).await,
        Some(TaskId::Http(uuid::Uuid::from_u128(0x1000))),
        "an unambiguous prefix must resolve"
    );

    // A prefix shared by two tasks is refused.
    let mut counts: HashMap<char, usize> = HashMap::default();
    for gid in &gids {
        *counts
            .entry(gid.chars().next().expect("non-empty gid"))
            .or_default() += 1;
    }
    let shared = counts
        .into_iter()
        .find(|(_, count)| *count >= 2)
        .map(|(ch, _)| ch.to_string())
        .expect("17 gids over 16 prefixes must collide");
    assert_eq!(
        resolve_gid(&ctx, &shared).await,
        None,
        "an ambiguous prefix must not resolve"
    );
}

// ── aria2 option/uri helpers ─────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn classify_aria2_uri_routes_magnets_to_bt() {
    assert_eq!(classify_aria2_uri("magnet:?xt=urn:btih:abc"), TaskKind::Bt);
    assert_eq!(
        classify_aria2_uri("  MAGNET:?xt=urn:btih:abc"),
        TaskKind::Bt
    );
    // aria2 downloads `.torrent` URLs as plain files via addUri.
    assert_eq!(
        classify_aria2_uri("https://example.com/file.torrent"),
        TaskKind::Http
    );
    assert_eq!(classify_aria2_uri("https://example.com/a"), TaskKind::Http);
}

#[test]
#[timeout(10_000)]
fn parse_select_file_converts_one_based_indices() {
    assert_eq!(parse_select_file(&json!("1,3")).unwrap(), vec![0, 2]);
    assert_eq!(parse_select_file(&json!(" 2 , 4 ")).unwrap(), vec![1, 3]);
    assert_eq!(parse_select_file(&json!("")).unwrap(), Vec::<usize>::new());
    assert_eq!(parse_select_file(&json!("1,,2")).unwrap(), vec![0, 1]);

    assert!(parse_select_file(&json!("0")).is_err(), "indices are 1-based");
    assert!(parse_select_file(&json!("abc")).is_err());
    assert!(parse_select_file(&json!(1)).is_err(), "must be a string");
}

#[test]
#[timeout(10_000)]
fn filter_status_keys_keeps_only_requested_fields() {
    let status = json!({
        "gid": "abc",
        "status": "active",
        "totalLength": "5",
    });
    let filtered = filter_status_keys(status, &["gid".into(), "totalLength".into()]);
    let object = filtered.as_object().expect("object");
    assert_eq!(object.len(), 2);
    assert!(object.contains_key("gid"));
    assert!(object.contains_key("totalLength"));
    assert!(!object.contains_key("status"));
}

#[test]
#[timeout(10_000)]
fn bt_files_to_aria2_uses_one_based_indices_and_selection() {
    let files = vec![
        BtFileStatus {
            index: 0,
            path: "a.bin".into(),
            size: 10,
            downloaded_bytes: 4,
            included: true,
        },
        BtFileStatus {
            index: 1,
            path: "b.bin".into(),
            size: 20,
            downloaded_bytes: 0,
            included: false,
        },
    ];

    let value = bt_files_to_aria2(&files);
    let entries = value.as_array().expect("array");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["index"], "1");
    assert_eq!(entries[0]["path"], "a.bin");
    assert_eq!(entries[0]["length"], "10");
    assert_eq!(entries[0]["completedLength"], "4");
    assert_eq!(entries[0]["selected"], "true");
    assert_eq!(entries[1]["index"], "2");
    assert_eq!(entries[1]["selected"], "false");
    assert_eq!(entries[1]["uris"], json!([]));
}

/// `aria2.changeOption` must apply what the engine can change and fail loudly
/// for everything else instead of silently pretending.
#[tokio::test(flavor = "multi_thread")]
#[timeout(30_000)]
async fn change_option_applies_pause_and_rejects_unsupported_keys() {
    let (tmp, dm) = crate::tests::dispatcher_tests::make_manager();
    let uuid = uuid::Uuid::from_u128(0xCAFE);
    let id = uuid.to_string();
    crate::tests::dispatcher_tests::inject_download(&dm, &id, DownloadState::Queued).await;

    let mut registry = BackendRegistry::new();
    registry.register_arc(TaskKind::Http, dm.clone());
    let ctx = make_ctx_with(Arc::new(registry), Arc::new(EventBus::new(64)));
    let gid = internal_id_to_gid(&id);

    // Unsupported at runtime: the error names the key.
    let error = handle_change_option(&ctx, vec![json!(gid.clone()), json!({"split": "4"})])
        .await
        .expect_err("split cannot be changed at runtime");
    assert!(error.message.contains("split"), "{}", error.message);

    // `pause` maps onto the lifecycle for HTTP tasks.
    handle_change_option(&ctx, vec![json!(gid.clone()), json!({"pause": "true"})])
        .await
        .expect("pause option");
    let state = ctx
        .dispatcher
        .status(&TaskId::Http(uuid))
        .await
        .expect("status")
        .state;
    assert_eq!(state, DownloadState::Paused);

    // BitTorrent-only options are rejected early for HTTP tasks.
    for options in [json!({"select-file": "1"}), json!({"max-download-limit": "1024"})] {
        let error = handle_change_option(&ctx, vec![json!(gid.clone()), options])
            .await
            .expect_err("BT-only option");
        assert!(error.message.contains("BitTorrent"), "{}", error.message);
    }

    let _ = tmp;
}

/// Every aria2 lifecycle notification must be published exactly once per
/// transition — engine and RPC handler must not both emit the same event.
#[tokio::test(flavor = "multi_thread")]
#[timeout(60_000)]
async fn http_lifecycle_notifications_are_emitted_exactly_once() {
    let server = crate::test_harness::TestServer::new(4 * 1024 * 1024).await;
    let (tmp, dm) = crate::tests::dispatcher_tests::make_manager();
    let out_dir = tmp.path().join("out");
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    let mut registry = BackendRegistry::new();
    registry.register_arc(TaskKind::Http, dm.clone());
    let event_bus = Arc::new(EventBus::new(256));
    let ctx = make_ctx_with(Arc::new(registry), event_bus.clone());

    // A throttled real download: resume stays active instead of racing into a
    // probe failure, which keeps the notification set deterministic.
    let task_id = ctx
        .dispatcher
        .start(StartDownloadRequest {
            kind: Some(TaskKind::Http),
            url: server.file_url_bandwidth(32 * 1024),
            destination_dir: out_dir.to_string_lossy().to_string(),
            file_name: Some("notify.bin".into()),
            user_agent: None,
            thread_mode: Some(crate::types::ThreadMode::Fixed),
            thread_count: Some(1),
            max_retries: Some(1),
            checksum: Some(crate::types::ChecksumMode::None),
            expected_checksum: None,
            selected_file_indices: None,
            headers: None,
            start_paused: false,
            mirror_urls: None,
            priority: None,
        })
        .await
        .expect("start download");
    let gid = internal_id_to_gid(&task_id.raw_id());
    let mut rx = event_bus.subscribe();

    handle_pause(&ctx, vec![json!(gid.clone())])
        .await
        .expect("pause");
    assert_eq!(
        take_aria2_events(&mut rx),
        vec!["aria2.onDownloadPause"],
        "pause must publish exactly one notification"
    );

    handle_unpause(&ctx, vec![json!(gid.clone())])
        .await
        .expect("unpause");
    assert_eq!(
        take_aria2_events(&mut rx),
        vec!["aria2.onDownloadStart"],
        "unpause must publish exactly one notification"
    );

    handle_remove(&ctx, vec![json!(gid.clone())])
        .await
        .expect("remove");
    assert_eq!(
        take_aria2_events(&mut rx),
        vec!["aria2.onDownloadStop"],
        "remove must publish exactly one notification"
    );
}

/// Drain the bus and keep only the aria2 notification names.
fn take_aria2_events(
    rx: &mut tokio::sync::broadcast::Receiver<DownloadEvent>,
) -> Vec<String> {
    let mut names = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let DownloadEvent::Aria2Notification { event_name, .. } = event {
            names.push(event_name);
        }
    }
    names
}

// ── aria2 temp-file cleanup ───────────────────────────────────────────

/// Only `.torrent` files older than an hour may be cleaned from the aria2
/// temp directory; fresh torrents and unrelated files must survive.
#[test]
#[timeout(10_000)]
fn cleanup_old_aria2_temp_files_removes_only_stale_torrents() {
    let dir = std::env::temp_dir().join("limedl_aria2");
    std::fs::create_dir_all(&dir).expect("create aria2 temp dir");

    let unique = uuid::Uuid::new_v4();
    let stale = dir.join(format!("stale-{unique}.torrent"));
    let fresh = dir.join(format!("fresh-{unique}.torrent"));
    let other = dir.join(format!("other-{unique}.txt"));
    std::fs::write(&stale, b"stale").expect("write stale torrent");
    std::fs::write(&fresh, b"fresh").expect("write fresh torrent");
    std::fs::write(&other, b"other").expect("write unrelated file");

    let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&stale)
        .expect("open stale torrent")
        .set_modified(two_hours_ago)
        .expect("age the stale torrent");

    cleanup_old_aria2_temp_files();

    assert!(!stale.exists(), "a >1h old .torrent must be removed");
    assert!(fresh.exists(), "a fresh .torrent must survive");
    assert!(other.exists(), "non-torrent files must survive");

    let _ = std::fs::remove_file(&fresh);
    let _ = std::fs::remove_file(&other);
}

// ── split_aria2_tail: client call shapes ──────────────────────────────────

#[test]
#[timeout(10_000)]
fn split_aria2_tail_ariang_add_torrent_shape() {
    // AriaNg: `addTorrent(torrent, [], options)`.
    let params = vec![
        json!("base64"),
        json!([]),
        json!({"dir": "/tmp/x", "pause": "true"}),
    ];
    let (uris, options, position) = split_aria2_tail(&params, 1);
    assert_eq!(uris, Some(Vec::new()));
    assert_eq!(options.as_ref().unwrap()["dir"], "/tmp/x");
    assert_eq!(position, None);
}

#[test]
#[timeout(10_000)]
fn split_aria2_tail_legacy_add_torrent_shape() {
    // limedl's own tests used to send `addTorrent(torrent, options)`.
    let params = vec![json!("base64"), json!({"dir": "/tmp/x"})];
    let (uris, options, position) = split_aria2_tail(&params, 1);
    assert_eq!(uris, None);
    assert_eq!(options.as_ref().unwrap()["dir"], "/tmp/x");
    assert_eq!(position, None);
}

#[test]
#[timeout(10_000)]
fn split_aria2_tail_add_uri_position() {
    // `addUri(uris, options, position)`.
    let params = vec![
        json!(["http://a"]),
        json!({"dir": "/tmp/x"}),
        json!(0),
    ];
    let (uris, options, position) = split_aria2_tail(&params, 1);
    assert_eq!(uris, None);
    assert!(options.is_some());
    assert_eq!(position, Some(0));
}

#[test]
#[timeout(10_000)]
fn split_aria2_tail_full_add_torrent_shape() {
    // `addTorrent(torrent, uris, options, position)`.
    let params = vec![
        json!("base64"),
        json!(["http://web-seed/"]),
        json!({"pause": "true"}),
        json!(2),
    ];
    let (uris, options, position) = split_aria2_tail(&params, 1);
    assert_eq!(uris, Some(vec!["http://web-seed/".to_string()]));
    assert!(options.is_some());
    assert_eq!(position, Some(2));
}

// ── bitfield_from_bits ────────────────────────────────────────────────────

#[test]
#[timeout(10_000)]
fn bitfield_from_bits_packs_msb_first() {
    // 4 pieces -> one nibble, piece 0 is the highest bit.
    assert_eq!(bitfield_from_bits(&[true, false, true, false]), "a");
    assert_eq!(bitfield_from_bits(&[false, false, false, true]), "1");
}

#[test]
#[timeout(10_000)]
fn bitfield_from_bits_pads_the_trailing_nibble() {
    // 1 piece -> the single bit is the MSB of the nibble.
    assert_eq!(bitfield_from_bits(&[true]), "8");
    assert_eq!(bitfield_from_bits(&[false]), "0");
    // 5 pieces -> two nibbles, the second left-aligned.
    assert_eq!(bitfield_from_bits(&[true, true, true, true, true]), "f8");
}

#[test]
#[timeout(10_000)]
fn bitfield_from_bits_empty_is_empty() {
    assert_eq!(bitfield_from_bits(&[]), "");
}
