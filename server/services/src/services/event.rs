//! Event point rewards (docs/research/20 §5): a point event (`eventCollectType 1`,
//! `eventShopFlg 0`) earns points from its quests (`bizarre_end.eventResult` type 1); there is no
//! claim route in 3.12, so a reward is paid into the Message Box when the points reach it and is
//! then shown as received (`getFlg 1`) in `/event/point_reward_list`.

use crate::accounts::Profile;
use crate::app::ApiResult;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// Event points per event-quest clear (ours).
pub const POINTS_PER_CLEAR: i64 = 50;

/// One point reward: the game's reward text label number, the item it gives (named by that
/// label's text) and how many (ours), and the points needed (ours).
struct Reward {
    n: i64,
    item: i64,
    count: i64,
    points: i64,
}

/// The point events: event 50045 has ten reward labels (`point_event_reward_500045_147..156`):
/// grey, blue, green and red Stone Shards, the Courage, Wisdom and Spirit stones and brooches.
fn rewards(event_id: &str) -> &'static [Reward] {
    const E50045: &[Reward] = &[
        Reward {
            n: 147,
            item: 1014,
            count: 5,
            points: 100,
        },
        Reward {
            n: 148,
            item: 1011,
            count: 5,
            points: 200,
        },
        Reward {
            n: 149,
            item: 1012,
            count: 5,
            points: 300,
        },
        Reward {
            n: 150,
            item: 1013,
            count: 5,
            points: 400,
        },
        Reward {
            n: 151,
            item: 1002,
            count: 3,
            points: 500,
        },
        Reward {
            n: 152,
            item: 1003,
            count: 3,
            points: 600,
        },
        Reward {
            n: 153,
            item: 1004,
            count: 3,
            points: 700,
        },
        Reward {
            n: 154,
            item: 1005,
            count: 1,
            points: 800,
        },
        Reward {
            n: 155,
            item: 1006,
            count: 1,
            points: 900,
        },
        Reward {
            n: 156,
            item: 1007,
            count: 1,
            points: 1000,
        },
    ];
    match event_id {
        "50045" => E50045,
        _ => &[],
    }
}

/// Whether an event is a point event (else it has the Fight Coin exchange).
pub fn is_point_event(event_id: &str) -> bool {
    !rewards(event_id).is_empty()
}

/// The label id form of an event (`50045` -> `500045`).
fn label_id(event_id: &str) -> String {
    if event_id.len() == 5 {
        format!("{}0{}", &event_id[..2], &event_id[2..])
    } else {
        event_id.to_string()
    }
}

fn item_set(item: i64, count: i64) -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": [], "clanObjectList": [],
            "itemList": [crate::items::reward_item(item, count)] })
}

pub fn points(p: &Profile, event_id: &str) -> i64 {
    p.progress.event_points.get(event_id).copied().unwrap_or(0)
}

/// Add points; every reward reached goes to the Message Box once. Returns the rewards reached
/// now, as `eventRewardList` entries.
pub fn add_points(p: &mut Profile, event_id: &str, n: i64, now: i64) -> Vec<Value> {
    let before = points(p, event_id);
    let after = before + n;
    p.progress.event_points.insert(event_id.to_string(), after);
    let mut got = Vec::new();
    for r in rewards(event_id) {
        if before < r.points && r.points <= after {
            p.progress.presents.add(
                crate::presents::DATA_ITEM,
                r.item,
                r.count,
                format!("Event point reward: {} points", r.points),
                now,
            );
            got.push(json!({ "itemSetInfo": item_set(r.item, r.count), "necessaryEventPoint": r.points }));
        }
    }
    got
}

/// `/event/point_reward_list {eventCampaignId}`.
pub fn point_reward_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    let id = match env.param.get("eventCampaignId") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    };
    let have = points(&p, &id);
    let list: Vec<Value> = rewards(&id)
        .iter()
        .map(|r| {
            json!({ "rewardTitle": format!("ITEM_{:04}_NAME", r.item), "rewardTitleReplace": "",
                    "rewardText": format!("point_event_reward_{}_{}_Personal_text", label_id(&id), r.n),
                    "rewardTextReplace": "", "necessaryEventPoint": r.points,
                    "getFlg": i64::from(have >= r.points), "itemSetInfo": item_set(r.item, r.count) })
        })
        .collect();
    ok(
        json!({ "userEventPoint": have, "clanEventPoint": 0, "userPointRewardList": list,
               "clanPointRewardList": [] }),
    )
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
            date: 20261118103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    #[test]
    fn points_reach_rewards_once_and_the_list_shows_them() {
        let dir = std::env::temp_dir().join(format!("jdr-evpt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        assert!(super::is_point_event("50045") && !super::is_point_event("50035"));
        let l = call(
            "/event/point_reward_list",
            json!({"eventCampaignId": "50045"}),
        )
        .unwrap();
        let rows = l["userPointRewardList"].as_array().unwrap().clone();
        assert_eq!(rows.len(), 10);
        assert!(rows.iter().all(|r| r["getFlg"] == 0));
        assert_eq!(
            rows[0]["rewardText"],
            "point_event_reward_500045_147_Personal_text"
        );
        assert_eq!(l["clanPointRewardList"], json!([]));
        let mut got = Vec::new();
        ctx.accounts
            .update(&uuid, |p| {
                got = super::add_points(p, "50045", 250, 0);
                got.extend(super::add_points(p, "50045", 50, 0));
                ((), crate::accounts::Change::Progress)
            })
            .unwrap();
        let reached: Vec<i64> = got
            .iter()
            .map(|g| g["necessaryEventPoint"].as_i64().unwrap())
            .collect();
        assert_eq!(
            reached,
            [100, 200, 300],
            "each reward once, as points cross it"
        );
        let l = call(
            "/event/point_reward_list",
            json!({"eventCampaignId": "50045"}),
        )
        .unwrap();
        assert_eq!(l["userEventPoint"], 300);
        let flags: Vec<i64> = l["userPointRewardList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["getFlg"].as_i64().unwrap())
            .collect();
        assert_eq!(&flags[..4], &[1, 1, 1, 0]);
        // The rewards wait in the Message Box (plus the two welcome gifts).
        let b = call(
            "/present/present_list",
            json!({"prevNum": 1, "receivedFlag": 0}),
        )
        .unwrap();
        assert_eq!(b["allPresentNum"], 5);
    }
}
