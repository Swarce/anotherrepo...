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
- Limits: 64 concurrent connections, 64 KiB of headers, 16 MiB bodies; handler panics are caught.
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
