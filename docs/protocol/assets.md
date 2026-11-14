# Assets and CDN (implementer spec)

Evidence and confidence: [../research/08-assets-cdn.md](../research/08-assets-cdn.md).
Code: `server/services/src/services/client.rs`, `server/services/src/cdn.rs`, `server/crypto/`.

## /client/getkey

`{} → {assetkey, assetvector, soundkey}`

- `assetkey`, `assetvector`: the original ASSET_KEY / ASSET_IV as 16-char strings. They decrypt JJEC
  bundles **and** the player's `vcs.bytes`. A wrong value makes the client delete `vcs.bytes` and
  re-download everything (and JJEC bundles fail).
- `soundkey`: CRI key as a decimal `UInt64` string. Wrong value → noisy downloaded audio.
- All three come from the local `keys.json` (`jdr-tool keys`); never from the repo.

## /client/getfiledata

`{filename: ["android/data/<obfuscated>.bytes", ...]} → {url: [...]}`

- Exactly one URL per filename, same order (matched by index).
- URLs are fetched with Mono `WebClient` GET, so `http://` works without any patch.
- URLs are re-requested after 282 s; do not rely on them being fetched immediately.
- Build 01: `http://127.0.0.1:<port>/cdn/<last path segment>`.

## GET of a file URL

- Body = the file exactly as it is stored on the device (JJEC, UnityFS or CPK bytes). No transport
  encryption or compression; send no `Content-Encoding`.
- md5 of the body must equal `md5Sum` in `versionfile`, or the client deletes the file and shows an
  error.
- The CDN serves only the player's own files directory; Build 01 supports Range and rejects path
  escapes.

## versionfile (server manifest; we build it)

Requested through getfiledata like any file (plain name `versionfile`), bound into memory every
boot, no md5 check, plain UnityFS accepted.

```
AssetBundleVersionInfo : ScriptableObject {          // ScSystem.ScIO, Assembly-CSharp.dll
  string CreateTime; long TotalSize; List<VersionInfo> VersionData;
}
VersionInfo { string path; string ObfuscationPath; string md5Sum; long size;
              bool isEncrypt; bool isObfuscation; }
```

Per served file:

| Field | Value |
|---|---|
| `path` | plain lowercase name + `.bytes`, no directory (e.g. `text.jp.bytes`) |
| `ObfuscationPath` | obfuscated name + `.bytes`, no directory |
| `md5Sum` | lowercase hex md5 of the served bytes |
| `size` | byte length of the served file |
| `isEncrypt` | true if the file starts with `JJEC` |
| `isObfuscation` | true |

Container path inside the bundle: `assets/resources/version/android/versionfile.asset`. Every file
the client may ask for needs an entry (missing entry → `NOT_FIND_ERR`). Replaced files (English
`text.jp`, `*_script_text_jp`, rebuilt `in_memory_master_data`) get their new md5 and size here.

## Up-to-date rule (client side)

A file is not downloaded when: `versionfile.md5Sum == vcs.bytes[obfuscated name]`, the file exists,
and its length equals `size`. So for an existing install, use the md5/size of the player's files and
nothing is re-downloaded; on a fresh install everything is fetched once from us.

## vcs.bytes

Local only. The client writes it; the server never serves it. Format in 08 §3.

## Envelope rule that affects assets

Send the same `datalVer` in every API response. A change mid-session makes every later bundle load
fail ("data was updated").

## Status in Build 01

| Piece | State |
|---|---|
| getkey, getfiledata, CDN file serving | implemented |
| versionfile builder | not yet (design: `docs/design/master-data.md`, M1.5) |
| in_memory_master_data | missing; rebuild in M1.5 |
| English text bundles | T1–T2 |
