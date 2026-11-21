# 15 — Items, enhancement with items, shops (Build 21; shops Build 42)

Read in the 3.12.0 client (H read in code, M inferred).

## Items
- **No item master table (H).** `item/item_list` and `item/exp_item_list` carry everything.
  - `ItemInfo {itemId, name, comment, icon, bg, flame, type, value, unit, sell, priority, count}`.
  - `ExpItemInfo` adds `stack, exp, bonusCondition, bonusRate, useGold`.
  - `name`/`comment` are label keys (`ITEM_<id:04>_NAME` / `_DETAIL`). The icon is built from the
    id (`it_<id:04>` in `ui_item_00`); the `icon` string is not read.
- **Which ids** (`tools/content/build_items.py` → `server/content/items.json`). An id is listed
  when it has both an icon and a name in the player's files and is not a profile currency (0, 10,
  20, 40): 414 items (Build 41: +85). The first categories come from the client's id ranges; the
  ranges marked *label* are ours, read from the item labels (`ITEM_<id>_NAME`/`_DETAIL`), not from
  client code. `type` is set only where a `Jjs.ItemType` name matches the item text (11
  EVENT_ITEM, 12 EVENT_POINT, 15 MIND_BOX_EXTEND), else 0; the client paths we found that read it
  never see these values (MindSale asks only for type 4; the gasha check reads the gasha reply).

  | Ids | Category | From |
  |---|---|---|
  | 31 | Memory Ring | client |
  | 41 | Town Medal | label |
  | 50–289 | tickets | client |
  | 300–306 | summon medals | client |
  | 1001, 7001 | AP recovery | label |
  | 1002–1050, 1200–1202 | growth-tree materials (keys 1021–23, 1038, 1047, 1048) | client |
  | 1100–1102 | BP Recovery Colas (1100 has no icon) | client |
  | 1301, 8500–8502 | awakening materials | label |
  | 1500–1533 | honors (13 certificates) | label |
  | 2000 (type 12), 2001 | event points | label |
  | 2002–2013 (type 11) | event items (2003–2012 are retail placeholder rows) | label |
  | 3000–3184 | event exchange items | client |
  | 4001–4003, 4006 (type 11); 4004–4005 (type 0, summon points) | exchange | label |
  | 5001–5002 | Raid Energy recovery | label |
  | 6001 (type 15) | Mind Box extension | label |
  | 6100–6138 | enhancement materials | client |
  | 8600–8749 | Awakening Medals (42 with an icon; 108 have none) | label |

  Left out: 25 ids with an icon but no name label (3031, 8000–8019, 9000–9003) and 126 with a name
  but no icon.

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

## Shops (`/shop/shop_list`, `/shop/shop_buy`; six shops in Build 42)
- **Six shops (H).** The player's files hold `SHOP_00001_NAME`-`SHOP_00006_NAME` (our English:
  Buy Diamond Coins, Expand Mind Box, Restore AP, Diamond Coin Exchange, Memory Ring Exchange, Buy
  BP Recovery Cola) and, in `ui_shop`, the sprites `ShopTypeIcon_00001`-`00006`.
  `ShopMenuBannerController.UpdateUI` (0xBE2050) turns on `menuIcons[shopId - 1]` and shows
  `TextManager.GetString(shopName)`, so the ids are 1-6. `Shop1UIController.Update` (0xD82C5C)
  jumps to shop 1 when sent for Diamond Coins and to shop 3 when sent for AP, and sorts the shops
  by `priority` (0xD83958). There is no Friend Point shop in this client.
- **Shop 1 is the store (H).** `checkShopList` (0xBD92F8) clears shop 1's goods during
  maintenance or when `InAppPurchaseManager.isSupportIAP` is false, and otherwise drops goods
  whose `productId` the store does not enable (`checkProductId` 0xBE26A4); the other shops lose
  only goods before their sale days (`checkSaleDaysBefore` 0xBE4828). Offline the store is never
  initialised (docs/protocol/boot.md), so shop 1 always shows empty with the "cannot be purchased
  at this time" alert. We still send it (last in order), and sell its coin packs in shop 4.
