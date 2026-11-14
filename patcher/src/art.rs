//! `ui-art`: the player's redrawn UI art.
//!
//! The pack (a zip as exported by `tools/ui-assets/export_ui.py` and redrawn by the player, with
//! `replacements/<bundle>/sprites/<sprite>.png`) is split in two:
//! - images for downloaded bundles go into the APK as a stored asset (`assets/jdr/art.bin`); the
//!   embedded server applies them when it rebuilds those bundles on the phone;
//! - images for the APK's own screens (`apk_<file>`, e.g. the title) are applied here, to the
//!   Resources files inside the APK. Those files carry no type trees, so the Texture2D/Sprite
//!   layouts are taken from the player's own downloaded bundles (`--data`); every object decoded
//!   that way must consume exactly its bytes.

use crate::apk::{ApkEdit, Error};
use jdr_crypto::Jjec;
use jdr_services::art::{self, ArtPack};
use jdr_unity::{bundle, sfile, typetree::TNode};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

pub const ART_ASSET: &str = "assets/jdr/art.bin";
const CLASS_TEXTURE2D: i32 = 28;
const CLASS_SPRITE: i32 = 213;

/// `<bundle>/sprites/<name>.png` -> PNG bytes from a pack zip (a leading `.../replacements/` is
/// stripped; anything else in the zip is ignored).
pub fn read_pack_zip(path: &Path) -> Result<BTreeMap<String, Vec<u8>>, Error> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path)?).map_err(|e| e.to_string())?;
    let mut out = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().replace('\\', "/");
        // macOS zip debris
        if name.starts_with("__MACOSX/")
            || name.rsplit('/').next().is_some_and(|b| b.starts_with("._"))
        {
            continue;
        }
        let rel = match name.find("replacements/") {
            Some(p) => name[p + "replacements/".len()..].to_string(),
            None => continue,
        };
        if !art::valid_path(&rel) {
            continue;
        }
        let mut b = Vec::new();
        f.read_to_end(&mut b)?;
        if out.insert(rel.clone(), b).is_some() {
            return Err(format!("art pack: {rel} appears twice in the zip").into());
        }
    }
    if out.is_empty() {
        return Err("art pack: no replacements/<bundle>/sprites/*.png in the zip".into());
    }
    Ok(out)
}

/// Texture2D and Sprite type nodes from the player's downloaded bundles (same engine build).
/// The 16-byte old type hash of a (non-MonoBehaviour) type entry.
fn type_hash(raw: &[u8]) -> Option<&[u8]> {
    raw.get(7..23)
}

pub fn template_nodes(
    data_dir: &Path,
    key: &[u8],
    iv: &[u8],
    want_tex: &[u8],
    want_sprite: &[u8],
) -> Result<(Vec<TNode>, Vec<TNode>), Error> {
    let mut files: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(data_dir)
        .map_err(|e| format!("{}: {e}", data_dir.display()))?
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.len(), e.path())))
        .filter(|(len, p)| {
            *len < 4 << 20 && p.extension().and_then(|x| x.to_str()) == Some("bytes")
        })
        .collect();
    files.sort();
    for (_, p) in files.iter().take(2000) {
        let Ok(mut d) = std::fs::read(p) else {
            continue;
        };
        if Jjec::is_jjec(&d) {
            match Jjec::decrypt(&d, key, iv) {
                Ok(x) => d = x,
                Err(_) => continue,
            }
        }
        let Ok(nodes) = bundle::read(&d) else {
            continue;
        };
        for n in nodes.iter().filter(|n| n.flags & 4 != 0) {
            let Ok(f) = sfile::parse(&n.data) else {
                continue;
            };
            // Same class and same type hash as the APK's own entries: same layout.
            let find = |c: i32, want: &[u8]| {
                f.types
                    .iter()
                    .find(|t| {
                        t.entry.class_id == c
                            && !t.nodes.is_empty()
                            && type_hash(&t.entry.raw) == Some(want)
                    })
                    .map(|t| t.nodes.clone())
            };
            if let (Some(t), Some(s)) = (
                find(CLASS_TEXTURE2D, want_tex),
                find(CLASS_SPRITE, want_sprite),
            ) {
                return Ok((t, s));
            }
        }
    }
    Err("no downloaded bundle with Texture2D and Sprite type trees of the same build found in --data".into())
}

pub struct Applied {
    pub stored: usize,
    pub in_apk: usize,
    pub apk_files: usize,
}

