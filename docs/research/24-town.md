# 24 — Town (the client's "clan"), solo with AI members

Read in the 3.12.0 client: H = read in the disassembly, M = inferred. Research pack notes (the
owner's private pack) are cited by their feature id. Everything marked "ours" is a choice of this
server. Server code: `server/services/src/town.rs` (state and rules),
`server/services/src/services/clan.rs` (routes), `server/services/src/town_socket.rs` (the Town
home's socket), `server/core/src/http.rs` (`StreamHook`).

## 1. Client facts

### 1.1 Entry (H)

- `LoadingManager.jumpClanCheck` (0xB782BC) sets up `ClanManager` and reads `/user/user_info`
  (`SetupUserClanInfoCoroutine` 0xF698CC): `userInfo.clanId`, `userInfo.position`,
  `userInfo.addStatus` and the response's `backgroundId`. Then `addStatus == 1` opens the Wait
  scene, `clanId >= 1` the Town home (map = `backgroundId`), anything else the Entrance.
- ClanStatus: None 0, Applying 1, AlreadyMember 2. ClanPosition: Applying -1, None 0, SubLeader 1,
  Leader 2. ClanManager's user id and uuid are the ones `create_user` gave (SaveDataManager).
- `/info/get_menu_notify`: My Page shows `myTownMember` as a number when it is above 0 and badges
  for `unreadChatFlg == 1` and `unapprovedMemberFlg == 1`; the Town home reads
  `unapprovedMemberFlg` for its new-applicant mark.

### 1.2 Routes (H: callers found for every request type)

| Route | Request | Response read |
|---|---|---|
| `/clan/recommend` | — | `clanInfo`; **`clanInfo.clanId` is read without a null check** (OnGetRecommend 0xF64E84); 0 shows `Clan_Osusume_Nashi` |
| `/clan/search` | `clanId` / `clanName` / `policy, activityTime, approval` | `clanList` |
| `/clan/create` | `clanName, policy, activityTime, approval, emblem, comment` | `clanId`; `>= 1` reads user_info and opens the home |
| `/clan/add` | `clanId` | `addStatus`: 2 joins (JumpLoad), else "PendingIn" |
| `/clan/cancel_approval` | — | — |
| `/clan/info` | `clanId` | `clanInfo` |
| `/clan/sync_server_info` | `clanId` | `ipAddress, port, crypt_key` |
| `/clan/check_add_request` | — | `requestFlg` |
| `/clan/add_request_list` | — | `requestList` (UserInfoClan) |
| `/clan/add_approval`, `/clan/denial_approval` | `id` | `requestList` (+ `clanInfo`) |
| `/clan/expulsion`, `/clan/set_sub_leader` | `id` | `clanInfo` |
| `/clan/change_info` | as create | `clanInfo` |
| `/clan/secession`, `/clan/invitation` | — / `id` | — |
| `/clan/clan_object_list`, `/clan/clan_bg_list` | — | `clanObjectIdList`, `clanBgIdList` (ints) |
| `/clan/donation_item_list` | — | `donationItemList [{itemInfo, donationPoint}]` |
| `/clan/clan_donation` | `itemList [{itemId, itemNum}]` | `beforeDonationLevel, beforeDonationPoint, clanInfo, getSetItem` |
| `/event/clan_member_event_point_list` | — | `clanMemberList [{userId, userEventPoint}]` |

`room_info`, `set_object`, `del_object`, `all_del_object` and `set_object_comment` have no caller
in 3.12.0: objects and the background are set over the socket.

### 1.3 Levels, functions, donation (H unless marked)

- `ClanTable.exp` is the cumulative total of a level (`GetTotalRankUpPoint`,
  `GetNextRankUpExperience` = exp[L+1] − exp[L], `GetPointToLevel` walks down from
  `clanLevelMax`), so level 1 has exp 0.
- `IsClanFunction(type)` = `clanInfo.clanFunctionList.Contains(type.ToString())`: the list holds
  `TOWN_FUNCTION_SUBMASTER`, `TOWN_FUNCTION_OBJECT_SET`, `TOWN_FUNCTION_MAP_CHANGE`.
- Labels: `ClanFunctionText_0002` Sub-Master unlocked, `_0003` object layout, `_0004` background
  map (`_0001` plain rank up, `_0005`..`_0020` dummies); `Clan_Object_Object_Unlock` says object
  placement needs Town Rank 8, `Clan_Object_Bg_Unlock` background change Rank 10;
  `ClanSubMasterMax` at most 3 Sub-Masters. The pack's guide (`social.town_growth`, provisional)
  gives 3/8/10, so Sub-Masters at Rank 3 (M).
- Town Medal = item 41 (`ITEM_0041_NAME`); at most 500 per donation (`MaxDonationAmount`,
  `Clan_Foundation_11`); the per-item maximum is also capped by the points left to the last
  level and by the count owned. `Clan_Foundation_10`: the donation gift goes to the Message Box.
  The result screen animates `beforeDonationPoint` → `clanInfo.clanPoint` and lists
  `getSetItem`; an ItemSetInfo can carry `clanObjectList` (ClanObjectInfo).
- `ClanPolicy_01..05`, `ClanActiveTime_01..06`, `ClanApproval_01` (Open) / `_02` (Approval
  Required), built as `ClanPolicy_{0:00}`; dropdown values are `value + IndexOrigin`. Emblems are
  `ce_{0:0000}.bytes` (`ui_clan_em_00`: ce_0000..ce_0010).
- `ClanEventData.eventCampaignName` is a label key; empty hides the home's event panel.

### 1.4 Objects and backgrounds (H)

- The Town map is `ClanObjectTable[backgroundId].clanObjectmodel` + `_hiru`/`_yoru`, or
  `map00_clan00` when the table has no such id. A row with an empty model would load "_hiru" (M:
  a hang), so a background id must name a row with a map.
- Object slots are the children of the map's `area_*` root "ClanObjectPosition"; an object loads
  `clanObjectmodel + ".bytes"`; it fits a slot when `clanObjectSize <= slot size`.
- The player's `clan_instance_object_00` holds 20 object models (`cl_frog`, `cl_turtle`, …);
  `map00_clan00..03` are the four maps; `ClanBGTitle_0001..0004` name them.

### 1.5 The Town home socket (H)

- `ClanManager.LoginCoroutine` (0xF6886C): `sync_server_info`, TCP connect (Dns host of
  `ipAddress`, `port`; send/receive timeouts 30 s), LoginRequest (1000) `{uu_id}`; a LoginResult
  with status 10000 sets IsLogin. The home (StartLoad 0xCEC7E4) then calls `info` and
  `check_add_request` and needs the player's own user id in the login notice.
- Frame: 26-byte little-endian header — `"GLPH"`, int64 version, int16 platform, int32 body size,
  int32 protocol id, int32 user id — then the body. Body: MsgPack of the message object as a map
  of field names (kazuki ObjectPacker; old spec: FixRaw/Raw16/Raw32 strings, no str8; Single only
  as float32; Int32 takes fixnum, int8-32 or uint8-32; missing fields stay null/0), XOR the
  ASCII crypt key (`dummy` when empty).
- Protocol ids: requests 1000 login, 1001 chat, 1002 move, 1003 logout, 1004 action, 1005 chat
  read, 1006 set objects, 1007 set background, 5000 heartbeat; responses +10000; notices 30000
  login, 30001 chat, 30002 move, 30003 logout, 30004 action, 30006 objects, 30007 background,
  31001 position change, 31002 rank up.
- Responses need `result 0, status 10000`, notices `status 10000`; any other status becomes an
  error message (`ClanMessageCompiler.StatusCheck`), which can send the player back to My Page.
- LoginNotify: per member `user_id, statue_id, model_list` (the statue's modelDataList),
  `x, y, z, user_name, vampire_flg, comment, position`; `clan_object [{object_id, slot_index}]`,
  `background_id`, `chat_log [{user_id, message, message_at ("yyyy-MM-dd HH:mm:ss"), seq_id}]`,
  `last_seq_id`, `leader_user_id` / `last_leader_user_id` (different = leader change).
- The client sends CharMoveRequest `{x, y, z}` (its target) about once a second while moving, a
  heartbeat every 10 s; a move notice sets another member's walk target.

## 2. What the server does (ours unless marked)

- **State** (`Progress::town`): the Town the player belongs to (own or AI), a pending request,
  the one-time welcome flag. A Town keeps name, policy, activity time, approval, emblem, comment,
  Master, Sub-Masters, points, settled level, background, owned and placed objects, AI members and
  applicants, chat (100 lines), the player's donations and walked positions.
- **AI Towns**: twelve (ids 2001-2012), stable: 4-13 AI members wearing statues from our roster,
  Rank 2-12, open or "Approval Required". Joining an open one is immediate; a request is approved
  after 60 s. The recommend screen shows a different one each game day (none while in a Town).
- **Own Town** (id 1001): the player leads it; three AI players apply at once and one or two more
  every game day while there is room; an open Town takes them in at once.
- **Days**: every AI member donates 5-30 Town Medals a game day (up to 7 days caught up).
- **Levels** (`clan_table`): 20 levels, exp 250·(L−1)·L; functions at 3/8/10 (above). Each
  level from 2 unlocks two of the 20 objects with a model; all four backgrounds at Rank 10.
- **Donation**: 10 Town points per Town Medal; gift 100 gold per medal to the Message Box.
- **Payout** (Message Box): level reward 10·L Diamond Coins per level reached; 100 Town Medals a
  game day while in a Town; 300 once on first joining.
- **Socket**: served on the HTTP port (the HTTP server hands a connection that starts with
  `GLPH` to the Town socket). Login answers with the player and every AI member online; chat is
  echoed as a notice and saved; objects and the background change only for the Master or a
  Sub-Master with the level's function; every 4 s one AI member walks to a spot the player has
  walked or does its action. Unknown requests are answered OK.
- **Town events**: the event panel stays hidden; member points are 0.
- **Shapes**: `recommend` always sends a ClanInfo (clanId 0 when there is none: the earlier `{}`
  would have thrown in OnGetRecommend). Only a member's view of a Town lists its members (the
  entrance's info window reads counts and names only, SetupClanInfoWindow 0xF56D90), which keeps
  the search reply small. An AI member's card comes from `/user/user_info_mini {id}`.
- **Null audit**: every Town reply and socket message is walked against the client's classes in
  tests (`services/shapes.rs`; the Town and `Jjs.Socket.*` classes were added to
  `testdata/client_shapes.json` from dump.cs).

## 3. Unverified

- The maps' spawn points: everybody starts near (0, 0, 0) (M); AI members walk only to positions
  the player has sent, so they stay on walkable ground.
- Object models matched to titles by name (M); `clanObjectSize` 0 fits every slot; object icons
  (`clanObjectIcon`) are 0.
- Background row ids 9001-9004 and their order (Morioh Station first, M).
- Item 41 has no icon in `ui_item_00`; the donation list and the Message Box rows may show an
  empty icon.
- `myTownMember` as the member count (it is shown as a number on My Page).
- What the original server did with chat notices to the sender, membership changes during Town
  events, and leaving one's own Town (ours: the Town closes).
