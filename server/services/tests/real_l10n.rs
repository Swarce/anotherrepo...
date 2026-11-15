//! Real-data test: rebuild every UI bundle and text.jp with a placeholder translation for every
//! string, as the phone would, and check the result. Needs JDR_DATA and JDR_KEYS.
//! JDR_L10N_OUT=<dir> keeps the rebuilt files (for the UnityPy check).

use jdr_crypto::{names, Jjec};
use jdr_services::l10n;

#[test]
fn rebuilds_every_text_bundle() {
    let (Ok(dir), Ok(keys)) = (std::env::var("JDR_DATA"), std::env::var("JDR_KEYS")) else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let k = jdr_core::settings::Settings::load_keys(std::path::Path::new(&keys)).unwrap();
    let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
    // A translation for every string: "EN <key>" (placeholders kept so the text stays valid).
    let mut entries = serde_json::Map::new();
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let f = e.file_name().to_string_lossy().into_owned();
        let Some(name) = f
            .strip_suffix(".bytes")
            .and_then(|s| names::deobfuscate(s, key))
        else {
            continue;
        };
        let ui = (name.starts_with("ui_") && !name.starts_with("ui_tips"))
            || name.starts_with("scene_")
            || name.starts_with("minigame_");
        if !(ui || name == "text.jp") {
            continue;
        }
        let mut b = std::fs::read(e.path()).unwrap();
        if Jjec::is_jjec(&b) {
            b = Jjec::decrypt(&b, key, iv).unwrap();
        }
        for found in l10n::collect(&b, &name).unwrap() {
            let en = format!("EN {}", l10n::placeholders(&found.source).join(" "));
            entries.insert(
                found.key,
                serde_json::json!([l10n::source_hash(&found.source), en]),
            );
        }
    }
    let js =
        serde_json::json!({"sources":[{"source":"jdr-default","note":"test"}],"entries":entries})
            .to_string();
    let tr = l10n::Translations::parse(&[("all", js.as_str())]).unwrap();
    let state = std::env::var("JDR_L10N_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::temp_dir().join(format!("jdr-real-l10n-{}", std::process::id()))
        });
    let _ = std::fs::remove_dir_all(&state);
    let t0 = std::time::Instant::now();
    let (loc, errs) = l10n::prepare(&dir, &state, key, iv, &tr, None);
    let first = t0.elapsed();
    assert!(errs.is_empty(), "{errs:?}");
    let replaced: usize = loc.iter().map(|l| l.stats.replaced).sum();
    let strings: usize = loc.iter().map(|l| l.stats.strings).sum();
    assert_eq!(replaced, strings, "every string replaced");
    assert!(replaced > 50_000, "{replaced}");
    // Rebuilt bundles hold no Japanese strings any more and are plain (not JJEC).
    for l in &loc {
        let b = std::fs::read(&l.file).unwrap();
        assert!(!Jjec::is_jjec(&b) && !l.encrypted);
        assert!(l10n::collect(&b, &l.name).unwrap().is_empty(), "{}", l.name);
    }
    let t1 = std::time::Instant::now();
    let (again, _) = l10n::prepare(&dir, &state, key, iv, &tr, None);
    assert_eq!(
        again.iter().map(|l| &l.md5).collect::<Vec<_>>(),
        loc.iter().map(|l| &l.md5).collect::<Vec<_>>()
    );
    eprintln!(
        "{} bundles, {replaced} strings: first build {} ms, cached {} ms",
        loc.len(),
        first.as_millis(),
        t1.elapsed().as_millis()
    );
}
