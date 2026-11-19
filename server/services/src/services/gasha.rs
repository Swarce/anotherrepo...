//! `/gasha/*`: summon. Offline every statue and mind is already owned, so a draw picks by the
//! rarity rates and shows the owned copy; draws are free (an offline sandbox has no currency).
//! The banners use banner bundles that exist in the player's files (`gw_<id>_<suffix>`; the
//! client downloads `gashaWindow + ".bytes"`). docs/research/13-menus.md §1; the box and Count Up
//! summons (Build 22): docs/research/21-box-and-count-up.md.

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

/// The box summon: "Statue Box Summon" (its name label and banner exist).
pub const BOX_ID: i64 = 600370;
const BOX_WINDOW: &str = "gw_600370_yr9w2hra";
/// A full box by rarity (ours): 1 five-star, 9 four-star, 40 three-star.
pub const BOX_CONTENTS: [(i64, i64); 3] = [(5, 1), (4, 9), (3, 40)];
/// The Count Up rounds 1-5 (ids, banner bundles and "Round N" labels exist) and their shared group.
const COUNT_UP: [(i64, &str); 5] = [
    (350010, "gw_350010_38cvemoj"),
    (350012, "gw_350012_hy60gcus"),
    (350014, "gw_350014_gva4qpe6"),
    (350016, "gw_350016_o28oiq0f"),
    (350018, "gw_350018_ywar1mxg"),
];
const COUNT_UP_GROUP: i64 = 35001;
/// Pulls that finish each round (research pack `v2.countup`: 20/30/50/50/50).
pub const COUNT_UP_TIERS: [i64; 5] = [20, 30, 50, 50, 50];
/// The ticket a finished round sends to the Message Box (ours): one draw, four stars or better.
pub const COUNT_UP_TICKET: i64 = 282;
/// The Friend Point Mind Summon (its name label and banner exist): pay item 40 (Friend Points).
pub const FRIEND_ID: i64 = 290000;
const FRIEND_WINDOW: &str = "gw_290000_9uwsb5se";
const FRIEND_POINT_ITEM: i64 = 40;
/// Friend Points per pull and per 10x (ours).
pub const FRIEND_PRICE: i64 = 200;
const E_FRIEND_POINT_LACK: i64 = 51304;
/// Friend Point draws: mostly low-rarity Minds (ours).
const FRIEND_RATES: &[(i64, f64)] = &[(1, 30.0), (2, 30.0), (3, 37.0), (4, 3.0)];
/// GashaPlayType Ticket.
const PLAY_TICKET: i64 = 4;
/// GashaPlayType Free: the banner's free button (`freeDrawFlag`; the client sends countType 1).
const PLAY_FREE: i64 = 3;
/// E_GASHA_REMAIN_FREE_EMPTY (client): no free draw left today.
const E_FREE_EMPTY: i64 = 51302;
/// Free Friend Point draws a day (research pack `economy.daily_reset`: a daily free friend draw,
/// turning at 04:00 JST; one is ours).
pub const FRIEND_FREE_PER_DAY: i64 = 1;
const E_NOT_ENOUGH: i64 = 50001;

/// Box and Count Up progress (saved with the story progress).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GashaState {
    /// Box draws taken so far, per entry of BOX_CONTENTS.
    #[serde(default)]
    pub box_drawn: [i64; 3],
    /// Count Up round 0..4 and pulls made in it.
    #[serde(default)]
    pub round: usize,
    #[serde(default)]
    pub round_pulls: i64,
    /// The game day (04:00 JST, `missions::game_day`) of the last free Friend Point draw; 0 = never.
    #[serde(default)]
    pub friend_free_day: i64,
}

impl GashaState {
    pub fn box_remain(&self) -> i64 {
        BOX_CONTENTS
            .iter()
            .zip(self.box_drawn)
            .map(|((_, n), d)| (n - d).max(0))
            .sum()
    }

    /// Take one rarity out of the box, weighted by what is left.
    fn box_take(&mut self, rng: &mut Rng) -> Option<i64> {
        let left: Vec<i64> = BOX_CONTENTS
            .iter()
            .zip(self.box_drawn)
            .map(|((_, n), d)| (n - d).max(0))
            .collect();
        let total: i64 = left.iter().sum();
        if total == 0 {
            return None;
        }
        let mut x = (rng.next() % total as u64) as i64;
        for (i, l) in left.iter().enumerate() {
            if x < *l {
                self.box_drawn[i] += 1;
                return Some(BOX_CONTENTS[i].0);
            }
            x -= l;
        }
        None
    }

