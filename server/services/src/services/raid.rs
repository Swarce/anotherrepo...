//! Raids (Elite Raid boss battles, `/raid/*`; docs/research/25-events-raids.md §1).
//!
//! Client facts (3.12.0, H):
//! - A raid boss "appears" after a tactical quest: `BattleSceneController.OnResult` checks
//!   `bizarre_end.raidStageInfo.IsValid()` (`raidStageUid != 0`); if so it loads the scenes
//!   `RaidStartsCutINUI` and `RaidBattleResultUI` instead of ending the battle, and the fight
//!   (`/raid/raid_start`, `RaidBattleSceneController`, `/raid/raid_end`) and the rescue call
//!   (`/raid/raid_offer_rescue`) start only from that result screen.
//! - **Neither scene is in the player's files**: the 18 scene bundles and the 23 built-in scenes
//!   hold no raid scene. `/raid/raid_list`, `/raid/raid_history_list` and
//!   `/raid/get_raid_reward` have no caller at all, and Event Home's raid panel
//!   (`UpdateUiRaidBoss`) is never called; My Page does not read `event_list.raidInfo`.
//!
//! So a raid cannot be reached with these files: the server never makes one appear (every
//! `raidStageInfo` it sends has `raidStageUid 0`) and answers the six routes with an inactive,
//! complete raid (no null strings or lists) in case a client calls one: no boss, the start refused
//! as timed out, the end a withdrawal, the rescue call failed.

use crate::app::ApiResult;
use crate::services::ok;
use crate::services::statue::profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// RaidStartState.Timeout, RaidResultType.Withdraw, RaidRescueType.FailedForEscape,
/// RaidBossStatusType.Unknown (client enums).
pub const START_TIMEOUT: i64 = 2;
pub const RESULT_WITHDRAW: i64 = 0;
pub const RESCUE_FAILED_ESCAPE: i64 = 2;
pub const BOSS_UNKNOWN: i64 = -1;

/// A RaidStageInfo (a StageInfo plus the raid's fields) that is not a raid: `raidStageUid 0`
/// (RaidStageInfo.IsValid false), every string and list present.
pub fn inactive_stage() -> Value {
    let k = || json!({ "rate": 1.0, "add": 0 });
    json!({
        "stageId": "", "name": "", "stageNo": 0, "stageDifficulty": 0, "type": 0, "text": "",
        "startTalkId": "", "endTalkId": "", "startConditionItemList": [], "startConditionList": [],
        "startConditionFlg": true, "startConditionText": "", "areaMax": 0, "useMentality": 0,
        "openSetting": 0, "nextStageIdList": [], "starList": [], "rewardItemSet": [],
        "reward3StarsItemSet": [], "steadyItemSet": [], "bossDropItemSet": [], "useStamina": 0,
        "gainRankExp": 0, "mindGashaPoint": 0, "playCount": 0, "clearFlg": 0, "clearCount": 0,
        "clearRank": 0, "specialRewardFlg": 0,
        "questTips": { "tipsId": "", "tipsType": 0, "tipsTitleText": "", "tipsText": "", "tipsPicture": "" },
        "meisyoId": 0, "limitTime": 0, "endureTime": 0, "enemyList": [],
        "staminaCoefficientList": k(), "expCoefficientList": k(), "goldCoefficientList": k(),
        "bizarreEffectList": [], "limitTurn": 0, "endureTurn": 0, "eventPointSettingList": [],
        "dayNight": 0, "clearStarList": [], "useStatueCount": 3, "highScore": -1,
        "cannotContinue": 0, "presetTeamId": "",
        "raidStageUid": 0, "stageName": "", "useRp": 0, "useFullRp": 0, "stageRare": 0, "category": 0
    })
}

/// A RaidBossInfo for "no boss".
pub fn no_boss() -> Value {
    json!({
        "statueId": "", "name": "", "statueType": 0, "maxHp": 0, "hp": 0, "level": 0,
        "appearanceTime": 0, "damagePointRate": 0,
        "rescueStatus": { "status": 0, "discoveryUser": 0, "userName": "", "isFollower": 0, "isAll": 0,
                          "follwerCount": 0, "allCount": 0 },
        "raidBossStatus": BOSS_UNKNOWN
    })
}

/// `raidInfo` of `/info/event_list` and `/bizarre/bizarre_list`: no raid.
pub fn raid_info() -> Value {
    json!({ "raidBossInfo": no_boss(), "raidStageInfo": inactive_stage(), "totalRescueCount": 0 })
}

fn empty_set() -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 0, "statueList": [], "mindList": [], "itemList": [], "clanObjectList": [] })
}

/// `/raid/raid_list {eventCampaignId}`: no bosses.
pub fn raid_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({
        "userInfo": crate::services::user::user_info(&p), "teamInfoList": [], "raidBossList": [],
        "eventPoint": 0, "rpRecoveryItemList": [], "eventBgm": ""
    }))
}

