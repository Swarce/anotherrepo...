# Dev loop

The URL-patched game on your phone talks to `jdr-server` on your PC over `adb reverse`.
Everything below runs on **your** machine with **your** copy of the game.

## Tools

- Rust (rustup; the repo pins 1.97.0), a JDK (for `keytool`/`apksigner`), Android build-tools
  (`zipalign`, `apksigner`; set `JDR_BUILD_TOOLS` to that folder), and `adb`.

```bash
cargo build --release
```

## 0. Back up your copy (before anything else)

Your downloaded game data is irreplaceable (the CDN is gone). Copy it with `adb` (file managers and
MTP cannot see `Android/data` on Android 11+, and partial copies are common):

```bash
mkdir -p local/backup
```

```bash
adb pull /sdcard/Android/data/com.bandainamcoent.jojodr/files local/backup/
```

Get the installed APK too. If `pm path` prints more than one line, the game was installed as split
APKs and the patcher cannot use it yet (it needs the single universal 3.12.0 APK).

```bash
adb shell pm path com.bandainamcoent.jojodr
```

```bash
adb pull /data/app/<path printed above>/base.apk local/game-3.12.0.apk
```

## 1. Keys from your copy

```bash
./target/release/jdr-tool keys --data local/backup/files --apk local/game-3.12.0.apk --soundkey <hex> --out local/keys.json
```

It must print **"copy is complete: all N files recorded by the game are present"**. If it warns
about missing files, pull again; do not continue to step 3.

The sound key is the 16-hex-digit entry for this game in vgmstream's public `hca_keys.h` list
(unverified; audio may be silent if wrong). `local/` is git-ignored.

## 2. Patch your APK

First time only, create your signing key (stored outside the repo, reused for every build):

```bash
./target/release/jdr-patch init-key
```

For the PC dev loop, build without the embedded server (the game then talks to `jdr-server` on
the PC over `adb reverse`). For the in-game server, see docs/PLAYER-SETUP.md.

```bash
JDR_BUILD_TOOLS=/path/to/build-tools/36.0.0 ./target/release/jdr-patch patch --apk local/game-3.12.0.apk --out deliver/jojodr-dev.apk --disable embed-server
```

Read `deliver/jojodr-dev.report.txt`: every patch `applied`, every post-check `ok`. Keep
`~/.config/JDR/` safe: losing the key means a future build cannot install over this one.

## 3. Install and restore your data

The original APK is signed by the publisher, so it must be uninstalled once before the patched one
can be installed. **This deletes the game's data folder on the phone.** Only do it after step 1
reported a complete copy.

```bash
adb uninstall com.bandainamcoent.jojodr
```

```bash
adb install deliver/jojodr-dev.apk
```

Launch the game once and quit, so Android creates its data folder, then push your files back
(about 1.9 GB):

```bash
adb push local/backup/files/. /sdcard/Android/data/com.bandainamcoent.jojodr/files/
```

Later builds install **over** this one with `adb install -r` and keep everything; never uninstall
again.

## 4. Run the server and connect

```bash
./target/release/jdr-server --keys local/keys.json --files local/backup/files --trace
```

```bash
adb reverse tcp:18080 tcp:18080
```

Start the game. The server prints one line per request; unknown endpoints are marked
`UNKNOWN ROUTE`.

When you switch back to an APK with the in-game server, remove the reverse first, or adb keeps
port 18080 on the phone and the in-game server cannot start:

```bash
adb reverse --remove-all
```

## Logs to send

- `local/server/logs/server-*.log` (newest)
- the device log (written under `local/`, which is git-ignored):

```bash
adb logcat -d > local/logcat.txt
```

Key fields and device identifiers are redacted from server logs; still skim a log before posting
it publicly.

## Options

`--tutorial play` (default `skip`), `--clock-offset SECS` (simulated time), `--port N` (only with an
APK patched for the same port: `jdr-patch --port N`). The server binds exactly one port and refuses
to start if it is taken (usually an old `jdr-server` still running).

The server serves files from the folder you give it and never writes to it.
