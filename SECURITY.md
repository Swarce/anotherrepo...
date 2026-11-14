# Security

## What the local server exposes

- An HTTP listener on **127.0.0.1 only** (port 18080). Requests whose `Host` header is not
  loopback are refused (blocks DNS-rebinding from a browser). Other apps on the same device can
  still reach a loopback port.
- What a local caller could read: the offline account state, the player's own game files under
  `/cdn/`, and the asset/sound keys from `/client/getkey` (recovered from the player's own copy;
  not personal data, but not something to publish). No personal credentials exist anywhere.
- Logs: key fields and device identifiers (`assetkey`, `assetvector`, `soundkey`, `signature`,
  `nonce`, `backupId`, `ad_id`, `platform_user_id`) are redacted from all log lines, so logs can be
  shared. Review a log before posting it publicly anyway.
- Limits: 64 concurrent connections (20 s idle timeout), 64 KiB of headers, 2 MiB bodies; handler
  panics are caught.

## In-game server (Build 02+)

- The server runs inside the game process and listens on 127.0.0.1:18080. Any app on the same phone
  can connect to it. Installed APKs are world-readable, so another app can also read the keys the
  patcher stored in the APK and talk the game's protocol: it could read or change the offline
  account, or tie up connections. There are no personal credentials, payments or network accounts to
  steal; the realistic harm is local mischief. Don't install untrusted apps alongside if that matters
  to you.
- If another app holds port 18080 first, the server keeps retrying and logs a FATAL line every
  minute; the game would then talk to that app.
- The "latch" (game keeps working if our module fails) covers Rust panics only. A native crash in the
  server thread ends the game process like any crash; the server code avoids `unsafe` except the one
  logcat call.
- No TLS (the game is redirected to plain `http://127.0.0.1`), no remote access, no telemetry.

## Signing key

- The patched APK is signed with the player's own key, stored outside the repo
  (`~/.config/JDR/jdr-signing.p12`, or `JDR_KEYSTORE`). Its keystore password defaults to
  `jdr-offline` (override with `JDR_KS_PASS`); the key only proves "same builder as last time".
- The certificate digest is pinned next to the key (`*.cert.sha256`); the patcher refuses a
  different key so a build can never silently stop installing over the previous one.

## Reporting

Open an issue without sensitive details, or contact the maintainer privately for anything that
could affect players' devices.
