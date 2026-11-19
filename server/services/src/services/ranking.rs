//! Rankings (`/event/get_event_ranking_list`, `get_event_ranking_detail`,
//! `get_event_ranking_reward`; docs/research/23-arena.md §3): the player against a field of AI
//! rivals, for the Arena (by season) and for the point events. Offline there is nobody else, so
//! the field is ours: deterministic, the same rivals all season (or for the whole event).
//!
//! Client facts (H, read in 3.12.0):
//! - The Arena home's ranking button stores the campaign, the event type (PvpAuto 3 when the
//!   home's battleType is 2, else Pvp 2) and `pvpUserInfo.rankingPoint`, then loads the
//!   `RankingUI` scene. The event home asks for the list when `eventInfo.eventRankingFlg` is 1
//!   and shows its ranking button (type Normal 1).
//! - The screen takes the `/event/get_event_ranking_list` entry whose `eventCampaignId` is the
//!   campaign (every entry's id is compared, so none may be null) and, in its `rankingList`, the
//!   ranking with the lowest `order`. Without one the screen stays empty.
//! - `get_event_ranking_detail {eventCampaignId, rankingKey, rankingRange}` (range 0 = around
//!   me, 1 = top): `type` 2 (Clan) shows `clanRankList`, anything else `personalRankList`
//!   (counted, so never null). `personalRank` is "my rank" (0 = not ranked: the around filter
//!   says so). The banner looks the player up by `userId` (String.Equals on every entry's
//!   userId, so none may be null) against `/user/user_info_mini`'s id, loads the `statueId`
//!   thumbnail, and shows `username`, `userRank`, `winCount`, `loseCount`, `lastLoginAt` (a
//!   date) and `userEventPoint`; `rankingName` is the title (a text label), `endDate` the time
//!   left.
//! - The request's `eventCampaignId` is the cached `/bizarre/bizarre_list` eventInfo's id when
//!   there is one (even on the Arena's ranking), so the server goes by `rankingKey`.
//! - `get_event_ranking_reward {eventCampaignId, rankingKey}`: `rankingRewardList` [{rewardItemSet,
//!   rankMin, rankMax, unit}]; "my reward" is the first row whose rankMax reaches my rank; every
//!   ItemSetInfo list is read.

use crate::accounts::Profile;
use crate::app::ApiResult;
use crate::collection;
use crate::services::{arena, event, ok};
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Rivals in a field (ours): with the player, a thousand places.
pub const FIELD_SIZE: usize = 999;
/// The best rival's points at the end of an Arena season (ours).
pub const ARENA_TOP: i64 = 10_000;
/// The best rival's event points (ours; a clear earns 50, the last point reward is at 1000).
pub const EVENT_TOP: i64 = 5_000;
/// Entries in the "top" view (ours).
pub const TOP_ENTRIES: usize = 100;
/// Places above and below the player in the "around me" view (ours).
pub const AROUND: usize = 10;
/// The Arena campaign's ranking key; an event's is `EVENT_KEY` + its id (ours: opaque to the
/// client, which sends it back).
pub const ARENA_KEY: &str = "arena-510027";
pub const EVENT_KEY: &str = "event-";
/// RankingType.Personal, RankingEventType Normal 1 / PvpAuto 3 (client).
const TYPE_PERSONAL: i64 = 1;
const EVENT_TYPE_NORMAL: i64 = 1;
const EVENT_TYPE_PVP_AUTO: i64 = 3;
/// Handles for AI players (ours; never game text).
pub const HANDLES: [&str; 8] = [
    "Rival",
    "Challenger",
    "Duelist",
    "Contender",
    "Ace",
    "Veteran",
    "Drifter",
    "Wanderer",
];

/// splitmix64.
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }
}

/// FNV-1a, for seeds from text.
pub fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// One AI rival.
#[derive(Debug, Clone, PartialEq)]
pub struct Rival {
    pub user_id: String,
    pub name: String,
    pub statue_id: String,
    pub user_rank: i64,
    pub points: i64,
    pub wins: i64,
    pub losses: i64,
    /// Minutes since the rival's last login (for `lastLoginAt`).
    pub away_mins: i64,
}

