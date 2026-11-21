//! Mind levelling in the Build scene (docs/research/16 §7): `/mind/strengthening` (fodder Minds)
//! and `/mind/strengthening_item` (EXP items). The client computes EXP and gold itself; the server
//! repeats the same arithmetic, refuses what the client would refuse, and clamps EXP at the cap
//! (the client allows one item past it).

use crate::accounts::{Change, Profile};
use crate::app::{ApiError, ApiResult};
use crate::collection::{self, OwnedMind};
use crate::items::{self, GOLD_RATIO_DENOMINATOR};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

const E_COMPOSE_PROTECTED: i64 = 51201;
const E_COMPOSE_DECK_MIND: i64 = 51202;
const E_COMPOSE_EMPTY_MIND: i64 = 51203;
const E_COMPOSE_LEVEL_LIMIT: i64 = 51205;
const E_VM_LACK: i64 = 10102;
const E_ITEM_LACK: i64 = 10108;
/// The client's cap on fodder Minds and item kinds per build.
const MAX_MATERIALS: usize = 20;
/// Gold per fodder Mind of rarity 1..4 (and above), times the target's level (system config).
const MIND_GROWTH_GOLD: [i64; 4] = [100, 200, 300, 400];

fn int(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(Value::as_i64)
}

/// Whether an EXP item's bonus applies to a Mind (client `CalcMaterialExp(ExpItemInfo)` for a
/// Mind target): condition 3 always; 7/8/9 when the Mind's rarity is 3/4/5; nothing else.
fn item_bonus_applies(cond: i64, rarity: i64) -> bool {
    cond == 3 || ((7..=9).contains(&cond) && rarity == cond - 4)
}

fn ratio_gold(base: i64, addition: i64, ratio: i64) -> i64 {
    ((base + addition) * ratio / GOLD_RATIO_DENOMINATOR).max(1)
}

struct Build {
    exp: i64,
    gold: i64,
}

/// What a set of fodder Minds gives (and costs) for a target.
fn fodder_build(
    p: &Profile,
    target: &OwnedMind,
    fodder: &[i64],
    add: i64,
    ratio: i64,
) -> Result<Build, i64> {
    let used = p.collection.equipped_minds();
    let mut b = Build { exp: 0, gold: 0 };
    for uid in fodder {
        let m = p
            .collection
            .minds
            .iter()
            .find(|m| m.uid == *uid && m.uid != target.uid)
            .ok_or(E_COMPOSE_EMPTY_MIND)?;
        if m.protected {
            return Err(E_COMPOSE_PROTECTED);
        }
        if used.contains(uid) {
            return Err(E_COMPOSE_DECK_MIND);
        }
        let r = collection::mind_rarity(&m.mind_id);
        // mindType 1 Minds get no bonus as fodder (only EXP Minds, type 3, would).
        b.exp += collection::mind_material_exp(r, m.level);
        let base = MIND_GROWTH_GOLD[(r.clamp(1, 4) - 1) as usize] * target.level.max(1);
        b.gold += ratio_gold(base, add, ratio);
    }
    Ok(b)
}

/// What a set of EXP items gives (and costs) for a target. Boost items (turtles) cannot be used.
fn item_build(
    p: &Profile,
    rarity: i64,
    use_items: &[(i64, i64)],
    add: i64,
    ratio: i64,
) -> Result<Build, i64> {
    let mut b = Build { exp: 0, gold: 0 };
    for (id, n) in use_items {
        let d = items::find(*id)
            .filter(|d| d.category == "exp" && d.stack == 0)
            .ok_or(E_ITEM_LACK)?;
        if *n < 1 || p.collection.item_count(*id) < *n {
            return Err(E_ITEM_LACK);
        }
        let base = d.exp * n;
        b.exp += if item_bonus_applies(d.bonus_condition, rarity) {
            (base as f32 * d.bonus_rate as f32 / 100.0) as i64
        } else {
            base
        };
        b.gold += ratio_gold(d.use_gold, add, ratio) * n;
    }
    Ok(b)
}

fn mind_list_of(p: &Profile) -> Vec<Value> {
    let used = p.collection.equipped_minds();
    p.collection
        .minds
        .iter()
        .map(|m| {
            let mut v = collection::mind_info(m);
            v["useFlg"] = json!(used.contains(&m.uid) as i64);
            v
        })
        .collect()
}

