//! One patch per concern. Each has an id, a one-line description for the patch report, an
//! on/off switch (`--disable <id>`) and tests.

use crate::apk::{ApkEdit, Error};
use crate::known;
use crate::meta::{self, Metadata};
use sha2::{Digest, Sha256};

pub const METADATA_PATH: &str = "assets/bin/Data/Managed/Metadata/global-metadata.dat";
pub const LIBIL2CPP_ARM64: &str = "lib/arm64-v8a/libil2cpp.so";

pub struct Config {
    /// Base URL the game will use instead of the dead production server.
    pub server_url: String,
    /// The embedded server library (`libjdr.so`, aarch64) to put into the APK.
    pub server_lib: Option<std::path::PathBuf>,
    /// The player's keys.json (from `jdr-tool keys`), embedded as a stored asset.
    pub keys_json: Option<Vec<u8>>,
    /// The player's redrawn UI art pack (zip), and their game files folder (layouts for the
    /// APK's own screens).
    /// The player's art packs (`--art`, repeatable: e.g. the redrawn UI art and the generated
    /// statue portraits); merged in order, a later pack winning on the same path.
    pub art_zips: Vec<std::path::PathBuf>,
    pub data_dir: Option<std::path::PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: format!("http://127.0.0.1:{}", DEFAULT_PORT),
            server_lib: None,
            keys_json: None,
            art_zips: Vec::new(),
            data_dir: None,
        }
    }
}

pub const DEFAULT_PORT: u16 = 18080;

pub trait Patch {
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;
    /// Apply to the APK. Returns a short detail line for the report.
    fn apply(
        &self,
        apk: &mut ApkEdit,
        cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error>;
}

pub fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// The fingerprinted build for this APK, or an error explaining why we refuse.
/// Call once on the untouched input, before any patch runs.
pub fn identify(apk: &mut ApkEdit) -> Result<&'static known::VerifiedBuild, Error> {
    let m = apk.read(METADATA_PATH)?;
    let sha = sha256_hex(&m);
    let build = known::by_metadata(&sha);
    if let Some(b) = build {
        let lib = apk
            .read(LIBIL2CPP_ARM64)
            .map_err(|_| "no lib/arm64-v8a/libil2cpp.so (split or 32-bit-only APK?)")?;
        let lsha = sha256_hex(&lib);
        if lsha != b.libil2cpp_arm64_sha256 {
            return Err(format!(
                "unverified build: libil2cpp.so sha256 {lsha} does not match the verified {}",
                b.version_name
            )
            .into());
        }
    }
    build.ok_or_else(|| {
        format!(
            "unverified build: global-metadata.dat sha256 {sha} is not in the verified list (see docs/PLAYER-SETUP.md for supported versions)"
        )
        .into()
    })
}

/// Rewrites the production server URL literal to the local server.
pub struct ServerUrl;

pub fn validate_server_url(url: &str, max_len: usize) -> Result<(), Error> {
    let rest = url
        .strip_prefix("http://127.0.0.1:")
        .ok_or("server URL must start with http://127.0.0.1:")?;
    let port: u16 = rest
        .parse()
        .map_err(|_| "server URL must end with a port number")?;
    if port == 0 {
        return Err("port 0 is not allowed".into());
    }
    if url.len() > max_len {
        return Err(format!(
            "server URL is {} bytes; at most {max_len} fit in place",
            url.len()
        )
        .into());
    }
    Ok(())
}

impl Patch for ServerUrl {
    fn id(&self) -> &'static str {
        "server-url"
    }
    fn description(&self) -> &'static str {
        "Point HttpConst's production URL literal at the local server (global-metadata.dat)"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        let data = apk.read(METADATA_PATH)?;
        let (idx, expected) = build.server_url_literal;
        let m = Metadata::parse(&data)?;
        let cur = m.literal(idx)?;
        if cur != expected.as_bytes() {
            return Err(format!(
                "fingerprint mismatch: literal {idx} is {:?}, expected {expected:?}",
                String::from_utf8_lossy(cur)
            )
            .into());
        }
        validate_server_url(&cfg.server_url, expected.len())?;
        let out = meta::replace_literal(&data, idx, cfg.server_url.as_bytes())?;
        let compress = apk.is_compressed(METADATA_PATH);
        apk.put(METADATA_PATH, out, compress);
        Ok(format!("literal #{idx}: {expected} -> {}", cfg.server_url))
    }
}

/// Removes 32-bit native libraries (the embedded server is arm64-only).
pub struct DropArmv7;

impl Patch for DropArmv7 {
    fn id(&self) -> &'static str {
        "drop-armv7"
    }
    fn description(&self) -> &'static str {
        "Remove lib/armeabi-v7a (32-bit); the game then installs as arm64-only"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        _build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        if !apk.exists(LIBIL2CPP_ARM64) {
            return Err(
                "refusing: no arm64-v8a libil2cpp.so, removing armeabi-v7a would leave no ABI"
                    .into(),
            );
        }
        let v: Vec<String> = apk
            .names()
            .into_iter()
            .filter(|n| n.starts_with("lib/armeabi-v7a/"))
            .collect();
        for n in &v {
            apk.remove(n);
        }
        Ok(format!("removed {} files", v.len()))
    }
}

