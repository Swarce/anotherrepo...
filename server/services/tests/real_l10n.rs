//! Real-data test: rebuild every UI bundle, text.jp and the story script bundles with a placeholder translation for every
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
        if !(ui || name == "text.jp" || name.ends_with("_script_text_jp")) {
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
    // Every label hinted for a fixed box got one (docs/LOCALIZATION.md).
    let errs: Vec<&String> = loc.iter().flat_map(|l| &l.stats.layout_errors).collect();
    assert!(errs.is_empty(), "{errs:?}");
    let boxes: usize = loc.iter().map(|l| l.stats.name_boxes).sum();
    let doc = serde_json::from_str::<serde_json::Value>(l10n::LAYOUT).unwrap();
    let hinted =
        doc["nameBox"].as_array().unwrap().len() + doc["lineBox"].as_array().unwrap().len();
    assert_eq!(boxes, hinted, "fixed boxes");
    // The Options menu edits (Achievements hidden, the buttons after it moved up).
    let ed = serde_json::from_str::<serde_json::Value>(l10n::EDITS).unwrap();
    let want = ed["hide"].as_array().unwrap().len() + ed["moveTo"].as_array().unwrap().len();
    assert_eq!(
        loc.iter().map(|l| l.stats.edits).sum::<usize>(),
        want,
        "scene edits"
    );
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

/// Every effect-line label of our affect_table (docs/research/16-minds.md §8) is an affectText_JP
/// row of the player's text.jp, in no other table (the client keeps one row per label), shows the
/// amount, and reads in English: its text has no Japanese, or we translate it for exactly this
/// source.
#[test]
fn effect_line_labels_are_in_text_jp() {
    let (Ok(dir), Ok(keys)) = (std::env::var("JDR_DATA"), std::env::var("JDR_KEYS")) else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let k = jdr_core::settings::Settings::load_keys(std::path::Path::new(&keys)).unwrap();
    let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
    let path =
        std::path::Path::new(&dir).join(format!("{}.bytes", names::obfuscate("text.jp", key)));
    let mut b = std::fs::read(&path).unwrap();
    if Jjec::is_jjec(&b) {
        b = Jjec::decrypt(&b, key, iv).unwrap();
    }
    let labels = l10n::text_labels(&b).unwrap();
    let tr = l10n::Translations::embedded().unwrap();
    let affects: serde_json::Value = serde_json::from_str(
        jdr_services::masterdata::TABLES
            .iter()
            .find(|(n, _)| *n == "affect_table")
            .unwrap()
            .1,
    )
    .unwrap();
    let mut n = 0;
    for a in affects["data"]["paramList"].as_array().unwrap() {
        let id = a["affectTextId"].as_str().unwrap();
        if id.is_empty() {
            continue;
        }
        let key = format!("text:affectText_JP:{id}");
        let src = labels.get(&key).unwrap_or_else(|| panic!("{key} is a row"));
        let elsewhere: Vec<&String> = labels
            .keys()
            .filter(|k| k.ends_with(&format!(":{id}")) && **k != key)
            .collect();
        assert!(elsewhere.is_empty(), "{id} also in {elsewhere:?}");
        if id.starts_with("AFFECT_") {
            let p = l10n::placeholders(src);
            assert!(
                p.iter()
                    .any(|x| x == "{decimal_Param0}" || x == "{mag_Param0}"),
                "{id}: shows the amount ({p:?})"
            );
        }
        if l10n::has_japanese(src) {
            assert!(
                matches!(tr.lookup(&key, src), l10n::Lookup::Found(_)),
                "{id}: English for this source"
            );
        }
        n += 1;
    }
    assert!(n >= 20, "{n}");
}
