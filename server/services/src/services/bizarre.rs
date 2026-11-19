//! `/bizarre/*` and `/support/support_user_s`: the story quest flow.

use crate::accounts::{Change, Profile};
use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::quests::{self, PendingBattle, STORY_CATEGORY};
use crate::services::statue::{profile, with_profile};
use crate::services::{dungeon, ok, support};
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use jdr_core::log;
use serde_json::{json, Value};

/// `BizarreSelectResponse`: the story button (daily and event categories come later).
pub fn get_bizarre_select(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    // Tiles by bannerType (1 story, 2 daily, 3 event); `category` is what a tapped tile asks
    // bizarre_list for. Every string must be a string: a null bannerFileName or eventMasterId
    // crashes the screen (docs/research/13-menus.md, Quest Select).
    let (open, close) = ("2020-01-01 00:00:00", "2099-12-31 23:59:59");
    let mut list = vec![quests::story_quest_info()];
    if let Some(d) = quests::daily_today() {
        list.push(
            json!({ "questHomeId": "DY", "category": d.category, "bannerType": 2,
            "bannerFileName": d.banner, "day": d.category - 6, "Priority": 2, "eventMasterId": "",
            "startDate": open, "endDate": close }),
        );
    }
    let mut campaigns = Vec::new();
    for (i, e) in quests::events().iter().enumerate() {
        list.push(
            json!({ "questHomeId": "EV", "category": e.category, "bannerType": 3,
            "bannerFileName": e.banner, "day": 0, "Priority": 3 + i, "eventMasterId": e.event_id,
            "startDate": open, "endDate": close }),
        );
        // Campaign Info: shown with bizarreHomeFlag, opens s3_url + htmlLink in the web view
        // (our web module answers any path).
        campaigns.push(json!({ "eventCampaignId": e.event_id, "banner": e.banner,
            "htmlLink": format!("/information/campaign_{}.html", e.event_id),
            "signboardPriority": i + 1, "bizarreHomeFlag": true, "signboardParam": "" }));
    }
    // dungeonLockFlg 0 opens the Labyrinth tile (docs/research/22-labyrinth.md).
    ok(json!({ "questList": list, "eventCampaignList": campaigns, "dungeonLockFlg": 0 }))
}

/// `BizarreListResponse` for a category: every story chapter (action and tactical; the client
/// splits them by `stageType`). All chapters are open (offline sandbox).
pub fn bizarre_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let category = env
        .param
        .get("category")
        .and_then(Value::as_i64)
        .unwrap_or(STORY_CATEGORY);
    let list: Vec<Value> = quests::chapters()
        .iter()
        .filter(|c| c.category == category)
        .map(|c| quests::chapter_info(c, &p.progress))
        .collect();
    // Event Home backs out with an "expired" dialog unless eventCampaignId is set. The exchange
    // is on (collect type 2 = items, Build 22): its button needs eventShopFlg 1, and Shop3 copies
    // eventItemList (never null). A point event has a ranking (eventRankingFlg 1: the home asks
    // for /event/get_event_ranking_list and shows its ranking button, services/ranking.rs). BGM
    // and the event summon stay off.
    let event_info = quests::events()
        .iter()
        .find(|e| e.category == category)
        .map(|e| {
            {
            // A point event shows its points and the rewards button (collect type 1, shop
            // flag 0); the others have the Fight Coin exchange (collect type 2, shop flag 1).
            let point = crate::services::event::is_point_event(&e.event_id);
            json!({ "eventCampaignId": e.event_id, "eventShopFlg": if point { 0 } else { 1 },
                "eventCollectType": if point { 1 } else { 2 },
                "eventRankingFlg": i64::from(point), "eventMissiongFlg": 0, "eventBg": "",
                "htmlLink": format!("/information/event_{}.html", e.event_id), "eventBgm": "",
                "eventTutorialImage": "",
                "eventItemList": crate::services::item::event_coin_list(&p.collection), "eventPointImage": "",
                "userEventPoint": crate::services::event::points(&p, &e.event_id), "clanEventPoint": 0,
                "endDate": "", "finishedDate": "", "banner": e.banner })
        }
        })
        .unwrap_or_else(|| json!({}));
    ok(json!({ "chapterList": list, "eventInfo": event_info, "raidInfo": {} }))
}

