# 06 — Native boundary: engine, libraries, scripting

The game is Unity 2018.4 IL2CPP: all game logic is C# compiled into `libil2cpp.so`, with names and
string literals in `global-metadata.dat`. Lua (SLua) only runs scenario/tutorial command scripts
stored in bundles. Audio and movies use CRI; API HTTP goes through a small Java proxy class.

Confidence: H for the engine, library list, metadata version and the Java HTTP proxy; M for the
exact split between C# and Lua (based on bindings and extracted tutorial scripts, not a full scan).

## 1. Engine and binaries (H)

- Unity **2018.4.1f1**, scripting backend IL2CPP, metadata version 24.x.
- ABIs in the APK: `arm64-v8a` and `armeabi-v7a` (we keep arm64 only, see 01).
- In `libil2cpp.so` the compiled game code sits in an ELF section named `il2cpp`. Addresses in our
  notes are arm64 virtual addresses as printed by our dumper (LibCpp2IL based).
- Launcher activity: `com.google.firebase.MessagingUnityPlayerActivity`. minSdk 16, targetSdk 28.

`lib/arm64-v8a/`:

| Library | Role |
|---|---|
| `libmain.so` | Unity bootstrap; loads `libunity.so`. Planned host for our embedded server (M2: a `DT_NEEDED` entry to `libjdrserver.so`). |
| `libunity.so` | Unity engine runtime |
| `libil2cpp.so` | all game C# (Assembly-CSharp, ScSystem framework, SDK wrappers), compiled |
| `libslua.so` | Lua runtime for SLua |
| `libcri_ware_unity.so` | CRI ADX2 (audio) and Sofdec (movie) |
| `libgpg.so` | Google Play Games C++ SDK |
| `libFirebaseCppApp-*.so`, `libFirebaseCppMessaging.so` | Firebase C++ (FCM) |
| `libSmartBeatNdk.so` | SmartBeat crash reporting |

We patch **no code**: only one metadata string literal and one manifest attribute (01).

## 2. Lua / SLua (H bindings, M scope)

- SLua is present: `libslua.so` and about 60 `SLua.*` types, plus generated binding classes
  (e.g. `Lua_JJsLua : SLua.LuaObject` for `JJsLua`).
- The tutorial and scenario scripts are Lua command text in a MonoBehaviour `command` field inside
  bundles (e.g. the `main_tutorial_NN` bundles). They call `jjs:` functions bound to `JJsLua`:
  `goto_bizarre` @0x204F2BC, `goto_gasha` @0x204F554, `name_input` @0x204F740, `goto_init`
  @0x204F0B8, `goto_home` @0x204F1CC, plus presentation commands (text, voice, characters, camera).
- No core game logic (battle, economy, networking) was found in Lua; it is all C#. The scripts drive
  UI flow and call into C#, which then makes the API calls (03 §4).
- Consequence: tutorial and story script **text** lives in `*_script_text_jp` bundles (see 08 for
  how English text replaces it); script logic needs no change.

## 3. CRI Ware (H)

- Audio: ADX2 ACF/ACB/AWB inside CPK archives (`common`, BGM CPKs, one CPK per statue, in-app CPK).
  HCA cipher type 56 throughout.
- Movies: Sofdec (CRID) file in the APK assets.
- Decryption key setup: `SoundPlayerManager.SetDecryption(keyStr, authFile)` @0x1062EE8 →
  `criWareUnity_SetDecryptionKey(UInt64, authFile, atom=true, mana=true)`. Two keys exist: a
  hard-coded in-app key (code literal) and the server-supplied `soundkey`. Each is paired with a
  256-byte auth file in the APK assets. Values are never written here; see 08 §8.

## 4. HTTP via JNI (H)

- `ScSystem.ScNet.HttpAndroid` (managed) forwards each request to the Java class
  `jjs.spchun.co.jp.httpandroid.HttpProxy` (with `HttpRequestData`, `BufferData`), which uses
  `java.net.HttpURLConnection` (`CreateRequest` sets method, timeouts, keep-alive, no redirects,
  no caches). Created in `HttpManager::Init` @0x20386DC.
- Consequence: API traffic obeys the Android network-security policy (cleartext, trust store),
  while Mono traffic (asset `WebClient`) does not. See 01 §3–4.

## 5. Other SDKs (H presence, M behaviour)

Firebase (FCM, Analytics), Play Games, Prime31 Google IAB, AdMob, Adjust, Growthbeat/GrowthPush,
SmartBeat, spchun local push, UniWebView. Boot-path behaviour (B-boot §7):

- Play Games auto sign-in **waits for its callback** (result ignored). A missing callback would hang
  on the connect UI (M; not yet seen).
- Google advertising id: login waits for it (H); behaviour without Play services not traced (L).
- Device id (`HardwareConst.IsGetBackupID` @0x2034F84): if no ICCID/IMSI/MAC/Android id can be read,
  the client quits (H).
- Others are fire-and-forget. Debloat is milestone M6.

## Open points

- Whether GPG and the advertising-id call back on devices without Play services.
- A full scan of Lua scripts for API-relevant calls beyond the tutorial set.