/// Allows plain HTTP so the game's Java HTTP stack may reach `http://127.0.0.1`.
///
/// The client sends API calls through `ScNet.HttpAndroid` (Java `HttpURLConnection`); with
/// targetSdk 28 Android blocks cleartext unless the manifest allows it.
pub struct Cleartext;

pub const MANIFEST: &str = "AndroidManifest.xml";
/// `android:usesCleartextTraffic`
pub const ATTR_USES_CLEARTEXT: u32 = 0x0101_04ec;

impl Patch for Cleartext {
    fn id(&self) -> &'static str {
        "cleartext"
    }
    fn description(&self) -> &'static str {
        "Set android:usesCleartextTraffic=true so the Java HTTP stack may use http://127.0.0.1"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        let data = apk.read(MANIFEST)?;
        let sha = sha256_hex(&data);
        if sha != build.manifest_sha256 {
            return Err(format!("fingerprint mismatch: AndroidManifest.xml sha256 {sha}").into());
        }
        let mut x = crate::axml::Axml::parse(&data)?;
        if crate::axml::Axml::parse(&x.write())? != x {
            return Err("manifest does not round-trip; refusing".into());
        }
        let what = x.set_android_bool(
            "application",
            "usesCleartextTraffic",
            ATTR_USES_CLEARTEXT,
            true,
        )?;
        let out = x.write();
        let check = crate::axml::Axml::parse(&out)?;
        if check.get_android_bool("application", ATTR_USES_CLEARTEXT) != Some(true) {
            return Err("post-check failed: attribute not readable".into());
        }
        let compress = apk.is_compressed(MANIFEST);
        apk.put(MANIFEST, out, compress);
        Ok(format!(
            "<application android:usesCleartextTraffic=\"true\"> {what}"
        ))
    }
}

pub const LIBMAIN_ARM64: &str = "lib/arm64-v8a/libmain.so";
pub const SERVER_LIB_ENTRY: &str = "lib/arm64-v8a/libjdr.so";
pub const KEYS_ASSET: &str = "assets/jdr/keys.json";
pub const SERVER_MARKER: &[u8] = b"JDR_EMBEDDED_SERVER_";

/// The build tag after the marker (e.g. `b002`), if present.
pub fn server_lib_tag(b: &[u8]) -> Option<String> {
    let i = b
        .windows(SERVER_MARKER.len())
        .position(|w| w == SERVER_MARKER)?;
    let tag = b.get(i + SERVER_MARKER.len()..i + SERVER_MARKER.len() + 4)?;
    tag.iter()
        .all(|c| c.is_ascii_alphanumeric())
        .then(|| String::from_utf8_lossy(tag).into_owned())
}

/// Check that a file is our aarch64 embedded server library.
pub fn validate_server_lib(b: &[u8]) -> Result<(), Error> {
    let e = crate::elf::Elf::parse(b)?;
    if e.machine != crate::elf::EM_AARCH64 {
        return Err("server library is not aarch64".into());
    }
    if !b.windows(SERVER_MARKER.len()).any(|w| w == SERVER_MARKER) {
        return Err("server library has no JDR marker (wrong file?)".into());
    }
    if e.load_alignments().iter().any(|&a| a < 0x4000) {
        return Err("server library is not 16 KiB page aligned".into());
    }
    Ok(())
}

/// Puts the embedded server into the game process: adds `libjdr.so`, makes `libmain.so` load it
/// (its `liblog.so` dependency is renamed to `libjdr.so`, which itself depends on `liblog.so`),
/// and embeds the player's keys as a stored asset.
pub struct EmbedServer;

impl Patch for EmbedServer {
    fn id(&self) -> &'static str {
        "embed-server"
    }
    fn description(&self) -> &'static str {
        "Add libjdr.so (in-process server), load it from libmain.so, embed the player's keys"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        let lib_path = cfg
            .server_lib
            .as_ref()
            .ok_or("needs the server library (--server-lib, or libjdr.so next to jdr-patch)")?;
        let keys = cfg
            .keys_json
            .as_ref()
            .ok_or("needs --keys local/keys.json (from jdr-tool keys)")?;
        let lib = std::fs::read(lib_path).map_err(|e| format!("{}: {e}", lib_path.display()))?;
        validate_server_lib(&lib)?;
        let k: serde_json::Value =
            serde_json::from_slice(keys).map_err(|e| format!("keys.json: {e}"))?;
        for f in ["asset_key", "asset_iv", "http_key"] {
            if k.get(f)
                .and_then(|v| v.as_str())
                .map(str::is_empty)
                .unwrap_or(true)
            {
                return Err(format!(
                    "keys.json has no {f}; run jdr-tool keys --data ... --apk ..."
                )
                .into());
            }
        }
        let main = apk.read(LIBMAIN_ARM64)?;
        let sha = sha256_hex(&main);
        if sha != build.libmain_arm64_sha256 {
            return Err(format!("fingerprint mismatch: libmain.so sha256 {sha}").into());
        }
        let patched = crate::elf::replace_needed(&main, "liblog.so", "libjdr.so")?;
        let compress = apk.is_compressed(LIBMAIN_ARM64);
        apk.put(LIBMAIN_ARM64, patched, compress);
        apk.put(SERVER_LIB_ENTRY, lib.clone(), true);
        let slim = serde_json::json!({
            "asset_key": k["asset_key"], "asset_iv": k["asset_iv"],
            "http_key": k["http_key"], "sound_key": k.get("sound_key").cloned().unwrap_or(serde_json::Value::Null),
        });
        apk.put(KEYS_ASSET, serde_json::to_vec(&slim)?, false);
        Ok(format!("libjdr.so JDR_EMBEDDED_SERVER_{} ({} KiB, sha256 {}…), libmain NEEDED liblog.so -> libjdr.so, {KEYS_ASSET} (stored)", server_lib_tag(&lib).unwrap_or_else(|| "?".into()), lib.len() / 1024, &sha256_hex(&lib)[..12]))
    }
}

