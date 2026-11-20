//! Redrawn UI art: replace sprites in the game's own UI bundles with the player's images.
//!
//! The art pack is the player's own (made from their export with `tools/ui-assets/export_ui.py`);
//! it is never in the repo. Layout, as exported: `<bundle>/sprites/<sprite>.png`, each PNG the exact
//! size of its sprite rectangle in the atlas. Each touched atlas is decoded, the sprites are pasted
//! in at their rectangles, and the texture is stored as uncompressed RGBA32 (no lossy
//! re-compression). The rest of the bundle is untouched.
//!
//! The patcher stores the pack in the APK (`assets/jdr/art.bin`, see [`ArtPack::to_bin`]); the
//! server applies it while it rebuilds the English bundles (same originals, cache and manifest).
//!
//! A pack can also add statue thumbnails the player's files have none for
//! (`ui_statue_01/thumbs/st_<statueId>.png`, made by `tools/content/render_portraits.py`): see
//! [`add_thumbnails`] and docs/research/28-portraits.md.

use jdr_unity::bundle::Node;
use jdr_unity::sfile::{self, V};
use jdr_unity::texture;
use std::collections::{BTreeMap, HashMap};

type Error = String;

pub const MAGIC: &[u8; 8] = b"JDRART1\n";
/// Bump when the way art is applied changes, so phones rebuild their cached files.
pub const ART_GEN_VERSION: u32 = 3;
/// Unity class ids.
const CLASS_TEXTURE2D: i32 = 28;
const CLASS_TEXTASSET: i32 = 49;
const CLASS_ASSETBUNDLE: i32 = 142;
const CLASS_SPRITE: i32 = 213;
/// Caps for a pack read from the APK.
const MAX_FILES: usize = 20_000;
const MAX_TOTAL: usize = 512 << 20;

#[derive(Default, Debug, Clone)]
pub struct ArtPack {
    /// `<bundle>/sprites/<file>.png` -> PNG bytes.
    files: BTreeMap<String, Vec<u8>>,
    pub digest: String,
}