/// A field at its final points, best first: rival i (0 = best) ends near
/// `top x (1 - i/N)^2`, give or take a tenth; stronger rivals lead rarer statues (ours).
fn generate(seed: u64, top: i64, battles: bool) -> Vec<Rival> {
    let mut rng = Rng(seed);
    let all: Vec<&collection::StatueDef> = collection::members().collect();
    let n = FIELD_SIZE as f64;
    let mut out: Vec<Rival> = (0..FIELD_SIZE)
        .map(|i| {
            let share = (n - i as f64) / n;
            let jitter = 0.9 + (rng.below(2001) as f64) / 10_000.0;
            let points = ((top as f64) * share * share * jitter).round().max(0.0) as i64;
            let rarity = (6 - (i * 4 / FIELD_SIZE) as i64).clamp(3, 6);
            let pool: Vec<&collection::StatueDef> = all
                .iter()
                .copied()
                .filter(|d| d.rarity() == rarity)
                .collect();
            let statue_id = if pool.is_empty() {
                all.get(rng.below(all.len()))
                    .map(|d| d.statue_id.clone())
                    .unwrap_or_default()
            } else {
                pool[rng.below(pool.len())].statue_id.clone()
            };
            let wins = if battles { points / 25 } else { 0 };
            Rival {
                user_id: format!("ai-{:016x}-{i}", seed),
                name: format!(
                    "{} {}",
                    HANDLES[rng.below(HANDLES.len())],
                    100 + rng.below(900)
                ),
                statue_id,
                user_rank: (220 - (i as i64) * 200 / FIELD_SIZE as i64 + rng.below(15) as i64)
                    .max(1),
                points,
                wins,
                losses: if battles {
                    wins / 3 + rng.below(5) as i64
                } else {
                    0
                },
                away_mins: rng.below(24 * 60) as i64,
            }
        })
        .collect();
    out.sort_by(|a, b| b.points.cmp(&a.points).then(a.user_id.cmp(&b.user_id)));
    out
}

fn cached(key: String, seed: u64, top: i64, battles: bool) -> Arc<Vec<Rival>> {
    static CACHE: Mutex<Option<HashMap<String, Arc<Vec<Rival>>>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if map.len() > 16 {
        map.clear();
    }
    map.entry(key)
        .or_insert_with(|| Arc::new(generate(seed, top, battles)))
        .clone()
}

/// When an Arena season starts (unix seconds): Monday 04:00 JST (arena::season_of).
pub fn arena_season_start(season: i64) -> i64 {
    (season * 7 - 3) * 86_400 - jdr_core::clock::TZ_OFFSET_SECS + 4 * 3_600
}

/// The Arena field at the end of a season.
pub fn arena_final_field(season: i64) -> Arc<Vec<Rival>> {
    cached(
        format!("arena:{season}"),
        hash(&format!("arena:{}:{season}", arena::CAMPAIGN_ID)),
        ARENA_TOP,
        true,
    )
}

/// The Arena field now: rivals earn their points over the season (5 % at its start, all of
/// them at its end; ours).
pub fn arena_field(season: i64, now: i64) -> Vec<Rival> {
    let len = 7 * 86_400;
    let elapsed = (now - arena_season_start(season)).clamp(0, len);
    let permille = 50 + 950 * elapsed / len;
    arena_final_field(season)
        .iter()
        .map(|r| Rival {
            points: r.points * permille / 1000,
            wins: r.wins * permille / 1000,
            losses: r.losses * permille / 1000,
            ..r.clone()
        })
        .collect()
}

/// A ranked event's field (events never close offline, so it does not move). A high-score
/// event's best rival totals `HIGH_SCORE_TOP_PER_STAGE` per high-score stage (ours).
pub fn event_field(event_id: &str) -> Arc<Vec<Rival>> {
    let top = match crate::quests::event_by_id(event_id) {
        Some(e) if event::is_high_score(event_id) => {
            HIGH_SCORE_TOP_PER_STAGE * (e.scoring.len() as i64).max(1)
        }
        _ => EVENT_TOP,
    };
    cached(
        format!("event:{event_id}"),
        hash(&format!("event:{event_id}")),
        top,
        false,
    )
}

/// The best rival's score per high-score stage (ours; a stage scores at most 1.3 million).
pub const HIGH_SCORE_TOP_PER_STAGE: i64 = 150_000;

