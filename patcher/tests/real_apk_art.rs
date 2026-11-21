//! Real-data test (JDR_APK, JDR_DATA, JDR_KEYS, JDR_ART_ZIP): run `ui-art` on the player's APK in
//! memory and read every in-APK file the pack names back:
//! - no file is left unchanged (every one has a proven type layout);
//! - putting the original data back into the changed objects writes the original file
//!   byte-for-byte (header, type table, externals and every other object are untouched), and only
//!   Texture2D objects changed;
//! - a whole-texture image (e.g. the loading-screen atlas, a file that holds only a Texture2D) is
//!   the texture exactly: RGBA32, in-file pixels equal to the PNG, every other field as before.
//!
//! JDR_ART_APK_OUT=<path> writes the patched (unsigned) APK, for `tools/proofs/art_check.py --apk`.

use jdr_patch::apk::ApkEdit;
use jdr_patch::art;
use jdr_unity::sfile::{self, V};
use jdr_unity::texture;
use std::collections::BTreeMap;
use std::path::Path;

const CLASS_TEXTURE2D: i32 = 28;

#[test]
fn applies_in_apk_art() {
    let (Ok(apk_path), Ok(data), Ok(keys), Ok(zip)) = (
        std::env::var("JDR_APK"),
        std::env::var("JDR_DATA"),
        std::env::var("JDR_KEYS"),
        std::env::var("JDR_ART_ZIP"),
    ) else {
        eprintln!("skipped: set JDR_APK, JDR_DATA, JDR_KEYS and JDR_ART_ZIP");
        return;
    };
    let keys_json = std::fs::read(&keys).unwrap();
    let k: serde_json::Value = serde_json::from_slice(&keys_json).unwrap();
    let (key, iv) = (
        k["asset_key"].as_str().unwrap().as_bytes(),
        k["asset_iv"].as_str().unwrap().as_bytes(),
    );
    let pack = art::read_pack_zip(Path::new(&zip)).unwrap();
    let mut apk = ApkEdit::open(Path::new(&apk_path)).unwrap();

    // apk_<file> -> its images (pack key without the folder).
    let mut by_file: BTreeMap<String, BTreeMap<String, Vec<u8>>> = BTreeMap::new();
    for (k, v) in &pack {
        if let Some(rest) = k.strip_prefix("apk_") {
            let (file, img) = rest.split_once('/').unwrap();
            by_file
                .entry(file.to_string())
                .or_default()
                .insert(img.to_string(), v.clone());
        }
    }
    assert!(!by_file.is_empty(), "no apk_* images in the pack");
    let orig: BTreeMap<String, Vec<u8>> = by_file
        .keys()
        .map(|f| {
            (
                f.clone(),
                apk.read(&format!("assets/bin/Data/{f}")).unwrap(),
            )
        })
        .collect();

    let r = art::apply(
        &mut apk,
        pack.clone(),
        Some(Path::new(&data)),
        Some(&keys_json),
    )
    .unwrap();
    eprintln!(
        "{} sprites/textures in {} in-APK files; left unchanged: {:?}",
        r.in_apk, r.apk_files, r.skipped
    );
    assert!(r.skipped.is_empty(), "left unchanged: {:?}", r.skipped);
    assert_eq!(r.apk_files, by_file.len());

    let mut whole = 0;
    for (file, images) in &by_file {
        let before = &orig[file];
        let after = apk.read(&format!("assets/bin/Data/{file}")).unwrap();
        assert_ne!(before, &after, "{file} unchanged");
        let f0 = sfile::parse(before).unwrap();
        assert_eq!(
            &f0.write(),
            before,
            "{file}: original does not re-write exactly"
        );
        let mut f1 = sfile::parse(&after).unwrap();
        assert!(!f1.type_trees, "{file}: still without type trees");
        assert_eq!(f0.objects.len(), f1.objects.len());
        let mut changed = Vec::new();
        for (a, b) in f0.objects.iter().zip(f1.objects.iter_mut()) {
            assert_eq!((a.path_id, a.type_index), (b.path_id, b.type_index));
            if a.data != b.data {
                let class = f0.types[a.type_index as usize].entry.class_id;
                assert_eq!(
                    class, CLASS_TEXTURE2D,
                    "{file}: object {} changed",
                    a.path_id
                );
                changed.push((a.path_id, a.type_index, a.data.clone(), b.data.clone()));
                b.data = a.data.clone();
            }
        }
        assert!(!changed.is_empty());
        // With the original data back in the changed objects, the file is the original.
        assert_eq!(
            &f1.write(),
            before,
            "{file}: more than the textures changed"
        );

        // Decode the changed textures through the tree for their type hash.
        for (pid, ti, old, new) in &changed {
            let h = f0.types[*ti as usize].entry.raw[7..23].to_vec();
            let trees =
                art::template_nodes(Path::new(&data), key, iv, &[(CLASS_TEXTURE2D, h.clone())])
                    .unwrap();
            let nodes = &trees[&(CLASS_TEXTURE2D, h)];
            let (v0, v1) = (
                sfile::read_object(nodes, old).unwrap(),
                sfile::read_object(nodes, new).unwrap(),
            );
            let int = |v: &V, k: &str| v.get(k).and_then(V::as_i64).unwrap();
            assert_eq!(int(&v1, "m_TextureFormat"), texture::FMT_RGBA32);
            let (w, h) = (int(&v0, "m_Width") as usize, int(&v0, "m_Height") as usize);
            assert_eq!(
                (int(&v1, "m_Width"), int(&v1, "m_Height")),
                (w as i64, h as i64)
            );
            let Some(V::Bytes(px)) = v1.get("image data") else {
                panic!("{file} {pid}: no in-file pixels");
            };
            assert_eq!(px.len(), w * h * 4);
            assert_eq!(int(v1.get("m_StreamData").unwrap(), "size"), 0);
            // Every field but the pixels and their description is as before.
            let edited = [
                "m_TextureFormat",
                "m_CompleteImageSize",
                "m_MipCount",
                "m_MipMap",
                "image data",
                "m_StreamData",
            ];
            let (V::Obj(a), V::Obj(b)) = (&v0, &v1) else {
                panic!()
            };
            for ((ka, va), (kb, vb)) in a.iter().zip(b) {
                assert_eq!(ka, kb);
                if !edited.contains(&ka.as_str()) {
                    assert_eq!(va, vb, "{file} {pid}: {ka} changed");
                }
            }
            // A whole-texture image: the texture is the PNG (rows flipped to engine order).
            let name = v0.get("m_Name").and_then(V::as_str).unwrap();
            let img = images
                .get(&format!(
                    "textures/{}.png",
                    jdr_services::art::safe_name(name)
                ))
                .or_else(|| {
                    images.get(&format!(
                        "textures/{}__{pid}.png",
                        jdr_services::art::safe_name(name)
                    ))
                });
            if let Some(png) = img {
                let (iw, ih, rgba) = texture::decode_png(png).unwrap();
                assert_eq!((iw, ih), (w, h));
                for row in 0..h {
                    let src = &rgba[row * w * 4..(row + 1) * w * 4];
                    let dst = &px[(h - 1 - row) * w * 4..(h - row) * w * 4];
                    assert!(src == dst, "{file} {name}: row {row} differs from the PNG");
                }
                whole += 1;
                eprintln!("{file}: {name} ({w}x{h}) is the PNG exactly");
            }
        }
    }
    let want_whole = pack
        .keys()
        .filter(|k| k.starts_with("apk_") && k.contains("/textures/"))
        .count();
    assert_eq!(whole, want_whole);
    if let Ok(out) = std::env::var("JDR_ART_APK_OUT") {
        apk.write(Path::new(&out)).unwrap();
        eprintln!("wrote {out}");
    }
}
