//! Items, statue enhancement with items, and the Ring Exchange shop (Build 21,
//! docs/research/15-items-and-enhancement.md).

use crate::accounts::Change;
use crate::app::{ApiError, ApiResult};
use crate::collection;
use crate::items::{self, ItemDef, MEMORY_RING};
use crate::services::ok;
use crate::services::statue::{profile, with_profile};
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// A refusal the client shows as an error dialog (and reloads its caches).
const E_NOT_ENOUGH: i64 = 50001;
/// Boost (turtle) gold cap; get_system_config sends the same value.
const BOOST_GOLD_LIMIT: i64 = 9_999_999;

fn held<'a>(
    c: &'a collection::Collection,
    cat: impl Fn(&ItemDef) -> bool + 'a,
) -> impl Iterator<Item = (&'static ItemDef, i64)> + 'a {
    items::items()
        .iter()
        .filter(move |d| cat(d))
        .map(|d| (d, c.item_count(d.item_id)))
        .filter(|(_, n)| *n > 0)
}

fn item_list_of(c: &collection::Collection) -> Vec<Value> {
    held(c, |_| true)
        .map(|(d, n)| items::item_info(d, n))
        .collect()
}

fn exp_list_of(c: &collection::Collection) -> Vec<Value> {
    held(c, |d| d.category == "exp")
        .map(|(d, n)| items::exp_item_info(d, n))
        .collect()
}

/// `ItemListResponse`: every item held.
pub fn item_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({ "itemList": item_list_of(&p.collection) }))
}

/// `ExpItemListResponse`: enhancement materials held, with the numbers the client computes with.
pub fn exp_item_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({ "itemList": exp_list_of(&p.collection) }))
}

/// `/statue/strengthening_item {statueuId, useItemList:[{itemId,count}], goldRatioInteger,
/// goldAddition}`: EXP materials add EXP (bonus on a matching statue), turtles add a boost to a
/// statue of their rarity; gold and items are taken. The client computes the same preview.
pub fn strengthening_item(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let uid = env
        .param
        .get("statueuId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let ratio = env
        .param
        .get("goldRatioInteger")
        .and_then(Value::as_i64)
        .unwrap_or(items::GOLD_RATIO_DENOMINATOR);
    let addition = env
        .param
        .get("goldAddition")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let uses: Vec<(i64, i64)> = env
        .param
        .get("useItemList")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|u| Some((u.get("itemId")?.as_i64()?, u.get("count")?.as_i64()?)))
                .filter(|(_, n)| *n > 0)
                .collect()
        })
        .unwrap_or_default();
    let (p, res) = with_profile(ctx, env, |p| {
        let Some(o) = p.collection.known_statue(uid).cloned() else {
            return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
        };
        let d = collection::find(&o.statue_id);
        let (stype, rare) = d.map(|d| (d.statue_type(), d.rarity())).unwrap_or((0, 0));
        let (mut exp, mut gold, mut boosts) = (0, 0, 0);
        for (id, n) in &uses {
            let Some(def) = items::find(*id).filter(|d| d.category == "exp") else {
                return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
            };
            if p.collection.item_count(*id) < *n || (def.stack > 0 && def.stack != rare) {
                return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
            }
            if def.stack > 0 {
                boosts += n;
            } else {
                exp += items::material_exp(def, *n, stype, rare);
            }
            gold += items::material_gold(def, *n, ratio, addition, o.boost, BOOST_GOLD_LIMIT);
        }
        if gold > p.gold {
            return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
        }
        p.gold -= gold;
        for (id, n) in &uses {
            p.collection.add_item(*id, -n);
        }
        p.collection.add_statue_exp(&[uid], exp);
        let now = jdr_core::clock::now();
        if exp > 0 {
            p.progress.missions.bump("enhance", 1, now);
        }
        if boosts > 0 {
            p.progress.missions.bump("boost", 1, now);
        }
        if let Some(s) = p.collection.statues.iter_mut().find(|s| s.uid == uid) {
            s.boost += boosts;
        }
        (Ok(()), Change::Progress)
    })?;
    res?;
    let statue = p
        .collection
        .known_statue(uid)
        .map(collection::statue_info)
        .unwrap_or(Value::Null);
    let minds: Vec<Value> = p
        .collection
        .minds
        .iter()
        .map(collection::mind_info)
        .collect();
    ok(
        json!({ "statueInfo": statue, "userInfo": crate::services::user::user_info(&p),
               "mindList": minds, "composeType": 1, "itemList": exp_list_of(&p.collection) }),
    )
}

