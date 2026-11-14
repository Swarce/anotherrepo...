# Player setup

> Status: **not ready for players yet.** Build 01 is the developer dev loop (docs/DEV-LOOP.md):
> the server runs on a PC. From M2 the server is inside the app and this page becomes the
> step-by-step guide.

## What you will need

- Your own copy of the game APK, version **3.12.0** (the patcher refuses other versions).
- Your own downloaded game data: the folder
  `Android/data/com.bandainamcoent.jojodr/files` from the phone where you played (back it up now).
- A 64-bit (arm64) Android phone.

## Where the logs are

Build 01: on the PC running `jdr-server`, in `local/server/logs/`. From M2: a "send logs" button on
the settings page.

## Troubleshooting (Build 01)

- **Patcher says "unverified build"**: your APK is not the verified 3.12.0 build.
- **Install fails with a signature error, first install** (original game still installed): back
  up `files/` with `adb pull` (docs/DEV-LOOP.md step 0), then uninstall the original once.
- **Install fails with a signature error, update of a patched build**: **do not uninstall** — that
  deletes your save. The new APK was signed with a different key; patch again with your original
  key (`JDR_KEYSTORE`). The patcher normally refuses this case (key pin).
- **The game shows a connection error at the title**: check that `jdr-server` is running and that
  `adb reverse tcp:18080 tcp:18080` was run after connecting the phone.
