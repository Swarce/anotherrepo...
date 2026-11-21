//! Items, statue enhancement with items, and the main shop (Build 21; six shops in Build 42,
//! docs/research/15-items-and-enhancement.md).

use crate::accounts::{Change, Profile};
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

// ---- Main shop (Shop1): the six shops of the 3.12.0 client (docs/research/15 §Shops) ----
//
// Ids, names and icons are the client's: shop N has the label SHOP_000NN_NAME and the sprite
// ShopTypeIcon_000NN (ui_shop holds 1-6; ShopMenuBannerController.UpdateUI 0xBE2050 shows
// menuIcons[shopId - 1]). Goods names are the game's GOODS_/ITEM_/STATUE_/MIND_ labels; which
// goods each shop sells and every price are ours.

const NO_LIMIT: &str = "2036-01-01 00:00:00";
/// Shop ids (SHOP_000NN_NAME, ShopTypeIcon_000NN).
pub const SHOP_BUY_DC: i64 = 1;
pub const SHOP_MIND_BOX: i64 = 2;
pub const SHOP_AP: i64 = 3;
pub const SHOP_DC_EXCHANGE: i64 = 4;
pub const RING_SHOP: i64 = 5;
pub const SHOP_BP: i64 = 6;
/// Profile currencies as item ids (presents and the summon screen use the same ids).
const DC_ITEM: i64 = 10;
const GOLD_ITEM: i64 = 20;
const FRIEND_POINT_ITEM: i64 = 40;
/// Most of one good per purchase (the client caps an unlimited good at 999 too).
const MAX_PER_BUY: i64 = 999;

/// How a good is paid (`Jjs.PayType`): 3 Item with `payItemId` (Memory Rings 31, gold 20, Friend
/// Points 40, any item), or 2 "Free": Diamond Coins, which the client counts as
/// `stoneNum + stoneNumFree` under item id 10 (ItemBuyFunc lambdas 0xBE6FE4/0xBE7090). 4 Cash is
/// the store purchase (StoneBuyFuncGate) and is never sent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pay {
    Item(i64),
    Dc,
}

impl Pay {
    fn pay_type(self) -> i64 {
        match self {
            Pay::Item(_) => 3,
            Pay::Dc => 2,
        }
    }
    fn item_id(self) -> i64 {
        match self {
            Pay::Item(id) => id,
            Pay::Dc => DC_ITEM,
        }
    }
}

/// What one unit of a good gives.
#[derive(Clone, Debug, PartialEq)]
pub enum Grant {
    Item(i64, i64),
    Dc(i64),
    /// A statue: a duplicate boost (`convergence`) when owned, else a new Lv1 statue.
    Statue(String),
    /// A new Lv1 instance of a Mind.
    Mind(String),
    /// Mind Box slots: the box is already at its offline maximum, so nothing changes.
    MindBox(i64),
}

#[derive(Clone, Debug)]
pub struct Good {
    pub shop: i64,
    pub goods_id: i64,
    name: String,
    detail: String,
    icon: String,
    /// `SaleType` (convSaleType 0xBE3728): "AP_RECOVER", "MIND_BOX", "NUMBER", else normal.
    sale: &'static str,
    /// Units per purchase as the client shows them (`num`; the Mind Box check adds num x count).
    num: i64,
    pub pay: Pay,
    pub price: i64,
    pub grant: Grant,
}

/// (goodsId, itemId, price in rings) for the Ring Exchange materials (also the event exchange).
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

fn item_good(
    shop: i64,
    goods_id: i64,
    label: Option<&str>,
    item: i64,
    n: i64,
    pay: Pay,
    price: i64,
) -> Good {
    let (name, detail) = match label {
        Some(l) => (format!("{l}_NAME"), format!("{l}_DETAIL")),
        None => (
            format!("ITEM_{item:04}_NAME"),
            format!("ITEM_{item:04}_DETAIL"),
        ),
    };
    Good {
        shop,
        goods_id,
        name,
        detail,
        icon: item.to_string(),
        sale: "3",
        num: n,
        pay,
        price,
        grant: Grant::Item(item, n),
    }
}

