# 29 — SDKs an offline game does not use (Build 32)

Read in the 3.12.0 APK: H = read in the disassembly, the manifest or the files, M = inferred.
Code: `patcher/src/patches.rs` (`Debloat`, `DEBLOAT_ELEMENTS`, `DEBLOAT_LIBS`), fingerprints in
`patcher/src/known.rs` (`sdk_stubs`, `title_play_games_button`), the Options edit in
`server/content/l10n/ui_edits.json`.

## 1. What the APK carries (H)

- **Native libraries (arm64).** `libFirebaseCppApp-5.6.1.so` (3.8 MB), `libFirebaseCppMessaging.so`
  (0.1 MB), `libSmartBeatNdk.so` (0.55 MB, crash reporting), `libgpg.so` (3.7 MB, Play Games).
  None is in any library's `DT_NEEDED`. libil2cpp names them only as `DllImport` targets, which
  the runtime loads on first call.
- **Manifest.** Firebase (init provider, messaging and instance-id services and receivers,
  component discovery), app measurement (services and receivers), Google Mobile Ads
  (`AdActivity`), Google Sign-In, `GoogleApiActivity`, Play Games (`NativeBridgeActivity`,
  `APP_ID`), Adjust's referrer receiver, Growth Push's activity and services, and the push
  (`c2dm RECEIVE`, `C2D_MESSAGE`) and `READ_PHONE_STATE` permissions.
- **Launcher.** `com.google.firebase.MessagingUnityPlayerActivity` extends `UnityPlayerActivity`.
  Its `onCreate` only calls the parent. Its `onNewIntent` forwards an intent to
  `MessageForwardingService` only when it carries a push message id; with the service gone,
  `startService` returns null. It stays the launcher.

## 2. Who calls the SDKs (H, a call graph of every `bl`/`b` in libil2cpp)

- **Firebase:** nothing in the game's code.
- **SmartBeat:** only its own `SmartBeatBehaviour` (Unity messages `Awake`, `OnApplicationPause`,
  `Update`; `InitAndroid` from `Awake`).
- **Adjust:** `Jjs.AdjustLauncher.Start`/`TrackEvent`/`TrackRevenueEvent` → `Adjust.start`,
  `Adjust.trackEvent`.
- **Play Games:** `ScSystem.ScSocial.AchievementManager`. `Init` (0x10612B0) builds the client
  configuration and calls `PlayGamesPlatform.Activate`. Every other method goes through
  `UnityEngine.Social`. Callers: `Jjs.AchievementUtil` (title sign-in button, automatic sign-in
  at start, Options Achievements, unlocks after the trophy list).
- **Growth Push:** already never started (`no-growthbeat`, Build 07).

## 3. What `debloat` does (ours)

- **Stubs** (32-byte fingerprint each; each address belongs to one method, no folded code):
  - SmartBeat: `Awake`, `OnApplicationPause`, `Update` → `ret`.
  - Adjust: `start`, `trackEvent`, `OnApplicationPause` → `ret`.
  - Play Games: `AchievementManager.Init` → `ret`. `PlayGamesPlatform` is never activated, so
    `UnityEngine.Social` keeps Unity's own local platform and libgpg is never loaded.
    `AchievementUtil.get_NeedAutoSignIn` → `mov w0, #0; ret`: no automatic sign-in.
  - Title: in `TitleUIController.SetupGooglePlayGamesLoginButton`, `SetActive(!Authenticated)`
    becomes `SetActive(false)` (`mvn w8, w20` → `mov w8, #0` at 0xB90DA4).
- **Manifest:** the 27 elements of `DEBLOAT_ELEMENTS` (with their children) leave. Each must be
  there exactly once before; afterwards none is, and the launcher is still there. Kept: the
  launcher, Unity's meta-data, the game's local notifications (`jp.co.spchun.localpushlib`), and
  in-app billing (prime31; the server answers purchases, nothing is paid).
- **Files:** the four libraries leave the APK.
- **Check:** `patcher/tests/real_apk_debloat.rs` on the player's APK. libil2cpp differs only at
  the stub sites, the listed elements are gone, the game's own are kept in order (85 → 40
  elements, children included), and exactly the four libraries are removed.

## 4. Options menu (ours)

- `scene_option` file 9 (`BuildPlayer-OptionUI`): `OptionButtonGroup` holds `Btn_Option1..15`,
  placed by hand in a 3-column grid (no layout group). `Btn_Option7` is Achievements
  (`option_category_06`, `OptionMenu.Achievement = 6`).
- The server hides `Btn_Option7` and gives `Btn_Option8..15` the anchors and position of the
  button before each, so the grid has no hole; the last place stays empty.
- `OptionUIController.Start` only toggles the platform icon on that button (`img_android`); it
  never activates the button again (H).
- Checked by `real_l10n` (9 edits applied) and `tools/proofs/l10n_check.py` (Achievements
  inactive, the other 14 buttons in 14 distinct places).

## 5. Open (M)

- Java code of the removed SDKs stays in `classes.dex`; nothing registers or starts it.
- Not removed: the `com.growthpush.*` and Firebase analytics meta-data flags (inert).