/// The player's place with `points` (1 = best): behind every rival with more points, ahead of
/// those with as many.
pub fn position(field: &[Rival], points: i64) -> i64 {
    1 + field.iter().filter(|r| r.points > points).count() as i64
}

fn date(t: i64) -> String {
    let (y, mo, d, h, mi, s) = jdr_core::clock::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// What a ranking key names.
enum Board {
    Arena,
    Event(String),
}

fn board(env: &RequestEnvelope) -> Option<Board> {
    let s = |k: &str| env.param.get(k).and_then(Value::as_str).unwrap_or("");
    let key = s("rankingKey");
    if key == ARENA_KEY {
        return Some(Board::Arena);
    }
    if let Some(id) = key.strip_prefix(EVENT_KEY) {
        return event::is_ranked(id).then(|| Board::Event(id.to_string()));
    }
    // No key we gave out: fall back to the campaign id.
    match s("eventCampaignId") {
        id if id == arena::CAMPAIGN_ID => Some(Board::Arena),
        id if event::is_ranked(id) => Some(Board::Event(id.to_string())),
        _ => None,
    }
}

/// The ranked events with a tile (point and high-score events: their home shows a ranking).
fn ranked_events() -> Vec<&'static str> {
    crate::quests::events()
        .iter()
        .map(|e| e.event_id.as_str())
        .filter(|id| event::is_ranked(id))
        .collect()
}

/// `EQ_<label>_Name`: the campaign's name label (client text).
fn name_label(event_id: &str) -> String {
    format!("EQ_{}_Name", event::label_id(event_id))
}

/// `/event/get_event_ranking_list {}`: one personal ranking per campaign: the Arena and each
/// point event.
pub fn ranking_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    let mut list = vec![json!({
        "eventCampaignId": arena::CAMPAIGN_ID, "order": 1,
        "rankingList": [{ "type": TYPE_PERSONAL, "eventType": EVENT_TYPE_PVP_AUTO, "groupNo": 0,
                          "rankingKey": ARENA_KEY, "rankingName": name_label(arena::CAMPAIGN_ID),
                          "order": 1 }]
    })];
    for (i, id) in ranked_events().into_iter().enumerate() {
        list.push(json!({
            "eventCampaignId": id, "order": i as i64 + 2,
            "rankingList": [{ "type": TYPE_PERSONAL, "eventType": EVENT_TYPE_NORMAL, "groupNo": 0,
                              "rankingKey": format!("{EVENT_KEY}{id}"), "rankingName": name_label(id),
                              "order": 1 }]
        }));
    }
    ok(json!({ "eventList": list }))
}

/// The player's own ranking entry (PersonalRankingInfo).
fn me(p: &Profile, rank: i64, points: i64, wins: i64, losses: i64, now: i64) -> Value {
    let leader = collection::main_team(&p.collection, 1)
        .and_then(|t| p.collection.known_statue(t.slots[0]))
        .map(|o| o.statue_id.clone())
        .unwrap_or_default();
    json!({ "rank": rank, "statueId": leader, "userId": p.user_id, "username": p.username,
            "userRank": p.rank.max(1), "clanName": "", "winCount": wins, "loseCount": losses,
            "lastLoginAt": date(now), "userEventPoint": points })
}

fn rival_entry(r: &Rival, rank: i64, now: i64) -> Value {
    json!({ "rank": rank, "statueId": r.statue_id, "userId": r.user_id, "username": r.name,
            "userRank": r.user_rank, "clanName": "", "winCount": r.wins, "loseCount": r.losses,
            "lastLoginAt": date(now - r.away_mins * 60), "userEventPoint": r.points })
}

/// The entries of a view: the player placed into the field; `top` the first TOP_ENTRIES places,
/// else AROUND places either side of the player.
pub fn entries(field: &[Rival], mine: Value, points: i64, top: bool, now: i64) -> Vec<Value> {
    let pos = position(field, points) as usize; // 1-based
    let total = field.len() + 1;
    let (from, to) = if top {
        (1, TOP_ENTRIES.min(total))
    } else {
        (pos.saturating_sub(AROUND).max(1), (pos + AROUND).min(total))
    };
    (from..=to)
        .map(|place| {
            if place == pos {
                let mut m = mine.clone();
                m["rank"] = json!(place);
                m
            } else {
                let i = if place < pos { place - 1 } else { place - 2 };
                rival_entry(&field[i], place as i64, now)
            }
        })
        .collect()
}

