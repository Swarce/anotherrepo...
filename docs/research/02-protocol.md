# 02 — API envelope (request/response wire format)

Every API call is an HTTP POST with one form field `data` holding base64(IV || AES-128-CBC(JSON)).
The response body is the same scheme without the form wrapper. The JSON is Unity `JsonUtility`
output with a fixed header and a `param` object; responses carry a `status` code that decides
whether the calling screen ever sees the response.

Confidence: H for crypto parameters, field names, status handling and HTTP error handling (read
from code); M for `JsonUtility` formatting details and exact retry triggers; L for the timezone of
`date`.

Implementer summary: [../protocol/envelope.md](../protocol/envelope.md). Addresses are arm64
`libil2cpp.so` VAs. The envelope key is referred to by role only: it is metadata string literal
**#4143** (`HttpConst.HTTPRijndaelKey`), read from the player's APK by `jdr-tool keys --apk`.

## 1. Crypto parameters (H)

`Jjs.HttpConst::.cctor` @0x2037BC4 sets them; `JjsManager::Init` @0xB6DB8C passes them to
`RequestResponseManager::SetEncryptor` @0xE24BDC, which builds one `RijndaelManaged`.

| Parameter | Value |
|---|---|
| Cipher | AES-128 (Rijndael, key size 128, block size 128) |
| Mode / padding | CBC / PKCS7 |
| Key | UTF-8 bytes of literal #4143 (16 chars). Same key both directions. |
| IV | 16 bytes, sent as a prefix of the ciphertext. Client: first 16 lowercase hex chars of `Guid.NewGuid()`. Any 16 bytes work. |
| MAC / signature / nonce | none at envelope level |

