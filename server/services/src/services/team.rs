//! `/team/*` edits and the profile card: change a team's leader, set its members or make it the
//! main team. The formation screen does not say which battle type it edits; the same team number
//! is changed for both types so action and tactical teams stay alike (My Page reads the tactical
//! one).
//!
//! The edits work on a [`Roster`]: the story teams, or the Arena's own five teams, which the
//! `/pvp/*_pvp` formation routes change the same way (services/arena.rs, docs/research/23-arena.md).

use crate::accounts::Change;
use crate::app::{ApiError, ApiResult};
use crate::collection::{self, Collection, Team};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

fn int(env: &RequestEnvelope, k: &str) -> Option<i64> {
    env.param.get(k).and_then(Value::as_i64)
}

/// Which list of teams an edit changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Roster {
    /// The story teams (one per battle type and number; both types follow an edit).
    Story,
    /// The Arena's own five teams (client: a separate roster, docs/research/23-arena.md).
    Arena,
}

impl Roster {
    pub fn teams(self, c: &Collection) -> &[Team] {
        match self {
            Roster::Story => &c.teams,
            Roster::Arena => &c.arena_teams,
        }
    }

    fn teams_mut(self, c: &mut Collection) -> &mut Vec<Team> {
        match self {
            Roster::Story => &mut c.teams,
            Roster::Arena => &mut c.arena_teams,
        }
    }
}

/// `teamNum` must name one of the player's teams (1..=TEAMS_PER_TYPE); anything else is refused
/// before the save is touched (a stray number must never clear the main team).
pub fn team_num(env: &RequestEnvelope) -> Result<i64, ApiError> {
    match int(env, "teamNum") {
        Some(n) if (1..=collection::TEAMS_PER_TYPE).contains(&n) => Ok(n),
        other => Err(ApiError::Internal(format!(
            "teamNum {other:?} out of range"
        ))),
    }
}

/// The tactical team with this number, for the response (the one My Page and the screen show).
fn shown(c: &Collection, team_num: i64) -> Value {
    c.teams
        .iter()
        .find(|t| t.stage_type == 1 && t.team_num == team_num)
        .map(|t| collection::team_info(c, t))
        .unwrap_or_else(|| json!({}))
}

fn teams_numbered(
    c: &mut Collection,
    roster: Roster,
    team_num: i64,
) -> impl Iterator<Item = &mut Team> {
    roster
        .teams_mut(c)
        .iter_mut()
        .filter(move |t| t.team_num == team_num)
}

/// The statue in seat `seq` (TeamStatueRole 2 left, 3 right) becomes the leader and the old
/// leader takes its place. Returns whether anything changed.
pub fn change_leader(c: &mut Collection, roster: Roster, team_num: i64, seq: i64) -> bool {
    if !(2..=3).contains(&seq) {
        return false;
    }
    let i = (seq - 1) as usize;
    let mut changed = false;
    for t in teams_numbered(c, roster, team_num) {
        if t.slots[i] != 0 {
            t.slots.swap(0, i);
            changed = true;
        }
    }
    changed
}

/// Put statue `uid` in seat `seq` (1 leader, 2 left, 3 right). A statue already in the team swaps
/// seats with the one it replaces; 0 empties a side seat (never the leader). Unknown statues or
/// seats change nothing. Returns whether anything changed.
pub fn select_statue(
    c: &mut Collection,
    roster: Roster,
    team_num: i64,
    seq: i64,
    uid: i64,
) -> bool {
    let usable = uid == 0 || c.known_statue(uid).is_some();
    if !(1..=3).contains(&seq) || !usable || (seq == 1 && uid == 0) {
        return false;
    }
    let mut changed = false;
    for t in teams_numbered(c, roster, team_num) {
        // A team without a leader takes its first statue as the leader, whatever the seat.
        let i = if t.slots[0] == 0 && uid != 0 {
            0
        } else {
            (seq - 1) as usize
        };
        let old = t.slots[i];
        if old == uid {
            continue;
        }
        if let Some(j) = t.slots.iter().position(|s| *s == uid && uid != 0) {
            // Already in the team: swap seats (an empty seat may not become the leader).
            if j == 0 && old == 0 {
                continue;
            }
            t.slots[j] = old;
        }
        t.slots[i] = uid;
        changed = true;
    }
    changed
}

/// The members a team would get from `wanted` (leader, left, right): statues the player does not
/// own become empty seats and a statue named twice keeps its first seat. `None` when no leader is
/// left (such a team is not saved).
pub fn clean_members(c: &Collection, wanted: [i64; 3]) -> Option<[i64; 3]> {
    let mut slots = wanted.map(|u| {
        if u != 0 && c.known_statue(u).is_some() {
            u
        } else {
            0
        }
    });
    for i in 1..3 {
        if slots[..i].contains(&slots[i]) {
            slots[i] = 0;
        }
    }
    (slots[0] != 0).then_some(slots)
}

