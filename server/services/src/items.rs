//! Items: the catalog (`server/content/items.json`, ids with an icon and a name in the player's
//! files), the JSON shapes the client reads, and the enhancement arithmetic the client also does
//! (docs/research/15-items-and-enhancement.md). Item names and details are label keys
//! (`ITEM_<id:04>_NAME` / `_DETAIL`); the icon comes from the id (`it_<id:04>`).

use serde::Deserialize;
use serde_json::{json, Value};

pub const ITEMS_JSON: &str = include_str!("../../content/items.json");
/// Memory Ring: the Ring Exchange currency (a client constant).
pub const MEMORY_RING: i64 = 31;
/// `goldRatioInteger` is divided by this (client GameConst): 100000 = gold costs as listed.
pub const GOLD_RATIO_DENOMINATOR: i64 = 100_000;
/// Fight Coin: the offline event-exchange currency for every event (the events' own currencies
/// are not recorded; ours).
pub const EVENT_COIN: i64 = 3017;
/// Fight Coins per event-quest clear (ours).
pub const EVENT_COINS_PER_CLEAR: i64 = 20;
/// Gold granted once with the starting item stock (offline sandbox; ours).
pub const STARTING_GOLD: i64 = 1_000_000;

#[derive(Debug, Clone, Deserialize)]
pub struct ItemDef {
    #[serde(rename = "itemId")]
    pub item_id: i64,
    pub category: String,
    #[serde(rename = "type")]
    pub item_type: i64,
    pub priority: i64,
    #[serde(default)]
    pub exp: i64,
    #[serde(rename = "bonusCondition", default)]
    pub bonus_condition: i64,
    #[serde(rename = "bonusRate", default)]
    pub bonus_rate: i64,
    #[serde(rename = "useGold", default)]
    pub use_gold: i64,
    #[serde(default)]
    pub stack: i64,
}

pub fn items() -> &'static [ItemDef] {
    static CELL: std::sync::OnceLock<Vec<ItemDef>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            items: Vec<ItemDef>,
        }
        serde_json::from_str::<Doc>(ITEMS_JSON)
            .map(|d| d.items)
            .expect("items.json")
    })
}

pub fn find(id: i64) -> Option<&'static ItemDef> {
    items().iter().find(|i| i.item_id == id)
}

/// The one-time starting stock (offline sandbox; amounts ours): enhancement materials, turtles,
/// growth-tree materials, keys, BP Recovery Colas and Memory Rings.
pub fn starting_stock() -> Vec<(i64, i64)> {
    items()
        .iter()
        .filter_map(|i| {
            let n = match i.category.as_str() {
                "exp" if i.stack > 0 => 10,        // turtles
                "exp" if i.bonus_rate >= 500 => 5, // platinum
                "exp" => [99, 50, 20, 10][(i.exp_tier()) as usize],
                "tree" => 300,
                "key" => 20,
                "bp" => 5,
                "ring" => 3000,
                _ => return None,
            };
            Some((i.item_id, n))
        })
        .collect()
}

impl ItemDef {
    /// Tier 0..3 (bronze..rainbow) from the EXP value.
    fn exp_tier(&self) -> i64 {
        match self.exp {
            e if e >= 64_000 => 3,
            e if e >= 16_000 => 2,
            e if e >= 4_000 => 1,
            _ => 0,
        }
    }
}

/// `ItemInfo` (item_list, shop counts).
pub fn item_info(d: &ItemDef, count: i64) -> Value {
    json!({ "itemId": d.item_id, "name": format!("ITEM_{:04}_NAME", d.item_id),
            "comment": format!("ITEM_{:04}_DETAIL", d.item_id), "icon": d.item_id.to_string(),
            "bg": 0, "flame": 0, "type": d.item_type, "value": 0, "unit": "", "sell": 0,
            "priority": d.priority, "count": count })
}

