//! Real-data test (JDR_APK): run `server-url` and `english-apk-text` on the player's APK in memory,
//! then read the scenes and the metadata back: every scene label and allow-listed literal with a
//! translation is our English, nothing else changed. With JDR_DATA and JDR_KEYS as well, the
//! patched labels are also decoded through the `UI.Text` type tree of a downloaded `scene_*`
//! bundle (same engine build), which must consume each object exactly.

use jdr_crypto::{names, Jjec};
use jdr_patch::apk::ApkEdit;
use jdr_patch::apktext;
use jdr_patch::meta::Metadata;
use jdr_patch::patches::{self, Config, Patch, METADATA_PATH};
use jdr_services::l10n::{has_japanese, Translations};
use jdr_unity::{bundle, sfile, typetree::TNode};

#[test]
fn patches_and_rereads_apk_text() {
    let Ok(apk_path) = std::env::var("JDR_APK") else {
        eprintln!("skipped: set JDR_APK");
        return;
    };
    let mut apk = ApkEdit::open(std::path::Path::new(&apk_path)).unwrap();
    let build = patches::identify(&mut apk).unwrap();
    let tr = Translations::embedded().unwrap();
    let orig_meta = apk.read(METADATA_PATH).unwrap();
    let orig_scenes: Vec<Vec<u8>> = build
        .apk_text_scenes
        .iter()
        .map(|(f, _)| apk.read(&format!("{}{f}", apktext::DATA_DIR)).unwrap())
        .collect();

    let cfg = Config::default();
    patches::ServerUrl.apply(&mut apk, &cfg, build).unwrap();
    let detail = patches::TranslateApkText
        .apply(&mut apk, &cfg, build)
        .unwrap();
    eprintln!("{detail}");

    // Scenes: every Japanese label has its English, every other object is unchanged.
    let mut labels = 0;
    let mut patched_objects = Vec::new();
    for ((file, _), orig) in build.apk_text_scenes.iter().zip(&orig_scenes) {
        let out = apk.read(&format!("{}{file}", apktext::DATA_DIR)).unwrap();
        let before = apktext::scene_texts(orig).unwrap();
        let after = apktext::scene_texts(&out).unwrap();
        assert_eq!(before.len(), after.len());
        for ((pid, a), (pid2, b)) in before.iter().zip(&after) {
            assert_eq!(pid, pid2);
            if has_japanese(a) {
                let en = tr.english(&apktext::scene_key(file, *pid)).unwrap();
                assert_eq!(b, en, "{file} {pid}");
                assert!(!has_japanese(b));
                labels += 1;
            } else {
                assert_eq!(a, b);
            }
        }
        let (f, g) = (sfile::parse(orig).unwrap(), sfile::parse(&out).unwrap());
        assert_eq!(f.objects.len(), g.objects.len());
        for (x, y) in f.objects.iter().zip(&g.objects) {
            if x.data != y.data {
                assert!(apktext::parse_ui_text(&y.data).is_ok());
                patched_objects.push(y.data.clone());
            }
        }
        assert_eq!(f.externals_raw, g.externals_raw);
    }
    assert_eq!(labels, 72);
    assert_eq!(patched_objects.len(), 72);

    // Literals: the allow-listed ones are English (four relocated), all others are as before
    // (except the server URL).
    let out_meta = apk.read(METADATA_PATH).unwrap();
    let (m0, m1) = (
        Metadata::parse(&orig_meta).unwrap(),
        Metadata::parse(&out_meta).unwrap(),
    );
    let allow: Vec<usize> = build.ui_literals.iter().map(|(i, _)| *i).collect();
    let (mut relocated, mut longer) = (0, 0);
    for i in 0..m0.literal_count() {
        let (a, b) = (m0.literal(i).unwrap(), m1.literal(i).unwrap());
        if allow.contains(&i) {
            let en = tr.english(&apktext::literal_key(i)).unwrap();
            assert_eq!(b, en.as_bytes(), "literal {i}");
            assert!(has_japanese(std::str::from_utf8(a).unwrap()));
            relocated += m1.is_relocated(i).unwrap() as usize;
            // English that does not fit in the original bytes is appended and repointed.
            longer += (en.len() > a.len()) as usize;
        } else if i != build.server_url_literal.0 {
            assert_eq!(a, b, "literal {i}");
        }
    }
    assert_eq!(allow.len(), 22);
    assert_eq!(
        relocated, longer,
        "relocated exactly the literals that grew"
    );
    assert!(relocated > 0);
    assert_eq!(out_meta[..24], orig_meta[..24], "header unchanged");
    assert!(out_meta.len() > orig_meta.len());

    // The post-check counts the same.
    let (s, l) = patches::apk_text_english(&mut apk, build).unwrap();
    assert_eq!((s, l), (72, 22));

    // Cross-check with the engine's own UI.Text type tree, when the game files are given.
    let (Ok(data), Ok(keys)) = (std::env::var("JDR_DATA"), std::env::var("JDR_KEYS")) else {
        eprintln!("type-tree cross-check skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let nodes = ui_text_tree(std::path::Path::new(&data), std::path::Path::new(&keys));
    for d in &patched_objects {
        let v = sfile::read_object(&nodes, d).unwrap();
        let t = apktext::parse_ui_text(d).unwrap();
        assert_eq!(
            v.get("m_Text").and_then(sfile::V::as_str),
            Some(t.text.as_str())
        );
        assert_eq!(&sfile::write_object(&nodes, &v).unwrap(), d);
    }
    for orig in &orig_scenes {
        let f = sfile::parse(orig).unwrap();
        for o in &f.objects {
            if let Ok(t) = apktext::parse_ui_text(&o.data) {
                let v = sfile::read_object(&nodes, &o.data).unwrap();
                assert_eq!(
                    v.get("m_Text").and_then(sfile::V::as_str),
                    Some(t.text.as_str())
                );
            }
        }
    }
    eprintln!("type-tree cross-check: {} objects", patched_objects.len());
}

/// The `UI.Text` type tree from the first `scene_*` bundle that has one.
fn ui_text_tree(data: &std::path::Path, keys: &std::path::Path) -> Vec<TNode> {
    let k: serde_json::Value = serde_json::from_slice(&std::fs::read(keys).unwrap()).unwrap();
    let key = k["asset_key"].as_str().unwrap().as_bytes();
    let iv = k["asset_iv"].as_str().unwrap().as_bytes();
    let mut files: Vec<_> = std::fs::read_dir(data).unwrap().flatten().collect();
    files.sort_by_key(|e| e.file_name());
    for e in files {
        let f = e.file_name().to_string_lossy().into_owned();
        let Some(name) = f
            .strip_suffix(".bytes")
            .and_then(|s| names::deobfuscate(s, key))
        else {
            continue;
        };
        if !name.starts_with("scene_") {
            continue;
        }
        let mut b = std::fs::read(e.path()).unwrap();
        if Jjec::is_jjec(&b) {
            b = Jjec::decrypt(&b, key, iv).unwrap();
        }
        for n in bundle::read(&b)
            .unwrap()
            .iter()
            .filter(|n| n.flags & 4 != 0)
        {
            let sf = sfile::parse(&n.data).unwrap();
            for t in &sf.types {
                let top: Vec<&str> = t
                    .nodes
                    .iter()
                    .filter(|x| x.level == 1)
                    .map(|x| x.name.as_str())
                    .collect();
                if t.entry.class_id == 114
                    && top.last() == Some(&"m_Text")
                    && top.contains(&"m_FontData")
                {
                    return t.nodes.clone();
                }
            }
        }
    }
    panic!("no scene_* bundle with a UI.Text type tree in JDR_DATA");
}
