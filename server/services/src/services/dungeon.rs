//! `/dungeon/*`: the Labyrinth of the Mind (docs/research/22-labyrinth.md), and its battles,
//! which `bizarre_start`/`bizarre_end` hand over here when the stage is a labyrinth room.
//!
//! Client flow (H): Quest Select tile → `user_info` + `dungeon_list` → Challenge (client only) →
//! `stage_list` (the run starts here; there is no start route) → pick a room and a team →
//! `tarot_select` → card scene (`tarot_flip` optional) → `bizarre_start` with the card →
//! `bizarre_end` → next floor, the clear panel (`earn_reward`) or back to the dungeon list.
//! Escape is `earn_reward` from the room list.

use crate::accounts::Change;
use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::labyrinth::{self, Outcome, DUNGEON_ID, E_INVALID, FLIP_COST, FLOORS};
use crate::quests::{self, PendingBattle};
use crate::services::bizarre::{end_response, no_event, EndInfo};
use crate::services::ok;
use crate::services::statue::{profile, with_profile};
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use jdr_core::log;
use serde_json::{json, Value};

fn int(env: &RequestEnvelope, k: &str) -> Option<i64> {
    env.param.get(k).and_then(Value::as_i64)
}

fn text<'a>(env: &'a RequestEnvelope, k: &str) -> &'a str {
    env.param.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Our one dungeon; an empty id is taken as it (ours).
fn check_dungeon(env: &RequestEnvelope) -> Result<(), ApiError> {
    match text(env, "dungeonId") {
        "" | DUNGEON_ID => Ok(()),
        _ => Err(ApiError::Status(E_INVALID)),
    }
}

/// Dealing needs variety, not cryptographic randomness.
fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
}

fn cards(list: &[labyrinth::Card]) -> Vec<Value> {
    list.iter().map(|c| labyrinth::tarot_info(*c)).collect()
}

/// `DungeonListResponse`: our dungeon, with the run's floor (-1 = none in progress). After a
/// battle the client re-selects the dungeon by id from this list, so it is always there.
pub fn dungeon_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({ "dungeonList": [labyrinth::dungeon_info(&p.progress.dungeon)] }))
}

/// `/dungeon/stage_list {dungeonId}` → `{stageList, floorNum, teamInfoList}`: the rooms of the
/// current floor, starting the run on floor 1 if none is in progress. teamInfoList holds the
/// tactical teams (empty would silently block `tarot_select`).
pub fn stage_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    check_dungeon(env)?;
    let (p, floor) = with_profile(ctx, env, |p| match p.progress.dungeon.enter(DUNGEON_ID) {
        Ok((run, started)) => {
            let change = if started {
                Change::Progress
            } else {
                Change::None
            };
            (Ok(run.floor), change)
        }
        Err(code) => (Err(code), Change::None),
    })?;
    let floor = floor.map_err(ApiError::Status)?.min(FLOORS);
    let rooms: Vec<Value> = labyrinth::rooms(floor)
        .iter()
        .map(|r| json!({ "stageInfo": labyrinth::room_stage_info(r), "mapGimmickFlg": 0 }))
        .collect();
    let teams: Vec<Value> = p
        .collection
        .teams
        .iter()
        .filter(|t| t.stage_type == 1)
        .map(|t| collection::team_info(&p.collection, t))
        .collect();
    ok(json!({ "stageList": rooms, "floorNum": floor, "teamInfoList": teams }))
}