/// A statue id as digits ("1000_01" -> 100001): goods ids and the numeric `goodsIcon`.
fn statue_num(sid: &str) -> i64 {
    sid.chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// Shop 4 statue prices in Diamond Coins by rarity (ours).
fn statue_price(rarity: i64) -> i64 {
    match rarity {
        6 => 300,
        5 => 150,
        4 => 60,
        _ => 30,
    }
}

/// Shop 5 Mind prices in Memory Rings by rarity (ours).
fn mind_price(rarity: i64) -> i64 {
    match rarity {
        4 => 300,
        3 => 100,
        2 => 30,
        _ => 10,
    }
}

/// Every good of the main shop, in shop order. Prices are ours.
pub fn catalog() -> &'static [Good] {
    static CELL: std::sync::OnceLock<Vec<Good>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let mut v = Vec::new();
        // The store's coin packs (GOODS_0001-0006, 1-90 coins), here for gold, so the coins the
        // other shops take can always be had offline. Shop 1 lists them as retail does, but the
        // client empties shop 1 unless the store is up (checkShopList 0xBD92F8), so the Diamond
        // Coin Exchange sells them too.
        let packs = |v: &mut Vec<Good>, shop: i64, base: i64| {
            for (i, n) in [1, 6, 12, 30, 60, 90].into_iter().enumerate() {
                let k = i as i64 + 1;
                v.push(Good {
                    shop,
                    goods_id: base + k,
                    name: format!("GOODS_{k:04}_NAME"),
                    detail: format!("GOODS_{k:04}_DETAIL"),
                    icon: DC_ITEM.to_string(),
                    sale: "3",
                    num: n,
                    pay: Pay::Item(GOLD_ITEM),
                    price: 10_000 * n,
                    grant: Grant::Dc(n),
                });
            }
        };
        packs(&mut v, SHOP_BUY_DC, 1_000_000);
        // 2 Expand Mind Box (GOODS_0007: 5 slots a purchase).
        v.push(Good {
            shop: SHOP_MIND_BOX,
            goods_id: 2_000_001,
            name: "GOODS_0007_NAME".into(),
            detail: "GOODS_0007_DETAIL".into(),
            icon: "6001".into(),
            sale: "MIND_BOX",
            num: 5,
            pay: Pay::Dc,
            price: 10,
            grant: Grant::MindBox(5),
        });
        // 3 Restore AP: the Sandwich (GOODS_0009). AP is always full offline, so the client
        // would refuse an AP_RECOVER good (GOODS_0008) as "over the limit"; the item is sold.
        v.push(item_good(
            SHOP_AP,
            3_000_001,
            Some("GOODS_0009"),
            1001,
            1,
            Pay::Dc,
            10,
        ));
        // 4 Diamond Coin Exchange: the coin packs for gold, expansion keys, the Rainbow Trophy
        // and turtles (GOODS_0010-0016 where a label exists), then every statue (a duplicate
        // adds a boost).
        packs(&mut v, SHOP_DC_EXCHANGE, 4_000_000);
        for (item, label, price) in [
            (1021, Some("GOODS_0010"), 5),
            (1022, Some("GOODS_0011"), 5),
            (1023, Some("GOODS_0012"), 5),
            (6115, Some("GOODS_0013"), 5),
            (6135, Some("GOODS_0014"), 3),
            (6136, Some("GOODS_0015"), 10),
            (6137, Some("GOODS_0016"), 30),
            (6138, None, 60),
        ] {
            v.push(item_good(
                SHOP_DC_EXCHANGE,
                4_000_000 + item,
                label,
                item,
                1,
                Pay::Dc,
                price,
            ));
        }
        for d in collection::statues() {
            let n = statue_num(&d.statue_id);
            if n == 0 {
                continue;
            }
            v.push(Good {
                shop: SHOP_DC_EXCHANGE,
                goods_id: 40_000_000 + n,
                name: format!("STATUE_NAME_{}", d.statue_id),
                detail: format!("STATUE_NAME_{}", d.statue_id),
                icon: n.to_string(),
                sale: "3",
                num: 1,
                pay: Pay::Dc,
                price: statue_price(d.rarity()),
                grant: Grant::Statue(d.statue_id.clone()),
            });
        }
        // 5 Memory Ring Exchange: materials, then every ability Mind (skill Minds stay with
        // their statues) as a new Lv1 copy.
        for (gid, item, price) in ring_goods() {
            v.push(item_good(
                RING_SHOP,
                gid,
                None,
                item,
                1,
                Pay::Item(MEMORY_RING),
                price,
            ));
        }
        for m in collection::minds() {
            let Ok(n) = m.mind_id.parse::<i64>() else {
                continue;
            };
            if collection::mind_skill(&m.mind_id).is_some() {
                continue;
            }
            v.push(Good {
                shop: RING_SHOP,
                goods_id: 5_000_000 + n,
                name: format!("MIND_NAME_{}", m.mind_id),
                detail: format!("MIND_TEXT_{}", m.mind_id),
                icon: n.to_string(),
                sale: "3",
                num: 1,
                pay: Pay::Item(MEMORY_RING),
                price: mind_price(m.rarity),
                grant: Grant::Mind(m.mind_id.clone()),
            });
        }
        // 6 Buy BP Recovery Cola: the 5-BP cola by one, five and twelve (GOODS_1102/1100/1101).
        for (k, label, n, price) in [
            (1, "GOODS_1102", 1, 5),
            (2, "GOODS_1100", 5, 20),
            (3, "GOODS_1101", 12, 40),
        ] {
            v.push(item_good(
                SHOP_BP,
                6_000_000 + k,
                Some(label),
                1102,
                n,
                Pay::Dc,
                price,
            ));
        }
        v
    })
}

