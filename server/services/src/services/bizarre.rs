//! `/bizarre/*` and `/support/support_user_s`: the story quest flow.

use crate::accounts::Change;
use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::quests::{self, PendingBattle, STORY_CATEGORY};
use crate::services::ok;
use crate::services::statue::{profile, with_profile};
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use jdr_core::log;
use serde_json::{json, Value};

/// `BizarreSelectResponse`: the story button (daily and event categories come later).
pub fn get_bizarre_select(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(
        json!({ "questList": [quests::story_quest_info()], "eventCampaignList": [], "dungeonLockFlg": 1 }),
    )
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
    let list: Vec<Value> = if category == STORY_CATEGORY {
        quests::chapters()
            .iter()
            .map(|c| quests::chapter_info(c, &p.progress))
            .collect()
    } else {
        Vec::new()
    };
    ok(json!({ "chapterList": list, "eventInfo": {}, "raidInfo": {} }))
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

/// `SupportUserResponse`: no other players offline.
pub fn support_user_s(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "supportUserList": [] }))
}

/// `BizarreStartResponse`: the battle description for the chosen stage and team.
pub fn bizarre_start(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let stage_id = env
        .param
        .get("stageId")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (c, s) = quests::find_stage(stage_id)
        .ok_or_else(|| ApiError::Internal(format!("unknown stage {stage_id:?}")))?;
    let team_no = env.param.get("teamNo").and_then(Value::as_i64).unwrap_or(1);
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
        });
        (team, Change::Bookkeeping)
    })?;
    let team = team
        .map(|t| collection::team_info(&p.collection, &t))
        .unwrap_or_else(|| json!({}));
    let n = c
        .stages
        .iter()
        .position(|x| x.stage_id == stage_id)
        .unwrap_or(0);
    ok(json!({
        "userInfo": crate::services::user::user_info(&p),
        "areaList": quests::area_list(c, s), "minigameInfo": {}, "supportUserInfo": {},
        "chapterInfo": quests::chapter_info(c, &p.progress),
        "teamInfo": team, "stageInfo": quests::stage_info(c, s, n, &p.progress),
        "continueCount": 0, "tarotInfo": {}, "dungeonId": "", "floorNum": 0,
        "seed": (jdr_core::clock::now() & 0x7fff_ffff)
    }))
}

/// `BizarreEndResponse`: for the battle `bizarre_start` began, records a clear (result 0) and
/// grants gold, rank EXP and statue EXP to the team that fought, once (a resent request pays
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
    let found = quests::find_stage(&stage_id);
    let (p, (paid, team)) = with_profile(ctx, env, |p| {
        let pending = p.progress.pending.take_if(|b| b.stage_id == stage_id);
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
                p.gold += s.gold;
                p.exp += s.gain_rank_exp;
                p.rank = collection::rank_for_exp(p.exp);
                let uids = team.as_ref().map(|t| t.slots.to_vec()).unwrap_or_default();
                p.collection.add_statue_exp(&uids, s.gain_rank_exp);
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
    let empty = || json!({ "itemSetIcon": "", "itemSetNum": 0, "statueList": [], "mindList": [], "itemList": [], "clanObjectList": [] });
    ok(json!({
        "clearRank": if won { 4 } else { 0 },
        "starClearList": env.param.get("starClearList").cloned().unwrap_or(json!("")),
        "userInfo": crate::services::user::user_info(&p),
        "rewardList": empty(), "dropList": empty(), "firstRewardList": empty(), "starRewardList": empty(),
        "newRecordFlg": 0, "teamInfo": team, "gold": gold, "rankExp": rank_exp, "levelExp": rank_exp,
        "mindGashaPoint": 0, "supportUserInfo": {},
        "eventResult": { "eventType": 0, "userEventPointAll": 0, "getUserEventPoint": 0, "eventRewardList": [], "getEventItemList": [] },
        "dungeonClearFlag": 0, "dungeonGameoverFlag": 0, "nowEventRankingPoint": 0
    }))
}

/// `BizarreContinueResponse`: continuing is free offline.
pub fn bizarre_continue(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({ "userInfo": crate::services::user::user_info(&p), "continueCount": 1 }))
}

/// `BizarreDestructionResponse`: abandoning a stage needs no bookkeeping.
pub fn destruction(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({}))
}
