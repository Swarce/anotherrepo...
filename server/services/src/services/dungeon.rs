//! `/dungeon/*`: the Labyrinth of the Mind (docs/research/22-labyrinth.md), and its battles,
//! which `bizarre_start`/`bizarre_end` hand over here when the stage is a labyrinth room.
//!
//! Client flow (H): Quest Select tile → `user_info` + `dungeon_list` → Challenge (client only) →
//! `stage_list` (the run starts here; there is no start route) → pick a room and a team →
//! `tarot_select` → card scene (`tarot_flip` optional) → `bizarre_start` with the card →
//! `bizarre_end` → next floor, the clear panel (`earn_reward`) or back to the dungeon list.
//! Escape is `earn_reward` from the room list.

use crate::accounts::Change;
use crate::accounts::Profile;
use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::labyrinth::{self, Dungeon, Outcome, DUNGEONS, E_INVALID, FLIP_COST, FLOORS};
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

/// The dungeon a request names: `None` for an empty id, 10103 for an unknown one.
fn check_dungeon(env: &RequestEnvelope) -> Result<Option<&'static Dungeon>, ApiError> {
    match text(env, "dungeonId") {
        "" => Ok(None),
        id => labyrinth::dungeon(id)
            .map(Some)
            .ok_or(ApiError::Status(E_INVALID)),
    }
}

/// The dungeon of the run in progress, else the Beginner one.
fn run_dungeon(p: &Profile) -> &'static Dungeon {
    p.progress
        .dungeon
        .run
        .as_ref()
        .and_then(|r| labyrinth::dungeon(&r.dungeon_id))
        .unwrap_or(&DUNGEONS[0])
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

/// `DungeonListResponse`: the Beginner, Intermediate and Advanced dungeons, with the run's floor
/// on the one in progress (-1 = none). After a battle the client re-selects the dungeon by id from
/// this list, so all three are always there.
pub fn dungeon_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    let list: Vec<Value> = DUNGEONS
        .iter()
        .map(|d| labyrinth::dungeon_info(d, &p.progress.dungeon))
        .collect();
    ok(json!({ "dungeonList": list }))
}

/// `/dungeon/stage_list {dungeonId}` → `{stageList, floorNum, teamInfoList}`: the rooms of the
/// current floor, starting the run on floor 1 if none is in progress. teamInfoList holds the
/// tactical teams (empty would silently block `tarot_select`).
pub fn stage_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let asked = check_dungeon(env)?;
    let d = match asked {
        Some(d) => d,
        None => run_dungeon(&profile(ctx, env)?),
    };
    let (p, floor) = with_profile(ctx, env, |p| match p.progress.dungeon.enter(d.id) {
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
    let rooms: Vec<Value> = labyrinth::rooms_in(d, floor)
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
    let asked = check_dungeon(env)?;
    let room = labyrinth::find_room(text(env, "stageId")).ok_or(ApiError::Status(E_INVALID))?;
    if asked.is_some_and(|d| d.id != room.dungeon.id) {
        return Err(ApiError::Status(E_INVALID));
    }
    let team_no = int(env, "teamNo").unwrap_or(1);
    let now = jdr_core::clock::now();
    let seed = seed();
    let (p, r) = with_profile(ctx, env, |p| {
        let d = &mut p.progress.dungeon;
        let floor = match d.enter(room.dungeon.id) {
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
        d.declined = None;
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
            return ((&DUNGEONS[0], Vec::new()), Change::None);
        }
        let d = run_dungeon(p);
        let floors = p.progress.dungeon.finish();
        for f in &floors {
            let r = labyrinth::floor_reward(d, *f);
            let (data_type, data_id) = crate::presents::reward_kind(r.item_id);
            p.progress.presents.add(
                data_type,
                data_id,
                r.count,
                format!("Labyrinth reward: floor {f}"),
                now,
            );
        }
        ((d, floors), Change::Progress)
    })?;
    let (d, floors) = floors;
    log::line(
        "dungeon",
        &format!("{} run ended: {} floor reward(s) paid", d.id, floors.len()),
    );
    let list: Vec<Value> = floors
        .iter()
        .map(|f| crate::missions::reward_set(&labyrinth::floor_reward(d, *f)))
        .collect();
    ok(json!({ "rewardItemList": list }))
}

/// `/dungeon/reward_check {dungeonId}` → `{rewardItemList}`: every floor's reward, for the
/// reward review.
pub fn reward_check(_ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let d = check_dungeon(env)?.unwrap_or(&DUNGEONS[0]);
    ok(json!({ "rewardItemList": labyrinth::reward_rows(d) }))
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
            support: None,
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
        "areaList": quests::area_list(room.chapter, &room.stage), "minigameInfo": {}, "supportUserInfo": {},
        "chapterInfo": quests::chapter_info(room.chapter, &p.progress),
        "teamInfo": team, "stageInfo": labyrinth::room_stage_info(&room),
        "continueCount": 0,
        "tarotInfo": card.map(labyrinth::tarot_info).unwrap_or_else(|| json!({})),
        "dungeonId": room.dungeon.id, "floorNum": floor,
        "seed": (jdr_core::clock::now() & 0x7fff_ffff)
    }))
}