/// `/event/get_event_ranking_detail {eventCampaignId, rankingKey, rankingRange}`.
pub fn ranking_detail(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let top = env.param.get("rankingRange").and_then(Value::as_i64) != Some(0);
    let now = jdr_core::clock::now();
    let (id, key, name, end, kind, field, points, wins, losses, p) = match board(env) {
        Some(Board::Arena) => {
            let p = arena::standing(ctx, env)?;
            let st = &p.progress.arena;
            let season = arena::season_of(now);
            (
                arena::CAMPAIGN_ID.to_string(),
                ARENA_KEY.to_string(),
                name_label(arena::CAMPAIGN_ID),
                date(arena_season_start(season + 1)),
                EVENT_TYPE_PVP_AUTO,
                arena_field(season, now),
                st.points,
                st.wins,
                st.losses,
                p,
            )
        }
        Some(Board::Event(id)) => {
            let p = crate::services::statue::profile(ctx, env)?;
            let points = event::points(&p, &id);
            (
                id.clone(),
                format!("{EVENT_KEY}{id}"),
                name_label(&id),
                arena::OPEN_UNTIL.to_string(),
                EVENT_TYPE_NORMAL,
                event_field(&id).to_vec(),
                points,
                0,
                0,
                p,
            )
        }
        None => {
            // Not a ranking we list: an empty board rather than an error dialog.
            let key = env
                .param
                .get("rankingKey")
                .and_then(Value::as_str)
                .unwrap_or("");
            return ok(json!({
                "eventCampaignName": "", "endDate": arena::OPEN_UNTIL, "personalRank": 0,
                "userGroupNo": 0, "userEventPoint": 0, "clanRank": 0, "clanEventPoint": 0,
                "type": TYPE_PERSONAL, "groupNo": 0, "rankingKey": key, "rankingName": "",
                "personalRankList": [], "clanRankList": [], "eventType": EVENT_TYPE_NORMAL
            }));
        }
    };
    let rank = position(&field, points);
    let mine = me(&p, rank, points, wins, losses, now);
    ok(json!({
        "eventCampaignName": name, "endDate": end, "personalRank": rank, "userGroupNo": 0,
        "userEventPoint": points, "clanRank": 0, "clanEventPoint": 0, "type": TYPE_PERSONAL,
        "groupNo": 0, "rankingKey": key, "rankingName": name_label(&id),
        "personalRankList": entries(&field, mine, points, top, now), "clanRankList": [],
        "eventType": kind
    }))
}