fn grant_set(g: &Grant, count: i64) -> Value {
    let (mut items, mut statues, mut minds) = (vec![], vec![], vec![]);
    match g {
        Grant::Item(id, n) => items.push(items::reward_item(*id, n * count)),
        Grant::Dc(n) => items.push(items::reward_item(DC_ITEM, n * count)),
        Grant::MindBox(_) => {}
        Grant::Statue(sid) => statues.push(collection::statue_info(&collection::OwnedStatue {
            statue_id: sid.clone(),
            level: 1,
            ..Default::default()
        })),
        Grant::Mind(mid) => minds.extend((0..count.min(MAX_PER_BUY)).map(|_| {
            collection::mind_info(&collection::OwnedMind {
                mind_id: mid.clone(),
                level: 1,
                ..Default::default()
            })
        })),
    }
    json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": statues, "mindList": minds,
            "clanObjectList": [], "itemList": items })
}

/// A goods entry's item set for the list: the statue or Mind with its ability and level tables
/// left empty. The shop's dialogs show only the id, name, rarity and icon
/// (searchItemSetInfo 0xBE13E8), and full infos made the list about 1.5 MB.
fn display_set(g: &Grant) -> Value {
    let mut v = grant_set(g, 1);
    for (list, empty) in [
        (
            "statueList",
            &[
                ("tribeTypeList", json!([])),
                ("leaderAbilityInfo", json!({})),
                ("dtLeaderAbilityInfo", json!({})),
                ("assistAbilityList", json!([])),
                ("dtAssistSkillInfo", json!({})),
                ("dtAssistAbilityList", json!([])),
            ][..],
        ),
        (
            "mindList",
            &[
                ("levelUpTable", json!([])),
                ("dtAbilityInfo", json!({})),
                ("skillInfo", json!({})),
                ("dtSkillInfo", json!({})),
            ][..],
        ),
    ] {
        for info in v[list].as_array_mut().into_iter().flatten() {
            for (k, e) in empty {
                info[*k] = e.clone();
            }
        }
    }
    v
}

fn goods_info(g: &Good) -> Value {
    json!({ "goodsId": g.goods_id, "baseGoodsId": g.goods_id, "saleFlg": 0, "stepGoodsId": 0,
        "SaleType": g.sale, "num": g.num, "payType": g.pay.pay_type(), "payItemId": g.pay.item_id(),
        "price": g.price, "productId": "", "limitNum": -1, "remainNum": -1,
        "goodsName": g.name, "goodsNameReplace": "", "goodsDetail": g.detail, "goodsDetailReplace": "",
        "warningMessage": "", "goodsIcon": g.icon, "goodsStart": "2000-01-01 00:00:00",
        "goodsend": NO_LIMIT, "itemSetInfo": display_set(&g.grant) })
}

/// Display order (`priority`; the screen sorts by it, Shop1UIController 0xD83958): id order,
/// with Buy Diamond Coins last because the client empties it offline (no store).
const SHOP_ORDER: [i64; 6] = [
    SHOP_MIND_BOX,
    SHOP_AP,
    SHOP_DC_EXCHANGE,
    RING_SHOP,
    SHOP_BP,
    SHOP_BUY_DC,
];