    /// Count `n` Count Up pulls; true when the round finished (the next round starts; after
    /// round 5 the series starts again).
    /// Free Friend Point draws left today (the count turns at 04:00 JST).
    pub fn friend_free_left(&self, now: i64) -> i64 {
        if self.friend_free_day == crate::missions::game_day(now) {
            0
        } else {
            FRIEND_FREE_PER_DAY
        }
    }

    fn count_up(&mut self, n: i64) -> bool {
        self.round_pulls += n;
        if self.round_pulls >= COUNT_UP_TIERS[self.round.min(4)] {
            self.round = (self.round + 1) % COUNT_UP.len();
            self.round_pulls = 0;
            true
        } else {
            false
        }
    }
}

/// Draw rates by rarity in percent (Gamerch's published rates: statues 5-star 1.5, 4-star 12.5,
/// 3-star 86; minds 4-star 7, 3-star 93). Statues of 6 stars share the 5-star rate.
const STATUE_RATES: &[(i64, f64)] = &[(3, 86.0), (4, 12.5), (5, 1.5)];
/// The guaranteed slot of a 10x statue summon (launch guide: 4-star 98.5%, 5-star 1.5%;
/// research pack `economy.guaranteed_slot`, launch-era banner evidence).
const STATUE_GUARANTEED: &[(i64, f64)] = &[(4, 98.5), (5, 1.5)];
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

/// The box banner: boxFlag 1 shows the box panel, boxResetEnable 1 the Reset button, and
/// boxRemain gates the play buttons (1 for a single, 10 for the 10x).
fn box_banner(g: &GashaState) -> Value {
    let mut b = banner(BOX_ID, 1, BOX_WINDOW);
    for (k, v) in [
        ("priority", json!(3)),
        ("gashaName", json!(format!("gs_{BOX_ID}_na"))),
        ("boxFlag", json!(1)),
        ("boxResetEnable", json!(1)),
        ("boxRemain", json!(g.box_remain())),
    ] {
        b[k] = v;
    }
    b
}

/// The current Count Up round: its own gashaId under one stepGroup, so the screen stays on the
/// series when the round changes. stepCount is the round number shown; a ticket button appears
/// when the player holds a Count Up ticket. Round 5's appeal says its 10x has a 5-star.
fn count_up_banner(g: &GashaState) -> Value {
    let (id, window) = COUNT_UP[g.round.min(4)];
    let mut b = banner(id, 1, window);
    for (k, v) in [
        ("priority", json!(4)),
        ("gashaName", json!(format!("gs_{id}_na"))),
        ("stepGroup", json!(COUNT_UP_GROUP)),
        ("stepFlg", json!(1)),
        ("stepCount", json!(g.round + 1)),
        ("ticketId", json!(COUNT_UP_TICKET)),
        (
            "appealTextId",
            json!(if g.round == 4 { "GASHA_STEP_01" } else { "" }),
        ),
    ] {
        b[k] = v;
    }
    b
}

/// The Friend Point Mind Summon: payType 3 with pay item 40 (Friend Points), and the daily free
/// draw: `freeDrawFlag` 1 adds the free button (play button type 10, setupPlayButton 0xDE21B8), lit
/// while `freeDrawRemainingNum` > 0 (0xDE2B34); it sends countType 1, playType 3 (H).
fn friend_banner(g: &GashaState, now: i64) -> Value {
    let mut b = banner(FRIEND_ID, 2, FRIEND_WINDOW);
    for (k, v) in [
        ("freeDrawFlag", json!(1)),
        ("freeDrawLimitNum", json!(FRIEND_FREE_PER_DAY)),
        ("freeDrawRemainingNum", json!(g.friend_free_left(now))),
        ("priority", json!(5)),
        ("payType", json!(3)),
        ("itemId", json!(FRIEND_POINT_ITEM)),
        ("price", json!(FRIEND_PRICE)),
        ("priceMulti", json!(FRIEND_PRICE * 10)),
        ("gashaName", json!(format!("gs_{FRIEND_ID}_na"))),
    ] {
        b[k] = v;
    }
    b
}

