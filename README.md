# JDR Offline

A preservation project for **JoJo's Bizarre Adventure: Diamond Records** (Android, 3.12.0), whose
online service has ended. The goal: the game stays fully playable with **no network, forever**, in
English.

A small game server runs **inside the app** (`libjdr.so`, loaded into the game's own process) and
answers every request the game makes. It also rebuilds, on the phone, the files the dead server
used to provide: the asset manifest, the game's master tables and the English text. Multiplayer is
out of scope: the clan room is hidden, and PvP and raid rescue use local stand-ins.

> **No game files are in this repository, and none ever will be.** You patch **your own copy** of
> the APK, using **your own** downloaded game data. We ship tools and a server, never the game.

## Status

**Build 34.** Each build's changes and its device checklist are in
[docs/STATUS.md](docs/STATUS.md), and the plan is in [docs/ROADMAP.md](docs/ROADMAP.md). Nothing
counts as fixed until a device log shows it.

What the game does offline today:

- **Boot and save:** title, login, the asset manifest and master tables rebuilt on the phone, saves
  kept on the device.
- **English:** the menus, the item, statue and Mind names, the story and the in-APK messages, plus
  redrawn UI art where text is drawn into images (from a separate art pack).
- **Tactical battles:** turns, damage, skills and cool turns, the leader, assist and Mind
  abilities, Reversal, chains, the boss intro, the win screen and stage missions, using values
  rebuilt from public wikis. Script cameras for attacks, skills and SP skills are being matched to
  the original's footage; Build 34 still needs a device test.
- **Action battles:** enemy waves on the stage's spawn points, plus attack, skill, SP and finisher
  (still needs a device test).
- **Collection:** statues (stats for 206 of 292 from the wikis), Minds, the growth tree,
  enhancement, Mind equipment, assists, the team and My Page screens.
- **Economy and events:** missions, the Message Box, items and exchanges, free summons (statue,
  Mind, box and Count Up), story, daily and event quests, the Labyrinth, the Arena against local
  teams, and the Town.
- **A lighter APK:** crash reporting, analytics, ads, push and Play Games removed.

## How it works

1. `jdr-tool keys` recovers the keys the dead server used to hand out, from your own downloaded
   files. They are written to a local `keys.json`, which is never committed.
2. `jdr-patch` takes your APK and applies a list of fingerprinted, switchable patches:
   - point the server address at `http://127.0.0.1` and allow plain HTTP to it;
   - embed the server (`libjdr.so`), loaded from `libmain.so`;
   - remove the 32-bit libraries;
   - fix Unity 2018's allocator crash on Android 11+;
   - keep the downloaded files, and let the Town connect without DNS;
   - remove the online SDKs that cannot work offline;
   - put English text in the APK, and redrawn art if you have the pack.

   It then re-signs the APK with your own key. Each run writes a patch report.
3. On the phone, the embedded server speaks the game's encrypted protocol, serves your own game
   files, and generates what no player copy contains: the manifest, the master tables and the
   English bundles. All of it comes from the content files in `server/content`. During
   development the same server can run on a PC (`hosts/standalone`).

## Getting started

- Players: [docs/PLAYER-SETUP.md](docs/PLAYER-SETUP.md)
- Developers: [docs/DEV-LOOP.md](docs/DEV-LOOP.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
  [docs/CONTENT.md](docs/CONTENT.md), [docs/LOCALIZATION.md](docs/LOCALIZATION.md),
  [CONTRIBUTING.md](CONTRIBUTING.md)
- Research notes (how the client works, read from the disassembly and from footage):
  [docs/research/](docs/research/)

## Layout

```
server/crypto      client crypto formats + key recovery (no keys inside)
server/core        HTTP server, settings, logger, clock, storage, envelope
server/unity       Unity bundle and serialized-file reader/writer
server/services    one module per backend service; manifest, master data and English rebuilds
server/content     JSON content: statues, quests, master tables, translations, economy, events
hosts/standalone   the server as a PC process (dev loop)
hosts/embedded     libjdr.so for the patched APK
patcher            APK in -> patched, re-signed APK out (with a patch report)
tools/jdr-tool     key recovery and file inspection on your own copy
tools/content      builders that turn research and scans of your own files into content
tools/l10n         translation export, checks and layout hints
tools/ui-assets    UI art export from your own files
tools/proofs       checks run against your own files
docs/              architecture, protocol, research, status
scripts/build.sh   build everything, run every test the environment allows
```

## Credits

Server-side values that no player copy contains are rebuilt from public community wikis; see
[CREDITS.md](CREDITS.md).

## Legal

This is an unofficial, non-commercial preservation project with no affiliation to Bandai Namco,
the original developers, or the rights holders of JoJo's Bizarre Adventure. It contains no game
code, data or assets. Use it only with a copy of the game you obtained yourself.
