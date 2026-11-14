# Player setup

> Status: **Build 02, for testing.** The server now runs inside the game (no PC needed after
> install). The game is still expected to stop at the loading screen until the master-data rebuild
> (M1.5) lands. Nothing here is confirmed on a device yet.

## What you need

- Your own copy of the game APK, version **3.12.0** (the patcher refuses other versions).
- Your own downloaded game data: `Android/data/com.bandainamcoent.jojodr/files` from the phone where
  you played.
- A 64-bit (arm64) Android phone, a PC with `adb`, a JDK, Android build-tools, and either Rust + the
  Android NDK (to build from source) or the prebuilt `jdr-patch`/`jdr-tool`/`libjdr.so`.

## Steps

1. **Back up and get your APK:** docs/DEV-LOOP.md steps 0 and 1 (`adb pull`, then
   `jdr-tool keys` must report a complete copy).
2. **Build the embedded server** (from source):

   ```bash
   ANDROID_NDK=/path/to/android-ndk ./scripts/build.sh
   ```

   This produces `target/aarch64-linux-android/release/libjdr.so`.
3. **Patch** (first time: `jdr-patch init-key`):

   ```bash
   JDR_BUILD_TOOLS=/path/to/build-tools ./target/release/jdr-patch patch --apk local/game-3.12.0.apk --out deliver/jojodr-b02.apk --keys local/keys.json
   ```

   The report must list `embed-server` as applied and every post-check `ok`. Your keys go into
   your patched APK (that is how the in-game server reads them), so **don't share that APK**.
4. **Install and restore your data:** docs/DEV-LOOP.md step 3. No `jdr-server`, no `adb reverse`.
5. **Play.** The phone needs no network.

## Where the logs are

- Server log, copied every few seconds once the game has started:
  `/sdcard/Android/data/com.bandainamcoent.jojodr/jdr/logs/server-*.log`

  ```bash
  adb pull /sdcard/Android/data/com.bandainamcoent.jojodr/jdr/logs local/device-logs
  ```

- Live, including the server's lines (tag `JDR`):

  ```bash
  adb logcat -s JDR Unity
  ```

- The game's own device log:

  ```bash
  adb logcat -d > local/logcat.txt
  ```

Saves live in the app's private storage (`/data/user/0/com.bandainamcoent.jojodr/jdr/`);
export/import comes with M3.

## Troubleshooting

- **Patcher says "unverified build"**: your APK is not the verified 3.12.0 build.
- **Patcher says "needs the server library"**: build `libjdr.so` (step 2) or pass `--server-lib`.
- **Install fails with a signature error, first install** (original game still installed): back
  up `files/` with `adb pull` (docs/DEV-LOOP.md step 0), then uninstall the original once.
- **Install fails with a signature error, update of a patched build**: **do not uninstall**, that
  deletes your save. The new APK was signed with a different key; patch again with your original
  key (`JDR_KEYSTORE`). The patcher normally refuses this case (key pin).
- **Connection error at the title**: check `adb logcat -s JDR` for `listening on 127.0.0.1:18080`
  or a `FATAL` line.