pub const LIBUNITY_ARM64: &str = "lib/arm64-v8a/libunity.so";

/// Replacement body (24 ARM64 instructions, our own code) for `UnityDefaultAllocator`'s
/// constructor at 0x9ff824 in the verified libunity: it builds a `DynamicHeapAllocator` instead,
/// field for field like the engine's own inline `ALLOC_FALLBACK` construction (MemoryManager
/// @0x50d27c..0x50d2dc), except the pool size: 16 MiB instead of the fallback heap's 1 MiB, so the
/// pool and large-allocation lists these busy heaps walk stay short. Disassembly:
/// ```text
/// stp x19,x30,[sp,#-16]! ; mov x19,x0 ; bl BaseAllocator::ctor(0x50f07c)
/// adrp x8,0x12cd000 ; add x8,x8,#0x620          ; DynamicHeapAllocator vtable
/// add x9,x19,#0xb8 ; add x10,x19,#0xc8 ; str xzr,[x19,#0xa8]
/// stp x9,x9,[x19,#0xb8] ; stp x10,x10,[x19,#0xc8] ; str x8,[x19]
/// add x0,x19,#0xd8 ; bl Mutex::ctor(0xa234c8)
/// mov w10,#0x1000000 ; stp x10,xzr,[x19,#0x108] ; mov w9,#1
/// str xzr,[x19,#0xb0] ; strb w9,[x19,#0x104] ; ldp x19,x30,[sp],#16 ; ret ; nop x4
/// ```
/// Equivalence proof on the real binary: tools/proofs/unity_heap_equiv.py.
pub const UNITY_HEAP_CTOR: [u32; 24] = [
    0xa9bf7bf3, 0xaa0003f3, 0x97ec3e14, 0xd0004668, 0x91188108, 0x9102e269, 0x9103226a, 0xf900567f,
    0xa90ba669, 0xa90caa6a, 0xf9000268, 0x91036260, 0x94008f1d, 0x52a0200a, 0xa910fe6a, 0x52800029,
    0xf9005a7f, 0x39041269, 0xa8c17bf3, 0xd65f03c0, 0xd503201f, 0xd503201f, 0xd503201f, 0xd503201f,
];

pub fn unity_heap_bytes() -> Vec<u8> {
    UNITY_HEAP_CTOR
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect()
}

/// Fixes Unity 2018's "Using memoryadresses from more that 16GB of memory" crash on Android 11+.
///
/// `UnityDefaultAllocator` (used for ALLOC_DEFAULT, ALLOC_GAMEOBJECT, ALLOC_GFX) records every
/// malloc'd block in a page table with only 5 slots of 4 GiB windows; Android's Scudo allocator
/// spreads memory over more windows, the lookup returns slot -1 and the engine crashes. The patch
/// makes those three allocators `DynamicHeapAllocator`s, which the engine already uses for its
/// fallback heap and which have no such table.
pub struct UnityHeap;

impl Patch for UnityHeap {
    fn id(&self) -> &'static str {
        "unity-heap"
    }
    fn description(&self) -> &'static str {
        "Fix Unity 2018 16GB-address crash on Android 11+: default allocators become DynamicHeapAllocators"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        let lib = apk.read(LIBUNITY_ARM64)?;
        let sha = sha256_hex(&lib);
        if sha != build.libunity_arm64_sha256 {
            return Err(format!("fingerprint mismatch: libunity.so sha256 {sha}").into());
        }
        let (off, want) = build.unity_default_alloc_ctor;
        let new = unity_heap_bytes();
        let window = lib
            .get(off..off + new.len())
            .ok_or("libunity.so too short")?;
        if sha256_hex(window) != want {
            return Err("fingerprint mismatch at the UnityDefaultAllocator constructor".into());
        }
        let mut out = lib.clone();
        out[off..off + new.len()].copy_from_slice(&new);
        let compress = apk.is_compressed(LIBUNITY_ARM64);
        apk.put(LIBUNITY_ARM64, out, compress);
        Ok(format!(
            "libunity.so @{off:#x}: UnityDefaultAllocator ctor -> DynamicHeapAllocator (96 bytes)"
        ))
    }
}

