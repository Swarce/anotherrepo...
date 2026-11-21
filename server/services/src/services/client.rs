//! `/client/*`: asset keys and asset download URLs.

use crate::app::{ApiError, ApiResult, CDN_PREFIX};
use crate::services::ok;
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde_json::{json, Value};

/// `{assetkey, assetvector, soundkey}`: the keys recovered from the player's own copy.
/// `soundkey` is a decimal string of the 64-bit CRI key.
pub fn getkey(ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    // While staged files are being imported, hold the reply (the client times out after 15 s and
    // retries), so the game waits on the title instead of failing on a missing vcs.bytes and
    // looping back to login.
    let t0 = std::time::Instant::now();
    while ctx.importing.load(std::sync::atomic::Ordering::Relaxed)
        && t0.elapsed() < std::time::Duration::from_secs(12)
    {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let k = ctx
        .settings
        .keys
        .as_ref()
        .ok_or_else(|| ApiError::Internal("no keys loaded".into()))?;
    let sound = k
        .sound_key
        .as_deref()
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .map(|v| v.to_string())
        // The client parses this with Convert.ToUInt64; an empty string would throw at boot.
        .unwrap_or_else(|| "0".to_string());
    ok(json!({ "assetkey": k.asset_key, "assetvector": k.asset_iv, "soundkey": sound }))
}

/// Map a requested asset path to a URL on our CDN.
/// The client asks for `"android/data/<obfuscated name>"`; files sit flat in the player's folder.
pub fn file_url(base: &str, requested: &str) -> String {
    let name = requested.rsplit('/').next().unwrap_or(requested);
    format!("{base}{CDN_PREFIX}/{name}")
}

/// `{filename:[..]} -> {url:[..]}`, one URL per name, same order.
pub fn getfiledata(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let base = ctx.base_url();
    let urls: Vec<String> = env
        .param
        .get("filename")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|v| file_url(&base, v.as_str().unwrap_or("")))
                .collect()
        })
        .unwrap_or_default();
    jdr_core::log::line(
        "assets",
        &format!(
            "getfiledata {} file(s): {}",
            urls.len(),
            env.param
                .get("filename")
                .map(|v| crate::app::truncate(&v.to_string(), 300))
                .unwrap_or_default()
        ),
    );
    ok(json!({ "url": urls }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        assert_eq!(
            file_url("http://127.0.0.1:18080", "android/data/0a59.bytes"),
            "http://127.0.0.1:18080/cdn/0a59.bytes"
        );
        assert_eq!(file_url("http://h", "x.bytes"), "http://h/cdn/x.bytes");
    }
}
