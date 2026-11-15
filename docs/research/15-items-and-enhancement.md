# 15 — Items, enhancement with items, Ring Exchange (Build 21)

Read in the 3.12.0 client (H read in code, M inferred).

## Items
- **No item master table (H).** `item/item_list` and `item/exp_item_list` carry everything.
  - `ItemInfo {itemId, name, comment, icon, bg, flame, type, value, unit, sell, priority, count}`.
  - `ExpItemInfo` adds `stack, exp, bonusCondition, bonusRate, useGold`.
  - `name`/`comment` are label keys (`ITEM_<id:04>_NAME` / `_DETAIL`). The icon is built from the
    id (`it_<id:04>` in `ui_item_00`); the `icon` string is not read.
- **Which ids** (`tools/content/build_items.py` → `server/content/items.json`). An id is listed
  when it has both an icon and a name in the player's files: 327 items. Categories come from the
  client's id ranges:

  | Ids | Category |
  |---|---|
  | 31 | Memory Ring |
  | 50–289 | tickets |
  | 300–306 | summon medals |
  | 1002–1050, 1200–1202 | growth-tree materials (keys 1021–23, 1038, 1047, 1048) |
  | 3000–3184 | event exchange items |
  | 6100–6138 | enhancement materials |

- **Enhancement materials.**
  - Medals per attribute: Courage 6100–03, Wisdom 6104–07, Spirit 6108–11, Unity 6120–23,
    Solitude 6124–27. Bonus condition 4/5/6/11/12, rate 120 (×1.2, wiki).
  - Trophies 6112–15 (condition 2: any statue). Mind medals 6116–19 (Minds only).
  - Platinum 6128–34 (×5, from the item text).
  - Turtles 6135–38: a boost item (`stack` = rarity 3–6). The client allows one only when the
    statue's rarity equals `stack` (H).
  - EXP per tier: bronze 1,000, silver 4,000, rainbow 64,000 (wiki); gold 16,000 (ours). All gold
    costs are ours.
- **Starting stock** (once per account, offline sandbox, amounts ours): medals 99/50/20/10 per
  tier, 5 platinum and 10 turtles of each kind, 300 of each tree material, 20 keys, 3,000 Memory
  Rings, and 1,000,000 gold.

## Enhancement (`/statue/strengthening_item`)
- **Request (H):** `{statueuId, useItemList:[{itemId,count}], goldRatioInteger, goldAddition}`.
- **The client computes the preview itself (H).**
  - EXP: `exp×count`, × `bonusRate/100` when the condition matches the statue. Conditions: 2 any;
    4/5/6 type 1/2/3; 11 type 4; 12 type 5; 13 type 4 or 5; 7/8/9 rarity 3/4/5.
  - Gold per item: `max(floor((useGold+goldAddition)×goldRatioInteger/100000), 1)`. Boost items
    are × (boost+1), capped by `statueGrowthBoostGoldLimit`.
- **Gold ratio bug.** `mind_list` sent `goldRatioInteger 0`, so every material cost 1 gold. It now
  sends 100000.
- **What the server does.** It repeats the same arithmetic, refuses when items or gold are short,
  takes them, then adds the EXP (levels by our curve) or the boost (`convergence`).
- **Response:** `{statueInfo, userInfo, mindList, composeType, itemList}`. In item mode `itemList`
  replaces the client's material list. In Mind mode (`/statue/strengthening`), `mindList`
  replaces the whole Mind list, so it is always sent in full (the old stub sent `[]`).

## Ring Exchange (`/shop/shop_list`, `/shop/shop_buy`)
- **Goods (H).** `shopName` is the label `SHOP_00005_NAME`. Each goods entry has `payType 3` and
  `payItemId 31`, which shows the Memory Ring counter.
  - `goodsName`/`goodsDetail` are label keys.
  - `goodsIcon` must be a numeric string (null or empty throws), and `goodsend` must be a string
    (null throws); `"2036-01-01 00:00:00"` means no time limit. `limitNum -1` means unlimited.
- **What we sell, in rings (prices ours):** materials 10/30/100/300 by tier, platinum 500,
  turtles 300, tree materials 20, keys 200.
- **Buy.** `{shopId, goodsId, count}` returns `userInfo`, `itemSetInfo`, `shopList`, `itemList`
  (ring count) and `haveItemSetInfo`. That last one lists the **gain**: the client adds it to its
  counts.

## Not yet
- Event exchange: it needs an event with `eventShopFlg` and the unrouted `/shop/event_shop_buy`
  and `/event/event_point_list`.
- Growth-tree squares still cost nothing.
- Mind levelling (`/mind/strengthening*`) is not routed yet.
