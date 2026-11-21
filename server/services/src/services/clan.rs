//! `/clan/*` (Town) and `/event/clan_member_event_point_list`: the player's Town with AI
//! members (state and rules in `town.rs`; the Town home's socket in `town_socket.rs`;
//! docs/research/24-town.md).
//!
//! Client facts (H, 3.12.0): `create` succeeds when `clanId >= 1` (the client then reads
//! `/user/user_info` and opens the Town home); `add` joins at once when `addStatus` is 2 and
//! otherwise shows the request as pending; `info` feeds the home and the member list; the
//! socket address comes from `sync_server_info`. `room_info`, `set_object`, `del_object`,
//! `all_del_object` and `set_object_comment` have no caller in 3.12.0 (objects and the
//! background are set over the socket); they answer with the Town's empty object list.

use crate::accounts::{Change, Profile};
use crate::app::{ApiError, ApiResult};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::town::{self, Applying, Me, Town, TownFields};
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

fn int(env: &RequestEnvelope, k: &str) -> Option<i64> {
    env.param.get(k).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

fn text(env: &RequestEnvelope, k: &str) -> String {
    match env.param.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Run `f` on the account after the Town's clock turned (approvals, AI days, rewards).
fn with_town<R>(
    ctx: &Ctx,
    env: &RequestEnvelope,
    f: impl FnOnce(&mut Profile, i64) -> Result<(R, bool), i64>,
) -> Result<(Profile, R), ApiError> {
    let now = jdr_core::clock::now();
    let (p, r) = with_profile(ctx, env, |p| {
        let me = p.user_id.clone();
        let ticked = p.progress.town.tick(&me, &mut p.progress.presents, now);
        match f(p, now) {
            Ok((r, changed)) => (
                Ok(r),
                if changed || ticked {
                    Change::Progress
                } else {
                    Change::None
                },
            ),
            Err(code) => (
                Err(code),
                if ticked {
                    Change::Progress
                } else {
                    Change::None
                },
            ),
        }
    })?;
    r.map(|r| (p, r)).map_err(ApiError::Status)
}

fn my_info(p: &Profile, now: i64) -> Value {
    match &p.progress.town.town {
        Some(t) => town::clan_info(t, Some(&Me::of(p)), now),
        None => json!({}),
    }
}

/// The player's Town, or `E_CLAN_NOT_AFFILIATION`.
fn mine(p: &mut Profile) -> Result<&mut Town, i64> {
    p.progress
        .town
        .town
        .as_mut()
        .ok_or(town::E_CLAN_NOT_AFFILIATION)
}

/// `GetRecommendClanResponse`: an AI Town with room (another one each game day; none while
/// the player belongs to a Town or is waiting for an answer).
pub fn recommend(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, info) = with_town(ctx, env, |p, now| {
        let st = &p.progress.town;
        if st.town.is_some() || st.applying.is_some() {
            return Ok((None, false));
        }
        let towns = town::ai_towns(now);
        let day = crate::missions::game_day(now);
        let n = towns.len() as i64;
        let t = (0..n)
            .map(|i| &towns[((day + i).rem_euclid(n)) as usize])
            .find(|t| !t.is_full())
            .map(|t| town::clan_info(t, None, now));
        Ok((t, false))
    })?;
    // The client reads `clanInfo.clanId` without a null check (OnGetRecommend 0xF64E84): no
    // recommendation is a ClanInfo with clanId 0, which shows `Clan_Osusume_Nashi`.
    ok(json!({ "clanInfo": info.unwrap_or_else(|| town::clan_info(&Town::default(), None, 0)) }))
}

/// `/clan/search` by id (`clanId`), name (`clanName`, part of it) or policy
/// (`policy, activityTime, approval`; 0 = any, M): `clanList`.
pub fn search(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, list) = with_town(ctx, env, |p, now| {
        let mut towns = town::ai_towns(now);
        if let Some(t) = &p.progress.town.town {
            towns.retain(|a| a.id != t.id);
            towns.insert(0, t.clone());
        }
        let me = Me::of(p);
        let found: Vec<Value> = if env.param.get("clanId").is_some() {
            let id = int(env, "clanId").unwrap_or(0);
            towns.iter().filter(|t| t.id == id).collect::<Vec<_>>()
        } else if env.param.get("clanName").is_some() {
            let q = text(env, "clanName").to_lowercase();
            towns
                .iter()
                .filter(|t| !q.trim().is_empty() && t.name.to_lowercase().contains(q.trim()))
                .collect()
        } else {
            let (po, ac, ap) = (
                int(env, "policy").unwrap_or(0),
                int(env, "activityTime").unwrap_or(0),
                int(env, "approval").unwrap_or(0),
            );
            towns
                .iter()
                .filter(|t| {
                    (po == 0 || t.policy == po)
                        && (ac == 0 || t.activity_time == ac)
                        && (ap == 0 || t.approval == ap)
                })
                .collect()
        }
        .into_iter()
        .map(|t| {
            let member = p.progress.town.town.as_ref().is_some_and(|m| m.id == t.id);
            town::clan_info(t, member.then_some(&me), now)
        })
        .collect();
        Ok((found, false))
    })?;
    ok(json!({ "clanList": list }))
}