// ---- Ring Exchange (shop 5): Memory Rings for materials (prices ours) ----

const RING_SHOP: i64 = 5;
const NO_LIMIT: &str = "2036-01-01 00:00:00";

/// (goodsId, itemId, price in rings) for the Ring Exchange.
fn ring_goods() -> Vec<(i64, i64, i64)> {
    items::items()
        .iter()
        .filter_map(|d| {
            let price = match d.category.as_str() {
                "exp" if d.stack > 0 => 300,
                "exp" if d.bonus_rate >= 500 => 500,
                "exp" => [10, 30, 100, 300][match d.exp {
                    e if e >= 64_000 => 3,
                    e if e >= 16_000 => 2,
                    e if e >= 4_000 => 1,
                    _ => 0,
                }],
                "tree" => 20,
                "key" => 200,
                "bp" => 50,
                _ => return None,
            };
            Some((50_000 + d.item_id, d.item_id, price))
        })
        .collect()
}

fn item_set(item: i64, count: i64) -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": [], "clanObjectList": [],
            "itemList": [items::reward_item(item, count)] })
}

fn shops() -> Value {
    let goods: Vec<Value> = ring_goods()
        .into_iter()
        .map(|(gid, item, price)| {
            json!({ "goodsId": gid, "baseGoodsId": gid, "saleFlg": 0, "stepGoodsId": 0, "SaleType": "3",
                "num": 1, "payType": 3, "payItemId": MEMORY_RING, "price": price, "productId": "",
                "limitNum": -1, "remainNum": -1,
                "goodsName": format!("ITEM_{item:04}_NAME"), "goodsNameReplace": "",
                "goodsDetail": format!("ITEM_{item:04}_DETAIL"), "goodsDetailReplace": "",
                "warningMessage": "", "goodsIcon": item.to_string(),
                "goodsStart": "2000-01-01 00:00:00", "goodsend": NO_LIMIT, "itemSetInfo": item_set(item, 1) })
        })
        .collect();
    json!([{ "shopId": RING_SHOP, "shopName": "SHOP_00005_NAME", "priority": 1, "displayMode": 0, "goodsList": goods }])
}

/// Pay-item counts the shop shows (Memory Rings).
fn pay_counts(c: &collection::Collection) -> Value {
    let d = items::find(MEMORY_RING);
    json!(d
        .map(|d| vec![items::item_info(d, c.item_count(MEMORY_RING))])
        .unwrap_or_default())
}

/// `ShopListResponse`: the Ring Exchange. The main shop reads `shopList[0].goodsList` without a
/// length check, so it is never empty.
pub fn shop_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(
        json!({ "shopList": shops(), "itemList": pay_counts(&p.collection), "pay": 0, "maxChargeDc": 0 }),
    )
}

