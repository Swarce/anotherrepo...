//! Event rules (docs/research/20-event-exchange.md, 25-events-raids.md): what an event quest pays
//! and how Event Home shows it, by the event's kind.
//!
//! - **exchange**: the quests drop the event currency (`bizarre_end.eventResult` type 3) for the
//!   exchange (Shop3, `eventCollectType 2`, `eventShopFlg 1`).
//! - **point**: the quests earn event points (type 1); there is no claim route in 3.12, so a point
//!   reward is paid into the Message Box when the points reach it and is then shown as received
//!   (`getFlg 1`) in `/event/point_reward_list`. Ranked against AI rivals (services/ranking.rs).
//! - **high_score** / **high_score_shop** (`eventCollectType` 4 / 5): a high-score stage scores the
//!   battle from the client's `eventRankingSendInfo {turn, maxDamage, totalDamage}` by the stage's
//!   rule (`eventPointSettingList`); the result screen compares it with the stage's best
//!   (`oldEventRankingResultInfo`, `nowEventRankingPoint`); the ranking point is the sum of the
//!   stage bests. The shop kind also drops its currency by score (type 8, UserHighScoreShop).
//! - **quiz**: a quiz chapter has a question stage and three answer stages; the first answer stage
//!   cleared after the question is the player's answer. The quests drop the event's collection
//!   item (type 3), more for a right answer; certificate tiers by the number collected are paid
//!   like point rewards (`eventShopFlg 0` opens the reward list).
//!
//! Event bonus (ours): each of the event's bonus statues in the team and each bonus Mind on it
//! raise what the clear drops (points, currency, collection items), not the score.

use crate::accounts::Profile;
use crate::app::ApiResult;
use crate::collection::{Collection, Team};
use crate::quests::{self, EventDef};
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Event points per event-quest clear (ours).
pub const POINTS_PER_CLEAR: i64 = 50;
/// Currency per clear of an exchange event, or of a high-score shop event's other stages (ours).
pub const CURRENCY_PER_CLEAR: i64 = crate::items::EVENT_COINS_PER_CLEAR;
/// Quiz collection items: per clear of another stage, for a right and a wrong answer (ours).
pub const QUIZ_PER_CLEAR: i64 = 10;
pub const QUIZ_RIGHT: i64 = 30;
pub const QUIZ_WRONG: i64 = 5;
/// Event bonus per bonus statue in the team and per bonus Mind on it, and the cap, in percent
/// (ours).
pub const BONUS_STATUE_PCT: i64 = 25;
pub const BONUS_MIND_PCT: i64 = 10;
pub const BONUS_CAP_PCT: i64 = 100;
/// High-score shop currency: one per this many score points, at least 5 and at most 100 a clear
/// (ours).
pub const SCORE_PER_COIN: i64 = 2_000;

/// The kinds of event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Exchange,
    Point,
    HighScore,
    HighScoreShop,
    Quiz,
}

pub fn kind_of(e: &EventDef) -> Kind {
    match e.kind.as_str() {
        "point" => Kind::Point,
        "high_score" => Kind::HighScore,
        "high_score_shop" => Kind::HighScoreShop,
        "quiz" => Kind::Quiz,
        _ => Kind::Exchange,
    }
}

fn kind(event_id: &str) -> Option<Kind> {
    quests::event_by_id(event_id).map(kind_of)
}

/// Whether an event is a point event.
pub fn is_point_event(event_id: &str) -> bool {
    kind(event_id) == Some(Kind::Point)
}

/// Whether an event has a ranking (point and high-score events).
pub fn is_ranked(event_id: &str) -> bool {
    matches!(
        kind(event_id),
        Some(Kind::Point | Kind::HighScore | Kind::HighScoreShop)
    )
}

/// Whether an event is a high-score event.
pub fn is_high_score(event_id: &str) -> bool {
    matches!(kind(event_id), Some(Kind::HighScore | Kind::HighScoreShop))
}

/// The event's currency or collection item (0 = none).
pub fn currency(event_id: &str) -> i64 {
    quests::event_by_id(event_id)
        .map(|e| e.currency)
        .unwrap_or(0)
}

/// One reward reached by points (or collected items): the game's reward text label number (0 =
/// none: the text is ours), the item it gives and how many (ours), and the points needed (ours).
struct Reward {
    n: i64,
    item: i64,
    count: i64,
    points: i64,
}

const fn r(n: i64, item: i64, count: i64, points: i64) -> Reward {
    Reward {
        n,
        item,
        count,
        points,
    }
}