/// `/clan/create`: the player founds and leads a Town (`clanId`).
pub fn create(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let fields = TownFields::from_param(&env.param);
    let (_, id) = with_town(ctx, env, |p, now| {
        if p.progress.town.town.is_some() {
            return Err(town::E_CLAN_AFFILIATION);
        }
        let mut f = fields;
        if f.name.trim().is_empty() {
            f.name = format!("{}'s Town", p.username);
        }
        let t = Town::create(&Me::of(p), f, now);
        let id = t.id;
        let st = &mut p.progress.town;
        st.join(t, &mut p.progress.presents, now);
        Ok((id, true))
    })?;
    ok(json!({ "clanId": id }))
}

/// `/clan/add {clanId}`: join an AI Town: at once when it is open (`addStatus` 2), else the
/// request waits for its Master (`addStatus` 1; approved after `APPROVAL_WAIT`, ours).
pub fn add(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let id = int(env, "clanId").unwrap_or(0);
    let (_, status) = with_town(ctx, env, |p, now| {
        if p.progress.town.town.is_some() {
            return Err(town::E_CLAN_AFFILIATION);
        }
        let t = town::ai_town(id, now).ok_or(town::E_CLAN_NOT_EXIST)?;
        if t.is_full() {
            return Err(town::E_CLAN_MEMBER_MAX);
        }
        let st = &mut p.progress.town;
        if t.approval == town::APPROVAL_REQUIRED {
            st.applying = Some(Applying {
                town: t,
                since: now,
            });
            Ok((town::ADD_APPLYING, true))
        } else {
            st.join(t, &mut p.progress.presents, now);
            Ok((town::ADD_MEMBER, true))
        }
    })?;
    ok(json!({ "addStatus": status }))
}

/// `/clan/cancel_approval`: withdraw the request.
pub fn cancel_approval(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    with_town(ctx, env, |p, _| {
        let had = p.progress.town.applying.take().is_some();
        Ok(((), had))
    })?;
    ok(json!({}))
}

/// `/clan/info {clanId}`: `clanInfo` of any Town (the player is listed in their own).
pub fn info(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, v) = with_town(ctx, env, |p, now| {
        let st = &p.progress.town;
        let id = int(env, "clanId")
            .filter(|&i| i > 0)
            .or_else(|| st.town.as_ref().map(|t| t.id))
            .unwrap_or(0);
        let me = Me::of(p);
        let t = st.find(id, now).ok_or(town::E_CLAN_NOT_EXIST)?;
        let member = st.town.as_ref().is_some_and(|m| m.id == t.id);
        Ok((town::clan_info(&t, member.then_some(&me), now), false))
    })?;
    ok(json!({ "clanInfo": v }))
}

/// `/clan/sync_server_info`: the Town socket is this server's own port.
pub fn sync_server_info(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    with_town(ctx, env, |p, _| {
        mine(p)?;
        Ok(((), false))
    })?;
    let port = ctx.port.load(std::sync::atomic::Ordering::Relaxed);
    ok(
        json!({ "ipAddress": "127.0.0.1", "port": port, "crypt_key": crate::town_socket::CRYPT_KEY }),
    )
}

/// `/clan/check_add_request`: AI players are waiting for the Master's answer.
pub fn check_add_request(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, flag) = with_town(ctx, env, |p, _| {
        let me = p.user_id.clone();
        Ok((
            p.progress
                .town
                .town
                .as_ref()
                .is_some_and(|t| t.can_manage(&me) && !t.applicants.is_empty()),
            false,
        ))
    })?;
    ok(json!({ "requestFlg": flag }))
}

