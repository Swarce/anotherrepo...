//! Real-data test: add statue thumbnails to the player's `ui_statue_01` as the phone does
//! (`art::apply` with `thumbs/st_<id>.png`) and read them back the way the client reaches them:
//! container key -> TextAsset -> decrypt -> a bundle whose Texture2D holds the image. The images
//! are synthetic. Needs JDR_DATA and JDR_KEYS; JDR_PORTRAIT_OUT=<file> keeps the bundle (for the
//! UnityPy check, tools/proofs/portrait_check.py).

use jdr_crypto::{names, Jjec};
use jdr_services::art;
use jdr_unity::sfile::{self, V};
use jdr_unity::{bundle, texture};
use std::collections::HashMap;

fn png_of(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(rgba).unwrap();
    out
}

/// A 128x128 test image: a gradient with an opaque corner marker, so a flip would show.
fn image(seed: u8) -> Vec<u8> {
    let mut px = Vec::with_capacity(128 * 128 * 4);
    for y in 0..128u32 {
        for x in 0..128u32 {
            let marker = x < 8 && y < 8;
            px.extend_from_slice(&[
                if marker { 255 } else { (x * 2) as u8 },
                if marker { 0 } else { (y * 2) as u8 },
                seed,
                255,
            ]);
        }
    }
    px
}

fn objects(data: &[u8]) -> (sfile::SFile, Vec<(i32, i64, V)>) {
    let f = sfile::parse(data).unwrap();
    let v = f
        .objects
        .iter()
        .map(|o| {
            let t = &f.types[o.type_index as usize];
            (
                t.entry.class_id,
                o.path_id,
                sfile::read_object(&t.nodes, &o.data).unwrap(),
            )
        })
        .collect();
    (f, v)
}

fn s(v: &V, k: &str) -> String {
    v.get(k).and_then(V::as_str).unwrap_or_default().to_string()
}

