#!/usr/bin/env bash
# Build everything and run every test suite the environment allows.
#   JDR_DATA   the game's files/ folder (your own copy)   -> real-data tests
#   JDR_APK    your own APK                                -> real-binary patch test
#   JDR_BUILD_TOOLS  folder with zipalign + apksigner      -> needed for the patch test
#   JDR_KEYS   your local keys.json                        -> patch test embeds the server
#   ANDROID_NDK  Android NDK folder                        -> builds libjdr.so (embedded server)
#   JDR_ART_ZIP  your redrawn UI art pack (zip)            -> patch test stores/applies it, art checks
set -euo pipefail
cd "$(dirname "$0")/.."
# Tests run from each crate's folder: pass file paths on as absolute paths.
for v in JDR_KEYS JDR_DATA JDR_APK JDR_ART_ZIP; do
  if [[ -n "${!v:-}" ]]; then export "$v=$(realpath "${!v}")"; fi
done

step() { printf '\n== %s\n' "$*"; }
skipped=()
skip() { step "SKIPPED: $*"; skipped+=("$*"); }

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
  skip "embedded server build (set ANDROID_NDK)"
fi

if [[ -n "${JDR_APK:-}" ]]; then
  if python3 -c "import unicorn, elftools" 2>/dev/null; then
    step "proofs on your binaries: unity-heap equivalence, keep-files callers"
    JDR_APK="$JDR_APK" python3 -I tools/proofs/unity_heap_equiv.py   # failure stops the build (set -e)
    JDR_APK="$JDR_APK" python3 -I tools/proofs/keep_files_check.py
  else
    echo "JDR_APK is set but python3 lacks unicorn/pyelftools: the unity-heap proof is required before patching" >&2
    exit 1
  fi
else
  skip "unity-heap equivalence proof (set JDR_APK)"
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
  if [[ -n "${JDR_ART_ZIP:-}" && -n "${JDR_DATA:-}" ]]; then
    embed+=(--art "$JDR_ART_ZIP" --data "$JDR_DATA")
  fi
  JDR_KEYSTORE="${JDR_KEYSTORE:-$HOME/.config/JDR/jdr-signing.p12}" \
    ./target/release/jdr-patch patch --apk "$JDR_APK" --out deliver/jdr-test.apk --force "${embed[@]}"
else
  skip "real-binary patch test (set JDR_APK and JDR_BUILD_TOOLS)"
fi

if [[ -n "${JDR_DATA:-}" && -n "${JDR_KEYS:-}" ]]; then
  step "real data: generate versionfile + in_memory_master_data from your files and check them with UnityPy"
  out="$(pwd)/deliver/versionfile-check.bytes"
  md="$(pwd)/deliver/masterdata-check.bytes"
  mkdir -p deliver
  l10n_out="$(pwd)/deliver/l10n-check"
  JDR_VERSIONFILE_OUT="$out" JDR_MASTERDATA_OUT="$md" JDR_KEYS="$(realpath "$JDR_KEYS")" cargo test -q --release -p jdr-services --test real_versionfile
  JDR_KEYS="$(realpath "$JDR_KEYS")" cargo test -q --release -p jdr-services --test real_unity_roundtrip
  JDR_L10N_OUT="$l10n_out" JDR_KEYS="$(realpath "$JDR_KEYS")" cargo test -q --release -p jdr-services --test real_l10n
  if python3 -c "import UnityPy" 2>/dev/null; then
    python3 -I tools/proofs/versionfile_check.py "$out"
    python3 -I tools/proofs/masterdata_check.py "$md" server/content/masterdata
    python3 -I tools/proofs/l10n_check.py "$l10n_out/l10n/gen"
    if [[ -n "${JDR_ART_ZIP:-}" ]]; then
      art_dir="$(mktemp -d)"; art_out="$(pwd)/deliver/art-check"
      python3 -I -c 'import sys,zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])' "$JDR_ART_ZIP" "$art_dir"
      reps="$(find "$art_dir" -type d -name replacements | head -1)"
      JDR_ART="$reps" JDR_ART_OUT="$art_out" cargo test -q --release -p jdr-services --test real_art
      python3 -I tools/proofs/art_check.py "$art_out" "$reps"
      rm -rf "$art_dir" "$art_out"
    else
      skip "redrawn art checks (set JDR_ART_ZIP)"
    fi
  else
    skip "UnityPy check of the generated bundles (pip install UnityPy)"
  fi
  rm -rf "$out" "$md" "$l10n_out"
else
  skip "bundle generation from real data (set JDR_DATA and JDR_KEYS)"
fi

if ((${#skipped[@]})); then
  step "done, with ${#skipped[@]} skipped check(s):"
  printf '   - %s\n' "${skipped[@]}"
else
  step "done: every check ran"
fi