/// Set a team's members (see [`clean_members`]); `None` (nothing saved) without a leader.
pub fn set_members(
    c: &mut Collection,
    roster: Roster,
    team_num: i64,
    wanted: [i64; 3],
) -> Option<[i64; 3]> {
    let slots = clean_members(c, wanted)?;
    for t in teams_numbered(c, roster, team_num) {
        t.slots = slots;
    }
    Some(slots)
}

/// `LeaderChangeRequest {teamNum, statueSeq}` (TeamStatueRole 1 leader, 2 left, 3 right): the
/// statue in that slot becomes the leader and the old leader takes its place.
pub fn leader_change(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let seq = int(env, "statueSeq").unwrap_or(1);
    if !(1..=3).contains(&seq) {
        // Nothing to change: answer with the team as it is.
        let p = crate::services::statue::profile(ctx, env)?;
        return ok(json!({ "teamInfo": shown(&p.collection, team_num) }));
    }
    let (p, ()) = with_profile(ctx, env, |p| {
        let changed = change_leader(&mut p.collection, Roster::Story, team_num, seq);
        if changed {
            p.progress
                .missions
                .bump("leader", 1, jdr_core::clock::now());
        }
        (
            (),
            if changed {
                Change::Bookkeeping // team edits are cheap to redo: no backup rotation
            } else {
                Change::None
            },
        )
    })?;
    ok(json!({ "teamInfo": shown(&p.collection, team_num) }))
}

/// `StatueChangeRequest {teamNum, statueSeq, statueuId}` (`/team/statue_select`): put that statue
/// in the seat (see [`select_statue`]). Unknown statues or seats change nothing and the reply
/// shows the team as it is.
pub fn statue_select(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let seq = int(env, "statueSeq").unwrap_or(0);
    let uid = int(env, "statueuId").unwrap_or(0);
    let (p, ()) = with_profile(ctx, env, |p| {
        let changed = select_statue(&mut p.collection, Roster::Story, team_num, seq, uid);
        (
            (),
            if changed {
                Change::Bookkeeping
            } else {
                Change::None
            },
        )
    })?;
    ok(json!({ "teamInfo": shown(&p.collection, team_num) }))
}

/// `SetTeamRequest {teamNum, mainFlg, leaderstatueuId, memberLeftStatueuId, memberRightStatueuId}`.
/// Statues the player does not own become empty slots; a team without a leader is not saved (the
/// reply shows the team as it is).
pub fn set_team(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let main = int(env, "mainFlg").unwrap_or(0) == 1;
    let wanted = members_of(env);
    let (p, ()) = with_profile(ctx, env, |p| {
        let Some(slots) = set_members(&mut p.collection, Roster::Story, team_num, wanted) else {
            return ((), Change::None);
        };
        if slots.iter().all(|u| *u != 0) {
            p.progress.missions.bump("team3", 1, jdr_core::clock::now());
        }
        if main {
            for t in &mut p.collection.teams {
                t.main = t.team_num == team_num;
            }
        }
        ((), Change::Bookkeeping)
    })?;
    ok(json!({ "teamInfo": shown(&p.collection, team_num) }))
}

/// `leaderstatueuId`, `memberLeftStatueuId`, `memberRightStatueuId` (SetTeamRequest and
/// PvpSetTeamRequest).
pub fn members_of(env: &RequestEnvelope) -> [i64; 3] {
    [
        int(env, "leaderstatueuId").unwrap_or(0),
        int(env, "memberLeftStatueuId").unwrap_or(0),
        int(env, "memberRightStatueuId").unwrap_or(0),
    ]
}

/// `UserInfoMiniResponse` for the profile card: this player, with the tactical main team's leader
/// as the card's statue, or (`id`) an AI member of the player's Town.
pub fn user_info_mini(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let id = match env.param.get("id") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let card = crate::services::user::ai_user_info_mini(&p, &id)
        .unwrap_or_else(|| crate::services::user::user_info_mini(&p));
    ok(json!({ "userInfoMini": card }))
}

/// `FollowerListResponse` / `BlockListResponse`: nobody, offline.
pub fn follower_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "sortType": 0, "followerList": [] }))
}

pub fn block_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "sortType": 0, "blockList": [] }))
}

/// `TrophyListResponse`: no achievements are tracked offline yet.
pub fn trophy_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let now = jdr_core::clock::now();
    let (p, ()) = with_profile(ctx, env, |p| {
        let day = p.progress.missions.day;
        p.progress.missions.roll(now);
        (
            (),
            if p.progress.missions.day != day {
                Change::Bookkeeping
            } else {
                Change::None
            },
        )
    })?;
    ok(json!({ "trophyList": p.progress.missions.trophy_list(p.rank, now) }))
}

