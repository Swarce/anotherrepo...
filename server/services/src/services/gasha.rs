//! `/gasha/*`: summon. Offline every statue and mind is already owned, so a draw picks by the
//! rarity rates and shows the owned copy; draws are free (an offline sandbox has no currency).
//! The two banners use banner bundles that exist in the player's files (`gw_<id>_<suffix>`; the
//! client downloads `gashaWindow + ".bytes"`). docs/research/13-menus.md §1.

use crate::accounts::Change;
use crate::app::{ApiError, ApiResult};
use crate::collection::{self, OwnedMind, OwnedStatue};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// (gashaId, gashaType 1 statue / 2 mind, banner bundle name in the player's files)
const BANNERS: &[(i64, i64, &str)] = &[
    (300000, 1, "gw_300000_8wc9smnp"),
    (400000, 2, "gw_400000_d74vumhb"),
];

/// Draw rates by rarity in percent (Gamerch's published rates: statues 5-star 1.5, 4-star 12.5,
/// 3-star 86; minds 4-star 7, 3-star 93). Statues of 6 stars share the 5-star rate.
const STATUE_RATES: &[(i64, f64)] = &[(3, 86.0), (4, 12.5), (5, 1.5)];
const MIND_RATES: &[(i64, f64)] = &[(3, 93.0), (4, 7.0)];

fn banner(id: i64, kind: i64, window: &str) -> Value {
    json!({
        // payType 2 (free, item 10): the play buttons exist only for pay types 1, 2, 3 and 5
        // (setupPlayButton, jump table 0x223E70C; Cash 4 shows none). Price 0, so the tap
        // check (item 10 count - price) never comes up short.
        "gashaId": id, "gashaType": kind, "payType": 2, "itemId": 0, "priority": if kind == 1 { 1 } else { 2 },
        "drowNum": 1, "price": 0, "limitNum": 0, "ticketId": -1,
        "drowNumMulti": 10, "priceMulti": 0, "limitNumMulti": 0, "ticketIdMulti": -1,
        "freeDrawFlag": 0, "freeDrawLimitNum": 0, "freeDrawRemainingNum": 0,
        "discountDrawFlag": 0, "discountDrawLimitNum": 0, "discountDrawPrice": 0,
        "discountDrawRemainingNum": 0, "discountDrawNum": 0,
        "start": "2017-06-01 00:00:00", "end": "2099-12-31 23:59:59", "gashaDetailUrl": "",
        "gashaAppealInfo": [], "gashaName": "", "gashaWindow": window, "gashaIcon": 0,
        "boxFlag": 0, "boxResetRemain": 0, "boxResetLimit": 0, "boxResetEnable": 0, "boxRemain": 0,
        "stepGroup": 0, "stepFlg": 0, "stepCount": 0, "stepBonusMulti": 0, "appealTextId": ""
    })
}

/// `GashaListResponse`: one statue and one mind banner (the screen reads entry 0 of each tab's
/// list without a length check).
pub fn gasha_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let list: Vec<Value> = BANNERS.iter().map(|(i, k, w)| banner(*i, *k, w)).collect();
    ok(
        json!({ "gashaList": list, "itemList": [], "userInfo": crate::services::user::user_info(&p) }),
    )
}

/// A small xorshift seeded from the clock: draws need variety, not cryptographic randomness.
struct Rng(u64);
impl Rng {
    fn new() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Rng(t | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Pick a rarity by rate, then an item of that rarity (falling back to the nearest lower rarity).
fn pick<'a, T>(
    rng: &mut Rng,
    rates: &[(i64, f64)],
    pool: &'a [T],
    rarity: impl Fn(&T) -> i64,
) -> Option<&'a T> {
    let total: f64 = rates.iter().map(|r| r.1).sum();
    let mut x = rng.unit() * total;
    let mut want = rates.last()?.0;
    for (r, w) in rates {
        if x < *w {
            want = *r;
            break;
        }
        x -= w;
    }
    for r in (1..=want).rev() {
        let of: Vec<&T> = pool
            .iter()
            .filter(|t| rarity(t) == r || (r == 5 && rarity(t) >= 5))
            .collect();
        if !of.is_empty() {
            return Some(of[(rng.next() % of.len() as u64) as usize]);
        }
    }
    pool.first()
}

/// Gold, silver, rainbow... the draw effect colour by rarity (GashaColorPattern).
fn colour(rarity: i64) -> i64 {
    match rarity {
        ..=3 => 1,
        4 => 2,
        5 => 3,
        _ => 4,
    }
}

