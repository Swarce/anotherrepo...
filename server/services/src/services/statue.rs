//! `/statue/*`: the player's statues (characters).

use crate::accounts::{Change, Profile};
use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// Change the calling device's account under the account lock, with its collection granted
/// first. A grant counts as progress (written with a backup rotation); otherwise `f` decides.
pub fn with_profile<R>(
    ctx: &Ctx,
    env: &RequestEnvelope,
    f: impl FnOnce(&mut Profile) -> (R, Change),
) -> Result<(Profile, R), ApiError> {
    let now = ctx.server_date();
    match ctx.accounts.update(&env.uuid, |p| {
        // The one-time starting stock (Build 21) comes with gold for enhancing (ours).
        let fresh = !p.collection.items_granted;
        let granted = p.collection.ensure(&now);
        if fresh {
            p.gold += crate::items::STARTING_GOLD;
        }
        let (r, change) = f(p);
        (r, if granted { Change::Progress } else { change })
    }) {
        Ok(Some(x)) => Ok(x),
        Ok(None) => Err(ApiError::Status(10003)), // E_SESSION_NG
        Err(e) => Err(ApiError::Internal(format!(
            "account unreadable or unwritable: {e}"
        ))),
    }
}

/// The calling device's account, with its collection granted (saved if that changed it).
pub fn profile(ctx: &Ctx, env: &RequestEnvelope) -> Result<Profile, ApiError> {
    with_profile(ctx, env, |_| ((), Change::None)).map(|(p, ())| p)
}

/// `StatueListResponse`: every owned statue.
pub fn statue_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let list: Vec<Value> = p
        .collection
        .statues
        .iter()
        .filter(|o| collection::find(&o.statue_id).is_some())
        .map(collection::statue_info)
        .collect();
    ok(json!({ "allStatueNum": list.len(), "statueList": list, "sortType": 0 }))
}

/// `MindListResponse`: every owned mind; `useFlg` marks the ones equipped on any team seat.
pub fn mind_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let used = p.collection.equipped_minds();
    let list: Vec<Value> = p
        .collection
        .minds
        .iter()
        .map(|m| {
            let mut v = collection::mind_info(m);
            v["useFlg"] = json!(used.contains(&m.uid) as i64);
            v
        })
        .collect();
    ok(
        json!({ "mindMax": collection::MIND_MAX, "mindList": list, "mindOverFlg": 0, "sortType": 0,
               "materialSortType": 0, "goldRatioInteger": crate::items::GOLD_RATIO_DENOMINATOR, "goldAddition": 0, "storageInfoList": [] }),
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
        .known_statue(uid)
        .ok_or_else(|| ApiError::Internal(format!("no statue {uid}")))?;
    ok(json!({ "statueInfo": collection::statue_info(o) }))
}

/// `/mind/protect {minduId, protectedFlg}`: the heart lock. The client reads
/// `response.mindInfo.minduId`, so the full MindInfo is required (an empty reply crashed it).
pub fn mind_protect(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let uid = env
        .param
        .get("minduId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let flag = env
        .param
        .get("protectedFlg")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        == 1;
    let (p, found) = with_profile(ctx, env, |p| {
        match p.collection.minds.iter_mut().find(|m| m.uid == uid) {
            Some(m) => {
                m.protected = flag;
                (true, Change::Bookkeeping)
            }
            None => (false, Change::None),
        }
    })?;
    if !found {
        return Err(ApiError::Status(51101)); // mind missing
    }
    let m = p.collection.minds.iter().find(|m| m.uid == uid).unwrap();
    let mut info = collection::mind_info(m);
    info["useFlg"] = json!(p.collection.equipped_minds().contains(&uid) as i64);
    ok(json!({ "mindInfo": info }))
}

/// `/mind/protect_multi {storageId, protectMindList, unprotectMindList}`: the client updates its
/// own list first; `{}` is enough.
pub fn mind_protect_multi(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let ids = |k: &str| -> Vec<i64> {
        env.param
            .get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default()
    };
    let (on, off) = (ids("protectMindList"), ids("unprotectMindList"));
    with_profile(ctx, env, |p| {
        for m in p.collection.minds.iter_mut() {
            if on.contains(&m.uid) {
                m.protected = true;
            } else if off.contains(&m.uid) {
                m.protected = false;
            }
        }
        ((), Change::Bookkeeping)
    })?;
    ok(json!({}))
}