/// Event 50045 has ten reward labels (`point_event_reward_500045_147..156`): grey, blue, green
/// and red Stone Shards, the Courage, Wisdom and Spirit stones and brooches.
const E50045: &[Reward] = &[
    r(147, 1014, 5, 100),
    r(148, 1011, 5, 200),
    r(149, 1012, 5, 300),
    r(150, 1013, 5, 400),
    r(151, 1002, 3, 500),
    r(152, 1003, 3, 600),
    r(153, 1004, 3, 700),
    r(154, 1005, 1, 800),
    r(155, 1006, 1, 900),
    r(156, 1007, 1, 1000),
];
/// Quiz certificate tiers by items collected (ours: the event's certificates are paid after the
/// event by the count, which events that never close cannot do; their rewards are not recorded).
const QUIZ_TIERS: &[Reward] = &[
    r(0, 1014, 10, 50),
    r(0, 1002, 3, 120),
    r(0, 1003, 3, 200),
    r(0, 1004, 3, 280),
    r(0, 1005, 2, 360),
    r(0, 1006, 2, 360),
    r(0, 1007, 2, 360),
];

fn rewards(event_id: &str) -> &'static [Reward] {
    match (event_id, kind(event_id)) {
        ("50045", _) => E50045,
        (_, Some(Kind::Quiz)) => QUIZ_TIERS,
        _ => &[],
    }
}

/// The label id form of an event (`50045` -> `500045`).
pub fn label_id(event_id: &str) -> String {
    if event_id.len() == 5 && event_id.is_ascii() {
        format!("{}0{}", &event_id[..2], &event_id[2..])
    } else {
        event_id.to_string()
    }
}

fn item_set(item: i64, count: i64) -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": [], "clanObjectList": [],
            "itemList": [crate::items::reward_item(item, count)] })
}

/// The event's points: event points, the high-score total or the quiz items collected.
pub fn points(p: &Profile, event_id: &str) -> i64 {
    p.progress.event_points.get(event_id).copied().unwrap_or(0)
}

/// Add points; every reward reached goes to the Message Box once. Returns the rewards reached
/// now, as `eventRewardList` entries.
pub fn add_points(p: &mut Profile, event_id: &str, n: i64, now: i64) -> Vec<Value> {
    let before = points(p, event_id);
    let after = before + n;
    p.progress.event_points.insert(event_id.to_string(), after);
    let mut got = Vec::new();
    for r in rewards(event_id) {
        if before < r.points && r.points <= after {
            p.progress.presents.add(
                crate::presents::DATA_ITEM,
                r.item,
                r.count,
                format!("Event reward: {} points", r.points),
                now,
            );
            got.push(json!({ "itemSetInfo": item_set(r.item, r.count), "necessaryEventPoint": r.points }));
        }
    }
    got
}

/// `/event/point_reward_list {eventCampaignId}`.
pub fn point_reward_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let id = match env.param.get("eventCampaignId") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    };
    let have = points(&p, &id);
    let list: Vec<Value> = rewards(&id)
        .iter()
        .map(|r| {
            let (text, replace) = if r.n > 0 {
                (format!("point_event_reward_{}_{}_Personal_text", label_id(&id), r.n), String::new())
            } else {
                (String::new(), format!("Certificate: {} collected", r.points))
            };
            json!({ "rewardTitle": format!("ITEM_{:04}_NAME", r.item), "rewardTitleReplace": "",
                    "rewardText": text, "rewardTextReplace": replace, "necessaryEventPoint": r.points,
                    "getFlg": i64::from(have >= r.points), "itemSetInfo": item_set(r.item, r.count) })
        })
        .collect();
    ok(
        json!({ "userEventPoint": have, "clanEventPoint": 0, "userPointRewardList": list,
               "clanPointRewardList": [] }),
    )
}

// ---- Event Home ----

/// The event currency as ItemInfo (Event Home, Shop3 header; counts are absolute). Shop3 copies
/// the list, so it is never null.
pub fn currency_list(c: &Collection, item: i64) -> Value {
    json!(crate::items::find(item)
        .map(|d| vec![crate::items::item_info(d, c.item_count(item))])
        .unwrap_or_default())
}

