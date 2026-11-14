//! zipalign + apksigner from Android build-tools, and the persistent signing key.
//!
//! The same key signs every build so a new APK installs over the previous one and keeps the save.
//! It lives outside the repo: `$JDR_KEYSTORE` or `~/.config/JDR/jdr-signing.p12`.

use crate::apk::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const KEY_ALIAS: &str = "jdr";

pub fn default_keystore() -> Result<PathBuf, Error> {
    if let Ok(p) = std::env::var("JDR_KEYSTORE") {
        return Ok(PathBuf::from(p));
    }
    let home = std::env::var("HOME")
        .map_err(|_| "HOME is not set; set JDR_KEYSTORE to your signing key path")?;
    Ok(PathBuf::from(home).join(".config/JDR/jdr-signing.p12"))
}

/// Sidecar file next to the keystore that records the certificate digest of the key.
/// Every build must be signed by the same certificate, or it will not install over the last one.
pub fn pin_path(keystore: &Path) -> PathBuf {
    let mut s = keystore.as_os_str().to_owned();
    s.push(".cert.sha256");
    PathBuf::from(s)
}

/// SHA-256 digest of the keystore's certificate (lowercase hex, no colons) via keytool.
pub fn keystore_cert_sha256(keystore: &Path) -> Result<String, Error> {
    let pass = keystore_pass();
    let out = run(Command::new("keytool").args([
        "-list",
        "-v",
        "-keystore",
        &keystore.to_string_lossy(),
        "-storetype",
        "PKCS12",
        "-storepass",
        &pass,
        "-alias",
        KEY_ALIAS,
    ]))?;
    parse_keytool_sha256(&out).ok_or_else(|| "keytool printed no SHA256 fingerprint".into())
}

pub fn parse_keytool_sha256(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("SHA256:"))
        .map(|v| v.trim().replace(':', "").to_ascii_lowercase())
        .filter(|v| v.len() == 64)
}

/// Check the key against its pin (writing the pin the first time). Refuses a different key.
pub fn check_pin(keystore: &Path) -> Result<String, Error> {
    let digest = keystore_cert_sha256(keystore)?;
    let pin = pin_path(keystore);
    match std::fs::read_to_string(&pin) {
        Ok(p) if p.trim() == digest => Ok(digest),
        Ok(p) => Err(format!(
            "signing key {} has certificate {digest}, but {} expects {}. Builds signed with another key cannot install over your current one. Use your original key (JDR_KEYSTORE).",
            keystore.display(),
            pin.display(),
            p.trim()
        )
        .into()),
        Err(_) => {
            std::fs::write(&pin, format!("{digest}\n"))?;
            Ok(digest)
        }
    }
}

/// Keystore password. The key never leaves the player's machine; override with `JDR_KS_PASS`.
pub fn keystore_pass() -> String {
    std::env::var("JDR_KS_PASS").unwrap_or_else(|_| "jdr-offline".into())
}

/// Locate build-tools: `$JDR_BUILD_TOOLS`, else the newest `$ANDROID_HOME/build-tools/*`.
pub fn build_tools() -> Result<PathBuf, Error> {
    if let Ok(p) = std::env::var("JDR_BUILD_TOOLS") {
        return Ok(PathBuf::from(p));
    }
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Ok(root) = std::env::var(var) {
            let bt = Path::new(&root).join("build-tools");
            if let Ok(rd) = std::fs::read_dir(&bt) {
                let mut v: Vec<PathBuf> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.join("apksigner").exists())
                    .collect();
                v.sort();
                if let Some(p) = v.pop() {
                    return Ok(p);
                }
            }
        }
    }
    Err("Android build-tools not found: set JDR_BUILD_TOOLS to a folder containing zipalign and apksigner".into())
}

fn run(cmd: &mut Command) -> Result<String, Error> {
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {:?}: {e}", cmd.get_program()))?;
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(format!("{:?} failed: {text}", cmd.get_program()).into());
    }
    Ok(text)
}

