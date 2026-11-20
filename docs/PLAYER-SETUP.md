# Player setup

> Status: **for testing.** Confirmed on a Galaxy S22 / Android 16 (device logs, Builds 02–04): the
> in-game server starts, the title screen, terms page, account creation and login work, and saves
> survive an update. The game then needs its downloaded files and stops at the loading screen until
> the master-data rebuild (M1.5) lands.

## Read first

- **Saves live in the app's private storage.** Uninstalling the game or "Clear storage" deletes
  them, and before M3 (export/import) you cannot copy them out except from the backup copy the
  server writes to `/sdcard/Android/data/com.bandainamcoent.jojodr/jdr/saves/` (also deleted by an
  uninstall: pull it to your PC first).
- **Your patched APK contains your asset keys** (that is how the in-game server reads them). Any
  app on the phone can read an installed APK, and phone/cloud backups of apps carry them too. Don't
  share the APK or post it anywhere.
- **If the in-game server cannot load, the game cannot start.** Recovery is below; it never needs
  an uninstall.
- Keep `local/backup/files` on your PC until the game is fully playable.
- **The game's downloaded files must be on the phone and complete** (`files/`, about 13,500 files
  including `vcs.bytes`). Uninstalling the original game deletes them, and the original game also
  wiped them once on its first start; step 6 puts them back and the `keep-files` patch prevents
  that wipe. The in-game server builds the asset list from them and logs what it finds at start-up.

## What you need

- Your own APK, version **3.12.0** (the patcher refuses other versions), and your own downloaded
  game data (`Android/data/com.bandainamcoent.jojodr/files`).
- A 64-bit Android phone with USB debugging, and a PC with `adb`, a JDK and Android build-tools.
- Rust and the Android NDK to build from source (Linux or macOS; Windows via WSL).

## Steps

1. **Back up your data and APK, recover the keys:** docs/DEV-LOOP.md steps 0 and 1.
   `jdr-tool keys` must print "copy is complete".
2. **Build the tools and the in-game server:**

   ```bash
   cargo build --release -p jdr-patch -p jdr-tool
   ```

   ```bash
   CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$ANDROID_NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang cargo build --release -p jdr-embedded --target aarch64-linux-android
   ```

   (On macOS use `darwin-x86_64` in that path.)
3. **Create your signing key** (first time only; keep `~/.config/JDR/` safe):

   ```bash
   ./target/release/jdr-patch init-key
   ```

4. **Patch:**

   ```bash
   JDR_BUILD_TOOLS=/path/to/build-tools ./target/release/jdr-patch patch --apk local/game-3.12.0.apk --out deliver/jojodr-b05.apk --keys local/keys.json --server-lib target/aarch64-linux-android/release/libjdr.so
   ```

   The report must show `embed-server` applied (with the current `JDR_EMBEDDED_SERVER_bNNN`) and
   every post-check `ok`.

   **Optional: redrawn UI art.** If you have an art pack (redrawn images made from your own export
   with `tools/ui-assets/export_ui.py`, as `replacements/<bundle>/sprites/<sprite>.png` in a zip),
   add it, together with your game files folder (the patcher reads the title-screen layouts from it):

   ```bash
   JDR_BUILD_TOOLS=/path/to/build-tools ./target/release/jdr-patch patch --apk local/game-3.12.0.apk --out deliver/jojodr.apk --keys local/keys.json --server-lib target/aarch64-linux-android/release/libjdr.so --art local/my-art-pack.zip --data local/files
   ```

   Title and boot screens are repainted in the APK; menu art is stored in the APK and applied by
   the server on the phone (one download, together with the English text files). Keep your art
   pack private: it is made from the game's own images.

   **Optional: portraits for the statues that have none.** 55 roster statues have no thumbnail in
   the game files. `tools/content/render_portraits.py` draws them from your own files (each
   statue's model in its menu pose, with the frame, stars and attribute badge of the real ones;
   docs/research/28). It needs your decrypted bundles, the `st_*` thumbnails decrypted with
   `jdr-tool decrypt`, Node with Playwright, and three.js (`npm pack three@0.160.0`):

   ```bash
   python3 -I tools/content/render_portraits.py --dec local/dec --thumbs local/thumbs --statues server/content/statues.json --three local/three/package/build/three.min.js --out local/portraits
   ```

   Then add `--art local/portraits/portraits.zip` to the patch command, after your art pack
   (`--art` may be given more than once). Keep it private as well.
