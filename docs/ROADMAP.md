# Roadmap

## Done
- Build 01: research M0 (in progress), key recovery from the player's files, patcher v0, standalone server v0.

## Milestones

| | Content | State |
|---|---|---|
| M0 | Research notes 01–08, protocol index, §2.2 decision | in progress (Build 01) |
| M1 | URL-patched game reaches the home screen against the standalone server over `adb reverse` | started (Build 01) |
| M2 | Embedded `libjdr.so` (libmain.so's liblog.so dependency renamed to libjdr.so), no network — **built in Build 02, device test pending**. The CDN must never serve from the folder the client downloads into (WebClient truncates the destination first); serve from a read-only copy | |
| M3 | Saves: verbatim storage, backups, export/import | |
| M4 | Economy, gacha, shop, present box, coupons (content from public wikis, cited) | |
| M5 | Event rotation, quests, PvP stand-ins, raid NPC helpers, simulated-clock run | |
| T1–T3 | English: text.en table, story scripts, server-authored text (rewritten, no real dates) | |
| M6 | Settings page, diagnostics, debloat, hitches from device logs | |
| 1.0 | Release checklist below | |

## Decisions
- Server language: **Rust** (no second runtime in Unity's process; mature Android target).
- **arm64 only**; armeabi-v7a removed by the patcher.
- Redirect: rewrite the single production URL literal in `global-metadata.dat`; the server then
  supplies the API and CDN base itself (`get_version.url` / `s3_url`).
- Multiplayer: clan room hidden; PvP vs generated rival teams; raid rescue by NPC helpers.
- Events: fresh rotation, texts rewritten without real-world dates.
- Content source for server-only data: public wikis. **Committed** in `server/content/` as JSON with
  per-file sources and credited in `CREDITS.md` (facts and numbers only, no copied prose).

## 1.0 release checklist
- [ ] Patched APK installs over the previous build and keeps the save
- [ ] Fresh install → tutorial → home with no network (device log)
- [ ] Every endpoint the client can call has a handler (unknown-route counter is 0 in a full playthrough log)
- [ ] English UI table and story scripts complete; overflow pass done
- [ ] Saves export/import round trip on device
- [ ] Simulated 8-week clock run green
- [ ] Debloat list with reasons; no third-party host contacted (device log / network off)
- [ ] PLAYER-SETUP walkthrough tested by someone who did not write it

## Later
- iOS (no build available), audio key verification from more archives, HD texture packs (no).
