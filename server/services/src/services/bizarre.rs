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
    ok(json!({ "questList": list, "eventCampaignList": campaigns, "dungeonLockFlg": 1 }))
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
    // Event Home backs out with an "expired" dialog unless eventCampaignId is set; ranking,
    // collection points, shop, BGM and the event summon stay off (all 0 or "").
    let event_info = quests::events()
        .iter()
        .find(|e| e.category == category)
        .map(|e| {
            json!({ "eventCampaignId": e.event_id, "eventShopFlg": 0, "eventCollectType": 0,
                "eventRankingFlg": 0, "eventMissiongFlg": 0, "eventBg": "",
                "htmlLink": format!("/information/event_{}.html", e.event_id), "eventBgm": "",
                "eventTutorialImage": "", "eventItemList": [], "eventPointImage": "",
                "userEventPoint": 0, "clanEventPoint": 0, "endDate": "", "finishedDate": "",
                "banner": e.banner })
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
        "dungeonClearFlag": 0, "dungeonGameoverFlag": 0, "nowEventRankingPoint": 0,
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

/// `BizarreDestructionResponse`: abandoning a stage needs no bookkeeping.
pub fn destruction(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
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
