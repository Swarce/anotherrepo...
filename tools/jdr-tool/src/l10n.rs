//! `jdr-tool l10n`: the translators' round trip.
//!
//! - `export` writes every translatable string from the player's own copy (text.jp, the UI
//!   prefabs; with `--apk` also the in-APK ClientMes, the labels of the APK's own scenes and the
//!   player-visible C# literals) to a local TSV: `key, hash, English, Japanese source`. The TSV
//!   contains game text: keep it out of the repo (`/local/` is git-ignored).
//! - `import` merges a TSV's English column into the repo's translation files, which store only
//!   `key -> [hash, English]`.

use crate::{load_keys, opt, Error};
use jdr_crypto::{names, Jjec};
use jdr_services::l10n::{self, Found};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Bundles whose strings we offer (UI prefabs, the minigame screens, the text tables and the
/// story scripts' dialogue).
pub fn wanted(name: &str) -> bool {
    name == "text.jp"
        || name.ends_with("_script_text_jp")
        || (name.starts_with("ui_") && !name.starts_with("ui_tips"))
        || name.starts_with("scene_")
        || name.starts_with("minigame_")
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub fn unesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

pub fn cmd(a: &[String]) -> Result<(), Error> {
    match a.first().map(String::as_str) {
        Some("export") => export(&a[1..]),
        Some("import") => import(&a[1..]),
        _ => Err("usage: jdr-tool l10n export --data <files> --keys <keys.json> [--apk <apk>] --out <work.tsv>\n       jdr-tool l10n import --in <work.tsv> [--expand-with <export.tsv>] --repo <checkout>".into()),
    }
}

fn export(a: &[String]) -> Result<(), Error> {
    let data = PathBuf::from(opt(a, "--data").ok_or("--data is required")?);
    let k = load_keys(Path::new(&opt(a, "--keys").ok_or("--keys is required")?))?;
    let out = PathBuf::from(opt(a, "--out").ok_or("--out is required")?);
    let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
    let tr = l10n::Translations::embedded()?;
    let mut found: Vec<Found> = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&data)?.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let f = e.file_name().to_string_lossy().into_owned();
        let Some(name) = f
            .strip_suffix(".bytes")
            .and_then(|s| names::deobfuscate(s, key))
        else {
            continue;
        };
        if !wanted(&name) {
            continue;
        }
        let mut b = std::fs::read(e.path())?;
        if Jjec::is_jjec(&b) {
            b = Jjec::decrypt(&b, key, iv)?;
        }
        found.extend(l10n::collect(&b, &name).map_err(|e| format!("{name}: {e}"))?);
    }
    if let Some(apk) = opt(a, "--apk") {
        found.extend(apk_strings(Path::new(&apk))?);
    }
    let mut tsv = String::from("key\thash\tenglish\tjapanese\tprevious_english\n");
    let (mut done, mut stale) = (0, 0);
    for f in &found {
        // A stale row: the source changed; the old English is shown as a reference only.
        let (en, previous) = match tr.lookup(&f.key, &f.source) {
            l10n::Lookup::Found(en) => {
                done += 1;
                (en.to_string(), String::new())
            }
            l10n::Lookup::Stale => {
                stale += 1;
                (
                    String::new(),
                    tr.english(&f.key).unwrap_or_default().to_string(),
                )
            }
            l10n::Lookup::Missing => (String::new(), String::new()),
        };
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            f.key,
            l10n::source_hash(&f.source),
            esc(&en),
            esc(&f.source),
            esc(&previous)
        ));
    }
    std::fs::write(&out, tsv)?;
    eprintln!(
        "{} strings ({} translated, {} stale) -> {} (contains game text: keep it out of the repo)",
        found.len(),
        done,
        stale,
        out.display()
    );
    Ok(())
}

