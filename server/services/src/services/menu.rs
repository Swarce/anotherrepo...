//! MyPage menu screens for a fresh offline player: quest select, present box, follow list,
//! items and shops. Every list is empty; M4/M5 fill them from server/content and the save.

use crate::app::ApiResult;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::json;

/// `BizarreSelectResponse`: no quest banners or campaigns. `dungeonLockFlg` 1 (`BoolType.True`)
/// keeps the dungeon button locked, as for a new player (its routes are not served yet).
pub fn get_bizarre_select(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "questList": [], "eventCampaignList": [], "dungeonLockFlg": 1 }))
}

/// `GetRecommendClanResponse`: no `clanInfo`, which the client shows as "no recommended Town
/// found" (Towns are the multiplayer guild feature; offline there is none to join).
pub fn clan_recommend(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({}))
}

/// `PresentListResponse` for either tab (`receivedFlag` 0 or 1): an empty box shown as page 1 of 1.
pub fn present_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "allPresentNum": 0, "allPageNum": 1, "presentList": [] }))
}

/// `FollowListResponse`: nobody followed; `sortType` 0 is `FollowListSort.SortType.Default`.
pub fn follow_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "sortType": 0, "followList": [] }))
}

/// `ItemListResponse`: no items owned.
pub fn item_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "itemList": [] }))
}

/// `ShopListResponse`: no in-game shops or goods, nothing paid, no charge cap.
pub fn shop_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "shopList": [], "itemList": [], "pay": 0, "maxChargeDc": 0 }))
}

#[cfg(test)]
mod tests {
    use crate::services::dispatch;
    use crate::Ctx;
    use jdr_core::envelope::RequestEnvelope;
    use jdr_core::settings::Settings;
    use serde_json::{json, Value};

    fn env(param: Value) -> RequestEnvelope {
        RequestEnvelope {
            uuid: String::new(),
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

    #[test]
    fn menu_routes_are_known_and_empty() {
        let dir = std::env::temp_dir().join(format!("jdr-menu-{}", std::process::id()));
        let ctx = Ctx::new(Settings::new(dir), false);
        for (path, param, list_fields) in [
            (
                "/bizarre/get_bizarre_select",
                json!({}),
                &["questList", "eventCampaignList"][..],
            ),
            (
                "/present/present_list",
                json!({"prevNum": 1, "receivedFlag": 0}),
                &["presentList"][..],
            ),
            ("/follow/follow_list", json!({}), &["followList"][..]),
            ("/item/item_list", json!({}), &["itemList"][..]),
            ("/shop/shop_list", json!({}), &["shopList", "itemList"][..]),
        ] {
            let (r, known) = dispatch(&ctx, path, &env(param));
            assert!(known, "{path}");
            let m = r.unwrap_or_else(|_| panic!("{path} failed"));
            for f in list_fields {
                assert_eq!(m.get(*f), Some(&json!([])), "{path} {f}");
            }
        }
    }
}