/// `/raid/raid_start {stageId, raidStageUid, teamNo, sallyStatus, payType}`: refused (timed out;
/// the client shows its notice and goes back).
pub fn raid_start(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let chapter = crate::quests::chapters()
        .first()
        .map(|c| crate::quests::chapter_info(c, &p.progress))
        .unwrap_or_else(|| json!({}));
    let team = crate::collection::main_team(&p.collection, 1)
        .map(|t| crate::collection::team_info(&p.collection, t))
        .unwrap_or_else(|| json!({}));
    ok(json!({
        "userInfo": crate::services::user::user_info(&p), "enableRaidStart": START_TIMEOUT,
        "areaList": [], "chapterInfo": chapter, "teamInfo": team, "seed": 0,
        "raidStageInfo": inactive_stage(), "raidBossInfo": no_boss(), "sallyAbilityList": []
    }))
}

/// `/raid/raid_end {result, defeatedEnemyList, dtDropList, totalDamage}`: a withdrawal, nothing
/// paid.
pub fn raid_end(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({
        "userInfo": crate::services::user::user_info(&p), "clearResult": RESULT_WITHDRAW,
        "raidDropList": [],
        "raidEventResult": { "userEventPointAll": 0, "getUserEventPoint": 0, "damage": 0,
                             "eventRewardList": [], "subjugateEventPoint": 0,
                             "subjugateRewardList": empty_set() },
        "raidStageInfo": inactive_stage(), "raidBossInfo": no_boss(), "rpRecoveryItemList": []
    }))
}

/// `/raid/raid_offer_rescue {offerType}`: nobody to ask offline.
pub fn raid_offer_rescue(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "result": RESCUE_FAILED_ESCAPE }))
}

/// `/raid/raid_history_list {nowDate}`: no raids fought.
pub fn raid_history_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    let (y, mo, d, h, mi, s) = jdr_core::clock::civil(jdr_core::clock::now());
    ok(json!({
        "raidHistoryList": [],
        "nowDate": format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
    }))
}

/// `/raid/get_raid_reward {raidStageUidList}`: nothing to collect.
pub fn get_raid_reward(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(
        json!({ "raidRewardInfo": { "eventPointAll": 0, "getEventPoint": 0, "eventRewardList": [] } }),
    )
}

#[cfg(test)]
mod tests {
    use crate::app::ApiError;
    use crate::services::dispatch;
    use crate::services::shapes::assert_complete;
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

    /// Every raid route answers a complete, inactive raid: nothing the client reads is null, no
    /// boss, the start refused, the end a withdrawal.
    #[test]
    fn the_raid_routes_answer_an_inactive_raid() {
        let dir = std::env::temp_dir().join(format!("jdr-raid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, class: &str, param: Value| {
            let (r, known) = dispatch(&ctx, path, &env(&uuid, param));
            assert!(known, "{path} is routed");
            let m = match r {
                Ok(m) => Value::Object(m),
                Err(ApiError::Status(c)) => panic!("{path}: status {c}"),
                Err(ApiError::Internal(e)) => panic!("{path}: {e}"),
            };
            assert_complete(class, &m);
            m
        };
        let l = call(
            "/raid/raid_list",
            "Jjs.RaidListResponse",
            json!({"eventCampaignId": "50035"}),
        );
        assert_eq!(l["raidBossList"], json!([]));
        let s = call(
            "/raid/raid_start",
            "Jjs.RaidStartResponse",
            json!({"stageId": "", "raidStageUid": 1, "teamNo": 1, "sallyStatus": 1, "payType": 0}),
        );
        assert_eq!(s["enableRaidStart"], super::START_TIMEOUT);
        assert_eq!(s["raidStageInfo"]["raidStageUid"], 0);
        let e = call(
            "/raid/raid_end",
            "Jjs.RaidEndResponse",
            json!({"result": 1, "defeatedEnemyList": [], "dtDropList": [], "totalDamage": 9}),
        );
        assert_eq!(e["clearResult"], super::RESULT_WITHDRAW);
        let o = call(
            "/raid/raid_offer_rescue",
            "Jjs.RaidOfferRescueResponse",
            json!({"offerType": 0}),
        );
        assert_eq!(o["result"], super::RESCUE_FAILED_ESCAPE);
        let h = call(
            "/raid/raid_history_list",
            "Jjs.RaidHistoryListResponse",
            json!({"nowDate": ""}),
        );
        assert_eq!(h["nowDate"].as_str().unwrap().len(), 19);
        call(
            "/raid/get_raid_reward",
            "Jjs.GetRaidRewardResponse",
            json!({"raidStageUidList": [1]}),
        );
        // The quest replies never make a boss appear (RaidStageInfo.IsValid: raidStageUid != 0).
        assert_complete("Jjs.RaidInfo", &super::raid_info());
        assert_eq!(super::raid_info()["raidStageInfo"]["raidStageUid"], 0);
    }
}
