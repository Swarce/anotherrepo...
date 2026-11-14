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
    let pack = ArtPack::from_dir(std::path::Path::new(&art)).unwrap();
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
}