/// `bizarre_list.eventInfo` (EventInfo). Event Home backs out with an "expired" dialog unless
/// `eventCampaignId` is set. The exchange button needs `eventShopFlg 1` and a collect type other
/// than 0/4; with flag 0 the "check rewards" button opens the reward list instead; collect type 1
/// shows the points, 2 and 5 the `eventItemList` icons; `eventRankingFlg 1` asks for the ranking
/// list and shows the ranking button. BGM and the event summon stay off.
pub fn event_info(p: &Profile, e: &EventDef) -> Value {
    let (shop, collect, ranking) = match kind_of(e) {
        Kind::Exchange => (1, 2, 0),
        Kind::Point => (0, 1, 1),
        Kind::HighScore => (0, 4, 1),
        Kind::HighScoreShop => (1, 5, 1),
        Kind::Quiz => (0, 2, 0),
    };
    json!({ "eventCampaignId": e.event_id, "eventShopFlg": shop, "eventCollectType": collect,
            "eventRankingFlg": ranking, "eventMissiongFlg": 0, "eventBg": "",
            "htmlLink": format!("/information/event_{}.html", e.event_id), "eventBgm": "",
            "eventTutorialImage": "", "eventItemList": currency_list(&p.collection, e.currency),
            "eventPointImage": "", "userEventPoint": points(p, &e.event_id), "clanEventPoint": 0,
            "endDate": "", "finishedDate": "", "banner": e.banner })
}

// ---- Event bonus ----

/// The bonus in percent for a team (0 without one), capped.
pub fn bonus_pct(c: &Collection, e: &EventDef, team: Option<&Team>) -> i64 {
    let Some(t) = team else { return 0 };
    let statues = t
        .slots
        .iter()
        .filter_map(|uid| c.known_statue(*uid))
        .filter(|o| e.bonus_statues.contains(&o.statue_id))
        .count() as i64;
    let minds = t
        .minds
        .iter()
        .flatten()
        .filter_map(|(_, uid)| c.minds.iter().find(|m| m.uid == *uid))
        .filter(|m| e.bonus_minds.contains(&m.mind_id))
        .count() as i64;
    (statues * BONUS_STATUE_PCT + minds * BONUS_MIND_PCT).min(BONUS_CAP_PCT)
}

fn boosted(n: i64, pct: i64) -> i64 {
    n * (100 + pct) / 100
}

// ---- High scores ----

/// A stage's best battle (DTEventPointType values of that battle).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct HighScore {
    pub score: i64,
    pub turn: i64,
    pub max_damage: i64,
    pub total_damage: i64,
}

/// The scoring rule's ranges per DTEventPointType (ours): turns score `TURN_MAX` for a one-turn
/// clear, `TURN_STEP` less per further turn, at least `TURN_MIN`; damage scores itself up to a cap.
pub const TURN_MAX: i64 = 100_000;
pub const TURN_MIN: i64 = 10_000;
pub const TURN_STEP: i64 = 5_000;
pub const MAX_DAMAGE_CAP: i64 = 200_000;
pub const TOTAL_DAMAGE_CAP: i64 = 1_000_000;

/// `eventPointSettingList` of a stage: one `{type, min, max}` per scored value; the client shows
/// the matching "Scored on: ..." condition text (BizarreQuestSceneManager.MakeBattleCondition).
pub fn point_settings(stage_id: &str) -> Vec<Value> {
    quests::scoring_of(stage_id)
        .unwrap_or(&[])
        .iter()
        .map(|t| {
            let (min, max) = match t {
                2 => (TURN_MIN, TURN_MAX),
                3 => (0, MAX_DAMAGE_CAP),
                _ => (0, TOTAL_DAMAGE_CAP),
            };
            json!({ "type": t, "min": min, "max": max })
        })
        .collect()
}

/// `StageInfo.highScore`: the stage's best score on a high-score stage, else -1 (hidden).
pub fn stage_high_score(p: &quests::Progress, stage_id: &str) -> i64 {
    if quests::scoring_of(stage_id).is_none() {
        return -1;
    }
    p.high_scores.get(stage_id).map(|h| h.score).unwrap_or(0)
}

/// Score a battle by the stage's rule.
pub fn score(rule: &[i64], turn: i64, max_damage: i64, total_damage: i64) -> i64 {
    rule.iter()
        .map(|t| match t {
            2 => (TURN_MAX - TURN_STEP * (turn.max(1) - 1)).max(TURN_MIN),
            3 => max_damage.clamp(0, MAX_DAMAGE_CAP),
            4 => total_damage.clamp(0, TOTAL_DAMAGE_CAP),
            _ => 0,
        })
        .sum()
}

/// The client's `eventRankingSendInfo {turn, maxDamage, totalDamage}` (an object, or a JSON
/// string of one).
fn send_info(env: &RequestEnvelope) -> (i64, i64, i64) {
    let v = match env.param.get("eventRankingSendInfo") {
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(Value::Null),
        Some(v) => v.clone(),
        None => Value::Null,
    };
    let n = |k: &str| v.get(k).and_then(Value::as_i64).unwrap_or(0).max(0);
    (n("turn"), n("maxDamage"), n("totalDamage"))
}