fn requests(p: &Profile, now: i64) -> Vec<Value> {
    p.progress
        .town
        .town
        .as_ref()
        .map(|t| town::request_list(t, now))
        .unwrap_or_default()
}

/// `/clan/add_request_list`: the applicants (`requestList`).
pub fn add_request_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, list) = with_town(ctx, env, |p, now| Ok((requests(p, now), false)))?;
    ok(json!({ "requestList": list }))
}

fn manage(
    ctx: &Ctx,
    env: &RequestEnvelope,
    op: impl FnOnce(&mut Town, &str, i64) -> Result<(), i64>,
) -> Result<(Profile, ()), ApiError> {
    let id = text(env, "id");
    with_town(ctx, env, |p, now| {
        let me = p.user_id.clone();
        let t = mine(p)?;
        if !t.can_manage(&me) {
            return Err(town::E_CLAN_NOT_AUTHORITY);
        }
        op(t, &id, now)?;
        Ok(((), true))
    })
}

/// `/clan/add_approval {id}`: `requestList`, `clanInfo`.
pub fn add_approval(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (p, ()) = manage(ctx, env, |t, id, now| t.approve(id, now))?;
    let now = jdr_core::clock::now();
    ok(json!({ "requestList": requests(&p, now), "clanInfo": my_info(&p, now) }))
}

/// `/clan/denial_approval {id}`: `requestList`.
pub fn denial_approval(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (p, ()) = manage(ctx, env, |t, id, _| t.deny(id))?;
    ok(json!({ "requestList": requests(&p, jdr_core::clock::now()) }))
}

/// `/clan/expulsion {id}`: `clanInfo`.
pub fn expulsion(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (p, ()) = manage(ctx, env, |t, id, _| t.expel(id))?;
    ok(json!({ "clanInfo": my_info(&p, jdr_core::clock::now()) }))
}

/// `/clan/set_sub_leader {id}`: appoint or dismiss a Sub-Master (the Master only).
pub fn set_sub_leader(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let id = text(env, "id");
    let (p, ()) = with_town(ctx, env, |p, _| {
        let me = p.user_id.clone();
        let t = mine(p)?;
        if t.position_of(&me) != town::POS_LEADER {
            return Err(town::E_CLAN_NOT_AUTHORITY);
        }
        t.toggle_sub_master(&id)?;
        Ok(((), true))
    })?;
    ok(json!({ "clanInfo": my_info(&p, jdr_core::clock::now()) }))
}

/// `/clan/change_info`: the Master edits name, policy, activity time, approval, emblem and
/// comment.
pub fn change_info(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let f = TownFields::from_param(&env.param);
    let (p, ()) = with_town(ctx, env, |p, now| {
        let me = p.user_id.clone();
        let t = mine(p)?;
        if t.position_of(&me) != town::POS_LEADER {
            return Err(town::E_CLAN_NOT_AUTHORITY);
        }
        if !f.name.trim().is_empty() {
            t.name = f.name;
        }
        t.policy = f.policy;
        t.activity_time = f.activity_time;
        t.approval = f.approval;
        if !f.emblem.is_empty() {
            t.emblem = f.emblem;
        }
        t.comment = f.comment;
        let _ = now;
        Ok(((), true))
    })?;
    ok(json!({ "clanInfo": my_info(&p, jdr_core::clock::now()) }))
}

/// `/clan/secession`: leave the Town (the player's own Town closes with them; ours).
pub fn secession(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    with_town(ctx, env, |p, _| {
        let st = &mut p.progress.town;
        if st.town.take().is_none() {
            return Err(town::E_CLAN_NOT_AFFILIATION);
        }
        Ok(((), true))
    })?;
    ok(json!({}))
}

/// `/clan/invitation {id}`: nobody else to invite offline; accepted.
pub fn invitation(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({}))
}

/// `/clan/clan_object_list`: the Town's objects (`clanObjectIdList`).
pub fn clan_object_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, list) = with_town(ctx, env, |p, _| {
        Ok((
            p.progress
                .town
                .town
                .as_ref()
                .map(|t| t.objects.clone())
                .unwrap_or_default(),
            false,
        ))
    })?;
    ok(json!({ "clanObjectIdList": list }))
}

