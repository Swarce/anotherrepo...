//! `/bizarre/*` and `/support/support_user_s`: the story quest flow.

use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::quests::{self, STORY_CATEGORY};
use crate::services::ok;
use crate::services::statue::profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
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
    let mut p = profile(ctx, env)?;
    let stage_id = env
        .param
        .get("stageId")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (c, s) = quests::find_stage(stage_id)
        .ok_or_else(|| ApiError::Internal(format!("unknown stage {stage_id:?}")))?;
    let team_no = env.param.get("teamNo").and_then(Value::as_i64).unwrap_or(1);
    let team = quests::team_for(&p.collection, c.stage_type, team_no)
        .map(|t| collection::team_info(&p.collection, t))
        .unwrap_or_else(|| json!({}));
    *p.progress
        .play_counts
        .entry(stage_id.to_string())
        .or_insert(0) += 1;
    ctx.accounts.save(&p).map_err(ApiError::Internal)?;
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

/// `BizarreEndResponse`: records a clear (result 0) and grants gold and rank EXP.
pub fn bizarre_end(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let mut p = profile(ctx, env)?;
    let stage_id = env
        .param
        .get("stageId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let won = env.param.get("result").and_then(Value::as_i64).unwrap_or(0) == 0;
    let found = quests::find_stage(&stage_id);
    let (gold, rank_exp) = match (won, found) {
        (true, Some((_, s))) => (s.gold, s.gain_rank_exp),
        _ => (0, 0),
    };
    if won && found.is_some() {
        p.progress.cleared.insert(stage_id.clone());
        p.gold += gold;
        p.exp += rank_exp;
    }
    ctx.accounts.save(&p).map_err(ApiError::Internal)?;
    let team = found
        .and_then(|(c, _)| collection::main_team(&p.collection, c.stage_type))
        .map(|t| collection::team_info(&p.collection, t))
        .unwrap_or_else(|| json!({}));
    let empty = || json!({ "itemSetIcon": "", "itemSetNum": 0, "statueList": [], "mindList": [], "itemList": [], "clanObjectList": [] });
    ok(json!({
        "clearRank": if won { 4 } else { 0 },
        "starClearList": env.param.get("starClearList").cloned().unwrap_or(json!("")),
        "userInfo": crate::services::user::user_info(&p),
        "rewardList": empty(), "dropList": empty(), "firstRewardList": empty(), "starRewardList": empty(),
        "newRecordFlg": 0, "teamInfo": team, "gold": gold, "rankExp": rank_exp, "levelExp": 0,
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
