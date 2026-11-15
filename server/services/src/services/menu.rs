//! MyPage menu screens that are still empty offline: present box, follow list, items, shops and
//! Town recommendations. Story quests live in `bizarre.rs`; the collection in `statue.rs`.

use crate::app::ApiResult;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

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

/// `NotifyListResponse`: no notice pages offline (the client opens these URLs in a web view).
pub fn notify_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "updateUrl": "", "failureUrl": "", "otherUrl": "",
               "updateNewFlg": 0, "failureNewFlg": 0, "otherNewFlg": 0 }))
}

/// `ShopListResponse`: one shop with no goods yet. The main shop reads
/// `shopList[0].goodsList.Count` without a length check, so the list may not be empty.
pub fn shop_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "shopList": shops(), "itemList": [], "pay": 0, "maxChargeDc": 0 }))
}

fn shops() -> Value {
    json!([{ "shopId": 2, "shopName": "", "priority": 1, "displayMode": 0, "goodsList": [] }])
}

fn empty_item_set() -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 0, "statueList": [], "mindList": [], "itemList": [],
            "clanObjectList": [] })
}

/// `ShopBuyResponse`: nothing is for sale offline yet; answer with the unchanged state.
pub fn shop_buy(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    ok(
        json!({ "userInfo": crate::services::user::user_info(&p), "itemSetInfo": empty_item_set(),
               "shopList": shops(), "itemList": [], "haveItemSetInfo": empty_item_set() }),
    )
}

/// `EventShopListResponse`: no event shops offline.
pub fn event_shop_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "eventShopList": [], "itemList": [] }))
}

/// `ExpItemListResponse`: no EXP items yet (the enhance screen asks on entry).
pub fn exp_item_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "itemList": [] }))
}

/// `PresentGetResponse` (inbox claim): the box is empty.
pub fn present_get(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = crate::services::statue::profile(ctx, env)?;
    ok(
        json!({ "recieveFlg": 0, "allPresentNum": 0, "allPageNum": 1, "presentList": [],
               "userInfo": crate::services::user::user_info(&p), "dataList": {},
               "getPresentList": [], "originalStatueInfo": [] }),
    )
}

/// `TrophyResultGetResponse`: no achievements tracked offline.
pub fn trophy_result_get(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "itemSetInfo": empty_item_set(), "trophyList": [], "popupFlg": false }))
}

/// Book-keeping routes the menus call (protect, sort order, "new" marks): accepted, nothing to
/// store offline.
pub fn accepted(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({}))
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
                "/present/present_list",
                json!({"prevNum": 1, "receivedFlag": 0}),
                &["presentList"][..],
            ),
            ("/follow/follow_list", json!({}), &["followList"][..]),
            ("/item/item_list", json!({}), &["itemList"][..]),
            ("/shop/shop_list", json!({}), &["itemList"][..]),
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
