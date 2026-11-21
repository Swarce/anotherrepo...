//! Small boot-path answers: system config, shop product list, info/notify, team.
//! Values here are Build 01 placeholders; M4/M5 move them to server/content.

use crate::app::ApiResult;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// Action-battle assist stat bonus (docs/research/19-assists.md §3.1). The client reads these
/// four `ConfigInfo` fields in `AssistStatueAddParamCalculator.CalcBonusParam` (0xC93510) and
/// computes `CalcMagnification` (0xC936C4):
/// `(add + min(convergence / count, limit) * step) / 1000.0` (the divisor is the class's static
/// float, set to 1000.0 by its `.cctor` at 0xC93E68), then `FloorToInt(stat * magnification)` of
/// the assist's own HP/ATK/DEF, summed over the seat's assists. The values are community-sourced:
/// Gamerch player posts 101615739 (2017-11-24), 101635727 (2017-11-27) and 104155773
/// (2019-08-20): 10% base, +2% per 5 boosts, 100% at 225 boosts. The original server's
/// values are not recovered.
pub const ASSIST_ADD_PERCENT: i32 = 100;
/// Boosts (`convergence`) per step.
pub const ASSIST_CONVERGENCE_COUNT: i32 = 5;
/// Per-mille added per step.
pub const ASSIST_CONVERGENCE_PERCENT: i32 = 20;
/// Most steps counted.
pub const ASSIST_CONVERGENCE_LIMIT: i32 = 45;

/// The client's `CalcMagnification` with our constants, in the client's own arithmetic (integer
/// steps, a count below 1 gives no steps, f32 division).
pub fn assist_magnification(convergence: i32) -> f32 {
    let steps = if ASSIST_CONVERGENCE_COUNT >= 1 {
        convergence / ASSIST_CONVERGENCE_COUNT
    } else {
        0
    };
    let per_mille =
        ASSIST_ADD_PERCENT + steps.min(ASSIST_CONVERGENCE_LIMIT) * ASSIST_CONVERGENCE_PERCENT;
    per_mille as f32 / 1000.0
}

/// One assist's HP/ATK/DEF share in the action battle (`CalcAddHp`/`Atk`/`Def`, 0xC937A8,
/// 0xC93820, 0xC93898: `FloorToInt(stat * magnification)` in f32).
pub fn assist_transfer(stat: i32, convergence: i32) -> i32 {
    (stat as f32 * assist_magnification(convergence)).floor() as i32
}

/// `ConfigInfo` (29 ints). Recovery times are divisors in the client, so never 0.
pub fn get_system_config(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "configList": {
        "statueGrowthRare1Gold": 100, "statueGrowthRare2Gold": 200, "statueGrowthRare3Gold": 300,
        "statueGrowthRare4Gold": 400, "statueGrowthBoostRare3Gold": 300, "statueGrowthBoostRare4Gold": 400,
        "statueGrowthBoostRare5Gold": 500, "statueGrowthBoostRare6Gold": 600, "statueGrowthBoostGoldLimit": 9999999,
        "mindGrowthRare1Gold": 100, "mindGrowthRare2Gold": 200, "mindGrowthRare3Gold": 300, "mindGrowthRare4Gold": 400,
        "playerLimitMaxMind": 999, "apOneRecoveryTime": 180, "bpOneRecoveryTime": 1800, "rpOneRecoveryTime": 1800,
        "bpMaxFatiguDown": 1, "assistStatusAddPercent": ASSIST_ADD_PERCENT,
        "assistStatusConvergenceCount": ASSIST_CONVERGENCE_COUNT,
        "assistStatusConvergencePercent": ASSIST_CONVERGENCE_PERCENT,
        "assistStatusConvergenceLimitCount": ASSIST_CONVERGENCE_LIMIT,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::dispatch;
    use jdr_core::settings::Settings;
    use serde_json::json;

    fn env() -> RequestEnvelope {
        RequestEnvelope {
            uuid: String::new(),
            session: String::new(),
            language: 1,
            date: 20261121103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param: json!({}),
        }
    }

    /// The system config carries the action assist constants the client reads (19 §3.1).
    #[test]
    fn system_config_sends_the_assist_constants() {
        let dir = std::env::temp_dir().join(format!("jdr-sysconf-{}", std::process::id()));
        let ctx = Ctx::new(Settings::new(dir), false);
        let (r, known) = dispatch(&ctx, "/common/get_system_config", &env());
        assert!(known);
        let c = Value::Object(r.unwrap())["configList"].clone();
        assert_eq!(c["assistStatusAddPercent"], 100);
        assert_eq!(c["assistStatusConvergenceCount"], 5);
        assert_eq!(c["assistStatusConvergencePercent"], 20);
        assert_eq!(c["assistStatusConvergenceLimitCount"], 45);
        // ConfigInfo has 29 ints; every one is sent.
        assert_eq!(c.as_object().unwrap().len(), 29);
    }

    /// 10% unboosted, +2% per 5 boosts (partial steps count nothing), 100% from 225 boosts on.
    #[test]
    fn assist_share_grows_with_boosts_and_caps_at_225() {
        assert_eq!(assist_magnification(0), 0.1);
        assert_eq!(assist_magnification(4), 0.1);
        assert_eq!(assist_magnification(5), 0.12);
        assert_eq!(assist_magnification(224), assist_magnification(220));
        assert_eq!(assist_magnification(225), 1.0);
        assert_eq!(assist_magnification(999), 1.0);
        // The share is floored per stat, like the client's FloorToInt.
        assert_eq!(assist_transfer(12345, 225), 12345);
        assert_eq!(assist_transfer(12345, 0), 1234);
        assert_eq!(assist_transfer(999, 5), 119);
    }
}