/// An ItemInfo for a reward or goods item set (any item id, catalog or not). Every string must be
/// set: reward panels build the thumbnail from `icon` (the Arena home crashed on a null `icon`).
pub fn reward_item(item_id: i64, count: i64) -> Value {
    let typ = find(item_id).map(|d| d.item_type).unwrap_or(0);
    json!({ "itemId": item_id, "name": format!("ITEM_{item_id:04}_NAME"),
            "comment": format!("ITEM_{item_id:04}_DETAIL"), "icon": format!("{item_id:04}"),
            "bg": 0, "flame": 0, "type": typ, "value": 0, "unit": "", "sell": 0, "priority": 0,
            "count": count })
}

/// `ExpItemInfo` (exp_item_list): ItemInfo plus the enhancement numbers the client computes with.
pub fn exp_item_info(d: &ItemDef, count: i64) -> Value {
    let mut v = item_info(d, count);
    for (k, x) in [
        ("stack", d.stack),
        ("exp", d.exp),
        ("bonusCondition", d.bonus_condition),
        ("bonusRate", d.bonus_rate),
        ("useGold", d.use_gold),
    ] {
        v[k] = json!(x);
    }
    v
}

/// Whether a material's bonus applies to a statue (client `CalcMaterialExp`): 2 always; 4/5/6 by
/// statue type 1/2/3; 11 type 4; 12 type 5; 13 type 4 or 5; 7/8/9 rarity 3/4/5; 3 and 10 never.
pub fn bonus_applies(cond: i64, statue_type: i64, rarity: i64) -> bool {
    match cond {
        2 => true,
        4..=6 => statue_type == cond - 3,
        11 => statue_type == 4,
        12 => statue_type == 5,
        13 => statue_type == 4 || statue_type == 5,
        7..=9 => rarity == cond - 4,
        _ => false,
    }
}

/// EXP a statue gains from `count` of a material (client arithmetic, float as the client does).
pub fn material_exp(d: &ItemDef, count: i64, statue_type: i64, rarity: i64) -> i64 {
    let base = d.exp * count;
    if bonus_applies(d.bonus_condition, statue_type, rarity) {
        (base as f32 * d.bonus_rate as f32 / 100.0) as i64
    } else {
        base
    }
}

/// Gold for `count` of a material (client `CalcNeedGold`): per item max(floor((useGold +
/// addition) × ratio / 100000), 1); boost items × (boost + 1), capped by `limit`.
pub fn material_gold(
    d: &ItemDef,
    count: i64,
    ratio: i64,
    addition: i64,
    boost: i64,
    limit: i64,
) -> i64 {
    let mut g = ((d.use_gold + addition) * ratio / GOLD_RATIO_DENOMINATOR).max(1);
    if d.stack > 0 {
        g = (g * (boost + 1)).min(limit);
    }
    g * count
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build 41: the ranges read from the item labels (docs/research/15). Town Medals are in the
    /// catalog (so presents of them are accepted); types only where an ItemType name matches the
    /// item text; profile currencies stay out.
    #[test]
    fn label_ranges_in_catalog() {
        assert_eq!(items().len(), 414);
        let kind = |id| find(id).map(|d| (d.category.as_str(), d.item_type));
        assert_eq!(kind(crate::town::MEDAL_ITEM), Some(("town", 0)));
        assert_eq!(kind(1001), Some(("ap", 0)));
        assert_eq!(kind(1533), Some(("honor", 0)));
        assert_eq!(kind(2000), Some(("event_point", 12)));
        assert_eq!(kind(2002), Some(("event", 11)));
        assert_eq!(kind(4001), Some(("exchange", 11)));
        assert_eq!(kind(4004), Some(("exchange", 0)));
        assert_eq!(kind(6001), Some(("mind_box", 15)));
        assert_eq!(kind(8749), Some(("awakening_medal", 0)));
        for id in [0, 10, 20, 40, 1100, 8605, 3031] {
            assert!(find(id).is_none(), "{id}");
        }
        assert_eq!(
            items()
                .iter()
                .filter(|d| d.category == "awakening_medal")
                .count(),
            42
        );
    }
}
