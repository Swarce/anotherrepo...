# Envelope (implementer spec)

Evidence and confidence: [../research/02-protocol.md](../research/02-protocol.md).
Code: `server/core/src/envelope.rs`, `server/services/src/app.rs`.

## Key

`K` = UTF-8 bytes of metadata string literal #4143 (16 bytes), read from the player's APK by
`jdr-tool keys --apk` into `keys.json` (`http_key`). Never commit it. Same key both directions.

## Request (client → server)

```
POST <base><apiName>            base = built-in URL for get_version, then get_version.url
Content-Type: application/x-www-form-urlencoded
Body: data=<pct(pct(base64(IV16 || AES-128-CBC-PKCS7(K, IV16, utf8(json)))))>
```

1. Strip `data=`; percent-decode repeatedly until no `%` remains; base64-decode.
2. First 16 bytes = IV, rest = ciphertext; decrypt; remove PKCS7 padding.
3. Parse JSON: `uuid, session, language, date, platform, version, lzAccept, lzEncode, param`.
4. If `lzEncode == 1` and `param` is a string: `param = json(gunzip(base64(param)))` (not sent by
   3.12.0, cheap to support).

## Response (server → client)

```
HTTP 200, no Content-Encoding (or exactly "gzip"), no redirect, any Content-Type
Body: base64(IV16 || AES-128-CBC-PKCS7(K, IV16, utf8(json)))      (no "data=", no line breaks)
```

```
{"session":"<s>","result":0,"date":"yyyy-MM-dd HH:mm:ss","datalVer":"<constant>",
 "serverVer":"<any>","lzEncode":0,
 "param":{"status":10000,"emergencyInfo":{"id":0,"info":"","scroll":0},"frenzyTextId":"", ...fields}}
```

Rules:

- IV: any 16 bytes; use 16 lowercase hex ASCII chars like the client.
- JSON: compact, UTF-8, control characters escaped (raw TAB/LF/CR/NUL/VT are deleted by the client).
- `session`: always present. Echo the client's, or issue one on login. An empty value wipes the
  client's session.
- `date`: JST wall clock in `yyyy-MM-dd HH:mm:ss`.
- `datalVer`: the same string for the whole session, every response. A change breaks all later
  bundle loads.
- `param.status`: 10000 for success. Game-level codes must be in the pass-through table
  (02 §6), or the client shows a generic error and drops the response. Never omit `status`.
- `param.emergencyInfo`: always include it with `id:0`.
- Field names are case-sensitive and must match the client classes exactly (e.g. `uuId`).
- Optional compression: `lzEncode:1` and `param` = base64(gzip(paramJSON)) string.

## HTTP-level outcomes

| Server sends | Client does |
|---|---|
| 200 + valid body | normal |
| 200 + status not pass-through | generic error dialog, response dropped, session not updated |
| 200 + status 10005 | version-error scene |
| 503 + plain text body | maintenance dialog with that text |
| no answer / 408 | retry dialog (retry builds a new body) |
| other HTTP codes, redirects | error dialog |

The client retries IO failures up to 3 times with a byte-identical body, so the same request can
arrive more than once. Requests are serialized (one in flight).
