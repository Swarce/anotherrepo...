# 01 — How the game finds its server, and how we redirect it

The client has exactly one built-in game-server URL (a metadata string literal). The first API call
(`/common/get_version`) returns the API base and the web-page base, so after that call the server
decides every URL. We rewrite that one literal and allow cleartext HTTP; no code is patched.

Confidence: H for the URL flow and transports (read from code), M for Android platform behaviour
(cleartext policy, trust store), L for third-party SDK traffic details.

Addresses are arm64 `libil2cpp.so` VAs of 3.12.0 (version code 60). Key values are never written
in this repo; they are referred to by role.

## 1. URL bootstrap (H)

| Step | Evidence | What happens |
|---|---|---|
| Static init | `Jjs.HttpConst::.cctor` @0x2037BC4 | Sets transport constants (timeouts, retries, AES parameters, notify URL). `ServerUrl` is still empty. |
| Default URL | `HttpConst::InitServerURL()` → `InitServerURL(int)` @0x203828C | Loads the production base `https://jdr.channel.or.jp` (metadata string literal **#4137**) and calls `SetURL`. |
| SetURL | `HttpConst::SetURL` @0x20382F4 | Stores the string verbatim. Nothing is appended. |
| Per call | `HttpConst::GetURL(apiName)` @0x2038430 | `ServerUrl + apiName`, plain concatenation. `apiName` starts with `/` (e.g. `/common/get_version`). |
| First call | `RequestResponseManager/<GetVersion>c__Iterator0` @0xE25E38 | `POST <default>/common/get_version` (bare host, no `/api`). |
| Switch | same, @0xE26090 / @0xE260DC | On status 10000 or 10012: `SetURL(param.url)` and `SetWebURL(param.s3_url)`. 10013 loads the `CheckVersionUI` scene (forced update). Anything else is an HTTP error dialog. |

Consequences:

- `get_version.url` is the API base for every later call. It must have no trailing slash.
- `get_version.s3_url` (`HttpConst.S3WebURL`) is only the base for WebView pages: terms
  (`/information/terms_a.html`), help, copyright, gasha detail and CA pages. It is **not** used for
  asset downloads (see 08). (H, all `S3WebURL` readers checked in C-assets Q4.)
- `RequestResponseManager.GetVersion(initServerURL=true)` resets `ServerUrl` to the built-in default
  first. Only `TakeOverController` uses that form; the normal login passes `false`. (H)
- `get_version` is called twice on a normal boot: once by `TitleUIController.GetVersion` @0xB913A0
  (reads only `is_ca`/`ca_message`) and once by `LoginChecker.Login` (sets the URLs). (H)

## 2. Hosts in the client

| Host | Where | Role | Offline |
|---|---|---|---|
| `https://jdr.channel.or.jp` | literal #4137, `InitServerURL(int)` | production API, first call only | rewritten by the patcher |
| `https://jdr-test-{0}.channel.or.jp/api` | `HttpConst::CreateTestServerURL` @0x2038358 | test-server template (debug env select) | unused; suggests prod returned a `.../api` base (M) |
| `https://jojodr-notify.s3-ap-northeast-1.amazonaws.com/notify.html` | `HttpConst.FirstNotifyURL` (static +0xA0) | notice page; consumer not traced (M) | dead; left as is (a failed WebView load is expected to be harmless, L) |
| `get_version.s3_url` | server-supplied | WebView pages | our `/web` prefix |
| `getfiledata` URLs | server-supplied per file | asset CDN (was S3 with signed, expiring URLs) | our `/cdn` prefix |
| Third-party SDK hosts | Firebase (FCM, Analytics), Google Play Games (libgpg), Play billing via Prime31, AdMob, Adjust, Growthbeat/GrowthPush, SmartBeat | contacted by their own Java/native code, not by game C# networking | fail harmlessly offline; removal planned in M6 (L: hostnames not enumerated) |

The realtime clan-room socket (`Jjs.Socket.*`) gets its address from `/clan/sync_server_info`
(server-supplied). The clan room will be hidden. (M)

## 3. Transports (H)

There are two separate HTTP stacks:

1. **API calls** use `ScSystem.ScNet.HttpAndroid`, created in `HttpManager::Init` @0x20386DC.
   It calls the Java class `jjs.spchun.co.jp.httpandroid.HttpProxy` (classes.dex) over JNI, which
   uses `java.net.HttpURLConnection`. Headers: `Connection: Keep-Alive`,
   `User-Agent: jgr (Android; <deviceModel>)`, `Content-Type: application/x-www-form-urlencoded`,
   `Accept-Encoding: identity`. Redirects are not followed (`allowAutoRedirect` forced false
   @0x1059338), no caches, 15 s connect and read timeout. Details in 02.
   (An earlier working note said API calls use managed `System.Net`. That was wrong.)
2. **Asset downloads** use Mono `System.Net.WebClient` (`DownloadRequestTask.startDownload`
   @0x10CC940, `WebClientWithTimeout.GetWebRequest` @0x105D7E8). Plain GET of the URL that
   `/client/getfiledata` returned. Details in 08.

## 4. Cleartext policy and our redirect

- The APK targets SDK 28, has no `networkSecurityConfig` and no `usesCleartextTraffic`. On Android 9+
  `HttpURLConnection` therefore refuses `http://` URLs, and for HTTPS only system CAs are trusted
  (user CAs are ignored for targetSdk >= 24). (M, Android platform rules)
- Mono's `WebClient` does its own sockets and ignores the Android cleartext policy, so asset URLs can
  be `http://` without any patch. (M)

What the patcher does (`patcher/src/patches.rs`, fingerprinted per build in `patcher/src/known.rs`):

| Patch id | Change | Why |
|---|---|---|
| `server-url` | Rewrites metadata literal #4137 in `global-metadata.dat` in place to `http://127.0.0.1:<port>` (default port 18080). The new string must not be longer than the original (25 bytes); the rest is zero-filled and the literal length updated. The literal must be used only once (checked). | Points the first `get_version` at the local server. |
| `cleartext` | Sets `android:usesCleartextTraffic="true"` on `<application>` in the binary manifest (round-trip checked). | Lets the Java stack talk plain HTTP to 127.0.0.1. |
| `drop-armv7` | Removes `lib/armeabi-v7a`. | The embedded server is arm64-only. |

Our server then answers `get_version` with `url = http://127.0.0.1:<port>/api` and
`s3_url = http://127.0.0.1:<port>/web` (`server/services/src/services/common.rs`). The bare-host
first call and `/api/...` calls both reach the same dispatcher (`server/services/src/app.rs`).

## 5. SSLChecker (H structure, M effect)

- `HttpBase..ctor` sets `ServicePointManager.ServerCertificateValidationCallback` to `Jjs.SSLChecker`
  (`OnRemoteCertificateValidationCallback`, class has a `FingerPrintList` and `IgnoreChainStatus`,
  .ctor @0xFD8BF8, .cctor @0xFD9024).
- This is a Mono hook. It is process-wide for Mono networking, so it affects the asset `WebClient`
  over HTTPS, but not the Java `HttpURLConnection` used for API calls.
- Whether the fingerprint list is populated was not checked. It does not matter for us: all our URLs
  are `http://`, so no certificate is ever validated.

## Open points

- Exact consumer and timing of `FirstNotifyURL` (M).
- Hostnames contacted by each third-party SDK (to be listed from a device log with network off, M6).