/// `ShopInfo` list in display order. The screen reads `shopList[0].goodsList` without a length
/// check, so no shop is sent empty.
fn shops() -> Value {
    static CELL: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let list: Vec<Value> = SHOP_ORDER
            .iter()
            .zip(1..)
            .map(|(&id, priority)| {
                let goods: Vec<Value> = catalog()
                    .iter()
                    .filter(|g| g.shop == id)
                    .map(goods_info)
                    .collect();
                json!({ "shopId": id, "shopName": format!("SHOP_{id:05}_NAME"), "priority": priority,
                        "displayMode": 0, "goodsList": goods })
            })
            .collect();
        Value::Array(list)
    })
    .clone()
}

/// What the player holds of a currency or item.
fn balance(p: &Profile, pay: Pay) -> i64 {
    match pay {
        Pay::Dc => p.stone_free,
        Pay::Item(GOLD_ITEM) => p.gold,
        Pay::Item(FRIEND_POINT_ITEM) => p.friend_points,
        Pay::Item(id) => p.collection.item_count(id),
    }
}

/// Take `n` of a currency or item (never below zero; callers check the balance first).
pub fn spend(p: &mut Profile, pay: Pay, n: i64) {
    match pay {
        Pay::Dc => p.stone_free = (p.stone_free - n).max(0),
        Pay::Item(GOLD_ITEM) => p.gold = (p.gold - n).max(0),
        Pay::Item(FRIEND_POINT_ITEM) => p.friend_points = (p.friend_points - n).max(0),
        Pay::Item(id) => p.collection.add_item(id, -n),
    }
}

/// Pay-item counts the shop shows and checks (`ShopListResponse.itemList`, read by
/// CheckPayItemRemains 0xBE2A24): every item a good is paid with. Diamond Coins come from
/// `userInfo` instead.
fn pay_counts(p: &Profile) -> Value {
    let mut ids: Vec<i64> = catalog()
        .iter()
        .filter_map(|g| match g.pay {
            Pay::Item(id) => Some(id),
            Pay::Dc => None,
        })
        .collect();
    ids.sort_unstable();
    ids.dedup();
    json!(ids
        .into_iter()
        .map(|id| match items::find(id) {
            Some(d) => items::item_info(d, balance(p, Pay::Item(id))),
            None => items::reward_item(id, balance(p, Pay::Item(id))),
        })
        .collect::<Vec<_>>())
}

/// Give `count` units of a grant; returns what was gained as an ItemSetInfo.
fn give(p: &mut Profile, g: &Grant, count: i64, now: &str) -> Value {
    match g {
        Grant::Item(id, n) => p.collection.add_item(*id, n * count),
        Grant::Dc(n) => p.stone_free += n * count,
        Grant::MindBox(_) => {}
        Grant::Statue(sid) => {
            let c = &mut p.collection;
            let (owned, extra) = match c.statues.iter().position(|s| &s.statue_id == sid) {
                Some(i) => (i, count),
                None => {
                    let uid = c.statues.iter().map(|s| s.uid).max().unwrap_or(0) + 1;
                    c.statues.push(collection::OwnedStatue {
                        uid,
                        statue_id: sid.clone(),
                        level: 1,
                        get_date: now.to_string(),
                        ..Default::default()
                    });
                    (c.statues.len() - 1, count - 1)
                }
            };
            c.statues[owned].boost += extra;
            let info = collection::statue_info(&c.statues[owned]);
            return json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": [info], "mindList": [],
                           "clanObjectList": [], "itemList": [] });
        }
        Grant::Mind(mid) => {
            let c = &mut p.collection;
            let first = c.minds.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
            let mut infos = Vec::new();
            for k in 0..count {
                let m = collection::OwnedMind {
                    uid: first + k,
                    mind_id: mid.clone(),
                    get_date: now.to_string(),
                    level: 1,
                    ..Default::default()
                };
                infos.push(collection::mind_info(&m));
                c.minds.push(m);
            }
            return json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": infos,
                           "clanObjectList": [], "itemList": [] });
        }
    }
    grant_set(g, count)
}

/// `ShopListResponse`: the six shops and the pay items' counts.
pub fn shop_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let p = profile(ctx, env)?;
    ok(json!({ "shopList": shops(), "itemList": pay_counts(&p), "pay": 0, "maxChargeDc": 0 }))
}

