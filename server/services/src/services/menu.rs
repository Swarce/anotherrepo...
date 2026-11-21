//! MyPage menu screens that are still empty offline: follow list and notices. Story quests live
//! in `bizarre.rs`; the collection in `statue.rs`; the Town in `clan.rs`.

use crate::app::ApiResult;
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::json;

/// `FollowListResponse`: nobody followed; `sortType` 0 is `FollowListSort.SortType.Default`.
pub fn follow_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "sortType": 0, "followList": [] }))
}

/// `NotifyListResponse`: the Notices screen's three tabs (Updates, Maintenance, Other). The client
/// loads each URL as it is in its web view (`InformationUIController.LoadPage` 0x203E5F0 →
/// `BaseWebView.Load`, no prefix), so they are absolute URLs of our web module: pages of
/// collapsible posts from `content/news.json`, one per tab (web.rs `notice_page`). The Updates tab is
/// marked new until the player has opened this build's notes once (ours).
pub fn notify_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (_, unseen) = crate::services::statue::with_profile(ctx, env, |p| {
        let unseen = p.progress.notes_seen_build != jdr_core::BUILD;
        p.progress.notes_seen_build = jdr_core::BUILD;
        let change = if unseen {
            crate::accounts::Change::Bookkeeping
        } else {
            crate::accounts::Change::None
        };
        (unseen, change)
    })?;
    let web = format!("{}{}", ctx.base_url(), crate::web::WEB_PREFIX);
    ok(
        json!({ "updateUrl": format!("{web}{}", crate::web::NOTES_PAGE),
               "failureUrl": format!("{web}{}", crate::web::MAINTENANCE_PAGE),
               "otherUrl": format!("{web}{}", crate::web::ABOUT_PAGE),
               "updateNewFlg": i64::from(unseen), "failureNewFlg": 0, "otherNewFlg": 0 }),
    )
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

    /// Notices (docs/research/27-support-and-news.md): three absolute URLs of our web pages, the
    /// Updates tab new (and the Home badge lit) until this build's notes are opened once; every
    /// page loads.
    #[test]
    fn notices_point_at_the_local_build_notes() {
        let dir = std::env::temp_dir().join(format!("jdr-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str| {
            let mut e = env(json!({}));
            e.uuid = uuid.clone();
            Value::Object(dispatch(&ctx, path, &e).0.unwrap())
        };
        assert_eq!(call("/info/get_menu_notify")["unopenedNotify"], 1);
        let n = call("/info/notify_list");
        crate::services::shapes::assert_complete("Jjs.NotifyListResponse", &n);
        assert_eq!(n["updateNewFlg"], 1);
        for (k, page) in [
            ("updateUrl", crate::web::NOTES_PAGE),
            ("failureUrl", crate::web::MAINTENANCE_PAGE),
            ("otherUrl", crate::web::ABOUT_PAGE),
        ] {
            let url = n[k].as_str().unwrap();
            assert_eq!(url, format!("{}/web{page}", ctx.base_url()));
            let body = String::from_utf8(crate::web::page(page).body).unwrap();
            assert!(!body.contains("not available offline"), "{page}");
        }
        let notes = String::from_utf8(crate::web::page(crate::web::NOTES_PAGE).body).unwrap();
        assert!(notes.contains(&format!("Build {}", jdr_core::BUILD)));
        assert_eq!(call("/info/notify_list")["updateNewFlg"], 0, "opened once");
        assert_eq!(call("/info/get_menu_notify")["unopenedNotify"], 0);
    }
}
