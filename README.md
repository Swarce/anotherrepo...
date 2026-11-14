# JDR Offline

A preservation project for **JoJo's Bizarre Adventure: Diamond Records** (Android, 3.12.0), whose
online service has ended. The goal: the game stays fully playable with **no network, forever**, in
English.

A small game server runs **inside the app** (`libjdrserver.so`, started in the game's own process)
and answers every request the game makes: login, saves, progression, economy, store, present box
and a schedule of rotating events. Multiplayer is out of scope: the clan room is hidden, PvP and
raid rescue use local stand-ins.

> **No game files are in this repository, and none ever will be.** You patch **your own copy** of
> the APK, using **your own** downloaded game data. We ship tools and a server, never the game.

## Status

Build 01 (dev loop). See [docs/STATUS.md](docs/STATUS.md) and [docs/ROADMAP.md](docs/ROADMAP.md).
Nothing is "fixed" until a device log shows it.

## How it works

1. `jdr-tool keys` recovers the keys the dead server used to hand out, from your own downloaded
   files (they are written to a local `keys.json`, never committed).
2. `jdr-patch` takes your APK, rewrites the server address to `http://127.0.0.1:18080`, allows plain
   HTTP to it, removes the 32-bit libraries, and re-signs it with your own key (debloat of SDKs that
   cannot work offline comes in M6).
3. The server (embedded from M2; standalone on your PC for now) speaks the game's encrypted protocol
   and serves the game's asset files from your own copy.

## Getting started

- Players: [docs/PLAYER-SETUP.md](docs/PLAYER-SETUP.md)
- Developers: [docs/DEV-LOOP.md](docs/DEV-LOOP.md), [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [CONTRIBUTING.md](CONTRIBUTING.md)

## Layout

```
server/crypto      client crypto formats + key recovery (no keys inside)
server/core        HTTP server, settings, logger, clock, storage, envelope
server/services    one module per backend service
server/content     JSON content (economy, store, events, text overlays)
hosts/standalone   the server as a PC process (dev loop)
hosts/embedded     libjdrserver.so for the patched APK (M2)
patcher            APK in -> patched, re-signed APK out
tools/jdr-tool     key recovery and file inspection on your own copy
docs/              architecture, protocol, research, status
scripts/build.sh   build everything, run every test the environment allows
```

## Legal

This is an unofficial, non-commercial preservation project with no affiliation to Bandai Namco,
the original developers, or the rights holders of JoJo's Bizarre Adventure. It contains no game
code, data or assets. Use it only with a copy of the game you obtained yourself.
