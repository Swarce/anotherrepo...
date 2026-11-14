# Player setup

> Status: **Build 02, for testing.** The server runs inside the game (no PC needed after install).
> The game is still expected to stop at the loading screen until the master-data rebuild (M1.5)
> lands. Nothing here is confirmed on a device yet.

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
   JDR_BUILD_TOOLS=/path/to/build-tools ./target/release/jdr-patch patch --apk local/game-3.12.0.apk --out deliver/jojodr-b02.apk --keys local/keys.json --server-lib target/aarch64-linux-android/release/libjdr.so
   ```

   The report must show `embed-server` applied (with `JDR_EMBEDDED_SERVER_b002`) and every
   post-check `ok`.
5. **Install.** First time only, the original game must go (back up first, step 1):

   ```bash
   adb uninstall com.bandainamcoent.jojodr
   ```

   ```bash
   adb install deliver/jojodr-b02.apk
   ```

   Later builds: `adb install -r deliver/jojodr-bNN.apk` (keeps everything; never uninstall).
6. **Restore your files:** launch the game once, quit, then

   ```bash
   adb push local/backup/files/. /sdcard/Android/data/com.bandainamcoent.jojodr/files/
   ```

7. **Test:** clear the device log, start the game, watch the server lines:

   ```bash
   adb logcat -c
   ```

   ```bash
   adb logcat -s JDR Unity
   ```

## What success looks like (Build 02)

1. `[boot] jdr embedded build 2 …`, `keys loaded`, `listening on 127.0.0.1:18080`.
2. `[api] POST /common/get_version 200` when the title screen appears.
3. Tap: terms page with an Accept button → `[account] created …` → `/api/user/login` again.
4. `getkey`, `get_system_config`, `get_tutorial_step`, `getfiledata …`, then the expected stall on
   the loading screen (`[cdn] … 404` for the bundles M1.5 will build).

Send `adb logcat -d > local/logcat.txt` and the server log:

```bash
adb pull /sdcard/Android/data/com.bandainamcoent.jojodr/jdr/logs local/device-logs
```

## Troubleshooting

- **The game closes immediately at launch (no `JDR` lines):** the in-game server library did not
  load. Patch the same APK again **with the same key** and `--disable embed-server`, then
  `adb install -r` it: that build talks to `jdr-server` on your PC (docs/DEV-LOOP.md) and keeps your
  data. Send the logcat from the failed launch.
- **`FATAL` in the `JDR` lines:** keys missing (re-patch with `--keys`) or the port is taken
  (another app holds 127.0.0.1:18080; the server keeps retrying and logs every minute).
- **Patcher says "unverified build"**: your APK is not the verified 3.12.0 build.
- **Patcher says "needs the server library"**: build `libjdr.so` (step 2) or pass `--server-lib`.
- **Signature error, first install:** uninstall the original once (after the backup).
- **Signature error, updating a patched build: do not uninstall** (it deletes your save). The new APK
  was signed with a different key; patch again with your original key (`JDR_KEYSTORE`). The
  patcher normally refuses this case (key pin).