fn zip_read(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>, Error> {
    let mut file = Vec::new();
    std::io::Read::read_to_end(&mut z.by_name(name)?, &mut file)?;
    Ok(file)
}

/// The strings inside the APK: ClientMes, the scene labels and the allow-listed C# literals.
pub fn apk_strings(apk: &Path) -> Result<Vec<Found>, Error> {
    use jdr_patch::{apktext, clientmes, known, patches};
    let mut z = zip::ZipArchive::new(std::fs::File::open(apk)?)?;
    let mut found = Vec::new();
    for (label, text) in clientmes::read(&zip_read(&mut z, clientmes::ENTRY)?)?.rows {
        if l10n::has_japanese(&text) {
            found.push(Found {
                key: format!("clientmes:{label}"),
                source: text,
            });
        }
    }
    let mut scenes: Vec<(u32, String)> = z
        .file_names()
        .filter(|n| apktext::is_scene(n))
        .filter_map(|n| Some((n.rsplit("level").next()?.parse().ok()?, n.to_owned())))
        .collect();
    scenes.sort();
    for (_, entry) in scenes {
        let file = entry.trim_start_matches(apktext::DATA_DIR).to_owned();
        let data = zip_read(&mut z, &entry)?;
        for (key, source) in
            apktext::scene_sources(&file, &data).map_err(|e| format!("{entry}: {e}"))?
        {
            found.push(Found { key, source });
        }
    }
    let meta = zip_read(&mut z, patches::METADATA_PATH)?;
    match known::by_metadata(&patches::sha256_hex(&meta)) {
        Some(build) => {
            for (key, source) in apktext::literal_sources(&meta, build)? {
                found.push(Found { key, source });
            }
        }
        None => eprintln!(
            "note: global-metadata.dat is not a verified build (or already patched): C# literals not exported"
        ),
    }
    Ok(found)
}

/// The text.jp tables of the first (menu) batch; everything else goes to `text_content`.
const MENU_TABLES: &[&str] = &[
    "AchievementText_All_JP",
    "AttributeText_JP",
    "Battle_JP",
    "CharacteristicText_JP",
    "DungeonSummary_JP",
    "Evolution_JP",
    "FilterText_JP",
    "FollowFollower_JP",
    "Formation_JP",
    "Growth_JP",
    "MindSale_JP",
    "MyPage_JP",
    "PvPHome_JP",
    "QuestCondition_JP",
    "QuestConfune_JP",
    "QuestResult_JP",
    "QuestType_JP",
    "ResultText_JP",
    "SkillTypeText_JP",
    "SortText_JP",
    "SpecialActionText_JP",
    "StRateTypeText_JP",
    "StatueTypeText_JP",
    "ThumbnailTextLabel_JP",
    "TitleText_JP",
    "TreeGridLabel_JP",
    "Tree_JP",
    "TutorialMes_JP",
    "gasha_JP",
    "gasha_frenzy_JP",
    "optionText_JP",
    "shop_JP",
];

/// Which translation file a key belongs to.
pub fn file_for(key: &str) -> &'static str {
    if key.starts_with("clientmes:") {
        "clientmes"
    } else if key.starts_with("apk:") || key.starts_with("literal:") {
        "apk_text"
    } else if key.starts_with("ui:") {
        "ui_prefabs"
    } else if key.starts_with("script:") {
        "story"
    } else if MENU_TABLES.contains(&key.split(':').nth(1).unwrap_or("")) {
        "text_menu"
    } else {
        "text_content"
    }
}

/// With `--expand-with <export.tsv>`: every row of the export whose Japanese source equals a
/// translated row's source gets the same English (translate unique strings once). Rows the
/// input translates itself, and rows already translated in the repo, are left as they are; with
/// `replace` (`--replace`, a retranslation) the rows already translated get the new English too.
fn expand(
    input: &str,
    export: &str,
    existing: &std::collections::HashSet<String>,
    replace: bool,
) -> Result<String, Error> {
    let mut by_source: std::collections::HashMap<String, String> = Default::default();
    let mut keys: std::collections::HashSet<String> = Default::default();
    let mut out = String::new();
    for (i, line) in input.lines().enumerate() {
        out.push_str(line);
        out.push('\n');
        if i == 0 {
            continue;
        }
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() >= 4 && !c[2].trim().is_empty() {
            by_source
                .entry(c[3].to_string())
                .or_insert_with(|| c[2].to_string());
            keys.insert(c[0].to_string());
        }
    }
    for line in export.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 4
            || keys.contains(c[0])
            || (!replace && (!c[2].is_empty() || existing.contains(c[0])))
        {
            continue;
        }
        if let Some(en) = by_source.get(c[3]) {
            out.push_str(&format!("{}\t{}\t{}\t{}\n", c[0], c[1], en, c[3]));
        }
    }
    Ok(out)
}

