#!/usr/bin/env bash
# Build everything and run every test suite the environment allows.
#   JDR_DATA   the game's files/ folder (your own copy)   -> real-data tests
#   JDR_APK    your own APK                                -> real-binary patch test
#   JDR_BUILD_TOOLS  folder with zipalign + apksigner      -> needed for the patch test
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$*"; }

step "format check"
cargo fmt --all -- --check

step "build (host)"
cargo build --workspace --release

step "shared code builds for Android (aarch64-linux-android)"
cargo check -p jdr-crypto -p jdr-core -p jdr-services -p jdr-embedded --target aarch64-linux-android

step "clippy"
cargo clippy --workspace --all-targets -- -D warnings

step "unit + integration tests (real-data suites skip without env vars)"
cargo test --workspace --release

if [[ -n "${JDR_APK:-}" && -n "${JDR_BUILD_TOOLS:-}" ]]; then
  step "real-binary: patch your APK"
  mkdir -p deliver
  JDR_KEYSTORE="${JDR_KEYSTORE:-$HOME/.config/JDR/jdr-signing.p12}" \
    ./target/release/jdr-patch patch --apk "$JDR_APK" --out deliver/jdr-test.apk
else
  step "real-binary patch test skipped (set JDR_APK and JDR_BUILD_TOOLS)"
fi

step "done"
