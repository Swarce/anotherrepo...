//! Builds the `versionfile` bundle (the server's asset manifest) from the player's own files.
//!
//! The client fetches it into memory at every boot (`AssetVersionManager.LoadServerVersionData`)
//! and compares each entry with its own record (`vcs.bytes`): same md5 + file present + same size
//! means "up to date". We list every file the game recorded, with the md5 it recorded, the real
//! size on disk and whether the file is encrypted, so nothing is re-downloaded.
//!
//! The engine type definitions (MonoScript, AssetBundle) come from one of the player's own
//! downloaded bundles; only their fingerprints are known here.

use jdr_crypto::{names, Jjec, Vcs};
use jdr_unity::bundle::{self, Node};
use jdr_unity::serialized::{self, Object, TypeEntry};
use jdr_unity::typetree::{class_hash, mono_behaviour_type, Field};
use jdr_unity::writer::W;
use jdr_unity::{CLASS_ASSETBUNDLE, CLASS_MONOSCRIPT, PLATFORM_ANDROID, UNITY_VERSION};
use std::path::Path;

pub const BUNDLE_NAME: &str = "versionfile";
pub const CONTAINER: &str = "assets/resources/version/android/versionfile.asset";
pub const NAMESPACE: &str = "ScSystem.ScIO";
pub const CLASS: &str = "AssetBundleVersionInfo";

/// MD5 of the raw type-table entries this engine build writes for MonoScript and AssetBundle.
pub const MONOSCRIPT_TYPE_MD5: &str = "83260eb10bffa21cb915158f03b4dbd1";
pub const ASSETBUNDLE_TYPE_MD5: &str = "70d7b502f7596fb4da2f438d50cc85d0";

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub obf: String,
    pub md5: String,
    pub size: i64,
    pub encrypted: bool,
}

pub fn md5_hex(b: &[u8]) -> String {
    use md5::{Digest, Md5};
    Md5::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn fields() -> Vec<Field> {
    vec![
        Field::string("CreateTime"),
        Field::i64("TotalSize"),
        Field::List {
            name: "VersionData".into(),
            elem_type: "VersionInfo".into(),
            fields: vec![
                Field::string("path"),
                Field::string("ObfuscationPath"),
                Field::string("md5Sum"),
                Field::i64("size"),
                Field::bool("isEncrypt"),
                Field::bool("isObfuscation"),
            ],
        },
    ]
}

/// What the player's `files/` folder holds compared with the game's own record.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FilesReport {
    pub files_on_disk: usize,
    pub vcs_present: bool,
    pub recorded: usize,
    /// Recorded in vcs.bytes but not on disk (obfuscated names).
    pub missing: Vec<String>,
}

/// Count files and compare them with vcs.bytes (cheap: metadata only).
pub fn files_report(dir: &Path, asset_key: &[u8], asset_iv: &[u8]) -> FilesReport {
    let mut r = FilesReport {
        files_on_disk: std::fs::read_dir(dir)
            .map(|d| d.flatten().filter(|e| e.path().is_file()).count())
            .unwrap_or(0),
        ..Default::default()
    };
    let Ok(raw) = std::fs::read(dir.join("vcs.bytes")) else {
        return r;
    };
    r.vcs_present = true;
    let Ok((_, _, plain)) = Vcs::decrypt(&raw, asset_key, asset_iv) else {
        return r;
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&plain) else {
        return r;
    };
    if let Some(keys) = v.get("keys").and_then(|k| k.as_array()) {
        r.recorded = keys.len();
        r.missing = keys
            .iter()
            .filter_map(|k| k.as_str())
            .filter(|k| !dir.join(k).is_file())
            .map(str::to_owned)
            .collect();
    }
    r
}