/// `mov w0, #1 ; ret`
pub const RETURN_TRUE: [u32; 2] = [0x5280_0020, 0xd65f_03c0];

/// Stops the game wiping its whole download folder on a fresh install.
///
/// `ResourceDownloadUIController.StartLoad` runs a one-time "cache clear of non-encrypted files"
/// (`DoCacheClearProcess` -> `IOManager.DeleteAllInDirectory(persistentDataPath)`) whenever the
/// PlayerPrefs flag read by `SaveDataManager.LoadNonObfuscatedClearFlag` is unset. That cleared
/// caches of very old versions before re-downloading from the server; offline it destroys the
/// player's only copy of the game data (seen on device after reinstalling). The patch makes the
/// flag read return true; its only caller is that cache-clear routine.
pub struct KeepFiles;

impl Patch for KeepFiles {
    fn id(&self) -> &'static str {
        "keep-files"
    }
    fn description(&self) -> &'static str {
        "Never run the one-time wipe of the downloaded files folder (fresh install / cleared storage)"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        let lib = apk.read(LIBIL2CPP_ARM64)?;
        if sha256_hex(&lib) != build.libil2cpp_arm64_sha256 {
            return Err("fingerprint mismatch: libil2cpp.so".into());
        }
        let (off, want) = build.clear_flag_fn;
        let window = lib.get(off..off + 32).ok_or("libil2cpp.so too short")?;
        if sha256_hex(window) != want {
            return Err("fingerprint mismatch at LoadNonObfuscatedClearFlag".into());
        }
        let mut out = lib.clone();
        let code: Vec<u8> = RETURN_TRUE.iter().flat_map(|w| w.to_le_bytes()).collect();
        out[off..off + code.len()].copy_from_slice(&code);
        let compress = apk.is_compressed(LIBIL2CPP_ARM64);
        apk.put(LIBIL2CPP_ARM64, out, compress);
        Ok(format!(
            "libil2cpp.so @{off:#x}: LoadNonObfuscatedClearFlag -> return true"
        ))
    }
}

/// `ret`
pub const RET: u32 = 0xd65f_03c0;

/// Turns off the Growthbeat analytics/push SDK. Offline it cannot reach its server: its Java
/// thread dies at startup with "Failed create HttpURLConnection" (logged as a FATAL EXCEPTION,
/// though Growthbeat's own handler keeps the app alive). Every game call goes through
/// `GrowthbeatBehaviour.Awake` (initialise + push token), `.SendTag` and `.SendEvent`; each now
/// returns at once, so the SDK is never started and nothing is tracked.
pub struct NoGrowthbeat;

impl Patch for NoGrowthbeat {
    fn id(&self) -> &'static str {
        "no-growthbeat"
    }
    fn description(&self) -> &'static str {
        "Never start the Growthbeat analytics SDK (its thread crashed at startup offline)"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        // Runs after keep-files (which checks the whole file), so only the sites are checked.
        let mut out = apk.read(LIBIL2CPP_ARM64)?;
        for (off, want) in build.growthbeat_fns {
            let window = out.get(off..off + 32).ok_or("libil2cpp.so too short")?;
            if sha256_hex(window) != want {
                return Err(format!("fingerprint mismatch at Growthbeat site {off:#x}").into());
            }
        }
        for (off, _) in build.growthbeat_fns {
            out[off..off + 4].copy_from_slice(&RET.to_le_bytes());
        }
        let compress = apk.is_compressed(LIBIL2CPP_ARM64);
        apk.put(LIBIL2CPP_ARM64, out, compress);
        let sites: Vec<String> = build
            .growthbeat_fns
            .iter()
            .map(|(o, _)| format!("{o:#x}"))
            .collect();
        Ok(format!(
            "libil2cpp.so @{}: GrowthbeatBehaviour Awake/SendTag/SendEvent -> return",
            sites.join(", ")
        ))
    }
}

/// `nop`
pub const NOP: u32 = 0xd503_201f;

/// Length of the `town-direct-ip` site: nine instructions.
pub const TOWN_DNS_LEN: usize = 36;

/// Lets the Town home connect without a name lookup. `SocketBase.Connect` (the client's
/// realtime socket) replaces the address `/clan/sync_server_info` gives with
/// `Dns.GetHostEntry(ip).HostName` before opening `TcpClient(host, port)`. That is a reverse
/// lookup of 127.0.0.1 and then a forward lookup of the name; Unity 2018.4's libil2cpp asks with
/// `AI_ADDRCONFIG`, which on Android finds no IPv4 address when the phone has no network, so
/// `Connect` throws, catches it and shows "Connection error E_00900030" without ever opening a
/// connection (seen on device, Build 25). The patch removes the nine instructions
/// (`bl GetHostEntry` .. `bl get_HostName; mov x22, x0`), so the IP string itself goes to
/// `TcpClient`, whose `Dns.GetHostAddresses` returns an IP literal without asking the resolver.
pub struct TownDirectIp;

