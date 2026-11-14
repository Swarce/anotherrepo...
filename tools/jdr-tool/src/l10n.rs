//! `jdr-tool l10n`: the translators' round trip.
//!
//! - `export` writes every translatable string from the player's own copy (text.jp, the UI
//!   prefabs, the in-APK ClientMes) to a local TSV: `key, hash, English, Japanese source`. The TSV
//!   contains game text: keep it out of the repo (`/local/` is git-ignored).
//! - `import` merges a TSV's English column into the repo's translation files, which store only
//!   `key -> [hash, English]`.

use crate::{load_keys, opt, Error};
use jdr_crypto::{names, Jjec};
use jdr_services::l10n::{self, Found};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Bundles whose strings we offer (UI prefabs and the text tables; story scripts come later).
pub fn wanted(name: &str) -> bool {
    name == "text.jp"
        || (name.starts_with("ui_") && !name.starts_with("ui_tips"))
        || name.starts_with("scene_")
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
        _ => Err("usage: jdr-tool l10n export --data <files> --keys <keys.json> [--apk <apk>] --out <work.tsv>\n       jdr-tool l10n import --in <work.tsv> --repo <checkout>".into()),
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
        let mut z = zip::ZipArchive::new(std::fs::File::open(apk)?)?;
        let mut file = Vec::new();
        std::io::Read::read_to_end(&mut z.by_name(jdr_patch::clientmes::ENTRY)?, &mut file)?;
        for (label, text) in jdr_patch::clientmes::read(&file)?.rows {
            if l10n::has_japanese(&text) {
                found.push(Found {
                    key: format!("clientmes:{label}"),
                    source: text,
                });
            }
        }
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

/// Which translation file a key belongs to.
pub fn file_for(key: &str) -> &'static str {
    if key.starts_with("clientmes:") {
        "clientmes"
    } else if key.starts_with("ui:") {
        "ui_prefabs"
    } else {
        "text_menu"
    }
}

fn import(a: &[String]) -> Result<(), Error> {
    let input = std::fs::read_to_string(opt(a, "--in").ok_or("--in is required")?)?;
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
        if en.trim().is_empty() {
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
    fn escaping_round_trips() {
        for s in ["a\tb\nc\\d", "", "\\n literal"] {
            assert_eq!(unesc(&esc(s)), s);
        }
        assert_eq!(file_for("clientmes:X"), "clientmes");
        assert_eq!(file_for("ui:ui_mypage:0:1:m_Text"), "ui_prefabs");
        assert_eq!(file_for("text:MyPage_JP:A"), "text_menu");
        assert!(
            wanted("ui_mypage") && wanted("scene_vs") && wanted("text.jp") && !wanted("ui_tips_00")
        );
    }
}
