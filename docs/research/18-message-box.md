# 18 — Message Box (`present/*`, Build 22)

The Message Box (presents) holds gifts until the player receives them. Build 21 sent an empty
box. Read in the 3.12.0 client: H = read in the disassembly, M = inferred.

## 1. Routes (H)

| Route | Request | Response |
|---|---|---|
| `/present/present_list` | `{prevNum, receivedFlag}` | `{allPresentNum, allPageNum, presentList}` |
| `/present/get` | `{presentuId, prevNum}` | GetPresentResponse |
| `/present/get_all` | `{prevNum}` | GetPresentResponse |

GetPresentResponse: `{recieveFlg, allPresentNum, allPageNum, presentList, userInfo, dataList,
getPresentList, originalStatueInfo}`.

## 2. Paging and tabs (H)

- `prevNum` is the 1-based page number, not an offset.
- `receivedFlag` is the tab: 0 waiting, 1 received. Switching tab resets the page to 1.
- The arrows wrap between page 1 and `allPageNum` and are active only when `allPageNum > 1`, so
  `allPageNum` is always at least 1.
- The client has no page size; ours is 20 (M).
- The total shown is `allPresentNum` as sent.

## 3. The row (H)

- The row reads only `dataType`, `dataId`, `count`, `notify`, `name`, `comment`,
  `indefinitely`, `dueDate`, `receiveDate`, and `presentuId` for the request.
- **`notify` is the "received" flag.** A row shows only in the tab whose `receivedFlag` equals its
  `notify`. With `notify 1` the receive button is hidden and `receiveDate` is shown.
- `name` is a label key (`ITEM_<id>_NAME`). An unknown key is shown raw.
- `comment` is literal text. Each `{LABEL}` in it is replaced by that label's text.
  - The game's `PRESENT_MISSION_MESSAGE` holds `{missionName}` and `{time}`, which the client
    cannot fill. So mission rewards send literal text with the mission title label, e.g.
    `Mission reward: {Daily_ACHIEVEMENT_Text_0001}`.
- **Icon trap.** For dataType 3–6 the icon calls `int.Parse(dataId)`. A null, empty or non-number
  `dataId` throws and the row is not drawn. Ids: 10 Diamond Coins, 20 gold, 40 Friend Points,
  otherwise the item id.
- `dataType`: 1 Statue, 2 Mind, 3 Item, 4 Diamond Coins, 5 gold, 6 Friend Points.
- `indefinitely 1` shows "no limit". Otherwise `dueDate` is parsed (a bad date does not crash,
  but shows a nonsense time).

## 4. Receiving (H)

- On status 10000 the client redraws the box from the response's `presentList` (it does not remove
  the row itself), so the server returns the refreshed current page of the waiting tab. A null list
  would leave the old rows on screen.
- `recieveFlg`: 0 no dialog; 1 "possession limit, nothing received"; 2 "some not received".
  Offline it is always 0.
- `get_all` first opens a dialog listing `getPresentList` (name and count); empty = no dialog.
- `originalStatueInfo: []`, or the client may start a statue awakening movie.
- The response cache copies `userInfo` into the user cache and refreshes the header, so the full
  updated `userInfo` is sent. Nothing reads `dataList`.
- Errors: 51401 not found, 51402 already received.
- Leaving the box makes Home call `/info/get_menu_notify` again; its `unopenedPresent` is the
  badge.

## 5. Server (Build 22)

- `presents::PresentBox` is saved with the story progress: waiting and received gifts, newest
  first; the newest 100 received are kept.
- **Into the box:** mission rewards (the Missions screen already says they go there), and a
  one-time welcome gift (100,000 gold and 500 Memory Rings; ours, for testing the box).
- **On receive:** gold, free Diamond Coins and catalog items are added; Friend Points have no use
  offline.
- `get_menu_notify.unopenedPresent` counts waiting gifts.
- Next: the Count Up summon ticket is delivered here (research pack: the ticket goes to the box).
