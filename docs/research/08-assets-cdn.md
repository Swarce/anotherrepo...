# 08 — Assets and CDN

Downloaded assets sit flat in the app's external files folder under XOR-obfuscated names, most of
them AES-encrypted ("JJEC") Unity bundles. The client asks `/client/getfiledata` for one URL per
file and checks each download against a server manifest bundle (`versionfile`) that was never
stored on the device, so we must build it. Keys come back from the player's own files.

Confidence: H for paths, name scheme, container formats, URL flow, up-to-date rule, `datalVer` rule
and language constant (read in code, formats verified on all player files); M for the key-recovery
method's generality and the versionfile field values; L/unverified for the sound key.

Keys are named by role only: ASSET_KEY / ASSET_IV (`getkey.assetkey` / `assetvector`), SOUND_KEY
(`getkey.soundkey`), INAPP_SOUND_KEY (code literal). Obfuscated file names are not written here
either, because together with the plain names they would reveal the key.

## 1. Location (H)

- `ResourceManagerExec.Awake` @0x10D5874: base path = `Application.persistentDataPath + "/"`.
- No WRITE_EXTERNAL_STORAGE permission, so Unity uses `getExternalFilesDir()`:
  `/storage/emulated/0/Android/data/com.bandainamcoent.jojodr/files/` (H for the dump, M for the
  Unity rule). Every file is `<obfuscated>.bytes` in that one folder, next to `vcs.bytes`.
- Player archive: 13,480 files (~1.9 GB): 4,368 JJEC bundles, 8,967 plain UnityFS bundles, 144 CRI
  CPKs, plus `vcs.bytes`.

## 2. Obfuscated names (H)

- `ResourceManager.checkBundleName` @0x10D2B5C lowercases the name and appends `.bytes`.
- `ConvObfuscationName` @0x10D4CEC: the stem (name without the `.bytes` extension) is XORed byte by
  byte with ASSET_KEY and hex-encoded in lowercase; the directory and extension stay readable.
  Position `i` uses key byte `i` while `i` is shorter than the key, and key byte 0 for every later
  position. Sound CPKs use the same scheme (`GameSoundUtil.GetObfuscationName` @0xE49E70).
- In-APK files use a different XOR key (a `ResourceConst` literal set at boot in `InitialLoad`
  @0x20414D4). Not needed for the server.
- Implemented in `server/crypto/src/names.rs`; `jdr-tool names` lists decoded names locally.

## 3. Container formats (H, verified on all files)

**JJEC bundle:** `"JJEC" | u32 0 | u32 encSize (<= 102400) | u32 16 | AES-128-CBC(first encSize
bytes) | plaintext rest`. Decrypts to a UnityFS bundle (4,368 of 4,368). The client decrypts only if
the header is present; plain UnityFS is always accepted (`BindRequestTask.downloadComplete`,
`Cryptgraphy.checkCryptHeader` @0x10602B4).

**`vcs.bytes` (local download index):** `u32 size | u32 encSize | char[32] md5 | AES-128-CBC(first
encSize bytes) | plaintext tail`. md5 = md5 of everything after the 40-byte header. Plaintext =
`JsonUtility` JSON `{"keys":[<obfuscated file>...], "values":[<md5>...]}`: one entry per file
present. Written only by the client (`SaveVersionControlDataInterval` @0x10C59BC). If header, md5 or
decrypt fails on load, the client deletes it and treats every file as stale
(`EncryptFile.Worker` @0x10CD2F4, `CheckLoadError` @0x10C5864). Both containers use ASSET_KEY/IV.
Implemented in `server/crypto/src/container.rs`.

## 4. Key recovery (H method; values never leave the player's machine)

The official server handed out ASSET_KEY/IV at every boot and the client never stored them. They
are recovered from the player's own copy (`server/crypto/src/keyrec.rs`, `jdr-tool keys`):

1. **Key, from names:** real bundle names use only `[a-z0-9_.]`. For each key position, try all 256
   byte values and keep the one that decodes the most name bytes at that position into that
   alphabet (a frequency attack over thousands of names; no plaintext list needed). Position 0 also
   counts every position past the key length. The result is scored by the fraction of all bytes
   that decode cleanly.
2. **IV, from a known header:** with the right key and any IV, CBC decryption of a JJEC payload is
   wrong only in the first block. Every Unity 2018 bundle starts with the same 16-byte UnityFS header,
   so `IV = AES_decrypt(C0) XOR header`. Self-check: block 1 must continue the header with the
   engine version.