fn effect(rarity: i64) -> Value {
    json!({ "statueEffectInfo": { "sillPatern": colour(rarity), "bgNum": 1, "charaAddNum": 0, "promotionType": 0 },
            "mindEffectInfo": { "paperPatern": colour(rarity), "cutInNum": 1, "OnomatopoeiaNum": 1, "promotionType": 0 } })
}

/// `GashPlayRequest {gashaId, countType 1 single | 2 multi, playType}` -> `GashaPlayResponse`.
pub fn gasha_play(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let id = env
        .param
        .get("gashaId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let multi = env.param.get("countType").and_then(Value::as_i64) == Some(2);
    let kind = BANNERS
        .iter()
        .find(|b| b.0 == id)
        .map(|b| b.1)
        .ok_or_else(|| ApiError::Internal(format!("unknown gasha {id}")))?;
    let count = if multi { 10 } else { 1 };
    let p = crate::services::statue::profile(ctx, env)?;
    let mut rng = Rng::new();
    let mut results = Vec::new();
    let mut lv1 = Vec::new();
    for _ in 0..count {
        if kind == 1 {
            let owned: Vec<&OwnedStatue> = p
                .collection
                .statues
                .iter()
                .filter(|o| collection::find(&o.statue_id).is_some())
                .collect();
            let rar = |o: &&OwnedStatue| {
                collection::find(&o.statue_id)
                    .map(|d| d.rarity())
                    .unwrap_or(3)
            };
            if let Some(o) = pick(&mut rng, STATUE_RATES, &owned, rar) {
                let info = collection::statue_info(o);
                lv1.push(collection::statue_info(&OwnedStatue {
                    level: 1,
                    exp: 0,
                    ..(*o).clone()
                }));
                results.push(json!({ "dataType": 1, "dataInfo": { "statueInfo": info, "mindInfo": null, "itemInfo": null },
                    "datauId": o.uid, "level": o.level, "newFlg": 0, "effectInfo": effect(rar(o)), "itemInfo": [] }));
            }
        } else {
            let owned: Vec<&OwnedMind> = p.collection.minds.iter().collect();
            let rar = |o: &&OwnedMind| {
                collection::minds()
                    .iter()
                    .find(|m| m.mind_id == o.mind_id)
                    .map(|m| m.rarity)
                    .unwrap_or(3)
            };
            if let Some(o) = pick(&mut rng, MIND_RATES, &owned, rar) {
                results.push(json!({ "dataType": 2, "dataInfo": { "statueInfo": null, "mindInfo": collection::mind_info(o), "itemInfo": null },
                    "datauId": o.uid, "level": 1, "newFlg": 0, "effectInfo": effect(rar(o)), "itemInfo": [] }));
            }
        }
    }
    // Nothing is charged or added (everything is owned); the touch keeps the login time fresh.
    let (p, ()) = with_profile(ctx, env, |_| ((), Change::None))?;
    ok(
        json!({ "userInfo": crate::services::user::user_info(&p), "getResultList": results,
               "originalStatueInfo": [], "Lv1StatueMasterInfo": lv1 }),
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
    fn list_has_a_statue_and_a_mind_banner_and_draws_work() {
        let dir = std::env::temp_dir().join(format!("jdr-gasha-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let (r, known) = dispatch(
            &ctx,
            "/gasha/gasha_list",
            &env(&uuid, json!({"gashaType": 0})),
        );
        assert!(known);
        let r = r.unwrap();
        let kinds: Vec<i64> = r["gashaList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["gashaType"].as_i64().unwrap())
            .collect();
        assert!(kinds.contains(&1) && kinds.contains(&2));
        for g in r["gashaList"].as_array().unwrap() {
            assert!(g["gashaAppealInfo"].is_array());
            assert!(g["gashaWindow"].as_str().unwrap().starts_with("gw_"));
        }
        for (id, n) in [(300000, 10), (400000, 1)] {
            let (r, _) = dispatch(
                &ctx,
                "/gasha/gasha_play",
                &env(
                    &uuid,
                    json!({"gashaId": id, "countType": if n == 10 { 2 } else { 1 }, "playType": 1}),
                ),
            );
            let r = r.unwrap();
            assert_eq!(r["getResultList"].as_array().unwrap().len(), n);
            assert!(r["Lv1StatueMasterInfo"].is_array() && r["originalStatueInfo"].is_array());
        }
    }
}