/// `EventRankingResultInfo` of a high-score stage's best: the score and one row per scored value
/// (EventRankingMode 2 turns, 3 max damage, 4 total damage). The result screen's high-score panel
/// shows one row per entry, beside this battle's value of the same mode.
fn ranking_result(rule: &[i64], h: &HighScore) -> Value {
    let rows: Vec<Value> = rule
        .iter()
        .map(|t| {
            let data = match t {
                2 => h.turn,
                3 => h.max_damage,
                _ => h.total_damage,
            };
            json!({ "mode": t, "data": data })
        })
        .collect();
    json!({ "point": h.score, "eventDataList": rows })
}

/// `oldEventRankingResultInfo` outside a high-score stage: an empty list shows no panel (the panel
/// reads the list's Count without a null check, BattleResultHighScoreUIController.Setup).
pub fn no_ranking_result() -> Value {
    json!({ "point": 0, "eventDataList": [] })
}

fn high_score_total(p: &Profile, e: &EventDef) -> i64 {
    e.scoring
        .keys()
        .filter_map(|s| p.progress.high_scores.get(s))
        .map(|h| h.score)
        .sum()
}

// ---- Quests ----

/// What an event-quest clear paid, for the `bizarre_end` reply.
pub struct Settlement {
    pub event_result: Value,
    /// `nowEventRankingPoint` (this battle's score on a high-score stage).
    pub now_point: i64,
    pub old_result: Value,
}

/// Pay an event-quest clear (once, for the battle that started).
pub fn settle(
    p: &mut Profile,
    e: &EventDef,
    chapter_id: &str,
    stage_id: &str,
    team: Option<&Team>,
    env: &RequestEnvelope,
    now: i64,
) -> Settlement {
    let pct = bonus_pct(&p.collection, e, team);
    let id = e.event_id.as_str();
    let mut out = Settlement {
        event_result: crate::services::bizarre::no_event(),
        now_point: 0,
        old_result: no_ranking_result(),
    };
    let items = |item: i64, n: i64| json!([{ "itemId": item, "count": n }]);
    match kind_of(e) {
        Kind::Point => {
            let n = boosted(POINTS_PER_CLEAR, pct);
            let got = add_points(p, id, n, now);
            out.event_result = json!({ "eventType": 1, "userEventPointAll": points(p, id),
                "getUserEventPoint": n, "eventRewardList": got, "getEventItemList": [] });
        }
        Kind::Exchange => {
            let n = boosted(CURRENCY_PER_CLEAR, pct);
            p.collection.add_item(e.currency, n);
            out.event_result = json!({ "eventType": 3, "userEventPointAll": 0, "getUserEventPoint": 0,
                "eventRewardList": [], "getEventItemList": items(e.currency, n) });
        }
        Kind::HighScore | Kind::HighScoreShop => {
            let shop = kind_of(e) == Kind::HighScoreShop;
            let mut coins = CURRENCY_PER_CLEAR;
            if let Some(rule) = e.scoring.get(stage_id) {
                let (turn, maxd, totd) = send_info(env);
                let s = score(rule, turn, maxd, totd);
                let old = p
                    .progress
                    .high_scores
                    .get(stage_id)
                    .copied()
                    .unwrap_or_default();
                out.old_result = ranking_result(rule, &old);
                out.now_point = s;
                if s > old.score {
                    p.progress.high_scores.insert(
                        stage_id.to_string(),
                        HighScore {
                            score: s,
                            turn,
                            max_damage: maxd,
                            total_damage: totd,
                        },
                    );
                    let total = high_score_total(p, e);
                    p.progress.event_points.insert(id.to_string(), total);
                }
                coins = (s / SCORE_PER_COIN).clamp(5, 100);
            }
            if shop {
                let n = boosted(coins, pct);
                p.collection.add_item(e.currency, n);
                out.event_result = json!({ "eventType": 8, "userEventPointAll": 0, "getUserEventPoint": 0,
                    "eventRewardList": [], "getEventItemList": items(e.currency, n) });
            }
        }
        Kind::Quiz => {
            let base = match e.quiz.iter().find(|q| q.chapter_id == chapter_id) {
                Some(q) if q.answers.iter().any(|a| a == stage_id) => {
                    let asked = p.progress.cleared.contains(&q.question);
                    match p.progress.quiz_answers.get(chapter_id) {
                        None if asked => {
                            p.progress
                                .quiz_answers
                                .insert(chapter_id.to_string(), stage_id.to_string());
                            if stage_id == q.correct {
                                QUIZ_RIGHT
                            } else {
                                QUIZ_WRONG
                            }
                        }
                        // Answered already (or not asked yet): the right stage pays as any other.
                        _ if stage_id == q.correct => QUIZ_PER_CLEAR,
                        _ => 0,
                    }
                }
                _ => QUIZ_PER_CLEAR,
            };
            let n = boosted(base, pct);
            p.collection.add_item(e.currency, n);
            let got = add_points(p, id, n, now);
            out.event_result = json!({ "eventType": 3, "userEventPointAll": 0, "getUserEventPoint": 0,
                "eventRewardList": got,
                "getEventItemList": if n > 0 { items(e.currency, n) } else { json!([]) } });
        }
    }
    out
}

