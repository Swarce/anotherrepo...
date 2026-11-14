//! `/statue/*`: the player's statues (characters).

use crate::accounts::Profile;
use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// The calling device's account, with its collection granted (saved if that changed it).
pub fn profile(ctx: &Ctx, env: &RequestEnvelope) -> Result<Profile, ApiError> {
    let mut p = match ctx.accounts.get(&env.uuid) {
        Ok(Some(p)) => p,
        Ok(None) => return Err(ApiError::Status(10003)), // E_SESSION_NG
        Err(e) => return Err(ApiError::Internal(format!("account unreadable: {e}"))),
    };
    let now = ctx.server_date();
    if p.collection.ensure(&now) {
        ctx.accounts.save(&p).map_err(ApiError::Internal)?;
    }
    Ok(p)
}

/// `StatueListResponse`: every owned statue.
pub fn statue_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let list: Vec<Value> = p
        .collection
        .statues
        .iter()
        .map(collection::statue_info)
        .collect();
    ok(json!({ "allStatueNum": list.len(), "statueList": list, "sortType": 0 }))
}

/// `MindListResponse`: every owned mind.
pub fn mind_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let list: Vec<Value> = p
        .collection
        .minds
        .iter()
        .map(collection::mind_info)
        .collect();
    ok(
        json!({ "mindMax": collection::MIND_MAX, "mindList": list, "mindOverFlg": 0, "sortType": 0,
               "materialSortType": 0, "goldRatioInteger": 0, "goldAddition": 0, "storageInfoList": [] }),
    )
}

/// `MindInfoResponse` for one mind (`minduId`).
pub fn mind_info(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let uid = env
        .param
        .get("minduId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let o = p
        .collection
        .minds
        .iter()
        .find(|m| m.uid == uid)
        .ok_or_else(|| ApiError::Internal(format!("no mind {uid}")))?;
    ok(json!({ "mindInfo": collection::mind_info(o) }))
}

/// `StatueInfoResponse` for one statue (`statueuId`).
pub fn statue_info(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let uid = env
        .param
        .get("statueuId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let o = p
        .collection
        .statue(uid)
        .ok_or_else(|| ApiError::Internal(format!("no statue {uid}")))?;
    ok(json!({ "statueInfo": collection::statue_info(o) }))
}
