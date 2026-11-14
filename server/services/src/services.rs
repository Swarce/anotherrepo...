//! API dispatch: one module per backend service. Unknown routes answer OK with no fields and are
//! counted and logged, so a device log lists exactly what is still missing.

use crate::app::ApiResult;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Map, Value};

pub mod client;
pub mod common;
pub mod menu;
pub mod misc;
pub mod statue;
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
        "/user/get_loginbonus" => user::get_loginbonus(ctx, env),
        "/client/getkey" => client::getkey(ctx, env),
        "/client/getfiledata" => client::getfiledata(ctx, env),
        "/shop/product_list" => misc::product_list(ctx, env),
        "/info/check_forced_notify" => misc::check_forced_notify(ctx, env),
        "/info/event_list" => misc::event_list(ctx, env),
        "/info/banner_list" => misc::banner_list(ctx, env),
        "/info/get_menu_notify" => misc::get_menu_notify(ctx, env),
        "/statue/statue_list" => statue::statue_list(ctx, env),
        "/statue/statue_info" => statue::statue_info(ctx, env),
        "/team/team_info" => misc::team_info(ctx, env),
        "/bizarre/bizarre_resumption" => misc::bizarre_resumption(ctx, env),
        "/bizarre/get_bizarre_select" => menu::get_bizarre_select(ctx, env),
        "/clan/recommend" => menu::clan_recommend(ctx, env),
        "/present/present_list" => menu::present_list(ctx, env),
        "/follow/follow_list" => menu::follow_list(ctx, env),
        "/item/item_list" => menu::item_list(ctx, env),
        "/shop/shop_list" => menu::shop_list(ctx, env),
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
