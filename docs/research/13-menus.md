# 13 — What the MyPage menus need from the server (Build 15)

Our own analysis of the 3.12.0 client. Addresses are RVAs in `libil2cpp.so`; H/M/L as in the
other notes. No game text is reproduced.

## 1. Summon (gasha)
- **Why it hung (H).** `GashaUIController.StartLoad` 0xDDEC80 splits `gashaList` into statue banners
  (types 1/3/5) and mind banners (types 2/4/6). With no statue banner the screen opens the Mind tab.
  `SetBoxGasha` 0xE63F8C then reads entry 0 of that tab's list (`boxFlag`, `boxResetEnable`,
  `boxRemain`) with no length check.
- **What we send.** One statue banner (300000) and one mind banner (400000).
  - `gashaWindow` must be a banner bundle that exists in the player's files: `gw_<id>_<suffix>`,
    461 of them. The client downloads `gashaWindow + ".bytes"`.
  - `gashaAppealInfo` must be a list.
  - `payType` 4 (paid in diamonds) with price 0: draws are free offline.
- **Draw** (`/gasha/gasha_play`).
  - The response is `{userInfo, getResultList, originalStatueInfo, Lv1StatueMasterInfo}`; both
    trailing lists are required.
  - Each result is `{dataType, dataInfo, datauId, level, newFlg, effectInfo{statue…, mind…}}`.
  - Rarity follows the Gamerch rates: statues ★5 1.5%, ★4 12.5%, ★3 86%; minds ★4 7%, ★3 93%.
  - Everything is already owned offline, so a draw shows the owned copy.

## 2. Shop
- **Why it hung (H).** `Shop1UIController.Update` 0xD82C5C reads `shopList[0].goodsList.Count` with
  no length check.
- **What we send.** One shop with an empty goods list. `itemSetInfo` objects carry all four lists.
  `shop_buy` and `event_shop_list` answer without changing anything.

## 3. Growth tree
- **The client has no tree data (H).** `ui_tree` holds only tiles and icons. `/tree/skill_list` must
  send the whole graph: `{treeInfo{treeId, squareList[SquareInfo]}, itemList}`.
- **What each square needs (H).**
  - A non-null `squareEffectList` object; it is a single object, not a list.
  - Non-null `openMaterialList` and `lockList`.
  - When closed, a `beforeSquareId` that names an existing square.
  - The root square is open, with effect 0.
- **Effect ids.** 1 ATK, 2 DEF, 3 HP, 4 skill, 5/7/9 special, 10 mind capacity, 11 leader, 12
  diamonds, 13 assist, 14 evolution.
- **No original layout survives, so ours is:**
  - a start square, then 14 squares in a line cycling ATK, DEF, HP;
  - together they add the wiki's tree-max minus Lv99 stats (20% of Lv99 when unknown);
  - squares open free (`/tree/acquisition`, in order: a square needs the one before it);
  - opened squares are saved per statue and added to its stats.

## 4. Other screens

| Screen | Routes | What we send |
|---|---|---|
| Dictionary | `/book/statue_list`, `/book/mind_list`, `/book/item_list`, `/book/yogo_list` | Every statue (Lv1 and Lv99 info) and every mind, all met |
| Enhance | `/item/exp_item_list` (on entry), `/statue/strengthening(_item)` | No EXP items yet; the statue is unchanged |
| Evolution | `/statue/evolution_info` | Every view shows the statue as it is |
| Mind sell | `/sell/sell` | Nothing sold |
| Inbox, trophies | `/present/get(_all)`, `/trophy/trophy_result_get` | Empty |
| Protect, sort order, "new" marks | `/mind|statue/protect`, `save_sort`, `new_flg_release` | Accepted |

All three priority diagnoses and every field layout are H. The secondary screens were checked for
which route they call and which lists must be non-null (M).
