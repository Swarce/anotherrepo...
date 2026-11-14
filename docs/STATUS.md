# Status

Newest build first. Nothing here is "fixed" or "working on device" until a device log shows it.

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