/// `StageListResponse` for one chapter.
pub fn stage_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let id = env
        .param
        .get("chapterId")
        .and_then(Value::as_str)
        .unwrap_or("");
    let list: Vec<Value> = quests::chapters()
        .iter()
        .find(|c| c.chapter_id == id)
        .map(|c| {
            c.stages
                .iter()
                .enumerate()
                .map(|(i, s)| quests::stage_info(c, s, i, &p.progress))
                .collect()
        })
        .unwrap_or_default();
    ok(json!({ "stageList": list }))
}

/// `CheckTimedChapterResponse`: no timed chapters offline.
pub fn check_timed_chapter(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "chapterList": [] }))
}

/// `SupportUserResponse`: today's AI supporters (services/support.rs; no other players offline).
pub fn support_user_s(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(support::support_list(&p, jdr_core::clock::now()))
}

/// `BizarreStartResponse`: the battle description for the chosen stage and team.
pub fn bizarre_start(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let stage_id = env
        .param
        .get("stageId")
        .and_then(Value::as_str)
        .unwrap_or("");
    if dungeon::is_dungeon_battle(env, stage_id) {
        return dungeon::bizarre_start(ctx, env, stage_id);
    }
    let (c, s) = quests::find_stage(stage_id)
        .ok_or_else(|| ApiError::Internal(format!("unknown stage {stage_id:?}")))?;
    let team_no = env.param.get("teamNo").and_then(Value::as_i64).unwrap_or(1);
    // The supporter picked on the support list (an id we handed out; anything else: none).
    let support = env
        .param
        .get("supportId")
        .and_then(Value::as_str)
        .and_then(support::Supporter::parse);
    let (p, team) = with_profile(ctx, env, |p| {
        let team = quests::team_for(&p.collection, c.stage_type, team_no).cloned();
        *p.progress
            .play_counts
            .entry(stage_id.to_string())
            .or_insert(0) += 1;
        p.progress.pending = team.as_ref().map(|t| PendingBattle {
            stage_id: stage_id.to_string(),
            stage_type: t.stage_type,
            team_num: t.team_num,
            support: support.as_ref().map(|s| s.id.clone()),
        });
        (team, Change::Bookkeeping)
    })?;
    let mut reply = battle_reply(&p, c, s, team.as_ref());
    reply["tarotInfo"] = json!({});
    ok(reply)
}

/// The battle's `supportUserInfo`: the supporter of the pending battle as a mutual friend (the
/// tactical battle applies its statue's leader ability), else `{}` (none).
fn support_info(p: &Profile) -> Value {
    p.progress
        .pending
        .as_ref()
        .and_then(|b| b.support.as_deref())
        .and_then(support::Supporter::parse)
        .map(|s| support::battle_support_info(&s, jdr_core::clock::now()))
        .unwrap_or_else(|| json!({}))
}

/// The battle description `bizarre_start` and `bizarre_resumption` share for a story, daily or
/// event stage: the areas, the chapter, the stage and the team.
fn battle_reply(
    p: &Profile,
    c: &quests::ChapterDef,
    s: &quests::StageDef,
    team: Option<&collection::Team>,
) -> Value {
    let team = team
        .map(|t| collection::team_info(&p.collection, t))
        .unwrap_or_else(|| json!({}));
    let n = c
        .stages
        .iter()
        .position(|x| x.stage_id == s.stage_id)
        .unwrap_or(0);
    json!({
        "userInfo": crate::services::user::user_info(p),
        "areaList": quests::area_list(c, s), "minigameInfo": {}, "supportUserInfo": support_info(p),
        "chapterInfo": quests::chapter_info(c, &p.progress),
        "teamInfo": team, "stageInfo": quests::stage_info(c, s, n, &p.progress),
        "continueCount": 0, "dungeonId": "", "floorNum": 0,
        "seed": (jdr_core::clock::now() & 0x7fff_ffff)
    })
}

/// The story, daily or event battle `bizarre_start` began and no `bizarre_end` settled.
fn suspended_story_battle(
    p: &Profile,
) -> Option<(&'static quests::ChapterDef, &'static quests::StageDef)> {
    let b = p.progress.pending.as_ref()?;
    if crate::labyrinth::find_room(&b.stage_id).is_some() {
        return None;
    }
    quests::find_stage(&b.stage_id)
}

