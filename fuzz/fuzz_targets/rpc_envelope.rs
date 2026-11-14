#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use limedl_rpc::fuzz_api::{JsonRpcRequest, parse_aria2_limit, parse_int_param, parse_keys};
use serde_json::{Map, Value, json};

#[derive(Arbitrary, Debug)]
enum FuzzId {
    Null,
    Num(i64),
    Str(String),
}

impl FuzzId {
    fn into_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Num(n) => json!(n),
            Self::Str(s) => Value::String(s),
        }
    }
}

#[derive(Arbitrary, Debug)]
enum FuzzParam {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    StrArray(Vec<String>),
    Obj(Vec<(String, String)>),
}

impl FuzzParam {
    fn into_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(b) => Value::Bool(b),
            Self::Int(n) => json!(n),
            Self::Str(s) => Value::String(s),
            Self::StrArray(v) => Value::Array(v.into_iter().map(Value::String).collect()),
            Self::Obj(entries) => {
                let mut map = Map::new();
                for (k, v) in entries {
                    map.insert(k, Value::String(v));
                }
                Value::Object(map)
            }
        }
    }
}

#[derive(Arbitrary, Debug)]
struct StructuredRpcCall {
    jsonrpc: String,
    id: Option<FuzzId>,
    method: String,
    params: Option<Vec<FuzzParam>>,
}

#[derive(Arbitrary, Debug)]
enum FuzzInput {
    Single(StructuredRpcCall),
    Batch(Vec<StructuredRpcCall>),
    Raw(Vec<u8>),
}

fn exercise_call(call: StructuredRpcCall) {
    let params_val: Option<Vec<Value>> = call
        .params
        .map(|p| p.into_iter().map(FuzzParam::into_value).collect());
    let mut obj = Map::new();
    obj.insert("jsonrpc".into(), Value::String(call.jsonrpc));
    if let Some(id) = call.id {
        obj.insert("id".into(), id.into_value());
    }
    obj.insert("method".into(), Value::String(call.method));
    if let Some(ref p) = params_val {
        obj.insert("params".into(), Value::Array(p.clone()));
    }
    let _ = serde_json::from_value::<JsonRpcRequest>(Value::Object(obj));

    if let Some(params) = params_val {
        for item in &params {
            let _ = parse_aria2_limit(item);
        }
        let _ = parse_int_param(&params, 0);
        let _ = parse_int_param(&params, 1);
        let _ = parse_keys(&params, 0);
        let _ = parse_keys(&params, 1);
    }
}

fuzz_target!(|input: FuzzInput| {
    match input {
        FuzzInput::Single(call) => exercise_call(call),
        FuzzInput::Batch(calls) => {
            for call in calls.into_iter().take(32) {
                exercise_call(call);
            }
        }
        FuzzInput::Raw(data) => {
            let _ = serde_json::from_slice::<JsonRpcRequest>(&data);
            let _ = serde_json::from_slice::<Vec<JsonRpcRequest>>(&data);
            if let Ok(value) = serde_json::from_slice::<Value>(&data) {
                let _ = parse_aria2_limit(&value);
                if let Value::Array(items) = &value {
                    let _ = parse_int_param(items, 0);
                    let _ = parse_keys(items, 0);
                }
            }
        }
    }
});
