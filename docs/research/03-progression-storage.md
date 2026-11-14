# 03 — Progression and storage

All player progression lived on the official server. The device keeps only an account handle
(`uuid`, `user_id` in PlayerPrefs) plus downloaded files and their local index (`vcs.bytes`).
The original accounts cannot be recovered; every player starts a fresh local account, and our
server must store the whole player state.

Confidence: H for what the client stores and where it reads the uuid; H that the player data dump
has no account data; M for the PlayerPrefs value encoding; L/unknown for the exact shape of several
server-side records (listed at the end).

## 1. What the device stores (H unless noted)

| Item | Where | Evidence |
|---|---|---|
| `uuid` | PlayerPrefs key `"uuid"` | `SaveDataManager.SaveUUID`/`LoadUUID` @0xFD9A58/@0xFD9B08 |
| `user_id` (display only) | PlayerPrefs key `"user_id"` | `SaveUserId`/`LoadUserId` @0xFD9B64/@0xFD9BD0; shown on the title screen |
| Small flags/settings | PlayerPrefs (e.g. auto sign-in cancelled, movie-played flags, sound/network settings) | `SaveDataManager` |
| Downloaded bundles, CPKs | `<persistentDataPath>/<obfuscated>.bytes`, flat | see 08 |
| Local download index | `<persistentDataPath>/vcs.bytes` (encrypted `{keys:[file], values:[md5]}`) | `AssetVersionManager` @0x10C4BDC; see 08 |

- Values go through `ScSystem.ScIO.LocalStorageManager`; `LoadFromLocalPlayerPrefs` decodes
  base64 of a BinaryFormatter-serialized `System.String`. (M)
- The **session is memory-only**: taken from each response and echoed; never persisted. (H)
- `HttpResponseCacheManager` caches user/team info from responses in memory only. (H)
- No local save of statues, items, currencies, quest progress or story flags was found. All of it
  arrives in responses as server-built DTOs (`UserInfo`, `TeamInfo`, `StatueInfo`, ...). (H for the
  DTO flow; M for "nothing else is saved", based on the `SaveDataManager` key list.)

## 2. The old account is not recoverable (H)

- The player's archive is `Android/data/com.bandainamcoent.jojodr/{cache,files}`: 13,480 bundle
  files, an il2cpp copy and a shader cache. It has no `shared_prefs` (Unity keeps PlayerPrefs in the
  app's internal `shared_prefs`, outside that folder, M), so not even the old `uuid` survives.
- Even with the uuid, the state was on the official server, which is gone.
- Data move (`/user/create_data_move_password`, `/user/data_move`) and device change
  (`/user/change_backup_id`) were server-side transfers; nothing to replay.
- Result: a fresh local account per device. `vcs.bytes` and the bundles are still useful (they save
  re-downloading, see 08).

## 3. Account identity flow (H)

1. `/user/login` with `uuid:""` → server answers `existStatus:4` (new user).
2. Terms page must load (`{s3_url}/information/terms_a.html`), then `/user/get_nonce` and
   `/user/create_user` → `{user_id, uuId}` (capital I). The client writes both to PlayerPrefs.
3. `/user/login` again with that uuid → `existStatus:0`.
4. Later boots send the stored uuid, and the same `backupId`/`ad_id`/`model` in the login request.
   Keying accounts on `uuid` is enough for a local server.

Other writers of the uuid: `/user/select` (multi-save choice), `/user/change_backup_id`, and
`TakeOverController` (data move). (H)

## 4. Tutorial flags (H, table contents unknown)

- `tutorialStep` is a positional list of 0/1 (`BoolType`), indexed by tutorial-flag id.
- Client side: `TutorialManager.InitFlagList` @0xB8E328 makes one entry per row of
  `tutorial_flag_table` (inside `in_memory_master_data`, missing; see 08).
  `UpdateTutorialFlag` @0xB8E4E4 copies `min(local, server)` entries; `IsFlagOn` @0xB9BE28 reads by id.
  Extra entries are ignored; missing entries count as "not done".
- Saving: `/user/tutorial` posts the **whole** list (`UpdateTutorialStepRequest`). The client
  re-reads it with `/user/get_tutorial_step` after quest clear and battle result.
- Skip-all: answer a long all-ones list. Build 01 does this (256 × 1, `TUTORIAL_FLAGS` in
  `server/services/src/lib.rs`) because the tutorial needs master data we have not rebuilt yet.
- Tutorial scripts call `/bizarre/bizarre_start`/`bizarre_end`, `/user/update_username` (with
  `tutorial:int`) and `/gasha/gasha_list`/`gasha_play`, then restart the login (B-boot §5).

## 5. What our server must store

Per account (keyed by `uuid`):

| Data | Needed by | Build 01 |
|---|---|---|
| `user_id`, `uuid`, username, comment, created/last-login time | login, `user_info`, `user_info_mini` | stored |
| rank, exp | `UserInfo` | stored (no level table yet) |
| tutorial flag list (verbatim as posted) | `get_tutorial_step` | stored; overridden by skip-all |
| currencies: free/paid stones, gold (`vm`), friend points, mind gasha point, soul, mentality | `UserInfo`, shop, gasha | free stones and gold stored; rest constant |
| stamina / battle point / raid point with their use and max times | `UserInfo`, quest/raid start, recover calls | constant placeholders |
| statue (character) inventory with per-instance level, skill tree squares, evolution, protect/new flags, storage | `statue/*`, `tree/*`, `team/*`, `book/statue_list` | not yet |
| mind (equipment) inventory, storages, equip slots | `mind/*`, `team/equip_*` | not yet |
| items with counts | `item/*`, `sell/sell`, strengthening | not yet |
| teams (normal and PvP), main-team flag, leader | `team/*`, `pvp/*`, MyPage leader model | not yet (empty team list) |
| story/quest progress: cleared stages, star clears, chapter unlocks, timed chapters | `bizarre/*` | not yet |
| interrupted battle (for resume) | `bizarre_resumption` | not yet (always 50301) |
| dungeon (tarot) run state | `dungeon/*` | not yet |
| trophies/missions, login bonus state, present box | `trophy/*`, `get_loginbonus`, `present/*` | not yet |
| event points, ranking, raid stages and history | `event/*`, `raid/*`, `pvp/*` (see 05) | not yet |
| follows/blocks, clan | `follow/*`, `clan/*` | not planned (offline; stand-ins only) |

Build 01 storage: one JSON file per account under `<data>/accounts/<uuid>.json`, written through
`storage::save_guarded` (validate, atomic write, backups) in `server/services/src/accounts.rs`.
Saves with export/import are milestone M3. Unknown request fields should be stored verbatim where a
later handler may need them.

## Open points / unknowns

- `tutorial_flag_table` contents: number of flags, ids, labels, which scripts run in 3.12 (needs the
  rebuilt `in_memory_master_data`).
- Exact semantics of `UserInfoResponse.halt` and valid `backgroundId` values (L).
- Server-side rules for timers (stamina regen uses `ConfigInfo.apOneRecoveryTime` etc.; formulas
  for "use time"/"max time" strings are inferred, M).
- Whether any server-side record was hash-checked by the client (none found so far; see 07).