/// `/clan/clan_bg_list`: the backgrounds the Town can choose (`clanBgIdList`).
pub fn clan_bg_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, list) = with_town(ctx, env, |p, _| {
        Ok((
            p.progress
                .town
                .town
                .as_ref()
                .map(|t| town::backgrounds_for(t.level(), t.background))
                .unwrap_or_default(),
            false,
        ))
    })?;
    ok(json!({ "clanBgIdList": list }))
}

/// `/clan/donation_item_list`: Town Medals, with the points each gives.
pub fn donation_item_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    ok(json!({ "donationItemList": [donation_item(&p)] }))
}

fn donation_item(p: &Profile) -> Value {
    json!({
        "itemInfo": crate::items::reward_item(town::MEDAL_ITEM, p.collection.item_count(town::MEDAL_ITEM)),
        "donationPoint": town::MEDAL_POINTS
    })
}

/// `/clan/clan_donation {itemList [{itemId, itemNum}]}`: Town Medals become Town points (at
/// most 500 at a time and up to the last level); the gift (gold, ours) goes to the Message Box;
/// levels reached unlock objects and send their reward.
pub fn clan_donation(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let mut medals = 0;
    for it in env
        .param
        .get("itemList")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = it.get("itemId").and_then(Value::as_i64).unwrap_or(0);
        let n = it.get("itemNum").and_then(Value::as_i64).unwrap_or(0);
        if id != town::MEDAL_ITEM && n > 0 {
            return Err(ApiError::Status(town::E_CLAN_NOT_DONATION_ITEM));
        }
        medals += n.max(0);
    }
    let (p, (before_level, before_point, gold, objects)) = with_town(ctx, env, |p, now| {
        let have = p.collection.item_count(town::MEDAL_ITEM);
        let t = mine(p)?;
        let top = town::level_total(town::level_max());
        if t.point >= top {
            return Err(town::E_CLAN_LEVEL_MAX);
        }
        let room = (top - t.point + town::MEDAL_POINTS - 1) / town::MEDAL_POINTS;
        let n = medals.min(town::DONATION_MAX).min(have).min(room);
        if n <= 0 {
            return Err(town::E_CLAN_NOT_DONATION_ITEM);
        }
        let (bl, bp) = (t.level(), t.point);
        t.point = (t.point + n * town::MEDAL_POINTS).min(top);
        t.my_donation += n * town::MEDAL_POINTS;
        let mut presents = std::mem::take(&mut p.progress.presents);
        let t = p
            .progress
            .town
            .town
            .as_mut()
            .ok_or(town::E_CLAN_NOT_AFFILIATION)?;
        let objects = t.settle(&mut presents, now);
        let gold = n * town::GIFT_GOLD_PER_MEDAL;
        presents.add(
            crate::presents::DATA_MONEY,
            20,
            gold,
            "Town donation gift".into(),
            now,
        );
        p.progress.presents = presents;
        p.collection.add_item(town::MEDAL_ITEM, -n);
        Ok(((bl, bp, gold, objects), true))
    })?;
    let now = jdr_core::clock::now();
    let mut sets = vec![json!({
        "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": [], "clanObjectList": [],
        "itemList": [crate::items::reward_item(20, gold)]
    })];
    if !objects.is_empty() {
        sets.push(json!({
            "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": [], "itemList": [],
            "clanObjectList": objects.iter().map(|o| town::object_info(*o)).collect::<Vec<_>>()
        }));
    }
    ok(json!({
        "beforeDonationLevel": before_level, "beforeDonationPoint": before_point,
        "clanInfo": my_info(&p, now), "getSetItem": sets
    }))
}

/// `/event/clan_member_event_point_list`: every member with their points for the Town event
/// (no Town event offline: 0 each).
pub fn member_point_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let mut list = Vec::new();
    if let Some(t) = &p.progress.town.town {
        list.push(json!({ "userId": p.user_id, "userEventPoint": 0 }));
        for m in &t.members {
            list.push(json!({ "userId": m.id, "userEventPoint": 0 }));
        }
    }
    ok(json!({ "clanMemberList": list }))
}

/// The dead room routes (no caller in 3.12.0): an empty object list.
pub fn room_dead(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "objList": [] }))
}

