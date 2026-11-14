#!/usr/bin/env bash
# Build everything and run every test suite the environment allows.
#   JDR_DATA   the game's files/ folder (your own copy)   -> real-data tests
#   JDR_APK    your own APK                                -> real-binary patch test
#   JDR_BUILD_TOOLS  folder with zipalign + apksigner      -> needed for the patch test
#   JDR_KEYS   your local keys.json                        -> patch test embeds the server
#   ANDROID_NDK  Android NDK folder                        -> builds libjdr.so (embedded server)
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n== %s\n' "$*"; }

step "format check"
cargo fmt --all -- --check

step "build (host)"
cargo build --workspace --release

step "shared code builds for Android (aarch64-linux-android)"
cargo check -p jdr-crypto -p jdr-core -p jdr-services -p jdr-embedded --target aarch64-linux-android

if [[ -n "${ANDROID_NDK:-}" ]]; then
  step "embedded server: libjdr.so for aarch64-linux-android"
  case "$(uname -s)" in
    Linux) ndk_host=linux-x86_64 ;;
    Darwin) ndk_host=darwin-x86_64 ;;  # the NDK ships one universal macOS toolchain under this name
    *) echo "unsupported host $(uname -s); build libjdr.so on Linux/macOS (or WSL)"; exit 1 ;;
  esac
  export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ANDROID_NDK/toolchains/llvm/prebuilt/$ndk_host/bin/aarch64-linux-android21-clang"
  libjdr_fresh=1
  cargo build --release -p jdr-embedded --target aarch64-linux-android
  ls -l target/aarch64-linux-android/release/libjdr.so
else
  step "embedded server build skipped (set ANDROID_NDK)"
fi

step "clippy"
cargo clippy --workspace --all-targets -- -D warnings

step "unit + integration tests (real-data suites skip without env vars)"
cargo test --workspace --release

if [[ -n "${JDR_APK:-}" && -n "${JDR_BUILD_TOOLS:-}" ]]; then
  step "real-binary: patch your APK"
  mkdir -p deliver
  if [[ -n "${JDR_KEYS:-}" && -n "${libjdr_fresh:-}" ]]; then
    embed=(--keys "$JDR_KEYS" --server-lib target/aarch64-linux-android/release/libjdr.so)
  else
    echo "(needs JDR_KEYS and ANDROID_NDK in the same run to embed a freshly built libjdr.so; building the dev-loop APK without it)"
    embed=(--disable embed-server)
  fi
  JDR_KEYSTORE="${JDR_KEYSTORE:-$HOME/.config/JDR/jdr-signing.p12}" \
    ./target/release/jdr-patch patch --apk "$JDR_APK" --out deliver/jdr-test.apk --force "${embed[@]}"
else
  step "real-binary patch test skipped (set JDR_APK and JDR_BUILD_TOOLS)"
fi

step "done"
