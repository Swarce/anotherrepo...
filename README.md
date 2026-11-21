# JDR Offline

this is my project to keep **JoJo's Bizarre Adventure: Diamond Records** (Android, 3.12.0) alive
after the servers shut down. the goal is simple: the game should stay fully playable with **no
internet, forever**, and in English.

how it works: a small game server runs **inside the app itself** (`libjdr.so`, loaded into the
game's own process) and answers everything the game asks for. it also rebuilds, right on your
phone, the stuff the dead server used to hand out: the asset manifest, the game's master tables
and the English text. multiplayer isn't part of this. the clan room is hidden, and PvP and raid
rescue use local stand-ins.

> **there are no game files in this repo, and there never will be.** you patch **your own copy**
> of the APK with **your own** downloaded game data. i only ship the tools and the server, never
> the game.

## where it's at

we're on **Build 37**. every build's changes and what to check on a phone are in
[docs/STATUS.md](docs/STATUS.md), and the plan is in [docs/ROADMAP.md](docs/ROADMAP.md). nothing
counts as fixed until a device log shows it.

what works offline right now:

- **boot and saves:** title, login, the manifest and master tables rebuilt on the phone, and your
  saves stay on the device.
- **English:** menus, item, statue and Mind names, the story and the in-APK messages. there's also
  redrawn UI art for text that's baked into images (a separate art pack).
- **tactical battles:** turns, damage, skills and cool turns, leader, assist and Mind abilities,
  Reversal, chains, the boss intro, the win screen and stage missions, with values rebuilt from
  public wikis. the battle cameras are still being matched to the original's footage, and Build
  37 still needs testing on a phone.
- **action battles:** enemy waves on the stage's spawn points, plus attack, skill, SP and finisher
  (also still needs a phone test).
- **collection:** statues (real stats for 206 of 292, from the wikis), Minds, the growth tree,
  enhancement, Mind equipment, assists, and the team and My Page screens.
- **economy and events:** missions, the Message Box, items and exchanges, free summons (statue,
  Mind, box and Count Up), story, daily and event quests, the Labyrinth, the Arena against local
  teams, and the Town.
- **a lighter APK:** crash reporting, analytics, ads, push and Play Games are all gone.

## how the patching works

1. `jdr-tool keys` pulls the keys the dead server used to hand out from your own downloaded files.
   they get written to a local `keys.json`, which never gets committed.
2. `jdr-patch` takes your APK and applies a set of patches (each one is fingerprinted and can be
   switched off):
   - point the game at `http://127.0.0.1` and allow plain HTTP to it;
   - embed the server (`libjdr.so`), loaded from `libmain.so`;
   - remove the 32-bit libraries;
   - fix Unity 2018's allocator crash on Android 11+;
   - keep the downloaded files, and let the Town connect without DNS;
   - strip the online SDKs that can't work offline anyway;
   - put English text in the APK, plus the redrawn art if you have the pack.

   then it re-signs the APK with your own key and writes a patch report.
3. on the phone, the embedded server speaks the game's encrypted protocol, serves your own game
   files, and generates everything a player copy doesn't have (the manifest, the master tables and
   the English bundles) from the content files in `server/content`. for development the same
   server can also run on a PC (`hosts/standalone`).

## getting started

- players: [docs/PLAYER-SETUP.md](docs/PLAYER-SETUP.md)
- devs: [docs/DEV-LOOP.md](docs/DEV-LOOP.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md),
  [docs/CONTENT.md](docs/CONTENT.md), [docs/LOCALIZATION.md](docs/LOCALIZATION.md),
  [CONTRIBUTING.md](CONTRIBUTING.md)
- research notes (how the client works, from the disassembly and from old gameplay footage):
  [docs/research/](docs/research/)

## what's where

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
scripts/build.sh   builds everything and runs every test the environment allows
```

## credits

the server-side values no player copy has were rebuilt from public community wikis, see
[CREDITS.md](CREDITS.md).

## legal

this is an unofficial, non-commercial preservation project. it's not affiliated with Bandai Namco,
the original developers, or the rights holders of JoJo's Bizarre Adventure. it contains no game
code, data or assets. only use it with a copy of the game you got yourself.