impl Patch for TownDirectIp {
    fn id(&self) -> &'static str {
        "town-direct-ip"
    }
    fn description(&self) -> &'static str {
        "Town socket connects to the server's IP without a DNS lookup (failed offline: E_00900030)"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        // Runs after keep-files (which checks the whole file), so only the site is checked.
        let mut out = apk.read(LIBIL2CPP_ARM64)?;
        let (off, want) = build.town_dns_site;
        let window = out
            .get(off..off + TOWN_DNS_LEN)
            .ok_or("libil2cpp.so too short")?;
        if sha256_hex(window) != want {
            return Err(format!("fingerprint mismatch at SocketBase.Connect {off:#x}").into());
        }
        out[off..off + TOWN_DNS_LEN].copy_from_slice(&town_dns_nops());
        let compress = apk.is_compressed(LIBIL2CPP_ARM64);
        apk.put(LIBIL2CPP_ARM64, out, compress);
        Ok(format!(
            "libil2cpp.so @{off:#x}: SocketBase.Connect uses the IP as given (no Dns.GetHostEntry)"
        ))
    }
}

/// The bytes the `town-direct-ip` site becomes.
pub fn town_dns_nops() -> Vec<u8> {
    NOP.to_le_bytes().repeat(TOWN_DNS_LEN / 4)
}

/// `mov w0, #0`
pub const MOV_W0_0: u32 = 0x5280_0000;
/// `mov w8, #0`
pub const MOV_W8_0: u32 = 0x5280_0008;

/// Manifest elements `debloat` removes: `(tag, android:name)`. Firebase (messaging, analytics,
/// instance id), Google Mobile Ads, Google Sign-In, Google Play Games, app measurement, Adjust
/// and Growth Push (already never started by `no-growthbeat`), plus the push and phone-state
/// permissions. Kept: the launcher, Unity, the game's local notifications
/// (`jp.co.spchun.localpushlib`) and in-app billing (prime31; no real payments are made).
pub const DEBLOAT_ELEMENTS: &[(&str, &str)] = &[
    ("uses-permission", "android.permission.READ_PHONE_STATE"),
    (
        "uses-permission",
        "com.google.android.c2dm.permission.RECEIVE",
    ),
    (
        "uses-permission",
        "com.bandainamcoent.jojodr.permission.C2D_MESSAGE",
    ),
    (
        "permission",
        "com.bandainamcoent.jojodr.permission.C2D_MESSAGE",
    ),
    ("activity", "com.growthpush.view.AlertActivity"),
    ("activity", "com.google.android.gms.ads.AdActivity"),
    (
        "activity",
        "com.google.android.gms.auth.api.signin.internal.SignInHubActivity",
    ),
    (
        "activity",
        "com.google.android.gms.common.api.GoogleApiActivity",
    ),
    ("activity", "com.google.games.bridge.NativeBridgeActivity"),
    ("service", "com.growthpush.TokenRefreshService"),
    ("service", "com.growthpush.ReceiverService"),
    (
        "service",
        "com.google.firebase.messaging.MessageForwardingService",
    ),
    (
        "service",
        "com.google.android.gms.auth.api.signin.RevocationBoundService",
    ),
    (
        "service",
        "com.google.android.gms.measurement.AppMeasurementService",
    ),
    (
        "service",
        "com.google.android.gms.measurement.AppMeasurementJobService",
    ),
    (
        "service",
        "com.google.firebase.components.ComponentDiscoveryService",
    ),
    (
        "service",
        "com.google.firebase.messaging.FirebaseMessagingService",
    ),
    (
        "service",
        "com.google.firebase.messaging.cpp.ListenerService",
    ),
    (
        "service",
        "com.google.firebase.messaging.cpp.FcmInstanceIDListenerService",
    ),
    (
        "service",
        "com.google.firebase.messaging.cpp.RegistrationIntentService",
    ),
    ("receiver", "com.adjust.sdk.AdjustReferrerReceiver"),
    (
        "receiver",
        "com.google.android.gms.measurement.AppMeasurementReceiver",
    ),
    (
        "receiver",
        "com.google.android.gms.measurement.AppMeasurementInstallReferrerReceiver",
    ),
    (
        "receiver",
        "com.google.firebase.iid.FirebaseInstanceIdReceiver",
    ),
    (
        "provider",
        "com.google.firebase.provider.FirebaseInitProvider",
    ),
    ("meta-data", "com.google.android.gms.games.APP_ID"),
    ("meta-data", "com.google.android.gms.games.unityVersion"),
];

/// Native libraries `debloat` removes. Nothing links them (no `DT_NEEDED`); only the SDKs above
/// load them, and none of those starts any more.
pub const DEBLOAT_LIBS: &[&str] = &[
    "lib/arm64-v8a/libFirebaseCppApp-5.6.1.so",
    "lib/arm64-v8a/libFirebaseCppMessaging.so",
    "lib/arm64-v8a/libSmartBeatNdk.so",
    "lib/arm64-v8a/libgpg.so",
];

/// The bytes a stubbed method starts with.
pub fn stub_bytes(s: known::Stub) -> Vec<u8> {
    match s {
        known::Stub::Ret => RET.to_le_bytes().to_vec(),
        known::Stub::RetFalse => [MOV_W0_0, RET]
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect(),
    }
}

