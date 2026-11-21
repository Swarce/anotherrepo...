//! Small boot-path answers: system config, shop product list, info/notify, team.
//! Values here are Build 01 placeholders; M4/M5 move them to server/content.

use crate::app::ApiResult;
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

/// `EventListResponse` (My Page): the always-open Arena campaign (services/arena.rs). My Page
/// shows the Arena button for the first entry of eventType 6/7 and parses its two dates and
/// `userBattleInfo.fieverEndTime` (client, docs/research/23-arena.md), so all three are dates.
pub fn event_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let now = jdr_core::clock::now();
    let fever = super::statue::profile(ctx, env)
        .map(|p| super::arena::fever_end_time(&p.progress.arena, now))
        .unwrap_or_else(|_| super::arena::fever_end_time(&Default::default(), now));
    ok(json!({ "eventList": [super::arena::campaign_entry()],
               "userBattleInfo": { "fieverEndTime": fever },
               "raidInfo": super::raid::raid_info() }))
}

pub fn banner_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "bannerList": [] }))
}

pub fn get_menu_notify(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    // The Missions badge: claimable missions.
    let p = super::statue::profile(ctx, env)?;
    let mut m = p.progress.missions.clone();
    m.roll(jdr_core::clock::now());
    // Town badges: members (the AI members are always around), unread chat, applicants waiting
    // for the Master or a Sub-Master (docs/research/24-town.md §1.1).
    let town = p.progress.town.town.as_ref();
    let members = town.map(|t| t.member_count()).unwrap_or(0);
    let unread = town.is_some_and(|t| t.unread_chat());
    let waiting = town.is_some_and(|t| t.can_manage(&p.user_id) && !t.applicants.is_empty());
    ok(json!({
        // Notices: this build's notes not opened yet (services/menu.rs notify_list).
        "notAllowedFunction": [],
        "unopenedNotify": i64::from(p.progress.notes_seen_build != jdr_core::BUILD),
        "unopenedPresent": p.progress.presents.waiting(),
        "unopenedAchievement": m.claimable(p.rank),
        "unopenedFollower": 0, "gashaPlayNum": 0, "myTownMember": members,
        "unreadChatFlg": i64::from(unread), "unapprovedMemberFlg": i64::from(waiting)
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
