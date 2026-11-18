//! `/tree/*`: the growth tree. The client has no tree data, so the server sends the whole square
//! graph (`collection::tree_squares`) with the player's progress. Since Build 23 squares cost tree
//! materials and gold, and the fork squares are locked behind an Expansion Key (amounts ours; the
//! client checks costs only against the tree screen's `itemList`, gold being item 20).
//! docs/research/13-menus.md §3.

use crate::accounts::Change;
use crate::app::{ApiError, ApiResult};
use crate::collection::{self, OwnedStatue};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// Gold is an ordinary tree material with this item id (client `golditemId`).
const GOLD_ITEM: i64 = 20;
const E_VM_LACK: i64 = 10102;
const E_ITEM_LACK: i64 = 10108;

/// Tree materials by tier for a statue type. The colours are the game's (the item details: Courage
/// blue, Wisdom green, Spirit red); which tier each depth uses is ours: Shard, Stone, Brooch, Gem,
/// Tablet, Slab; Unity white and Solitude black (Gray Shard, Tablet, Slab, Chain, Crown).
fn materials(statue_type: i64) -> &'static [i64] {
    match statue_type {
        1 => &[1011, 1002, 1005, 1008, 1015, 1018],
        2 => &[1012, 1003, 1006, 1009, 1016, 1019],
        3 => &[1013, 1004, 1007, 1010, 1017, 1020],
        4 => &[1014, 1030, 1032, 1034, 1036],
        _ => &[1014, 1031, 1033, 1035, 1037],
    }
}

/// The Expansion Key that unlocks a statue type's fork squares (colour as above; ours).
fn key(statue_type: i64) -> i64 {
    match statue_type {
        1 => 1021,
        2 => 1022,
        3 => 1023,
        _ => 1038,
    }
}

/// What a square costs to open (materials, gold last) and, for a locked square, its key.
/// (item id, count) pairs.
type Costs = Vec<(i64, i64)>;

fn cost(d: &collection::StatueDef, sq: &collection::TreeSquare) -> (Costs, Costs) {
    if sq.effect == 0 {
        return (Vec::new(), Vec::new());
    }
    let depth = sq.pos.0.max(1);
    let mats = materials(d.statue_type());
    let tier = ((depth - 1) as usize).min(mats.len() - 1);
    let open = vec![
        (mats[tier], 2 + depth),
        (GOLD_ITEM, 1000 * depth * d.rarity().max(1)),
    ];
    let lock = if sq.id.ends_with("f1") {
        vec![(key(d.statue_type()), 1)]
    } else {
        Vec::new()
    };
    (open, lock)
}

fn material_info(list: &[(i64, i64)]) -> Vec<Value> {
    list.iter()
        .map(|(id, n)| json!({ "itemId": id, "name": format!("ITEM_{id:04}_NAME"), "icon": id, "bg": 0, "flame": 0, "count": n }))
        .collect()
}

/// The tree screen's own counts: every material the tree uses and gold (item 20) as `vm`.
fn item_list(p: &crate::accounts::Profile, d: Option<&collection::StatueDef>) -> Vec<Value> {
    let mut ids: Vec<i64> = d
        .map(|d| {
            let mut v = materials(d.statue_type()).to_vec();
            v.push(key(d.statue_type()));
            v
        })
        .unwrap_or_default();
    ids.sort();
    ids.dedup();
    let mut out: Vec<Value> = ids
        .iter()
        .filter_map(|id| crate::items::find(*id))
        .map(|it| crate::items::item_info(it, p.collection.item_count(it.item_id)))
        .collect();
    out.push(json!({ "itemId": GOLD_ITEM, "name": "ITEM_0020_NAME", "comment": "ITEM_0020_DETAIL",
                     "icon": GOLD_ITEM.to_string(), "bg": 0, "flame": 0, "type": 0, "value": 0, "unit": "",
                     "sell": 0, "priority": 0, "count": p.gold }));
    out
}

