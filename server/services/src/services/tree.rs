//! `/tree/*`: the growth tree. The client has no tree data, so the server sends the whole square
//! graph (`collection::tree_squares`) with the player's progress; opening squares is free offline.
//! docs/research/13-menus.md §3.

use crate::accounts::Change;
use crate::app::{ApiError, ApiResult};
use crate::collection::{self, OwnedStatue};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// `treeInfo` for an owned statue: every square, opened ones flagged. The client needs every
/// square's `squareEffectList` object and both material lists, and a closed square's
/// `beforeSquareId` must name an existing square.
pub fn tree_info(o: &OwnedStatue) -> Value {
    let squares: Vec<Value> = collection::find(&o.statue_id)
        .map(collection::tree_squares)
        .unwrap_or_default()
        .iter()
        .map(|s| {
            let open = s.effect == 0 || o.tree.contains(&s.id);
            json!({ "squareId": s.id, "posX": s.pos.0, "posY": s.pos.1, "openFlg": open as i64,
                    "squareEffectList": { "squareEffectId": s.effect, "param": s.param },
                    "beforeSquareId": s.before, "iconLevel": if s.effect == 0 { 0 } else { 1 },
                    "openMaterialList": [], "lockList": [], "unlockFlg": 1 })
        })
        .collect();
    json!({ "treeId": o.statue_id, "squareList": squares })
}

fn uid(env: &RequestEnvelope) -> i64 {
    env.param
        .get("statueuId")
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

/// `TreeListRequest {statueuId}` -> `{treeInfo, itemList}`.
pub fn skill_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let o = p
        .collection
        .known_statue(uid(env))
        .ok_or_else(|| ApiError::Internal(format!("no statue {}", uid(env))))?;
    ok(json!({ "treeInfo": tree_info(o), "itemList": [] }))
}

/// `TreeSquareRequest {statueuId, squareId: [..], multiOpen}`: open each listed square whose
/// previous square is open (in order, so a path can be opened at once).
pub fn acquisition(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let id = uid(env);
    let want: Vec<String> = match env.param.get("squareId") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    };
    let (p, ok_id) = with_profile(ctx, env, |p| {
        let Some(o) = p.collection.statues.iter_mut().find(|o| o.uid == id) else {
            return (false, Change::None);
        };
        let Some(d) = collection::find(&o.statue_id) else {
            return (false, Change::None);
        };
        let squares = collection::tree_squares(d);
        let mut changed = false;
        for w in &want {
            let Some(sq) = squares.iter().find(|s| &s.id == w) else {
                continue;
            };
            let before_open = sq.before.is_empty()
                || o.tree.contains(&sq.before)
                || squares.iter().any(|s| s.id == sq.before && s.effect == 0);
            if sq.effect != 0 && before_open && !o.tree.contains(w) {
                o.tree.push(w.clone());
                changed = true;
            }
        }
        (
            true,
            if changed {
                Change::Progress
            } else {
                Change::None
            },
        )
    })?;
    if !ok_id {
        return Err(ApiError::Internal(format!("no statue {id}")));
    }
    let o = p.collection.known_statue(id).unwrap();
    ok(json!({ "treeInfo": tree_info(o), "itemList": [],
               "userInfo": crate::services::user::user_info(&p),
               "statueInfo": collection::statue_info(o) }))
}

#[cfg(test)]
mod tests {
    use crate::services::dispatch;
    use crate::Ctx;
    use jdr_core::envelope::RequestEnvelope;
    use jdr_core::settings::Settings;
    use serde_json::{json, Value};

    fn env(uuid: &str, param: Value) -> RequestEnvelope {
        RequestEnvelope {
            uuid: uuid.into(),
            session: String::new(),
            language: 1,
            date: 20261114103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    #[test]
    fn tree_opens_in_order_and_raises_stats() {
        let dir = std::env::temp_dir().join(format!("jdr-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let (r, _) = dispatch(&ctx, "/team/team_info", &env(&uuid, json!({})));
        r.unwrap();
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let o = p.collection.statues[0].clone();
        let (r, known) = dispatch(
            &ctx,
            "/tree/skill_list",
            &env(&uuid, json!({"statueuId": o.uid})),
        );
        assert!(known);
        let t = r.unwrap()["treeInfo"].clone();
        let sq = t["squareList"].as_array().unwrap();
        assert_eq!(sq[0]["openFlg"], 1);
        assert!(sq
            .iter()
            .all(|s| s["squareEffectList"].is_object() && s["openMaterialList"].is_array()));
        let ids: Vec<&str> = sq.iter().map(|s| s["squareId"].as_str().unwrap()).collect();
        for s in sq.iter().skip(1) {
            assert!(ids.contains(&s["beforeSquareId"].as_str().unwrap()));
        }
        // Square 3 needs square 2 first; asking for both in order opens both.
        let (r, _) = dispatch(
            &ctx,
            "/tree/acquisition",
            &env(&uuid, json!({"statueuId": o.uid, "squareId": ["3"]})),
        );
        assert_eq!(r.unwrap()["treeInfo"]["squareList"][2]["openFlg"], 0);
        let (r, _) = dispatch(
            &ctx,
            "/tree/acquisition",
            &env(&uuid, json!({"statueuId": o.uid, "squareId": ["2", "3"]})),
        );
        let r = r.unwrap();
        assert_eq!(r["treeInfo"]["squareList"][2]["openFlg"], 1);
        let before = crate::collection::statue_info(&o);
        assert!(r["statueInfo"]["atk"].as_i64().unwrap() > before["atk"].as_i64().unwrap());
    }
}
