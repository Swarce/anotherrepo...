//! MyPage menu screens that are still empty offline: follow list, notices, event shops and Town
//! recommendations. Story quests live in `bizarre.rs`; the collection in `statue.rs`.

use crate::app::ApiResult;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::json;

/// `GetRecommendClanResponse`: no `clanInfo`, which the client shows as "no recommended Town
/// found" (Towns are the multiplayer guild feature; offline there is none to join).
pub fn clan_recommend(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({}))
}

/// `FollowListResponse`: nobody followed; `sortType` 0 is `FollowListSort.SortType.Default`.
pub fn follow_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "sortType": 0, "followList": [] }))
}

/// `NotifyListResponse`: no notice pages offline (the client opens these URLs in a web view).
pub fn notify_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "updateUrl": "", "failureUrl": "", "otherUrl": "",
               "updateNewFlg": 0, "failureNewFlg": 0, "otherNewFlg": 0 }))
}

/// `EventShopListResponse`: no event shops offline.
pub fn event_shop_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "eventShopList": [], "itemList": [] }))
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
        for (path, param, list_fields) in [("/follow/follow_list", json!({}), &["followList"][..])]
        {
            let (r, known) = dispatch(&ctx, path, &env(param));
            assert!(known, "{path}");
            let m = r.unwrap_or_else(|_| panic!("{path} failed"));
            for f in list_fields {
                assert_eq!(m.get(*f), Some(&json!([])), "{path} {f}");
            }
        }
    }
}
