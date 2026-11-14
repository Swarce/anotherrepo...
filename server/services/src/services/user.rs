//! `/user/*`: login, account creation, profile, tutorial flags.

use crate::accounts::Profile;
use crate::app::{ApiError, ApiResult};
use crate::services::ok;
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
    json!({
        "id": p.user_id, "username": p.username, "userRank": p.rank, "clanId": 0, "clanName": "",
        "position": 0, "comment": p.comment, "lastLoginAt": date(p.last_login_at), "followNum": 0,
        "followerNum": 0, "followStatus": 0, "equipMindList": [], "supportSkillId": "",
        "leaderAblityId": "", "availableAt": "", "friendPoint": 0
    })
}

/// Placeholder economy values until M4 (content-driven).
pub fn user_info(p: &Profile) -> Value {
    let now = date(clock::now());
    json!({
        "username": p.username, "userRank": p.rank, "birthday": "", "exp": p.exp, "nextExp": 100,
        "clanId": 0, "clanName": "", "position": 0, "addStatus": 0,
        "staminaPt": 50, "staminaMax": 50, "staminaUseTime": now, "staminaMaxTime": now,
        "battlePoint": 5, "battlePointMax": 5, "battlePointUseTime": now, "battlePointMaxTime": now,
        "raidPoint": 3, "raidPointMax": 3, "raidPointUseTime": now, "raidPointMaxTime": now,
        "statueNum": 0, "mindNum": 0, "mindMax": 300, "followNum": 0, "followMax": 50,
        "followerNum": 0, "followerMax": 50, "blockMax": 50, "comment": p.comment,
        "stoneNum": 0, "stoneNumFree": p.stone_free, "vm": p.gold, "mindGashaPoint": 0,
        "lastLoginAt": date(p.last_login_at), "soul": 0, "mentality": 0, "mentalityUseTime": now,
        "apUnlimitedFlg": 0
    })
}

fn current(ctx: &Ctx, env: &RequestEnvelope) -> Result<Profile, ApiError> {
    ctx.accounts.get(&env.uuid).ok_or(ApiError::Status(10003)) // E_SESSION_NG
}

pub fn login(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let model = env.param.get("model").and_then(Value::as_str).unwrap_or("");
    let os = env.param.get("os").and_then(Value::as_str).unwrap_or("");
    log::line(
        "login",
        &format!("uuid={:?} model={model:?} os={os:?}", env.uuid),
    );
    match ctx.accounts.get(&env.uuid) {
        None => ok(json!({
            "existStatus": EXIST_NEW_USER, "existUser": [], "buyUncompleteFlg": 0, "is_ca": 0, "ca_html": ""
        })),
        Some(mut p) => {
            p.last_login_at = clock::now();
            ctx.accounts.save(&p).map_err(ApiError::Internal)?;
            ok(json!({
                "existStatus": EXIST_NORMAL, "existUser": [], "buyUncompleteFlg": 0, "is_ca": 0, "ca_html": "",
                "userInfoMini": user_info_mini(&p),
                "teamInfo": {}
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
    let mut p = current(ctx, env)?;
    if let Some(list) = env.param.get("tutorialStep").and_then(Value::as_array) {
        p.tutorial_step = list
            .iter()
            .map(|v| (v.as_i64().unwrap_or(0) != 0) as u8)
            .collect();
        ctx.accounts.save(&p).map_err(ApiError::Internal)?;
    }
    ok(json!({}))
}

pub fn user_info_api(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = current(ctx, env)?;
    ok(json!({ "userInfo": user_info(&p), "halt": 0, "backgroundId": 0 }))
}

pub fn update_username(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let mut p = current(ctx, env)?;
    if let Some(n) = env.param.get("username").and_then(Value::as_str) {
        let n: String = n.chars().take(12).collect();
        if !n.trim().is_empty() {
            p.username = n;
            ctx.accounts.save(&p).map_err(ApiError::Internal)?;
        }
    }
    ok(json!({ "userInfo": user_info(&p) }))
}

pub fn get_loginbonus(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "loginBonusInfo": [] }))
}