/// Store the bundle images as `art.bin` and apply the in-APK ones.
pub fn apply(
    apk: &mut ApkEdit,
    pack: BTreeMap<String, Vec<u8>>,
    data_dir: Option<&Path>,
    keys_json: Option<&[u8]>,
) -> Result<Applied, Error> {
    let (in_apk, stored): (BTreeMap<_, _>, BTreeMap<_, _>) =
        pack.into_iter().partition(|(k, _)| k.starts_with("apk_"));
    let stored_pack = ArtPack::from_files(stored)?;
    let mut out = Applied {
        stored: stored_pack.len(),
        in_apk: 0,
        apk_files: 0,
    };
    let data_dir = data_dir.ok_or(
        "--art needs --data <your game files folder> (to check the pack against your bundles and read the title-screen layouts)",
    )?;
    let keys: serde_json::Value =
        serde_json::from_slice(keys_json.ok_or("--keys is required with --art")?)?;
    let key = keys["asset_key"]
        .as_str()
        .ok_or("keys.json: no asset_key")?;
    let iv = keys["asset_iv"].as_str().ok_or("keys.json: no asset_iv")?;
    // Dry run: every menu image must land on a sprite of the player's own bundles, so a pack
    // that does not fit is found here and not on the phone.
    for b in stored_pack.bundles() {
        let obf = format!("{}.bytes", jdr_crypto::names::obfuscate(&b, key.as_bytes()));
        let raw = std::fs::read(data_dir.join(&obf))
            .map_err(|e| format!("art pack: bundle {b} not in --data ({e})"))?;
        let plain = if Jjec::is_jjec(&raw) {
            Jjec::decrypt(&raw, key.as_bytes(), iv.as_bytes()).map_err(|e| format!("{b}: {e}"))?
        } else {
            raw
        };
        let mut nodes = bundle::read(&plain).map_err(|e| format!("{b}: {e}"))?;
        let r =
            art::apply(&mut nodes, &stored_pack.for_bundle(&b)).map_err(|e| format!("{b}: {e}"))?;
        if !r.unmatched.is_empty() || !r.errors.is_empty() {
            return Err(format!(
                "art pack does not fit bundle {b}: images match no sprite: {:?}; errors: {:?}",
                r.unmatched, r.errors
            )
            .into());
        }
    }
    if !stored_pack.is_empty() {
        apk.put(ART_ASSET, stored_pack.to_bin(), false);
    }
    if in_apk.is_empty() {
        return Ok(out);
    }
    let groups = ArtPack::from_files(in_apk)?;
    let mut templates: HashMap<(Vec<u8>, Vec<u8>), (Vec<TNode>, Vec<TNode>)> = HashMap::new();
    for b in groups.bundles() {
        let hash = b.strip_prefix("apk_").unwrap_or(&b);
        if hash.len() != 32 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("art pack: {b} does not name an APK Resources file").into());
        }
        let entry = format!("assets/bin/Data/{hash}");
        let raw = apk.read(&entry)?;
        let mut f = sfile::parse(&raw).map_err(|e| format!("{entry}: {e}"))?;
        let hash_of = |c: i32| {
            f.types
                .iter()
                .find(|t| t.entry.class_id == c)
                .and_then(|t| type_hash(&t.entry.raw))
                .map(<[u8]>::to_vec)
                .unwrap_or_default()
        };
        let want = (hash_of(CLASS_TEXTURE2D), hash_of(CLASS_SPRITE));
        if !templates.contains_key(&want) {
            let t = template_nodes(data_dir, key.as_bytes(), iv.as_bytes(), &want.0, &want.1)?;
            templates.insert(want.clone(), t);
        }
        let (tex_nodes, sprite_nodes) = &templates[&want];
        for t in f.types.iter_mut() {
            if t.nodes.is_empty() {
                match t.entry.class_id {
                    CLASS_TEXTURE2D => t.nodes = tex_nodes.clone(),
                    CLASS_SPRITE => t.nodes = sprite_nodes.clone(),
                    _ => {}
                }
            }
        }
        let images = groups.for_bundle(&b);
        let mut files = vec![Some(f)];
        let r = art::apply_files(&mut files, &HashMap::new(), &images);
        if !r.unmatched.is_empty() || !r.errors.is_empty() {
            return Err(format!(
                "{entry}: images match no sprite: {:?}; errors: {:?}",
                r.unmatched, r.errors
            )
            .into());
        }
        let n = r.replaced;
        let f = files.pop().flatten().expect("file");
        let compress = apk.is_compressed(&entry);
        apk.put(&entry, f.write(), compress);
        out.in_apk += n;
        out.apk_files += 1;
    }
    Ok(out)
}
