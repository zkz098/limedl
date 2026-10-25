//! JSON-RPC envelope: requests, responses, errors and the `ERR_*` codes.
//!
//! Split out of `protocol` so `context` (which builds `JsonRpcError`s for auth
//! failures) does not have to depend on `protocol` (which needs `RpcContext`
//! for its aria2 status mapping) — that pair was the last dependency cycle
//! inside `aria2_rpc`.
//!
//! The `ERR_*` constants are documented in `protocol`'s history: aria2 answers
//! `code: 1` for every domain failure, and `ERR_PARSE`/`ERR_INVALID_REQUEST`
//! stay at their JSON-RPC values because they are wire-level failures that
//! never reach a method.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub(crate) jsonrpc: String,
    #[serde(default)]
    pub(crate) id: Option<Value>,
    pub(crate) method: String,
    #[serde(default)]
    pub(crate) params: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub(crate) jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub(crate) code: i32,
    pub(crate) message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcNotification {
    pub(crate) jsonrpc: &'static str,
    pub(crate) method: String,
    pub(crate) params: Vec<Value>,
}

pub const ERR_PARSE: i32 = -32700;
pub const ERR_INVALID_REQUEST: i32 = -32600;
/// aria2 answers `code: 1` for **every** domain failure — unauthorized, unknown
/// method, missing/invalid params, unknown GID, internal error — rather than the
/// JSON-RPC-specific codes. limedl matches aria2 on the wire (AriaNg and other
/// clients key off `message`); the separate constants are kept for readability
/// at the call sites. `ERR_PARSE`/`ERR_INVALID_REQUEST` stay standard because
/// they are wire-level failures that never reach a method.
///
/// Verified against aria2 1.37.0 by the Tier 2 oracle (`oracle_error_objects`).
pub const ERR_METHOD_NOT_FOUND: i32 = 1;
pub const ERR_INVALID_PARAMS: i32 = 1;
pub const ERR_INTERNAL: i32 = 1;

pub fn make_error(code: i32, message: impl Into<String>) -> JsonRpcError {
    JsonRpcError {
        code,
        message: message.into(),
    }
}

pub fn error_response(id: Option<Value>, code: i32, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(make_error(code, message)),
    }
}

pub fn success_response(id: Option<Value>, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}