/// Quiz standing: (answered, right) for an event.
pub fn quiz_standing(p: &Profile, e: &EventDef) -> (usize, usize) {
    let answered: Vec<&quests::QuizDef> = e
        .quiz
        .iter()
        .filter(|q| p.progress.quiz_answers.contains_key(&q.chapter_id))
        .collect();
    let right = answered
        .iter()
        .filter(|q| p.progress.quiz_answers.get(&q.chapter_id) == Some(&q.correct))
        .count();
    (answered.len(), right)
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
    fn points_reach_rewards_once_and_the_list_shows_them() {
        let dir = std::env::temp_dir().join(format!("jdr-evpt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        assert!(super::is_point_event("50045") && !super::is_point_event("50035"));
        let l = call(
            "/event/point_reward_list",
            json!({"eventCampaignId": "50045"}),
        )
        .unwrap();
        let rows = l["userPointRewardList"].as_array().unwrap().clone();
        assert_eq!(rows.len(), 10);
        assert!(rows.iter().all(|r| r["getFlg"] == 0));
        assert_eq!(
            rows[0]["rewardText"],
            "point_event_reward_500045_147_Personal_text"
        );
        assert_eq!(l["clanPointRewardList"], json!([]));
        let mut got = Vec::new();
        ctx.accounts
            .update(&uuid, |p| {
                got = super::add_points(p, "50045", 250, 0);
                got.extend(super::add_points(p, "50045", 50, 0));
                ((), crate::accounts::Change::Progress)
            })
            .unwrap();
        let reached: Vec<i64> = got
            .iter()
            .map(|g| g["necessaryEventPoint"].as_i64().unwrap())
            .collect();
        assert_eq!(
            reached,
            [100, 200, 300],
            "each reward once, as points cross it"
        );
        let l = call(
            "/event/point_reward_list",
            json!({"eventCampaignId": "50045"}),
        )
        .unwrap();
        assert_eq!(l["userEventPoint"], 300);
        let flags: Vec<i64> = l["userPointRewardList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["getFlg"].as_i64().unwrap())
            .collect();
        assert_eq!(&flags[..4], &[1, 1, 1, 0]);
        // The rewards wait in the Message Box (plus the two welcome gifts).
        let b = call(
            "/present/present_list",
            json!({"prevNum": 1, "receivedFlag": 0}),
        )
        .unwrap();
        assert_eq!(b["allPresentNum"], 5);
    }
}

/// Event kinds end to end: high scores, the high-score shop, quizzes, bonuses and the null walk of
/// every reply they touch (docs/research/25-events-raids.md).
#[cfg(test)]
mod kind_tests {
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
        let dir = std::env::temp_dir().join(format!("jdr-evkind-{tag}-{}", std::process::id()));
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
        if !class.is_empty() {
            assert_complete(class, &Value::Object(m.clone()));
        }
        m
    }

    /// Start and win a stage; `send` is the client's eventRankingSendInfo.
    fn win(ctx: &Ctx, uuid: &str, stage: &str, send: Value) -> Map<String, Value> {
        call(
            ctx,
            uuid,
            "/bizarre/bizarre_start",
            "",
            json!({"stageId": stage, "teamNo": 1}),
        );
        call(
            ctx,
            uuid,
            "/bizarre/bizarre_end",
            "Jjs.BizarreEndResponse",
            json!({"stageId": stage, "result": 0, "starClearList": "1", "eventRankingSendInfo": send}),
        )
    }

    fn ev(id: &str) -> &'static EventDef {
        quests::event_by_id(id).unwrap()
    }

    fn stage_row(ctx: &Ctx, uuid: &str, chapter: &str, stage: &str) -> Value {
        let l = call(
            ctx,
            uuid,
            "/bizarre/stage_list",
            "Jjs.StageListResponse",
            json!({"chapterId": chapter, "stageType": 1}),
        );
        l["stageList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["stageId"] == stage)
            .unwrap()
            .clone()
    }

    fn got(r: &Map<String, Value>) -> i64 {
        r["eventResult"]["getEventItemList"]
            .get(0)
            .map(|x| x["count"].as_i64().unwrap())
            .unwrap_or(0)
    }

    #[test]
    fn scoring_follows_the_stage_rule() {
        // Max & Total & Turns: each value scores in its range.
        assert_eq!(score(&[3, 4, 2], 1, 1_000, 5_000), 1_000 + 5_000 + TURN_MAX);
        assert_eq!(score(&[2], 3, 0, 0), TURN_MAX - 2 * TURN_STEP);
        assert_eq!(score(&[2], 99, 0, 0), TURN_MIN, "never below the minimum");
        assert_eq!(score(&[3], 1, 9_999_999, 0), MAX_DAMAGE_CAP, "capped");
        assert_eq!(score(&[4], 1, 0, -5), 0);
        // The rules come from the stage names of the player's files.
        let e = ev("560005");
        assert_eq!(e.scoring["STAGE_EV_560005_01_01"], [3], "Max Damage");
        assert_eq!(e.scoring["STAGE_EV_560005_01_07"], [2], "Turns");
        assert_eq!(e.scoring["STAGE_EV_560005_02_01"], [3, 4, 2]);
        assert_eq!(ev("560001").scoring.len(), 1);
    }

    #[test]
    fn a_high_score_stage_keeps_its_best_and_ranks_the_total() {
        let (ctx, uuid) = setup("hs");
        let e = ev("560005");
        let (stage, chapter) = ("STAGE_EV_560005_02_01", "CHAPTER_EV_560005_02");
        // Event Home: collect type 4, no shop, a ranking.
        let l = call(
            &ctx,
            &uuid,
            "/bizarre/bizarre_list",
            "Jjs.BizarreListResponse",
            json!({"category": e.category}),
        );
        assert_eq!(l["eventInfo"]["eventCollectType"], 4);
        assert_eq!(l["eventInfo"]["eventShopFlg"], 0);
        assert_eq!(l["eventInfo"]["eventRankingFlg"], 1);
        // The stage lists its rule and shows a best of 0; other stages hide the box (-1).
        let row = stage_row(&ctx, &uuid, chapter, stage);
        let kinds: Vec<i64> = row["eventPointSettingList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["type"].as_i64().unwrap())
            .collect();
        assert_eq!(kinds, [3, 4, 2]);
        assert_eq!(row["highScore"], 0);
        let plain = stage_row(&ctx, &uuid, "CHAPTER_EV_560005_03", "STAGE_EV_560005_03_01");
        assert_eq!(plain["highScore"], -1);
        assert_eq!(plain["eventPointSettingList"], json!([]));
        // First win: the panel shows a best of 0 with the stage's three rows.
        let r = win(
            &ctx,
            &uuid,
            stage,
            json!({"turn": 3, "maxDamage": 4_000, "totalDamage": 20_000}),
        );
        let first = 4_000 + 20_000 + TURN_MAX - 2 * TURN_STEP;
        assert_eq!(r["nowEventRankingPoint"], first);
        let old = &r["oldEventRankingResultInfo"];
        assert_eq!(old["point"], 0);
        let modes: Vec<i64> = old["eventDataList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["mode"].as_i64().unwrap())
            .collect();
        assert_eq!(modes, [3, 4, 2]);
        assert_eq!(
            r["eventResult"]["eventType"], 0,
            "no currency without a shop"
        );
        assert_eq!(r["raidStageInfo"]["raidStageUid"], 0, "never a raid");
        // A worse battle keeps the best; the panel compares with it.
        let r = win(
            &ctx,
            &uuid,
            stage,
            json!({"turn": 9, "maxDamage": 100, "totalDamage": 100}),
        );
        assert_eq!(r["oldEventRankingResultInfo"]["point"], first);
        assert_eq!(
            r["oldEventRankingResultInfo"]["eventDataList"][0]["data"],
            4_000
        );
        assert_eq!(stage_row(&ctx, &uuid, chapter, stage)["highScore"], first);
        // A second stage adds its best to the ranking total.
        let r = win(
            &ctx,
            &uuid,
            "STAGE_EV_560005_01_01",
            json!({"turn": 2, "maxDamage": 7_000, "totalDamage": 9_000}),
        );
        assert_eq!(r["nowEventRankingPoint"], 7_000);
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert_eq!(points(&p, "560005"), first + 7_000);
        // The ranking: listed, and the player placed by the total.
        let list = call(
            &ctx,
            &uuid,
            "/event/get_event_ranking_list",
            "Jjs.RankingListResponse",
            json!({}),
        );
        assert!(list["eventList"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["eventCampaignId"] == "560005"));
        let d = call(
            &ctx,
            &uuid,
            "/event/get_event_ranking_detail",
            "Jjs.RankingDetailResponse",
            json!({"eventCampaignId": "560005", "rankingKey": "event-560005", "rankingRange": 0}),
        );
        assert_eq!(d["userEventPoint"], first + 7_000);
        let field = crate::services::ranking::event_field("560005");
        assert_eq!(
            d["personalRank"],
            crate::services::ranking::position(&field, first + 7_000)
        );
        assert!(field[0].points > 500_000, "a field scaled to seven stages");
        // A lost battle scores nothing and shows no panel.
        call(
            &ctx,
            &uuid,
            "/bizarre/bizarre_start",
            "",
            json!({"stageId": stage, "teamNo": 1}),
        );
        let r = call(
            &ctx,
            &uuid,
            "/bizarre/bizarre_end",
            "Jjs.BizarreEndResponse",
            json!({"stageId": stage, "result": 1}),
        );
        assert_eq!(r["nowEventRankingPoint"], 0);
        assert_eq!(r["oldEventRankingResultInfo"]["eventDataList"], json!([]));
    }

    #[test]
    fn the_high_score_shop_pays_its_coin_by_score() {
        let (ctx, uuid) = setup("hsshop");
        let e = ev("560001");
        assert_eq!(kind_of(e), Kind::HighScoreShop);
        let l = call(
            &ctx,
            &uuid,
            "/bizarre/bizarre_list",
            "Jjs.BizarreListResponse",
            json!({"category": e.category}),
        );
        assert_eq!(l["eventInfo"]["eventCollectType"], 5);
        assert_eq!(l["eventInfo"]["eventShopFlg"], 1);
        assert_eq!(l["eventInfo"]["eventItemList"][0]["itemId"], e.currency);
        let have = |ctx: &Ctx| {
            ctx.accounts
                .get(&uuid)
                .unwrap()
                .unwrap()
                .collection
                .item_count(e.currency)
        };
        let start = have(&ctx);
        let r = win(
            &ctx,
            &uuid,
            "STAGE_EV_560001_02_01",
            json!({"turn": 4, "maxDamage": 30_000, "totalDamage": 50_000}),
        );
        // Max Damage only: the score is the largest hit; coins one per 2,000 points.
        assert_eq!(r["nowEventRankingPoint"], 30_000);
        assert_eq!(r["eventResult"]["eventType"], 8, "UserHighScoreShop");
        assert_eq!(
            r["eventResult"]["getEventItemList"][0]["itemId"],
            e.currency
        );
        assert_eq!(got(&r), 15);
        // A training stage pays the flat amount.
        let r = win(&ctx, &uuid, "STAGE_EV_560001_01_01", json!({}));
        assert_eq!(got(&r), CURRENCY_PER_CLEAR);
        assert_eq!(r["nowEventRankingPoint"], 0);
        let held = start + 15 + CURRENCY_PER_CLEAR;
        assert_eq!(have(&ctx), held);
        // The exchange takes that coin.
        let s = call(
            &ctx,
            &uuid,
            "/shop/event_shop_list",
            "Jjs.EventShopListResponse",
            json!({"eventId": "560001"}),
        );
        assert_eq!(
            s["eventShopList"][0]["goodsList"][0]["payItemId"],
            e.currency
        );
        assert_eq!(s["itemList"][0]["itemId"], e.currency);
        assert_eq!(s["itemList"][0]["count"], held);
        let g = s["eventShopList"][0]["goodsList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["price"].as_i64().unwrap() <= held)
            .expect("affordable goods")
            .clone();
        let b = call(
            &ctx,
            &uuid,
            "/shop/event_shop_buy",
            "Jjs.EventShopBuyResponse",
            json!({"eventId": "560001", "goodsId": g["goodsId"], "count": 1}),
        );
        assert_eq!(
            b["itemList"][0]["count"].as_i64().unwrap(),
            held - g["price"].as_i64().unwrap()
        );
    }

    #[test]
    fn a_quiz_pays_the_first_answer_and_certificates() {
        let (ctx, uuid) = setup("quiz");
        let e = ev("50090");
        assert_eq!(e.quiz.len(), 10);
        let l = call(
            &ctx,
            &uuid,
            "/bizarre/bizarre_list",
            "Jjs.BizarreListResponse",
            json!({"category": e.category}),
        );
        assert_eq!(l["eventInfo"]["eventCollectType"], 2);
        assert_eq!(
            l["eventInfo"]["eventShopFlg"], 0,
            "the reward list, not a shop"
        );
        assert_eq!(l["eventInfo"]["eventItemList"][0]["itemId"], 3106);
        let start = ctx
            .accounts
            .get(&uuid)
            .unwrap()
            .unwrap()
            .collection
            .item_count(3106);
        let q = &e.quiz[0];
        let wrong = q.answers.iter().find(|a| **a != q.correct).unwrap();
        // An answer before the question does not count.
        let r = win(&ctx, &uuid, wrong, json!({}));
        assert_eq!(got(&r), 0);
        // After the question: the first answer counts (here a wrong one) ...
        let r = win(&ctx, &uuid, &q.question, json!({}));
        assert_eq!(got(&r), QUIZ_PER_CLEAR);
        let r = win(&ctx, &uuid, wrong, json!({}));
        assert_eq!(got(&r), QUIZ_WRONG);
        assert_eq!(r["eventResult"]["eventType"], 3);
        // ... and later the right stage pays as any quest.
        let r = win(&ctx, &uuid, &q.correct, json!({}));
        assert_eq!(got(&r), QUIZ_PER_CLEAR);
        // A right first answer pays the most.
        let q2 = &e.quiz[1];
        win(&ctx, &uuid, &q2.question, json!({}));
        let r = win(&ctx, &uuid, &q2.correct, json!({}));
        assert_eq!(got(&r), QUIZ_RIGHT);
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert_eq!(quiz_standing(&p, e), (2, 1));
        let total = 2 * QUIZ_PER_CLEAR + QUIZ_WRONG + QUIZ_PER_CLEAR + QUIZ_RIGHT;
        assert_eq!(points(&p, "50090"), total);
        assert_eq!(p.collection.item_count(3106), start + total);
        // The certificate tiers (the reward list): the first one is reached.
        let l = call(
            &ctx,
            &uuid,
            "/event/point_reward_list",
            "Jjs.EventPointListResponse",
            json!({"eventCampaignId": "50090"}),
        );
        assert_eq!(l["userEventPoint"], total);
        let rows = l["userPointRewardList"].as_array().unwrap();
        assert_eq!(rows.len(), QUIZ_TIERS.len());
        assert_eq!(rows[0]["getFlg"], 1);
        assert_eq!(rows[1]["getFlg"], 0);
        assert!(rows[0]["rewardTextReplace"].as_str().unwrap().len() > 3);
    }

    #[test]
    fn bonus_units_raise_the_drops() {
        let (ctx, uuid) = setup("bonus");
        let e = ev("50035");
        let stage = &quests::chapters()
            .iter()
            .find(|c| c.category == e.category)
            .unwrap()
            .stages[0]
            .stage_id;
        let r = win(&ctx, &uuid, stage, json!({}));
        assert_eq!(got(&r), CURRENCY_PER_CLEAR);
        // Put two bonus statues and a bonus Mind into the main tactical team.
        ctx.accounts
            .update(&uuid, |p| {
                let c = &mut p.collection;
                let uid = |id: &str| c.statues.iter().find(|o| o.statue_id == id).unwrap().uid;
                let (a, b) = (uid(&e.bonus_statues[0]), uid(&e.bonus_statues[1]));
                let mind = c
                    .minds
                    .iter()
                    .find(|m| m.mind_id == e.bonus_minds[0])
                    .unwrap()
                    .uid;
                let t = c
                    .teams
                    .iter_mut()
                    .find(|t| t.stage_type == 1 && t.main)
                    .unwrap();
                t.slots[1] = a;
                t.slots[2] = b;
                t.minds[0] = vec![(1, mind)];
                ((), Change::Progress)
            })
            .unwrap();
        let r = win(&ctx, &uuid, stage, json!({}));
        let pct = 2 * BONUS_STATUE_PCT + BONUS_MIND_PCT;
        assert_eq!(got(&r), CURRENCY_PER_CLEAR * (100 + pct) / 100);
        // The event page names them in English.
        let page = crate::web::page(&format!("/information/event_{}.html", e.event_id));
        let page = String::from_utf8(page.body).unwrap();
        assert!(
            page.contains("Event bonus") && !page.contains("STATUE_NAME_"),
            "{page}"
        );
    }
}