fn digest_of(files: &BTreeMap<String, Vec<u8>>) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(ART_GEN_VERSION.to_le_bytes());
    // the build too: how art is applied can change with the code (a probe build did not rebuild
    // its bundles because the images had not changed)
    h.update(jdr_core::BUILD.to_le_bytes());
    for (k, v) in files {
        h.update((k.len() as u32).to_le_bytes());
        h.update(k.as_bytes());
        h.update((v.len() as u64).to_le_bytes());
        h.update(v);
    }
    h.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A pack path is `<bundle>/sprites/<name>.png` (a sprite) or `<bundle>/textures/<name>.png` (a
/// whole texture, as the exporter wrote it at `<bundle>/<name>.png`), with plain components only.
pub fn valid_path(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    parts.len() == 3
        && match parts[1] {
            "sprites" | "textures" => true,
            "thumbs" => is_thumb_file(parts[2]),
            _ => false,
        }
        && parts[2].ends_with(".png")
        && parts
            .iter()
            .all(|c| !c.is_empty() && *c != "." && *c != ".." && !c.contains('\\'))
}

/// `st_<4 digits>_<2 digits>.png`, a statue thumbnail's file name.
fn is_thumb_file(f: &str) -> bool {
    let b = f.as_bytes();
    b.len() == 14
        && f.starts_with("st_")
        && f.ends_with(".png")
        && b[3..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'_'
        && b[8..10].iter().all(u8::is_ascii_digit)
}

impl ArtPack {
    pub fn from_files(files: BTreeMap<String, Vec<u8>>) -> Result<Self, Error> {
        if let Some(bad) = files.keys().find(|k| !valid_path(k)) {
            return Err(format!(
                "art pack: unexpected path {bad:?} (want <bundle>/sprites/<name>.png, <bundle>/textures/<name>.png or <bundle>/thumbs/st_<id>.png)"
            ));
        }
        let digest = digest_of(&files);
        Ok(Self { files, digest })
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// A `replacements/` folder as delivered (`<dir>/<bundle>/sprites/*.png`).
    pub fn from_dir(dir: &std::path::Path) -> Result<Self, Error> {
        let mut files = BTreeMap::new();
        let rd =
            |p: &std::path::Path| std::fs::read_dir(p).map_err(|e| format!("{}: {e}", p.display()));
        for b in rd(dir)?.flatten() {
            // Whole textures sit directly in the bundle folder, as the exporter wrote them.
            if b.path().is_dir() {
                for f in rd(&b.path())?.flatten() {
                    let name = f.file_name().to_string_lossy().into_owned();
                    if name.ends_with(".png") && f.path().is_file() {
                        let key = format!("{}/textures/{name}", b.file_name().to_string_lossy());
                        files.insert(key, std::fs::read(f.path()).map_err(|e| e.to_string())?);
                    }
                }
            }
            for kind in ["sprites", "thumbs"] {
                let sub = b.path().join(kind);
                if !sub.is_dir() {
                    continue;
                }
                for f in rd(&sub)?.flatten() {
                    let name = f.file_name().to_string_lossy().into_owned();
                    if name.ends_with(".png") {
                        let key = format!("{}/{kind}/{name}", b.file_name().to_string_lossy());
                        files.insert(key, std::fs::read(f.path()).map_err(|e| e.to_string())?);
                    }
                }
            }
        }
        Self::from_files(files)
    }

    /// `JDRART1\n`, u32 count, then per file: u32 path length, path, u32 data length, data.
    pub fn to_bin(&self) -> Vec<u8> {
        let mut w = MAGIC.to_vec();
        w.extend_from_slice(&(self.files.len() as u32).to_le_bytes());
        for (k, v) in &self.files {
            w.extend_from_slice(&(k.len() as u32).to_le_bytes());
            w.extend_from_slice(k.as_bytes());
            w.extend_from_slice(&(v.len() as u32).to_le_bytes());
            w.extend_from_slice(v);
        }
        w
    }

    pub fn from_bin(b: &[u8]) -> Result<Self, Error> {
        let bad = || "art pack: truncated or corrupt".to_string();
        if b.get(..8) != Some(&MAGIC[..]) {
            return Err("art pack: bad magic".into());
        }
        let mut o = 8;
        let u32_at = |o: &mut usize| -> Result<usize, Error> {
            let v = b.get(*o..*o + 4).ok_or_else(bad)?;
            *o += 4;
            Ok(u32::from_le_bytes(v.try_into().unwrap()) as usize)
        };
        let n = u32_at(&mut o)?;
        if n > MAX_FILES || b.len() > MAX_TOTAL {
            return Err("art pack too large".into());
        }
        let mut files = BTreeMap::new();
        for _ in 0..n {
            let kl = u32_at(&mut o)?;
            let k = std::str::from_utf8(b.get(o..o + kl).ok_or_else(bad)?).map_err(|_| bad())?;
            o += kl;
            let vl = u32_at(&mut o)?;
            let v = b.get(o..o + vl).ok_or_else(bad)?.to_vec();
            o += vl;
            files.insert(k.to_string(), v);
        }
        Self::from_files(files)
    }

    /// Bundle names with at least one image.
    pub fn bundles(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .files
            .keys()
            .filter_map(|k| k.split('/').next().map(str::to_owned))
            .collect();
        v.dedup();
        v
    }

    /// `<file>.png` (sprites), `textures/<file>.png` (whole textures) and `thumbs/<file>.png`
    /// (new thumbnails) -> PNG bytes for one bundle.
    pub fn for_bundle(&self, bundle: &str) -> HashMap<String, &[u8]> {
        let sprites = format!("{bundle}/sprites/");
        let textures = format!("{bundle}/textures/");
        let thumbs = format!("{bundle}/thumbs/");
        self.files
            .iter()
            .filter_map(|(k, v)| {
                let key = if let Some(f) = k.strip_prefix(&sprites) {
                    f.to_string()
                } else if let Some(f) = k.strip_prefix(&textures) {
                    format!("textures/{f}")
                } else {
                    format!("thumbs/{}", k.strip_prefix(&thumbs)?)
                };
                Some((key, v.as_slice()))
            })
            .collect()
    }

    /// How many new thumbnails the pack adds.
    pub fn thumbs(&self) -> usize {
        self.files.keys().filter(|k| k.contains("/thumbs/")).count()
    }
}

/// The exporter's file-name rule (`export_ui.py::safe`).
pub fn safe_name(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_.-[]() ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    if t.is_empty() {
        "_".into()
    } else {
        t
    }
}

fn num(v: Option<&V>) -> Option<f64> {
    match v? {
        V::F32(f) => Some(*f as f64),
        V::F64(f) => Some(*f),
        V::I(i) => Some(*i as f64),
        V::U(u) => Some(*u as f64),
        _ => None,
    }
}

struct SpriteRef {
    file_index: usize,
    texture: i64,
    /// textureRect as stored (x, y, width, height; y up, fractional).
    rect: (f64, f64, f64, f64),
    file: String,
    png: Vec<u8>,
}

/// What applying a pack to one bundle did.
#[derive(Debug, Default)]
pub struct Applied {
    pub replaced: usize,
    /// New thumbnails added.
    pub added: usize,
    /// Indices of the serialized files that changed.
    pub changed: std::collections::HashSet<usize>,
    /// Images that match no sprite.
    pub unmatched: Vec<String>,
    /// Sprites or textures skipped (the rest of the bundle, its text included, still applies).
    pub errors: Vec<String>,
}

/// Asset key and IV (keys.json `asset_key`, `asset_iv`): new thumbnails are encrypted with them.
pub type Keys<'a> = Option<(&'a [u8], &'a [u8])>;

/// Replace sprites in a decoded bundle's nodes, and add new thumbnails (`thumbs/...`, which need
/// `keys`).
pub fn apply(
    nodes: &mut [Node],
    images: &HashMap<String, &[u8]>,
    keys: Keys,
) -> Result<Applied, Error> {
    if images.is_empty() {
        return Ok(Applied::default());
    }
    type Images<'a> = HashMap<String, &'a [u8]>;
    let (thumbs, sprites): (Images, Images) = images
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .partition(|(k, _)| k.starts_with("thumbs/"));
    let mut files = Vec::new();
    for n in nodes.iter() {
        files.push(if n.flags & 4 != 0 {
            Some(sfile::parse(&n.data)?)
        } else {
            None
        });
    }
    let mut r = if sprites.is_empty() {
        Applied::default()
    } else {
        let res: HashMap<&str, &[u8]> = nodes
            .iter()
            .filter(|n| n.flags & 4 == 0)
            .map(|n| (n.name.as_str(), n.data.as_slice()))
            .collect();
        apply_files(&mut files, &res, &sprites)
    };
    if !thumbs.is_empty() {
        match keys {
            Some((key, iv)) => add_thumbnails(&mut files, &thumbs, key, iv, &mut r),
            None => r.errors.push(format!(
                "{} new thumbnails need the asset key",
                thumbs.len()
            )),
        }
    }
    for (i, (n, f)) in nodes.iter_mut().zip(files).enumerate() {
        if let (true, Some(f)) = (r.changed.contains(&i), f) {
            n.data = f.write();
        }
    }
    Ok(r)
}

/// Add statue thumbnails (`thumbs/st_<id>.png`, 128x128) to a thumbnail bundle (`ui_statue_*`).
/// Each thumbnail there is a TextAsset `st_<id>` holding an encrypted bundle of its own (a
/// Texture2D, a Sprite and an AssetBundle); the client finds it by the container key
/// `.../<bundle>/st_<id>.bytes` (`ResourceManager` registers every container key of a loaded
/// bundle). A new one is cloned from the first thumbnail already there: renamed, its pixels stored
/// as RGBA32, its serialized file given a name of its own, encrypted, and listed in the container
/// and the preload table. Thumbnails the bundle already has are left alone.
fn add_thumbnails(
    files: &mut [Option<sfile::SFile>],
    thumbs: &HashMap<String, &[u8]>,
    key: &[u8],
    iv: &[u8],
    out: &mut Applied,
) {
    let Some(fi) = files.iter().position(|f| {
        f.as_ref().is_some_and(|f| {
            f.objects
                .iter()
                .any(|o| class_of(f, o) == CLASS_ASSETBUNDLE)
        })
    }) else {
        out.errors.push("thumbnails: no AssetBundle object".into());
        return;
    };
    let f = files[fi].as_mut().expect("serialized file");
    let r = (|| -> Result<usize, Error> {
        let ab_i = f
            .objects
            .iter()
            .position(|o| class_of(f, o) == CLASS_ASSETBUNDLE)
            .ok_or("no AssetBundle")?;
        // The template: the thumbnail whose sprite covers the most of its texture. Thumbnail
        // sprites are tight meshes around their own outline, and a new portrait must not be
        // clipped to another frame's: a full 128x128 quad (4 vertices) draws all of it.
        let mut best: Option<(f64, i64, usize, V, Vec<u8>)> = None;
        for (i, o) in f.objects.iter().enumerate() {
            if class_of(f, o) != CLASS_TEXTASSET {
                continue;
            }
            let t = &f.types[o.type_index as usize];
            let Ok(v) = sfile::read_object(&t.nodes, &o.data) else {
                continue;
            };
            let script = match v.get("m_Script") {
                Some(V::Str(b)) | Some(V::Bytes(b)) => b.clone(),
                _ => continue,
            };
            let inner = if jdr_crypto::Jjec::is_jjec(&script) {
                match jdr_crypto::Jjec::decrypt(&script, key, iv) {
                    Ok(b) => b,
                    Err(_) => continue,
                }
            } else {
                script
            };
            let Some((area, verts)) = sprite_cover(&inner) else {
                continue;
            };
            let better = match &best {
                None => true,
                Some(b) => (area, -verts) > (b.0, -b.1),
            };
            if better {
                best = Some((area, verts, i, v, inner));
            }
        }
        let (_, _, tpl_i, tpl, tpl_inner) = best.ok_or("no thumbnail to clone")?;
        let ab_t = f.objects[ab_i].type_index as usize;
        let tx_t = f.objects[tpl_i].type_index as usize;
        let mut ab = sfile::read_object(&f.types[ab_t].nodes, &f.objects[ab_i].data)?;
        let tpl_name = tpl
            .get("m_Name")
            .and_then(V::as_str)
            .ok_or("template name")?
            .to_string();
        let (prefix, mut keys) = {
            let Some(V::Arr(items)) = ab.get("m_Container") else {
                return Err("no m_Container".into());
            };
            let keys: Vec<Vec<u8>> = items
                .iter()
                .filter_map(|e| match e.get("first") {
                    Some(V::Str(k)) => Some(k.clone()),
                    _ => None,
                })
                .collect();
            let first =
                String::from_utf8_lossy(keys.first().ok_or("empty container")?).into_owned();
            (
                first[..first.rfind('/').ok_or("container key without folder")? + 1].to_string(),
                keys,
            )
        };
        let mut used: std::collections::HashSet<i64> =
            f.objects.iter().map(|o| o.path_id).collect();
        let mut added = 0;
        let mut names: Vec<&String> = thumbs.keys().collect();
        names.sort();
        for file in names {
            let name = file.trim_start_matches("thumbs/").trim_end_matches(".png");
            let ckey = format!("{prefix}{name}.bytes").into_bytes();
            if keys.contains(&ckey) {
                out.errors
                    .push(format!("{name}: the bundle already has it"));
                continue;
            }
            let built =
                make_thumbnail(&tpl_inner, &tpl_name, name, thumbs[file]).and_then(|inner| {
                    jdr_crypto::Jjec::encrypt(&inner, key, iv, inner.len())
                        .map_err(|e| e.to_string())
                });
            let enc = match built {
                Ok(b) => b,
                Err(e) => {
                    out.errors.push(format!("{name}: {e}"));
                    continue;
                }
            };
            let mut pid = path_id_for(name);
            while used.contains(&pid) {
                pid = pid.wrapping_add(1);
            }
            used.insert(pid);
            let mut tv = tpl.clone();
            set(&mut tv, "m_Name", V::Str(name.as_bytes().to_vec()));
            set(&mut tv, "m_Script", V::Str(enc));
            f.objects.push(jdr_unity::serialized::Object {
                path_id: pid,
                type_index: tx_t as i32,
                data: sfile::write_object(&f.types[tx_t].nodes, &tv)?,
            });
            f.layout.push(f.objects.len() - 1);
            // preload table and container entry (kept sorted by key, as the engine writes it)
            let pre_len = match ab.get_mut("m_PreloadTable") {
                Some(V::Arr(pre)) => {
                    let mut e = pre.first().cloned().ok_or("empty preload table")?;
                    set(&mut e, "m_FileID", V::I(0));
                    set(&mut e, "m_PathID", V::I(pid));
                    pre.push(e);
                    pre.len() - 1
                }
                _ => return Err("no m_PreloadTable".into()),
            };
            let Some(V::Arr(items)) = ab.get_mut("m_Container") else {
                return Err("no m_Container".into());
            };
            let mut e = items.first().cloned().ok_or("empty container")?;
            set(&mut e, "first", V::Str(ckey.clone()));
            if let Some(sec) = e.get_mut("second") {
                set(sec, "preloadIndex", V::I(pre_len as i64));
                set(sec, "preloadSize", V::I(1));
                if let Some(a) = sec.get_mut("asset") {
                    set(a, "m_FileID", V::I(0));
                    set(a, "m_PathID", V::I(pid));
                }
            }
            let at = keys.partition_point(|k| k < &ckey);
            keys.insert(at, ckey);
            items.insert(at, e);
            added += 1;
        }
        if added > 0 {
            f.objects[ab_i].data = sfile::write_object(&f.types[ab_t].nodes, &ab)?;
            // the engine finds objects by binary search over path ids
            f.sort_objects();
        }
        Ok(added)
    })();
    match r {
        Ok(n) if n > 0 => {
            out.added += n;
            out.changed.insert(fi);
        }
        Ok(_) => {}
        Err(e) => out.errors.push(format!("thumbnails: {e}")),
    }
}

/// A thumbnail bundle's sprite: the area of its texture rectangle and its mesh's vertex count.
fn sprite_cover(inner: &[u8]) -> Option<(f64, i64)> {
    let nodes = jdr_unity::bundle::read(inner).ok()?;
    let f = sfile::parse(&nodes.iter().find(|n| n.flags & 4 != 0)?.data).ok()?;
    let o = f.objects.iter().find(|o| class_of(&f, o) == CLASS_SPRITE)?;
    let v = sfile::read_object(&f.types[o.type_index as usize].nodes, &o.data).ok()?;
    let rd = v.get("m_RD")?;
    let r = rd.get("textureRect")?;
    let area = num(r.get("width"))? * num(r.get("height"))?;
    let verts = rd
        .get("m_VertexData")
        .and_then(|d| d.get("m_VertexCount"))
        .and_then(V::as_i64)
        .unwrap_or(i64::MAX);
    Some((area, verts))
}

fn class_of(f: &sfile::SFile, o: &jdr_unity::serialized::Object) -> i32 {
    f.types[o.type_index as usize].entry.class_id
}

/// Set a field that exists, keeping its integer kind (signed or unsigned) as stored.
fn set(v: &mut V, k: &str, x: V) {
    if let Some(slot) = v.get_mut(k) {
        *slot = match (&*slot, x) {
            (V::U(_), V::I(i)) => V::U(i as u64),
            (_, x) => x,
        };
    }
}

/// A stable path id for a new object (the name's SHA-256, never 0).
fn path_id_for(name: &str) -> i64 {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(format!("jdr-thumb:{name}").as_bytes());
    let v = i64::from_le_bytes(h[..8].try_into().expect("8 bytes"));
    if v == 0 {
        1
    } else {
        v
    }
}

/// One thumbnail's own bundle: `tpl` (a thumbnail named `old`) renamed `new`, with `png` as its
/// pixels (stored RGBA32 in the serialized file, so the `.resS` stream goes) and its serialized
/// file renamed `CAB-<md5 of new>` (the engine refuses two loaded bundles with the same file).
pub fn make_thumbnail(tpl: &[u8], old: &str, new: &str, png: &[u8]) -> Result<Vec<u8>, Error> {
    use md5::{Digest, Md5};
    let mut nodes = jdr_unity::bundle::read(tpl)?;
    let si = nodes
        .iter()
        .position(|n| n.flags & 4 != 0)
        .ok_or("no serialized file")?;
    let mut f = sfile::parse(&nodes[si].data)?;
    let (iw, ih, img) = texture::decode_png(png)?;
    let rename =
        |s: &[u8]| -> Vec<u8> { String::from_utf8_lossy(s).replace(old, new).into_bytes() };
    let mut textures = 0;
    for i in 0..f.objects.len() {
        let t = f.objects[i].type_index as usize;
        let class = f.types[t].entry.class_id;
        if ![CLASS_TEXTURE2D, CLASS_SPRITE, CLASS_ASSETBUNDLE].contains(&class) {
            continue;
        }
        let mut v = sfile::read_object(&f.types[t].nodes, &f.objects[i].data)?;
        set(&mut v, "m_Name", V::Str(new.as_bytes().to_vec()));
        match class {
            CLASS_TEXTURE2D => {
                let w = v.get("m_Width").and_then(V::as_i64).ok_or("no m_Width")? as usize;
                let h = v.get("m_Height").and_then(V::as_i64).ok_or("no m_Height")? as usize;
                if (iw, ih) != (w, h) {
                    return Err(format!("image is {iw}x{ih}, thumbnails are {w}x{h}"));
                }
                let mut px = vec![0u8; w * h * 4];
                texture::paste(&mut px, w, h, &img, iw, ih, 0, 0)?;
                set(&mut v, "m_TextureFormat", V::I(texture::FMT_RGBA32));
                set(&mut v, "m_CompleteImageSize", V::I((w * h * 4) as i64));
                set(&mut v, "m_MipCount", V::I(1));
                set(&mut v, "m_MipMap", V::U(0));
                set(&mut v, "image data", V::Bytes(px));
                if let Some(sd) = v.get_mut("m_StreamData") {
                    set(sd, "offset", V::U(0));
                    set(sd, "size", V::U(0));
                    set(sd, "path", V::Str(Vec::new()));
                }
                textures += 1;
            }
            CLASS_ASSETBUNDLE => {
                set(&mut v, "m_AssetBundleName", V::Str(new.as_bytes().to_vec()));
                if let Some(V::Arr(items)) = v.get_mut("m_Container") {
                    for e in items {
                        if let Some(V::Str(k)) = e.get_mut("first") {
                            *k = rename(k);
                        }
                    }
                }
            }
            _ => {}
        }
        f.objects[i].data = sfile::write_object(&f.types[t].nodes, &v)?;
    }
    if textures != 1 {
        return Err(format!("template has {textures} textures, expected 1"));
    }
    let cab: String = Md5::digest(new.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    nodes[si].data = f.write();
    nodes[si].name = format!("CAB-{cab}");
    // the pixels now live in the serialized file: the old stream file goes
    nodes.retain(|n| n.flags & 4 != 0 || !n.name.ends_with(".resS"));
    Ok(jdr_unity::bundle::write_lz4(
        &nodes,
        jdr_unity::UNITY_VERSION,
    ))
}

/// The sprite's pixel box exactly as the exporter cut it (`export_ui.py`): edges rounded with
/// ties-to-even, measured from the top. Returns (left, top, width, height), or None if empty.
pub fn crop_box(rect: (f64, f64, f64, f64), tex_h: usize) -> Option<(usize, usize, usize, usize)> {
    let (x, y, w, h) = rect;
    let th = tex_h as f64;
    let (x0, x1) = (x.round_ties_even(), (x + w).round_ties_even());
    let (top, bot) = ((th - y - h).round_ties_even(), (th - y).round_ties_even());
    (x1 > x0 && bot > top && x0 >= 0.0 && top >= 0.0).then_some((
        x0 as usize,
        top as usize,
        (x1 - x0) as usize,
        (bot - top) as usize,
    ))
}

fn find_sprite(v: &V, file: &str, fi: usize, png: &[u8]) -> Result<SpriteRef, Error> {
    let rd = v.get("m_RD").ok_or("sprite without m_RD")?;
    // A separate alpha texture (ETC1 split alpha) or a rotated packing would need more than a
    // paste into the main texture: refuse rather than draw something wrong.
    if rd
        .get("alphaTexture")
        .and_then(|t| t.get("m_PathID"))
        .and_then(V::as_i64)
        .unwrap_or(0)
        != 0
    {
        return Err(format!(
            "{file}: sprite uses a separate alpha texture (not supported)"
        ));
    }
    let settings = rd.get("settingsRaw").and_then(V::as_i64).unwrap_or(0);
    if (settings >> 2) & 0xF != 0 {
        return Err(format!(
            "{file}: sprite is stored rotated in its atlas (not supported)"
        ));
    }
    let tex = rd.get("texture").ok_or("sprite without texture")?;
    if tex.get("m_FileID").and_then(V::as_i64) != Some(0) {
        return Err(format!("{file}: atlas texture is in another file"));
    }
    let pid = tex
        .get("m_PathID")
        .and_then(V::as_i64)
        .ok_or("bad texture ref")?;
    let r = rd.get("textureRect").ok_or("sprite without textureRect")?;
    let f = |k: &str| num(r.get(k)).ok_or_else(|| format!("{file}: bad rect"));
    Ok(SpriteRef {
        file_index: fi,
        texture: pid,
        rect: (f("x")?, f("y")?, f("width")?, f("height")?),
        file: file.to_string(),
        png: png.to_vec(),
    })
}

/// Decode a texture, paste its sprites, store it as RGBA32. Returns the new object data.
fn repaint(
    f: &sfile::SFile,
    pid: i64,
    sprites: &[&SpriteRef],
    res: &HashMap<&str, &[u8]>,
) -> Result<(usize, Vec<u8>), Error> {
    let oi = f
        .objects
        .iter()
        .position(|o| o.path_id == pid)
        .ok_or_else(|| format!("texture {pid} not found"))?;
    let t = &f.types[f.objects[oi].type_index as usize];
    if t.entry.class_id != CLASS_TEXTURE2D {
        return Err(format!("object {pid} is not a Texture2D"));
    }
    let mut v = sfile::read_object(&t.nodes, &f.objects[oi].data)?;
    let w = v.get("m_Width").and_then(V::as_i64).ok_or("no m_Width")? as usize;
    let h = v.get("m_Height").and_then(V::as_i64).ok_or("no m_Height")? as usize;
    let fmt = v
        .get("m_TextureFormat")
        .and_then(V::as_i64)
        .ok_or("no format")?;
    let sd = v.get("m_StreamData");
    let sd_size = sd
        .and_then(|s| s.get("size"))
        .and_then(V::as_i64)
        .unwrap_or(0) as usize;
    let mut px = if sd_size > 0 {
        let sd = sd.expect("stream data");
        let off = sd.get("offset").and_then(V::as_i64).unwrap_or(0) as usize;
        let path = sd.get("path").and_then(V::as_str).unwrap_or("");
        let node = path.rsplit('/').next().unwrap_or(path);
        let data = res
            .get(node)
            .and_then(|b| b.get(off..off + sd_size))
            .ok_or_else(|| format!("texture data {path} not in the bundle"))?;
        texture::decode(fmt, w, h, data)?
    } else {
        match v.get("image data") {
            Some(V::Bytes(b)) => texture::decode(fmt, w, h, b)?,
            _ => return Err("texture without image data".into()),
        }
    };
    for s in sprites {
        let (iw, ih, img) = texture::decode_png(&s.png).map_err(|e| format!("{}: {e}", s.file))?;
        let (x, top, rw, rh) =
            crop_box(s.rect, h).ok_or_else(|| format!("{}: empty rect", s.file))?;
        if (iw, ih) != (rw, rh) {
            return Err(format!(
                "{}: image is {iw}x{ih} but its sprite is {rw}x{rh} (keep the exported size)",
                s.file
            ));
        }
        // `top` counts from the top (as in the exported PNG); the texture's rows count from the bottom.
        let y = h
            .checked_sub(top + rh)
            .ok_or_else(|| format!("{}: rect outside the texture", s.file))?;
        texture::paste(&mut px, w, h, &img, iw, ih, x, y)
            .map_err(|e| format!("{}: {e}", s.file))?;
    }
    let set = |v: &mut V, k: &str, x: V| {
        if let Some(f) = v.get_mut(k) {
            *f = x;
        }
    };
    set(&mut v, "m_TextureFormat", V::I(texture::FMT_RGBA32));
    set(&mut v, "m_CompleteImageSize", V::I((w * h * 4) as i64));
    set(&mut v, "m_MipCount", V::I(1));
    set(&mut v, "m_MipMap", V::U(0));
    set(&mut v, "image data", V::Bytes(px));
    if let Some(sd) = v.get_mut("m_StreamData") {
        set(sd, "offset", V::U(0));
        set(sd, "size", V::U(0));
        set(sd, "path", V::Str(Vec::new()));
    }
    Ok((oi, sfile::write_object(&t.nodes, &v)?))
}

/// The core of [`apply`] on parsed files (`None` = not a serialized file). Files without type trees
/// (in-APK Resources files) must have their Texture2D/Sprite type nodes filled in by the caller.
/// A sprite or texture that cannot be applied is skipped and reported; the rest still applies.
pub fn apply_files(
    files: &mut [Option<sfile::SFile>],
    res: &HashMap<&str, &[u8]>,
    images: &HashMap<String, &[u8]>,
) -> Applied {
    let mut out = Applied::default();
    // Pass 1: find the sprites (file name -> texture + rectangle), as the exporter named them.
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut found: Vec<SpriteRef> = Vec::new();
    let mut used: std::collections::HashSet<String> = Default::default();
    // Textures the exporter wrote (non-empty, not the font texture): path id -> height. Only
    // sprites cut from those were exported, and only they take part in its naming.
    let mut exported: HashMap<i64, usize> = HashMap::new();
    let mut tex_seen: HashMap<String, usize> = HashMap::new();
    for (fi, f) in files.iter().enumerate() {
        let Some(f) = f else { continue };
        for o in &f.objects {
            let t = &f.types[o.type_index as usize];
            if t.entry.class_id != CLASS_TEXTURE2D {
                continue;
            }
            let Ok(v) = sfile::read_object(&t.nodes, &o.data) else {
                continue;
            };
            let w = v.get("m_Width").and_then(V::as_i64).unwrap_or(0);
            let h = v.get("m_Height").and_then(V::as_i64).unwrap_or(0);
            let name = v.get("m_Name").and_then(V::as_str).unwrap_or("");
            if w > 0 && h > 0 && name != "Font Texture" {
                exported.insert(o.path_id, h as usize);
                // A whole-texture image: named as the exporter named the texture's PNG.
                let base = safe_name(name);
                let first = *tex_seen
                    .entry(base.clone())
                    .and_modify(|n| *n += 1)
                    .or_insert(1)
                    == 1;
                let mut candidates = vec![format!("textures/{base}__{}.png", o.path_id)];
                if first {
                    candidates.insert(0, format!("textures/{base}.png"));
                }
                if let Some(file) = candidates.into_iter().find(|c| images.contains_key(c)) {
                    used.insert(file.clone());
                    found.push(SpriteRef {
                        file_index: fi,
                        texture: o.path_id,
                        rect: (0.0, 0.0, w as f64, h as f64),
                        file: file.clone(),
                        png: images[&file].to_vec(),
                    });
                }
            }
        }
    }
    for (fi, f) in files.iter().enumerate() {
        let Some(f) = f else { continue };
        for o in &f.objects {
            let t = &f.types[o.type_index as usize];
            if t.entry.class_id != CLASS_SPRITE {
                continue;
            }
            let Ok(v) = sfile::read_object(&t.nodes, &o.data) else {
                continue;
            };
            let Some(name) = v.get("m_Name").and_then(V::as_str) else {
                continue;
            };
            let rd = v.get("m_RD");
            let tex_pid = rd
                .and_then(|r| r.get("texture"))
                .and_then(|t| t.get("m_PathID"))
                .and_then(V::as_i64)
                .unwrap_or(0);
            let rect = rd.and_then(|r| r.get("textureRect")).and_then(|r| {
                Some((
                    num(r.get("x"))?,
                    num(r.get("y"))?,
                    num(r.get("width"))?,
                    num(r.get("height"))?,
                ))
            });
            let was_exported = match (exported.get(&tex_pid), rect) {
                (Some(&th), Some(r)) => crop_box(r, th).is_some(),
                _ => false,
            };
            if !was_exported {
                continue;
            }
            let base = safe_name(name);
            let first = *seen
                .entry(base.clone())
                .and_modify(|n| *n += 1)
                .or_insert(1)
                == 1;
            let candidates = if first {
                vec![format!("{base}.png"), format!("{base}__{}.png", o.path_id)]
            } else {
                vec![format!("{base}__{}.png", o.path_id)]
            };
            let Some(file) = candidates.into_iter().find(|c| images.contains_key(c)) else {
                continue;
            };
            used.insert(file.clone());
            match find_sprite(&v, &file, fi, images[&file]) {
                Ok(s) => found.push(s),
                Err(e) => out.errors.push(e),
            }
        }
    }
    out.unmatched = images
        .keys()
        .filter(|k| !used.contains(*k))
        .cloned()
        .collect();
    // Pass 2: per texture, decode, paste, store as RGBA32.
    let mut by_tex: BTreeMap<(usize, i64), Vec<&SpriteRef>> = BTreeMap::new();
    for s in &found {
        by_tex.entry((s.file_index, s.texture)).or_default().push(s);
    }
    for ((fi, pid), sprites) in by_tex {
        let f = files[fi].as_mut().expect("serialized file");
        match repaint(f, pid, &sprites, res) {
            Ok((oi, data)) => {
                f.objects[oi].data = data;
                out.replaced += sprites.len();
                out.changed.insert(fi);
            }
            Err(e) => out.errors.push(format!("texture {pid}: {e}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_textures_have_their_own_keys() {
        assert!(valid_path("effect_awk/textures/a_tex_004.png"));
        assert!(!valid_path("effect_awk/other/a_tex_004.png"));
        let mut m = BTreeMap::new();
        m.insert("effect_awk/textures/a_tex_004.png".to_string(), vec![1]);
        m.insert("effect_awk/sprites/S.png".to_string(), vec![2]);
        let p = ArtPack::from_files(m).unwrap();
        let f = p.for_bundle("effect_awk");
        assert_eq!(
            f.get("textures/a_tex_004.png").map(|b| b.to_vec()),
            Some(vec![1])
        );
        assert_eq!(f.get("S.png").map(|b| b.to_vec()), Some(vec![2]));
        assert_eq!(p.bundles(), ["effect_awk"]);
    }

    #[test]
    fn crop_box_matches_the_exporter() {
        // Exporter: x0=round(x), x1=round(x+w), top=round(h-y-hh), bot=round(h-y), ties to even.
        assert_eq!(
            crop_box((1384.0, 328.29, 32.0, 31.41), 2048),
            Some((1384, 1688, 32, 32))
        );
        assert_eq!(crop_box((0.5, 0.0, 2.0, 2.0), 4), Some((0, 2, 2, 2)));
        assert_eq!(crop_box((1.5, 0.0, 2.0, 2.0), 4), Some((2, 2, 2, 2)));
        assert_eq!(crop_box((0.0, 0.0, 0.2, 2.0), 4), None);
    }

    #[test]
    fn bin_round_trip_and_path_rules() {
        let mut m = BTreeMap::new();
        m.insert("ui_mypage/sprites/A.png".to_string(), vec![1, 2, 3]);
        m.insert("ui_gasha/sprites/B__5.png".to_string(), vec![4]);
        let p = ArtPack::from_files(m).unwrap();
        let back = ArtPack::from_bin(&p.to_bin()).unwrap();
        assert_eq!((back.len(), back.digest.clone()), (2, p.digest.clone()));
        assert_eq!(back.bundles(), ["ui_gasha", "ui_mypage"]);
        assert_eq!(back.for_bundle("ui_mypage").len(), 1);
        assert!(ArtPack::from_bin(&p.to_bin()[..20]).is_err());
        for bad in [
            "a/b.png",
            "../sprites/x.png",
            "ui/sprites/../x.png",
            "ui/other/x.png",
            "ui_statue_01/thumbs/x.png",
            "ui_statue_01/thumbs/st_1080_1.png",
            "ui_statue_01/thumbs/st_1080_01.jpg",
            "ui_statue_01/thumbs/st_10a0_01.png",
        ] {
            assert!(!valid_path(bad), "{bad}");
        }
        assert!(valid_path("ui_statue_01/thumbs/st_1080_01.png"));
        let mut m = BTreeMap::new();
        m.insert("ui_statue_01/thumbs/st_1080_01.png".to_string(), vec![1]);
        m.insert("ui_mypage/sprites/A.png".to_string(), vec![2]);
        let p = ArtPack::from_files(m).unwrap();
        assert_eq!(p.thumbs(), 1);
        assert_eq!(
            p.for_bundle("ui_statue_01").keys().collect::<Vec<_>>(),
            ["thumbs/st_1080_01.png"]
        );
        assert_eq!(safe_name("ガシャ:01"), "____01");
    }
}
