//! English localization of the game's own text bundles.
//!
//! The game keeps its UI text in `text.jp` (111 `{Label, Text}` tables) and as labels baked into
//! the UI prefabs (`ui_*`/`scene_*` bundles). We rebuild those bundles with the English strings
//! from `server/content/l10n/en/*.json`, list the rebuilt files in the manifest with their new md5
//! and size, and serve them on `/cdn`: the client then downloads them once, through its own update
//! path, and records them in `vcs.bytes`. The language stays "jp"; only the contents change.
//!
//! Translation files hold `key -> [source hash, English]`, never the Japanese text: a translation
//! applies only while the game's source string still hashes the same.
//!
//! Keys:
//! - `text:<Table>:<Label>` for text.jp rows (`Table` is the object name, e.g. `MyPage_JP`; a
//!   label repeated in one table gets `#2`, `#3`, … from its second occurrence);
//! - `ui:<bundle>:<file>:<path id>:<field>` for strings in UI prefabs (`m_Text`, `defaultText`, …;
//!   `file` is the index of the serialized file inside the bundle, since scene bundles hold several);
//! - `clientmes:<Label>` for the in-APK ClientMes table (applied by the patcher).
//!
//! The originals are kept (`<data>/l10n/orig/`) so the English files can always be rebuilt, and
//! reverted, after the client has replaced its copies with ours.

use jdr_crypto::{names, Jjec};
use jdr_unity::bundle;
use jdr_unity::sfile::{self, V};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

type Error = String;

/// Bump when the rewrite logic changes, so cached English files are rebuilt.
pub const GEN_VERSION: u32 = 1;

/// The translation files compiled into the server.
pub const FILES: &[(&str, &str)] = &[
    (
        "clientmes",
        include_str!("../../content/l10n/en/clientmes.json"),
    ),
    (
        "ui_prefabs",
        include_str!("../../content/l10n/en/ui_prefabs.json"),
    ),
    (
        "text_menu",
        include_str!("../../content/l10n/en/text_menu.json"),
    ),
];

/// First 8 hex digits of SHA-256 of the source string (UTF-8).
pub fn source_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(s.as_bytes())[..4]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The `{name}` placeholders the game fills in at runtime (sorted; duplicates kept). A translation
/// must keep exactly the same set.
pub fn placeholders(s: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(j)
                if j > 0
                    && j <= 24
                    && after[..j]
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'_') =>
            {
                v.push(rest[i..i + j + 2].to_string());
                rest = &after[j + 1..];
            }
            _ => rest = after,
        }
    }
    v.sort();
    v
}

/// True when a string contains Japanese script (what we translate).
pub fn has_japanese(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c as u32, 0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xff66..=0xff9f)
    })
}

#[derive(Default, Debug)]
pub struct Translations {
    /// key -> (source hash, English)
    map: HashMap<String, (String, String)>,
    /// Identifies this set of translations (plus the generator version), for cache keys.
    pub digest: String,
}

