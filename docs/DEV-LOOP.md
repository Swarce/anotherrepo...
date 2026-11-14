# Dev loop

The URL-patched game on your phone talks to `jdr-server` on your PC over `adb reverse`.
Everything below runs on **your** machine with **your** copy of the game.

## Tools

- Rust (rustup; the repo pins 1.97.0), a JDK (for `keytool`/`apksigner`), Android build-tools
  (`zipalign`, `apksigner`; set `JDR_BUILD_TOOLS` to that folder), and `adb`.

```bash
cargo build --release
```

## 1. Keys from your copy

`--data` is the game's `files/` folder from your phone
(`/sdcard/Android/data/com.bandainamcoent.jojodr/files`, copied to the PC).

```bash
./target/release/jdr-tool keys --data /path/to/files --apk /path/to/game.apk --soundkey <hex> --out local/keys.json
```

The sound key is the 16-hex-digit entry for this game in vgmstream's public `hca_keys.h` list
(unverified; audio may be silent if wrong). `local/` is git-ignored.

## 2. Patch your APK

```bash
JDR_BUILD_TOOLS=/path/to/build-tools/36.0.0 ./target/release/jdr-patch patch --apk /path/to/game.apk --out deliver/jojodr-b01.apk
```

The first run creates your signing key at `~/.config/JDR/jdr-signing.p12`. Keep it: every later
build must use the same key to install over the previous one. Read `deliver/jojodr-b01.report.txt`.

## 3. Install and restore your data

The patched APK is signed with your key, so the original must be uninstalled first, **which deletes
the game's data folder on the phone**. Keep your backup of `files/`.

```bash
adb uninstall com.bandainamcoent.jojodr
```

```bash
adb install deliver/jojodr-b01.apk
```

Launch the game once and quit, so Android creates its data folder, then push your files back
(about 1.9 GB):

```bash
adb push /path/to/files/. /sdcard/Android/data/com.bandainamcoent.jojodr/files/
```

## 4. Run the server and connect

```bash
./target/release/jdr-server --keys local/keys.json --files /path/to/files --trace
```

```bash
adb reverse tcp:18080 tcp:18080
```

Start the game. The server prints one line per request; unknown endpoints are marked
`UNKNOWN ROUTE`.

## Logs to send

- `local/server/logs/server-*.log` (newest)
- the device log:

```bash
adb logcat -d > logcat.txt
```

## Options

`--tutorial play` (default `skip`), `--clock-offset SECS` (simulated time), `--port N`.
