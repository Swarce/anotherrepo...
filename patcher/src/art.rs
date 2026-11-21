//! `ui-art`: the player's redrawn UI art.
//!
//! The pack (a zip as exported by `tools/ui-assets/export_ui.py` and redrawn by the player, with
//! `replacements/<bundle>/sprites/<sprite>.png`) is split in two:
//! - images for downloaded bundles go into the APK as a stored asset (`assets/jdr/art.bin`); the
//!   embedded server applies them when it rebuilds those bundles on the phone;
//! - images for the APK's own screens (`apk_<file>`, e.g. the title) are applied here, to the
//!   Resources files inside the APK. Those files carry no type trees, so the Texture2D/Sprite
//!   layouts are taken from the player's own downloaded bundles (`--data`), matched by the type
//!   entry's old type hash, one class at a time (a file may hold only a Texture2D, as the
//!   loading-screen sprite-atlas texture does: its sprites and atlas live in other files). Before
//!   anything is edited, every object of a filled type must decode through its tree consuming
//!   exactly its bytes and encode back to the same bytes; otherwise the file is left unchanged and
//!   reported.

use crate::apk::{ApkEdit, Error};
use jdr_crypto::Jjec;
use jdr_services::art::{self, ArtPack};
use jdr_unity::sfile::{self, V};
use jdr_unity::{bundle, typetree::TNode};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::Path;

pub const ART_ASSET: &str = "assets/jdr/art.bin";
const CLASS_TEXTURE2D: i32 = 28;
const CLASS_SPRITE: i32 = 213;

/// `<bundle>/sprites/<name>.png` (and whole textures, `<bundle>/<name>.png` stored as
/// `<bundle>/textures/<name>.png`) -> PNG bytes from a pack zip (a leading `.../replacements/` is
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
        let mut rel = match name.find("replacements/") {
            Some(p) => name[p + "replacements/".len()..].to_string(),
            None => continue,
        };
        // A whole texture sits directly in its bundle folder (`<bundle>/<texture>.png`).
        if let [bundle, file] = rel.split('/').collect::<Vec<_>>()[..] {
            if file.ends_with(".png") {
                rel = format!("{bundle}/textures/{file}");
            }
        }
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

/// A type entry the patcher fills in: (class id, old type hash).
pub type TypeKey = (i32, Vec<u8>);

/// The 16-byte old type hash of a (non-MonoBehaviour) type entry.
fn type_hash(raw: &[u8]) -> Option<&[u8]> {
    raw.get(7..23)
}

fn class_name(c: i32) -> &'static str {
    match c {
        CLASS_TEXTURE2D => "Texture2D",
        CLASS_SPRITE => "Sprite",
        _ => "?",
    }
}

/// Type trees for the given (class, old type hash) pairs, from the player's downloaded bundles
/// (the same old type hash means the same engine type layout). Each pair may come from a
/// different bundle. Fails unless every pair is found.
pub fn template_nodes(
    data_dir: &Path,
    key: &[u8],
    iv: &[u8],
    wants: &[TypeKey],
) -> Result<HashMap<TypeKey, Vec<TNode>>, Error> {
    let mut found: HashMap<TypeKey, Vec<TNode>> = HashMap::new();
    let all = |found: &HashMap<TypeKey, Vec<TNode>>| wants.iter().all(|w| found.contains_key(w));
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
        if all(&found) {
            break;
        }
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
            for t in f.types.iter().filter(|t| !t.nodes.is_empty()) {
                let Some(h) = type_hash(&t.entry.raw) else {
                    continue;
                };
                let k = (t.entry.class_id, h.to_vec());
                if wants.contains(&k) {
                    found.entry(k).or_insert_with(|| t.nodes.clone());
                }
            }
        }
    }
    if !all(&found) {
        let mut missing: Vec<&str> = wants
            .iter()
            .filter(|w| !found.contains_key(*w))
            .map(|w| class_name(w.0))
            .collect();
        missing.dedup();
        return Err(format!(
            "no downloaded bundle with {} type trees of the same build found in --data",
            missing.join(" and ")
        )
        .into());
    }
    Ok(found)
}