/// `/dungeon/tarot_select {stageId, dungeonId, floorNum, teamNo}` →
/// `{selectTarotList, usedTarotList}`: pays the room's Mentality (52102 when short) and deals
/// the offer the server keeps. A resent request for the room already paid for returns the same
/// offer without charging again (ours).
pub fn tarot_select(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    check_dungeon(env)?;
    let room = labyrinth::find_room(text(env, "stageId")).ok_or(ApiError::Status(E_INVALID))?;
    let team_no = int(env, "teamNo").unwrap_or(1);
    let now = jdr_core::clock::now();
    let seed = seed();
    let (p, r) = with_profile(ctx, env, |p| {
        let d = &mut p.progress.dungeon;
        let floor = match d.enter(DUNGEON_ID) {
            Ok((run, _)) => run.floor,
            Err(code) => return (Err(code), Change::None),
        };
        if room.floor != floor {
            return (Err(E_INVALID), Change::None);
        }
        if d.run
            .as_ref()
            .is_some_and(|r| r.room.as_deref() == Some(room.id.as_str()) && !r.offer.is_empty())
        {
            return (Ok(()), Change::None);
        }
        if let Err(code) = d.spend(room.cost, now) {
            return (Err(code), Change::None);
        }
        if let Some(run) = d.run.as_mut() {
            run.room = Some(room.id.clone());
            run.team_no = team_no;
            run.chosen = None;
            run.deal(seed);
        }
        (Ok(()), Change::Progress)
    })?;
    r.map_err(ApiError::Status)?;
    let run = p.progress.dungeon.run.clone().unwrap_or_default();
    ok(json!({ "selectTarotList": cards(&run.offer), "usedTarotList": cards(&run.used) }))
}

/// `/dungeon/tarot_flip {}` → `{selectTarotList}`: reverses the three cards on offer for 20
/// Mentality (52102 when short). The client then asks `user_info` for the new Mentality.
pub fn tarot_flip(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let now = jdr_core::clock::now();
    let (p, r) = with_profile(ctx, env, |p| {
        let d = &mut p.progress.dungeon;
        if !d.run.as_ref().is_some_and(|r| !r.offer.is_empty()) {
            return (Err(E_INVALID), Change::None);
        }
        if let Err(code) = d.spend(FLIP_COST, now) {
            return (Err(code), Change::None);
        }
        if let Some(run) = d.run.as_mut() {
            run.flip();
        }
        (Ok(()), Change::Progress)
    })?;
    r.map_err(ApiError::Status)?;
    let offer = p
        .progress
        .dungeon
        .run
        .as_ref()
        .map(|r| cards(&r.offer))
        .unwrap_or_default();
    ok(json!({ "selectTarotList": offer }))
}

/// `/dungeon/earn_reward {}` → `{rewardItemList}`: the Clear and Escape panels. Ends the run and
/// pays the pending floor rewards to the Message Box (ours); a second call pays nothing.
pub fn earn_reward(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let now = jdr_core::clock::now();
    let (_, floors) = with_profile(ctx, env, |p| {
        if p.progress.dungeon.run.is_none() {
            return (Vec::new(), Change::None);
        }
        let floors = p.progress.dungeon.finish();
        for f in &floors {
            let r = labyrinth::floor_reward(*f);
            let (data_type, data_id) = crate::presents::reward_kind(r.item_id);
            p.progress.presents.add(
                data_type,
                data_id,
                r.count,
                format!("Labyrinth reward: floor {f}"),
                now,
            );
        }
        (floors, Change::Progress)
    })?;
    log::line(
        "dungeon",
        &format!("run ended: {} floor reward(s) paid", floors.len()),
    );
    let list: Vec<Value> = floors
        .iter()
        .map(|f| crate::missions::reward_set(&labyrinth::floor_reward(*f)))
        .collect();
    ok(json!({ "rewardItemList": list }))
}

/// `/dungeon/reward_check {dungeonId}` → `{rewardItemList}`: every floor's reward, for the
/// reward review.
pub fn reward_check(_ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    check_dungeon(env)?;
    ok(json!({ "rewardItemList": labyrinth::reward_rows() }))
}

/// A battle is a labyrinth battle when its stage is a room (the request also carries
/// `dungeonStartFlg` 1).
pub fn is_dungeon_battle(env: &RequestEnvelope, stage_id: &str) -> bool {
    let room = labyrinth::find_room(stage_id).is_some();
    if !room && int(env, "dungeonStartFlg") == Some(1) {
        log::line(
            "dungeon",
            &format!("WARN dungeonStartFlg 1 for {stage_id:?}, which is not a room"),
        );
    }
    room
}