/// `/shop/shop_buy {shopId, goodsId, count}`: pay by the good's payType (an item, gold, Friend
/// Points or Diamond Coins), then give items, coins, statues (a boost when owned) or new Lv1
/// Minds. `haveItemSetInfo` lists the **gain** (the client adds it to its counts).
pub fn shop_buy(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let int = |k: &str| env.param.get(k).and_then(Value::as_i64);
    let gid = int("goodsId").unwrap_or(0);
    let shop = int("shopId");
    let count = int("count").unwrap_or(1).clamp(1, MAX_PER_BUY);
    let good = catalog()
        .iter()
        .find(|g| g.goods_id == gid && shop.map_or(true, |s| s == g.shop));
    let now = ctx.server_date();
    let (p, res) = with_profile(ctx, env, |p| {
        let Some(g) = good else {
            return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
        };
        let cost = g.price * count;
        let box_full = matches!(g.grant, Grant::Mind(_))
            && p.collection.minds.len() as i64 + count > collection::MIND_MAX;
        if balance(p, g.pay) < cost || box_full {
            return (Err(ApiError::Status(E_NOT_ENOUGH)), Change::None);
        }
        spend(p, g.pay, cost);
        let gained = give(p, &g.grant, count, &now);
        (Ok((grant_set(&g.grant, 1), gained)), Change::Progress)
    })?;
    let (unit, gained) = res?;
    ok(
        json!({ "userInfo": crate::services::user::user_info(&p), "itemSetInfo": unit,
               "shopList": shops(), "itemList": pay_counts(&p), "haveItemSetInfo": gained }),
    )
}

