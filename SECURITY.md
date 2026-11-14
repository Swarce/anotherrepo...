# Security

## What the local server exposes

- An HTTP listener on **127.0.0.1 only** (port 18080, fallback 18081). It is not reachable from the
  network. Other apps on the same phone can reach a loopback port; the server holds only the local
  offline account and game state, no credentials of any kind.
- No TLS (the game is redirected to plain `http://127.0.0.1`), no remote access, no telemetry.
- The patched APK is signed with the player's own key, stored outside the repo
  (`~/.config/JDR/jdr-signing.p12`, password from `JDR_KS_PASS`).

## Reporting

Open an issue without sensitive details, or contact the maintainer privately for anything that
could affect players' devices.