/// `BizarreStartResponse` for a room: takes the chosen card off the offer (used until the deck
/// resets) and describes the battle. chapterInfo is never null (its stageType picks the
/// tactical battle; null throws).
pub fn bizarre_start(ctx: &Ctx, env: &RequestEnvelope, stage_id: &str) -> ApiResult {
    let room = labyrinth::find_room(stage_id).ok_or(ApiError::Status(E_INVALID))?;
    let team_no = int(env, "teamNo").unwrap_or(1);
    let tarot = env
        .param
        .get("tarotInfo")
        .and_then(|t| t.get("tarotId"))
        .and_then(Value::as_str)
        .and_then(labyrinth::parse_tarot_id);
    let (p, r) = with_profile(ctx, env, |p| {
        let team = quests::team_for(&p.collection, 1, team_no).cloned();
        let Some(run) = p
            .progress
            .dungeon
            .run
            .as_mut()
            .filter(|r| r.room.as_deref() == Some(room.id.as_str()))
        else {
            return (Err(E_INVALID), Change::None);
        };
        let card = if run.offer.is_empty() {
            run.chosen // a resent start: the card already taken
        } else {
            tarot.and_then(|id| run.choose(id))
        };
        let floor = run.floor;
        p.progress.pending = team.as_ref().map(|t| PendingBattle {
            stage_id: room.id.clone(),
            stage_type: 1,
            team_num: t.team_num,
        });
        (Ok((team, card, floor)), Change::Progress)
    })?;
    let (team, card, floor) = r.map_err(ApiError::Status)?;
    if card.is_none() {
        log::line(
            "dungeon",
            &format!("battle in {} without a card on offer", room.id),
        );
    }
    let team = team
        .map(|t| collection::team_info(&p.collection, &t))
        .unwrap_or_else(|| json!({}));
    ok(json!({
        "userInfo": crate::services::user::user_info(&p),
        "areaList": quests::area_list(room.chapter, room.stage), "minigameInfo": {}, "supportUserInfo": {},
        "chapterInfo": quests::chapter_info(room.chapter, &p.progress),
        "teamInfo": team, "stageInfo": labyrinth::room_stage_info(&room),
        "continueCount": 0,
        "tarotInfo": card.map(labyrinth::tarot_info).unwrap_or_else(|| json!({})),
        "dungeonId": DUNGEON_ID, "floorNum": floor,
        "seed": (jdr_core::clock::now() & 0x7fff_ffff)
    }))
}

