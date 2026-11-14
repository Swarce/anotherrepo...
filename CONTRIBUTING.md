# Contributing

## Legal rules (non-negotiable)

- **No game files in the repo, ever:** no APK, libraries, metadata, decompiled code, dumps, bundles,
  text extracted from the game, textures, audio, or keys. `.gitignore` blocks the common ones; review
  your diff anyway.
- Tests that need the real game read the developer's own copy through env vars and **skip** without
  them: `JDR_DATA` (the game's `files/` folder; crypto real-data test) and `JDR_APK` +
  `JDR_BUILD_TOOLS` (real patch run in `scripts/build.sh`). A skipped suite prints "skipped" in the
  test output; check for it before claiming a real-binary pass.
- Translations are stored as `label -> English` plus a hash of the source line, never the Japanese
  source text.
- **Server-only content** (values the dead server held: stats, skill numbers, curves, gacha pools,
  prices, stages) is committed under `server/content/` as JSON **with its sources**: every file has a
  `sources` list (site, page, date accessed), and every source appears in `CREDITS.md`. Numbers and
  facts only; never copy descriptions or other prose from the sources. Values we chose ourselves are
  marked `"source": "jdr-default"` (with a `note`); facts from our own analysis of the client are
  marked `"source": "jdr-research"` (with the `doc` they are written up in). An outside source needs
  its `page` and the date it was `accessed`. The server refuses a content file that breaks this.
- `server/content/masterdata/schema.json` holds only the field layouts our content needs (code
  identifiers, no values or text); it grows with the content, never as a full metadata dump.
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