/// `BizarreResumptionResponse` for a suspended labyrinth run (docs/research/22 §1.8), if any:
/// - a room battle started and never ended → `returnDestination` 1 (Battle): the battle again,
///   with the card it was started with as `selectTarotList[0]` (the battle takes its Tarot from
///   there, `GetSelectTarotInfo` 0xE03604);
/// - a room paid for whose cards are still on offer → 0 (TarotSelect): the card scene again
///   (`SetResumptionCard` 0xD4936C), unless the player declined to resume it.
pub fn resumption(p: &Profile) -> Option<Value> {
    let d = &p.progress.dungeon;
    let run = d.run.as_ref()?;
    let room = labyrinth::find_room(run.room.as_deref()?)?;
    let pending = p
        .progress
        .pending
        .as_ref()
        .filter(|b| b.stage_id == room.id);
    let in_battle = pending.is_some();
    let team_no = pending.map(|b| b.team_num).unwrap_or(run.team_no).max(1);
    let team = quests::team_for(&p.collection, 1, team_no)
        .or_else(|| collection::main_team(&p.collection, 1))
        .map(|t| collection::team_info(&p.collection, t))
        .unwrap_or_else(|| json!({}));
    let (destination, select, area) = if in_battle {
        let chosen: Vec<labyrinth::Card> = run.chosen.into_iter().collect();
        (
            1,
            cards(&chosen),
            quests::area_list(room.chapter, &room.stage),
        )
    } else if !run.offer.is_empty() && d.declined.as_deref() != Some(room.id.as_str()) {
        (0, cards(&run.offer), Vec::new())
    } else {
        return None;
    };
    Some(json!({
        "userInfo": crate::services::user::user_info(p),
        "areaList": area, "minigameInfo": {}, "supportUserInfo": {},
        "chapterInfo": quests::chapter_info(room.chapter, &p.progress),
        "teamInfo": team, "stageInfo": labyrinth::room_stage_info(&room),
        "continueCount": 0, "returnDestination": destination,
        "selectTarotList": select, "usedTarotList": cards(&run.used),
        "dungeonId": room.dungeon.id, "floorNum": run.floor,
        "seed": (jdr_core::clock::now() & 0x7fff_ffff), "isRaidDelete": 0
    }))
}