/// `/bizarre/bizarre_resumption` (My Page, every load; docs/research/22 §1.8): status 10000 with
/// the suspended battle makes the client ask "Resume?" (Client_Mes_0009, or Client_Mes_0010 when
/// `dungeonId` is set); Yes replays it from this reply (`returnDestination` 1 Battle) or reopens the
/// labyrinth's card scene (0 TarotSelect), No sends `/bizarre/destruction`. 50301
/// (E_BIZALLE_NOT_START) when nothing is suspended. Suspended: a battle `bizarre_start` began and
/// no `bizarre_end` settled (story, daily, event or labyrinth room), or a labyrinth room paid for
/// whose Tarot pick was not made.
pub fn bizarre_resumption(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let Ok(p) = profile(ctx, env) else {
        return Err(ApiError::Status(E_NOT_START));
    };
    if let Some((c, s)) = suspended_story_battle(&p) {
        let team = p.progress.pending.as_ref().and_then(|b| {
            p.collection
                .teams
                .iter()
                .find(|t| t.stage_type == b.stage_type && t.team_num == b.team_num)
        });
        let mut reply = battle_reply(&p, c, s, team);
        for (k, v) in [
            ("returnDestination", json!(1)),
            ("selectTarotList", json!([])),
            ("usedTarotList", json!([])),
            ("isRaidDelete", json!(0)),
        ] {
            reply[k] = v;
        }
        log::line("bizarre", &format!("resumption offered: {}", s.stage_id));
        return ok(reply);
    }
    if let Some(reply) = dungeon::resumption(&p) {
        log::line(
            "dungeon",
            &format!(
                "resumption offered: {} ({})",
                reply["stageInfo"]["stageId"].as_str().unwrap_or(""),
                if reply["returnDestination"] == 1 {
                    "battle"
                } else {
                    "tarot select"
                }
            ),
        );
        return ok(reply);
    }
    Err(ApiError::Status(E_NOT_START))
}

/// E_BIZALLE_NOT_START: nothing to resume.
const E_NOT_START: i64 = 50301;

/// `BizarreEndResponse`: for the battle `bizarre_start` began, records a clear (result 0) and
/// grants gold, rank EXP and statue EXP to the team that fought, once (a resent request pays
/// Friend Points per quest clear (the wiki gives 10 for stage 1-1 without a friend; ours for all).
pub const FRIEND_POINTS_PER_CLEAR: i64 = 10;

