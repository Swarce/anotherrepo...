# Boot sequence (implementer spec)

Evidence: `B-boot` research (summarized in [../research/01-boot-redirect.md](../research/01-boot-redirect.md),
[../research/03-progression-storage.md](../research/03-progression-storage.md)). Confidence per row:
H = read in code, M = inferred.

All requests go through one serial queue, so the wire order is the order below.

## Call order

| # | Call | Caller (evidence) | Notes |
|---|---|---|---|
| 1 | `POST /common/get_version` (built-in base) | `TitleUIController.GetVersion` @0xB913A0 | title screen; reads only `is_ca`/`ca_message` |
| 2 | `POST /common/get_version` | `LoginChecker.Login` → `RequestResponseManager.GetVersion` @0xE25E38 | user tapped Start; sets API base = `url`, web base = `s3_url` |
| 3 | `POST /user/login` | `LoginChecker/<OnCheckLogin>` @0xB7C2E0 | branches on `existStatus` |
| 3a | `GET {s3_url}/information/terms_a.html` | `PolicyUIController` @0xDC2058 | new user only; Accept button appears only if this loads |
| 4 | `POST /user/get_nonce` | `LoginChecker/<OnCreateUser>` @0xB7D100 | new user only |
| 5 | `POST /user/create_user` | same | saves `user_id` + `uuId` to PlayerPrefs |
| 6 | `POST /user/login` | same | again, now with uuid → `existStatus:0` |
| – | `POST /user/select` / `/user/change_backup_id` | `<OnSaveDataSelect>` @0xB7D8F0 / `<OnConfirmTerminalInfoChange>` @0xB7CD58 | only for `existStatus` 1/3 or 2; then login again |
| 7 | `POST /client/getkey` | `TitleLoad/<Load>` @0xB8E8C0 | |
| 8 | `POST /common/get_system_config` | same | 7–9 queued back to back |
| 9 | `POST /user/get_tutorial_step` | same | |
| – | `POST /client/getfiledata` + `GET` each URL | `AssetVersionManager`, BGM, `LoadAllAssets`, tutorial data | `versionfile` first, then stale files; any number of times (see [assets.md](assets.md)) |
| 10 | `POST /shop/product_list` | `InAppPurchaseManager.InitIAP` (awaited) | |
| 11 | `POST /user/user_info` | TitleLoad | only if a main tutorial is pending; then the tutorial takes over |
| 12 | `POST /user/user_info` | `MypageUIController/<StartLoad>` @0xC393A4 | MyPage, 12–17 queued together |
| 13 | `POST /team/team_info` | same | |
| 14 | `POST /info/check_forced_notify` | same | |
| 15 | `POST /info/event_list` | same | |
| 16 | `POST /bizarre/bizarre_resumption` | same (0xC3A0CC..0xC3A35C) | |
| 17 | `POST /info/banner_list` | same | |
| 18 | `POST /user/get_loginbonus` | MyPage | started together with 19 |
| 19 | `POST /info/get_menu_notify` | `UpdateMenuNotify` | |
| – | `POST /info/notify_list` | InformationUI | only if `check_forced_notify.category != 0` |

The optional "download all data" start button adds `/client/getkey`, getfiledata/CDN and a heartbeat
(`/common/get_version` with an empty `HeartbeatResponse`) before TitleLoad.
After the tutorial, `goto_init` restarts at step 2 (get_version + login again).

## Must return

Every response also needs the envelope fields and `param.status/emergencyInfo/frenzyTextId`
([envelope.md](envelope.md)).

| Call | Must return | Evidence | Conf. |
|---|---|---|---|
| get_version | `url` = own API base (no trailing slash), `s3_url` = own web base, `is_ca:0`, `ca_message:""`, status 10000 (10012 also OK, 10013 = forced update) | SetURL @0xE26090 | H |
| terms page | HTTP 200 with any HTML | @0xDC2058 | H |
| login | unknown/empty uuid → `existStatus:4`; known → `existStatus:0` with `userInfoMini` and `teamInfo`; `is_ca:0` (1 shows a page then quits) | jump table @0x223C3AC | H |
| get_nonce | `nonce` (any string), status 10000 | @0xB7D2CC | H |
| create_user | non-empty `user_id` and `uuId` (capital I); signature may be ignored | @0xB7D674/@0xB7D6E0 | H |
| select / change_backup_id | `user_id`, `uuid` (lower-case here) | B-boot §3.6 | H |
| getkey | the real `assetkey`, `assetvector` (ASSET_KEY/IV) and `soundkey` (decimal string) | @0xB8EB34, @0xB8F80C | H |
| get_system_config | `configList` with 29 ints; recovery times non-zero | @0xB8EBB8 | M |
| get_tutorial_step | `tutorialStep` list of 0/1; all ones (e.g. 256) skips every tutorial | @0xB8E4E4 | H |
| getfiledata | `url[i]` for every `filename[i]`, same order | @0xFD4FD4, @0x10CE080 | H |
| product_list | `key:""`, `productId:[]` (IAP never initialized) | @0x203BD98 | M |
| user_info | `userInfo` with sane values (rank >= 1, stamina/BP/RP and their max, `*UseTime`/`*MaxTime` as parsable dates), `halt:0`, `backgroundId` | B-boot §3.12 | M |
| team_info | `teamList`; for the leader model a main team (`IsMainTeam`) whose leader has `statueInfo.statueId` and `modelDataList` pointing at existing bundles; an empty list skips the model | @0xC38708, @0xBFF798 | H/M |
| check_forced_notify | `category:0` | B-boot §3.14 | M |
| event_list | empty `eventList`, empty `userBattleInfo` / `raidInfo` | B-boot §3.15 | M |
| bizarre_resumption | **status 50301** (no interrupted battle); 10000 offers a resume | @0xC3A34C | H |
| banner_list | `bannerList:[]` | | M |
| get_loginbonus | `loginBonusInfo:[]` (no popup) | @0xDE6718 | M |
| get_menu_notify | `notAllowedFunction:[]` (an entry locks a menu) and zero counters | | M |

Build 01 implements every call in this table except `/user/select` and `/user/change_backup_id`
(never triggered: our login only answers `existStatus` 0 or 4) (`server/services/src/services/`).
Build 01 answers `team_info` with an empty list, so the MyPage leader model is skipped until
statue content exists.

## Client prerequisites outside the server

- Play Games auto sign-in waits for its callback before the title scene.
- Login waits for the Google advertising id.
- The client quits if no device id (ICCID/IMSI or MAC/Android id) can be read.