fn can_pay(p: &crate::accounts::Profile, list: &[(i64, i64)]) -> Result<(), i64> {
    for (id, n) in list {
        if *id == GOLD_ITEM {
            if p.gold < *n {
                return Err(E_VM_LACK);
            }
        } else if p.collection.item_count(*id) < *n {
            return Err(E_ITEM_LACK);
        }
    }
    Ok(())
}

fn pay(p: &mut crate::accounts::Profile, list: &[(i64, i64)]) {
    for (id, n) in list {
        if *id == GOLD_ITEM {
            p.gold -= n;
        } else {
            p.collection.add_item(*id, -n);
        }
    }
}

/// `treeInfo` for an owned statue: every square, opened ones flagged. The client needs every
/// square's `squareEffectList` object and both material lists, and a closed square's
/// `beforeSquareId` must name an existing square.
pub fn tree_info(o: &OwnedStatue) -> Value {
    let d = collection::find(&o.statue_id);
    let squares: Vec<Value> = d
        .map(collection::tree_squares)
        .unwrap_or_default()
        .iter()
        .map(|s| {
            let open = s.effect == 0 || o.tree.contains(&s.id);
            let (mats, lock) = d.map(|d| cost(d, s)).unwrap_or_default();
            // An opened square is unlocked; a closed fork square shows its key.
            let locked = !open && !lock.is_empty();
            json!({ "squareId": s.id, "posX": s.pos.0, "posY": s.pos.1, "openFlg": open as i64,
                    "squareEffectList": { "squareEffectId": s.effect, "param": s.param },
                    "beforeSquareId": s.before, "iconLevel": if s.effect == 0 { 0 } else { 1 },
                    "openMaterialList": if open { vec![] } else { material_info(&mats) },
                    "lockList": if locked { material_info(&lock) } else { vec![] },
                    "unlockFlg": if locked { 0 } else { 1 } })
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
    ok(
        json!({ "treeInfo": tree_info(o), "itemList": item_list(&p, collection::find(&o.statue_id)) }),
    )
}

/// `TreeSquareRequest {statueuId, squareId: [..], multiOpen}`: open each listed square whose
/// previous square is open, in order, paying as it goes; the path stops at the first square the
/// player cannot pay for (the client trims the same way), and nothing affordable is an error.
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
    let (p, res) = with_profile(ctx, env, |p| {
        let Some(sid) = p
            .collection
            .statues
            .iter()
            .find(|o| o.uid == id)
            .map(|o| o.statue_id.clone())
        else {
            return (Err(None), Change::None);
        };
        let Some(d) = collection::find(&sid) else {
            return (Err(None), Change::None);
        };
        let squares = collection::tree_squares(d);
        let mut opened = 0;
        let mut short = None;
        for w in &want {
            let Some(sq) = squares.iter().find(|s| &s.id == w) else {
                continue;
            };
            let o = p
                .collection
                .statues
                .iter()
                .find(|o| o.uid == id)
                .expect("statue");
            let before_open = sq.before.is_empty()
                || o.tree.contains(&sq.before)
                || squares.iter().any(|s| s.id == sq.before && s.effect == 0);
            if sq.effect == 0 || !before_open || o.tree.contains(w) {
                continue;
            }
            let (mats, lock) = cost(d, sq);
            let all: Vec<(i64, i64)> = mats.into_iter().chain(lock).collect();
            if let Err(code) = can_pay(p, &all) {
                short = Some(code);
                break;
            }
            pay(p, &all);
            let o = p
                .collection
                .statues
                .iter_mut()
                .find(|o| o.uid == id)
                .expect("statue");
            o.tree.push(w.clone());
            opened += 1;
        }
        match (opened, short) {
            (0, Some(code)) => (Err(Some(code)), Change::None),
            (0, None) => (Ok(()), Change::None),
            _ => (Ok(()), Change::Progress),
        }
    })?;
    match res {
        Err(None) => return Err(ApiError::Internal(format!("no statue {id}"))),
        Err(Some(code)) => return Err(ApiError::Status(code)),
        Ok(()) => {}
    }
    let o = p.collection.known_statue(id).unwrap();
    ok(
        json!({ "treeInfo": tree_info(o), "itemList": item_list(&p, collection::find(&o.statue_id)),
               "userInfo": crate::services::user::user_info(&p),
               "statueInfo": collection::statue_info(o) }),
    )
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
        // A2 needs A1 first; asking for both in order opens both. The tree branches: three
        // squares hang off the start square.
        assert_eq!(sq.iter().filter(|s| s["beforeSquareId"] == "S").count(), 3);
        let open = |r: &serde_json::Map<String, Value>, id: &str| {
            r["treeInfo"]["squareList"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["squareId"] == id)
                .unwrap()["openFlg"]
                .clone()
        };
        let (r, _) = dispatch(
            &ctx,
            "/tree/acquisition",
            &env(&uuid, json!({"statueuId": o.uid, "squareId": ["A2"]})),
        );
        assert_eq!(open(&r.unwrap(), "A2"), 0);
        let (r, _) = dispatch(
            &ctx,
            "/tree/acquisition",
            &env(&uuid, json!({"statueuId": o.uid, "squareId": ["A1", "A2"]})),
        );
        let r = r.unwrap();
        assert_eq!(open(&r, "A2"), 1);
        let before = crate::collection::statue_info(&o);
        assert!(r["statueInfo"]["atk"].as_i64().unwrap() > before["atk"].as_i64().unwrap());
    }

    #[test]
    fn squares_cost_materials_and_gold_and_forks_need_a_key() {
        let dir = std::env::temp_dir().join(format!("jdr-treecost-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        call("/team/team_info", json!({})).unwrap();
        let o = ctx.accounts.get(&uuid).unwrap().unwrap().collection.statues[0].clone();
        let r = call("/tree/skill_list", json!({"statueuId": o.uid})).unwrap();
        let sq = r["treeInfo"]["squareList"].as_array().unwrap().clone();
        let a1 = sq.iter().find(|s| s["squareId"] == "A1").unwrap();
        let mats = a1["openMaterialList"].as_array().unwrap();
        assert!(mats.iter().any(|m| m["itemId"] == 20), "gold is item 20");
        let gold_cost = mats.iter().find(|m| m["itemId"] == 20).unwrap()["count"]
            .as_i64()
            .unwrap();
        let mat = mats.iter().find(|m| m["itemId"] != 20).unwrap().clone();
        let items = r["itemList"].as_array().unwrap();
        let have = |l: &[Value], id: &Value| {
            l.iter()
                .find(|i| &i["itemId"] == id)
                .map(|i| i["count"].as_i64().unwrap())
        };
        let gold0 = have(items, &json!(20)).unwrap();
        let mat0 = have(items, &mat["itemId"]).unwrap();
        assert!(
            sq.iter()
                .any(|s| s["unlockFlg"] == 0 && !s["lockList"].as_array().unwrap().is_empty()),
            "a locked fork"
        );

        let r = call(
            "/tree/acquisition",
            json!({"statueuId": o.uid, "squareId": ["A1"], "multiOpen": 0}),
        )
        .unwrap();
        let items = r["itemList"].as_array().unwrap();
        assert_eq!(have(items, &json!(20)), Some(gold0 - gold_cost));
        assert_eq!(
            have(items, &mat["itemId"]),
            Some(mat0 - mat["count"].as_i64().unwrap())
        );
        assert_eq!(r["userInfo"]["vm"].as_i64(), Some(gold0 - gold_cost));

        // With no gold left, the next square is refused and nothing opens.
        ctx.accounts
            .update(&uuid, |p| {
                p.gold = 0;
                ((), crate::accounts::Change::Progress)
            })
            .unwrap();
        assert!(call(
            "/tree/acquisition",
            json!({"statueuId": o.uid, "squareId": ["A2"], "multiOpen": 0})
        )
        .is_err());
    }
}