/// nothing more).
pub fn bizarre_end(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let stage_id = env
        .param
        .get("stageId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let result = env.param.get("result").and_then(Value::as_i64);
    if result.is_none() {
        // BattleResult.Success is 0 (docs/research/10-content.md §3.9, medium confidence).
        log::line(
            "bizarre",
            "WARN bizarre_end without `result`; counted as a win",
        );
    }
    let won = result.unwrap_or(0) == 0;
    if dungeon::is_dungeon_battle(env, &stage_id) {
        return dungeon::bizarre_end(ctx, env, &stage_id, won);
    }
    let found = quests::find_stage(&stage_id);
    let event_id = found.and_then(|(c, _)| {
        quests::events()
            .iter()
            .find(|e| e.category == c.category)
            .map(|e| e.event_id.clone())
    });
    let is_event = event_id.is_some();
    let point_event = event_id
        .as_deref()
        .is_some_and(crate::services::event::is_point_event);
    let mut event_rewards: Vec<Value> = Vec::new();
    let mut support_points = 0;
    let (p, (paid, team)) = with_profile(ctx, env, |p| {
        let pending = p.progress.pending.take_if(|b| b.stage_id == stage_id);
        let supported = pending.as_ref().is_some_and(|b| b.support.is_some());
        let team = pending.as_ref().and_then(|b| {
            p.collection
                .teams
                .iter()
                .find(|t| t.stage_type == b.stage_type && t.team_num == b.team_num)
                .cloned()
        });
        match (won, found, pending.is_some()) {
            (true, Some((_, s)), true) => {
                p.progress.cleared.insert(stage_id.clone());
                // The client sends the missions it cleared ("1,2,10"); keep them for the stars.
                let done = env
                    .param
                    .get("starClearList")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let set = p.progress.stars.entry(stage_id.clone()).or_default();
                set.extend(
                    done.split(',')
                        .filter_map(|x| x.trim().parse::<i64>().ok())
                        .filter(|t| (1..=16).contains(t)),
                );
                p.gold += s.gold;
                p.friend_points += FRIEND_POINTS_PER_CLEAR;
                if supported {
                    support_points = support::SUPPORT_FRIEND_POINTS;
                    p.friend_points += support_points;
                }
                p.exp += s.gain_rank_exp;
                p.rank = collection::rank_for_exp(p.exp);
                let now = jdr_core::clock::now();
                let m = &mut p.progress.missions;
                m.bump("clear", 1, now);
                m.bump("exp", s.gain_rank_exp, now);
                m.bump("gold", s.gold, now);
                if found.is_some_and(|(c, _)| (6..=12).contains(&c.category)) {
                    m.bump("daily_quest", 1, now);
                }
                let uids = team.as_ref().map(|t| t.slots.to_vec()).unwrap_or_default();
                p.collection.add_statue_exp(&uids, s.gain_rank_exp);
                // Event quests drop the exchange currency (Build 22) or event points (Build 23).
                if point_event {
                    event_rewards = crate::services::event::add_points(
                        p,
                        event_id.as_deref().unwrap_or(""),
                        crate::services::event::POINTS_PER_CLEAR,
                        now,
                    );
                } else if is_event {
                    p.collection.add_item(
                        crate::items::EVENT_COIN,
                        crate::items::EVENT_COINS_PER_CLEAR,
                    );
                }
                ((Some(s), team), Change::Progress)
            }
            (_, _, true) => ((None, team), Change::Bookkeeping),
            _ => ((None, team), Change::None),
        }
    })?;
    if won && paid.is_none() {
        log::line(
            "bizarre",
            &format!(
                "bizarre_end for {stage_id:?} without a matching start: nothing paid (resent?)"
            ),
        );
    }
    let (gold, rank_exp) = paid.map(|s| (s.gold, s.gain_rank_exp)).unwrap_or((0, 0));
    let team = team
        .or_else(|| {
            found.and_then(|(c, _)| collection::main_team(&p.collection, c.stage_type).cloned())
        })
        .map(|t| collection::team_info(&p.collection, &t))
        .unwrap_or_else(|| json!({}));
    // eventType 1 (points) or 3 (Item: the event currency gained); never null.
    let event_result = if point_event && paid.is_some() {
        json!({ "eventType": 1,
                "userEventPointAll": crate::services::event::points(&p, event_id.as_deref().unwrap_or("")),
                "getUserEventPoint": crate::services::event::POINTS_PER_CLEAR,
                "eventRewardList": event_rewards, "getEventItemList": [] })
    } else if is_event && paid.is_some() {
        json!({ "eventType": 3, "userEventPointAll": 0, "getUserEventPoint": 0, "eventRewardList": [],
                "getEventItemList": [{ "itemId": crate::items::EVENT_COIN, "count": crate::items::EVENT_COINS_PER_CLEAR }] })
    } else {
        no_event()
    };
    end_response(
        &p,
        env,
        EndInfo {
            won,
            gold,
            rank_exp,
            team,
            event_result,
            clear: 0,
            gameover: 0,
            friend_points: if paid.is_some() {
                FRIEND_POINTS_PER_CLEAR + support_points
            } else {
                0
            },
        },
    )
}

/// What a `BizarreEndResponse` reports besides the echo of the request.
pub(crate) struct EndInfo {
    pub won: bool,
    pub gold: i64,
    pub rank_exp: i64,
    pub team: Value,
    pub event_result: Value,
    /// `dungeonClearFlag` / `dungeonGameoverFlag` (labyrinth battles only).
    pub clear: i64,
    pub gameover: i64,
    /// Friend Points this battle earned (`mindGashaPoint`).
    pub friend_points: i64,
}

/// `eventResult` of a battle outside an event (never null).
pub(crate) fn no_event() -> Value {
    json!({ "eventType": 0, "userEventPointAll": 0, "getUserEventPoint": 0, "eventRewardList": [], "getEventItemList": [] })
}

/// `BizarreEndResponse`, shared by story, event and labyrinth battles.
pub(crate) fn end_response(p: &Profile, env: &RequestEnvelope, e: EndInfo) -> ApiResult {
    let empty = || json!({ "itemSetIcon": "", "itemSetNum": 0, "statueList": [], "mindList": [], "itemList": [], "clanObjectList": [] });
    ok(json!({
        "clearRank": if e.won { 4 } else { 0 },
        "starClearList": env.param.get("starClearList").cloned().unwrap_or(json!("")),
        "userInfo": crate::services::user::user_info(p),
        "rewardList": empty(), "dropList": empty(), "firstRewardList": empty(), "starRewardList": empty(),
        "newRecordFlg": 0, "teamInfo": e.team, "gold": e.gold, "rankExp": e.rank_exp, "levelExp": e.rank_exp,
        "mindGashaPoint": e.friend_points, "supportUserInfo": {},
        "eventResult": e.event_result,
        "dungeonClearFlag": e.clear, "dungeonGameoverFlag": e.gameover, "nowEventRankingPoint": 0,
        // The result screen's high-score panel reads eventDataList.Count without a null check
        // (BattleResultHighScoreUIController.Setup 0xC0D4B4); an empty list shows no panel.
        "oldEventRankingResultInfo": { "point": 0, "eventDataList": [] },
        "raidStageInfo": {}, "raidBossInfo": {}
    }))
}

/// `BizarreContinueResponse`: continuing is free offline.
pub fn bizarre_continue(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({ "userInfo": crate::services::user::user_info(&p), "continueCount": 1 }))
}