/// `/trophy/trophy_result_get {achievementId}` (one) and `/trophy/get_all_trophy_result {}`
/// (every claimable mission): the full updated list replaces the client's.
pub fn trophy_claim(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let one = env
        .param
        .get("achievementId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let now = jdr_core::clock::now();
    let (p, sets) = with_profile(ctx, env, |p| {
        p.progress.missions.roll(now);
        let ids: Vec<String> = match &one {
            Some(id) => vec![id.clone()],
            None => crate::missions::missions()
                .iter()
                .map(|m| m.id.clone())
                .collect(),
        };
        let mut sets = Vec::new();
        for id in ids {
            if let Some(m) = p.progress.missions.claim(&id, p.rank) {
                // The screen says rewards go to the Message Box, so they do (Build 22).
                crate::services::present::send_mission_reward(p, m, now);
                sets.push(crate::missions::reward_set(&m.reward));
            }
        }
        let change = if sets.is_empty() {
            Change::None
        } else {
            Change::Progress
        };
        (sets, change)
    })?;
    let list = p.progress.missions.trophy_list(p.rank, now);
    let empty = json!({ "itemSetIcon": "", "itemSetNum": 0, "statueList": [], "mindList": [], "itemList": [], "clanObjectList": [] });
    if one.is_some() {
        ok(
            json!({ "itemSetInfo": sets.first().cloned().unwrap_or(empty), "trophyList": list, "popupFlg": false }),
        )
    } else {
        ok(json!({ "itemSetList": sets, "trophyList": list, "popupFlg": false }))
    }
}

// ---- Mind equipment (docs/research/16-minds.md) ----

/// Status codes the client shows (ResponseParameterCode.Status).
const E_MIND_MISSING: i64 = 51103;
/// Not enough statues for a team (client ErrorCode).
const E_TEAM_EMPTY_MEMBER: i64 = 51706;
const E_NO_STATUE: i64 = 51702;
const E_CP_OVER: i64 = 51705;
const E_SLOT_LOCKED: i64 = 51709;

/// Apply `(seq, uid)` changes to one seat of a team number (both battle types follow), in order:
/// uid 0 empties the slot; a Mind already on another seat of the team moves here. A Mind must fit
/// its frame and the statue (`collection::mind_fits`: ability Minds in the golden frames 1-4, a
/// skill Mind in the frame of its kind and only for the statues on its list); a skill Mind the
/// seat's statue can no longer use (the statue changed) is dropped. The result must fit the
/// statue's capacity.
pub fn equip(
    c: &mut Collection,
    roster: Roster,
    team_num: i64,
    seat: usize,
    changes: &[(i64, i64)],
) -> Result<(), i64> {
    let Some(t) = roster.teams(c).iter().find(|t| t.team_num == team_num) else {
        return Err(E_NO_STATUE);
    };
    let Some(statue) = c.known_statue(t.slots[seat]) else {
        return Err(E_NO_STATUE);
    };
    let statue_id = statue.statue_id.clone();
    let mut seats = t.minds.clone();
    for (seq, uid) in changes {
        if !(1..=collection::MIND_SLOTS_OPEN).contains(seq) {
            return Err(E_SLOT_LOCKED);
        }
        seats[seat].retain(|(s, _)| s != seq);
        if *uid != 0 {
            let Some(m) = c.minds.iter().find(|m| m.uid == *uid) else {
                return Err(E_MIND_MISSING);
            };
            if !collection::mind_fits(&m.mind_id, *seq, &statue_id) {
                return Err(E_SLOT_LOCKED);
            }
            for s in seats.iter_mut() {
                s.retain(|(_, u)| u != uid);
            }
            seats[seat].push((*seq, *uid));
        }
    }
    let fit: Vec<(i64, i64)> = collection::seat_minds(c, &seats[seat], &statue_id)
        .iter()
        .map(|(s, m)| (*s, m.uid))
        .collect();
    seats[seat] = fit;
    if seat_cost(c, &seats[seat]) > collection::MIND_CAPACITY {
        return Err(E_CP_OVER);
    }
    for s in seats.iter_mut() {
        s.sort();
    }
    for t in teams_numbered(c, roster, team_num) {
        t.minds = seats.clone();
    }
    Ok(())
}

/// The Mind cost (CP) of one seat's Minds.
pub fn seat_cost(c: &Collection, seat: &[(i64, i64)]) -> i64 {
    seat.iter()
        .filter_map(|(_, u)| c.minds.iter().find(|m| m.uid == *u))
        .map(|m| collection::mind_cost(&m.mind_id))
        .sum()
}

/// `statueSeq` (1..3) as a seat index; anything else is the client's "no statue" error.
pub fn seat_of(env: &RequestEnvelope) -> Result<usize, ApiError> {
    match int(env, "statueSeq") {
        Some(s @ 1..=3) => Ok(s as usize - 1),
        _ => Err(ApiError::Status(E_NO_STATUE)),
    }
}

/// `(equipSeq, <uid key>)` pairs of a `mindList` / `statueList` diff.
pub fn changes_of(list: Option<&Value>, uid_key: &str) -> Vec<(i64, i64)> {
    list.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| Some((m.get("equipSeq")?.as_i64()?, m.get(uid_key)?.as_i64()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// `/team/equip_multiple_minds {teamNum, statueSeq, mindList:[{equipSeq, minduId}]}`: the client's
/// diff of changed slots (clears first). Replies with the team.
pub fn equip_multiple_minds(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let seat = seat_of(env)?;
    let changes = changes_of(env.param.get("mindList"), "minduId");
    let (p, r) = with_profile(ctx, env, |p| {
        match equip(&mut p.collection, Roster::Story, team_num, seat, &changes) {
            Ok(()) => {
                if changes.iter().any(|(_, u)| *u != 0) {
                    p.progress
                        .missions
                        .bump("equip_mind", 1, jdr_core::clock::now());
                }
                (Ok(()), Change::Bookkeeping)
            }
            Err(code) => (Err(code), Change::None),
        }
    })?;
    r.map_err(ApiError::Status)?;
    ok(json!({ "teamInfo": shown(&p.collection, team_num) }))
}

/// Apply an assist diff to a seat (docs/research/19-assists.md): seq 1..3, uid 0 empties the
/// slot; an assist must be owned and not a main member of the team, and moves off the team's
/// other seats.
pub fn equip_assists(
    c: &mut Collection,
    roster: Roster,
    team_num: i64,
    seat: usize,
    changes: &[(i64, i64)],
) -> Result<(), i64> {
    let Some(t) = roster.teams(c).iter().find(|t| t.team_num == team_num) else {
        return Err(E_NO_STATUE);
    };
    if c.known_statue(t.slots[seat]).is_none() {
        return Err(E_NO_STATUE);
    }
    let mut seats = t.assists.clone();
    for (seq, uid) in changes {
        if !(1..=collection::MAX_ASSISTS).contains(seq) {
            return Err(E_SLOT_LOCKED);
        }
        seats[seat].retain(|(s, _)| s != seq);
        if *uid != 0 {
            if c.known_statue(*uid).is_none() || t.slots.contains(uid) {
                return Err(E_NO_STATUE);
            }
            for s in seats.iter_mut() {
                s.retain(|(_, u)| u != uid);
            }
            seats[seat].push((*seq, *uid));
        }
    }
    for s in seats.iter_mut() {
        s.sort();
    }
    for t in teams_numbered(c, roster, team_num) {
        t.assists = seats.clone();
    }
    Ok(())
}

/// `/team/equip_multiple_assist {teamNum, statueSeq, statueList:[{equipSeq, statueuId}]}`: the
/// client's diff of changed assist slots. The reply's team replaces the cached team `teamNum`.
pub fn equip_multiple_assist(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let seat = seat_of(env)?;
    let changes = changes_of(env.param.get("statueList"), "statueuId");
    let (p, r) = with_profile(ctx, env, |p| {
        match equip_assists(&mut p.collection, Roster::Story, team_num, seat, &changes) {
            Ok(()) => (Ok(()), Change::Bookkeeping),
            Err(code) => (Err(code), Change::None),
        }
    })?;
    r.map_err(ApiError::Status)?;
    ok(json!({ "teamInfo": shown(&p.collection, team_num) }))
}

/// The auto formation (`changePlan` 1 total of HP/ATK/DEF, 2 ATK, 3 DEF, 4 HP): the three best
/// owned statues by the plan, one per character, and for each seat the highest-rarity free Minds
/// that fit slots 1-4 and the Mind capacity (ours). Minds on the roster's other teams are not
/// taken; on the Arena roster neither are statues of its other teams (the 2019 guide keeps a
/// statue to one Arena team, research pack `arena.teams`). Assists stay as they were (one that
/// became a main member is no longer shown).
pub fn auto_formation(
    c: &mut Collection,
    roster: Roster,
    team_num: i64,
    plan: i64,
) -> Result<(), i64> {
    let others = || {
        roster
            .teams(c)
            .iter()
            .filter(move |t| t.team_num != team_num)
    };
    let busy_statues: std::collections::HashSet<i64> = if roster == Roster::Arena {
        others().flat_map(|t| t.slots).filter(|u| *u != 0).collect()
    } else {
        Default::default()
    };
    let busy_minds: std::collections::HashSet<i64> = others()
        .flat_map(|t| t.minds.iter().flatten().map(|(_, u)| *u))
        .collect();
    let score = |o: &collection::OwnedStatue| {
        let v = collection::statue_info(o);
        let g = |k: &str| v[k].as_i64().unwrap_or(0);
        match plan {
            2 => g("atk"),
            3 => g("def"),
            4 => g("hp"),
            _ => g("hp") + g("atk") + g("def"),
        }
    };
    let mut ranked: Vec<(i64, i64, String)> = c
        .statues
        .iter()
        .filter(|o| c.known_statue(o.uid).is_some() && !busy_statues.contains(&o.uid))
        .map(|o| {
            let chara = o.statue_id.split('_').next().unwrap_or("").to_string();
            (score(o), o.uid, chara)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut slots = [0i64; 3];
    let mut charas: Vec<String> = Vec::new();
    let mut n = 0;
    for (_, uid, chara) in &ranked {
        if n == 3 {
            break;
        }
        if charas.contains(chara) {
            continue;
        }
        charas.push(chara.clone());
        slots[n] = *uid;
        n += 1;
    }
    if n < 3 {
        return Err(E_TEAM_EMPTY_MEMBER);
    }
    // Minds: highest rarity (then level) first, each seat within slots 1-4 and capacity; skill
    // Minds are left to the player (their frames take one character's Minds only).
    let mut minds: Vec<(i64, i64, i64, i64)> = c
        .minds
        .iter()
        .filter(|m| !busy_minds.contains(&m.uid) && collection::mind_skill(&m.mind_id).is_none())
        .map(|m| {
            (
                collection::mind_rarity(&m.mind_id),
                m.level,
                collection::mind_cost(&m.mind_id),
                m.uid,
            )
        })
        .collect();
    minds.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)).then(a.3.cmp(&b.3)));
    let mut seats: [Vec<(i64, i64)>; 3] = Default::default();
    let mut taken = std::collections::HashSet::new();
    for seat in seats.iter_mut() {
        let mut cost = 0;
        for (_, _, c_, uid) in &minds {
            if seat.len() as i64 == collection::MIND_GOLDEN_SLOTS {
                break;
            }
            if taken.contains(uid) || cost + c_ > collection::MIND_CAPACITY {
                continue;
            }
            cost += c_;
            taken.insert(*uid);
            seat.push((seat.len() as i64 + 1, *uid));
        }
    }
    for t in teams_numbered(c, roster, team_num) {
        t.slots = slots;
        t.minds = seats.clone();
    }
    Ok(())
}

/// `/team/auto_set_and_equip {teamNum, changePlan}` (the auto formation popup; see
/// [`auto_formation`]).
pub fn auto_set_and_equip(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let plan = int(env, "changePlan").unwrap_or(1);
    let (p, ok_) = with_profile(ctx, env, |p| {
        match auto_formation(&mut p.collection, Roster::Story, team_num, plan) {
            Ok(()) => {
                p.progress.missions.bump("team3", 1, jdr_core::clock::now());
                (Ok(()), Change::Bookkeeping)
            }
            Err(code) => (Err(code), Change::None),
        }
    })?;
    ok_.map_err(ApiError::Status)?;
    ok(json!({ "teamInfo": shown(&p.collection, team_num) }))
}

/// `/team/equip_mind {teamNum, statueSeq, equipSeq, minduId}` (unused by 3.12, kept for safety).
pub fn equip_mind(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let seat = seat_of(env)?;
    let change = one_change(env);
    let (p, r) = with_profile(ctx, env, |p| {
        match equip(&mut p.collection, Roster::Story, team_num, seat, &[change]) {
            Ok(()) => (Ok(()), Change::Bookkeeping),
            Err(code) => (Err(code), Change::None),
        }
    })?;
    r.map_err(ApiError::Status)?;
    ok(json!({ "teamInfo": shown(&p.collection, team_num) }))
}

/// `(equipSeq, minduId)` of an EquipMindRequest / PvpEquipMindRequest.
pub fn one_change(env: &RequestEnvelope) -> (i64, i64) {
    (
        int(env, "equipSeq").unwrap_or(0),
        int(env, "minduId").unwrap_or(0),
    )
}

/// The open Mind slots (`mindSeqInfo`): the four golden frames, the skill frame (5) and the SP
/// skill frame (6) (docs/research/26 §3). The client takes each frame's kind from its seq
/// (`GameConst.GetMindSeqType`); `onlySkillFlg` is set on the two skill frames (no reader found).
pub fn mind_seq_info() -> Value {
    let seq: Vec<Value> = (1..=collection::MIND_SLOTS_OPEN)
        .map(|s| {
            let skill = s > collection::MIND_GOLDEN_SLOTS;
            json!({ "seq": s, "openFlg": 1, "goldenFlameFlg": 0, "onlySkillFlg": skill as i64 })
        })
        .collect();
    json!({ "seqInfoList": seq, "unlockInfo": { "openRank": 0, "openMaterialList": [], "itemList": [] } })
}

/// `/team/unlock_mind_seq {}`: no further slots offline; the same info.
pub fn unlock_mind_seq(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "mindSeqInfo": mind_seq_info() }))
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
            date: 20261114103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    fn slots(r: &serde_json::Map<String, Value>) -> [i64; 3] {
        let o = &r["teamInfo"]["teamOrder"];
        ["leaderStatue", "memberLeftStatue", "memberRightStatue"]
            .map(|k| o[k]["statueuId"].as_i64().unwrap())
    }

    #[test]
    fn leader_change_and_set_team() {
        let dir = std::env::temp_dir().join(format!("jdr-team-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let (r, known) = dispatch(&ctx, "/team/team_info", &env(&uuid, json!({})));
        assert!(known && r.is_ok());
        let (r, _) = dispatch(
            &ctx,
            "/team/leader_change",
            &env(&uuid, json!({"teamNum": 1, "statueSeq": 2})),
        );
        let after = slots(&r.unwrap());
        let (r, _) = dispatch(
            &ctx,
            "/team/leader_change",
            &env(&uuid, json!({"teamNum": 1, "statueSeq": 2})),
        );
        let back = slots(&r.unwrap());
        assert_eq!(
            [after[1], after[0], after[2]],
            back,
            "swapping twice restores"
        );
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        for t in p.collection.teams.iter().filter(|t| t.team_num == 1) {
            assert_eq!(t.slots, back, "both battle types follow");
        }

        let free: Vec<i64> = p.collection.statues.iter().map(|s| s.uid).take(5).collect();
        let (r, _) = dispatch(
            &ctx,
            "/team/set_team",
            &env(
                &uuid,
                json!({"teamNum": 2, "mainFlg": 1, "leaderstatueuId": free[3],
                       "memberLeftStatueuId": free[3], "memberRightStatueuId": 999_999}),
            ),
        );
        assert_eq!(
            slots(&r.unwrap()),
            [free[3], 0, 0],
            "duplicates and unknown uids dropped"
        );
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert!(p
            .collection
            .teams
            .iter()
            .all(|t| t.main == (t.team_num == 2)));
        let (r, _) = dispatch(
            &ctx,
            "/team/set_team",
            &env(&uuid, json!({"teamNum": 3, "leaderstatueuId": 0})),
        );
        assert_eq!(
            slots(&r.unwrap()),
            [0, 0, 0],
            "a team without a leader is not saved"
        );
        let (r, _) = dispatch(
            &ctx,
            "/team/leader_change",
            &env(&uuid, json!({"teamNum": 1, "statueSeq": 7})),
        );
        assert_eq!(
            slots(&r.unwrap()),
            back,
            "an out-of-range seat changes nothing"
        );
        for path in [
            "/follow/follower_list",
            "/follow/block_list",
            "/trophy/trophy_list",
            "/info/notify_list",
            "/gasha/gasha_list",
        ] {
            let (r, known) = dispatch(&ctx, path, &env(&uuid, json!({})));
            assert!(known && r.is_ok(), "{path}");
        }
        for bad in [
            json!({"teamNum": 99, "mainFlg": 1, "leaderstatueuId": free[3]}),
            json!({}),
        ] {
            let (r, _) = dispatch(&ctx, "/team/set_team", &env(&uuid, bad));
            assert!(r.is_err(), "out-of-range teamNum refused");
        }
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert!(
            p.collection.teams.iter().any(|t| t.main),
            "a main team remains"
        );

        // statue_select: a new statue into a side seat, then a team member into the leader seat
        // (swap), then emptying a side seat; the leader seat never empties.
        let t1 = |p: &crate::accounts::Profile| {
            p.collection
                .teams
                .iter()
                .find(|t| t.stage_type == 1 && t.team_num == 1)
                .unwrap()
                .slots
        };
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let before = t1(&p);
        let outsider = p
            .collection
            .statues
            .iter()
            .map(|s| s.uid)
            .find(|u| !before.contains(u))
            .unwrap();
        let sel = |seq: i64, uid: i64| {
            let (r, known) = dispatch(
                &ctx,
                "/team/statue_select",
                &env(
                    &uuid,
                    json!({"teamNum": 1, "statueSeq": seq, "statueuId": uid}),
                ),
            );
            assert!(known);
            slots(&r.unwrap())
        };
        assert_eq!(sel(2, outsider), [before[0], outsider, before[2]]);
        assert_eq!(
            sel(1, outsider),
            [outsider, before[0], before[2]],
            "swap with the leader"
        );
        assert_eq!(sel(3, 0), [outsider, before[0], 0], "a side seat may empty");
        assert_eq!(
            sel(1, 0),
            [outsider, before[0], 0],
            "the leader seat never empties"
        );
        assert_eq!(
            sel(2, 999_999),
            [outsider, before[0], 0],
            "unknown statue ignored"
        );
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        for t in p.collection.teams.iter().filter(|t| t.team_num == 1) {
            assert_eq!(
                t.slots,
                [outsider, before[0], 0],
                "both battle types follow"
            );
        }
        // An empty team (team 4) takes its first statue as the leader, not a side member.
        let (r, _) = dispatch(
            &ctx,
            "/team/statue_select",
            &env(
                &uuid,
                json!({"teamNum": 4, "statueSeq": 2, "statueuId": outsider}),
            ),
        );
        let o = &r.unwrap()["teamInfo"]["teamOrder"];
        assert_eq!(o["leaderStatue"]["statueuId"], outsider);
        assert_eq!(o["memberLeftStatue"]["statueuId"], 0);

        let (r, _) = dispatch(&ctx, "/user/user_info_mini", &env(&uuid, json!({"id": ""})));
        let card = r.unwrap();
        assert!(!card["userInfoMini"]["statueInfo"]["statueId"]
            .as_str()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn minds_equip_move_and_lock() {
        let dir = std::env::temp_dir().join(format!("jdr-minds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        call("/team/team_info", json!({})).unwrap();
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let cost = |u: i64| {
            let m = p.collection.minds.iter().find(|m| m.uid == u).unwrap();
            crate::collection::mind_cost(&m.mind_id)
        };
        let mut uids: Vec<i64> = p.collection.minds.iter().map(|m| m.uid).collect();
        uids.sort_by_key(|u| cost(*u));
        let (a, b) = (uids[0], uids[1]);
        // Two Minds on the leader of team 1.
        let r = call(
            "/team/equip_multiple_minds",
            json!({"teamNum": 1, "statueSeq": 1,
            "mindList": [{"equipSeq": 1, "minduId": a}, {"equipSeq": 2, "minduId": b}]}),
        )
        .unwrap();
        let eq = r["teamInfo"]["teamOrder"]["leaderStatue"]["equipMindList"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(
            eq.iter()
                .map(|m| (m["seq"].as_i64().unwrap(), m["minduId"].as_i64().unwrap()))
                .collect::<Vec<_>>(),
            [(1, a), (2, b)]
        );
        let p2 = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert!(
            p2.collection
                .teams
                .iter()
                .filter(|t| t.team_num == 1)
                .all(|t| t.minds[0].len() == 2),
            "both battle types follow"
        );
        // Moving one to the left seat takes it off the leader.
        if p2
            .collection
            .teams
            .iter()
            .any(|t| t.team_num == 1 && t.slots[1] != 0)
        {
            let r = call(
                "/team/equip_multiple_minds",
                json!({"teamNum": 1, "statueSeq": 2,
                "mindList": [{"equipSeq": 1, "minduId": a}]}),
            )
            .unwrap();
            let lead = r["teamInfo"]["teamOrder"]["leaderStatue"]["equipMindList"]
                .as_array()
                .unwrap()
                .len();
            assert_eq!(lead, 1);
        }
        // An ability Mind in the skill frame, an unknown Mind and too much CP are refused.
        assert!(call(
            "/team/equip_multiple_minds",
            json!({"teamNum": 1, "statueSeq": 1, "mindList": [{"equipSeq": 5, "minduId": b}]})
        )
        .is_err());
        assert!(call(
            "/team/equip_multiple_minds",
            json!({"teamNum": 1, "statueSeq": 1, "mindList": [{"equipSeq": 3, "minduId": 999999}]})
        )
        .is_err());
        let costly: Vec<i64> = uids.iter().rev().take(4).copied().collect();
        if costly.iter().map(|u| cost(*u)).sum::<i64>() > crate::collection::MIND_CAPACITY {
            let list: Vec<Value> = costly
                .iter()
                .enumerate()
                .map(|(i, u)| json!({"equipSeq": i + 1, "minduId": u}))
                .collect();
            assert!(call(
                "/team/equip_multiple_minds",
                json!({"teamNum": 1, "statueSeq": 1, "mindList": list})
            )
            .is_err());
        }
        // Remove all: every filled slot cleared.
        let r = call(
            "/team/equip_multiple_minds",
            json!({"teamNum": 1, "statueSeq": 1,
            "mindList": [{"equipSeq": 1, "minduId": 0}, {"equipSeq": 2, "minduId": 0}]}),
        )
        .unwrap();
        assert!(r["teamInfo"]["teamOrder"]["leaderStatue"]["equipMindList"]
            .as_array()
            .unwrap()
            .is_empty());
        // The skill frame (5) takes a skill Mind of the seat's character, not a golden frame
        // (docs/research/26 §3).
        let leader = r["teamInfo"]["teamOrder"]["leaderStatue"]["statueInfo"]["statueId"]
            .as_str()
            .unwrap()
            .to_string();
        let skill = crate::collection::mind_skills()
            .iter()
            .find(|m| !m.skill.special && m.statues.contains(&leader))
            .expect("a skill Mind for the starter leader");
        let su = p
            .collection
            .minds
            .iter()
            .find(|m| m.mind_id == skill.mind_id)
            .unwrap()
            .uid;
        for seq in [1, 6] {
            assert!(call(
                "/team/equip_multiple_minds",
                json!({"teamNum": 1, "statueSeq": 1, "mindList": [{"equipSeq": seq, "minduId": su}]})
            )
            .is_err());
        }
        let r = call(
            "/team/equip_multiple_minds",
            json!({"teamNum": 1, "statueSeq": 1, "mindList": [{"equipSeq": 5, "minduId": su}]}),
        )
        .unwrap();
        let eq = &r["teamInfo"]["teamOrder"]["leaderStatue"]["equipMindList"];
        assert_eq!(
            (eq[0]["seq"].as_i64(), eq[0]["mindType"].as_i64()),
            (Some(5), Some(2))
        );
        let info = call("/team/team_info", json!({})).unwrap();
        let open: Vec<i64> = info["mindSeqInfo"]["seqInfoList"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["openFlg"] == 1)
            .map(|s| s["seq"].as_i64().unwrap())
            .collect();
        assert_eq!(open, [1, 2, 3, 4, 5, 6]);
        call(
            "/team/equip_multiple_minds",
            json!({"teamNum": 1, "statueSeq": 1, "mindList": [{"equipSeq": 5, "minduId": 0}]}),
        )
        .unwrap();
        // The lock returns the full MindInfo (the client reads mindInfo.minduId).
        let r = call("/mind/protect", json!({"minduId": b, "protectedFlg": 1})).unwrap();
        assert_eq!(
            (
                r["mindInfo"]["minduId"].as_i64(),
                r["mindInfo"]["protectedFlg"].as_i64()
            ),
            (Some(b), Some(1))
        );
        let list = call("/mind/mind_list", json!({})).unwrap();
        assert!(list["mindList"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["useFlg"].is_i64()));
    }

    #[test]
    fn assists_equip_move_and_refuse() {
        let dir = std::env::temp_dir().join(format!("jdr-assists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let team = call("/team/team_info", json!({})).unwrap();
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let t = p.collection.teams.iter().find(|t| t.team_num == 1).unwrap();
        let main = t.slots;
        let free: Vec<i64> = p
            .collection
            .statues
            .iter()
            .map(|o| o.uid)
            .filter(|u| !main.contains(u) && p.collection.known_statue(*u).is_some())
            .take(2)
            .collect();
        let (a, b) = (free[0], free[1]);
        assert!(
            Value::Object(team)
                .to_string()
                .contains("\"assistStatueList\":[]"),
            "empty lists, never null"
        );

        let r = call(
            "/team/equip_multiple_assist",
            json!({"teamNum": 1, "statueSeq": 1,
                   "statueList": [{"equipSeq": 1, "statueuId": a}, {"equipSeq": 2, "statueuId": b}]}),
        )
        .unwrap();
        let lead = &r["teamInfo"]["teamOrder"]["leaderStatue"];
        let list = lead["assistStatueList"].as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            (list[0]["seq"].as_i64(), list[0]["statueuId"].as_i64()),
            (Some(1), Some(a))
        );
        assert!(
            list[0]["statueId"].as_str().is_some_and(|s| !s.is_empty()),
            "a whole StatueInfo"
        );
        // Both ability lists carry the same ids in the same order (the client's index trap).
        let ids = |k: &str| -> Vec<Value> {
            list[0][k]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["abilityId"].clone())
                .collect()
        };
        assert!(!ids("dtAssistAbilityList").is_empty());
        assert_eq!(ids("assistAbilityList"), ids("dtAssistAbilityList"));
        assert!(list[0]["dtAssistSkillInfo"].is_object());

        // Move `a` to the left seat: it leaves the leader.
        let r = call(
            "/team/equip_multiple_assist",
            json!({"teamNum": 1, "statueSeq": 2, "statueList": [{"equipSeq": 1, "statueuId": a}]}),
        )
        .unwrap();
        let order = &r["teamInfo"]["teamOrder"];
        assert_eq!(
            order["leaderStatue"]["assistStatueList"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            order["memberLeftStatue"]["assistStatueList"][0]["statueuId"],
            a
        );

        // Refused: a main member, slot 4, an unknown statue.
        for (seq, uid) in [(3, main[1]), (4, a), (3, 999_999)] {
            let bad = json!({"teamNum": 1, "statueSeq": 1,
                             "statueList": [{"equipSeq": seq, "statueuId": uid}]});
            assert!(
                call("/team/equip_multiple_assist", bad).is_err(),
                "{seq} {uid}"
            );
        }
        // Clear.
        let r = call(
            "/team/equip_multiple_assist",
            json!({"teamNum": 1, "statueSeq": 1, "statueList": [{"equipSeq": 2, "statueuId": 0}]}),
        )
        .unwrap();
        assert_eq!(
            r["teamInfo"]["teamOrder"]["leaderStatue"]["assistStatueList"],
            json!([])
        );
    }

    #[test]
    fn auto_formation_picks_three_and_equips_minds() {
        let dir = std::env::temp_dir().join(format!("jdr-auto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        call("/team/team_info", json!({})).unwrap();
        for plan in 1..=4 {
            let r = call(
                "/team/auto_set_and_equip",
                json!({"teamNum": 2, "changePlan": plan}),
            )
            .unwrap();
            let order = &r["teamInfo"]["teamOrder"];
            let seats = ["leaderStatue", "memberLeftStatue", "memberRightStatue"];
            let ids: Vec<String> = seats
                .iter()
                .map(|s| {
                    order[*s]["statueInfo"]["statueId"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect();
            let charas: std::collections::HashSet<&str> =
                ids.iter().map(|i| i.split('_').next().unwrap()).collect();
            assert_eq!(charas.len(), 3, "one statue per character: {ids:?}");
            for s in seats {
                let minds = order[s]["equipMindList"].as_array().unwrap();
                assert!(!minds.is_empty() && minds.len() <= 4);
                let cost: i64 = minds.iter().map(|m| m["cost"].as_i64().unwrap()).sum();
                assert!(cost <= crate::collection::MIND_CAPACITY);
                assert!(order[s]["assistStatueList"].is_array());
            }
        }
        // ATK plan: the leader has the highest ATK of any owned statue.
        let r = call(
            "/team/auto_set_and_equip",
            json!({"teamNum": 2, "changePlan": 2}),
        )
        .unwrap();
        let lead_atk = r["teamInfo"]["teamOrder"]["leaderStatue"]["statueInfo"]["atk"]
            .as_i64()
            .unwrap();
        let all = call("/statue/statue_list", json!({})).unwrap();
        let best = all["statueList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["atk"].as_i64().unwrap())
            .max()
            .unwrap();
        assert_eq!(lead_atk, best);
    }
}
