# 20 — Event exchange (Build 22)

The event shop ("Shop3") trades an event currency for goods. Read in the 3.12.0 client: H = read
in the disassembly, M = inferred.

## 1. Entry (H)

- Event Home reads `bizarre_list.eventInfo`. Its exchange button shows only when
  `eventShopFlg == 1` and `eventCollectType` is not 0 (none) or 4 (high score).
  Collect types: 1 points, 2 items, 3 points and items, 4 high score, 5 high-score shop.
- Shop3 copies `eventInfo.eventItemList` (a null list throws) and shows it as header icons.
- Flag 1: `/shop/event_shop_list {eventId}`. Flag 0: the point-reward list
  (`/event/point_reward_list {eventCampaignId}`) instead.

## 2. Routes (H)

| Route | Request | Response |
|---|---|---|
| `/shop/event_shop_list` | `{eventId}` | `{eventShopList: [{eventId, eventName, goodsList}], itemList}` |
| `/shop/event_shop_buy` | `{eventId, goodsId, count}` | `{userInfo, itemSetInfo, eventShopList, itemList, haveItemSetInfo}` |

- The screen reads `eventShopList[0].goodsList.Count` without a length check: never an empty
  list, never a null `goodsList`.
- Goods cells use the normal shop's drawing code, so the normal shop's rules hold
  (docs/research/15): numeric `goodsIcon`, non-null `goodsend`, "2036-01-01 00:00:00" = no limit.
- After a buy, `itemList` counts replace the header counts (absolute, not a delta). Shop3 does not
  add `haveItemSetInfo` to its counts.

## 3. Currency (H)

`bizarre_end.eventResult` (never null): `eventType` 3 (items) with
`getEventItemList [{itemId, count}]` lists the currency gained on the result screen. Types 1/2
are points.

## 4. Server (Build 22)

- Every event has the exchange on: `eventShopFlg 1`, `eventCollectType 2`, `eventItemList` = the
  player's Fight Coins.
- **Currency:** Fight Coin (item 3017) for every event. The events' own currencies are not
  recorded, so this is ours. An event-quest clear drops 20 coins (ours) and the result screen
  lists them.
- **Goods:** the Ring Exchange goods (materials, turtles, tree materials, keys) at the same
  prices in Fight Coins (ours). The game's own goods labels exist for a few events, but not what
  they gave.
- Not yet: rankings.

## 5. Point events (Build 23)

- **Client (H).** A point event is `eventCollectType 1, eventShopFlg 0`: Event Home shows
  `eventInfo.userEventPoint` and a "check rewards" button, which opens Shop3 on
  `/event/point_reward_list {eventCampaignId}` → `{userEventPoint, clanEventPoint,
  userPointRewardList, clanPointRewardList}`. A reward row shows its title and text labels, the
  points needed and the item; `getFlg 1` marks it received. **There is no claim route**: the
  server pays a reward when the points reach it and lists it in `bizarre_end.eventResult`
  (`eventType 1`, `getUserEventPoint`, `userEventPointAll`, `eventRewardList`).
- **Server.** Event 50045 is the point event, because it is the one of our four with reward labels
  (`point_event_reward_500045_147..156_Personal_text`, describing Stone Shards, the attribute
  stones and brooches). Its quests give 50 points (ours); the ten rewards are those items at every
  100 points up to 1,000 (thresholds and counts ours) and go to the Message Box once. The other
  events keep the exchange.