Other `HttpConst` secrets: `UserCreateSecretKey` (literal #4141) is app-level, used only for the
`/user/create_user` signature (see 07). `HttpCommonKey` (literal #4140) has no reader (M: probably dead).
No custom HTTP headers: `HttpRequestDefaultParameter.AddRequestHeader` @0x105B924 has no callers.

## 2. Request (H)

Built by `RequestBase`1::CreatePostRequestParameterCallback` @0x1337804:

1. `CreateRequestHeader` @0x1337644 fills the header fields (table below).
2. `GetJsonData` @0x1338980: copies the request param into `param`, then `JsonUtility.ToJson(this)`.
3. `Regex.Replace(json, "[\t\n\r\0\v]*", "")` (`HttpConst.RemoveJsonCode`).
4. Encrypt with a fresh IV, prepend IV, standard base64 (with `=` padding, no line breaks).
5. Body = `"data=" + EscapeDataString(EscapeDataString(b64))`. **Escaped twice** (0x1337B80,
   0x1337B88): `+` becomes `%252B`. The server must percent-decode until no `%` is left.

| JSON key | Type | Value |
|---|---|---|
| `uuid` | string | PlayerPrefs `uuid`; `""` before account creation |
| `session` | string | last response's `session` (memory only) |
| `language` | int | `PlatformConst.LanguageId` = 1 (JP, compile-time) |
| `date` | long | device local time as `yyyyMMddHHmmss` digits |
| `platform` | int | 2 (Android) |
| `version` | string | `VersionConst.ApplicationServerVersion` literal (build id; usable for the 10005/10013 gate) |
| `lzAccept` | int | 1 |
| `lzEncode` | int | 0 (`SetParamCompression` @0x203806C has no callers) |
| `param` | object | the endpoint's request class fields (`paramData` is `[NonSerialized]`) |

Requests always use `Request<T>` (never `RequestCompression<T>`). Transport headers are in 01 §3.
The post buffer is 1 MiB (`HttpPostDataMaxSize`).

## 3. JsonUtility rules (M, standard Unity 2018.4 behaviour)

- Keys are C# field names, case-sensitive, base class first, in declaration order.
- Compact output; null string → `""`; enums and `long` → integers; `bool` → `true`/`false`.
- Nested `[Serializable]` classes are objects and `List<T>` are arrays, never null.
- No `Dictionary`, `DateTime` or `TimeSpan` (those fields stay default).
- On read: unknown keys are ignored; missing keys keep their default. So the risks for a server are
  wrong values (status, enums, empty ids), not missing fields.
- Some names are misspelled in the client and must be matched exactly (e.g. `uuId` in
  `CreateUserResponse`, `LoginResponeStatusPattern`, `GashPlayRequest`).

## 4. Compression ("lz" is gzip) (H)

- SharpZipLib `GZipOutputStream`/`GZipInputStream`, applied to the `param` JSON only, inside the
  envelope, carried as a base64 **string** in `param`.
- Requests: never compressed in this build.
- Responses: `ResponseBase`1::CreateInstance` @0x13389C8 first reads only `lzEncode`. Exactly 1 means
  `param` is `base64(gzip(paramJSON))` (`ResponseCompression`1::Init` @0x1338B10); any other value
  means `param` is an object.
- HTTP `Content-Encoding`: absent or exactly `gzip`. Anything else throws in the Java reader.

## 5. Response (H)

Decoded by `ResponseInfo`1::ResponseCallback` @0x133952C:

1. HTTP status must be 200 (else §7).
2. Body text → one optional percent-decode → base64-decode → first 16 bytes IV → AES decrypt.
3. Quirk: `GetDecryptData` @0xE24F10 returns the whole buffer including the padding area, so the
   string ends in NULs; the `RemoveJsonCode` regex deletes them. The same regex deletes **raw**
   TAB/LF/CR/NUL/VT anywhere in the JSON, so control characters must be JSON-escaped (any standard
   encoder does this).
4. `ResponseBase<T>.CreateInstance(json)`.

Top level (`Jjs.ResponseBase`1`):

| Key | Type | Client use |
|---|---|---|
| `session` | string | becomes the next request's `session` (§8). Always send one. |
| `result` | int (OK=0, NG=1) | not read in the decode path (M); send 0 |
| `date` | string | `DateTime.TryParse` → server time; on failure the device clock is used. Use `yyyy-MM-dd HH:mm:ss` (JST assumed, L). |
| `datalVer` | string | asset data version (§8). Keep constant. |
| `serverVer` | string | no reader found (M); we echo the request `version` |
| `lzEncode` | int | §4 |
| `param` | object or string | the endpoint response class |

Every response class derives from `ResponseParamBase`:

| `param` key | Rule |
|---|---|
| `status` | **10000 = E_OK**. Required; a missing status (0) is an error. |
| `emergencyInfo` | `{id, info, scroll}`. `id != 0` shows an in-game emergency notice (`EmergencyInfo.IsValid` @0xD11EE4). Always send `{"id":0,"info":"","scroll":0}`: `SetEmergencyInfo` @0xE25AF8 has no null check. |
| `frenzyTextId` | string, unused in the envelope path; send `""` |

## 6. Status pass-through (H)

After decode (`ResponseCallback` 0x1339AA0..0x1339F54):

1. `status == 10005` (E_VERSION_INVALID) → response discarded → `CheckVersionUI` scene.
2. Else if `ResponseParameterCode.IsGameErrorCode(status)` @0xFD5448 is true (code is in the static
   table with `IsThroughSystemError = 1`) → state updated (§8), response delivered to the caller as
   OK. The screen handles the code itself.
3. Else → response discarded, generic error dialog (`ErrorManager.SetHttpError`), request dropped.
   Session, server time and `datalVer` are **not** updated.

Table from `ResponseParameterCode::.cctor` @0xFD57E4:

- **Pass-through:** 10000, 10012, 10013, 10102–10110, 10112–10114, 10116, 10117, 20005–20008,
  40001–40004, 50001, 50002, 50004–50008, 50010, 50101–50106, 50111–50117, 50119, 50201–50213,
  50301–50303, 50308, 50309, 51102, 51104, 51201–51206, 51301, 51302, 51401, 51402, 51501, 51707,
  51801–51804, 51901, 52001–52003, 52006–52008, 52010, 52015, 52017–52019, 52101, 52102, 52201.
- **System error:** -1, 10001–10006, 10008, 10009, 10011, 10014, 10015, 10101, 10111, 10115,
  20001–20004, 20009–20012, 30001, 40005, 50003, 50009, 51001, 51002, 51101, 51103, 51303, 51304,
  51601, 51701–51706, 51708–51715, 52004, 52005, 52009, 52011–52014, 52016.
- Codes not in the table (e.g. 0, and enum members 50118, 51310–51312) behave as system errors.
  Ranges above list only codes that exist in the `Status` enum (50107–50110 do not exist).
- Note: an earlier draft (A-envelope) listed 10109 and 50308 as undecoded; the full parse
  (B-boot §9) has both as pass-through.

Examples used in the boot flow: 50301 (E_BIZALLE_NOT_START) is the normal answer to
`/bizarre/bizarre_resumption`; 10003 (E_SESSION_NG) gives an error dialog.

## 7. HTTP errors and retries (H, M for exact triggers)

`ResponseInfo`1::ErrorCallback` @0x133A040:

| HTTP | Client result |
|---|---|
| 200 | normal decode |
| 0 (no response) or 408 | retry dialog; "retry" rebuilds the body (new IV and date) |
| 503 with a body | body read as plain UTF-8 text, shown in a maintenance dialog (M on the exact dialog) |
| anything else (3xx, 4xx, 5xx, 503 without body) | error dialog, request dropped |

Transport retries (`HttpBase::OnGetResponse` @0x105AFE0): up to 3 attempts in total
(`HttpRequestRetryMaxNum`), 1000 ms apart, only on IO failure/timeout. Retries resend the
**byte-identical** body, so the server can see the same request up to 3 times. Requests are strictly
serialized: one in flight, the rest queued.

## 8. Client state taken from responses (H)

`RequestResponseManager::UpdateExecuteResponseCache` @0xE25924, only for pass-through statuses:

| Client field | Source |
|---|---|
| `session` | `response.session`, overwritten unconditionally (empty wipes it) |
| `serverTime` | `DateTime.TryParse(response.date)` |
| `datalVer` | `response.datalVer`, also `AssetVersionManager.SetDataVersion` @0x10C6C10: a later different value marks the data as changed, and every later bundle load fails with the "data was updated" error (see 08) |
| response cache | the response object, keyed by response type (client-side, memory only) |

`uuid` is never taken from the envelope. App code stores it in PlayerPrefs after `/user/create_user`,
`/user/select`, `/user/change_backup_id` and data move (see 03). No response headers are read.

## 9. What Build 01 implements

`server/core/src/envelope.rs`: request decode (repeated percent-decoding, control-character strip,
optional gzip `param`), response encode (always `lzEncode:0`, base fields added when missing, hex IV).
`server/services/src/app.rs`: handler status codes are passed through as `param.status`; internal
errors become 10001; constant `datalVer` `"1"`; session echoed or issued per uuid; `serverVer` =
request `version`.

## Open points

- `serverVer` and `result` readers outside the envelope path (not scanned).
- Exact UI for 503 maintenance and for each non-pass-through code.
- Timezone of `date` (JST assumed, L).
