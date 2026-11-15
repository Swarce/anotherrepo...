//! API dispatch: one module per backend service. Unknown routes answer OK with no fields and are
//! counted and logged, so a device log lists exactly what is still missing.

use crate::app::ApiResult;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Map, Value};

pub mod bizarre;
pub mod book;
pub mod client;
pub mod common;
pub mod gasha;
pub mod item;
pub mod menu;
pub mod misc;
pub mod statue;
pub mod team;
pub mod tree;
pub mod user;

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// Returns the handler result and whether the route is known.
pub fn dispatch(ctx: &Ctx, api: &str, env: &RequestEnvelope) -> (ApiResult, bool) {
    let r = match api {
        "/common/get_version" => common::get_version(ctx, env),
        "/common/get_system_config" => misc::get_system_config(ctx, env),
        "/user/login" => user::login(ctx, env),
        "/user/get_nonce" => user::get_nonce(ctx, env),
        "/user/create_user" => user::create_user(ctx, env),
        "/user/get_tutorial_step" => user::get_tutorial_step(ctx, env),
        "/user/tutorial" => user::tutorial(ctx, env),
        "/user/user_info" => user::user_info_api(ctx, env),
        "/user/update_username" => user::update_username(ctx, env),
        "/user/update_comment" => user::update_comment(ctx, env),
        "/user/get_loginbonus" => user::get_loginbonus(ctx, env),
        "/client/getkey" => client::getkey(ctx, env),
        "/client/getfiledata" => client::getfiledata(ctx, env),
        "/shop/product_list" => misc::product_list(ctx, env),
        "/info/check_forced_notify" => misc::check_forced_notify(ctx, env),
        "/info/event_list" => misc::event_list(ctx, env),
        "/info/banner_list" => misc::banner_list(ctx, env),
        "/info/get_menu_notify" => misc::get_menu_notify(ctx, env),
        "/mind/mind_list" => statue::mind_list(ctx, env),
        "/mind/mind_info" => statue::mind_info(ctx, env),
        "/statue/statue_list" => statue::statue_list(ctx, env),
        "/statue/statue_info" => statue::statue_info(ctx, env),
        "/team/team_info" => misc::team_info(ctx, env),
        "/team/leader_change" => team::leader_change(ctx, env),
        "/team/set_team" => team::set_team(ctx, env),
        "/team/statue_select" => team::statue_select(ctx, env),
        "/user/user_info_mini" => team::user_info_mini(ctx, env),
        "/follow/follower_list" => team::follower_list(ctx, env),
        "/follow/block_list" => team::block_list(ctx, env),
        "/trophy/trophy_list" => team::trophy_list(ctx, env),
        "/bizarre/bizarre_resumption" => misc::bizarre_resumption(ctx, env),
        "/bizarre/bizarre_list" => bizarre::bizarre_list(ctx, env),
        "/bizarre/stage_list" => bizarre::stage_list(ctx, env),
        "/bizarre/bizarre_start" => bizarre::bizarre_start(ctx, env),
        "/bizarre/bizarre_end" => bizarre::bizarre_end(ctx, env),
        "/bizarre/bizarre_continue" => bizarre::bizarre_continue(ctx, env),
        "/bizarre/destruction" => bizarre::destruction(ctx, env),
        "/bizarre/check_timed_chapter" => bizarre::check_timed_chapter(ctx, env),
        "/support/support_user_s" => bizarre::support_user_s(ctx, env),
        "/bizarre/get_bizarre_select" => bizarre::get_bizarre_select(ctx, env),
        "/clan/recommend" => menu::clan_recommend(ctx, env),
        "/present/present_list" => menu::present_list(ctx, env),
        "/follow/follow_list" => menu::follow_list(ctx, env),
        "/item/item_list" => item::item_list(ctx, env),
        "/shop/shop_list" => item::shop_list(ctx, env),
        "/shop/shop_buy" => item::shop_buy(ctx, env),
        "/shop/event_shop_list" => menu::event_shop_list(ctx, env),
        "/item/exp_item_list" => item::exp_item_list(ctx, env),
        "/present/get" | "/present/get_all" => menu::present_get(ctx, env),
        "/trophy/trophy_result_get" | "/trophy/get_all_trophy_result" => {
            menu::trophy_result_get(ctx, env)
        }
        "/mind/protect"
        | "/statue/protect"
        | "/mind/save_sort"
        | "/statue/save_sort"
        | "/mind/new_flg_release"
        | "/statue/new_flg_release"
        | "/follow/save_sort" => menu::accepted(ctx, env),
        "/book/statue_list" => book::statue_list(ctx, env),
        "/book/mind_list" => book::mind_list(ctx, env),
        "/book/item_list" => book::item_list(ctx, env),
        "/book/yogo_list" => book::yogo_list(ctx, env),
        "/statue/evolution_info" => book::evolution_info(ctx, env),
        "/statue/strengthening_item" => item::strengthening_item(ctx, env),
        "/statue/strengthening" => book::statue_strengthening(ctx, env),
        "/sell/sell" => book::sell(ctx, env),
        "/info/notify_list" => menu::notify_list(ctx, env),
        "/gasha/gasha_list" => gasha::gasha_list(ctx, env),
        "/gasha/gasha_play" => gasha::gasha_play(ctx, env),
        "/tree/skill_list" => tree::skill_list(ctx, env),
        "/tree/acquisition" => tree::acquisition(ctx, env),
        _ => return (Ok(Map::new()), false),
    };
    (r, true)
}

pub(crate) fn ok(v: Value) -> ApiResult {
    Ok(obj(v))
}

#[allow(dead_code)]
pub(crate) fn empty() -> ApiResult {
    ok(json!({}))
}