/// Both build routes: `mindList` (fodder) or `useItemList` (items) for `minduIdBase`.
pub fn strengthening(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let base = int(&env.param, "minduIdBase").unwrap_or(0);
    let ratio = int(&env.param, "goldRatioInteger").unwrap_or(GOLD_RATIO_DENOMINATOR);
    let add = int(&env.param, "goldAddition").unwrap_or(0);
    let fodder: Vec<i64> = env
        .param
        .get("mindList")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|m| int(m, "minduId")).collect())
        .unwrap_or_default();
    let use_items: Vec<(i64, i64)> = env
        .param
        .get("useItemList")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|i| Some((int(i, "itemId")?, int(i, "count")?)))
                .collect()
        })
        .unwrap_or_default();
    let item_mode = fodder.is_empty();
    let (p, res) = with_profile(ctx, env, |p| {
        let r = (|| {
            let target = p
                .collection
                .minds
                .iter()
                .find(|m| m.uid == base)
                .cloned()
                .ok_or(E_COMPOSE_EMPTY_MIND)?;
            let rarity = collection::mind_rarity(&target.mind_id);
            let curve = collection::mind_curve(rarity);
            let cap = curve
                .get(
                    (collection::MIND_MAX_LEVEL as usize)
                        .min(curve.len())
                        .saturating_sub(1),
                )
                .copied()
                .unwrap_or(0);
            if target.level >= collection::MIND_MAX_LEVEL || target.exp >= cap {
                return Err(E_COMPOSE_LEVEL_LIMIT);
            }
            if fodder.len() > MAX_MATERIALS
                || use_items.len() > MAX_MATERIALS
                || (item_mode && use_items.is_empty())
            {
                return Err(E_COMPOSE_EMPTY_MIND);
            }
            let b = if item_mode {
                item_build(p, rarity, &use_items, add, ratio)?
            } else {
                fodder_build(p, &target, &fodder, add, ratio)?
            };
            if p.gold < b.gold {
                return Err(E_VM_LACK);
            }
            p.gold -= b.gold;
            if item_mode {
                for (id, n) in &use_items {
                    p.collection.add_item(*id, -n);
                }
            } else {
                p.collection.minds.retain(|m| !fodder.contains(&m.uid));
            }
            let m = p
                .collection
                .minds
                .iter_mut()
                .find(|m| m.uid == base)
                .expect("target");
            m.exp = (m.exp + b.exp).min(cap);
            m.level = collection::mind_level_for(rarity, m.exp);
            p.progress
                .missions
                .bump("enhance", 1, jdr_core::clock::now());
            Ok(m.clone())
        })();
        let change = if r.is_ok() {
            Change::Progress
        } else {
            Change::None
        };
        (r, change)
    })?;
    let m = res.map_err(ApiError::Status)?;
    let item_list: Vec<Value> = items::items()
        .iter()
        .filter(|d| d.category == "exp")
        .map(|d| (d, p.collection.item_count(d.item_id)))
        .filter(|(_, n)| *n > 0)
        .map(|(d, n)| items::exp_item_info(d, n))
        .collect();
    let mut info = collection::mind_info(&m);
    info["useFlg"] = json!(p.collection.equipped_minds().contains(&m.uid) as i64);
    // composeType only picks the result animation (1 Success).
    ok(
        json!({ "mindInfo": info, "userInfo": crate::services::user::user_info(&p),
               "mindList": if item_mode { json!([]) } else { json!(mind_list_of(&p)) },
               "composeType": 1, "itemList": item_list }),
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
            date: 20261118103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    #[test]
    fn minds_level_with_items_and_fodder() {
        let dir = std::env::temp_dir().join(format!("jdr-mindlv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let list = call("/mind/mind_list", json!({})).unwrap();
        let minds = list["mindList"].as_array().unwrap().clone();
        let target = minds.iter().find(|m| m["rare"] == 3).unwrap().clone();
        let uid = target["minduId"].as_i64().unwrap();
        assert_eq!(target["levelUpTable"].as_array().unwrap().len(), 10);
        assert_eq!(target["dtAbilityInfo"]["abilityLevel"], 1);
        let gold0 = call("/user/user_info", json!({})).unwrap()["userInfo"]["vm"]
            .as_i64()
            .unwrap();

        // One bronze Mind medal (6116: 1000 EXP, x1.2 on Minds) = 1200 EXP: a 3-star reaches Lv3.
        let r = call(
            "/mind/strengthening_item",
            json!({"minduIdBase": uid, "useItemList": [{"itemId": 6116, "count": 1}],
                     "goldRatioInteger": 100000, "goldAddition": 0}),
        )
        .unwrap();
        assert_eq!(
            (
                r["mindInfo"]["exp"].as_i64(),
                r["mindInfo"]["level"].as_i64()
            ),
            (Some(1200), Some(3))
        );
        assert_eq!(
            r["mindInfo"]["dtAbilityInfo"]["abilityLevel"], 3,
            "battle reads abilityLevel"
        );
        assert_eq!(r["mindInfo"]["maxLevel"], 10);
        assert_eq!(
            r["userInfo"]["vm"].as_i64(),
            Some(gold0 - 100),
            "useGold 100"
        );
        assert!(r["itemList"].is_array());

        // Fodder: one 3-star Lv1 Mind = 300 EXP, 300 gold x target level 3.
        let fodder = minds
            .iter()
            .find(|m| m["rare"] == 3 && m["minduId"] != uid)
            .unwrap()["minduId"]
            .as_i64()
            .unwrap();
        let r = call(
            "/mind/strengthening",
            json!({"minduIdBase": uid, "mindList": [{"minduId": fodder}],
                     "goldRatioInteger": 100000, "goldAddition": 0}),
        )
        .unwrap();
        assert_eq!(r["mindInfo"]["exp"], 1500);
        assert!(
            !r["mindList"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["minduId"] == fodder),
            "fodder used up"
        );
        assert_eq!(r["userInfo"]["vm"].as_i64(), Some(gold0 - 100 - 900));

        // A big pile clamps at the cap; then the Mind refuses more.
        let r = call(
            "/mind/strengthening_item",
            json!({"minduIdBase": uid, "useItemList": [{"itemId": 6119, "count": 1}],
                     "goldRatioInteger": 100000, "goldAddition": 0}),
        )
        .unwrap();
        assert_eq!(
            (
                r["mindInfo"]["level"].as_i64(),
                r["mindInfo"]["exp"].as_i64()
            ),
            (Some(10), Some(13500))
        );
        assert!(call(
            "/mind/strengthening_item",
            json!({"minduIdBase": uid, "useItemList": [{"itemId": 6116, "count": 1}],
                     "goldRatioInteger": 100000, "goldAddition": 0})
        )
        .is_err());
        // Turtles (boost items) cannot be used on Minds; an unknown Mind is refused.
        assert!(call(
            "/mind/strengthening_item",
            json!({"minduIdBase": fodder, "useItemList": [{"itemId": 6135, "count": 1}],
                     "goldRatioInteger": 100000, "goldAddition": 0})
        )
        .is_err());
    }
}