pub fn ensure_key(keystore: &Path) -> Result<bool, Error> {
    if keystore.exists() {
        return Ok(false);
    }
    if let Some(d) = keystore.parent() {
        std::fs::create_dir_all(d)?;
    }
    let pass = keystore_pass();
    run(Command::new("keytool").args([
        "-genkeypair",
        "-keystore",
        &keystore.to_string_lossy(),
        "-storetype",
        "PKCS12",
        "-storepass",
        &pass,
        "-keypass",
        &pass,
        "-alias",
        KEY_ALIAS,
        "-keyalg",
        "RSA",
        "-keysize",
        "4096",
        "-validity",
        "36500",
        "-dname",
        "CN=JDR Offline Preservation",
    ]))?;
    Ok(true)
}

pub struct SignResult {
    pub schemes: Vec<String>,
    pub cert_sha256: String,
}

/// zipalign (4-byte, .so on 16 KiB pages) then sign v1+v2+v3, then verify.
pub fn align_and_sign(unsigned: &Path, out: &Path, keystore: &Path) -> Result<SignResult, Error> {
    let bt = build_tools()?;
    let aligned = unsigned.with_extension("aligned.apk");
    run(Command::new(bt.join("zipalign"))
        .args(["-f", "-P", "16", "4"])
        .arg(unsigned)
        .arg(&aligned))?;
    let pass = format!("pass:{}", keystore_pass());
    run(Command::new(bt.join("apksigner"))
        .args([
            "sign",
            "--ks",
            &keystore.to_string_lossy(),
            "--ks-type",
            "PKCS12",
            "--ks-pass",
            &pass,
            "--ks-key-alias",
            KEY_ALIAS,
            "--v1-signing-enabled",
            "true",
            "--v2-signing-enabled",
            "true",
            "--v3-signing-enabled",
            "true",
            "--v4-signing-enabled",
            "false",
            "--out",
            &out.to_string_lossy(),
        ])
        .arg(&aligned))?;
    let _ = std::fs::remove_file(&aligned);
    run(Command::new(bt.join("zipalign"))
        .args(["-c", "-P", "16", "4"])
        .arg(out))?;
    let v = run(Command::new(bt.join("apksigner"))
        .args(["verify", "--verbose", "--print-certs"])
        .arg(out))?;
    parse_verify(&v)
}

pub fn parse_verify(text: &str) -> Result<SignResult, Error> {
    let mut schemes = vec![];
    for (scheme, label) in [
        ("v1", "v1 scheme (JAR signing)"),
        ("v2", "v2 scheme (APK Signature Scheme v2)"),
        ("v3", "v3 scheme (APK Signature Scheme v3)"),
    ] {
        if text
            .lines()
            .any(|l| l.contains(label) && l.trim_end().ends_with("true"))
        {
            schemes.push(scheme.to_string());
        }
    }
    let cert = text
        .lines()
        .find_map(|l| l.split("certificate SHA-256 digest: ").nth(1))
        .map(|s| s.trim().to_string())
        .ok_or("apksigner verify printed no certificate digest")?;
    if schemes.len() < 3 {
        return Err(format!("expected v1+v2+v3 signatures, got {schemes:?}").into());
    }
    Ok(SignResult {
        schemes,
        cert_sha256: cert,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_keytool_fingerprint() {
        let t = "Alias name: jdr\nCertificate fingerprints:\n\t SHA1: 00:11\n\t SHA256: AB:CD:EF:01:23:45:67:89:AB:CD:EF:01:23:45:67:89:AB:CD:EF:01:23:45:67:89:AB:CD:EF:01:23:45:67:89\n";
        assert_eq!(
            parse_keytool_sha256(t).unwrap(),
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
        );
        assert_eq!(parse_keytool_sha256("SHA256: 12:34"), None);
    }

    #[test]
    fn parses_apksigner_output() {
        let t = "Verifies\nVerified using v1 scheme (JAR signing): true\nVerified using v2 scheme (APK Signature Scheme v2): true\nVerified using v3 scheme (APK Signature Scheme v3): true\nVerified using v4 scheme (APK Signature Scheme v4): false\nSigner #1 certificate SHA-256 digest: abcd\n";
        let r = parse_verify(t).unwrap();
        assert_eq!(r.schemes, ["v1", "v2", "v3"]);
        assert_eq!(r.cert_sha256, "abcd");
        assert!(parse_verify(
            "Verified using v1 scheme (JAR signing): true\nSigner #1 certificate SHA-256 digest: x"
        )
        .is_err());
    }
}