- **Goods (H).** `goodsName`/`goodsDetail` are label keys; the game has `GOODS_NNNN_*` labels
  for its own goods (coin packs 0001-0006, Mind Box 0007, AP 0008, Sandwich 0009, keys,
  trophy and turtles 0010-0016, colas 1100-1102). `goodsIcon` must be a numeric string (null or
  empty throws), and `goodsend` must be a string (null throws); `"2036-01-01 00:00:00"` means no
  time limit. `limitNum -1` means unlimited (the dialog caps it at 999).
- **`payType` (`Jjs.PayType`, H):** 0 UnUsed, 1 Compensation, 2 Free, 3 Item, 4 Cash, 5 Hybrid.
  - A tap on a good with payType 1-3 opens the item purchase (`ItemBuyFuncGate`); 4 opens the
    store purchase (`StoneBuyFuncGate`); 0 and 5 do nothing (`ShopItemOnClickThumnail` 0xD82AE0).
  - The pay count (ItemBuyFunc lambdas 0xBE6FE4 / 0xBE7090): 1 = paid coins (`stoneNum`, item 0);
    2 = free + paid coins (`stoneNum + stoneNumFree`, item 10); 3 = the count of `payItemId` in
    the shop's `itemList` (`CheckPayItemRemains` 0xBE2A24). `checkShopBuyStone` (0xBE4198)
    refuses 1/2 when the coins are short. There is no gold payType: gold is item 20 with
    payType 3.
  - The Memory Ring counter shows while the shop has a payType 3 / item 31 good
    (`setMemoryRingDispMode` 0xD832F0).
- **`SaleType` (`convSaleType` 0xBE3728, H):** `"AP_RECOVER"`, `"MIND_BOX"`, `"NUMBER"`, else
  normal.
  - AP_RECOVER is refused while AP is at its maximum (`checkShopBuyApRecovery` 0xBE3A04), which it
    always is offline, so no such good is sent.
  - MIND_BOX is refused when `mindMax + num x count` passes `max(playerLimitMaxMind, 200)`
    (`checkShopBuyMindBox` 0xBE3CB4). Offline `mindMax` is already 999, so the client shows "The
    Mind Box cannot be expanded beyond its maximum size."
- **What each shop sells (ours, docs/CONTENT.md).** The coin packs sell Diamond Coins for gold
  (shop 1, and shop 4 where they show), shops 2/3/4/6 take Diamond Coins, and shop 5 takes Memory
  Rings (the materials as before plus every ability Mind). Goods with a statue or a Mind carry it in `itemSetInfo.statueList`/`mindList`
  (searchItemSetInfo 0xBE13E8 reads the id); the list sends them without ability tables to keep
  `shop_list` near 0.9 MB.
- **Buy.** `{shopId, goodsId, count}` returns `userInfo`, `itemSetInfo`, `shopList`, `itemList`
  (pay item counts: rings, gold) and `haveItemSetInfo`. That last one lists the **gain**: the
  client adds it to its counts.
  - The server pays by the good's payType: an item, gold (20), Friend Points (40) or Diamond
    Coins (`stone_free`; `userInfo` always sends 0 paid coins).
  - It gives items, coins, a statue (a boost, `convergence`, when owned, else a new Lv1 statue)
    or new Lv1 Minds. A short balance or a full Mind Box answers 50001 and takes nothing.
- **Open.** The retail goods ids, prices and the store packs' real use are unknown (server data);
  what MIND_BOX goods add on the server side is unknown; a Friend Point shop would need a shop
  id and name the client does not have.

## Not yet
- Event exchange: built in Build 22 (docs/research/20). The point-reward route is
  `/event/point_reward_list`; there is no `/event/event_point_list`.
- Growth-tree squares still cost nothing.
- Mind levelling (`/mind/strengthening*`) is not routed yet.
