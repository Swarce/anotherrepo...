# Status

Newest build first. Nothing here is "fixed" or "working on device" until a device log shows it.

## Build 10 — characters, teams, story quests from the wikis (branch `wip/b10`)

### What the Build 09 device log showed
Redrawn English art on screen; the My Page background was black (no team: the background and the
leader model come from the tactical main team's leader and its `statue_setting_table` row).

### What changed
- **Collection:** the player owns every obtainable statue (266: model, animations, action set and
  leader ability present in the player's files) at level 1, and 5 teams per battle type; the
  tactical main team (Jonathan, Jotaro, Josuke) is sent with login and team_info, which should give
  My Page its background (`bg_00NN` by JoJo part) and the leader model. Routes: `statue/statue_list`,
  `statue/statue_info`, `team/team_info`.
- **Statue numbers from the wikis:** rarity, type, Lv1/Lv99 and max-tree stats for 142 of 266 statues
  (129 from Gamerch matched exactly by leader-ability name, 13 from ShoutWiki by elimination); stats
  per level are linear Lv1→Lv99 (the wikis' Lv99 = 10 × Lv1 for 195 of 200 statues). The rest,
  mostly released after the 2018 snapshot, use marked defaults.
- **Minds:** the player owns the 370 minds matched to game ids on Gamerch (rarity, cost, flat
  stats); routes `mind/mind_list`, `mind/mind_info`.
- **Generated master tables:** `statue_setting_table` (266 rows), `statue_level_table` (266 × 99
  levels), `rank_table` (ranks 1–99 from Gamerch).
- **Story quests:** story button, 82 chapters (31 action, 51 tactical with stages) and 370 stages from
  the player's files; 100 stages carry Gamerch numbers (AP, rank EXP, gold, recommended level); battles
  are described with the player's area scenes and a wave of enemy statues; clears are recorded and
  grant gold and rank EXP; rank-ups and AP max follow the Gamerch rank table. Routes: `bizarre_list`,
  `stage_list`, `bizarre_start`, `bizarre_end`, `bizarre_continue`, `destruction`,
  `check_timed_chapter`, `support/support_user_s`, `clan/recommend`.
- **Wiki fetch:** ShoutWiki (351 pages), JoJoWiki (86 pages), Gamerch via the Wayback Machine (178
  archived data pages of 1,004; most were never archived). Raw pages stay local; only numbers with
  their source pages are committed (`docs/CONTENT.md`, `CREDITS.md`).
- Build number 10, server marker `JDR_EMBEDDED_SERVER_b010`.

### Known gaps / risks (expected)
- Battles have never run offline: enemy waves, spawn naming (`WAVE_A/01`), maps per stage and the
  empty battle tables (skills, abilities, reactions) are untested and may fail; the stage list and
  start/end should work.
- Enemy line-ups and rewards are defaults (the wiki shows them as images); stages after chapter 23
  and the tactical chapters have default numbers.
- Skills and abilities are not filled yet (statue details show none).

### Device checklist — expected; to be confirmed on a device
- [ ] My Page shows a background and the leader model (Jonathan)
- [ ] Team/Enhance and the statue list show the roster with names, rarity, levels, stats
- [ ] Quests → Story shows chapters and stages; starting a stage loads (or note where it stops)

## Build 09 — redrawn UI art (branch `wip/b09`)

### What came in
The owner's redrawn art pack: 464 sprites (all in English lettering) from 41 UI bundles and 4
title/boot screen files in the APK, at the exported sizes; terms Summon, Enhance,
Courage/Wisdom/Spirit/Unity/Solitude.

### What changed
- **Art import.** `jdr-patch … --art <pack.zip> --data <files>`: title/boot screens repainted in
  the APK (13 sprites, 4 files); menu art (451 sprites) stored in the APK (`assets/jdr/art.bin`,
  ~19 MB) and applied by the server on the phone while it rebuilds the English bundles (same
  originals, cache and single download). Atlases with replaced sprites are stored as RGBA32.
- `jdr_unity::texture`: ETC1/ETC2_RGBA8/RGB24/RGBA32/ARGB32/Alpha8 decode, PNG decode, paste.
- The bundle rebuild runs on up to 4 threads (desktop: ~1.1–1.5 s for all 60 bundles with art).
- Text terms aligned with the art (Gacha → Summon, Power Up → Enhance, Bravery/Intellect/
  Tenacity/Maverick → Courage/Wisdom/Spirit/Solitude).
- Server log: `[art] art pack loaded: N image(s)` and `[art] … sprite(s) replaced in this rebuild`.
- **Two reviews** (correctness; behaviour/risk). Fixed: sprite boxes now computed exactly as the
  exporter cut them (ties-to-even edges; three sprites would otherwise have been off by a pixel),
  the exporter's naming rule for repeated names, art generator version in the cache key, sprites
  with a separate alpha texture or rotation refused instead of drawn wrong, a bad image skips only
  itself (the bundle keeps its English), the patcher checks the whole pack against the player's
  bundles before building, title-screen layouts borrowed only from types with the same hash,
  macOS zip debris ignored, 2 rebuild workers when art is present, text fixes after the term
  switch, CONTRIBUTING: no game data in releases/issues/PRs.
- Build number 9, server marker `JDR_EMBEDDED_SERVER_b009`.

### Verified off-device
- Real data: all 451 menu sprites matched and replaced, 0 unmatched; UnityPy confirms every
  replaced sprite equals its PNG exactly and, in the textures whose originals are unencrypted, every
  other pixel equals the original (so the ETC decoder and the paste change nothing else).
- Real patch with the pack: 13 sprites in the 4 in-APK screen files equal their PNGs
  (`tools/proofs/art_check.py --apk`, run by build.sh).
- Removing the pack reverts: bundles with text go back to the text-only files, art-only bundles to
  their originals (real-data test).

### Known gaps / risks (expected)
- Animated lettering made of several sprites (e.g. SUCCESS!!, RETIRED, RATE UP!) may need spacing
  or timing adjustments (owner's notes); nine tutorial screens are not redrawn yet.
- RGBA32 atlases use 4x the memory of ETC2 (8x of ETC1) ones while those screens are open; check
  with `adb shell dumpsys meminfo com.bandainamcoent.jojodr` on the busiest menus.
- Storage on the phone (measured on the owner's files): originals kept 111 MiB, served English/art
  files 182 MiB (the game's own copies of those are replaced by the downloaded ones), plus the
  19 MB pack in the APK.

### Device result (Galaxy S22, Android 16) — from the owner's log and screenshots
- [x] `[art] art pack loaded: 451 image(s)`; `[l10n] 60 English file(s) ready in 1322 ms, 60 already
      on the phone` (cached after the first boot's download).
- [x] My Page buttons, header and side menu show the redrawn English art (Inbox, Notices, Missions,
      Friends, Home, Team/Enhance, Quests, My Town, Summon, Shop, Menu); Options screen in English.
- [ ] My Page 3D background is black: expected without a team (no leader statue, no
      `statue_setting_table` row) — next build.
- Town "Pick Up" dialog body still Japanese (description text tables, next batches); one route
  unknown: `clan/recommend` (now handled).

## Build 08 — English names (branch `wip/b08`)

### What the Build 07 device result showed
The download prompt appeared on the first boot; menu text labels show the English. Labels drawn
into images stay Japanese (the owner is redrawing them).

### What changed
- **Second English batch (names): 31,039 rows** from 7,690 unique strings: statue/character, event
  character and place names; skill, ability, buff and tarot names; Mind and item names, rewards and
  event shop items; chapter, stage, quest, tutorial and title names. With Build 07's batch, 33,336
  of the 50,845 text strings are English (0 stale), plus all 236 ClientMes messages.
- Names follow one name list (709 entries: characters, Stands, places, titles), so every batch
  spells them the same way.
- `jdr-tool l10n import --expand-with`: one translation per unique Japanese string, applied to
  every row with that source. New translation file `text_content.json`.
- Server log: `[l10n] … N already on the phone` (how many English files the client holds).
- Build number 8, server marker `JDR_EMBEDDED_SERVER_b008`.
- **Reviews:** code (nothing serious; `--expand-with` now checks the repo's files, not the compiled-in
  copy) and translation quality ("good": glossary followed, no placeholder or line-break errors).
  Fixed from the quality review: item names unified across tables, Hamon (not Ripple) throughout,
  Mind/skill titles reused in shop and reward tables synced to one English, the team screen's type
  labels (Bravery/Intellect/Tenacity/Maverick), a stray space in tarot effects, two mistranslations.

### Verified off-device
- Real data with the shipped translations: 32 English files, 33,336 strings replaced, 0 stale.

### Device checklist — expected; to be confirmed on a device
- [ ] Download prompt once (text.jp changed); names, skills, items, chapters show English
- [ ] `[l10n] 32 English file(s) ready …`, then on the next boot `32 already on the phone`

## Build 07 — English menus, UI art export (branch `wip/b07`)

### What the Build 06 device log showed
MyPage reached; five menu routes fell back to the unknown-route answer (present box, follow list,
items, shop, quest select).

### What changed
- **English in place** (docs/LOCALIZATION.md). The server rebuilds `text.jp` and the UI prefab
  bundles with English from `server/content/l10n/en/`, keeps the originals, and lists the English
  files in the manifest with their md5/size. The client downloads them once through its own update
  path (expect a download prompt on the first boot, about 100 MB). Replaced UI labels get "best fit"
  so longer English shrinks into its box. Log line: `[l10n] N English file(s) ready …`.
- **English boot/download/error messages**: new patch `english-clientmes` rewrites the in-APK
  ClientMes table (fingerprinted, `--disable english-clientmes`).
- **First English batch** (2,533 strings): all ClientMes messages (236), all UI prefab labels
  (1,210), and the menu text tables (1,087: My Page, options, title, team, training, evolution,
  filters/sorting, battle UI, quest types/results, arena, shop, gacha, special actions, …).
  Statue/skill/item names and descriptions, missions and events (~48,500 strings) follow in later
  batches; story scripts later still. The in-game terms text is replaced by a short "unofficial
  offline build" notice rather than translated.
- **Translator tools:** `jdr-tool l10n export` (every string of the player's copy into a local TSV)
  and `jdr-tool l10n import` (refuses lost placeholders, wrong hashes, Japanese in the English).
- **jdr-unity:** whole SerializedFile parse/write (layout and data offset preserved), generic
  type-tree decode/encode, Unity's common string table, LZ4 bundle writer.
- **Importer:** never overwrites the game's own `vcs.bytes` or our English files with a staging copy
  (otherwise a staging folder left in place would undo them at every boot).
- **Menu routes:** `get_bizarre_select`, `present_list`, `follow_list`, `item_list`, `shop_list` answer
  with a well-formed empty state.
- **UI art export** (`tools/ui-assets/export_ui.py`): every menu texture/atlas and cut-out sprite from
  the player's own copy, with a manifest for re-import (re-import comes later). Sent to the owner.
- Build number 7, server marker `JDR_EMBEDDED_SERVER_b007`.
- **Two reviews** (correctness; behaviour/risk). Both found the same serious bug, now fixed: after a
  translation update, the English file still on the phone could have been saved as the
  "original", losing the Japanese copy. Also fixed: cached English files kept their encryption
  flag; a bundle that loses its last translation is reverted; a failed state save offers no English
  that boot (instead of risking originals); a half-written or English file never replaces a kept
  original; `/cdn` serves from the last build instead of rebuilding mid-download; importer keep
  rules only while files are really ours; ClientMes post-check now counts the English rows;
  translator import requires the Japanese column and a well-formed hash; corrupt array counts are
  refused; the UI export manifest no longer pairs plain and obfuscated names; term fixes.

### Verified off-device
- Real data: all 287 serialized files / 137,728 objects of the UI bundles and text.jp decode through
  their type trees and encode back byte-identically; the files re-serialize identically.
- Real data: rebuilding every target with a placeholder for **every** string (50,845) replaces all of
  them; the rebuilt bundles hold no Japanese, parse with UnityPy (exact byte consumption), and a
  second run reuses the cache with identical md5s. Desktop: ~3 s first build, ~0.5 s cached.
- With the real translations: 32 English files, 2,297 strings replaced, 0 stale (+ 236 ClientMes in
  the APK); the real ClientMes rewrites byte-identically when nothing changes.

### Known gaps / risks (expected)
- First boot after installing builds the English files on the phone (desktop: ~2–3 s; the phone is
  slower) while the client waits for the manifest.
- Extra storage: originals + English copies (~220 MB).
- Best fit could make some labels small; layouts are untested on device.
- Text in UI images stays Japanese until the redrawn art is imported.

### Device result (Galaxy S22, Android 16) — from the owner's log (a later boot of Build 07)
- [x] `[l10n] 32 English file(s) ready in 1145 ms (0 rebuilt now …)`: the English files built on an
      earlier boot were reused from the cache (phone: ~1.1 s cached).
- [x] Boot to My Page as before; `unknown_routes=0`: the five menu routes now have handlers.
- [x] Import with the staging folder still present: `13480 already fine, 0 copied` (nothing undone).
- [x] First boot after installing showed the download prompt (owner); the English files were
      downloaded then, and later boots reuse them.
- [x] Text labels in the menus show the English (owner: "the ones that are text were translated
      fine"). Labels drawn into icons/images stay Japanese until the redrawn art is imported.

## Build 06 — master tables generated on the phone (branch `wip/b06`)

### What the Build 05 device logs showed (Galaxy S22, Android 16)
- Boot reached `getfiledata` for `in_memory_master_data` and stopped on `[cdn] … 404`: the bundle
  the dead server used to send with the game's master tables. No player copy has it (it was only
  ever held in memory).
- During an import, the client looped title → login until `vcs.bytes` arrived.

### What changed
- **`in_memory_master_data` is generated** by the server (embedded and standalone), next to the
  manifest, and listed in it with the md5 and size of exactly what `/cdn` serves (plain UnityFS,
  `isEncrypt=false`). Log line: `[assets] versionfile built: … in_memory_master_data: N tables (…)`.
- **Schema** (`server/content/masterdata/schema.json`): the 17 core assets and the serialized fields
  of the 57 types they reach, from our analysis of the client (docs/research/09-master-data.md);
  only what our content needs (code identifiers, no values or text). The bundle holds about 45
  assets, not 18: 27 more battle tables are read from the same bundle; their layouts are mapped in
  the research and join the schema with their content. Their boot readers null-check (checked in
  the code for the two read earliest).
- **Content:** `tutorial_flag_table` with the 34 flag labels the client queries (ids 0..33, no
  tutorial script downloads), so, with `get_tutorial_step` answering all ones, every tutorial and hint
  counts as done and the daily/event quest categories unlock. The other 16 core tables are written
  empty (each loader accepts an empty list); battle tables are left out until they have content.
- `jdr_unity::managed` (type trees + binary layout in the engine's own node shapes) and
  `jdr_unity::objects` (AssetBundle/MonoScript, shared with the manifest; manifest bytes unchanged).
- Import: leftovers of an interrupted import are removed; `/client/getkey` waits (≤ 12 s) while an
  import runs, so the game holds on the title instead of looping (the flag clears even if the import
  panics).
- Content files must carry well-formed sources (`jdr-research` + doc, `jdr-default` + note, outside
  sources + page + access date); CONTRIBUTING and CREDITS updated.
- `jdr-server --tutorial play` now warns: there are no tutorial scripts in the tables yet.
- If the master tables ever fail to build, the manifest is still built and served (without them),
  and the error is logged.
- Two reviews (correctness; behaviour/risk). The correctness review compared our type trees with real
  engine trees from the player's files (MonoBehaviour header, nested-class lists, strings, bools,
  Vector3, vectors: exact match) and found nothing serious. Fixed: preload order now object → script
  as the engine writes it; informational 0x8000 no longer set for lists of fixed-size structs;
  `List<bool>` refused until its packing is needed; duplicate content files refused; the import
  flag is set before the first request can arrive; the UnityPy check now compares every field
  (defaults included) with the content.
- Build number 6, server marker `JDR_EMBEDDED_SERVER_b006`.

### Verified off-device
- Real data, through the server context as on the phone: the manifest lists the master-data
  bundle with the md5 and size of exactly the bytes `/cdn` serves.
- Unit tests: the tutorial-table type tree equals the research template node for node; binary layout
  (string/bool alignment, lists); every class in the schema resolves and serializes; content rules
  (sources required, typos refused, enum names); deterministic bundle.
- Real data (`JDR_DATA`/`JDR_KEYS`): bundle built with the player's own MonoScript/AssetBundle type
  entries; `tools/proofs/masterdata_check.py` (UnityPy) reads all 17 tables at the client's container
  paths, each consuming exactly its bytes, and every tutorial row equals the content file.

### Known gaps / risks (expected)
- Whether the engine reads our objects through the shipped type trees (script hash differs from the
  build's) is expected from Unity's behaviour and from the manifest working, but is **first tested on
  the device** in this build.
- No team yet, so MyPage shows no leader model. Glossary empty. Battles, gacha scenes and special-move
  cameras need real rows (M4 content) and the battle tables.
- `rank_table` exists but is empty: player-rank lookups return a zero row (battle-result exp bar).

### Device result (Galaxy S22, Android 16) — confirmed by device log
- [x] `versionfile built: 13480 entries, 1845 KiB; in_memory_master_data: 17 tables
      (tutorial_flag_table 34 rows), 32 KiB; 1061 ms` (in the background, before the title).
- [x] `[cdn] GET /cdn/0a59….bytes 200 out=33596B (generated in_memory_master_data)`: the client
      accepted the generated tables (our MonoScripts and type trees) and left TitleLoad.
- [x] **MyPage reached** (owner: "We got to the menu"): `product_list`, `banner_list`, `user_info`,
      `team_info`, `check_forced_notify`, `event_list`, `bizarre_resumption`, `get_loginbonus`,
      `get_menu_notify`, then menu navigation (present box, follow list, items, shop).
- [x] No tutorial ran (the 34-flag table + all-ones `get_tutorial_step` work as intended).
- Unknown routes hit (answered with an empty OK): `bizarre/get_bizarre_select`,
  `present/present_list`, `follow/follow_list`, `item/item_list`, `shop/shop_list`. Next build adds
  proper handlers.
- Import re-run was a no-op (13,480 already fine, 10 s); the staging folder can be deleted.

## Build 05 — keep the game files, import them, review fixes (branch `wip/b05`)

### What the Build 04 device logs showed (Galaxy S22, Android 16)
- The account persisted across the update (login with the saved uuid, `existStatus 0`).
- `vcs.bytes` was missing: the game had **deleted its whole download folder**. Cause, from the
  client code: `ResourceDownloadUIController.StartLoad` runs a one-time "cache clear of non-encrypted
  files" (`DoCacheClearProcess` → `IOManager.DeleteAllInDirectory(persistentDataPath)`) whenever the
  PlayerPrefs flag behind `SaveDataManager.LoadNonObfuscatedClearFlag` is unset, which it is after a
  reinstall. Our server never writes there.
- After the owner pushed the files back with adb: `vcs.bytes: Permission denied`; the files belong to
  the shell user, the game falls back to the title and logs in again in a loop.

### What changed
- **`keep-files` patch** (libil2cpp @0xfd1534, fingerprinted): `LoadNonObfuscatedClearFlag` returns
  true, so the wipe never runs. `tools/proofs/keep_files_check.py` confirms on the player's binary
  that the cache clear is its only caller.
- **File importer** in the embedded host: copies `/data/local/tmp/jdr-import/files` into the game's
  `files/` as the app (replacing unreadable shell-owned copies, `vcs.bytes` last, temp file +
  rename, source untouched), then the manifest is built. Expected to work (apps may read
  world-readable files there); to be confirmed on a device.
- Boot diagnostics: `[assets] game files: N file(s), …` (vcs.bytes present? recorded files missing?).
- Manifest build outside the lock, failures cached for 30 s, poison-safe, template scan capped.
- **Review fixes (Builds 03–04):** `unity-heap` now uses 16 MiB pools (shorter pool/large lists in
  busy heaps; the proof allows exactly that one field to differ and also checks the calling
  convention, the callers and the storage size); bundle reader caps block/node counts and sizes;
  type-tree flags match Unity's (0x8000 on variable-size nodes); UnityPy check script for the
  manifest; `build.sh` lists skipped checks and requires the proofs when `JDR_APK` is set.
- Docs: restore step via the staging folder, missing-files and permission troubleshooting,
  `--disable unity-heap`, log redaction; CREDITS links.
- Build number 5, server marker `JDR_EMBEDDED_SERVER_b005`.

### Device result (Galaxy S22, Android 16) — confirmed by device logs
- [x] Import from `/data/local/tmp/jdr-import/files` works (apps may read it): first run copied
      11,000 files / 1.2 GB in ~20 s before the game was closed; the next run resumed and finished in
      13 s (2,380 copied, 2,378 unreadable shell-owned copies replaced, 0 failed).
- [x] `[assets] game files: 13480 file(s), all 13479 recorded files present`;
      `versionfile built: 13479 entries, 1845 KiB, 3544 ms` (background, before the title).
- [x] The client accepted the generated manifest: `[cdn] … 200 … (generated versionfile)`, **no other
      downloads** (every local file judged up to date), no errors.
- [x] Then `get_system_config`, `get_tutorial_step`, and `getfiledata` for `in_memory_master_data`
      → 404: the expected M1.5 stop.
- [ ] After a reinstall or "Clear storage", the files are no longer wiped (not tested yet).
- Note: during an import the client loops title → login until `vcs.bytes` arrives (it is copied
  last). Next build: hold `/client/getkey` until the import finishes; clean leftover temp files.

## Build 04 — generated asset manifest (branch `wip/b04`)

### What changed
- **`server/unity` (new, shared):** UnityFS read (LZ4/LZ4HC) and write (uncompressed),
  SerializedFile v17 type-table parse and write, type trees for our MonoBehaviour classes.
- **`versionfile` generated by the server** from the game's own `files/` folder (read-only): one
  entry per file recorded in `vcs.bytes` (path = decoded name + `.bytes`, obfuscated name, the md5 the
  game recorded, real size, encrypted flag from the JJEC header). Engine type definitions
  (MonoScript, AssetBundle) are lifted at runtime from one of the player's bundles and must match
  their fingerprints. Cached until `vcs.bytes` changes; the embedded host builds it in the
  background at boot. Served when `getfiledata` asks for the manifest's obfuscated name.
- Settings split: `files_dir` (CDN import folder) vs `game_files_dir` (manifest source).
- Content policy: wiki-sourced values committed with sources, credited in `CREDITS.md`.
- Build number 4, server marker `JDR_EMBEDDED_SERVER_b004`.

### Verified off-device
- Real data: 13,479 entries (4,368 encrypted), 1.89 MB bundle, built in ~0.1 s on a PC.
- UnityPy (an independent Unity reader) parses the bundle (`tools/proofs/versionfile_check.py`, run
  by `build.sh` with `JDR_DATA` + `JDR_KEYS`): container
  `assets/resources/version/android/versionfile.asset`, MonoScript
  `ScSystem.ScIO.AssetBundleVersionInfo` / `Assembly-CSharp.dll`, every entry decodes through our
  type tree.
- End to end on the standalone server: `getfiledata` returns the URL for the manifest's obfuscated
  name, and that URL serves the generated bundle (cached on repeat).

### Known gaps / risks (expected)
- `in_memory_master_data` is not built yet: the game is expected to stop at that request (M1.5,
  Build 05).
- Our `AssetBundleVersionInfo` type carries its own script hash, so the engine must read it through
  the type tree (the standard path for bundles with type trees). To be confirmed on a device.

### Device checklist — expected; to be confirmed on a device
- [ ] `[assets] versionfile built: … entries` in the `JDR` log soon after boot
- [ ] `[cdn] GET /cdn/<manifest>.bytes 200 … (generated versionfile)`
- [ ] no re-download of game files (no other `/cdn/` requests for existing files)
- [ ] next requests: get_system_config / get_tutorial_step and/or the master-data bundle (expected stop)

## Build 03 — Unity allocator crash fix (branch `wip/b03`)

### What the Build 02 device log showed (Galaxy S22, Android 16)
- **The embedded server works on the device:** `jdr embedded build 2` (+5 ms), `keys loaded`
  (+19 ms), `listening on 127.0.0.1:18080`; then `POST /common/get_version 200` twice and
  `POST /api/user/login 200` (new user, `existStatus 4`).
- 26 ms after the login reply the game crashed in `libunity.so` (SIGSEGV, fault addr 0x106645) inside
  `JsonUtility.FromJson`, right after Unity's own error **"Using memoryadresses from more that 16GB
  of memory"**. Growthbeat (analytics) failed offline and caught its own exception (not the cause).

### Cause
Unity 2018.4.1's `UnityDefaultAllocator` (ALLOC_DEFAULT, ALLOC_GAMEOBJECT, ALLOC_GFX) records every
malloc'd block in a page table with 5 slots of 4 GiB address windows. Android 11+'s Scudo allocator
spreads memory over more windows; the slot search returns -1 and the next access faults (crash
frames 0x9ffdb4 / 0x9ff968 are exactly that code). This is an engine bug on new Android, not a
server-data problem; Unity fixed it in a later 2018.4 release. The `-systemallocator` switch does
not help: in this release build its result is ignored.

### What changed
- **`unity-heap` patch** (evidence: the boot crash is gone on the device; long sessions, memory use
  and frame pacing with the new allocators are not yet observed): replaces the 96-byte body of the `UnityDefaultAllocator` constructor
  (libunity @0x9ff824, fingerprinted by library and window SHA-256) with code that builds a
  `DynamicHeapAllocator`, field for field like the engine's own inline `ALLOC_FALLBACK` heap. The
  proof script checks on the real binary that only the three allocators above call that
  constructor, that the page-table code is reachable only from its own class, and that each object
  has 0x140 bytes of storage (the constructor writes below 0x118). The new heaps lock (like the
  fallback heap) and use 1 MiB pools.
- **Equivalence proof on the real binary** (`tools/proofs/unity_heap_equiv.py`; `build.sh` runs it
  and requires it whenever `JDR_APK` is set):
  arm64 emulation of the patched constructor vs the engine's inline construction on a poisoned
  object → 129 bytes written, **0 bytes differ**, same mutex-init call.
- Build number 3, server marker `JDR_EMBEDDED_SERVER_b003`.

### Device result (Galaxy S22, Android 16) — confirmed by device log
- [x] no "Using memoryadresses from more that 16GB" line, no SIGSEGV after login (fix confirmed)
- [x] terms page served (`GET /web/information/terms_a.html 200`), Accept → `get_nonce` →
      `[account] created user_id=100000001` → second login with the new uuid
- [x] `/api/client/getkey 200`, then `getfiledata` for **one** file: `versionfile` (decoded name) →
      `GET /cdn/<obfuscated>.bytes 404` → the expected stall (M1.5 builds `versionfile`)
- [ ] `get_system_config` / `get_tutorial_step` not seen yet (they come after the manifest)
- Notes: Growthbeat throws offline and catches it itself (harmless; debloat in M6). The process
  ended with SIG 9 (app closed), not a crash.

## Build 02 — embedded server (branch `wip/b02`)

### What changed
- **`libjdr.so`** (`hosts/embedded`, aarch64, NDK r28c API 21, 16 KiB pages): the same server as
  `jdr-server`, inside the game process. A constructor only spawns a thread (the game never waits);
  that thread finds the app's own APK from where `libjdr.so` is mapped, reads the player's keys from
  the stored asset `assets/jdr/keys.json`, and serves on 127.0.0.1:18080 (retries forever if the port
  is taken, FATAL line once a minute). State in `/data/user/<n>/<pkg>/jdr/` (adoptable-storage
  fallback); logs and save backups copied to `/sdcard/Android/data/<pkg>/jdr/{logs,saves}/`
  (changed files only, logs pruned); every log line mirrored to logcat, tag `JDR`.
- **`embed-server` patch:** adds `lib/arm64-v8a/libjdr.so` (validated: aarch64, 16 KiB, build
  marker `JDR_EMBEDDED_SERVER_b002` shown in the report), embeds the keys asset, and renames
  `libmain.so`'s `liblog.so` dependency to `libjdr.so` (same length, in place; libmain imports no
  liblog symbols, and libjdr depends on liblog itself). Fingerprinted on the libmain SHA-256; the ELF
  editor proves the string is unshared (dynamic tags, verneed/verdef, all symbols via DT_HASH,
  tail-merged strings) and returns errors on malformed input.
- `--port` is refused with the in-game server; `--disable embed-server` builds the PC dev-loop APK.
- Server limits: 2 MiB bodies, 32 connections, 20 s read/write timeouts.
- `build.sh` builds `libjdr.so` when `ANDROID_NDK` is set and only embeds a library built in the
  same run.
- Docs: PLAYER-SETUP rewritten for the in-game server (inline steps, success checklist, recovery
  without uninstall, save/key warnings), SECURITY in-game section, DEV-LOOP `adb reverse --remove-all`.
- Two independent reviews (correctness inside the game process; behaviour/risk): all findings fixed.

### Tests
| Suite | Result |
|---|---|
| core | 16 passed |
| crypto (+ real data with `JDR_DATA`) | 14 + 1 passed |
| embedded (zip reader, own-APK detection with foreign APKs mapped, boots from an APK asset and answers get_version, log pruning, marker) | 7 passed |
| patcher (incl. ELF NEEDED swap and its refusals) | 15 passed |
| services | 9 passed |
| end-to-end boot sequence | 5 passed |
| real patch with the embedded server (`JDR_APK`, `JDR_KEYS`, `ANDROID_NDK`) | all post-checks ok |

Verified off-device: `libjdr.so` has one constructor in `.init_array` (relocated to `jdr_init`),
BIND_NOW, 16 KiB LOAD alignment; patched `libmain.so` NEEDED = libjdr, libstdc++, libm, libdl, libc;
the keys asset reads back from the signed APK (after the APK Signing Block) identically to `zipfile`.

### Known gaps (expected)
- Same stall point as Build 01: `versionfile` and `in_memory_master_data` (M1.5).
- The "game keeps working if our module fails" latch covers Rust panics only; if `libjdr.so` cannot
  load, the game cannot start (recovery documented, no uninstall needed).

### Device checklist (Galaxy S22, Android 16) — expected; to be confirmed on a device
Follow docs/PLAYER-SETUP.md. Send `local/logcat.txt` and `adb pull …/jdr/logs`.
- [ ] patched APK installs; the game launches (does not close immediately)
- [ ] logcat `JDR`: `jdr embedded build 2`, `keys loaded`, `listening on 127.0.0.1:18080`
- [ ] `[api] POST /common/get_version 200` at the title screen
- [ ] terms page → `[account] created` → second login
- [ ] getkey / get_system_config / get_tutorial_step / getfiledata, then the expected stall
- [ ] `/sdcard/Android/data/com.bandainamcoent.jojodr/jdr/logs/` and `…/saves/` appear
- [ ] restart the game: same account (`existStatus 0`), no FATAL bind line

## Build 01 — dev loop groundwork (branch `wip/b01`)

### What changed
- **Research (M0):** docs/research/01–08, docs/PROTOCOL.md (171 request classes → paths),
  docs/protocol/{envelope,boot,assets}.md. Decision on §2.2: Rust server, arm64 only, redirect by
  rewriting one metadata string literal + allowing cleartext HTTP; the server then hands out its own
  API/web bases.
- **Key recovery** (`jdr-tool keys`): asset key + IV recovered from the player's own files (frequency
  attack on obfuscated names, IV from the fixed UnityFS header), envelope key read from the player's
  own APK (fingerprinted), sound key supplied by the player. Checks the copy is complete against the
  game's own `vcs.bytes` record. Keys go to a git-ignored `local/keys.json`.
- **Patcher v0** (`jdr-patch`): `server-url` (metadata literal #4137 → `http://127.0.0.1:18080`),
  `cleartext` (`android:usesCleartextTraffic="true"` via a binary-manifest editor), `drop-armv7`.
  Build fingerprinted on metadata, libil2cpp and manifest SHA-256. zipalign (16 KiB pages for .so),
  apksigner v1+v2+v3, signing-key pin, input/output guards, patch report with post-checks.
- **Server v0** (`jdr-server`, standalone): std-only HTTP/1.1 server on 127.0.0.1, envelope codec,
  local accounts (schema-versioned, backups, unreadable save ≠ new user), the new-player boot chain
  (get_version, login, get_nonce, create_user, getkey, get_system_config, get_tutorial_step,
  tutorial, getfiledata, product_list, user_info, update_username, team_info, check_forced_notify,
  event_list, banner_list, get_loginbonus, get_menu_notify, bizarre_resumption), terms page, CDN from
  the player's files, per-request log lines, `[status]` line, secret-field redaction, loopback-Host
  check. Unknown routes answer OK and are logged as `UNKNOWN ROUTE`.
- Two independent reviews (correctness; behaviour/risk): all findings fixed except the content
  question below.

### Tests
| Suite | Result |
|---|---|
| core (http, envelope, clock, storage) | 16 passed |
| crypto (aes, names, containers, key recovery) | 14 passed |
| crypto real-data (`JDR_DATA`): key recovery, all 4,368 encrypted bundles decrypt, vcs md5/size valid | 1 passed |
| patcher (metadata, AXML, zip, signing parsers, patches) | 12 passed |
| services (accounts, CDN, redaction, host check, URLs) | 9 passed |
| end-to-end over a socket (boot sequence in client order) | 5 passed |
| real patch of the 3.12.0 APK (`JDR_APK`) | all post-checks ok; `aapt2` parses the manifest |
| `cargo check --target aarch64-linux-android` (shared code) | ok |

### Known gaps (expected)
- `versionfile` and `in_memory_master_data` do not exist in any player copy (they were only ever
  held in memory). Until M1.5 rebuilds them, **TitleLoad is expected to stall** after getkey /
  get_system_config / get_tutorial_step / getfiledata.
- Tutorial is skipped by default (`--tutorial play` exists, but the tutorial needs master data).
- Sound key unverified (public list entry); a wrong key means silent downloaded audio.
- No team/statues yet: MyPage would show no leader model.

### Device checklist (Galaxy S22, Android 16) — expected; to be confirmed on a device
Follow docs/DEV-LOOP.md. Please send the newest `local/server/logs/server-*.log` and
`local/logcat.txt`.
- [ ] `jdr-tool keys` prints "copy is complete"
- [ ] patched APK installs (arm64), icon launches
- [ ] title screen; server log shows `POST /common/get_version 200`
- [ ] tap start → terms page appears with an Accept button (`GET /web/information/terms_a.html 200`)
- [ ] account created (`[account] created …`), second `/api/user/login` with existStatus 0
- [ ] `/api/client/getkey`, `/api/common/get_system_config`, `/api/user/get_tutorial_step` answered
- [ ] which files `getfiledata` asks for (log line `[assets] getfiledata …`) and where it stalls
- [ ] any `UNKNOWN ROUTE` lines
- [ ] logcat: exceptions after the last server request (to confirm the stall cause)

### Open
- **Content policy question (owner):** commit wiki-sourced values (stats, skill numbers) for the
  master-data rebuild as cited JSON, or keep them out of the repo.
- M1.5 design: docs/design/master-data.md (draft).
