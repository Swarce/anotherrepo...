# Contributing

## Legal rules (non-negotiable)

- **No game files in the repo, ever:** no APK, libraries, metadata, decompiled code, dumps, bundles,
  text extracted from the game, textures, audio, or keys. `.gitignore` blocks the common ones; review
  your diff anyway.
- Tests that need the real game read the developer's own copy through env vars and **skip** without
  them: `JDR_APK` (the APK), `JDR_DATA` (the game's `files/` folder), `JDR_KEYS` (a local keys.json).
- Translations are stored as `label -> English` plus a hash of the source line, never the Japanese
  source text.
- No real payments, no tracking. Analytics/ads/crash endpoints are accepted and dropped.
- Use real user data only from logs the user sends; synthetic data everywhere else.

## Patch rules

- **Fingerprint every patch site** against a verified build (`patcher/src/known.rs`). No match: the
  patch refuses. Nothing silently half-patches.
- **One patch per concern**, each with an id, a one-line description (it appears in the patch
  report), a `--disable <id>` switch and tests.
- Calls from game code into our code go through a latch: if our module is missing or fails, the game
  runs its original path.
- Behaviour/performance patches need an equivalence proof on the real binaries (original vs patched,
  same inputs, compare outputs and side effects).
- Every debloat removal records its reason.
- Every output is verified: zipalign, apksigner v1+v2+v3, certificate digest, post-checks, report.

## Code rules

- Shared code follows docs/MODULES.md.
- `cargo fmt`, `cargo clippy`, `scripts/build.sh` green before a PR.
- Never say something is fixed before a device log shows it; docs say "expected; to be confirmed on
  a device".

## Build cadence

Each build has a number (`jdr_core::BUILD`), a branch `wip/bNN`, a "Build NN" section in
docs/STATUS.md and one PR using `.github/pull_request_template.md`.
