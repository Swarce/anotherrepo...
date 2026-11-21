//! `/book/*` (the dictionary) and the statue screens that only need the current statue back
//! (evolution preview, enhance). docs/research/13-menus.md §4.

use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// `BookStatueListResponse`: every statue, all met (everything is owned offline).
pub fn statue_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    touch(ctx, env, "book_statue");
    let list: Vec<Value> = collection::statues()
        .iter()
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
                boost: 0,
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
pub fn mind_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    touch(ctx, env, "book_mind");
    let list: Vec<Value> = collection::minds()
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let info = collection::mind_info(&collection::OwnedMind {
                uid: 0,
                mind_id: d.mind_id.clone(),
                get_date: String::new(),
                protected: false,
                level: 1,
                exp: 0,
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

/// Enhance with Minds as fodder (`/statue/strengthening`): not offered offline yet (Minds stay
/// owned); the statue is unchanged. In this mode the client replaces its whole Mind list with
/// `mindList`, so the full list is sent back (an empty one blanked the Mind list).
pub fn statue_strengthening(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let s = owned(ctx, env)?;
    let minds: Vec<Value> = p
        .collection
        .minds
        .iter()
        .map(crate::collection::mind_info)
        .collect();
    ok(
        json!({ "statueInfo": s, "userInfo": crate::services::user::user_info(&p), "mindList": minds,
               "composeType": 1, "itemList": null }),
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

/// Count a first-time visit for the missions (opening an encyclopedia).
fn touch(ctx: &Ctx, env: &RequestEnvelope, counter: &str) {
    let _ = crate::services::statue::with_profile(ctx, env, |p| {
        if p.progress
            .missions
            .totals
            .get(counter)
            .copied()
            .unwrap_or(0)
            > 0
        {
            return ((), crate::accounts::Change::None);
        }
        p.progress.missions.bump(counter, 1, jdr_core::clock::now());
        ((), crate::accounts::Change::Bookkeeping)
    });
}
