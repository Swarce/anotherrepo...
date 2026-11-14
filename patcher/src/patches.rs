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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: format!("http://127.0.0.1:{}", DEFAULT_PORT),
            server_lib: None,
            keys_json: None,
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
pub const SERVER_MARKER: &[u8] = b"JDR_EMBEDDED_SERVER_v";

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
        Ok(format!("libjdr.so ({} KiB, sha256 {}…), libmain NEEDED liblog.so -> libjdr.so, {KEYS_ASSET} (stored)", lib.len() / 1024, &sha256_hex(&lib)[..12]))
    }
}

pub fn all() -> Vec<Box<dyn Patch>> {
    vec![
        Box::new(ServerUrl),
        Box::new(Cleartext),
        Box::new(DropArmv7),
        Box::new(EmbedServer),
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
    fn ids_are_unique() {
        let ids: Vec<_> = all().iter().map(|p| p.id()).collect();
        let mut s = ids.clone();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), ids.len());
    }
}