/// Fill the file's Texture2D and Sprite type entries that carry no tree from `templates`
/// (fetched from `--data` as needed), then prove the layout on this very file: every object of a
/// filled type must decode through it consuming exactly its bytes and encode back to the same
/// bytes. On error the file is left as it was (the caller reports it).
fn fill_types(
    f: &mut sfile::SFile,
    templates: &mut HashMap<TypeKey, Vec<TNode>>,
    data_dir: &Path,
    key: &[u8],
    iv: &[u8],
) -> Result<(), Error> {
    let mut wants: Vec<(usize, TypeKey)> = Vec::new();
    for (i, t) in f.types.iter().enumerate() {
        let c = t.entry.class_id;
        if t.nodes.is_empty() && (c == CLASS_TEXTURE2D || c == CLASS_SPRITE) {
            let h = type_hash(&t.entry.raw)
                .ok_or_else(|| format!("{} type entry without a type hash", class_name(c)))?;
            wants.push((i, (c, h.to_vec())));
        }
    }
    let mut missing: Vec<TypeKey> = wants
        .iter()
        .map(|(_, k)| k.clone())
        .filter(|k| !templates.contains_key(k))
        .collect();
    missing.sort();
    missing.dedup();
    if !missing.is_empty() {
        templates.extend(template_nodes(data_dir, key, iv, &missing)?);
    }
    let mut filled = f.clone();
    for (i, k) in &wants {
        filled.types[*i].nodes = templates[k].clone();
    }
    for o in &filled.objects {
        let ti = o.type_index as usize;
        if !wants.iter().any(|(i, _)| *i == ti) {
            continue;
        }
        let nodes = &filled.types[ti].nodes;
        let what = || {
            format!(
                "{} {} does not decode exactly through the type tree of the same type hash",
                class_name(filled.types[ti].entry.class_id),
                o.path_id
            )
        };
        let v = sfile::read_object(nodes, &o.data).map_err(|e| format!("{}: {e}", what()))?;
        if sfile::write_object(nodes, &v).map_err(|e| format!("{}: {e}", what()))? != o.data {
            return Err(what().into());
        }
    }
    *f = filled;
    Ok(())
}

/// The `.resS` streams this file's textures read their pixels from, read from the APK next to it
/// (`assets/bin/Data/<name>`), by the name the texture gives. A texture whose stream is not in the
/// APK makes the file one we cannot rewrite.
fn stream_files(f: &sfile::SFile, apk: &mut ApkEdit) -> Result<HashMap<String, Vec<u8>>, Error> {
    let mut out = HashMap::new();
    for o in &f.objects {
        let t = &f.types[o.type_index as usize];
        if t.entry.class_id != CLASS_TEXTURE2D || t.nodes.is_empty() {
            continue;
        }
        let v = sfile::read_object(&t.nodes, &o.data)?;
        let Some(sd) = v.get("m_StreamData") else {
            continue;
        };
        if sd.get("size").and_then(V::as_i64).unwrap_or(0) == 0 {
            continue;
        }
        let path = sd.get("path").and_then(V::as_str).unwrap_or("");
        let name = path.rsplit('/').next().unwrap_or(path);
        if name.is_empty() || name == "." || name == ".." || out.contains_key(name) {
            continue;
        }
        let entry = format!("assets/bin/Data/{name}");
        if !apk.exists(&entry) {
            return Err(format!(
                "texture {} reads its pixels from {path:?}, not in the APK",
                o.path_id
            )
            .into());
        }
        out.insert(name.to_string(), apk.read(&entry)?);
    }
    Ok(out)
}

pub struct Applied {
    pub stored: usize,
    pub in_apk: usize,
    pub apk_files: usize,
    /// In-APK files left unchanged, with the reason (no matching type layout to write with).
    pub skipped: Vec<String>,
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
        skipped: Vec::new(),
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
    let mut templates: HashMap<TypeKey, Vec<TNode>> = HashMap::new();
    for b in groups.bundles() {
        let hash = b.strip_prefix("apk_").unwrap_or(&b);
        if hash.len() != 32 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("art pack: {b} does not name an APK Resources file").into());
        }
        let entry = format!("assets/bin/Data/{hash}");
        let raw = apk.read(&entry)?;
        let mut f = sfile::parse(&raw).map_err(|e| format!("{entry}: {e}"))?;
        // Without a proven type layout for this file's Texture2D/Sprite (no downloaded bundle of
        // the same build has it, or an object does not round-trip through it), or with pixels in
        // a stream the APK lacks, the file cannot be rewritten safely: leave it as it is.
        if let Err(e) = fill_types(
            &mut f,
            &mut templates,
            data_dir,
            key.as_bytes(),
            iv.as_bytes(),
        ) {
            out.skipped.push(format!("{b}: {e}"));
            continue;
        }
        let streams = match stream_files(&f, apk) {
            Ok(s) => s,
            Err(e) => {
                out.skipped.push(format!("{b}: {e}"));
                continue;
            }
        };
        let res: HashMap<&str, &[u8]> = streams
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();
        let images = groups.for_bundle(&b);
        let mut files = vec![Some(f)];
        let r = art::apply_files(&mut files, &res, &images);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_asked_for_per_class() {
        // Only the classes asked for are required (a file with only a Texture2D needs no Sprite
        // tree), and a missing one is named in the reason the file is left unchanged.
        let dir = std::env::temp_dir().join(format!("jdr-art-templates-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(template_nodes(&dir, b"k", b"iv", &[]).unwrap().is_empty());
        let e = template_nodes(&dir, b"k", b"iv", &[(CLASS_TEXTURE2D, vec![0; 16])])
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("no downloaded bundle with Texture2D type trees"),
            "{e}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