/// `/bizarre/destruction {stageType}`: sent only when the player answers No to the resume dialog
/// (`DestroyBizarreResumption` 0xC37A48, its one caller). A suspended story battle is dropped
/// (nothing paid); a labyrinth run's suspended battle counts as retired and its Tarot pick is not
/// offered again (`dungeon::decline`).
pub fn destruction(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    with_profile(ctx, env, |p| {
        if suspended_story_battle(p).is_some() {
            p.progress.pending = None;
            return ((), Change::Bookkeeping);
        }
        if dungeon::decline(p) {
            return ((), Change::Progress);
        }
        ((), Change::None)
    })?;
    ok(json!({}))
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
            date: 20261115103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    /// AI supporters (services/support.rs): the list walked against SupportUserResponse; a pick
    /// starts the battle with the supporter as a mutual friend (its whole statue, walked against
    /// BizarreStartResponse) and pays extra Friend Points on the clear.
    #[test]
    fn an_ai_supporter_joins_a_quest() {
        use crate::services::shapes::assert_complete;
        let dir = std::env::temp_dir().join(format!("jdr-support-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| {
            Value::Object(dispatch(&ctx, path, &env(&uuid, param)).0.unwrap())
        };
        let l = call(
            "/support/support_user_s",
            json!({"dummyFlg": 0, "stageType": 1}),
        );
        assert_complete("Jjs.SupportUserResponse", &l);
        let list = l["supportUserList"].as_array().unwrap();
        assert_eq!(list.len(), crate::services::support::SUPPORTERS);
        let pick = &list[0]["supportUserInfo"];
        assert_eq!(
            pick["followStatus"], 3,
            "the tactical battle uses FollowEach only"
        );
        assert!(pick["dtLeaderAbilityInfo"]["abilityId"].is_string());
        let c = crate::quests::chapters()
            .iter()
            .find(|c| c.stage_type == 1)
            .unwrap();
        let sid = c.stages[0].stage_id.clone();
        let st = call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1, "supportId": pick["id"]}),
        );
        assert_complete("Jjs.BizarreStartResponse", &st);
        let sup = &st["supportUserInfo"];
        assert_eq!(sup["id"], pick["id"]);
        assert_eq!(sup["followStatus"], 3);
        assert_eq!(
            sup["statueInfo"]["statueId"],
            pick["statueInfoSupport"]["statueId"]
        );
        assert_eq!(
            sup["statueInfo"]["dtLeaderAbilityInfo"]["abilityId"],
            pick["dtLeaderAbilityInfo"]["abilityId"]
        );
        let fp0 = ctx.accounts.get(&uuid).unwrap().unwrap().friend_points;
        let end = call(
            "/bizarre/bizarre_end",
            json!({"stageId": sid, "result": 0, "starClearList": ""}),
        );
        let paid = super::FRIEND_POINTS_PER_CLEAR + crate::services::support::SUPPORT_FRIEND_POINTS;
        assert_eq!(end["mindGashaPoint"], paid);
        assert_eq!(
            ctx.accounts.get(&uuid).unwrap().unwrap().friend_points,
            fp0 + paid
        );
        // Without a pick (or with an id we never handed out) the battle has no support.
        let st = call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1, "supportId": "someone"}),
        );
        assert_eq!(st["supportUserInfo"], json!({}));
        assert_complete("Jjs.BizarreStartResponse", &st);
    }

    #[test]
    fn a_win_records_the_missions_the_client_cleared() {
        let dir = std::env::temp_dir().join(format!("jdr-bizarre-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let c = crate::quests::chapters()
            .iter()
            .find(|c| c.stage_type == 1)
            .unwrap();
        let sid = c.stages[0].stage_id.clone();
        let (r, _) = dispatch(
            &ctx,
            "/bizarre/bizarre_start",
            &env(&uuid, json!({"stageId": sid, "teamNo": 1})),
        );
        let r = r.unwrap();
        let stars = r["stageInfo"]["starList"].as_array().unwrap().clone();
        assert_eq!(stars.len(), 3);
        assert!(r["stageInfo"]["questTips"]["tipsTitleText"]
            .as_str()
            .unwrap()
            .starts_with("TipsTitle_"));
        let (r, _) = dispatch(
            &ctx,
            "/bizarre/bizarre_end",
            &env(
                &uuid,
                json!({"stageId": sid, "result": 0, "starClearList": "1,10,99"}),
            ),
        );
        assert_eq!(
            r.unwrap()["starClearList"],
            "1,10,99",
            "echoed for the result screen"
        );
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let done: Vec<i64> = p.progress.stars[&sid].iter().copied().collect();
        assert_eq!(done, [1, 10], "only mission types 1..16 are kept");
    }

    fn no_nulls(v: &Value) -> bool {
        match v {
            Value::Null => false,
            Value::Array(a) => a.iter().all(no_nulls),
            Value::Object(o) => o.values().all(no_nulls),
            _ => true,
        }
    }

    #[test]
    fn daily_event_and_campaign_tiles_open_their_chapters() {
        let dir = std::env::temp_dir().join(format!("jdr-select-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let (r, _) = dispatch(&ctx, "/bizarre/get_bizarre_select", &env(&uuid, json!({})));
        let r = r.unwrap();
        assert!(
            no_nulls(&Value::Object(r.clone())),
            "a null string crashes the screen"
        );
        let tiles = r["questList"].as_array().unwrap();
        let kinds: Vec<i64> = tiles
            .iter()
            .map(|q| q["bannerType"].as_i64().unwrap())
            .collect();
        assert_eq!(
            kinds.iter().filter(|k| **k == 2).count(),
            1,
            "one daily tile (today)"
        );
        assert!(kinds.contains(&1) && kinds.iter().filter(|k| **k == 3).count() >= 3);
        let camps = r["eventCampaignList"].as_array().unwrap();
        assert!(
            !camps.is_empty()
                && camps.iter().all(|c| c["bizarreHomeFlag"] == true
                    && c["htmlLink"].as_str().unwrap().starts_with('/'))
        );
        let tr = crate::l10n::Translations::embedded().unwrap();
        let label = |l: &str| {
            tr.english(&format!("text:ChapterName_JP:{l}")).is_some()
                || tr.english(&format!("text:StageText_JP:{l}")).is_some()
                || tr
                    .english(&format!("text:EventChapterName_JP:{l}"))
                    .is_some()
        };
        for q in tiles.iter().filter(|q| q["bannerType"] != 1) {
            let cat = q["category"].as_i64().unwrap();
            assert!(q["bannerFileName"].as_str().unwrap().len() > 3);
            let (l, _) = dispatch(
                &ctx,
                "/bizarre/bizarre_list",
                &env(&uuid, json!({"category": cat})),
            );
            let l = l.unwrap();
            let chapters = l["chapterList"].as_array().unwrap();
            assert!(
                !chapters.is_empty(),
                "an empty list shows an expired dialog ({cat})"
            );
            assert!(chapters.iter().all(|c| c["category"] == cat));
            if q["bannerType"] == 3 {
                assert_eq!(l["eventInfo"]["eventCampaignId"], q["eventMasterId"]);
                // Exchange events: shop flag 1 and collect type 2; point events: flag 0 and type 1.
                // Shop3 copies eventItemList, so it is always a list.
                let point = l["eventInfo"]["eventShopFlg"] == 0;
                assert_eq!(
                    l["eventInfo"]["eventCollectType"],
                    if point { 1 } else { 2 }
                );
                assert!(l["eventInfo"]["eventItemList"].is_array());
                assert!(
                    cat >= 13,
                    "event chapters return to Event Home only from 13 up"
                );
            }
            for c in chapters {
                assert!(label(c["name"].as_str().unwrap()), "{}", c["name"]);
                for s in c["stageList"].as_array().unwrap() {
                    assert!(label(s["stageId"].as_str().unwrap()), "{}", s["stageId"]);
                }
            }
            // The first stage plays like a story stage.
            let sid = chapters[0]["stageList"][0]["stageId"]
                .as_str()
                .unwrap()
                .to_string();
            let (s, _) = dispatch(
                &ctx,
                "/bizarre/bizarre_start",
                &env(&uuid, json!({"stageId": sid, "teamNo": 1})),
            );
            assert_eq!(
                s.unwrap()["stageInfo"]["starList"]
                    .as_array()
                    .unwrap()
                    .len(),
                3
            );
            let (e, _) = dispatch(
                &ctx,
                "/bizarre/bizarre_end",
                &env(
                    &uuid,
                    json!({"stageId": sid, "result": 0, "starClearList": "1"}),
                ),
            );
            assert!(e.is_ok());
        }
        // Story still lists only story chapters.
        let (l, _) = dispatch(
            &ctx,
            "/bizarre/bizarre_list",
            &env(&uuid, json!({"category": 1})),
        );
        assert!(l.unwrap()["chapterList"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["category"] == 1));
    }

    #[test]
    fn missions_count_clears_and_pay_out() {
        let dir = std::env::temp_dir().join(format!("jdr-missions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let list = call("/trophy/trophy_list", json!({"allFlag": 0})).unwrap();
        let rows = list["trophyList"].as_array().unwrap().clone();
        assert!(!rows.is_empty());
        for r in &rows {
            // The row drawing throws on a missing list or an unparseable end date.
            for k in ["statueList", "mindList", "itemList"] {
                assert!(r["itemSetInfo"][k].is_array(), "{r}");
            }
            assert!(r["acEnd"].as_str().unwrap().len() == 19);
            let day = r["day"].as_i64().unwrap();
            assert!((0..=7).contains(&day));
        }
        let days: std::collections::BTreeSet<i64> = rows
            .iter()
            .map(|r| r["day"].as_i64().unwrap())
            .filter(|d| *d > 0)
            .collect();
        assert_eq!(days.len(), 1, "only today's dailies");
        let status = |l: &serde_json::Map<String, Value>, id: &str| {
            l["trophyList"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["achievementId"] == id)
                .unwrap()["status"]
                .as_i64()
                .unwrap()
        };
        assert_eq!(status(&list, "daily_0001"), 0);
        // Win a stage: a clear, its EXP and gold count.
        let c = crate::quests::chapters()
            .iter()
            .find(|c| c.stage_type == 1)
            .unwrap();
        let sid = c.stages[0].stage_id.clone();
        call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1}),
        )
        .unwrap();
        call(
            "/bizarre/bizarre_end",
            json!({"stageId": sid, "result": 0, "starClearList": ""}),
        )
        .unwrap();
        let list = call("/trophy/trophy_list", json!({"allFlag": 0})).unwrap();
        assert_eq!(status(&list, "daily_0001"), 1);
        assert_eq!(status(&list, "daily_0002"), 0);
        let badge = call("/info/get_menu_notify", json!({})).unwrap()["unopenedAchievement"]
            .as_i64()
            .unwrap();
        assert!(badge >= 1);
        // Claim one: status 2, the reward recorded; claiming it again pays nothing.
        let r = call(
            "/trophy/trophy_result_get",
            json!({"achievementId": "daily_0001"}),
        )
        .unwrap();
        assert_eq!(status(&r, "daily_0001"), 2);
        assert_eq!(r["popupFlg"], false);
        let r2 = call(
            "/trophy/trophy_result_get",
            json!({"achievementId": "daily_0001"}),
        )
        .unwrap();
        assert_eq!(r2["itemSetInfo"]["itemList"].as_array().unwrap().len(), 0);
        // Claim all: nothing claimable is left.
        let all = call("/trophy/get_all_trophy_result", json!({})).unwrap();
        assert!(all["trophyList"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["status"] != 1));
        assert_eq!(
            call("/info/get_menu_notify", json!({})).unwrap()["unopenedAchievement"],
            0
        );
    }
}