fn import(a: &[String]) -> Result<(), Error> {
    let mut input = std::fs::read_to_string(opt(a, "--in").ok_or("--in is required")?)?;
    let repo_dir = PathBuf::from(opt(a, "--repo").unwrap_or_else(|| ".".into()));
    if let Some(e) = opt(a, "--expand-with") {
        // Keys already translated in the repo's files (not the copy compiled into this binary).
        let mut existing = std::collections::HashSet::new();
        for f in std::fs::read_dir(repo_dir.join("server/content/l10n/en"))?.flatten() {
            if f.path().extension().and_then(|x| x.to_str()) == Some("json") {
                let v: Value = serde_json::from_slice(&std::fs::read(f.path())?)?;
                if let Some(m) = v.get("entries").and_then(Value::as_object) {
                    existing.extend(m.keys().cloned());
                }
            }
        }
        let replace = a.iter().any(|x| x == "--replace");
        input = expand(&input, &std::fs::read_to_string(e)?, &existing, replace)?;
    }
    let repo = PathBuf::from(opt(a, "--repo").unwrap_or_else(|| ".".into()));
    let dir = repo.join("server/content/l10n/en");
    let mut files: std::collections::BTreeMap<&str, Value> = Default::default();
    let mut n = 0;
    for (i, line) in input.lines().enumerate().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 {
            return Err(format!(
                "line {}: expected key, hash, english, japanese (the Japanese column is needed for the checks)",
                i + 1
            )
            .into());
        }
        let (key, hash, en) = (cols[0], cols[1], unesc(cols[2]));
        // An empty cell is untranslated; a cell of only spaces is a deliberate blank (a suffix
        // that English does not need, e.g. after a number).
        if en.is_empty() {
            continue;
        }
        if l10n::has_japanese(&en) {
            return Err(format!(
                "line {}: {key}: the English column contains Japanese",
                i + 1
            )
            .into());
        }
        if hash.len() != 8 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("line {}: {key}: bad hash {hash:?}", i + 1).into());
        }
        if let Some(src) = cols.get(3).map(|s| unesc(s)) {
            if l10n::placeholders(&src) != l10n::placeholders(&en) {
                return Err(format!(
                    "line {}: {key}: placeholders differ: source {:?}, English {:?}",
                    i + 1,
                    l10n::placeholders(&src),
                    l10n::placeholders(&en)
                )
                .into());
            }
            if l10n::source_hash(&src) != hash {
                return Err(
                    format!("line {}: {key}: hash does not match the source text", i + 1).into(),
                );
            }
        }
        let name = file_for(key);
        let doc = match files.entry(name) {
            std::collections::btree_map::Entry::Occupied(o) => o.into_mut(),
            std::collections::btree_map::Entry::Vacant(v) => {
                let p = dir.join(format!("{name}.json"));
                v.insert(serde_json::from_slice(&std::fs::read(&p)?)?)
            }
        };
        let entries = doc
            .get_mut("entries")
            .and_then(Value::as_object_mut)
            .ok_or("translation file without entries")?;
        entries.insert(key.to_string(), Value::Array(vec![hash.into(), en.into()]));
        n += 1;
    }
    for (name, mut doc) in files {
        if let Some(e) = doc.get_mut("entries").and_then(Value::as_object_mut) {
            let sorted: Map<String, Value> = std::mem::take(e)
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .collect();
            *e = sorted;
        }
        let p = dir.join(format!("{name}.json"));
        std::fs::write(&p, serde_json::to_string_pretty(&doc)? + "\n")?;
        eprintln!("updated {}", p.display());
    }
    eprintln!("{n} translations imported");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expand_fills_same_source() {
        let input = "key\thash\tenglish\tjapanese\nk1\taaaaaaaa\tSword\t剣\n";
        let export = "key\thash\tenglish\tjapanese\tprevious_english\nk1\taaaaaaaa\t\t剣\t\nk2\taaaaaaaa\t\t剣\t\nk3\tbbbbbbbb\t\t盾\t\n";
        let out = expand(input, export, &Default::default(), false).unwrap();
        assert!(out.contains("k2\taaaaaaaa\tSword\t剣"));
        assert!(!out.contains("k3"));
        assert_eq!(out.matches("k1\t").count(), 1);
        // A row already translated keeps its English unless --replace.
        let export = "key\thash\tenglish\tjapanese\nk2\taaaaaaaa\tBlade\t剣\n";
        assert!(!expand(input, export, &Default::default(), false)
            .unwrap()
            .contains("k2"));
        assert!(expand(input, export, &Default::default(), true)
            .unwrap()
            .contains("k2\taaaaaaaa\tSword\t剣"));
    }

    #[test]
    fn escaping_round_trips() {
        for s in ["a\tb\nc\\d", "", "\\n literal"] {
            assert_eq!(unesc(&esc(s)), s);
        }
        assert_eq!(file_for("clientmes:X"), "clientmes");
        assert_eq!(file_for("ui:ui_mypage:0:1:m_Text"), "ui_prefabs");
        assert_eq!(file_for("script:r_ev_tuto_s01_script_text_jp:3"), "story");
        assert_eq!(file_for("text:MyPage_JP:A"), "text_menu");
        assert_eq!(file_for("text:SkillName_JP:A"), "text_content");
        assert_eq!(file_for("apk:level2:158:m_Text"), "apk_text");
        assert_eq!(file_for("literal:3197"), "apk_text");
        assert!(
            wanted("ui_mypage")
                && wanted("scene_vs")
                && wanted("text.jp")
                && wanted("minigame_01")
                && wanted("r_ev_tuto_s01_script_text_jp")
                && !wanted("r_ev_tuto_s01")
                && !wanted("ui_tips_00")
        );
    }
}
