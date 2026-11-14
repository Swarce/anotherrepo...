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

use jdr_unity::bundle::Node;
use jdr_unity::sfile::{self, V};
use jdr_unity::texture;
use std::collections::{BTreeMap, HashMap};

type Error = String;

pub const MAGIC: &[u8; 8] = b"JDRART1\n";
/// Bump when the way art is applied changes, so phones rebuild their cached files.
pub const ART_GEN_VERSION: u32 = 2;
/// Unity class ids.
const CLASS_TEXTURE2D: i32 = 28;
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

/// A pack path is `<bundle>/sprites/<name>.png` with plain components only.
pub fn valid_path(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    parts.len() == 3
        && parts[1] == "sprites"
        && parts[2].ends_with(".png")
        && parts
            .iter()
            .all(|c| !c.is_empty() && *c != "." && *c != ".." && !c.contains('\\'))
}

impl ArtPack {
    pub fn from_files(files: BTreeMap<String, Vec<u8>>) -> Result<Self, Error> {
        if let Some(bad) = files.keys().find(|k| !valid_path(k)) {
            return Err(format!(
                "art pack: unexpected path {bad:?} (want <bundle>/sprites/<name>.png)"
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
            let sprites = b.path().join("sprites");
            if !sprites.is_dir() {
                continue;
            }
            for f in rd(&sprites)?.flatten() {
                let name = f.file_name().to_string_lossy().into_owned();
                if name.ends_with(".png") {
                    let key = format!("{}/sprites/{name}", b.file_name().to_string_lossy());
                    files.insert(key, std::fs::read(f.path()).map_err(|e| e.to_string())?);
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

    /// `<file>.png` -> PNG bytes for one bundle.
    pub fn for_bundle(&self, bundle: &str) -> HashMap<String, &[u8]> {
        let prefix = format!("{bundle}/sprites/");
        self.files
            .iter()
            .filter_map(|(k, v)| Some((k.strip_prefix(&prefix)?.to_string(), v.as_slice())))
            .collect()
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
    /// Indices of the serialized files that changed.
    pub changed: std::collections::HashSet<usize>,
    /// Images that match no sprite.
    pub unmatched: Vec<String>,
    /// Sprites or textures skipped (the rest of the bundle, its text included, still applies).
    pub errors: Vec<String>,
}

/// Replace sprites in a decoded bundle's nodes.
pub fn apply(nodes: &mut [Node], images: &HashMap<String, &[u8]>) -> Result<Applied, Error> {
    if images.is_empty() {
        return Ok(Applied::default());
    }
    let mut files = Vec::new();
    for n in nodes.iter() {
        files.push(if n.flags & 4 != 0 {
            Some(sfile::parse(&n.data)?)
        } else {
            None
        });
    }
    let r = {
        let res: HashMap<&str, &[u8]> = nodes
            .iter()
            .filter(|n| n.flags & 4 == 0)
            .map(|n| (n.name.as_str(), n.data.as_slice()))
            .collect();
        apply_files(&mut files, &res, images)
    };
    for (i, (n, f)) in nodes.iter_mut().zip(files).enumerate() {
        if let (true, Some(f)) = (r.changed.contains(&i), f) {
            n.data = f.write();
        }
    }
    Ok(r)
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
    for f in files.iter().flatten() {
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
            if w > 0 && h > 0 && v.get("m_Name").and_then(V::as_str) != Some("Font Texture") {
                exported.insert(o.path_id, h as usize);
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
        ] {
            assert!(!valid_path(bad), "{bad}");
        }
        assert_eq!(safe_name("ガシャ:01"), "____01");
    }
}
