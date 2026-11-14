//! `/user/*`: login, account creation, profile, tutorial flags.

use crate::accounts::{Change, Profile};
use crate::app::{ApiError, ApiResult};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use jdr_core::{clock, log};
use serde_json::{json, Value};

/// `LoginResponeStatusPattern`: 0 = normal login, 4 = new user (terms + create_user).
const EXIST_NORMAL: i64 = 0;
const EXIST_NEW_USER: i64 = 4;

fn date(t: i64) -> String {
    let (y, mo, d, h, mi, s) = clock::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

pub fn user_info_mini(p: &Profile) -> Value {
    // The card shows the tactical main team's leader.
    let leader = crate::collection::main_team(&p.collection, 1)
        .and_then(|t| p.collection.known_statue(t.slots[0]))
        .map(crate::collection::statue_info)
        .unwrap_or_else(|| json!({ "statueId": "" }));
    json!({
        "id": p.user_id, "username": p.username, "userRank": p.rank, "clanId": 0, "clanName": "",
        "position": 0, "comment": p.comment, "lastLoginAt": date(p.last_login_at), "followNum": 0,
        "followerNum": 0, "followStatus": 0, "statueInfo": leader, "equipMindList": [], "supportSkillId": "",
        "leaderAblityId": "", "availableAt": "", "friendPoint": 0
    })
}

/// Placeholder economy values until M4 (content-driven).
pub fn user_info(p: &Profile) -> Value {
    let now = date(clock::now());
    json!({
        "username": p.username, "userRank": p.rank.max(1), "birthday": "", "exp": p.exp,
        "nextExp": crate::collection::next_rank_exp(p.rank.max(1)),
        "clanId": 0, "clanName": "", "position": 0, "addStatus": 0,
        "staminaPt": crate::collection::ap_max(p.rank.max(1)), "staminaMax": crate::collection::ap_max(p.rank.max(1)), "staminaUseTime": now, "staminaMaxTime": now,
        "battlePoint": 5, "battlePointMax": 5, "battlePointUseTime": now, "battlePointMaxTime": now,
        "raidPoint": 3, "raidPointMax": 3, "raidPointUseTime": now, "raidPointMaxTime": now,
        "statueNum": p.collection.statues.len(), "mindNum": p.collection.minds.len(), "mindMax": crate::collection::MIND_MAX, "followNum": 0, "followMax": 50,
        "followerNum": 0, "followerMax": 50, "blockMax": 50, "comment": p.comment,
        "stoneNum": 0, "stoneNumFree": p.stone_free, "vm": p.gold, "mindGashaPoint": 0,
        "lastLoginAt": date(p.last_login_at), "soul": 0, "mentality": 0, "mentalityUseTime": now,
        "apUnlimitedFlg": 0
    })
}

fn current(ctx: &Ctx, env: &RequestEnvelope) -> Result<Profile, ApiError> {
    match ctx.accounts.get(&env.uuid) {
        Ok(Some(p)) => Ok(p),
        Ok(None) => Err(ApiError::Status(10003)), // E_SESSION_NG
        Err(e) => Err(ApiError::Internal(format!("account unreadable: {e}"))),
    }
}

pub fn login(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let model = env.param.get("model").and_then(Value::as_str).unwrap_or("");
    let os = env.param.get("os").and_then(Value::as_str).unwrap_or("");
    log::line(
        "login",
        &format!("uuid={:?} model={model:?} os={os:?}", env.uuid),
    );
    let acc = ctx.accounts.get(&env.uuid).map_err(|e| {
        // Never answer "new user" for an account we merely failed to read.
        log::line(
            "account",
            &format!("ERROR reading account {:?}: {e}", env.uuid),
        );
        ApiError::Internal(format!("account unreadable: {e}"))
    })?;
    match acc {
        None => ok(json!({
            "existStatus": EXIST_NEW_USER, "existUser": [], "buyUncompleteFlg": 0, "is_ca": 0, "ca_html": ""
        })),
        Some(before) => {
            // Every obtainable statue, and the teams (My Page draws its background and leader from
            // the tactical main team sent here). The login time alone is bookkeeping.
            let (p, ()) = with_profile(ctx, env, |p| {
                p.last_login_at = clock::now();
                ((), Change::Bookkeeping)
            })?;
            if p.collection != before.collection {
                log::line(
                    "account",
                    &format!(
                        "collection: {} statue(s), {} mind(s), {} team(s)",
                        p.collection.statues.len(),
                        p.collection.minds.len(),
                        p.collection.teams.len()
                    ),
                );
            }
            let team = crate::collection::main_team(&p.collection, 1)
                .map(|t| crate::collection::team_info(&p.collection, t))
                .unwrap_or_else(|| json!({}));
            ok(json!({
                "existStatus": EXIST_NORMAL, "existUser": [], "buyUncompleteFlg": 0, "is_ca": 0, "ca_html": "",
                "userInfoMini": user_info_mini(&p),
                "teamInfo": team
            }))
        }
    }
}

pub fn get_nonce(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "nonce": format!("{:x}", clock::now()) }))
}

/// The client signs `nonce` with a secret from its own code; offline there is nothing to protect,
/// so the signature is not checked.
pub fn create_user(ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    let tut = if ctx.skip_tutorial {
        vec![1u8; crate::TUTORIAL_FLAGS]
    } else {
        vec![]
    };
    let p = ctx.accounts.create(&tut).map_err(ApiError::Internal)?;
    log::line(
        "account",
        &format!("created user_id={} uuid={}", p.user_id, p.uuid),
    );
    ok(json!({ "user_id": p.user_id, "uuId": p.uuid }))
}

pub fn get_tutorial_step(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = current(ctx, env)?;
    let mut steps = p.tutorial_step.clone();
    if ctx.skip_tutorial {
        steps = vec![1; crate::TUTORIAL_FLAGS.max(steps.len())];
    }
    ok(json!({ "tutorialStep": steps }))
}

pub fn tutorial(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    if let Some(list) = env.param.get("tutorialStep").and_then(Value::as_array) {
        let steps: Vec<u8> = list
            .iter()
            .map(|v| (v.as_i64().unwrap_or(0) != 0) as u8)
            .collect();
        with_profile(ctx, env, |p| {
            p.tutorial_step = steps;
            ((), Change::Progress)
        })?;
    } else {
        current(ctx, env)?;
    }
    ok(json!({}))
}

pub fn user_info_api(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = current(ctx, env)?;
    ok(json!({ "userInfo": user_info(&p), "halt": 0, "backgroundId": 0 }))
}

pub fn update_username(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let name: Option<String> = env
        .param
        .get("username")
        .and_then(Value::as_str)
        .map(|n| n.chars().take(12).collect())
        .filter(|n: &String| !n.trim().is_empty());
    let (p, ()) = with_profile(ctx, env, |p| match name {
        Some(n) => {
            p.username = n;
            ((), Change::Progress)
        }
        None => ((), Change::None),
    })?;
    ok(json!({ "userInfo": user_info(&p) }))
}

pub fn get_loginbonus(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "loginBonusInfo": [] }))
}