/// Takes out the analytics, crash-reporting, ads, push and Play Games SDKs, which an offline
/// game cannot use: their entry points in the game code return at once (SmartBeat, Adjust,
/// Play Games activation and auto sign-in; the title's Play Games button stays hidden), their
/// Android components and the push and phone-state permissions leave the manifest, and their
/// native libraries leave the APK (about 8 MB).
pub struct Debloat;

impl Patch for Debloat {
    fn id(&self) -> &'static str {
        "debloat"
    }
    fn description(&self) -> &'static str {
        "Remove Firebase, ads, SmartBeat, Adjust, Growth Push and Play Games (stubs, manifest, native libs)"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        // Check everything before changing anything.
        let mut lib = apk.read(LIBIL2CPP_ARM64)?;
        let (bo, bsha) = build.title_play_games_button;
        let sites = build
            .sdk_stubs
            .iter()
            .map(|&(o, sha, what, _)| (o, sha, what))
            .chain([(
                bo,
                bsha,
                "TitleUIController.SetupGooglePlayGamesLoginButton",
            )]);
        for (off, sha, what) in sites {
            let window = lib.get(off..off + 32).ok_or("libil2cpp.so too short")?;
            if sha256_hex(window) != sha {
                return Err(format!("fingerprint mismatch at {what} ({off:#x})").into());
            }
        }
        let data = apk.read(MANIFEST)?;
        let mut x = crate::axml::Axml::parse(&data)?;
        if crate::axml::Axml::parse(&x.write())? != x {
            return Err("manifest does not round-trip; refusing".into());
        }
        let have = x.elements();
        for (tag, name) in DEBLOAT_ELEMENTS {
            let n = have.iter().filter(|(t, v)| t == tag && v == name).count();
            if n != 1 {
                return Err(format!("manifest: expected one <{tag} {name}>, found {n}").into());
            }
        }
        for l in DEBLOAT_LIBS {
            if !apk.exists(l) {
                return Err(format!("no {l}").into());
            }
        }
        // libil2cpp.so
        for &(off, _, _, stub) in build.sdk_stubs {
            let b = stub_bytes(stub);
            lib[off..off + b.len()].copy_from_slice(&b);
        }
        lib[bo..bo + 4].copy_from_slice(&MOV_W8_0.to_le_bytes());
        let compress = apk.is_compressed(LIBIL2CPP_ARM64);
        apk.put(LIBIL2CPP_ARM64, lib, compress);
        // manifest
        let gone = x.remove_elements(|t, v| DEBLOAT_ELEMENTS.contains(&(t, v)));
        let out = x.write();
        let check = crate::axml::Axml::parse(&out)?;
        let left = check.elements();
        if let Some((t, v)) = left
            .iter()
            .find(|(t, v)| DEBLOAT_ELEMENTS.contains(&(t.as_str(), v.as_str())))
        {
            return Err(format!("post-check failed: <{t} {v}> still in the manifest").into());
        }
        for keep in [
            ("application", ""),
            (
                "activity",
                "com.google.firebase.MessagingUnityPlayerActivity",
            ),
        ] {
            if !left.iter().any(|(t, v)| (t.as_str(), v.as_str()) == keep) {
                return Err(format!("post-check failed: <{} {}> missing", keep.0, keep.1).into());
            }
        }
        let compress = apk.is_compressed(MANIFEST);
        apk.put(MANIFEST, out, compress);
        // native libraries
        for l in DEBLOAT_LIBS {
            apk.remove(l);
        }
        Ok(format!(
            "libil2cpp.so: {} SDK entry points stubbed, title Play Games button hidden; manifest: {} elements removed; {} native libraries removed",
            build.sdk_stubs.len(),
            gone.len(),
            DEBLOAT_LIBS.len()
        ))
    }
}

/// English for the in-APK ClientMes table (boot, download and error messages), from the
/// project's translation file; rows without a current translation stay as they are.
pub struct TranslateClientMes;

/// Apply the embedded English to a ClientMes Resources file. Returns (file, replaced, total).
pub fn translate_clientmes(file: &[u8]) -> Result<(Vec<u8>, usize, usize), Error> {
    let tr = jdr_services::l10n::Translations::embedded()?;
    let total = crate::clientmes::read(file)?.rows.len();
    let (out, n) = crate::clientmes::rewrite(file, &|label, text| match tr
        .lookup(&format!("clientmes:{label}"), text)
    {
        jdr_services::l10n::Lookup::Found(en) => Some(en.to_string()),
        _ => None,
    })?;
    Ok((out, n, total))
}

/// Rows of a ClientMes file whose text is exactly our English for that label.
pub fn clientmes_english_rows(file: &[u8]) -> Result<usize, Error> {
    let tr = jdr_services::l10n::Translations::embedded()?;
    Ok(crate::clientmes::read(file)?
        .rows
        .iter()
        .filter(|(l, t)| tr.english(&format!("clientmes:{l}")) == Some(t.as_str()))
        .count())
}