/// `/event/get_event_ranking_reward {eventCampaignId, rankingKey}`: the Arena's season rewards by
/// final place (arena::SEASON_REWARDS); events never close offline, so they pay no ranking
/// reward and list none.
pub fn ranking_reward(_ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let list: Vec<Value> = match board(env) {
        Some(Board::Arena) => {
            let mut from = 1;
            arena::season_reward_tiers()
                .into_iter()
                .map(|(upto, item, n)| {
                    let to = upto.min(FIELD_SIZE as i64 + 1);
                    let row = json!({ "rewardItemSet": arena::reward_set_of(item, n),
                                      "rankMin": from, "rankMax": to, "unit": "" });
                    from = to + 1;
                    row
                })
                .collect()
        }
        _ => Vec::new(),
    };
    ok(json!({ "rankingRewardList": list }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::Change;
    use crate::app::ApiError;
    use crate::services::dispatch;
    use crate::services::shapes::assert_complete;
    use jdr_core::settings::Settings;
    use serde_json::Map;

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

    fn setup(tag: &str) -> (Ctx, String) {
        let dir = std::env::temp_dir().join(format!("jdr-ranking-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        (ctx, uuid)
    }

    fn call(ctx: &Ctx, uuid: &str, path: &str, class: &str, param: Value) -> Map<String, Value> {
        let (r, known) = dispatch(ctx, path, &env(uuid, param));
        assert!(known, "{path} is routed");
        let m = match r {
            Ok(m) => m,
            Err(ApiError::Status(c)) => panic!("{path}: status {c}"),
            Err(ApiError::Internal(e)) => panic!("{path}: {e}"),
        };
        assert_complete(class, &Value::Object(m.clone()));
        m
    }

    fn detail(ctx: &Ctx, uuid: &str, key: &str, campaign: Value, range: i64) -> Map<String, Value> {
        call(
            ctx,
            uuid,
            "/event/get_event_ranking_detail",
            "Jjs.RankingDetailResponse",
            json!({"eventCampaignId": campaign, "rankingKey": key, "rankingRange": range}),
        )
    }

    fn ranks(list: &Value) -> Vec<i64> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|e| e["rank"].as_i64().unwrap())
            .collect()
    }

    /// The list names one personal ranking for the Arena and one per point event, each found
    /// by its campaign id with a non-null key (client).
    #[test]
    fn the_list_names_the_arena_and_the_point_events() {
        let (ctx, uuid) = setup("list");
        let l = call(
            &ctx,
            &uuid,
            "/event/get_event_ranking_list",
            "Jjs.RankingListResponse",
            json!({}),
        );
        let list = l["eventList"].as_array().unwrap();
        let arena = &list[0];
        assert_eq!(arena["eventCampaignId"], arena::CAMPAIGN_ID);
        assert_eq!(arena["rankingList"][0]["rankingKey"], ARENA_KEY);
        assert_eq!(arena["rankingList"][0]["type"], 1);
        assert_eq!(arena["rankingList"][0]["eventType"], 3);
        let ev = list
            .iter()
            .find(|e| e["eventCampaignId"] == "50045")
            .expect("the point event");
        assert_eq!(ev["rankingList"][0]["rankingKey"], "event-50045");
        assert_eq!(ev["rankingList"][0]["eventType"], 1);
        assert_eq!(ev["rankingList"][0]["rankingName"], "EQ_500045_Name");
        // Its event home shows the ranking button.
        let cat = crate::quests::events()
            .iter()
            .find(|e| e.event_id == "50045")
            .unwrap()
            .category;
        let (r, _) = dispatch(
            &ctx,
            "/bizarre/bizarre_list",
            &env(&uuid, json!({"category": cat})),
        );
        let info = Value::Object(r.unwrap())["eventInfo"].clone();
        assert_eq!(info["eventRankingFlg"], 1);
        assert_complete("Jjs.EventInfo", &info);
    }

    /// The Arena board: the player among the season's rivals, at the place `/pvp/home` reports,
    /// found by the id `/user/user_info_mini` gives; top and around views; the same rivals all
    /// season. The request's campaign id may be another event's or missing (client).
    #[test]
    fn arena_board_places_the_player_among_the_rivals() {
        let (ctx, uuid) = setup("arena");
        ctx.accounts
            .update(&uuid, |p| {
                p.progress.arena.season = arena::season_of(jdr_core::clock::now());
                p.progress.arena.points = 1_500;
                ((), Change::Bookkeeping)
            })
            .unwrap();
        let me = call(
            &ctx,
            &uuid,
            "/user/user_info_mini",
            "Jjs.UserInfoMiniResponse",
            json!({}),
        )["userInfoMini"]["id"]
            .clone();
        let (h, _) = dispatch(&ctx, "/pvp/home", &env(&uuid, json!({})));
        let home_rank = Value::Object(h.unwrap())["pvpUserInfo"]["rank"].clone();
        let mine = home_rank.as_i64().unwrap();
        for campaign in [json!(arena::CAMPAIGN_ID), json!("50045"), Value::Null] {
            let around = detail(&ctx, &uuid, ARENA_KEY, campaign.clone(), 0);
            assert_eq!(around["personalRank"], home_rank);
            assert_eq!(around["userEventPoint"], 1_500);
            assert_eq!(around["eventType"], 3);
            assert_eq!(around["type"], 1);
            let list = &around["personalRankList"];
            let r = ranks(list);
            assert!(r.contains(&mine) && r.len() <= 2 * AROUND + 1);
            assert!(r.windows(2).all(|w| w[1] == w[0] + 1), "consecutive places");
            let entry = list
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["userId"] == me)
                .expect("the player's own entry");
            assert_eq!(entry["rank"], mine);
            let pts: Vec<i64> = list
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["userEventPoint"].as_i64().unwrap())
                .collect();
            assert!(pts.windows(2).all(|w| w[0] >= w[1]), "best first");
        }
        let top = detail(&ctx, &uuid, ARENA_KEY, Value::Null, 1);
        assert_eq!(
            ranks(&top["personalRankList"]),
            (1..=TOP_ENTRIES as i64).collect::<Vec<_>>()
        );
        let again = detail(&ctx, &uuid, ARENA_KEY, Value::Null, 1);
        let ids = |m: &Map<String, Value>| -> Vec<Value> {
            m["personalRankList"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["userId"].clone())
                .collect()
        };
        assert_eq!(ids(&top), ids(&again), "the same rivals within the season");
        assert!(top["endDate"].as_str().unwrap() > "2026");
        // More points, a better place.
        ctx.accounts
            .update(&uuid, |p| {
                p.progress.arena.points = 6_000;
                ((), Change::Bookkeeping)
            })
            .unwrap();
        let better = detail(&ctx, &uuid, ARENA_KEY, Value::Null, 0);
        assert!(better["personalRank"].as_i64().unwrap() < mine);
        // Another season, other rivals.
        let s = arena::season_of(jdr_core::clock::now());
        let a: Vec<String> = arena_final_field(s)
            .iter()
            .map(|r| r.user_id.clone())
            .collect();
        let b: Vec<String> = arena_final_field(s + 1)
            .iter()
            .map(|r| r.user_id.clone())
            .collect();
        assert_ne!(a, b);
        assert_eq!(a.len(), FIELD_SIZE);
        // Rivals earn their points over the season.
        let start = arena_season_start(s);
        let early = arena_field(s, start);
        let late = arena_field(s, start + 7 * 86_400 - 1);
        assert!(early[0].points < late[0].points);
        assert!(late[0].points <= ARENA_TOP * 11 / 10);
    }

    /// Season rewards by place: contiguous tiers from 1 to the bottom of the field, every
    /// ItemSetInfo complete; events list none.
    #[test]
    fn rewards_by_place() {
        let (ctx, uuid) = setup("reward");
        let r = call(
            &ctx,
            &uuid,
            "/event/get_event_ranking_reward",
            "Jjs.RankingRewardResponse",
            json!({"eventCampaignId": null, "rankingKey": ARENA_KEY}),
        );
        let list = r["rankingRewardList"].as_array().unwrap();
        assert_eq!(list.len(), arena::season_reward_tiers().len());
        assert_eq!(list[0]["rankMin"], 1);
        for w in list.windows(2) {
            assert_eq!(
                w[1]["rankMin"].as_i64().unwrap(),
                w[0]["rankMax"].as_i64().unwrap() + 1
            );
        }
        assert_eq!(list.last().unwrap()["rankMax"], FIELD_SIZE as i64 + 1);
        for row in list {
            for it in row["rewardItemSet"]["itemList"].as_array().unwrap() {
                assert!(it["icon"].is_string() && it["name"].is_string());
            }
        }
        let e = call(
            &ctx,
            &uuid,
            "/event/get_event_ranking_reward",
            "Jjs.RankingRewardResponse",
            json!({"eventCampaignId": "50045", "rankingKey": "event-50045"}),
        );
        assert_eq!(e["rankingRewardList"], json!([]));
    }

    /// An event board ranks the player's event points; an unknown key is an empty board.
    #[test]
    fn event_board_and_unknown_keys() {
        let (ctx, uuid) = setup("event");
        let zero = detail(&ctx, &uuid, "event-50045", json!("50045"), 1);
        assert_eq!(zero["eventType"], 1);
        assert_eq!(zero["userEventPoint"], 0);
        let low = zero["personalRank"].as_i64().unwrap();
        ctx.accounts
            .update(&uuid, |p| {
                p.progress.event_points.insert("50045".into(), 2_000);
                ((), Change::Bookkeeping)
            })
            .unwrap();
        let some = detail(&ctx, &uuid, "event-50045", json!("50045"), 0);
        assert_eq!(some["userEventPoint"], 2_000);
        let high = some["personalRank"].as_i64().unwrap();
        assert!(high < low, "{high} < {low}");
        assert_eq!(high, position(&event_field("50045"), 2_000));
        // By the campaign id alone, too.
        let by_id = detail(&ctx, &uuid, "", json!("50045"), 0);
        assert_eq!(by_id["personalRank"], high);
        let none = detail(&ctx, &uuid, "nope", json!("1"), 0);
        assert_eq!(none["personalRankList"], json!([]));
        assert_eq!(none["personalRank"], 0);
    }
}
