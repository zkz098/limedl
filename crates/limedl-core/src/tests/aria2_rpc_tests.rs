//! Unit tests for aria2_rpc.rs — JSON-RPC gateway.
//!
//! Coverage:
//!   - JSON-RPC protocol types: JsonRpcResponse, JsonRpcError serialization
//!   - Factory functions: error_response, success_response, make_error
//!   - Stateless handlers: handle_version, handle_list_methods, handle_list_notifications
//!   - ID conversion: internal_id_to_gid
//!   - Request deserialization: JsonRpcRequest
//!
//! What requires E2E / full backends (not tested here):
//!   - dispatch_method() — depends on RpcContext with Arc<BackendRegistry>
//!   - process_jsonrpc_message() — depends on RpcContext
//!   - All handle_* functions that call ctx.manager or ctx.bt_backend
//!   - WebSocket and HTTP server integration

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
            code: -32601,
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
    assert_eq!(parsed["error"]["code"], -32601);
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
    assert_eq!(resp.error.as_ref().unwrap().code, -32601);
}

#[test]
#[timeout(10_000)]
fn error_response_invalid_params() {
    let resp = error_response(None, ERR_INVALID_PARAMS, "Missing GID parameter");
    assert_eq!(resp.error.as_ref().unwrap().code, -32602);
}

#[test]
#[timeout(10_000)]
fn error_response_internal_error() {
    let resp = error_response(None, ERR_INTERNAL, "Something went wrong");
    assert_eq!(resp.error.as_ref().unwrap().code, -32603);
}

#[test]
#[timeout(10_000)]
fn error_response_serializes_to_valid_json() {
    let resp = error_response(Some(json!(42)), ERR_METHOD_NOT_FOUND, "Not found");
    let json_str = serde_json::to_string(&resp).expect("serialize");
    let parsed: Value = serde_json::from_str(&json_str).expect("valid JSON");

    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["error"]["code"], -32601);
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
    assert_eq!(result["version"], "0.1.0");
    let features = result["enabledFeatures"]
        .as_array()
        .expect("enabledFeatures should be an array");
    assert!(features.contains(&json!("BitTorrent")));
    assert!(features.contains(&json!("HTTPS")));
    assert!(features.contains(&json!("Async DNS")));
    assert!(features.contains(&json!("Firefox3 Cookie")));
    assert!(features.contains(&json!("GZip")));
    assert!(features.contains(&json!("Message Digest")));
    assert!(features.contains(&json!("XML-RPC")));
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
    assert!(methods.contains(&json!("aria2.getVersion")));
    assert!(methods.contains(&json!("aria2.getFiles")));
    assert!(methods.contains(&json!("aria2.getUris")));
    assert!(methods.contains(&json!("aria2.getPeers")));
    assert!(methods.contains(&json!("aria2.shutdown")));
    assert!(methods.contains(&json!("system.listMethods")));
    assert!(methods.contains(&json!("system.listNotifications")));

    // Verify the exact count
    assert_eq!(methods.len(), 30);
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
