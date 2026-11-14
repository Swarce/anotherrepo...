//! API dispatch: one module per backend service. Unknown routes answer OK with no fields and are
//! counted and logged, so a device log lists exactly what is still missing.

use crate::app::ApiResult;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Map, Value};

pub mod common;

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// Returns the handler result and whether the route is known.
pub fn dispatch(ctx: &Ctx, api: &str, env: &RequestEnvelope) -> (ApiResult, bool) {
    let r = match api {
        "/common/get_version" => common::get_version(ctx, env),
        _ => return (Ok(Map::new()), false),
    };
    (r, true)
}

pub(crate) fn ok(v: Value) -> ApiResult {
    Ok(obj(v))
}

#[allow(dead_code)]
pub(crate) fn empty() -> ApiResult {
    ok(json!({}))
}