fn item_set(item: i64, count: i64) -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": [], "clanObjectList": [],
            "itemList": [items::reward_item(item, count)] })
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
        // Ring Exchange (shop 5): never empty, end dates and icons the client can parse.
        let shop = call("/shop/shop_list", json!({})).unwrap();
        let ring = shop["shopList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["shopId"] == 5)
            .unwrap();
        let goods = ring["goodsList"].as_array().unwrap().clone();
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

    fn fresh(tag: &str) -> (Ctx, String) {
        let dir = std::env::temp_dir().join(format!("jdr-shop-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        (ctx, uuid)
    }

    fn goods_of(list: &serde_json::Map<String, Value>, shop: i64) -> Vec<Value> {
        list["shopList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["shopId"] == shop)
            .unwrap()["goodsList"]
            .as_array()
            .unwrap()
            .clone()
    }

    fn l10n_keys(file: &str) -> std::collections::HashSet<String> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../content/l10n/en/");
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(format!("{path}{file}")).unwrap())
                .unwrap();
        v["entries"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.rsplit(':').next().unwrap().to_string())
            .collect()
    }

    /// The six shops of the client (SHOP_000NN_NAME / ShopTypeIcon_000NN), each with goods the
    /// client can parse, named by labels we translate.
    #[test]
    fn main_shop_lists_the_six_client_shops() {
        let (ctx, uuid) = fresh("list");
        let list = dispatch(&ctx, "/shop/shop_list", &env(&uuid, json!({})))
            .0
            .unwrap();
        let shops = list["shopList"].as_array().unwrap();
        let ids: Vec<i64> = shops
            .iter()
            .map(|s| s["shopId"].as_i64().unwrap())
            .collect();
        assert_eq!(ids, [2, 3, 4, 5, 6, 1], "Buy Diamond Coins last");
        assert!(shops
            .iter()
            .zip(1..)
            .all(|(s, i)| s["priority"] == json!(i)));
        let menu = l10n_keys("text_menu.json");
        let content = l10n_keys("text_content.json");
        let mut gids = std::collections::HashSet::new();
        for s in shops {
            let name = s["shopName"].as_str().unwrap();
            assert_eq!(
                name,
                format!("SHOP_{:05}_NAME", s["shopId"].as_i64().unwrap())
            );
            assert!(menu.contains(name), "{name}");
            let goods = s["goodsList"].as_array().unwrap();
            assert!(!goods.is_empty(), "shop {} has goods", s["shopId"]);
            for g in goods {
                assert!(
                    gids.insert(g["goodsId"].as_i64().unwrap()),
                    "unique goodsId"
                );
                assert!(g["goodsIcon"].as_str().unwrap().parse::<i64>().is_ok());
                assert_eq!(g["goodsend"], "2036-01-01 00:00:00");
                assert_eq!(g["limitNum"], -1);
                assert!(
                    [2, 3].contains(&g["payType"].as_i64().unwrap()),
                    "never Cash (4)"
                );
                assert!(g["price"].as_i64().unwrap() > 0);
                for k in ["goodsName", "goodsDetail"] {
                    let label = g[k].as_str().unwrap();
                    if label.starts_with("GOODS_") {
                        assert!(menu.contains(label), "{label}");
                    } else if label.starts_with("ITEM_") {
                        assert!(content.contains(label), "{label}");
                    }
                }
            }
        }
        // Pay items the client counts: Memory Rings and gold.
        let pay: Vec<i64> = list["itemList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["itemId"].as_i64().unwrap())
            .collect();
        assert_eq!(pay, [20, 31]);
        // About 0.9 MB (statue_list is of the same order); kept in check.
        let size = serde_json::to_string(&list).unwrap().len();
        assert!(size < 1_200_000, "shop_list is {size} bytes");
    }

    /// Gold buys Diamond Coins (shop 1), coins buy the Sandwich (shop 3); the reply reports the
    /// gain; short coins are refused with nothing taken.
    #[test]
    fn shop_buy_pays_gold_and_coins_and_reports_the_gain() {
        let (ctx, uuid) = fresh("buy");
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let list = call("/shop/shop_list", json!({})).unwrap();
        // The coin packs: in shop 1 (emptied by the client offline) and in shop 4.
        assert_eq!(goods_of(&list, 1).len(), 6);
        let pack = goods_of(&list, 4)
            .into_iter()
            .find(|g| g["num"] == 6 && g["payItemId"] == 20)
            .unwrap();
        assert_eq!(
            (pack["payType"].clone(), pack["payItemId"].clone()),
            (json!(3), json!(20))
        );
        let price = pack["price"].as_i64().unwrap();
        let p0 = ctx.accounts.get(&uuid).unwrap().unwrap();
        let r = call(
            "/shop/shop_buy",
            json!({"shopId": 4, "goodsId": pack["goodsId"], "count": 2}),
        )
        .unwrap();
        let p1 = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert_eq!(p0.gold - p1.gold, 2 * price);
        assert_eq!(p1.stone_free - p0.stone_free, 12);
        assert_eq!(r["haveItemSetInfo"]["itemList"][0]["itemId"], 10);
        assert_eq!(r["haveItemSetInfo"]["itemList"][0]["count"], 12, "the gain");
        assert_eq!(r["userInfo"]["stoneNumFree"], p1.stone_free);
        assert!(r["itemList"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["itemId"] == 20 && i["count"] == p1.gold));
        // Coins (payType 2) for the Sandwich.
        let sandwich = &goods_of(&list, 3)[0];
        assert_eq!(sandwich["payType"], 2);
        let cost = sandwich["price"].as_i64().unwrap();
        let r = call(
            "/shop/shop_buy",
            json!({"shopId": 3, "goodsId": sandwich["goodsId"], "count": 1}),
        )
        .unwrap();
        let p2 = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert_eq!(p1.stone_free - p2.stone_free, cost);
        assert_eq!(
            p2.collection.item_count(1001),
            p1.collection.item_count(1001) + 1
        );
        assert_eq!(r["haveItemSetInfo"]["itemList"][0]["itemId"], 1001);
        // Not enough coins: refused, nothing taken.
        let big = goods_of(&list, 6)
            .into_iter()
            .max_by_key(|g| g["price"].as_i64().unwrap())
            .unwrap();
        let n = p2.stone_free / big["price"].as_i64().unwrap() + 1;
        assert!(call(
            "/shop/shop_buy",
            json!({"shopId": 6, "goodsId": big["goodsId"], "count": n})
        )
        .is_err());
        assert_eq!(
            ctx.accounts.get(&uuid).unwrap().unwrap().stone_free,
            p2.stone_free
        );
        // A good asked for under another shop's id is unknown.
        assert!(call(
            "/shop/shop_buy",
            json!({"shopId": 2, "goodsId": sandwich["goodsId"], "count": 1})
        )
        .is_err());
        // Mind Box: answers success and changes nothing (the box is at its offline maximum).
        let mb = &goods_of(&list, 2)[0];
        assert_eq!(mb["SaleType"], "MIND_BOX");
        ctx.accounts
            .update(&uuid, |p| {
                p.stone_free += mb["price"].as_i64().unwrap();
                ((), crate::accounts::Change::Progress)
            })
            .unwrap();
        let r = call(
            "/shop/shop_buy",
            json!({"shopId": 2, "goodsId": mb["goodsId"], "count": 1}),
        )
        .unwrap();
        assert_eq!(r["userInfo"]["mindMax"], crate::collection::MIND_MAX);
    }

    /// A statue bought while owned adds a boost (`convergence`), like a turtle.
    #[test]
    fn statue_bought_while_owned_adds_convergence() {
        let (ctx, uuid) = fresh("statue");
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let list = call("/shop/shop_list", json!({})).unwrap();
        let g = goods_of(&list, 4)
            .into_iter()
            .find(|g| {
                !g["itemSetInfo"]["statueList"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            })
            .unwrap();
        let sid = g["itemSetInfo"]["statueList"][0]["statueId"]
            .as_str()
            .unwrap()
            .to_string();
        let price = g["price"].as_i64().unwrap();
        assert!(
            call(
                "/shop/shop_buy",
                json!({"shopId": 4, "goodsId": g["goodsId"]})
            )
            .is_err(),
            "no coins yet"
        );
        ctx.accounts
            .update(&uuid, |p| {
                p.stone_free = 2 * price;
                ((), crate::accounts::Change::Progress)
            })
            .unwrap();
        let boost = |p: &crate::accounts::Profile| {
            p.collection
                .statues
                .iter()
                .find(|s| s.statue_id == sid)
                .map(|s| s.boost)
        };
        let p0 = ctx.accounts.get(&uuid).unwrap().unwrap();
        let before = boost(&p0).expect("owned offline");
        let n0 = p0.collection.statues.len();
        let r = call(
            "/shop/shop_buy",
            json!({"shopId": 4, "goodsId": g["goodsId"], "count": 2}),
        )
        .unwrap();
        let p1 = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert_eq!(boost(&p1), Some(before + 2));
        assert_eq!(p1.collection.statues.len(), n0, "no second copy");
        assert_eq!(p1.stone_free, 0);
        assert_eq!(
            r["haveItemSetInfo"]["statueList"][0]["convergence"],
            before + 2
        );
    }

    /// Ring Exchange Minds arrive as new Lv1 instances.
    #[test]
    fn ring_exchange_minds_are_new_lv1_copies() {
        let (ctx, uuid) = fresh("mind");
        let call = |path: &str, param: Value| dispatch(&ctx, path, &env(&uuid, param)).0;
        let list = call("/shop/shop_list", json!({})).unwrap();
        let g = goods_of(&list, 5)
            .into_iter()
            .find(|g| !g["itemSetInfo"]["mindList"].as_array().unwrap().is_empty())
            .unwrap();
        assert_eq!(g["payItemId"], 31);
        let mid = g["itemSetInfo"]["mindList"][0]["mindId"].clone();
        let p0 = ctx.accounts.get(&uuid).unwrap().unwrap();
        let r = call(
            "/shop/shop_buy",
            json!({"shopId": 5, "goodsId": g["goodsId"], "count": 2}),
        )
        .unwrap();
        let p1 = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert_eq!(p1.collection.minds.len(), p0.collection.minds.len() + 2);
        assert!(p1.collection.minds[p0.collection.minds.len()..]
            .iter()
            .all(|m| m.level == 1 && m.exp == 0 && json!(m.mind_id) == mid));
        assert_eq!(
            p0.collection.item_count(31) - p1.collection.item_count(31),
            2 * g["price"].as_i64().unwrap()
        );
        let got = r["haveItemSetInfo"]["mindList"].as_array().unwrap();
        assert_eq!(got.len(), 2);
        assert!(got.iter().all(|m| m["level"] == 1));
        assert_ne!(got[0]["minduId"], got[1]["minduId"]);
    }

    /// Item 40 is the Friend Point wallet: paying with it lowers Friend Points.
    #[test]
    fn paying_with_item_40_lowers_friend_points() {
        let (ctx, uuid) = fresh("fp");
        let mut p = ctx.accounts.get(&uuid).unwrap().unwrap();
        p.friend_points = 500;
        let pay = super::Pay::Item(40);
        assert_eq!(super::balance(&p, pay), 500);
        super::spend(&mut p, pay, 200);
        assert_eq!(p.friend_points, 300);
        assert_eq!(super::balance(&p, super::Pay::Item(20)), p.gold);
    }
}