#[cfg(test)]
mod tests {
    use crate::services::dispatch;
    use crate::services::shapes::assert_complete;
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

    fn setup(tag: &str) -> (Ctx, String) {
        let dir = std::env::temp_dir().join(format!("jdr-clan-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let p = ctx.accounts.create(&[]).unwrap();
        (ctx, p.uuid)
    }

    fn call(ctx: &Ctx, uuid: &str, path: &str, param: Value) -> Result<Map<String, Value>, i64> {
        let (r, known) = dispatch(ctx, path, &env(uuid, param));
        assert!(known, "{path}");
        r.map_err(|e| match e {
            crate::app::ApiError::Status(c) => c,
            crate::app::ApiError::Internal(m) => panic!("{path}: {m}"),
        })
    }

    fn walk(class: &str, m: &Map<String, Value>) {
        assert_complete(class, &Value::Object(m.clone()));
    }

    #[test]
    fn entrance_routes_without_a_town() {
        let (ctx, u) = setup("entrance");
        let r = call(&ctx, &u, "/clan/recommend", json!({})).unwrap();
        walk("Jjs.GetRecommendClanResponse", &r);
        let id = r["clanInfo"]["clanId"].as_i64().unwrap();
        for q in [
            json!({ "clanId": id }),
            json!({ "clanName": "a" }),
            json!({ "policy": 0, "activityTime": 0, "approval": 0 }),
        ] {
            let r = call(&ctx, &u, "/clan/search", q.clone()).unwrap();
            walk("Jjs.SearchClanResponse", &r);
            assert!(!r["clanList"].as_array().unwrap().is_empty(), "{q}");
        }
        let r = call(&ctx, &u, "/clan/info", json!({ "clanId": id })).unwrap();
        walk("Jjs.GetClanInfoResponse", &r);
        let r = call(&ctx, &u, "/user/user_info", json!({})).unwrap();
        walk("Jjs.UserInfoResponse", &r);
        assert_eq!(r["userInfo"]["clanId"], 0);
        let r = call(&ctx, &u, "/info/get_menu_notify", json!({})).unwrap();
        walk("Jjs.GetMenuNotifyResponse", &r);
        assert_eq!(
            call(&ctx, &u, "/clan/sync_server_info", json!({ "clanId": id })).unwrap_err(),
            crate::town::E_CLAN_NOT_AFFILIATION
        );
        let r = call(&ctx, &u, "/clan/check_add_request", json!({})).unwrap();
        walk("Jjs.CheckAddResponse", &r);
        assert_eq!(r["requestFlg"], false);
        for path in [
            "/clan/clan_object_list",
            "/clan/clan_bg_list",
            "/clan/add_request_list",
            "/clan/donation_item_list",
            "/event/clan_member_event_point_list",
        ] {
            call(&ctx, &u, path, json!({})).unwrap();
        }
    }

    #[test]
    fn create_lead_donate_and_manage() {
        let (ctx, u) = setup("create");
        let r = call(
            &ctx,
            &u,
            "/clan/create",
            json!({ "clanName": "Mine", "policy": 1,
            "activityTime": 6, "approval": 2, "emblem": "ce_0003.bytes", "comment": "hi" }),
        )
        .unwrap();
        walk("Jjs.CreateClanResponse", &r);
        let id = r["clanId"].as_i64().unwrap();
        assert!(id >= 1);
        assert_eq!(
            call(&ctx, &u, "/clan/create", json!({ "clanName": "Again" })).unwrap_err(),
            crate::town::E_CLAN_AFFILIATION
        );
        let r = call(&ctx, &u, "/user/user_info", json!({})).unwrap();
        walk("Jjs.UserInfoResponse", &r);
        assert_eq!(r["userInfo"]["clanId"], id);
        assert_eq!(r["userInfo"]["position"], crate::town::POS_LEADER);
        assert_eq!(r["userInfo"]["addStatus"], crate::town::ADD_MEMBER);
        assert_eq!(r["backgroundId"], crate::town::DEFAULT_BACKGROUND);
        let r = call(&ctx, &u, "/clan/info", json!({ "clanId": id })).unwrap();
        walk("Jjs.GetClanInfoResponse", &r);
        assert_eq!(r["clanInfo"]["clanMemberNum"], 1);
        assert_eq!(r["clanInfo"]["clanName"], "Mine");
        let r = call(&ctx, &u, "/clan/sync_server_info", json!({ "clanId": id })).unwrap();
        walk("Jjs.GetConnectServerInfoResponse", &r);
        // Applicants.
        let r = call(&ctx, &u, "/clan/check_add_request", json!({})).unwrap();
        assert_eq!(r["requestFlg"], true);
        let r = call(&ctx, &u, "/clan/add_request_list", json!({})).unwrap();
        walk("Jjs.AddListClanResponse", &r);
        let ids: Vec<String> = r["requestList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(ids.len(), 3);
        let r = call(&ctx, &u, "/clan/add_approval", json!({ "id": ids[0] })).unwrap();
        walk("Jjs.AddApprovalClanResponse", &r);
        assert_eq!(r["clanInfo"]["clanMemberNum"], 2);
        let r = call(&ctx, &u, "/clan/denial_approval", json!({ "id": ids[1] })).unwrap();
        walk("Jjs.DenialApprovalClanResponse", &r);
        assert_eq!(r["requestList"].as_array().unwrap().len(), 1);
        // Sub-Master needs Town Rank 3.
        assert_eq!(
            call(&ctx, &u, "/clan/set_sub_leader", json!({ "id": ids[0] })).unwrap_err(),
            crate::town::E_CLAN_NOT_AUTHORITY
        );
        // Donation: the welcome medals are in the Message Box; receive them first.
        let r = call(&ctx, &u, "/clan/donation_item_list", json!({})).unwrap();
        walk("Jjs.DonationItemListResponse", &r);
        assert_eq!(
            call(
                &ctx,
                &u,
                "/clan/clan_donation",
                json!({ "itemList": [{ "itemId": 41, "itemNum": 10 }] })
            )
            .unwrap_err(),
            crate::town::E_CLAN_NOT_DONATION_ITEM
        );
        call(&ctx, &u, "/present/get_all", json!({ "prevNum": 1 })).unwrap();
        let r = call(&ctx, &u, "/clan/donation_item_list", json!({})).unwrap();
        assert_eq!(
            r["donationItemList"][0]["itemInfo"]["count"],
            crate::town::WELCOME_MEDALS
        );
        let r = call(
            &ctx,
            &u,
            "/clan/clan_donation",
            json!({ "itemList": [{ "itemId": 41, "itemNum": 300 }] }),
        )
        .unwrap();
        walk("Jjs.DonationResponse", &r);
        assert_eq!(r["beforeDonationLevel"], 1);
        assert_eq!(r["beforeDonationPoint"], 0);
        assert_eq!(r["clanInfo"]["clanPoint"], 3000);
        assert_eq!(r["clanInfo"]["clanLevel"], crate::town::level_of(3000));
        assert!(
            r["getSetItem"].as_array().unwrap().len() == 2,
            "objects unlocked"
        );
        let r = call(&ctx, &u, "/clan/set_sub_leader", json!({ "id": ids[0] })).unwrap();
        walk("Jjs.SetSubMasterResponse", &r);
        let objs = call(&ctx, &u, "/clan/clan_object_list", json!({})).unwrap();
        walk("Jjs.ClanObjectListResponse", &objs);
        assert!(!objs["clanObjectIdList"].as_array().unwrap().is_empty());
        let bgs = call(&ctx, &u, "/clan/clan_bg_list", json!({})).unwrap();
        walk("Jjs.ClanBgListResponse", &bgs);
        assert_eq!(
            bgs["clanBgIdList"],
            json!([crate::town::DEFAULT_BACKGROUND])
        );
        let r = call(&ctx, &u, "/event/clan_member_event_point_list", json!({})).unwrap();
        walk("Jjs.EventClanMemberPointListResponse", &r);
        assert_eq!(r["clanMemberList"].as_array().unwrap().len(), 2);
        let r = call(
            &ctx,
            &u,
            "/clan/change_info",
            json!({ "clanName": "Renamed", "policy": 2,
            "activityTime": 1, "approval": 1, "emblem": "ce_0001.bytes", "comment": "" }),
        )
        .unwrap();
        walk("Jjs.ChangeClanInfoResponse", &r);
        assert_eq!(r["clanInfo"]["clanName"], "Renamed");
        let r = call(&ctx, &u, "/clan/expulsion", json!({ "id": ids[0] })).unwrap();
        walk("Jjs.ExplusionClanResponse", &r);
        assert_eq!(r["clanInfo"]["clanMemberNum"], 1);
        let r = call(&ctx, &u, "/info/get_menu_notify", json!({})).unwrap();
        walk("Jjs.GetMenuNotifyResponse", &r);
        let r = call(&ctx, &u, "/clan/invitation", json!({ "id": "1" })).unwrap();
        walk("Jjs.InviteClanResponse", &r);
        // The Message Box has the donation gift and the level rewards.
        let p = ctx.accounts.get(&u).unwrap().unwrap();
        assert!(p
            .progress
            .presents
            .list
            .iter()
            .any(|x| x.comment == "Town donation gift"));
        assert!(p
            .progress
            .presents
            .list
            .iter()
            .any(|x| x.comment.starts_with("Town Rank")));
        let r = call(&ctx, &u, "/clan/secession", json!({})).unwrap();
        walk("Jjs.LeaveClanResponse", &r);
        let r = call(&ctx, &u, "/user/user_info", json!({})).unwrap();
        assert_eq!(r["userInfo"]["clanId"], 0);
    }

    #[test]
    fn join_an_ai_town_open_or_by_request() {
        let (ctx, u) = setup("join");
        let now = jdr_core::clock::now();
        let towns = crate::town::ai_towns(now);
        let open = towns
            .iter()
            .find(|t| t.approval != crate::town::APPROVAL_REQUIRED)
            .unwrap();
        let closed = towns
            .iter()
            .find(|t| t.approval == crate::town::APPROVAL_REQUIRED)
            .unwrap();
        let r = call(&ctx, &u, "/clan/add", json!({ "clanId": closed.id })).unwrap();
        walk("Jjs.AddClanResponse", &r);
        assert_eq!(r["addStatus"], crate::town::ADD_APPLYING);
        let r = call(&ctx, &u, "/user/user_info", json!({})).unwrap();
        assert_eq!(r["userInfo"]["addStatus"], crate::town::ADD_APPLYING);
        assert_eq!(r["userInfo"]["clanId"], closed.id);
        assert_eq!(r["userInfo"]["position"], crate::town::POS_APPLYING);
        let r = call(&ctx, &u, "/clan/info", json!({ "clanId": closed.id })).unwrap();
        walk("Jjs.GetClanInfoResponse", &r);
        let r = call(&ctx, &u, "/clan/cancel_approval", json!({})).unwrap();
        walk("Jjs.CancelAddClanResponse", &r);
        let r = call(&ctx, &u, "/clan/add", json!({ "clanId": open.id })).unwrap();
        assert_eq!(r["addStatus"], crate::town::ADD_MEMBER);
        let r = call(&ctx, &u, "/clan/info", json!({ "clanId": open.id })).unwrap();
        walk("Jjs.GetClanInfoResponse", &r);
        assert_eq!(r["clanInfo"]["clanMemberNum"], open.members.len() + 1);
        let leader = r["clanInfo"]["clanLeaderName"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(leader, open.members[0].name);
        let r = call(&ctx, &u, "/user/user_info", json!({})).unwrap();
        assert_eq!(r["userInfo"]["position"], crate::town::POS_MEMBER);
        // A member cannot manage.
        assert_eq!(
            call(
                &ctx,
                &u,
                "/clan/expulsion",
                json!({ "id": open.members[1].id })
            )
            .unwrap_err(),
            crate::town::E_CLAN_NOT_AUTHORITY
        );
        // An AI member's card.
        let r = call(
            &ctx,
            &u,
            "/user/user_info_mini",
            json!({ "id": open.members[1].id }),
        )
        .unwrap();
        walk("Jjs.UserInfoMiniResponse", &r);
        assert_eq!(r["userInfoMini"]["username"], open.members[1].name.as_str());
        let r = call(&ctx, &u, "/clan/search", json!({ "clanId": open.id })).unwrap();
        walk("Jjs.SearchClanResponse", &r);
        // No recommendation while in a Town.
        let r = call(&ctx, &u, "/clan/recommend", json!({})).unwrap();
        walk("Jjs.GetRecommendClanResponse", &r);
        assert_eq!(r["clanInfo"]["clanId"], 0);
    }
}