impl Patch for TranslateClientMes {
    fn id(&self) -> &'static str {
        "english-clientmes"
    }
    fn description(&self) -> &'static str {
        "English boot/download/error messages (in-APK ClientMes table)"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        let entry = crate::clientmes::ENTRY;
        let file = apk.read(entry)?;
        if sha256_hex(&file) != build.clientmes_sha256 {
            return Err("fingerprint mismatch: ClientMes".into());
        }
        let (out, n, total) = translate_clientmes(&file)?;
        let compress = apk.is_compressed(entry);
        apk.put(entry, out, compress);
        Ok(format!("ClientMes: {n} of {total} messages in English"))
    }
}

/// English for the Japanese built into the APK outside the ClientMes table: the `UI.Text` labels
/// of the built-in scenes (loading overlay, download, save select, contact and error screens) and
/// the player-visible C# string literals in `global-metadata.dat` (see `crate::apktext`). Each
/// string is replaced only while its source still hashes as the translation expects.
pub struct TranslateApkText;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApkTextCount {
    /// Scene labels in English / scene labels with Japanese.
    pub scenes: usize,
    pub scene_total: usize,
    /// Allow-listed literals in English / allow-listed literals.
    pub literals: usize,
    pub literal_total: usize,
    /// Literals longer than the original, moved to the end of the metadata file.
    pub relocated: usize,
}

fn embedded_english(
    tr: &jdr_services::l10n::Translations,
    key: &str,
    source: &str,
) -> Option<String> {
    match tr.lookup(key, source) {
        jdr_services::l10n::Lookup::Found(en) => Some(en.to_string()),
        _ => None,
    }
}

fn literal_str(m: &Metadata, i: usize) -> Result<String, Error> {
    String::from_utf8(m.literal(i)?.to_vec())
        .map_err(|_| format!("metadata literal {i} is not UTF-8").into())
}

/// Apply the embedded English to the scenes and literals of `build` in `apk`.
pub fn translate_apk_text(
    apk: &mut ApkEdit,
    build: &known::VerifiedBuild,
) -> Result<ApkTextCount, Error> {
    use crate::apktext;
    let tr = jdr_services::l10n::Translations::embedded()?;
    let mut c = ApkTextCount::default();
    for (file, sha) in build.apk_text_scenes {
        let entry = format!("{}{file}", apktext::DATA_DIR);
        let data = apk.read(&entry)?;
        if sha256_hex(&data) != *sha {
            return Err(format!("fingerprint mismatch: {entry}").into());
        }
        c.scene_total += apktext::scene_texts(&data)?
            .iter()
            .filter(|(_, t)| jdr_services::l10n::has_japanese(t))
            .count();
        let (out, n) = apktext::rewrite_scene(&data, &|pid, text| {
            embedded_english(&tr, &apktext::scene_key(file, pid), text)
        })?;
        if n > 0 {
            let compress = apk.is_compressed(&entry);
            apk.put(&entry, out, compress);
        }
        c.scenes += n;
    }
    let mut meta_file = apk.read(METADATA_PATH)?;
    for (i, _) in build.ui_literals {
        c.literal_total += 1;
        let source = literal_str(&Metadata::parse(&meta_file)?, *i)?;
        if let Some(en) = embedded_english(&tr, &apktext::literal_key(*i), &source) {
            let (out, moved) = meta::set_literal(&meta_file, *i, en.as_bytes())?;
            meta_file = out;
            c.literals += 1;
            c.relocated += moved as usize;
        }
    }
    if c.literals > 0 {
        let compress = apk.is_compressed(METADATA_PATH);
        apk.put(METADATA_PATH, meta_file, compress);
    }
    Ok(c)
}

/// Post-check: scene labels and allow-listed literals of `apk` that are exactly our English.
pub fn apk_text_english(
    apk: &mut ApkEdit,
    build: &known::VerifiedBuild,
) -> Result<(usize, usize), Error> {
    use crate::apktext;
    let tr = jdr_services::l10n::Translations::embedded()?;
    let mut scenes = 0;
    for (file, _) in build.apk_text_scenes {
        let data = apk.read(&format!("{}{file}", apktext::DATA_DIR))?;
        scenes += apktext::scene_texts(&data)?
            .iter()
            .filter(|(pid, t)| tr.english(&apktext::scene_key(file, *pid)) == Some(t.as_str()))
            .count();
    }
    let m_file = apk.read(METADATA_PATH)?;
    let m = Metadata::parse(&m_file)?;
    let mut literals = 0;
    for (i, _) in build.ui_literals {
        if tr.english(&apktext::literal_key(*i)).map(str::as_bytes) == Some(m.literal(*i)?) {
            literals += 1;
        }
    }
    Ok((scenes, literals))
}

impl Patch for TranslateApkText {
    fn id(&self) -> &'static str {
        "english-apk-text"
    }
    fn description(&self) -> &'static str {
        "English labels in the APK's own scenes (loading, download, save select) and C# UI literals"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        _cfg: &Config,
        build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        let c = translate_apk_text(apk, build)?;
        Ok(format!(
            "scenes: {} of {} labels in English ({} files); metadata: {} of {} UI literals ({} relocated)",
            c.scenes,
            c.scene_total,
            build.apk_text_scenes.len(),
            c.literals,
            c.literal_total,
            c.relocated
        ))
    }
}

