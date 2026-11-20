//! Real-data test: apply the player's redrawn art (JDR_ART = their `replacements/` folder) to their
//! own bundles, as the phone would. Needs JDR_DATA, JDR_KEYS, JDR_ART.
//! JDR_ART_OUT=<dir> keeps the rebuilt files (for the UnityPy check).

use jdr_services::{art::ArtPack, l10n};

#[test]
fn applies_the_art_pack() {
    let (Ok(dir), Ok(keys), Ok(art)) = (
        std::env::var("JDR_DATA"),
        std::env::var("JDR_KEYS"),
        std::env::var("JDR_ART"),
    ) else {
        eprintln!("skipped: set JDR_DATA, JDR_KEYS and JDR_ART");
        return;
    };
    let k = jdr_core::settings::Settings::load_keys(std::path::Path::new(&keys)).unwrap();
    let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
    // a replacements/ folder, or the art.bin a patched APK carries (what the phone reads)
    let pack = if art.ends_with(".bin") {
        ArtPack::from_bin(&std::fs::read(&art).unwrap()).unwrap()
    } else {
        ArtPack::from_dir(std::path::Path::new(&art)).unwrap()
    };
    // In-APK screens (apk_*) are the patcher's job; count only the downloaded bundles here.
    let want: usize = pack
        .bundles()
        .iter()
        .filter(|b| !b.starts_with("apk_"))
        .map(|b| pack.for_bundle(b).len())
        .sum();
    let tr = l10n::Translations::embedded().unwrap();
    let state = std::env::var("JDR_ART_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::temp_dir().join(format!("jdr-real-art-{}", std::process::id()))
        });
    let _ = std::fs::remove_dir_all(&state);
    let t0 = std::time::Instant::now();
    let (loc, errs) = l10n::prepare(
        std::path::Path::new(&dir),
        &state,
        key,
        iv,
        &tr,
        Some(&pack),
    );
    let errs: Vec<_> = errs
        .into_iter()
        .filter(|e| !e.starts_with("apk_"))
        .collect();
    assert!(errs.is_empty(), "{errs:?}");
    let done: usize = loc.iter().map(|l| l.stats.art).sum();
    let unmatched: usize = loc.iter().map(|l| l.stats.art_unmatched).sum();
    eprintln!(
        "{} bundles, {done} of {want} sprites replaced, {unmatched} unmatched, {} ms",
        loc.len(),
        t0.elapsed().as_millis()
    );
    assert_eq!((done, unmatched), (want, 0));
    let errors: Vec<_> = loc
        .iter()
        .flat_map(|l| l.stats.art_errors.clone())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
    // Storage: originals kept vs the files we serve.
    let size = |d: &str| -> u64 {
        std::fs::read_dir(state.join("l10n").join(d))
            .map(|r| {
                r.flatten()
                    .filter_map(|e| e.metadata().ok())
                    .map(|m| m.len())
                    .sum()
            })
            .unwrap_or(0)
    };
    eprintln!(
        "storage: originals {} MiB, served files {} MiB",
        size("orig") >> 20,
        size("gen") >> 20
    );
    // Removing the pack serves text-only bundles again (the atlases revert to the originals).
    // Done on a copy of the state, so JDR_ART_OUT keeps the art build for the UnityPy check.
    fn copy_dir(a: &std::path::Path, b: &std::path::Path) {
        std::fs::create_dir_all(b).unwrap();
        for e in std::fs::read_dir(a).unwrap().flatten() {
            let to = b.join(e.file_name());
            if e.path().is_dir() {
                copy_dir(&e.path(), &to);
            } else {
                std::fs::copy(e.path(), &to).unwrap();
            }
        }
    }
    let state_copy =
        std::env::temp_dir().join(format!("jdr-real-art-revert-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&state_copy);
    copy_dir(&state, &state_copy);
    let state = state_copy;
    let (plain, errs) = l10n::prepare(std::path::Path::new(&dir), &state, key, iv, &tr, None);
    assert!(errs.is_empty(), "{errs:?}");
    assert!(plain.iter().all(|l| l.stats.art == 0));
    let fresh = std::env::temp_dir().join(format!("jdr-real-art-text-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&fresh);
    let (text_only, _) = l10n::prepare(std::path::Path::new(&dir), &fresh, key, iv, &tr, None);
    let md5s = |v: &[l10n::Localized]| {
        let mut m: Vec<_> = v.iter().map(|l| (l.name.clone(), l.md5.clone())).collect();
        m.sort();
        m
    };
    let text: std::collections::HashMap<_, _> = md5s(&text_only).into_iter().collect();
    for (name, md5) in md5s(&plain) {
        match text.get(&name) {
            // Bundles with text: the text-only English file.
            Some(t) => assert_eq!(&md5, t, "{name}: revert gives the text-only file"),
            // Art-only bundles: still listed, now with the original, so the phone reverts too.
            None => {
                let l = plain.iter().find(|l| l.name == name).unwrap();
                let orig = std::fs::read(state.join("l10n/orig").join(&l.obf)).unwrap();
                assert_eq!(
                    md5,
                    jdr_services::versionfile::md5_hex(&orig),
                    "{name}: original served"
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(&fresh);
    let _ = std::fs::remove_dir_all(&state);
}