/// `BizarreEndResponse` for a room, once per started battle: a win banks the floor's reward and
/// opens the next floor (`dungeonClearFlag` 1 after the last), a loss or retire costs a Soul
/// (`dungeonGameoverFlag` 1 when the last one goes, and the run with its pending rewards is
/// lost). A win also gives the stage's rank EXP to the player and the team (ours); gold comes
/// only from the floor rewards.
pub fn bizarre_end(ctx: &Ctx, env: &RequestEnvelope, stage_id: &str, won: bool) -> ApiResult {
    let room = labyrinth::find_room(stage_id).ok_or(ApiError::Status(E_INVALID))?;
    let (p, (outcome, exp, team)) = with_profile(ctx, env, |p| {
        let pending = p.progress.pending.take_if(|b| b.stage_id == room.id);
        let team = pending.as_ref().and_then(|b| {
            p.collection
                .teams
                .iter()
                .find(|t| t.stage_type == b.stage_type && t.team_num == b.team_num)
                .cloned()
        });
        if pending.is_none() {
            // Resent: report the run as it stands, change nothing.
            let cleared = p
                .progress
                .dungeon
                .run
                .as_ref()
                .is_some_and(|r| r.floor > FLOORS);
            let o = if cleared {
                Outcome::Cleared
            } else {
                Outcome::Continue
            };
            return ((o, 0, team), Change::None);
        }
        let outcome = if won {
            p.progress.dungeon.battle_won()
        } else {
            p.progress.dungeon.battle_lost()
        };
        let mut exp = 0;
        if won {
            exp = room.stage.gain_rank_exp;
            p.exp += exp;
            p.rank = collection::rank_for_exp(p.exp);
            p.progress.missions.bump("exp", exp, jdr_core::clock::now());
            let uids = team.as_ref().map(|t| t.slots.to_vec()).unwrap_or_default();
            p.collection.add_statue_exp(&uids, exp);
        }
        ((outcome, exp, team), Change::Progress)
    })?;
    log::line(
        "dungeon",
        &format!(
            "{} {} -> {outcome:?}, floor {}, souls {}",
            room.id,
            if won { "won" } else { "lost" },
            p.progress.dungeon.floor_num(DUNGEON_ID),
            p.progress.dungeon.souls()
        ),
    );
    let team = team
        .or_else(|| collection::main_team(&p.collection, 1).cloned())
        .map(|t| collection::team_info(&p.collection, &t))
        .unwrap_or_else(|| json!({}));
    end_response(
        &p,
        env,
        EndInfo {
            won,
            gold: 0,
            rank_exp: exp,
            team,
            event_result: no_event(),
            clear: (outcome == Outcome::Cleared) as i64,
            gameover: (outcome == Outcome::GameOver) as i64,
            friend_points: 0,
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::accounts::Change;
    use crate::app::ApiError;
    use crate::labyrinth::{self, FLOORS, MENTALITY_MAX, SOUL_MAX};
    use crate::services::dispatch;
    use crate::Ctx;
    use jdr_core::envelope::RequestEnvelope;
    use jdr_core::settings::Settings;
    use serde_json::{json, Map, Value};

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

    /// Crash traps: the client reads every list and object it is sent.
    fn no_nulls(v: &Value) -> bool {
        match v {
            Value::Null => false,
            Value::Array(a) => a.iter().all(no_nulls),
            Value::Object(o) => o.values().all(no_nulls),
            _ => true,
        }
    }

    struct T {
        ctx: Ctx,
        uuid: String,
    }

    impl T {
        fn new(tag: &str) -> T {
            let dir =
                std::env::temp_dir().join(format!("jdr-dungeon-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let ctx = Ctx::new(Settings::new(dir), false);
            let uuid = ctx.accounts.create(&[]).unwrap().uuid;
            T { ctx, uuid }
        }
        fn try_call(&self, path: &str, param: Value) -> Result<Map<String, Value>, ApiError> {
            let (r, known) = dispatch(&self.ctx, path, &env(&self.uuid, param));
            assert!(known, "{path} is routed");
            if let Ok(m) = &r {
                assert!(
                    no_nulls(&Value::Object(m.clone())),
                    "{path}: a null in {m:?}"
                );
            }
            r
        }
        fn call(&self, path: &str, param: Value) -> Map<String, Value> {
            self.try_call(path, param).unwrap()
        }
        fn user(&self) -> Value {
            self.call("/user/user_info", json!({}))["userInfo"].clone()
        }
        fn floor(&self) -> i64 {
            let l = self.call("/dungeon/dungeon_list", json!({}));
            let list = l["dungeonList"].as_array().unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0]["dungeonId"], labyrinth::DUNGEON_ID);
            assert_eq!(list[0]["floorMax"], FLOORS);
            list[0]["floorNum"].as_i64().unwrap()
        }
        fn refill(&self) {
            self.ctx
                .accounts
                .update(&self.uuid, |p| {
                    p.progress.dungeon.mentality_used = 0;
                    ((), Change::Progress)
                })
                .unwrap();
        }
        fn waiting(&self) -> i64 {
            self.call("/info/get_menu_notify", json!({}))["unopenedPresent"]
                .as_i64()
                .unwrap()
        }
        /// stage_list → tarot_select (first room) → bizarre_start → bizarre_end.
        fn battle(&self, result: i64) -> Map<String, Value> {
            let sl = self.call("/dungeon/stage_list", json!({"dungeonId": "DU01"}));
            let floor = sl["floorNum"].as_i64().unwrap();
            let rooms = sl["stageList"].as_array().unwrap();
            assert_eq!(rooms.len(), labyrinth::rooms_on(floor));
            let teams = sl["teamInfoList"].as_array().unwrap();
            assert!(!teams.is_empty(), "an empty team list blocks tarot_select");
            assert_eq!(teams.iter().filter(|t| t["mainFlg"] == 1).count(), 1);
            let sid = rooms[0]["stageInfo"]["stageId"]
                .as_str()
                .unwrap()
                .to_string();
            let ts = self.call(
                "/dungeon/tarot_select",
                json!({"stageId": sid, "dungeonId": "DU01", "floorNum": floor, "teamNo": 1}),
            );
            let card = ts["selectTarotList"][0].clone();
            let st = self.call(
                "/bizarre/bizarre_start",
                json!({"stageId": sid, "teamNo": 1, "dungeonStartFlg": 1, "dungeonId": "DU01",
                       "floorNum": floor, "stageType": 1,
                       "tarotInfo": {"tarotId": card["tarotId"], "tarotPosition": card["position"]}}),
            );
            assert_eq!(
                st["chapterInfo"]["stageType"], 1,
                "chapterInfo picks the battle type"
            );
            assert_eq!(st["tarotInfo"]["tarotId"], card["tarotId"]);
            assert_eq!(st["floorNum"], floor);
            assert!(!st["areaList"].as_array().unwrap().is_empty());
            self.call(
                "/bizarre/bizarre_end",
                json!({"stageId": sid, "result": result, "starClearList": "", "dungeonStartFlg": 1,
                       "dungeonId": "DU01", "floorNum": floor}),
            )
        }
    }

    fn flags(end: &Map<String, Value>) -> (i64, i64) {
        (
            end["dungeonClearFlag"].as_i64().unwrap(),
            end["dungeonGameoverFlag"].as_i64().unwrap(),
        )
    }

    #[test]
    fn a_full_labyrinth_run() {
        let t = T::new("run");
        let sel = t.call("/bizarre/get_bizarre_select", json!({}));
        assert_eq!(sel["dungeonLockFlg"], 0, "the Labyrinth tile is open");
        let u = t.user();
        assert_eq!(u["soul"], SOUL_MAX);
        assert_eq!(u["mentality"], MENTALITY_MAX);
        assert_eq!(t.floor(), -1, "no run yet");
        let review = t.call("/dungeon/reward_check", json!({"dungeonId": "DU01"}));
        let rows = review["rewardItemList"].as_array().unwrap();
        assert_eq!(rows.len(), FLOORS as usize);
        assert!(rows
            .iter()
            .all(|r| r["rewardItemSet"]["itemList"].as_array().unwrap().len() == 1));

        // Entering the room list starts the run on floor 1.
        let sl = t.call("/dungeon/stage_list", json!({"dungeonId": "DU01"}));
        assert_eq!(sl["floorNum"], 1);
        assert_eq!(t.floor(), 1);
        let room = sl["stageList"][1]["stageInfo"].clone();
        let sid = room["stageId"].as_str().unwrap().to_string();
        let cost = room["useMentality"].as_i64().unwrap();
        assert!((22..=35).contains(&cost));
        let param = json!({"stageId": sid, "dungeonId": "DU01", "floorNum": 1, "teamNo": 1});

        // Tarot: the room's Mentality is paid, three unused cards are dealt.
        let ts = t.call("/dungeon/tarot_select", param.clone());
        let offer = ts["selectTarotList"].as_array().unwrap().clone();
        assert_eq!(offer.len(), 3);
        assert_eq!(ts["usedTarotList"], json!([]));
        assert_eq!(t.user()["mentality"], MENTALITY_MAX - cost);
        for c in &offer {
            assert_eq!(c["position"], 0);
            assert!((0..22).contains(&c["imgNo"].as_i64().unwrap()));
            assert!(c["abilityInfo"].is_object());
        }
        // A resent select returns the same offer for free.
        let again = t.call("/dungeon/tarot_select", param.clone());
        assert_eq!(again["selectTarotList"].as_array().unwrap(), &offer);
        assert_eq!(t.user()["mentality"], MENTALITY_MAX - cost);
        // Flip: the same three reversed, for 20.
        let fl = t.call("/dungeon/tarot_flip", json!({}));
        let flipped = fl["selectTarotList"].as_array().unwrap().clone();
        assert_eq!(flipped.len(), 3);
        for (a, b) in offer.iter().zip(&flipped) {
            assert_eq!(a["tarotId"], b["tarotId"]);
            assert_eq!(b["position"], 1);
            assert!(b["description"].as_str().unwrap().ends_with("_REV"));
        }
        assert_eq!(t.user()["mentality"], MENTALITY_MAX - cost - 20);

        // Battle: a win opens floor 2.
        let card = &flipped[2];
        let st = t.call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1, "dungeonStartFlg": 1, "dungeonId": "DU01",
                   "floorNum": 1, "stageType": 1,
                   "tarotInfo": {"tarotId": card["tarotId"], "tarotPosition": 1}}),
        );
        assert_eq!(st["chapterInfo"]["stageType"], 1);
        assert_eq!(st["stageInfo"]["stageId"], sid.as_str());
        assert_eq!(st["tarotInfo"]["position"], 1);
        assert_eq!(st["userInfo"]["soul"], SOUL_MAX);
        let exp0 = t.user()["exp"].as_i64().unwrap();
        let end = t.call(
            "/bizarre/bizarre_end",
            json!({"stageId": sid, "result": 0, "starClearList": "1", "dungeonStartFlg": 1,
                   "dungeonId": "DU01", "floorNum": 1}),
        );
        assert_eq!(flags(&end), (0, 0));
        assert!(t.user()["exp"].as_i64().unwrap() >= exp0);
        assert_eq!(t.floor(), 2);
        // A resent end changes nothing.
        let again = t.call(
            "/bizarre/bizarre_end",
            json!({"stageId": sid, "result": 0, "dungeonStartFlg": 1}),
        );
        assert_eq!(flags(&again), (0, 0));
        assert_eq!(t.floor(), 2);
        // The card is used; flipping needs an offer.
        assert!(t.try_call("/dungeon/tarot_flip", json!({})).is_err());
        let sl = t.call("/dungeon/stage_list", json!({"dungeonId": "DU01"}));
        assert_eq!(sl["floorNum"], 2);
        let sid2 = sl["stageList"][0]["stageInfo"]["stageId"]
            .as_str()
            .unwrap()
            .to_string();
        let ts = t.call(
            "/dungeon/tarot_select",
            json!({"stageId": sid2, "dungeonId": "DU01", "floorNum": 2, "teamNo": 1}),
        );
        let used = ts["usedTarotList"].as_array().unwrap();
        assert_eq!(used.len(), 1);
        assert_eq!(used[0]["tarotId"], card["tarotId"]);
        assert!(ts["selectTarotList"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["tarotId"] != card["tarotId"]));
        // A room of another floor is refused.
        assert!(t.try_call("/dungeon/tarot_select", param).is_err());

        // Not enough Mentality: 52102, nothing spent.
        t.ctx
            .accounts
            .update(&t.uuid, |p| {
                p.progress.dungeon.mentality_used = MENTALITY_MAX - 5;
                ((), Change::Progress)
            })
            .unwrap();
        assert!(matches!(
            t.try_call("/dungeon/tarot_flip", json!({})),
            Err(ApiError::Status(52102))
        ));
        let sid3 = sl["stageList"][1]["stageInfo"]["stageId"].as_str().unwrap();
        assert!(matches!(
            t.try_call(
                "/dungeon/tarot_select",
                json!({"stageId": sid3, "dungeonId": "DU01", "floorNum": 2, "teamNo": 1})
            ),
            Err(ApiError::Status(52102))
        ));
        assert_eq!(t.user()["mentality"], 5);
        t.refill();

        // A defeat costs a Soul and keeps the floor.
        assert_eq!(flags(&t.battle(1)), (0, 0));
        assert_eq!(t.user()["soul"], SOUL_MAX - 1);
        assert_eq!(t.floor(), 2);

        // Escape: floor 1's reward goes to the Message Box, the Souls are restored.
        t.call("/present/get_all", json!({"prevNum": 1})); // the welcome gifts out of the way
        let box0 = t.waiting();
        let er = t.call("/dungeon/earn_reward", json!({}));
        let paid = er["rewardItemList"].as_array().unwrap();
        assert_eq!(paid.len(), 1, "one floor cleared");
        assert_eq!(paid[0]["itemList"][0]["itemId"], 20, "floor 1 pays gold");
        assert_eq!(t.waiting(), box0 + 1);
        assert_eq!(t.floor(), -1);
        assert_eq!(t.user()["soul"], SOUL_MAX);
        let gold0 = t.user()["vm"].as_i64().unwrap();
        t.call("/present/get_all", json!({"prevNum": 1}));
        assert_eq!(
            t.user()["vm"].as_i64().unwrap(),
            gold0 + labyrinth::floor_reward(1).count
        );
        assert_eq!(
            t.call("/dungeon/earn_reward", json!({}))["rewardItemList"],
            json!([]),
            "paid once"
        );

        // Zero Souls forfeits: win floor 1, then lose three times.
        t.refill();
        assert_eq!(flags(&t.battle(0)), (0, 0));
        assert_eq!(flags(&t.battle(1)), (0, 0));
        assert_eq!(flags(&t.battle(1)), (0, 0));
        assert_eq!(t.user()["soul"], 1);
        assert_eq!(flags(&t.battle(1)), (0, 1));
        assert_eq!(t.floor(), -1, "the run is over");
        let box1 = t.waiting();
        assert_eq!(
            t.call("/dungeon/earn_reward", json!({}))["rewardItemList"],
            json!([]),
            "forfeited"
        );
        assert_eq!(t.waiting(), box1);

        // A full clear: twenty floors, then the clear panel pays every floor.
        for f in 1..=FLOORS {
            t.refill();
            let end = t.battle(0);
            assert_eq!(flags(&end), ((f == FLOORS) as i64, 0), "floor {f}");
        }
        assert_eq!(
            t.floor(),
            FLOORS + 1,
            "past floorMax: the client asks earn_reward"
        );
        let er = t.call("/dungeon/earn_reward", json!({}));
        let paid = er["rewardItemList"].as_array().unwrap();
        assert_eq!(paid.len(), FLOORS as usize);
        for set in paid {
            for k in ["statueList", "mindList", "itemList", "clanObjectList"] {
                assert!(set[k].is_array(), "{set}");
            }
        }
        assert_eq!(t.waiting(), box1 + FLOORS);
        assert_eq!(t.floor(), -1);
        let p = t.ctx.accounts.get(&t.uuid).unwrap().unwrap();
        assert_eq!(p.progress.dungeon.clears, 1);
        assert_eq!(p.progress.dungeon.best_floor, FLOORS);
        assert!(
            p.progress
                .cleared
                .iter()
                .all(|s| !s.starts_with("STAGE_DU01_")),
            "rooms are not story progress"
        );
    }

    #[test]
    fn story_battles_are_unchanged() {
        let t = T::new("story");
        let c = crate::quests::chapters()
            .iter()
            .find(|c| c.stage_type == 1)
            .unwrap();
        let sid = c.stages[0].stage_id.clone();
        let st = t.call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1}),
        );
        assert_eq!(st["tarotInfo"], json!({}));
        assert_eq!(st["dungeonId"], "");
        let end = t.call(
            "/bizarre/bizarre_end",
            json!({"stageId": sid, "result": 0, "starClearList": ""}),
        );
        assert_eq!(flags(&end), (0, 0));
        assert!(end["gold"].as_i64().unwrap() > 0);
        assert_eq!(t.floor(), -1);
    }

    #[test]
    fn requests_outside_the_run_are_refused() {
        let t = T::new("refused");
        let sl = t.call("/dungeon/stage_list", json!({"dungeonId": "DU01"}));
        let sid = sl["stageList"][0]["stageInfo"]["stageId"].as_str().unwrap();
        assert!(
            t.try_call(
                "/bizarre/bizarre_start",
                json!({"stageId": sid, "teamNo": 1, "dungeonStartFlg": 1})
            )
            .is_err(),
            "a start needs the room paid for"
        );
        assert!(t
            .try_call("/dungeon/stage_list", json!({"dungeonId": "XX"}))
            .is_err());
        assert!(t
            .try_call(
                "/dungeon/tarot_select",
                json!({"stageId": "nope", "dungeonId": "DU01", "teamNo": 1})
            )
            .is_err());
    }
}
