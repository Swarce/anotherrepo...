//! Small boot-path answers: system config, shop product list, info/notify, team, resumption.
//! Values here are Build 01 placeholders; M4/M5 move them to server/content.

use crate::app::{ApiError, ApiResult};
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// `ConfigInfo` (29 ints). Recovery times are divisors in the client, so never 0.
pub fn get_system_config(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "configList": {
        "statueGrowthRare1Gold": 100, "statueGrowthRare2Gold": 200, "statueGrowthRare3Gold": 300,
        "statueGrowthRare4Gold": 400, "statueGrowthBoostRare3Gold": 300, "statueGrowthBoostRare4Gold": 400,
        "statueGrowthBoostRare5Gold": 500, "statueGrowthBoostRare6Gold": 600, "statueGrowthBoostGoldLimit": 9999999,
        "mindGrowthRare1Gold": 100, "mindGrowthRare2Gold": 200, "mindGrowthRare3Gold": 300, "mindGrowthRare4Gold": 400,
        "playerLimitMaxMind": 999, "apOneRecoveryTime": 180, "bpOneRecoveryTime": 1800, "rpOneRecoveryTime": 1800,
        "bpMaxFatiguDown": 1, "assistStatusAddPercent": 10, "assistStatusConvergenceCount": 5,
        "assistStatusConvergencePercent": 50, "assistStatusConvergenceLimitCount": 10,
        "pvpTeamCharaChangeIntervalTime": 10, "characteristicLevelUpperLimit": 10,
        "evolutionPossibleFlagRare3": 1, "evolutionPossibleFlagRare4": 1, "evolutionPossibleFlagRare5": 1,
        "evolutionPossibleFlagRare6": 1, "gameStartStamina0Rank": 1
    }}))
}

/// Empty `key` keeps in-app purchases uninitialised (no store contact).
pub fn product_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "key": "", "productId": [] }))
}

pub fn check_forced_notify(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "category": 0 }))
}

pub fn event_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "eventList": [], "userBattleInfo": {}, "raidInfo": { "totalRescueCount": 0 } }))
}

pub fn banner_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "bannerList": [] }))
}

pub fn get_menu_notify(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    // The Missions badge: claimable missions.
    let p = super::statue::profile(ctx, env)?;
    let mut m = p.progress.missions.clone();
    m.roll(jdr_core::clock::now());
    ok(json!({
        "notAllowedFunction": [], "unopenedNotify": 0, "unopenedPresent": 0,
        "unopenedAchievement": m.claimable(p.rank),
        "unopenedFollower": 0, "gashaPlayNum": 0, "myTownMember": 0, "unreadChatFlg": 0, "unapprovedMemberFlg": 0
    }))
}

/// No team yet: MyPage skips the leader model when no main team exists (statues arrive with M1.5).
pub fn team_info(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = super::statue::profile(ctx, env)?;
    let teams: Vec<Value> = p
        .collection
        .teams
        .iter()
        .map(|t| crate::collection::team_info(&p.collection, t))
        .collect();
    ok(json!({ "teamList": teams, "mindSeqInfo": super::team::mind_seq_info() }))
}

/// 50301 = E_BIZALLE_NOT_START: no interrupted battle (10000 would offer to resume one).
pub fn bizarre_resumption(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    Err(ApiError::Status(50301))
}