/// `GashaListResponse`: the normal statue and mind banners first (the screen reads entry 0 of
/// each tab's list without a length check), then the box and the Count Up round. `itemList`
/// (never null) holds the ticket count for the ticket button.
pub fn gasha_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let g = &p.progress.gasha;
    let mut list: Vec<Value> = BANNERS.iter().map(|(i, k, w)| banner(*i, *k, w)).collect();
    list.push(box_banner(g));
    list.push(count_up_banner(g));
    list.push(friend_banner(g, jdr_core::clock::now()));
    let tickets = p.collection.item_count(COUNT_UP_TICKET);
    // Item 40 with type 5 (summon points) gives the Friend Point banner its point buttons. Both
    // are complete ItemLongInfos (every string set).
    let mut points = crate::items::reward_item(FRIEND_POINT_ITEM, p.friend_points);
    points["type"] = json!(5);
    ok(json!({ "gashaList": list,
               "itemList": [crate::items::reward_item(COUNT_UP_TICKET, tickets), points],
               "userInfo": crate::services::user::user_info(&p) }))
}

/// `/gasha/get_box_gasha {gashaId}` -> `{html}`: the "Box Status" page (the client shows it in a
/// web view; never null).
pub fn get_box_gasha(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let g = &p.progress.gasha;
    let rows: String = BOX_CONTENTS
        .iter()
        .zip(g.box_drawn)
        .map(|((r, n), d)| {
            format!(
                "<tr><td>{r}&#9733;</td><td>{} / {n}</td></tr>",
                (n - d).max(0)
            )
        })
        .collect();
    let html = format!(
        "<html><head><meta name=\"viewport\" content=\"width=device-width\"></head>\
         <body style=\"background:#111;color:#eee;font-family:sans-serif\"><h3>Box contents</h3>\
         <table>{rows}</table><p>{} of {} left.</p></body></html>",
        g.box_remain(),
        BOX_CONTENTS.iter().map(|c| c.1).sum::<i64>()
    );
    ok(json!({ "html": html }))
}

