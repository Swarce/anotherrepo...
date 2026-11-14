# Status

Newest build first. Nothing here is "fixed" or "working on device" until a device log shows it.

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