5. **Install.** First time only, the original game must go (back up first, step 1):

   ```bash
   adb uninstall com.bandainamcoent.jojodr
   ```

   ```bash
   adb install deliver/jojodr-b05.apk
   ```

   Later builds: `adb install -r deliver/jojodr-bNN.apk` (keeps everything; never uninstall).
6. **Restore your files.** Do **not** push them into `Android/data/…/files/` directly: on Android 11+
   files pushed there belong to the shell user and the game cannot read them (`Permission
   denied`). Push them to a staging folder instead; the in-game server copies them in as the app
   (about 1.9 GB, expected to take under a minute; expected to work, to be confirmed on a device):

   ```bash
   adb push local/backup/files /data/local/tmp/jdr-import/files
   ```

   ```bash
   adb shell chmod -R a+rX /data/local/tmp/jdr-import
   ```

   Start the game and watch `adb logcat -s JDR` until it prints `[import] import complete`, then
   close and restart the game. Afterwards you may free the space:

   ```bash
   adb shell rm -rf /data/local/tmp/jdr-import
   ```

   The `keep-files` patch stops the game from ever wiping this folder again (it used to do that
   once on a fresh install).

7. **Test:** clear the device log, start the game, watch the server lines:

   ```bash
   adb logcat -c
   ```

   ```bash
   adb logcat -s JDR Unity
   ```

## What success looks like

1. `[boot] jdr embedded build N …`, `keys loaded`, `listening on 127.0.0.1:18080`.
2. `[assets] game files: 13480 file(s), all 13479 recorded files present` (your numbers may
   differ slightly), then `[assets] versionfile built: … entries`.
3. `[api] POST /common/get_version 200` when the title screen appears.
4. First start only: terms page → `[account] created …` → `/api/user/login` again. Later starts:
   one login with your saved account.
5. `getkey`, then `getfiledata` for the manifest and
   `[cdn] GET /cdn/….bytes 200 … (generated versionfile)`, then the expected stop on the loading
   screen at the master-data bundle (until M1.5).

Updating from an earlier patched build: just `adb install -r` the new APK (same key); your account
and files stay.

Send `adb logcat -d > local/logcat.txt` and the server log:

```bash
adb pull /sdcard/Android/data/com.bandainamcoent.jojodr/jdr/logs local/device-logs
```

Before posting logs publicly, replace the long hex file names (`/cdn/<hex>.bytes`): they are derived
from your asset key.

## Troubleshooting

- **The game closes immediately at launch (no `JDR` lines):** the in-game server library did not
  load. Patch the same APK again **with the same key** and `--disable embed-server`, then
  `adb install -r` it: that build talks to `jdr-server` on your PC (docs/DEV-LOOP.md) and keeps your
  data. Send the logcat from the failed launch.
- **`vcs.bytes MISSING` or `… recorded files MISSING`:** your downloaded files are not (all) on the
  phone. Stage them again (step 6) and restart the game.
- **`Permission denied` for `vcs.bytes`:** the files were pushed straight into `Android/data`; stage
  them in `/data/local/tmp/jdr-import/files` instead (step 6); the import replaces the unreadable
  copies.
- **A new crash inside `libunity.so`:** the `unity-heap` engine fix can be switched off. Patch the same
  APK again with the same key and `--disable unity-heap`, then `adb install -r`, and send the logcat
  (without it the game is expected to crash right after login on Android 11+).
- **Something stopped working since Build 32** (a crash at start, a screen that never opens): the
  SDK removal can be switched off. Patch the same APK again with the same key and
  `--disable debloat`, then `adb install -r`, and send the logcat.
- **`FATAL` in the `JDR` lines:** keys missing (re-patch with `--keys`) or the port is taken.
  The usual cause is a leftover `adb reverse` from the PC dev loop: run `adb reverse --remove-all`
  and restart the game. Otherwise another app holds 127.0.0.1:18080 (the server keeps retrying and
  logs every minute).
- **Patcher says "unverified build"**: your APK is not the verified 3.12.0 build.
- **Patcher says "needs the server library"**: build `libjdr.so` (step 2) or pass `--server-lib`.
- **Signature error, first install:** uninstall the original once (after the backup).
- **Signature error, updating a patched build: do not uninstall** (it deletes your save). The new APK
  was signed with a different key; patch again with your original key (`JDR_KEYSTORE`). The
  patcher normally refuses this case (key pin).