/// Entries from the player's `vcs.bytes` (obfuscated name -> md5) and the files on disk.
/// Files recorded but missing on disk are left out (the client cannot load them anyway).
pub fn entries_from_files(
    dir: &Path,
    asset_key: &[u8],
    asset_iv: &[u8],
) -> Result<Vec<Entry>, String> {
    let raw = std::fs::read(dir.join("vcs.bytes")).map_err(|e| {
        format!("{}/vcs.bytes: {e} (the game's downloaded files are not on this device; see PLAYER-SETUP: restore files/)", dir.display())
    })?;
    let (_, _, plain) =
        Vcs::decrypt(&raw, asset_key, asset_iv).map_err(|e| format!("vcs.bytes: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_slice(&plain).map_err(|e| format!("vcs.bytes JSON: {e}"))?;
    let keys = v
        .get("keys")
        .and_then(|k| k.as_array())
        .ok_or("vcs.bytes has no keys")?;
    let vals = v
        .get("values")
        .and_then(|k| k.as_array())
        .ok_or("vcs.bytes has no values")?;
    if keys.len() != vals.len() {
        return Err("vcs.bytes keys/values differ in length".into());
    }
    let mut out = Vec::with_capacity(keys.len());
    for (k, m) in keys.iter().zip(vals) {
        let obf = k.as_str().ok_or("bad key")?;
        let md5 = m.as_str().ok_or("bad md5")?;
        let stem = obf.strip_suffix(".bytes").unwrap_or(obf);
        let Some(name) = names::deobfuscate(stem, asset_key) else {
            continue;
        };
        let p = dir.join(obf);
        let Ok(meta) = std::fs::metadata(&p) else {
            continue;
        }; // not on disk: cannot be served
        let mut head = [0u8; 16];
        let encrypted = std::fs::File::open(&p)
            .and_then(|mut f| std::io::Read::read(&mut f, &mut head))
            .map(|n| Jjec::is_jjec(&head[..n]))
            .unwrap_or(false);
        out.push(Entry {
            path: format!("{name}.bytes"),
            obf: obf.to_string(),
            md5: md5.to_string(),
            size: meta.len() as i64,
            encrypted,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// The MonoScript and AssetBundle type entries from the first matching bundle in `dir`.
pub fn find_template_types(
    dir: &Path,
    asset_key: &[u8],
    asset_iv: &[u8],
) -> Result<(TypeEntry, TypeEntry), String> {
    let mut files: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.len(), e.path())))
        .filter(|(len, p)| {
            *len < 256 * 1024 && p.extension().and_then(|x| x.to_str()) == Some("bytes")
        })
        .collect();
    files.sort();
    // The engine types sit in every bundle with a MonoScript; the smallest few hundred suffice.
    for (_, p) in files.iter().take(400) {
        let Ok(mut d) = std::fs::read(p) else {
            continue;
        };
        if Jjec::is_jjec(&d) {
            match Jjec::decrypt(&d, asset_key, asset_iv) {
                Ok(x) => d = x,
                Err(_) => continue,
            }
        }
        let Ok(nodes) = bundle::read(&d) else {
            continue;
        };
        for n in nodes.iter().filter(|n| n.flags & 4 != 0) {
            let Ok(sf) = serialized::parse_types(&n.data) else {
                continue;
            };
            let ms = sf
                .types
                .iter()
                .find(|t| t.class_id == CLASS_MONOSCRIPT && md5_hex(&t.raw) == MONOSCRIPT_TYPE_MD5);
            let ab = sf.types.iter().find(|t| {
                t.class_id == CLASS_ASSETBUNDLE && md5_hex(&t.raw) == ASSETBUNDLE_TYPE_MD5
            });
            if let (Some(ms), Some(ab)) = (ms, ab) {
                return Ok((ms.clone(), ab.clone()));
            }
        }
    }
    Err("no bundle with the expected MonoScript/AssetBundle type definitions found in the game files".into())
}

const PATH_ASSETBUNDLE: i64 = 1;
const PATH_MONOSCRIPT: i64 = 2;
const PATH_MANIFEST: i64 = 3;

fn monoscript_data(hash: [u8; 16]) -> Vec<u8> {
    let mut w = W::new();
    w.string(CLASS); // m_Name
    w.i32(0); // m_ExecutionOrder
    w.bytes(&hash); // m_PropertiesHash
    w.string(CLASS);
    w.string(NAMESPACE);
    w.string("Assembly-CSharp.dll");
    w.buf
}

fn manifest_data(entries: &[Entry], create_time: &str) -> Vec<u8> {
    let mut w = W::new();
    w.pptr(0, 0); // m_GameObject (ScriptableObject: none)
    w.bool_aligned(true); // m_Enabled
    w.pptr(0, PATH_MONOSCRIPT); // m_Script
    w.string(BUNDLE_NAME); // m_Name
    w.string(create_time);
    w.i64(entries.iter().map(|e| e.size).sum());
    w.i32(entries.len() as i32);
    for e in entries {
        w.string(&e.path);
        w.string(&e.obf);
        w.string(&e.md5);
        w.i64(e.size);
        w.bool_aligned(e.encrypted);
        w.bool_aligned(true); // isObfuscation
    }
    w.align(4);
    w.buf
}

fn assetbundle_data() -> Vec<u8> {
    let mut w = W::new();
    w.string(BUNDLE_NAME); // m_Name
    w.i32(2); // m_PreloadTable
    w.pptr(0, PATH_MONOSCRIPT);
    w.pptr(0, PATH_MANIFEST);
    w.i32(1); // m_Container
    w.string(CONTAINER);
    w.i32(0); // preloadIndex
    w.i32(2); // preloadSize
    w.pptr(0, PATH_MANIFEST);
    w.i32(0); // m_MainAsset.preloadIndex
    w.i32(0);
    w.pptr(0, 0);
    w.u32(1); // m_RuntimeCompatibility
    w.string(BUNDLE_NAME); // m_AssetBundleName
    w.i32(0); // m_Dependencies
    w.bool_aligned(false); // m_IsStreamedSceneAssetBundle
    w.i32(0); // m_ExplicitDataLayout
    w.i32(7); // m_PathFlags
    w.i32(0); // m_SceneHashes
    w.buf
}

/// Build the bundle bytes.
pub fn build(
    entries: &[Entry],
    monoscript_type: &TypeEntry,
    assetbundle_type: &TypeEntry,
    create_time: &str,
) -> Vec<u8> {
    let f = fields();
    let hash = class_hash(NAMESPACE, CLASS, &f);
    let mb_type = mono_behaviour_type(&f, hash, hash);
    let types = vec![monoscript_type.clone(), mb_type, assetbundle_type.clone()];
    let objects = vec![
        Object {
            path_id: PATH_ASSETBUNDLE,
            type_index: 2,
            data: assetbundle_data(),
        },
        Object {
            path_id: PATH_MONOSCRIPT,
            type_index: 0,
            data: monoscript_data(hash),
        },
        Object {
            path_id: PATH_MANIFEST,
            type_index: 1,
            data: manifest_data(entries, create_time),
        },
    ];
    let sf = serialized::write(
        UNITY_VERSION,
        PLATFORM_ANDROID,
        &types,
        &objects,
        &[(0, PATH_MONOSCRIPT)],
    );
    let cab = format!("CAB-{}", md5_hex(BUNDLE_NAME.as_bytes()));
    bundle::write(
        &[Node {
            name: cab,
            data: sf,
            flags: 4,
        }],
        UNITY_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jdr_unity::serialized::parse_types;

    fn fake_type(class_id: i32) -> TypeEntry {
        let mut w = W::new();
        w.i32(class_id);
        w.u8(0);
        w.i16(-1);
        w.bytes(&[0u8; 16]);
        w.i32(0);
        w.i32(0);
        TypeEntry {
            class_id,
            raw: w.buf,
        }
    }

    #[test]
    fn files_report_without_vcs() {
        let d = std::env::temp_dir().join(format!("jdr-vf-report-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.bytes"), b"x").unwrap();
        let r = files_report(&d, b"0123456789abcdef", b"0123456789abcdef");
        assert_eq!((r.files_on_disk, r.vcs_present, r.recorded), (1, false, 0));
        assert!(
            entries_from_files(&d, b"0123456789abcdef", b"0123456789abcdef")
                .unwrap_err()
                .contains("restore files/")
        );
    }

    #[test]
    fn builds_a_readable_bundle() {
        let e = vec![Entry {
            path: "text.jp.bytes".into(),
            obf: "abcd.bytes".into(),
            md5: "0".repeat(32),
            size: 10,
            encrypted: true,
        }];
        let b = build(&e, &fake_type(115), &fake_type(142), "2026/11/14 00:00:00");
        let nodes = bundle::read(&b).unwrap();
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].name.starts_with("CAB-"));
        let p = parse_types(&nodes[0].data).unwrap();
        assert_eq!(
            p.types.iter().map(|t| t.class_id).collect::<Vec<_>>(),
            vec![115, 114, 142]
        );
    }
}