/// `/gasha/reset_box_gasha {gashaId}` -> `{remain}`: a full box again. The client then asks for
/// gasha_list, which shows the refilled box.
pub fn reset_box_gasha(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (p, ()) = with_profile(ctx, env, |p| {
        p.progress.gasha.box_drawn = [0; 3];
        ((), Change::Progress)
    })?;
    ok(json!({ "remain": p.progress.gasha.box_remain() }))
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

fn statue_result(o: &OwnedStatue, rarity: i64, lv1: &mut Vec<Value>) -> Value {
    lv1.push(collection::statue_info(&OwnedStatue {
        level: 1,
        exp: 0,
        ..o.clone()
    }));
    json!({ "dataType": 1, "dataInfo": { "statueInfo": collection::statue_info(o), "mindInfo": null, "itemInfo": null },
            "datauId": o.uid, "level": o.level, "newFlg": 0, "effectInfo": effect(rarity), "itemInfo": [] })
}

fn statue_rarity(o: &&OwnedStatue) -> i64 {
    collection::find(&o.statue_id)
        .map(|d| d.rarity())
        .unwrap_or(3)
}

/// `GashPlayRequest {gashaId, countType 1 single | 2 multi, playType}` -> `GashaPlayResponse`.
/// Normal banners pick by rate; the box takes from what is left in it; a Count Up round counts
/// pulls and, when it finishes, sends a ticket to the Message Box; playType 4 spends a ticket.
pub fn gasha_play(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let id = env
        .param
        .get("gashaId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let multi = env.param.get("countType").and_then(Value::as_i64) == Some(2);
    let play_type = env.param.get("playType").and_then(Value::as_i64);
    let ticket = play_type == Some(PLAY_TICKET);
    let free = play_type == Some(PLAY_FREE);
    let is_box = id == BOX_ID;
    let count_up = COUNT_UP.iter().position(|c| c.0 == id);
    let kind = BANNERS
        .iter()
        .find(|b| b.0 == id)
        .map(|b| b.1)
        .or((is_box || count_up.is_some()).then_some(1))
        .or((id == FRIEND_ID).then_some(2))
        .ok_or_else(|| ApiError::Internal(format!("unknown gasha {id}")))?;
    let count = if multi && !ticket && !free { 10 } else { 1 };
    let now = jdr_core::clock::now();
    let (p, results) = with_profile(ctx, env, |p| {
        let mut rng = Rng::new();
        let mut results = Vec::new();
        let mut lv1 = Vec::new();
        if ticket {
            if p.collection.item_count(COUNT_UP_TICKET) < 1 {
                return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
            }
            p.collection.add_item(COUNT_UP_TICKET, -1);
        }
        if is_box && p.progress.gasha.box_remain() < count {
            return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
        }
        let friend = id == FRIEND_ID;
        if free {
            // Only the Friend Point summon has a free draw (one a day).
            if !friend || p.progress.gasha.friend_free_left(now) < 1 {
                return (Err(ApiError::Status(E_FREE_EMPTY)), Change::None);
            }
            p.progress.gasha.friend_free_day = crate::missions::game_day(now);
        } else if friend {
            let cost = FRIEND_PRICE * count;
            if p.friend_points < cost {
                return (Err(ApiError::Status(E_FRIEND_POINT_LACK)), Change::None);
            }
            p.friend_points -= cost;
        }
        let statues: Vec<OwnedStatue> = p
            .collection
            .statues
            .iter()
            .filter(|o| collection::find(&o.statue_id).is_some())
            .cloned()
            .collect();
        let owned: Vec<&OwnedStatue> = statues.iter().collect();
        for i in 0..count {
            let last = multi && i == count - 1;
            if kind == 1 {
                let picked = if ticket {
                    pick(&mut rng, STATUE_GUARANTEED, &owned, statue_rarity)
                } else if is_box {
                    let r = p.progress.gasha.box_take(&mut rng).unwrap_or(3);
                    pick(&mut rng, &[(r, 1.0)], &owned, statue_rarity)
                } else if last && count_up == Some(4) {
                    pick(&mut rng, &[(5, 1.0)], &owned, statue_rarity)
                } else {
                    // The tenth draw of a 10x summon is the guaranteed slot.
                    let rates = if last {
                        STATUE_GUARANTEED
                    } else {
                        STATUE_RATES
                    };
                    pick(&mut rng, rates, &owned, statue_rarity)
                };
                if let Some(o) = picked {
                    results.push(statue_result(o, statue_rarity(o), &mut lv1));
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
                let rates = if friend { FRIEND_RATES } else { MIND_RATES };
                if let Some(o) = pick(&mut rng, rates, &owned, rar) {
                    results.push(json!({ "dataType": 2, "dataInfo": { "statueInfo": null, "mindInfo": collection::mind_info(o), "itemInfo": null },
                        "datauId": o.uid, "level": 1, "newFlg": 0, "effectInfo": effect(rar(o)), "itemInfo": [] }));
                }
            }
        }
        if let (Some(round), false) = (count_up, ticket) {
            // Pulls count for the round being shown; a finished round pays its ticket.
            if round == p.progress.gasha.round && p.progress.gasha.count_up(count) {
                p.progress.presents.add(
                    crate::presents::DATA_ITEM,
                    COUNT_UP_TICKET,
                    1,
                    format!("Count Up reward: {{gs_{}_na}}", COUNT_UP[round].0),
                    now,
                );
            }
        }
        p.progress.missions.bump("summon", 1, now);
        // Nothing else is charged or added (everything is owned).
        let change = if is_box || ticket || friend || count_up.is_some() {
            Change::Progress
        } else {
            Change::Bookkeeping
        };
        (Ok((results, lv1)), change)
    })?;
    let (results, lv1) = results?;
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

    #[test]
    fn the_guaranteed_slot_never_gives_a_3_star() {
        let pool: Vec<i64> = (0..60).map(|i| 3 + i % 3).collect(); // rarities 3, 4, 5
        let mut rng = super::Rng::new();
        let mut fives = 0;
        for _ in 0..4000 {
            let r = *super::pick(&mut rng, super::STATUE_GUARANTEED, &pool, |r| *r).unwrap();
            assert!(r >= 4);
            fives += (r == 5) as i32;
        }
        assert!(fives < 200, "about 1.5% five-stars, got {fives}/4000");
    }

    #[test]
    fn box_draws_down_and_resets_and_count_up_pays_a_ticket() {
        let dir = std::env::temp_dir().join(format!("jdr-box-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let find = |id: i64| {
            let l = call("/gasha/gasha_list", json!({"gashaType": 0})).unwrap();
            assert!(l["itemList"].is_array(), "never null");
            l["gashaList"]
                .as_array()
                .unwrap()
                .iter()
                .find(|g| g["gashaId"] == id || (id == 0 && g["stepFlg"] == 1))
                .cloned()
                .unwrap()
        };
        let full: i64 = super::BOX_CONTENTS.iter().map(|c| c.1).sum();
        let b = find(super::BOX_ID);
        assert_eq!(
            (b["boxFlag"].as_i64(), b["boxRemain"].as_i64()),
            (Some(1), Some(full))
        );
        assert_eq!(b["boxResetEnable"], 1);
        let mut fives = 0;
        for _ in 0..full / 10 {
            let r = call(
                "/gasha/gasha_play",
                json!({"gashaId": super::BOX_ID, "countType": 2, "playType": 1}),
            )
            .unwrap();
            for x in r["getResultList"].as_array().unwrap() {
                let id = x["dataInfo"]["statueInfo"]["statueId"]
                    .as_str()
                    .unwrap()
                    .to_string();
                fives += (crate::collection::find(&id).unwrap().rarity() >= 5) as i64;
            }
        }
        assert_eq!(fives, 1, "the box holds exactly one five-star");
        assert_eq!(find(super::BOX_ID)["boxRemain"], 0);
        assert!(call(
            "/gasha/gasha_play",
            json!({"gashaId": super::BOX_ID, "countType": 1, "playType": 1})
        )
        .is_err());
        let html = call("/gasha/get_box_gasha", json!({"gashaId": super::BOX_ID})).unwrap();
        assert!(html["html"].as_str().unwrap().contains("<table>"));
        assert_eq!(
            call("/gasha/reset_box_gasha", json!({"gashaId": super::BOX_ID})).unwrap()["remain"],
            full
        );
        assert_eq!(find(super::BOX_ID)["boxRemain"], full);

        // Count Up: round 1 ends after 20 pulls, the ticket goes to the Message Box, round 2 shows.
        let c = find(0);
        assert_eq!(
            (c["gashaId"].as_i64(), c["stepCount"].as_i64()),
            (Some(350010), Some(1))
        );
        assert_eq!(c["appealTextId"], "", "never null");
        for _ in 0..2 {
            call(
                "/gasha/gasha_play",
                json!({"gashaId": 350010, "countType": 2, "playType": 1}),
            )
            .unwrap();
        }
        let c = find(0);
        assert_eq!(
            (c["gashaId"].as_i64(), c["stepCount"].as_i64()),
            (Some(350012), Some(2))
        );
        assert_eq!(c["stepGroup"], super::COUNT_UP_GROUP);
        let box_ = call(
            "/present/present_list",
            json!({"prevNum": 1, "receivedFlag": 0}),
        )
        .unwrap();
        let row = box_["presentList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["dataId"].as_str() == Some(&super::COUNT_UP_TICKET.to_string()))
            .cloned()
            .expect("the ticket waits in the box");
        assert!(
            call(
                "/gasha/gasha_play",
                json!({"gashaId": 350012, "countType": 1, "playType": 4})
            )
            .is_err(),
            "no ticket yet"
        );
        call(
            "/present/get",
            json!({"presentuId": row["presentuId"], "prevNum": 1}),
        )
        .unwrap();
        let l = call("/gasha/gasha_list", json!({"gashaType": 0})).unwrap();
        assert_eq!(l["itemList"][0]["count"], 1);
        let r = call(
            "/gasha/gasha_play",
            json!({"gashaId": 350012, "countType": 1, "playType": 4}),
        )
        .unwrap();
        assert_eq!(r["getResultList"].as_array().unwrap().len(), 1);
        let id = r["getResultList"][0]["dataInfo"]["statueInfo"]["statueId"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            crate::collection::find(&id).unwrap().rarity() >= 4,
            "a ticket draw is 4 stars or better"
        );
        assert_eq!(
            find(0)["gashaId"],
            350012,
            "ticket draws do not count for the round"
        );
    }

    #[test]
    fn friend_points_pay_for_the_friend_summon() {
        let dir = std::env::temp_dir().join(format!("jdr-fp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let l = call("/gasha/gasha_list", json!({"gashaType": 0})).unwrap();
        let b = l["gashaList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["gashaId"] == super::FRIEND_ID)
            .cloned()
            .unwrap();
        assert_eq!(
            (
                b["payType"].as_i64(),
                b["itemId"].as_i64(),
                b["gashaType"].as_i64()
            ),
            (Some(3), Some(40), Some(2))
        );
        let fp = l["itemList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["itemId"] == 40)
            .cloned()
            .unwrap();
        assert_eq!(
            (fp["type"].as_i64(), fp["count"].as_i64()),
            (Some(5), Some(0))
        );
        let pull = json!({"gashaId": super::FRIEND_ID, "countType": 1, "playType": 1});
        assert!(
            call("/gasha/gasha_play", pull.clone()).is_err(),
            "no Friend Points yet"
        );
        ctx.accounts
            .update(&uuid, |p| {
                p.friend_points = 2 * super::FRIEND_PRICE;
                ((), crate::accounts::Change::Progress)
            })
            .unwrap();
        let r = call("/gasha/gasha_play", pull).unwrap();
        assert_eq!(r["getResultList"][0]["dataType"], 2, "a Mind");
        assert_eq!(r["userInfo"]["mindGashaPoint"], super::FRIEND_PRICE);
        let l = call("/gasha/gasha_list", json!({"gashaType": 0})).unwrap();
        let fp = l["itemList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["itemId"] == 40)
            .cloned()
            .unwrap();
        assert_eq!(fp["count"], super::FRIEND_PRICE);
    }

    /// The daily free Friend Point draw (research pack `economy.daily_reset`): the free button is
    /// lit once a day, a free pull costs nothing, a second one is refused (51302), and the count
    /// comes back at the 04:00 JST day turn.
    #[test]
    fn the_friend_summon_has_one_free_draw_a_day() {
        let dir = std::env::temp_dir().join(format!("jdr-fpfree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let banner = || {
            call("/gasha/gasha_list", json!({"gashaType": 0})).unwrap()["gashaList"]
                .as_array()
                .unwrap()
                .iter()
                .find(|g| g["gashaId"] == super::FRIEND_ID)
                .cloned()
                .unwrap()
        };
        let b = banner();
        assert_eq!(
            (
                b["freeDrawFlag"].as_i64(),
                b["freeDrawLimitNum"].as_i64(),
                b["freeDrawRemainingNum"].as_i64()
            ),
            (Some(1), Some(1), Some(1))
        );
        // Both replies walked like the client's reader (services/shapes.rs).
        crate::services::shapes::assert_complete(
            "Jjs.GashaListResponse",
            &Value::Object(call("/gasha/gasha_list", json!({"gashaType": 0})).unwrap()),
        );
        // No Friend Points needed; the 10x countType is ignored for a free draw.
        let free = json!({"gashaId": super::FRIEND_ID, "countType": 2, "playType": 3});
        let r = call("/gasha/gasha_play", free.clone()).unwrap();
        crate::services::shapes::assert_complete(
            "Jjs.GashaPlayResponse",
            &Value::Object(r.clone()),
        );
        assert_eq!(r["getResultList"].as_array().unwrap().len(), 1);
        assert_eq!(r["userInfo"]["mindGashaPoint"], 0);
        assert_eq!(banner()["freeDrawRemainingNum"], 0);
        assert!(matches!(
            call("/gasha/gasha_play", free),
            Err(crate::app::ApiError::Status(51302))
        ));
        // Other banners have no free draw.
        assert!(matches!(
            call(
                "/gasha/gasha_play",
                json!({"gashaId": 300000, "countType": 1, "playType": 3})
            ),
            Err(crate::app::ApiError::Status(51302))
        ));
        // The day turns at 04:00 JST (19:00 UTC).
        let g = super::GashaState {
            friend_free_day: crate::missions::game_day(1_700_000_000),
            ..Default::default()
        };
        let next_4am_jst = (1_700_000_000i64 + 9 * 3600 - 4 * 3600).div_euclid(86_400) * 86_400
            + 86_400
            - 9 * 3600
            + 4 * 3600;
        assert_eq!(g.friend_free_left(next_4am_jst - 1), 0);
        assert_eq!(g.friend_free_left(next_4am_jst), 1);
    }
}