3. The envelope key is a plain metadata literal (#4143) read from the player's APK
   (`jdr-tool keys --apk`). The result goes to a local, git-ignored `keys.json`.

## 5. Download flow: getfiledata (H)

- `JjsManager.Init` @0xB6DC4C installs `ResourceUtility.RequestFileURLs` as the URL resolver.
- Stale files are collected into `VersionCheckData`; `ResourceUtility.<OnRequestFileURLs>`
  @0xFD4FD4 sends `/client/getfiledata {filename: ["android/data/<obfuscated>.bytes", ...]}`.
- Response `{url: [...]}` is matched **by index** (`GetURLRequestTask.GetUrlsResultCallback`
  @0x10CE080): one URL per filename, same order.
- Each URL is fetched with Mono `WebClient` GET (`DownloadRequestTask.startDownload` @0x10CC940);
  memory targets use `DownloadDataAsync`, files `DownloadFileAsync`. The body must be the stored file
  bytes, unchanged. No query string is added by the client.
- URLs expire after `UrlExpires` = 282 s and are requested again; parallel downloads and retries are
  both 3 (`HttpConst`; M on which field is which).
- `s3_url` is not used for assets.
- Build 01 maps `android/data/<name>` → `http://127.0.0.1:<port>/cdn/<name>` and serves the
  player's files directory (`client.rs`, `cdn.rs`, with Range support and path-escape checks).

## 6. Up-to-date check and manifest (H)

`AssetVersionManager.CheckNewVersion` @0x10C5D74: a file is local and current when the server md5
(from `versionfile`) equals the md5 recorded in `vcs.bytes` for its obfuscated name, the file exists,
and its length equals the server `size`. Files on disk are not re-hashed. After each download the
client checks md5(downloaded bytes) against the server md5 (`CheckSumThread.Work` @0x10CA6F0;
mismatch deletes the file) and records it in `vcs.bytes`. All 13,479 `vcs.bytes` values match the
md5 of the files on disk, so with the player's own `vcs.bytes`, the right key and a matching
manifest, **nothing is downloaded**.

**`versionfile`** (`ResourceConst.VersionAssetBundleName`; asset path
`Assets/Resources/Version/Android/versionfile.asset`):

- Fetched through getfiledata like any file, bound into memory every boot with `force=true`
  (no md5 check, JJEC or plain both accepted), never written to disk
  (`AssetVersionManager.<LoadServerVersionData>` @0x10C6D00). It is not in any player copy.
- Content: one `ScSystem.ScIO.AssetBundleVersionInfo` ScriptableObject
  `{CreateTime, TotalSize, VersionData: List<VersionInfo>}`, with
  `VersionInfo {path, ObfuscationPath, md5Sum, size, isEncrypt, isObfuscation}`.
- Values per entry (M): `path` = plain lowercase name with `.bytes`, no directory;
  `ObfuscationPath` = obfuscated name with `.bytes`; `md5Sum` = lowercase md5 of the served file;
  `size` = its length; `isEncrypt` = starts with `JJEC`; `isObfuscation` = true. A file the client
  asks for with no entry fails with `NOT_FIND_ERR`.
- **We must build it**: a Unity 2018.4.1f1 UnityFS bundle with typetrees, one MonoScript and one
  MonoBehaviour, using a downloaded bundle as the template. Design: `docs/design/master-data.md` (M1.5).

## 7. datalVer rule (H)

`datalVer` from every pass-through response goes to `AssetVersionManager.SetDataVersion`
@0x10C6C10. If it ever differs from the first value in a session, `CheckServerVersion` @0x10C5270
fails every later bundle load with ERROR_TEXT_00000007 ("data was updated", back to title).
**Send a constant** (Build 01 sends `"1"`; empty also works).

## 8. Language: hard-coded JP (H)

- `PlatformConst..cctor` @0xDC1668 sets `Language = 1` (JP) with an immediate (@0xDC16A0); no code
  writes it later, and neither device locale nor any server field changes it.
- It selects `text.jp` (UI text tables; `TextManager.SetupExternal` registers every asset in the
  bundle, so asset names inside do not matter), `<pack>_script_text_jp` bundles (story and tutorial
  script text) and the in-APK `Text/ClientMes_jp` resource (system messages).
- Plan: keep the client on JP and **replace the contents** of `text.jp` and each
  `*_script_text_jp` bundle with our English text (same bundle names and container paths), listed in
  our versionfile with new md5/size. In-APK system messages stay Japanese unless the APK is changed.
  Switching to EN instead would need a code patch plus new `.en` bundles and an APK resource.

## 9. Sound keys (H mechanism; key value unverified)

- CPKs are not file-encrypted; the HCA audio inside is (cipher type 56).
- In-app audio uses INAPP_SOUND_KEY (code literal) with an in-APK auth file (`SetupInAppSoundCue`
  @0xE4A018).
- Downloaded audio uses SOUND_KEY from `getkey.soundkey` (decimal `UInt64` string) with a second
  auth file (`SetupDefaultSoundCue` @0xE4A954, TitleLoad @0xB8F7E8). It was server-only and is not
  on the device. A wrong key gives noise, not a crash (M).
- Candidate source: the public vgmstream HCA key list has an entry for this game (with AWB
  subkeys). **Not verified** against our files yet; the HCA header sync check is not a valid key
  test, a real decode is. `jdr-tool keys --soundkey <hex>` stores it; the server converts it to
  decimal for `getkey`.

## 10. in_memory_master_data: missing, must be rebuilt (H)

- A normal CDN bundle (obfuscated name not reproduced) bound **into memory** by TitleLoad
  @0xB8FD28 and never saved, so it is absent from the player copy and from `vcs.bytes`.
- Holds 18 ScriptableObject tables under `Assets/Resources/Common/InMemoryMasterData/` (rank,
  terminology, statue settings and special scenes, team rank, tutorial flags, statue/mind level,
  skill, ability, signboard, assist-ability conditions, map gimmicks, affect-text rules, action
  branches, area positions, two camera sets).
- Without it the game cannot pass TitleLoad. Plan: rebuild from type definitions plus authored
  content (M1.5, `docs/design/master-data.md`).

## Open points

1. SOUND_KEY: verify the public candidate with a real HCA decode (with AWB subkey).
2. Crafted `versionfile`: not yet tested on a device (MonoScript resolution expected to work).
3. `in_memory_master_data` table contents (M1.5).
4. `vcs.bytes` encSize rule (57-byte plain tail seen); only matters if we pre-seed it.
