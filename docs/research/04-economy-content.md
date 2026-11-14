# 04 — Economy and content (stub)

What the client expects from the server for prices, gacha, shops and master data. Details, values
and sources come in M4; this note records what is known from the client code in Build 01.

Confidence: H for endpoint names and request fields (from `B-api-paths.tsv`); M for how the client
uses `ConfigInfo` values; content values themselves are not researched yet.

## 1. ConfigInfo (`/common/get_system_config`) (H names, M use)

`SystemConfigResponse { configList: ConfigInfo }`, 29 ints, stored by
`SystemConfigManager.set_SystemConfig` (TitleLoad @0xB8EBB8). Groups:

- growth gold costs per rarity (`statueGrowthRare1Gold`.., `statueGrowthBoostRare3Gold`..,
  `statueGrowthBoostGoldLimit`, `mindGrowthRare1Gold`..);
- limits (`playerLimitMaxMind`, `characteristicLevelUpperLimit`);
- recovery timers (`apOneRecoveryTime`, `bpOneRecoveryTime`, `rpOneRecoveryTime`, `bpMaxFatiguDown`).
  These divide time deltas, so they must never be 0;
- assist-status tuning (`assistStatus*`), `pvpTeamCharaChangeIntervalTime`,
  `evolutionPossibleFlagRare3..6`, `gameStartStamina0Rank`.

Build 01 sends placeholder values (`server/services/src/services/misc.rs`).

## 2. In-app purchases: off (H/M)

- `/shop/product_list` → `{key, productId[]}`. `InAppPurchaseManager` initializes the Prime31 store
  only when `key` is non-empty (callback @0x203BD98, M). Build 01 sends `key:""`, `productId:[]`,
  so the store is never contacted.
- `/shop/buy_trans`, `/shop/buy_complete` (receipt, purchaseData, signature), `/shop/buy_cancel`,
  `/shop/buy_list` exist for paid stones. Offline policy (ECONOMY.md): no real payments; paid
  currency is either disabled or granted locally from content.

## 3. Gacha and shops (H endpoint names)

| Area | Endpoints |
|---|---|
| Gacha | `/gasha/gasha_list {gashaType}`, `/gasha/gasha_detail {gashaId}`, `/gasha/gasha_play {gashaId, countType, playType}`, `/gasha/get_box_gasha`, `/gasha/reset_box_gasha` |
| Shops | `/shop/shop_list`, `/shop/shop_buy {shopId, goodsId, count}`, `/shop/event_shop_list {eventId}`, `/shop/event_shop_buy` |
| Rewards | `/present/present_list`, `/present/get`, `/present/get_all`, `/user/get_loginbonus`, `/trophy/*` |
| Stamina | `/user/recover_stamina {payType}`, `/user/recover_battle_point {payType, itemId}` |
| Growth | `/statue/strengthening*`, `/statue/evolution_*`, `/mind/strengthening*`, `/tree/acquisition`, `/sell/sell` |

The gacha detail page is a WebView at `s3_url + /gasha/<id>/detail.html` (01).
`gasha_play` is also driven by the tutorial (`goto_gasha(1,0)`, gacha group 1).

## 4. Master data is mostly server-side ("fat DTOs") (H for the pattern)

- `StatueInfo`, `MindInfo`, `StageInfo`, `AreaInfo` and similar arrive fully populated from the
  server: names, skills, abilities, model ids, level tables. Examples:
  `GashaPlayResponse.Lv1StatueMasterInfo`, `BaseStatueInfo.ModelName` @0xBFF798 reading
  `modelDataList`.
- The client's own master data (`in_memory_master_data`, 18 tables: settings, levels, skills,
  abilities, tutorial flags, camera sets, ...) covers only part of it, and is itself missing
  (rebuild planned in M1.5; see 08 and `docs/design/master-data.md`).
- So the server needs a statue/mind/stage/drop/gacha-pool source. Plan: author it in
  `server/content` from **public wikis, cited per value**, plus ids/references that the player's own
  bundles imply (model and stage asset names). Nothing extracted from game files is committed.

## To do in M4

- Content schema for statues, minds, items, stages, gacha pools, shops, login bonus.
- Per-source citation list (wiki page and access date) in this file.
- Gaps per statue (skills/abilities numbers) listed here, as the master-data design asks.