/// The player's redrawn UI art (`--art`); nothing to do without a pack.
pub struct UiArt;

impl Patch for UiArt {
    fn id(&self) -> &'static str {
        "ui-art"
    }
    fn description(&self) -> &'static str {
        "Redrawn UI art from the player's pack (--art): title screens here, menus on the phone"
    }
    fn apply(
        &self,
        apk: &mut ApkEdit,
        cfg: &Config,
        _build: &known::VerifiedBuild,
    ) -> Result<String, Error> {
        if cfg.art_zips.is_empty() {
            return Ok("no art pack given (--art): skipped".into());
        }
        let mut pack = std::collections::BTreeMap::new();
        for zip in &cfg.art_zips {
            pack.extend(crate::art::read_pack_zip(zip)?);
        }
        let total = pack.len();
        let thumbs = pack.keys().filter(|k| k.contains("/thumbs/")).count();
        let r = crate::art::apply(apk, pack, cfg.data_dir.as_deref(), cfg.keys_json.as_deref())?;
        let skipped = if r.skipped.is_empty() {
            String::new()
        } else {
            format!("; left unchanged: {}", r.skipped.join("; "))
        };
        Ok(format!(
            "{total} images: {} stored for the menus (applied on the phone; {thumbs} new statue thumbnails), {} sprites replaced in {} title/boot screen file(s){skipped}",
            r.stored, r.in_apk, r.apk_files
        ))
    }
}

pub fn all() -> Vec<Box<dyn Patch>> {
    vec![
        Box::new(ServerUrl),
        Box::new(Cleartext),
        Box::new(DropArmv7),
        Box::new(UnityHeap),
        Box::new(KeepFiles),
        Box::new(NoGrowthbeat),
        Box::new(TownDirectIp),
        Box::new(Debloat),
        Box::new(EmbedServer),
        Box::new(TranslateClientMes),
        Box::new(TranslateApkText),
        Box::new(UiArt),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_validation() {
        assert!(validate_server_url("http://127.0.0.1:18080", 25).is_ok());
        assert!(validate_server_url("http://127.0.0.1:18080", 21).is_err());
        assert!(validate_server_url("http://10.0.0.1:18080", 25).is_err());
        assert!(validate_server_url("https://127.0.0.1:18080", 25).is_err());
        assert!(validate_server_url("http://127.0.0.1:0", 25).is_err());
        assert!(validate_server_url("http://127.0.0.1:x", 25).is_err());
    }

    #[test]
    fn town_direct_ip_is_nine_nops_after_the_other_lib_patches() {
        let b = town_dns_nops();
        assert_eq!(b.len(), TOWN_DNS_LEN);
        assert!(b.chunks(4).all(|w| w == NOP.to_le_bytes()));
        let ids: Vec<&str> = all().iter().map(|p| p.id()).collect();
        let at = |id| ids.iter().position(|&i| i == id).unwrap();
        // keep-files checks the whole libil2cpp.so first; this patch checks only its site.
        assert!(at("keep-files") < at("town-direct-ip"));
        let (off, _) = known::BUILDS[0].town_dns_site;
        let others = [known::BUILDS[0].clear_flag_fn.0]
            .into_iter()
            .chain(known::BUILDS[0].growthbeat_fns.iter().map(|f| f.0));
        for o in others {
            assert!(o + 32 <= off || off + TOWN_DNS_LEN <= o, "sites overlap");
        }
    }

    #[test]
    fn unity_heap_code_shape() {
        let b = unity_heap_bytes();
        assert_eq!(b.len(), 96);
        // ends with ret + nops, starts with the stp prologue
        assert_eq!(UNITY_HEAP_CTOR[0], 0xa9bf7bf3);
        assert_eq!(UNITY_HEAP_CTOR[19], 0xd65f03c0);
        assert!(UNITY_HEAP_CTOR[20..].iter().all(|&w| w == 0xd503201f));
        // both BLs land on the verified targets
        let bl = |i: usize| {
            let w = UNITY_HEAP_CTOR[i];
            let mut imm = (w & 0x03ff_ffff) as i64;
            if imm & (1 << 25) != 0 {
                imm -= 1 << 26;
            }
            (0x9ff824 + 4 * i as i64 + imm * 4) as u64
        };
        assert_eq!(bl(2), 0x50f07c);
        assert_eq!(bl(12), 0xa234c8);
    }

    #[test]
    fn return_true_encoding() {
        // movz w0, #1 ; ret
        assert_eq!(RETURN_TRUE[0] & 0xffe0_001f, 0x5280_0000);
        assert_eq!((RETURN_TRUE[0] >> 5) & 0xffff, 1);
        assert_eq!(RETURN_TRUE[1], 0xd65f_03c0);
    }

    #[test]
    fn ids_are_unique() {
        let ids: Vec<_> = all().iter().map(|p| p.id()).collect();
        let mut s = ids.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), ids.len());
    }
}