#[test]
fn adds_thumbnails_the_client_can_reach() {
    let (Ok(dir), Ok(keys)) = (std::env::var("JDR_DATA"), std::env::var("JDR_KEYS")) else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let k = jdr_core::settings::Settings::load_keys(std::path::Path::new(&keys)).unwrap();
    let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
    let obf = format!("{}.bytes", names::obfuscate("ui_statue_01", key));
    let raw = std::fs::read(std::path::Path::new(&dir).join(obf)).unwrap();
    let plain = if Jjec::is_jjec(&raw) {
        Jjec::decrypt(&raw, key, iv).unwrap()
    } else {
        raw
    };
    let mut nodes = bundle::read(&plain).unwrap();
    let si = nodes.iter().position(|n| n.flags & 4 != 0).unwrap();
    let (_, before) = objects(&nodes[si].data);
    let before_ab = before.iter().find(|o| o.0 == 142).unwrap().2.clone();
    let n_before = match before_ab.get("m_Container") {
        Some(V::Arr(a)) => a.len(),
        _ => panic!("container"),
    };
    let original: HashMap<i64, Vec<u8>> = sfile::parse(&nodes[si].data)
        .unwrap()
        .objects
        .iter()
        .map(|o| (o.path_id, o.data.clone()))
        .collect();

    // two new ones (ids no thumbnail has), and one the bundle has (left alone, reported)
    let have = before
        .iter()
        .find(|o| o.0 == 49)
        .map(|o| s(&o.2, "m_Name"))
        .unwrap();
    let (a, b) = (png_of(128, 128, &image(10)), png_of(128, 128, &image(200)));
    let existing = png_of(128, 128, &image(1));
    let mut images: HashMap<String, &[u8]> = HashMap::new();
    images.insert("thumbs/st_9990_01.png".into(), &a);
    images.insert("thumbs/st_9990_02.png".into(), &b);
    images.insert(format!("thumbs/{have}.png"), &existing);
    let r = art::apply(&mut nodes, &images, Some((key, iv))).unwrap();
    assert_eq!(r.added, 2, "{:?}", r.errors);
    assert_eq!(
        r.errors.len(),
        1,
        "the existing one is reported: {:?}",
        r.errors
    );
    assert!(art::apply(&mut nodes.clone(), &images, None).unwrap().added == 0);

    // the rebuilt bundle round-trips and every original object is byte-identical
    let rebuilt = bundle::write_lz4(&nodes, jdr_unity::UNITY_VERSION);
    let nodes2 = bundle::read(&rebuilt).unwrap();
    let si2 = nodes2.iter().position(|n| n.flags & 4 != 0).unwrap();
    let f2 = sfile::parse(&nodes2[si2].data).unwrap();
    for o in &f2.objects {
        if let Some(d) = original.get(&o.path_id) {
            if f2.types[o.type_index as usize].entry.class_id != 142 {
                assert_eq!(&o.data, d, "object {} changed", o.path_id);
            }
        }
    }
    let (_, after) = objects(&nodes2[si2].data);
    let ab = &after.iter().find(|o| o.0 == 142).unwrap().2;
    let Some(V::Arr(cont)) = ab.get("m_Container") else {
        panic!()
    };
    let Some(V::Arr(pre)) = ab.get("m_PreloadTable") else {
        panic!()
    };
    assert_eq!(cont.len(), n_before + 2);
    let keys_in: Vec<String> = cont.iter().map(|e| s(e, "first")).collect();
    let mut sorted = keys_in.clone();
    sorted.sort();
    assert_eq!(keys_in, sorted, "container stays sorted");

    for (name, seed) in [("st_9990_01", 10u8), ("st_9990_02", 200u8)] {
        // container key -> preload slice and asset -> the TextAsset
        let e = cont
            .iter()
            .find(|e| s(e, "first").ends_with(&format!("/ui_statue_01/{name}.bytes")))
            .unwrap_or_else(|| panic!("{name} not in the container"));
        let sec = e.get("second").unwrap();
        let pid = sec
            .get("asset")
            .unwrap()
            .get("m_PathID")
            .and_then(V::as_i64)
            .unwrap();
        let pi = sec.get("preloadIndex").and_then(V::as_i64).unwrap() as usize;
        assert_eq!(pre[pi].get("m_PathID").and_then(V::as_i64), Some(pid));
        let ta = &after.iter().find(|o| o.1 == pid).unwrap();
        assert_eq!((ta.0, s(&ta.2, "m_Name").as_str()), (49, name));
        let Some(V::Str(enc)) = ta.2.get("m_Script") else {
            panic!()
        };
        assert!(Jjec::is_jjec(enc));
        let inner = Jjec::decrypt(enc, key, iv).unwrap();
        let inodes = bundle::read(&inner).unwrap();
        assert_eq!(inodes.len(), 1, "no .resS stream left");
        assert!(inodes[0].name.starts_with("CAB-") && inodes[0].name.len() == 36);
        let (_, io) = objects(&inodes[0].data);
        let tex = io.iter().find(|o| o.0 == 28).unwrap();
        let spr = io.iter().find(|o| o.0 == 213).unwrap();
        let iab = io.iter().find(|o| o.0 == 142).unwrap();
        assert_eq!(s(&tex.2, "m_Name"), name);
        assert_eq!(s(&spr.2, "m_Name"), name);
        assert_eq!(s(&iab.2, "m_AssetBundleName"), name);
        let Some(V::Arr(ic)) = iab.2.get("m_Container") else {
            panic!()
        };
        assert!(ic
            .iter()
            .all(|e| s(e, "first").ends_with(&format!("/{name}.png"))));
        assert_eq!(
            spr.2
                .get("m_RD")
                .unwrap()
                .get("texture")
                .unwrap()
                .get("m_PathID")
                .and_then(V::as_i64),
            Some(tex.1),
            "the sprite still points at the texture"
        );
        assert_eq!(
            tex.2.get("m_TextureFormat").and_then(V::as_i64),
            Some(texture::FMT_RGBA32)
        );
        let Some(V::Bytes(px)) = tex.2.get("image data") else {
            panic!()
        };
        // engine order: the bottom row first (the PNG's top row, with the red marker, comes last)
        let back = texture::decode(texture::FMT_RGBA32, 128, 128, px).unwrap();
        let flipped: Vec<u8> = image(seed)
            .chunks(128 * 4)
            .rev()
            .flatten()
            .copied()
            .collect();
        assert!(back == flipped, "{name}: the pixels are not the image");
        assert_eq!(
            &back[127 * 512..127 * 512 + 4],
            &[255, 0, seed, 255],
            "marker at the top left"
        );
    }
    if let Ok(out) = std::env::var("JDR_PORTRAIT_OUT") {
        std::fs::write(out, &rebuilt).unwrap();
    }
    eprintln!("{} thumbnails in ui_statue_01 (was {n_before})", cont.len());
}
