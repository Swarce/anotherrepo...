//! The Message Box screen (`present/*`, docs/research/18-message-box.md).

use crate::accounts::{Change, Profile};
use crate::app::{ApiError, ApiResult};
use crate::presents::{self, Gift};
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

fn int(env: &RequestEnvelope, k: &str) -> Option<i64> {
    env.param.get(k).and_then(Value::as_i64)
}

/// Apply a received present to the account.
pub fn apply(p: &mut Profile, g: &Gift) {
    match g.data_type {
        presents::DATA_MONEY => p.gold += g.count,
        presents::DATA_STONE => p.stone_free += g.count,
        presents::DATA_FRIEND_POINT => p.friend_points += g.count,
        presents::DATA_ITEM
            if crate::items::find(g.data_id).is_some() || g.data_id == crate::town::MEDAL_ITEM =>
        {
            p.collection.add_item(g.data_id, g.count)
        }
        _ => {}
    }
}

/// Put a mission reward in the box. The comment names the mission by its title label.
pub fn send_mission_reward(p: &mut Profile, m: &crate::missions::MissionDef, now: i64) {
    let (data_type, data_id) = presents::reward_kind(m.reward.item_id);
    p.progress.presents.add(
        data_type,
        data_id,
        m.reward.count,
        format!("Mission reward: {{{}}}", m.text),
        now,
    );
}

/// `PresentListResponse` for a tab (`receivedFlag` 0 waiting, 1 received) and a 1-based page
/// (`prevNum`).
pub fn present_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let tab = int(env, "receivedFlag").unwrap_or(0);
    let page = int(env, "prevNum").unwrap_or(1);
    ok(p.progress.presents.page(tab, page))
}

/// `/present/get {presentuId, prevNum}` and `/present/get_all {prevNum}`. The client redraws
/// the box from `presentList` (the refreshed waiting tab) and the header from `userInfo`;
/// `getPresentList` feeds the "received" dialog of get_all.
pub fn present_get(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let uid = int(env, "presentuId");
    let page = int(env, "prevNum").unwrap_or(1);
    let now = jdr_core::clock::now();
    let (p, got) = with_profile(ctx, env, |p| match p.progress.presents.receive(uid, now) {
        Ok(gifts) => {
            for g in &gifts {
                apply(p, g);
            }
            let change = if gifts.is_empty() {
                Change::None
            } else {
                Change::Progress
            };
            (Ok(gifts), change)
        }
        Err(code) => (Err(code), Change::None),
    })?;
    let gifts = got.map_err(ApiError::Status)?;
    let mut m = p.progress.presents.page(0, page);
    let history: Vec<Value> = gifts.iter().map(presents::gift_info).collect();
    for (k, v) in [
        ("recieveFlg", json!(0)),
        ("userInfo", crate::services::user::user_info(&p)),
        (
            "dataList",
            json!({ "statueList": [], "mindList": [], "itemList": [] }),
        ),
        (
            "getPresentList",
            if uid.is_some() {
                json!([])
            } else {
                json!(history)
            },
        ),
        ("originalStatueInfo", json!([])),
    ] {
        m[k] = v;
    }
    ok(m)
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
            date: 20261116103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    #[test]
    fn welcome_gift_and_claims() {
        let dir = std::env::temp_dir().join(format!("jdr-present-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;

        let notify = call("/info/get_menu_notify", json!({})).unwrap();
        assert_eq!(
            notify["unopenedPresent"], 2,
            "the welcome gifts wait in the box"
        );
        let list = call(
            "/present/present_list",
            json!({"prevNum": 1, "receivedFlag": 0}),
        )
        .unwrap();
        assert_eq!(
            (list["allPresentNum"].as_i64(), list["allPageNum"].as_i64()),
            (Some(2), Some(1))
        );
        let rows = list["presentList"].as_array().unwrap().clone();
        for r in &rows {
            assert!(
                r["dataId"].as_str().unwrap().parse::<i64>().is_ok(),
                "dataId parses: {r}"
            );
            assert_eq!(r["notify"], 0);
            assert_eq!(r["indefinitely"], 1);
        }
        let gold_row = rows.iter().find(|r| r["dataType"] == 5).unwrap();
        let gold0 = call("/user/user_info", json!({})).unwrap()["userInfo"]["vm"]
            .as_i64()
            .unwrap();

        let one = json!({"presentuId": gold_row["presentuId"], "prevNum": 1});
        let got = call("/present/get", one.clone()).unwrap();
        assert_eq!(got["recieveFlg"], 0);
        assert_eq!(
            got["allPresentNum"], 1,
            "the received row left the waiting tab"
        );
        assert_eq!(got["originalStatueInfo"], json!([]));
        assert_eq!(got["userInfo"]["vm"].as_i64(), Some(gold0 + 100_000));
        assert!(
            call("/present/get", one).is_err(),
            "a present is received once"
        );

        let all = call("/present/get_all", json!({"prevNum": 1})).unwrap();
        assert_eq!(all["getPresentList"].as_array().unwrap().len(), 1);
        assert_eq!(all["presentList"], json!([]));
        let hist = call(
            "/present/present_list",
            json!({"prevNum": 1, "receivedFlag": 1}),
        )
        .unwrap();
        assert_eq!(hist["allPresentNum"], 2);
        assert_eq!(hist["presentList"][0]["notify"], 1);
        assert_eq!(
            call("/info/get_menu_notify", json!({})).unwrap()["unopenedPresent"],
            0
        );
    }
}