/// `/bizarre/destruction` after the player declined to resume a labyrinth run: a battle that was
/// started counts as retired (a Soul, as a retire costs one; ours), a Tarot pick is not offered
/// again (the room and its cards stay paid for). Returns whether the run changed.
pub fn decline(p: &mut Profile) -> bool {
    let Some(room) = p.progress.dungeon.run.as_ref().and_then(|r| r.room.clone()) else {
        return false;
    };
    if p.progress
        .pending
        .as_ref()
        .is_some_and(|b| b.stage_id == room)
    {
        p.progress.pending = None;
        let o = p.progress.dungeon.battle_lost();
        log::line(
            "dungeon",
            &format!("{room}: suspended battle declined -> retired ({o:?})"),
        );
        return true;
    }
    p.progress.dungeon.declined = Some(room);
    true
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
            p.progress.dungeon.floor_num(room.dungeon.id),
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
            self.floor_in(labyrinth::DUNGEON_ID)
        }
        fn floor_in(&self, id: &str) -> i64 {
            let l = self.call("/dungeon/dungeon_list", json!({}));
            let list = l["dungeonList"].as_array().unwrap();
            assert_eq!(list.len(), 3, "Beginner, Intermediate, Advanced");
            let d = list.iter().find(|d| d["dungeonId"] == id).unwrap();
            assert_eq!(d["floorMax"], FLOORS);
            d["floorNum"].as_i64().unwrap()
        }
        fn resumption(&self) -> Result<Map<String, Value>, ApiError> {
            self.try_call("/bizarre/bizarre_resumption", json!({}))
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
            gold0 + labyrinth::floor_reward(&labyrinth::DUNGEONS[0], 1).count
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
        assert_eq!(p.progress.dungeon.best("DU01"), FLOORS);
        assert_eq!(p.progress.dungeon.best("DU02"), 0);
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

    /// The three dungeons (docs/research/22 §3.1): their own names and grades, one run at a time
    /// (52101 for another dungeon while one is in progress), rooms with stronger enemies and the
    /// floor rewards multiplied; every reply walked against the client's classes.
    #[test]
    fn the_harder_dungeons() {
        use crate::services::shapes::assert_complete;
        let t = T::new("grades");
        let l = Value::Object(t.call("/dungeon/dungeon_list", json!({})));
        assert_complete("Jjs.DungeonListResponse", &l);
        let names: Vec<&str> = l["dungeonList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "CHAPTER_NAME_D_01",
                "CHAPTER_NAME_D_02",
                "CHAPTER_NAME_D_03"
            ]
        );
        let easy = Value::Object(t.call("/dungeon/stage_list", json!({"dungeonId": "DU01"})));
        // Escape the Beginner run (nothing earned) and enter the Advanced one.
        t.call("/dungeon/earn_reward", json!({}));
        let sl = Value::Object(t.call("/dungeon/stage_list", json!({"dungeonId": "DU03"})));
        assert_complete("Jjs.DungeonStageListResponse", &sl);
        assert_eq!(t.floor_in("DU03"), 1);
        assert_eq!(t.floor_in("DU01"), -1);
        assert!(matches!(
            t.try_call("/dungeon/stage_list", json!({"dungeonId": "DU01"})),
            Err(ApiError::Status(52101))
        ));
        let (e, h) = (
            &easy["stageList"][0]["stageInfo"],
            &sl["stageList"][0]["stageInfo"],
        );
        assert_eq!(h["stageId"].as_str().unwrap(), "STAGE_DU03_01_01");
        assert_eq!(
            h["name"], e["name"],
            "the Beginner room labels name every grade's rooms"
        );
        assert!(
            h["enemyList"][0]["level"].as_i64().unwrap()
                > e["enemyList"][0]["level"].as_i64().unwrap()
        );
        let count = |v: &Value| v["rewardItemSet"][0]["count"].as_i64().unwrap();
        assert_eq!(count(h), 3 * count(e));
        // A room of another grade is refused, and so is a mismatched dungeon id.
        let sid = h["stageId"].as_str().unwrap();
        assert!(t
            .try_call(
                "/dungeon/tarot_select",
                json!({"stageId": sid, "dungeonId": "DU02", "floorNum": 1, "teamNo": 1})
            )
            .is_err());
        let ts = Value::Object(t.call(
            "/dungeon/tarot_select",
            json!({"stageId": sid, "dungeonId": "DU03", "floorNum": 1, "teamNo": 1}),
        ));
        assert_complete("Jjs.DungeonTarotSelectResponse", &ts);
        let card = ts["selectTarotList"][0].clone();
        assert_complete(
            "Jjs.DungeonTarotFlipResponse",
            &Value::Object(t.call("/dungeon/tarot_flip", json!({}))),
        );
        let st = Value::Object(t.call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1, "dungeonStartFlg": 1, "dungeonId": "DU03",
                   "floorNum": 1, "stageType": 1, "tarotInfo": {"tarotId": card["tarotId"]}}),
        ));
        assert_eq!(st["dungeonId"], "DU03");
        assert_complete("Jjs.BizarreStartResponse", &st);
        // (BizarreEndResponse is not walked here: its raid objects belong to the raid work.)
        let end = t.call(
            "/bizarre/bizarre_end",
            json!({"stageId": sid, "result": 0, "dungeonStartFlg": 1, "dungeonId": "DU03",
                   "floorNum": 1}),
        );
        assert_eq!(flags(&end), (0, 0));
        assert_complete(
            "Jjs.DungeonRewardCheckResponse",
            &Value::Object(t.call("/dungeon/reward_check", json!({"dungeonId": "DU03"}))),
        );
        let er = Value::Object(t.call("/dungeon/earn_reward", json!({})));
        assert_complete("Jjs.DungeonEarnRewardResponse", &er);
        assert_eq!(
            er["rewardItemList"][0]["itemList"][0]["count"].as_i64(),
            Some(labyrinth::floor_reward(&labyrinth::DUNGEONS[2], 1).count)
        );
        let p = t.ctx.accounts.get(&t.uuid).unwrap().unwrap();
        assert_eq!(p.progress.dungeon.best("DU03"), 1);
        assert_eq!(p.progress.dungeon.best("DU01"), 0);
    }

    /// Resumption (docs/research/22 §1.8): nothing suspended → 50301; a story battle without its
    /// end → Battle; a paid room whose card was not picked → TarotSelect; a room battle without
    /// its end → Battle with the card it started with. No (destruction) drops a story battle,
    /// retires a room battle (a Soul) and stops offering the Tarot pick. Every reply is walked
    /// against BizarreResumptionResponse.
    #[test]
    fn suspended_battles_and_tarot_picks_resume() {
        use crate::services::shapes::assert_complete;
        let t = T::new("resume");
        assert!(matches!(t.resumption(), Err(ApiError::Status(50301))));

        // A story battle.
        let c = crate::quests::chapters()
            .iter()
            .find(|c| c.stage_type == 1)
            .unwrap();
        let sid = c.stages[0].stage_id.clone();
        t.call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1}),
        );
        let r = Value::Object(t.resumption().unwrap());
        assert_complete("Jjs.BizarreResumptionResponse", &r);
        assert_eq!(r["returnDestination"], 1);
        assert_eq!(r["dungeonId"], "", "Client_Mes_0009 (not the labyrinth's)");
        assert_eq!(r["stageInfo"]["stageId"], sid.as_str());
        assert_eq!(r["chapterInfo"]["stageType"], 1);
        assert!(!r["areaList"].as_array().unwrap().is_empty());
        // Resumed: the battle ends as usual and pays.
        let end = t.call(
            "/bizarre/bizarre_end",
            json!({"stageId": sid, "result": 0, "starClearList": ""}),
        );
        assert!(end["gold"].as_i64().unwrap() > 0);
        assert!(matches!(t.resumption(), Err(ApiError::Status(50301))));
        // Declined: dropped, nothing paid.
        t.call(
            "/bizarre/bizarre_start",
            json!({"stageId": sid, "teamNo": 1}),
        );
        assert!(t.resumption().is_ok());
        t.call("/bizarre/destruction", json!({"stageType": 1}));
        assert!(matches!(t.resumption(), Err(ApiError::Status(50301))));

        // A Tarot pick.
        let sl = t.call("/dungeon/stage_list", json!({"dungeonId": "DU02"}));
        let room = sl["stageList"][0]["stageInfo"]["stageId"]
            .as_str()
            .unwrap()
            .to_string();
        let param = json!({"stageId": room, "dungeonId": "DU02", "floorNum": 1, "teamNo": 1});
        let offer = t.call("/dungeon/tarot_select", param.clone())["selectTarotList"].clone();
        let r = Value::Object(t.resumption().unwrap());
        assert_complete("Jjs.BizarreResumptionResponse", &r);
        assert_eq!(r["returnDestination"], 0);
        assert_eq!(r["dungeonId"], "DU02", "Client_Mes_0010");
        assert_eq!(r["selectTarotList"], offer);
        assert_eq!(r["floorNum"], 1);
        assert_eq!(r["stageInfo"]["stageId"], room.as_str());
        assert!(r["teamInfo"]["teamOrder"].is_object());
        // Declined: not offered again; the room's cards stay paid for.
        t.call("/bizarre/destruction", json!({"stageType": 1}));
        assert!(matches!(t.resumption(), Err(ApiError::Status(50301))));
        let again = t.call("/dungeon/tarot_select", param.clone());
        assert_eq!(again["selectTarotList"], offer);

        // A room battle.
        let card = offer[1].clone();
        t.call(
            "/bizarre/bizarre_start",
            json!({"stageId": room, "teamNo": 1, "dungeonStartFlg": 1, "dungeonId": "DU02",
                   "floorNum": 1, "stageType": 1, "tarotInfo": {"tarotId": card["tarotId"]}}),
        );
        let r = Value::Object(t.resumption().unwrap());
        assert_complete("Jjs.BizarreResumptionResponse", &r);
        assert_eq!(r["returnDestination"], 1);
        assert_eq!(r["dungeonId"], "DU02");
        assert_eq!(
            r["selectTarotList"][0]["tarotId"], card["tarotId"],
            "the battle takes its Tarot from selectTarotList[0]"
        );
        assert!(!r["areaList"].as_array().unwrap().is_empty());
        assert!(r["usedTarotList"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["tarotId"] == card["tarotId"]));
        // Declined: retired, a Soul gone; the room is free again.
        t.call("/bizarre/destruction", json!({"stageType": 1}));
        assert_eq!(t.user()["soul"], SOUL_MAX - 1);
        assert_eq!(t.floor_in("DU02"), 1);
        assert!(matches!(t.resumption(), Err(ApiError::Status(50301))));
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