/// `/shop/shop_buy {shopId, goodsId, count}`: pay rings, receive the item. `haveItemSetInfo`
/// lists what was gained (the client adds it to its counts).
pub fn shop_buy(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let gid = env
        .param
        .get("goodsId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let count = env
        .param
        .get("count")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .max(1);
    let goods = ring_goods().into_iter().find(|g| g.0 == gid);
    let (p, res) = with_profile(ctx, env, |p| match goods {
        Some((_, item, price)) if p.collection.item_count(MEMORY_RING) >= price * count => {
            p.collection.add_item(MEMORY_RING, -price * count);
            p.collection.add_item(item, count);
            (Ok(item), Change::Progress)
        }
        _ => (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None),
    })?;
    let item = res?;
    ok(
        json!({ "userInfo": crate::services::user::user_info(&p), "itemSetInfo": item_set(item, 1),
               "shopList": shops(), "itemList": pay_counts(&p.collection),
               "haveItemSetInfo": item_set(item, count) }),
    )
}

// ---- Event exchange (Shop3): Fight Coins for materials (docs/research/20-event-exchange.md) ----

/// (goodsId, itemId, price in Fight Coins): the Ring Exchange goods at the same prices (ours).
fn event_goods() -> Vec<(i64, i64, i64)> {
    ring_goods()
        .into_iter()
        .map(|(_, item, price)| (60_000 + item, item, price))
        .collect()
}

/// The currency an event's exchange takes: its own (the high-score shop's Fight Coin 2), else the
/// Fight Coin.
pub fn shop_currency(event_id: &str) -> i64 {
    match crate::services::event::currency(event_id) {
        0 => items::EVENT_COIN,
        c => c,
    }
}

fn event_shops(event_id: &str) -> Value {
    let pay = shop_currency(event_id);
    let goods: Vec<Value> = event_goods()
        .into_iter()
        .map(|(gid, item, price)| {
            json!({ "goodsId": gid, "baseGoodsId": gid, "saleFlg": 0, "stepGoodsId": 0, "SaleType": "3",
                "num": 1, "payType": 3, "payItemId": pay, "price": price, "productId": "",
                "limitNum": -1, "remainNum": -1,
                "goodsName": format!("ITEM_{item:04}_NAME"), "goodsNameReplace": "",
                "goodsDetail": format!("ITEM_{item:04}_DETAIL"), "goodsDetailReplace": "",
                "warningMessage": "", "goodsIcon": item.to_string(),
                "goodsStart": "2000-01-01 00:00:00", "goodsend": NO_LIMIT, "itemSetInfo": item_set(item, 1) })
        })
        .collect();
    // The screen reads eventShopList[0].goodsList.Count without a length check: never empty.
    json!([{ "eventId": event_id, "eventName": "", "goodsList": goods }])
}

/// The event's exchange currency as ItemInfo (Shop3 header; counts are absolute).
pub fn event_coin_list(c: &collection::Collection, event_id: &str) -> Value {
    crate::services::event::currency_list(c, shop_currency(event_id))
}

fn event_id(env: &RequestEnvelope) -> String {
    match env.param.get("eventId") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// `/shop/event_shop_list {eventId}`: one shop of goods for the event's Fight Coins.
pub fn event_shop_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(
        json!({ "eventShopList": event_shops(&event_id(env)), "itemList": event_coin_list(&p.collection, &event_id(env)) }),
    )
}

/// `/shop/event_shop_buy {eventId, goodsId, count}`: pay Fight Coins, receive the item. The client
/// sets its coin count from `itemList` and does not add `haveItemSetInfo` itself.
pub fn event_shop_buy(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let gid = env
        .param
        .get("goodsId")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let count = env
        .param
        .get("count")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .max(1);
    let goods = event_goods().into_iter().find(|g| g.0 == gid);
    let (p, res) = with_profile(ctx, env, |p| match goods {
        Some((_, item, price))
            if p.collection.item_count(shop_currency(&event_id(env))) >= price * count =>
        {
            p.collection
                .add_item(shop_currency(&event_id(env)), -price * count);
            p.collection.add_item(item, count);
            (Ok(item), Change::Progress)
        }
        _ => (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None),
    })?;
    let item = res?;
    ok(
        json!({ "userInfo": crate::services::user::user_info(&p), "itemSetInfo": item_set(item, 1),
               "eventShopList": event_shops(&event_id(env)), "itemList": event_coin_list(&p.collection, &event_id(env)),
               "haveItemSetInfo": item_set(item, count) }),
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
            date: 20261116103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    #[test]
    fn materials_enhance_and_the_ring_shop_sells() {
        let dir = std::env::temp_dir().join(format!("jdr-items-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        // Starting stock and the gold ratio the client divides by 100000.
        let exp = call("/item/exp_item_list", json!({})).unwrap();
        let list = exp["itemList"].as_array().unwrap();
        assert!(list
            .iter()
            .any(|i| i["itemId"] == 6103 && i["exp"] == 64000 && i["bonusRate"] == 120));
        assert_eq!(
            call("/mind/mind_list", json!({})).unwrap()["goldRatioInteger"],
            100000
        );
        // A Courage statue (type 1) with one rainbow Courage medal: 64000 x 1.2.
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let o = p
            .collection
            .statues
            .iter()
            .find(|o| crate::collection::find(&o.statue_id).is_some_and(|d| d.statue_type() == 1))
            .unwrap()
            .clone();
        let gold0 = p.gold;
        let r = call("/statue/strengthening_item", json!({"statueuId": o.uid,
            "useItemList": [{"itemId": 6103, "count": 1}], "goldRatioInteger": 100000, "goldAddition": 0})).unwrap();
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let s = p
            .collection
            .statues
            .iter()
            .find(|s| s.uid == o.uid)
            .unwrap();
        assert_eq!(s.exp - o.exp, 76800);
        assert_eq!(gold0 - p.gold, 6400);
        assert_eq!(p.collection.item_count(6103), 9);
        assert!(r["itemList"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["itemId"] == 6103 && i["count"] == 9));
        assert!(r["mindList"].is_array());
        // More than held, or a turtle of another rarity: refused, nothing taken.
        assert!(call(
            "/statue/strengthening_item",
            json!({"statueuId": o.uid,
            "useItemList": [{"itemId": 6103, "count": 99}], "goldRatioInteger": 100000})
        )
        .is_err());
        let rare = crate::collection::find(&o.statue_id).unwrap().rarity();
        let wrong = if rare == 3 { 6136 } else { 6135 };
        assert!(call(
            "/statue/strengthening_item",
            json!({"statueuId": o.uid,
            "useItemList": [{"itemId": wrong, "count": 1}], "goldRatioInteger": 100000})
        )
        .is_err());
        if (3..=6).contains(&rare) {
            let right = 6135 + (rare - 3);
            call(
                "/statue/strengthening_item",
                json!({"statueuId": o.uid,
                "useItemList": [{"itemId": right, "count": 1}], "goldRatioInteger": 100000}),
            )
            .unwrap();
            let p = ctx.accounts.get(&uuid).unwrap().unwrap();
            assert_eq!(
                p.collection
                    .statues
                    .iter()
                    .find(|s| s.uid == o.uid)
                    .unwrap()
                    .boost,
                1
            );
        }
        // Ring Exchange: never empty, end dates and icons the client can parse.
        let shop = call("/shop/shop_list", json!({})).unwrap();
        let goods = shop["shopList"][0]["goodsList"].as_array().unwrap().clone();
        assert!(!goods.is_empty());
        assert!(goods.iter().all(|g| g["goodsend"].is_string()
            && g["goodsIcon"].as_str().unwrap().parse::<i64>().is_ok()));
        let g = &goods[0];
        let (item, price) = (
            g["itemSetInfo"]["itemList"][0]["itemId"].as_i64().unwrap(),
            g["price"].as_i64().unwrap(),
        );
        let before = ctx.accounts.get(&uuid).unwrap().unwrap().collection;
        let b = call(
            "/shop/shop_buy",
            json!({"shopId": 5, "goodsId": g["goodsId"], "count": 2}),
        )
        .unwrap();
        let after = ctx.accounts.get(&uuid).unwrap().unwrap().collection;
        assert_eq!(after.item_count(31), before.item_count(31) - 2 * price);
        assert_eq!(after.item_count(item), before.item_count(item) + 2);
        assert_eq!(
            b["haveItemSetInfo"]["itemList"][0]["count"], 2,
            "the gain, not the total"
        );
        assert!(call(
            "/shop/shop_buy",
            json!({"shopId": 5, "goodsId": g["goodsId"], "count": 100000})
        )
        .is_err());
    }

    #[test]
    fn event_exchange_sells_for_fight_coins() {
        let dir = std::env::temp_dir().join(format!("jdr-evshop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let list = call("/shop/event_shop_list", json!({"eventId": "50035"})).unwrap();
        let goods = list["eventShopList"][0]["goodsList"]
            .as_array()
            .unwrap()
            .clone();
        assert!(
            !goods.is_empty(),
            "never empty: the screen reads [0].goodsList"
        );
        for g in &goods {
            assert!(g["goodsIcon"].as_str().unwrap().parse::<i64>().is_ok());
            assert_eq!(g["goodsend"], "2036-01-01 00:00:00");
            assert_eq!(g["payItemId"], crate::items::EVENT_COIN);
        }
        let g = &goods[0];
        let gid = g["goodsId"].as_i64().unwrap();
        let price = g["price"].as_i64().unwrap();
        let buy = json!({"eventId": "50035", "goodsId": gid, "count": 1});
        assert!(
            call("/shop/event_shop_buy", buy.clone()).is_err(),
            "no coins yet"
        );
        ctx.accounts
            .update(&uuid, |p| {
                p.collection.add_item(crate::items::EVENT_COIN, price);
                ((), crate::accounts::Change::Progress)
            })
            .unwrap();
        let r = call("/shop/event_shop_buy", buy).unwrap();
        assert_eq!(r["itemList"][0]["itemId"], crate::items::EVENT_COIN);
        assert_eq!(r["itemList"][0]["count"], 0, "absolute count after paying");
        let item = g["itemSetInfo"]["itemList"][0]["itemId"].as_i64().unwrap();
        assert_eq!(r["haveItemSetInfo"]["itemList"][0]["itemId"], item);
        assert!(!r["eventShopList"][0]["goodsList"]
            .as_array()
            .unwrap()
            .is_empty());
    }
}
