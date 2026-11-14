# 07 — Integrity checks and anti-cheat

The client has a root flag that it only reports to the server, memory obfuscation for numbers, and
a Mono certificate-pinning hook. No APK signature, file-integrity-of-code or emulator check was
found, and nothing found so far blocks offline play or a re-signed APK.

Confidence: H for the classes and the login `os` suffix; M for "no signature/tamper checks" (string
and xref search plus the boot path reading, not a full audit); L for what `AndroidCheckRoot` tests.

## 1. CheatChecker / root flag (H for use, L for detection method)

- `Jjs.CheatChecker`: `Check` @0xF1F0B0, `CheckRoot` @0xF1F10C, `AndroidCheckRoot` @0xF1F110,
  property `IsRoot`.
- Called once from `JjsManager.Init` @0xB6D96C.
- Known consumer: `LoginChecker/<OnCheckLogin>` (0xB7C400..0xB7C5CC) appends `":root"` to the
  `os` field of `/user/login` when `IsRoot` is true. No client-side refusal to run was found (M).
- What the root test looks at was not traced (L). Offline it does not matter: our server logs the
  `os` string and ignores the suffix.
- Battle-end requests carry `enableCheatCheck` (`/bizarre/bizarre_end`, `/raid/raid_end`): a flag
  the client sends so the server could validate results. Our server may ignore it. (H field, M use)

## 2. Memory value obfuscation (H)

- `ScSystem.ScSecurity.SecurityIntValue` / `SecurityLongValue` / `SecurityFloatValue` /
  `SecurityDoubleValue` (fields `seed`, `securityValue`; getters e.g. @0xA99694) with
  `SecurityValueRandom`, plus `CryptValue`, `Obfuscation`.
- They hide in-memory numbers from memory scanners. They do not touch the network or files and are
  irrelevant for a server.

## 3. TLS pinning: SSLChecker (H structure, M effect)

- `Jjs.SSLChecker : ScSystem.ScNet.SSLCertificate` with `FingerPrintList` and `IgnoreChainStatus`,
  installed as the Mono `ServicePointManager.ServerCertificateValidationCallback` in `HttpBase..ctor`.
- It applies to Mono HTTPS only (the asset `WebClient`). API calls go through Java
  `HttpURLConnection`, which uses the Android trust store instead.
- Whether the list is populated was not checked. We use `http://` everywhere, so no certificate is
  validated on either path.

## 4. Envelope and account checks (H)

- The envelope has no MAC, signature, nonce or replay check (02).
- `/user/create_user` carries `signature` = uppercase hex of SHA-512 over the nonce and the
  `UserCreateSecretKey` (metadata literal #4141) (`HttpUtility.CreateUserCreateSignature`
  @0x20397C0). Only the server checked it. Our server does not verify it.
- Status codes 10011 (E_HASH_ERROR), 50001 and 50002 (nonce/signature) exist but are server-side
  verdicts; the client only shows them.

## 5. File integrity (H)

- Downloaded files are md5-checked against the server manifest (`versionfile`) after download, and
  `vcs.bytes` carries an md5 of its own body (08). This is data integrity, which we satisfy by
  building the manifest from the files we serve.
- No hash of `libil2cpp.so`, `global-metadata.dat` or `classes.dex` was found being computed or
  sent. (M)

## 6. APK signature and environment (M)

- No `PackageManager` signature query, emulator or debugger check was found by string and xref
  search in C# or classes.dex. The patched APK is re-signed with a persistent local key
  (`patcher/src/sign.rs`); the original must be uninstalled once because the signature differs
  (PLAYER-SETUP), and the game itself does not notice.
- Third-party SDKs may check signatures for their own services (Play Games, billing). Those services
  are not needed offline. (L)

## Summary: what blocks offline play

| Check | Blocks offline? |
|---|---|
| Root flag | no (reported only) |
| Memory obfuscation | no |
| SSLChecker | no (we use http) |
| Envelope / create-user signature | no (server-side only) |
| md5 of downloads | no, if `versionfile` matches the served files |
| APK signature / tamper / emulator | none found (M) |
| Device id required (`IsGetBackupID`) | could quit on unusual devices with no id at all (H rule, rare) |
