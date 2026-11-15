//! `/book/*` (the dictionary) and the statue screens that only need the current statue back
//! (evolution preview, enhance). docs/research/13-menus.md §4.

use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// `BookStatueListResponse`: every statue, all met (everything is owned offline).
pub fn statue_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    let list: Vec<Value> = collection::playable_statues()
        .enumerate()
        .map(|(i, d)| {
            let at = |level: i64| {
                collection::statue_info(&collection::OwnedStatue {
                    uid: 0,
                    statue_id: d.statue_id.clone(),
                    level,
                    exp: 0,
                    get_date: String::new(),
                    tree: Vec::new(),
                })
            };
            json!({ "statueListId": i + 1, "statueId": d.statue_id, "statueInfoLvMin": at(1),
                    "statueInfoLvMax": at(collection::MAX_LEVEL), "statueInfoTreeMax": at(collection::MAX_LEVEL),
                    "encountFlg": 1 })
        })
        .collect();
    ok(json!({ "encountNum": list.len(), "statueList": list }))
}

/// `BookMindListResponse`: every mind.
pub fn mind_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    let list: Vec<Value> = collection::minds()
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let info = collection::mind_info(&collection::OwnedMind {
                uid: 0,
                mind_id: d.mind_id.clone(),
                get_date: String::new(),
            });
            json!({ "mindListId": i + 1, "mindId": d.mind_id, "mindInfo": info, "encountFlg": 1 })
        })
        .collect();
    ok(json!({ "encountNum": list.len(), "mindList": list }))
}

pub fn item_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "allNum": 0, "itemList": [] }))
}

pub fn yogo_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "allNum": 0, "yogoList": [] }))
}

fn owned(ctx: &Ctx, env: &RequestEnvelope) -> Result<Value, ApiError> {
    let p = crate::services::statue::profile(ctx, env)?;
    let uid = env
        .param
        .get("statueuId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    p.collection
        .known_statue(uid)
        .map(collection::statue_info)
        .ok_or_else(|| ApiError::Internal(format!("no statue {uid}")))
}

/// `StatueEvolutionInfoResponse`: no evolution offline yet; every view shows the statue as it is.
pub fn evolution_info(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let s = owned(ctx, env)?;
    ok(
        json!({ "prevBaseStatueInfo": s, "afterBaseStatueInfo": s, "afterEvolutionStatueInfo": s,
               "afterEvolutionStatueInfoMax": s, "afterEvolutionStatueInfoTreeMax": s }),
    )
}

/// Enhance (`/statue/strengthening`, `_item`): no materials offline yet; the statue is unchanged.
pub fn statue_strengthening(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let s = owned(ctx, env)?;
    ok(
        json!({ "statueInfo": s, "userInfo": crate::services::user::user_info(&p), "mindList": [],
               "composeType": 1, "itemList": [] }),
    )
}

/// `SellResponse`: selling is not offered offline (everything owned stays owned).
pub fn sell(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    ok(
        json!({ "userInfo": crate::services::user::user_info(&p), "dataList": {}, "vm": p.gold,
               "getItemList": [], "itemList": [] }),
    )
}