impl Translations {
    pub fn parse(files: &[(&str, &str)]) -> Result<Self, Error> {
        use sha2::{Digest, Sha256};
        let mut map = HashMap::new();
        let mut h = Sha256::new();
        h.update(GEN_VERSION.to_le_bytes());
        for (name, js) in files {
            h.update(js.as_bytes());
            let v: Value = serde_json::from_str(js).map_err(|e| format!("{name}.json: {e}"))?;
            crate::masterdata::check_sources(&v).map_err(|e| format!("{name}.json: {e}"))?;
            let entries = v
                .get("entries")
                .and_then(Value::as_object)
                .ok_or_else(|| format!("{name}.json: no \"entries\" object"))?;
            for (k, e) in entries {
                let pair = e.as_array().filter(|a| a.len() == 2);
                let (Some(hash), Some(en)) = (
                    pair.and_then(|a| a[0].as_str()),
                    pair.and_then(|a| a[1].as_str()),
                ) else {
                    return Err(format!(
                        "{name}.json: {k}: expected [\"<hash>\", \"<English>\"]"
                    ));
                };
                if hash.len() != 8 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                    return Err(format!("{name}.json: {k}: bad source hash {hash:?}"));
                }
                if has_japanese(en) {
                    return Err(format!(
                        "{name}.json: {k}: the English text contains Japanese"
                    ));
                }
                if map
                    .insert(k.clone(), (hash.to_string(), en.to_string()))
                    .is_some()
                {
                    return Err(format!("{name}.json: {k} defined twice"));
                }
            }
        }
        let digest = h.finalize()[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self { map, digest })
    }

    pub fn embedded() -> Result<Self, Error> {
        Self::parse(FILES)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The English for `key`, if there is one for exactly this source string.
    pub fn lookup(&self, key: &str, source: &str) -> Lookup<'_> {
        match self.map.get(key) {
            None => Lookup::Missing,
            Some((h, en)) if *h == source_hash(source) => Lookup::Found(en),
            Some(_) => Lookup::Stale,
        }
    }

    /// Bundles with at least one prefab translation (from `ui:<bundle>:…` keys).
    pub fn ui_bundles(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .map
            .keys()
            .filter_map(|k| k.strip_prefix("ui:")?.split(':').next().map(str::to_owned))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// All entries with a prefix (e.g. `clientmes:`), for the patcher.
    pub fn with_prefix(&self, prefix: &str) -> BTreeMap<String, (String, String)> {
        self.map
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

pub enum Lookup<'a> {
    Found(&'a str),
    Missing,
    /// A translation exists but the game's source string changed.
    Stale,
}

/// One translatable string found in a bundle.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub key: String,
    pub source: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Stats {
    pub strings: usize,
    pub replaced: usize,
    pub stale: usize,
    pub best_fit: usize,
}

/// Visit every translatable string in a decoded MonoBehaviour. `edit` returns the replacement.
fn visit_object(
    bundle_name: &str,
    file_index: usize,
    path_id: i64,
    v: &mut V,
    edit: &mut dyn FnMut(&str, &str) -> Option<String>,
    stats: &mut Stats,
) -> bool {
    let mut changed = false;
    let table = v.get("m_Name").and_then(V::as_str).map(str::to_owned);
    // text.jp tables: {m_Name, Contents: [{Label, Text}]}
    if let (Some(table), Some(V::Arr(rows))) = (table.clone(), v.get_mut("Contents")) {
        // Some tables repeat a label; later occurrences get `#2`, `#3`, … so every row has a key.
        let mut seen: HashMap<String, usize> = HashMap::new();
        for row in rows {
            let label = row.get("Label").and_then(V::as_str).map(str::to_owned);
            let text = row.get("Text").and_then(V::as_str).map(str::to_owned);
            let (Some(label), Some(text)) = (label, text) else {
                continue;
            };
            let n = seen.entry(label.clone()).or_insert(0);
            *n += 1;
            if !has_japanese(&text) {
                continue;
            }
            let key = if *n == 1 {
                format!("text:{table}:{label}")
            } else {
                format!("text:{table}:{label}#{n}")
            };
            stats.strings += 1;
            if let Some(en) = edit(&key, &text) {
                *row.get_mut("Text").expect("Text") = V::Str(en.into_bytes());
                stats.replaced += 1;
                changed = true;
            }
        }
        return changed;
    }
    // Prefab components: top-level string fields with Japanese (m_Text, defaultText, …).
    let mut replaced_text = false;
    if let V::Obj(fields) = v {
        for (name, fv) in fields.iter_mut() {
            if name == "m_Name" {
                continue;
            }
            let Some(s) = fv.as_str().filter(|s| has_japanese(s)).map(str::to_owned) else {
                continue;
            };
            stats.strings += 1;
            if let Some(en) = edit(
                &format!("ui:{bundle_name}:{file_index}:{path_id}:{name}"),
                &s,
            ) {
                *fv = V::Str(en.into_bytes());
                stats.replaced += 1;
                changed = true;
                replaced_text |= name == "m_Text";
            }
        }
    }
    // English runs longer than Japanese: let a UI.Text we changed shrink to fit its box.
    if replaced_text {
        if let Some(fd) = v.get_mut("m_FontData") {
            let size = fd.get("m_FontSize").and_then(V::as_i64);
            let best = fd.get("m_BestFit").and_then(V::as_i64);
            if let (Some(size), Some(0)) = (size, best) {
                let min = (size / 2).max(8).min(size);
                *fd.get_mut("m_BestFit").expect("m_BestFit") = V::U(1);
                if let Some(x) = fd.get_mut("m_MinSize") {
                    *x = V::I(min);
                }
                if let Some(x) = fd.get_mut("m_MaxSize") {
                    *x = V::I(size);
                }
                stats.best_fit += 1;
            }
        }
    }
    changed
}

/// Walk a (decrypted) bundle; returns the rewritten bundle if anything changed.
pub fn rewrite_bundle(
    data: &[u8],
    bundle_name: &str,
    edit: &mut dyn FnMut(&str, &str) -> Option<String>,
) -> Result<(Option<Vec<u8>>, Stats), Error> {
    let mut nodes = bundle::read(data)?;
    let mut stats = Stats::default();
    let mut any = false;
    for (file_index, node) in nodes.iter_mut().filter(|n| n.flags & 4 != 0).enumerate() {
        let mut f = sfile::parse(&node.data)?;
        let mut file_changed = false;
        for i in 0..f.objects.len() {
            let t = f.objects[i].type_index as usize;
            if f.types[t].entry.class_id != jdr_unity::CLASS_MONOBEHAVIOUR {
                continue;
            }
            let nodes_t = &f.types[t].nodes;
            let Ok(mut v) = sfile::read_object(nodes_t, &f.objects[i].data) else {
                continue; // not decodable through its tree: leave it alone
            };
            if visit_object(
                bundle_name,
                file_index,
                f.objects[i].path_id,
                &mut v,
                edit,
                &mut stats,
            ) {
                f.objects[i].data = sfile::write_object(nodes_t, &v)?;
                file_changed = true;
            }
        }
        if file_changed {
            node.data = f.write();
            any = true;
        }
    }
    Ok((
        any.then(|| bundle::write_lz4(&nodes, jdr_unity::UNITY_VERSION)),
        stats,
    ))
}

/// Every translatable string in a bundle (for the translators' export).
pub fn collect(data: &[u8], bundle_name: &str) -> Result<Vec<Found>, Error> {
    let mut out = Vec::new();
    rewrite_bundle(data, bundle_name, &mut |k, s| {
        out.push(Found {
            key: k.to_string(),
            source: s.to_string(),
        });
        None
    })?;
    Ok(out)
}

/// Apply translations to a bundle.
pub fn translate_bundle(
    data: &[u8],
    bundle_name: &str,
    tr: &Translations,
) -> Result<(Option<Vec<u8>>, Stats), Error> {
    let mut stale = 0;
    let (out, mut st) = rewrite_bundle(data, bundle_name, &mut |k, s| match tr.lookup(k, s) {
        Lookup::Found(en) => Some(en.to_string()),
        Lookup::Stale => {
            stale += 1;
            None
        }
        Lookup::Missing => None,
    })?;
    st.stale = stale;
    Ok((out, st))
}

// ---------------------------------------------------------------------------------------------
// On-device preparation: originals, cache, what the manifest lists.

/// A file we serve instead of the game's own copy.
#[derive(Debug, Clone)]
pub struct Localized {
    pub name: String,
    pub obf: String,
    pub file: PathBuf,
    pub md5: String,
    pub size: i64,
    pub encrypted: bool,
    pub stats: Stats,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct StateEntry {
    src_md5: String,
    digest: String,
    out_md5: String,
    out_size: i64,
}

fn md5_hex(b: &[u8]) -> String {
    crate::versionfile::md5_hex(b)
}

/// The bundles we localize: text.jp plus every UI bundle with a prefab translation.
pub fn targets(tr: &Translations) -> Vec<String> {
    let mut v = vec!["text.jp".to_string()];
    v.extend(tr.ui_bundles());
    v
}

/// Build (or reuse) the English versions of the target bundles.
///
/// `game_dir` is the game's own folder (read-only here); `state_dir` is ours. Per target: if the
/// file on disk is not one we produced, it is an original and is kept under `orig/`; the English
/// file is rebuilt when the original, the translations or the generator changed.
pub fn prepare(
    game_dir: &Path,
    state_dir: &Path,
    asset_key: &[u8],
    asset_iv: &[u8],
    tr: &Translations,
) -> (Vec<Localized>, Vec<String>) {
    let root = state_dir.join("l10n");
    let orig = root.join("orig");
    let gen = root.join("gen");
    let state_path = root.join("state.json");
    let mut state: BTreeMap<String, StateEntry> = std::fs::read(&state_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    for name in targets(tr) {
        let obf = format!("{}.bytes", names::obfuscate(&name, asset_key));
        match prepare_one(
            &name, &obf, game_dir, &orig, &gen, asset_key, asset_iv, tr, &mut state,
        ) {
            Ok(Some(l)) => out.push(l),
            Ok(None) => {}
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    let _ = std::fs::create_dir_all(&root);
    if let Ok(js) = serde_json::to_vec_pretty(&state) {
        let _ = jdr_core::storage::write_atomic(&state_path, &js);
    }
    (out, errors)
}

#[allow(clippy::too_many_arguments)]
fn prepare_one(
    name: &str,
    obf: &str,
    game_dir: &Path,
    orig_dir: &Path,
    gen_dir: &Path,
    key: &[u8],
    iv: &[u8],
    tr: &Translations,
    state: &mut BTreeMap<String, StateEntry>,
) -> Result<Option<Localized>, Error> {
    let disk_path = game_dir.join(obf);
    let Ok(disk) = std::fs::read(&disk_path) else {
        return Ok(None); // not on this device: nothing to replace
    };
    let disk_md5 = md5_hex(&disk);
    let prev = state.get(obf).cloned();
    let orig_path = orig_dir.join(obf);
    // Which bytes are the original?
    let source = if prev.as_ref().is_some_and(|p| p.out_md5 == disk_md5) {
        std::fs::read(&orig_path).map_err(|e| {
            format!("original copy missing ({e}); restore the game's files to rebuild")
        })?
    } else {
        std::fs::create_dir_all(orig_dir).map_err(|e| e.to_string())?;
        if std::fs::read(&orig_path).ok().as_deref() != Some(&disk[..]) {
            jdr_core::storage::write_atomic(&orig_path, &disk).map_err(|e| e.to_string())?;
        }
        disk
    };
    let src_md5 = md5_hex(&source);
    let gen_path = gen_dir.join(obf);
    if let Some(p) = prev.filter(|p| p.src_md5 == src_md5 && p.digest == tr.digest) {
        if std::fs::metadata(&gen_path).map(|m| m.len() as i64).ok() == Some(p.out_size) {
            return Ok(Some(Localized {
                name: name.into(),
                obf: obf.into(),
                file: gen_path,
                md5: p.out_md5,
                size: p.out_size,
                encrypted: false,
                stats: Stats::default(),
            }));
        }
    }
    let plain = if Jjec::is_jjec(&source) {
        Jjec::decrypt(&source, key, iv).map_err(|e| format!("decrypt: {e}"))?
    } else {
        source.clone()
    };
    let (rewritten, stats) = translate_bundle(&plain, name, tr)?;
    // Nothing to change: serve the original as is (also reverts a copy we replaced earlier).
    let (bytes, encrypted) = match rewritten {
        Some(b) => (b, false),
        None => (source.clone(), Jjec::is_jjec(&source)),
    };
    std::fs::create_dir_all(gen_dir).map_err(|e| e.to_string())?;
    jdr_core::storage::write_atomic(&gen_path, &bytes).map_err(|e| e.to_string())?;
    let out_md5 = md5_hex(&bytes);
    state.insert(
        obf.to_string(),
        StateEntry {
            src_md5,
            digest: tr.digest.clone(),
            out_md5: out_md5.clone(),
            out_size: bytes.len() as i64,
        },
    );
    Ok(Some(Localized {
        name: name.into(),
        obf: obf.into(),
        file: gen_path,
        md5: out_md5,
        size: bytes.len() as i64,
        encrypted,
        stats,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jdr_unity::managed::{self, Ty, Val};
    use jdr_unity::serialized::{self, Object};

    fn text_bundle(rows: &[(&str, &str)]) -> Vec<u8> {
        let row = Ty::Struct {
            name: "TextContent".into(),
            fields: vec![("Label".into(), Ty::Str), ("Text".into(), Ty::Str)],
        };
        let fields = vec![("Contents".into(), Ty::List(Box::new(row)))];
        let vals = vec![Val::List(
            rows.iter()
                .map(|(l, t)| Val::Struct(vec![Val::Str((*l).into()), Val::Str((*t).into())]))
                .collect(),
        )];
        let ty = managed::mono_behaviour_type(&fields, 0, [3; 16]);
        let data = managed::mono_behaviour_data(2, "MyPage_JP", &fields, &vals).unwrap();
        let sf = serialized::write(
            "2018.4.1f1",
            13,
            &[ty],
            &[Object {
                path_id: 7,
                type_index: 0,
                data,
            }],
            &[(0, 2)],
        );
        bundle::write(
            &[bundle::Node {
                name: "CAB-x".into(),
                data: sf,
                flags: 4,
            }],
            "2018.4.1f1",
        )
    }

    fn tr(entries: &[(&str, &str, &str)]) -> Translations {
        let e: serde_json::Map<String, Value> = entries
            .iter()
            .map(|(k, src, en)| (k.to_string(), serde_json::json!([source_hash(src), en])))
            .collect();
        let js = serde_json::json!({"sources":[{"source":"jdr-default","note":"t"}],"entries":e})
            .to_string();
        Translations::parse(&[("t", js.as_str())]).unwrap()
    }

    #[test]
    fn placeholder_sets() {
        assert_eq!(placeholders("{Param0}を{time}秒"), ["{Param0}", "{time}"]);
        assert_eq!(placeholders("{ x} {} {a-b} {ok}"), ["{ok}"]);
    }

    #[test]
    fn hashes_and_script_detection() {
        assert_eq!(source_hash("abc"), "ba7816bf");
        assert!(has_japanese("マイページ") && has_japanese("決定") && !has_japanese("OK 123"));
    }

    #[test]
    fn translates_text_tables_by_label_and_hash() {
        let b = text_bundle(&[("A", "決定"), ("B", "戻る"), ("C", "123")]);
        let found = collect(&b, "text.jp").unwrap();
        assert_eq!(
            found.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
            ["text:MyPage_JP:A", "text:MyPage_JP:B"],
            "non-Japanese rows are not offered"
        );
        let t = tr(&[
            ("text:MyPage_JP:A", "決定", "OK"),
            ("text:MyPage_JP:B", "変わった", "Back"), // source changed: stale
        ]);
        let (out, st) = translate_bundle(&b, "text.jp", &t).unwrap();
        assert_eq!((st.strings, st.replaced, st.stale), (2, 1, 1));
        let out = out.unwrap();
        let again = collect(&out, "text.jp").unwrap();
        assert_eq!(again.len(), 1, "only the stale row is still Japanese");
        // Nothing to translate: no rewrite.
        assert!(translate_bundle(&b, "text.jp", &tr(&[]))
            .unwrap()
            .0
            .is_none());
    }

    #[test]
    fn translation_file_rules() {
        let bad = r#"{"sources":[{"source":"jdr-default","note":"n"}],"entries":{"k":["zz","x"]}}"#;
        assert!(Translations::parse(&[("b", bad)]).is_err());
        let jp = r#"{"sources":[{"source":"jdr-default","note":"n"}],"entries":{"k":["0011aabb","決定"]}}"#;
        assert!(Translations::parse(&[("b", jp)])
            .unwrap_err()
            .contains("Japanese"));
        let ok = r#"{"sources":[{"source":"jdr-default","note":"n"}],"entries":{"ui:ui_mypage:0:5:m_Text":["0011aabb","Menu"]}}"#;
        let t = Translations::parse(&[("a", ok)]).unwrap();
        assert_eq!(t.ui_bundles(), ["ui_mypage"]);
        assert!(Translations::parse(&[("a", ok), ("b", ok)])
            .unwrap_err()
            .contains("twice"));
        Translations::embedded().unwrap();
    }

    #[test]
    fn prepare_keeps_the_original_and_reuses_the_cache() {
        let base = std::env::temp_dir().join(format!("jdr-l10n-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (game, state) = (base.join("files"), base.join("state"));
        std::fs::create_dir_all(&game).unwrap();
        let key = b"0123456789abcdef";
        let obf = format!("{}.bytes", names::obfuscate("text.jp", key));
        let original = text_bundle(&[("A", "決定")]);
        std::fs::write(game.join(&obf), &original).unwrap();
        let t = tr(&[("text:MyPage_JP:A", "決定", "OK")]);
        let (l, errs) = prepare(&game, &state, key, key, &t);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].stats.replaced, 1);
        let english = std::fs::read(&l[0].file).unwrap();
        assert_eq!(md5_hex(&english), l[0].md5);
        // The client downloads ours: the disk now holds the English file.
        std::fs::write(game.join(&obf), &english).unwrap();
        let (l2, _) = prepare(&game, &state, key, key, &t);
        assert_eq!(
            l2[0].md5, l[0].md5,
            "rebuilt from the kept original, same output"
        );
        assert_eq!(
            std::fs::read(state.join("l10n/orig").join(&obf)).unwrap(),
            original
        );
        // Translations removed: the original is served again (revert).
        let (l3, _) = prepare(&game, &state, key, key, &tr(&[]));
        assert_eq!(l3[0].md5, md5_hex(&original));
        let _ = std::fs::remove_dir_all(&base);
    }
}
