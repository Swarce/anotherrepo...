//! `/common/*`: version check, maintenance state, system config.

use crate::app::{ApiResult, API_PREFIX};
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::json;

/// `GetVersionResponse {url, s3_url, is_ca, ca_message}`: always "current"; hands out our own
/// API base and web-page base (assets get their URLs from `/client/getfiledata`) (the client stores both verbatim and appends paths).
pub fn get_version(ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    let base = ctx.base_url();
    ok(json!({
        "url": format!("{base}{API_PREFIX}"),
        "s3_url": format!("{base}{}", crate::web::WEB_PREFIX),
        "is_ca": 0,
        "ca_message": "",
    }))
}
