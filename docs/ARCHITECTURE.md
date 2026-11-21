# Architecture

## What the game has, and what is dead

| Part | State offline |
|---|---|
| Unity 2018.4 IL2CPP client (arm64 + armv7), game code in `libil2cpp.so` + `global-metadata.dat` | kept; we patch data only (one string literal, one manifest attribute). Behaviour offline: to be confirmed on a device |
| API server `jdr.channel.or.jp` (encrypted form posts, ~171 endpoints) | **dead** → our server |
| Asset CDN (S3, URLs handed out by `/client/getfiledata`) | **dead** → our server serves the player's own downloaded files |
| `versionfile` bundle (server's asset manifest, fetched into memory each boot) | **dead, never stored** → we build it |
| `in_memory_master_data` bundle (master tables, fetched into memory each boot) | **dead, never stored** → we build it from our own schema + content (Build 06; research 09) |
| Asset/sound keys (`/client/getkey`) | **dead, never stored** → recovered from the player's files / public sound-key list |
| Clan room (realtime socket) | dead → hidden |
| Firebase, Play Games, AdMob, Adjust, GrowthPush, SmartBeat, Prime31 IAP | IAP stays uninitialised (empty product key); the others are expected to fail harmlessly (to be confirmed on a device); removal in M6 (debloat) |

## Components

```
              player's PC                                   phone
 ┌──────────────────────────────┐         ┌────────────────────────────────────────┐
 │ jdr-tool keys  ← your files/ │         │ patched APK                            │
 │   → local/keys.json          │         │  libil2cpp (unchanged code)            │
 │ jdr-patch     ← your APK     │ ──────► │  metadata: server URL → 127.0.0.1:18080│
 │   → deliver/*.apk + report   │  adb    │  manifest: usesCleartextTraffic        │
 │ jdr-server (dev loop) ◄──────┼─reverse─┤  or: libjdr.so in-process (embed-server)│
 └──────────────────────────────┘         └────────────────────────────────────────┘
```

- `server/crypto` — client formats: AES-CBC, name obfuscation, JJEC/vcs containers, key recovery.
- `server/core` — HTTP/1.1 server (std only), envelope codec, logger, clock (JST), storage.
- `server/services` — dispatch + one module per service; `app.rs` routes `/common/get_version`
  (bare host), `/api/*` (envelope), `/cdn/*` (asset files), `/web/*` (WebView pages).
- `hosts/standalone` — `jdr-server` for the dev loop.
- `hosts/embedded` — `libjdr.so` (aarch64, 16 KiB pages). The `embed-server` patch renames
  `libmain.so`'s `liblog.so` dependency to `libjdr.so` (same length, in place); `libjdr.so` itself
  depends on `liblog.so`, so libmain's imports still resolve. Its constructor only spawns a thread
  that reads the keys from `assets/jdr/keys.json` in the app's own APK (stored by the patcher),
  binds 127.0.0.1:18080 and serves. State: `/data/user/<n>/<pkg>/jdr/`; logs copied to
  `/sdcard/Android/data/<pkg>/jdr/logs/` and mirrored to logcat (tag `JDR`); extra CDN files from
  `/sdcard/Android/data/<pkg>/jdr/cdn/` (never the game's own `files/` folder).
- `patcher` — `jdr-patch`: fingerprinted patches, zip rewrite, zipalign, apksigner, report.
- `tools/jdr-tool` — keys, decoded name listing, decryption of the player's files.

## How the game reaches the server

1. The client starts with `HttpConst.ServerUrl = "https://jdr.channel.or.jp"` (metadata literal #4137).
   The patcher rewrites it to `http://127.0.0.1:18080` (same-length-or-shorter, in place, fingerprinted).
2. API calls go through `ScNet.HttpAndroid` → Java `HttpURLConnection`, which honours Android's
   cleartext policy; the `cleartext` patch sets `android:usesCleartextTraffic="true"`.
3. First call: `POST /common/get_version`. Our answer sets `url = http://127.0.0.1:<port>/api`
   (API base) and `s3_url = http://127.0.0.1:<port>/web` (WebView pages).
4. Assets: the client asks `/client/getfiledata` for URLs and downloads them with Mono `WebClient`;
   we answer `http://127.0.0.1:<port>/cdn/<file>`, served from the player's own copy.

## Build constraints

- Rust 1.97.0 pinned; shared code must build for `aarch64-linux-android` (docs/MODULES.md).
- arm64 only. The patcher removes `lib/armeabi-v7a`.
- No keys, no game data in the repo; everything game-specific is read from the player's copy at
  patch/run time.
