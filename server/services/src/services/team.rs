//! `/team/*` edits and the profile card: change a team's leader, set its members or make it the
//! main team. The formation screen does not say which battle type it edits; the same team number
//! is changed for both types so action and tactical teams stay alike (My Page reads the tactical
//! one).

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

/// `teamNum` must name one of the player's teams (1..=TEAMS_PER_TYPE); anything else is refused
/// before the save is touched (a stray number must never clear the main team).
fn team_num(env: &RequestEnvelope) -> Result<i64, ApiError> {
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

fn teams_numbered(c: &mut Collection, team_num: i64) -> impl Iterator<Item = &mut Team> {
    c.teams.iter_mut().filter(move |t| t.team_num == team_num)
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
        let mut changed = false;
        for t in teams_numbered(&mut p.collection, team_num) {
            let i = (seq - 1) as usize;
            if i != 0 && t.slots[i] != 0 {
                t.slots.swap(0, i);
                changed = true;
            }
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
/// in the seat (TeamStatueRole 1 leader, 2 left, 3 right). A statue already in the team swaps
/// seats with the one it replaces; 0 empties a side seat (never the leader). Unknown statues or
/// seats change nothing and the reply shows the team as it is.
pub fn statue_select(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = team_num(env)?;
    let seq = int(env, "statueSeq").unwrap_or(0);
    let uid = int(env, "statueuId").unwrap_or(0);
    let (p, ()) = with_profile(ctx, env, |p| {
        let usable = uid == 0 || p.collection.known_statue(uid).is_some();
        if !(1..=3).contains(&seq) || !usable || (seq == 1 && uid == 0) {
            return ((), Change::None);
        }
        let i = (seq - 1) as usize;
        let mut changed = false;
        for t in teams_numbered(&mut p.collection, team_num) {
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
    let wanted = [
        int(env, "leaderstatueuId").unwrap_or(0),
        int(env, "memberLeftStatueuId").unwrap_or(0),
        int(env, "memberRightStatueuId").unwrap_or(0),
    ];
    let (p, ()) = with_profile(ctx, env, |p| {
        let mut slots = wanted.map(|u| {
            if u != 0 && p.collection.known_statue(u).is_some() {
                u
            } else {
                0
            }
        });
        // The same statue twice: keep the first.
        for i in 1..3 {
            if slots[..i].contains(&slots[i]) {
                slots[i] = 0;
            }
        }
        if slots[0] == 0 {
            return ((), Change::None);
        }
        for t in teams_numbered(&mut p.collection, team_num) {
            t.slots = slots;
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

/// `UserInfoMiniResponse` for the profile card: this player (offline there is nobody else), with
/// the tactical main team's leader as the card's statue.
pub fn user_info_mini(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    ok(json!({ "userInfoMini": crate::services::user::user_info_mini(&p) }))
}

/// `FollowerListResponse` / `BlockListResponse`: nobody, offline.
pub fn follower_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "sortType": 0, "followerList": [] }))
}

pub fn block_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "sortType": 0, "blockList": [] }))
}

/// `TrophyListResponse`: no achievements are tracked offline yet.
pub fn trophy_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "trophyList": [] }))
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

        let (r, _) = dispatch(&ctx, "/user/user_info_mini", &env(&uuid, json!({"id": ""})));
        let card = r.unwrap();
        assert!(!card["userInfoMini"]["statueInfo"]["statueId"]
            .as_str()
            .unwrap()
            .is_empty());
    }
}
