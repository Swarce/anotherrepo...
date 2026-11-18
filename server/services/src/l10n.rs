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
//! - `clientmes:<Label>` for the in-APK ClientMes table (applied by the patcher);
//! - `script:<bundle>:<line>` for the story scripts' dialogue lines (`*_script_text_jp` bundles:
//!   one MonoBehaviour `{m_Name: "<name>_script_text_JP", text: [line, …]}`; `line` counts from 0);
//! - `apk:<file>:<path id>:m_Text` for `UI.Text` labels in the scenes built into the APK
//!   (`assets/bin/Data/level<N>`), and `literal:<index>` for player-visible C# string literals in
//!   `global-metadata.dat` (both applied by the patcher, `english-apk-text`).
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
pub const GEN_VERSION: u32 = 3;

/// UI.Text layout hints (`tools/l10n/ui_fit.py`): label ids `bundle:file:pathId`, no game text.
pub const LAYOUT: &str = include_str!("../../content/l10n/ui_layout.json");

/// Labels that get best fit (they wrap and truncate) and labels that get wrap, truncate and best
/// fit (they spilled sideways with our English).
struct Layout {
    best_fit: std::collections::HashSet<String>,
    wrap: std::collections::HashSet<String>,
}

fn layout() -> &'static Layout {
    static CELL: std::sync::OnceLock<Layout> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let v: Value = serde_json::from_str(LAYOUT).expect("ui_layout.json");
        let set = |k: &str| {
            v[k].as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        Layout {
            best_fit: set("bestFit"),
            wrap: set("wrap"),
        }
    })
}

/// Set an integer field, keeping its stored kind (signed or unsigned).
fn set_int(v: &mut V, n: i64) {
    *v = match v {
        V::U(_) => V::U(n.max(0) as u64),
        _ => V::I(n),
    };
}

/// The translation files compiled into the server.
pub const FILES: &[(&str, &str)] = &[
    (
        "clientmes",
        include_str!("../../content/l10n/en/clientmes.json"),
    ),
    (
        "apk_text",
        include_str!("../../content/l10n/en/apk_text.json"),
    ),
    (
        "ui_prefabs",
        include_str!("../../content/l10n/en/ui_prefabs.json"),
    ),
    (
        "text_menu",
        include_str!("../../content/l10n/en/text_menu.json"),
    ),
    (
        "text_content",
        include_str!("../../content/l10n/en/text_content.json"),
    ),
    ("story", include_str!("../../content/l10n/en/story.json")),
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

    /// The English stored for `key`, whatever source it was made for (translator reference).
    pub fn english(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(|(_, en)| en.as_str())
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

    /// Story script bundles with at least one translated line (from `script:<bundle>:…` keys).
    pub fn script_bundles(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .map
            .keys()
            .filter_map(|k| {
                k.strip_prefix("script:")?
                    .split(':')
                    .next()
                    .map(str::to_owned)
            })
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
    /// Sprites replaced with redrawn art.
    pub art: usize,
    /// Images in the art pack that matched no sprite of this bundle.
    pub art_unmatched: usize,
    /// Sprites or textures of the art pack that could not be applied (skipped).
    pub art_errors: Vec<String>,
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
    // Story script dialogue: {m_Name: "<name>_script_text_JP", text: [line, …]}.
    if table
        .as_deref()
        .is_some_and(|t| t.ends_with("_script_text_JP"))
    {
        if let Some(V::Arr(lines)) = v.get_mut("text") {
            for (i, line) in lines.iter_mut().enumerate() {
                let Some(s) = line.as_str().filter(|s| has_japanese(s)).map(str::to_owned) else {
                    continue;
                };
                stats.strings += 1;
                if let Some(en) = edit(&format!("script:{bundle_name}:{i}"), &s) {
                    *line = V::Str(en.into_bytes());
                    stats.replaced += 1;
                    changed = true;
                }
            }
            return changed;
        }
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
    // English runs longer than Japanese: let a UI.Text shrink to fit its box. That is every label
    // we changed, and the labels the layout hints list: those that wrap and truncate (the game
    // fills many at runtime from text.jp), and those that spilled sideways with our English, which
    // first get wrap and truncate so best fit has a box to fit.
    let id = format!("{bundle_name}:{file_index}:{path_id}");
    let hints = layout();
    let wrap = hints.wrap.contains(&id);
    if replaced_text || wrap || hints.best_fit.contains(&id) {
        if let Some(fd) = v.get_mut("m_FontData") {
            if wrap {
                for k in ["m_HorizontalOverflow", "m_VerticalOverflow"] {
                    if let Some(x) = fd.get_mut(k) {
                        if x.as_i64() != Some(0) {
                            set_int(x, 0);
                            changed = true;
                        }
                    }
                }
            }
            let size = fd.get("m_FontSize").and_then(V::as_i64);
            let best = fd.get("m_BestFit").and_then(V::as_i64);
            if let (Some(size @ 1..), Some(0)) = (size, best) {
                let min = (size / 2).max(8).min(size);
                *fd.get_mut("m_BestFit").expect("m_BestFit") = V::U(1);
                if let Some(x) = fd.get_mut("m_MinSize") {
                    *x = V::I(min);
                }
                if let Some(x) = fd.get_mut("m_MaxSize") {
                    *x = V::I(size);
                }
                stats.best_fit += 1;
                changed = true;
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
    art: Option<&HashMap<String, &[u8]>>,
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
    if let Some(images) = art {
        // Art problems never cost the bundle its English: bad images are skipped and reported.
        match crate::art::apply(&mut nodes, images) {
            Ok(r) => {
                stats.art += r.replaced;
                stats.art_unmatched += r.unmatched.len();
                stats.art_errors.extend(r.errors);
                any |= r.replaced > 0;
            }
            Err(e) => stats.art_errors.push(format!("art: {e}")),
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
    rewrite_bundle(
        data,
        bundle_name,
        &mut |k, s| {
            out.push(Found {
                key: k.to_string(),
                source: s.to_string(),
            });
            None
        },
        None,
    )?;
    Ok(out)
}

/// Apply translations to a bundle.
pub fn translate_bundle(
    data: &[u8],
    bundle_name: &str,
    tr: &Translations,
    art: Option<&HashMap<String, &[u8]>>,
) -> Result<(Option<Vec<u8>>, Stats), Error> {
    let mut stale = 0;
    let (out, mut st) = rewrite_bundle(
        data,
        bundle_name,
        &mut |k, s| match tr.lookup(k, s) {
            Lookup::Found(en) => Some(en.to_string()),
            Lookup::Stale => {
                stale += 1;
                None
            }
            Lookup::Missing => None,
        },
        art,
    )?;
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
    /// The game's folder already holds exactly this file (downloaded on an earlier boot).
    pub on_device: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct StateEntry {
    /// Bundle name (so a bundle that lost its last translation is still reverted).
    #[serde(default)]
    name: String,
    src_md5: String,
    digest: String,
    out_md5: String,
    out_size: i64,
    /// Modification time of the generated file when we wrote it (seconds), to detect tampering.
    #[serde(default)]
    out_mtime: u64,
    #[serde(default)]
    encrypted: bool,
    /// md5 of every file we ever produced for this name (newest last): a copy of any of them on
    /// disk is ours, never an original.
    #[serde(default)]
    outputs: Vec<String>,
}

/// How many past outputs we remember per file.
const MAX_OUTPUTS: usize = 32;

fn mtime_secs(p: &Path) -> Option<u64> {
    std::fs::metadata(p)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

fn md5_hex(b: &[u8]) -> String {
    crate::versionfile::md5_hex(b)
}

/// Whether `bytes` can be an original: it decrypts, parses, and still holds Japanese strings.
fn looks_original(bytes: &[u8], name: &str, key: &[u8], iv: &[u8]) -> bool {
    let plain = if Jjec::is_jjec(bytes) {
        match Jjec::decrypt(bytes, key, iv) {
            Ok(p) => p,
            Err(_) => return false,
        }
    } else {
        bytes.to_vec()
    };
    collect(&plain, name).is_ok_and(|f| !f.is_empty())
}

/// Whether `bytes` decrypts and parses as a bundle.
fn parses(bytes: &[u8], key: &[u8], iv: &[u8]) -> bool {
    if Jjec::is_jjec(bytes) {
        Jjec::decrypt(bytes, key, iv).is_ok_and(|p| bundle::read(&p).is_ok())
    } else {
        bundle::read(bytes).is_ok()
    }
}

/// The bundles we localize: text.jp plus every UI bundle with a prefab translation and every story
/// script bundle with a translated line.
pub fn targets(tr: &Translations) -> Vec<String> {
    let mut v = vec!["text.jp".to_string()];
    v.extend(tr.ui_bundles());
    v.extend(tr.script_bundles());
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
    art: Option<&crate::art::ArtPack>,
) -> (Vec<Localized>, Vec<String>) {
    let root = state_dir.join("l10n");
    // Cache key: the translations and the art pack (an English file is rebuilt when either changes).
    let digest = match art {
        Some(a) if !a.is_empty() => format!("{}+art{}", tr.digest, a.digest),
        _ => tr.digest.clone(),
    };
    let orig = root.join("orig");
    let gen = root.join("gen");
    let state_path = root.join("state.json");
    let mut state: BTreeMap<String, StateEntry> = std::fs::read(&state_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    // Current targets plus every bundle we replaced before (removing its last translation must
    // still serve the original again).
    let mut names = targets(tr);
    for b in art.map(|a| a.bundles()).unwrap_or_default() {
        if !names.contains(&b) {
            names.push(b);
        }
    }
    for e in state.values() {
        if !e.name.is_empty() && !names.contains(&e.name) {
            names.push(e.name.clone());
        }
    }
    // Bundles are independent: rebuild them on a few threads (a first build with art decodes
    // dozens of atlases; the client waits for the manifest meanwhile).
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    // With art, each job holds several decoded atlases (16 MB each): fewer at a time.
    let max_workers = if art.is_some_and(|a| !a.is_empty()) {
        2
    } else {
        4
    };
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(1, max_workers);
    std::thread::scope(|sc| {
        for _ in 0..workers {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(name) = names.get(i) else { break };
                let obf = format!("{}.bytes", names::obfuscate(name, asset_key));
                let images = art.map(|a| a.for_bundle(name)).filter(|m| !m.is_empty());
                let job = Job {
                    name,
                    obf: &obf,
                    tr,
                    digest: &digest,
                    art: images.as_ref(),
                };
                let prev = state.get(&obf).cloned();
                // A panic in one bundle must not take the others (or the server) down.
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    prepare_one(&job, game_dir, &orig, &gen, asset_key, asset_iv, prev)
                }))
                .unwrap_or_else(|_| Err("rebuild panicked".into()));
                results
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push((i, name.clone(), obf, r));
            });
        }
    });
    let mut results = results.into_inner().unwrap_or_else(|e| e.into_inner());
    results.sort_by_key(|r| r.0);
    for (_, name, obf, r) in results {
        match r {
            Ok((l, entry)) => {
                if let Some(e) = entry {
                    state.insert(obf, e);
                }
                out.extend(l);
            }
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    // Without a saved state, the next boot could not tell our files from originals: in that case
    // list nothing (the game keeps its current files) rather than risk losing an original.
    let saved = std::fs::create_dir_all(&root).and_then(|_| {
        let js = serde_json::to_vec_pretty(&state).map_err(std::io::Error::other)?;
        jdr_core::storage::write_atomic(&state_path, &js)
    });
    if let Err(e) = saved {
        errors.push(format!(
            "cannot save {}: {e}; English files not offered this time",
            state_path.display()
        ));
        return (Vec::new(), errors);
    }
    (out, errors)
}

/// One bundle to prepare.
struct Job<'a> {
    name: &'a str,
    obf: &'a str,
    tr: &'a Translations,
    /// Cache key for this run (translations + art).
    digest: &'a str,
    art: Option<&'a HashMap<String, &'a [u8]>>,
}

fn prepare_one(
    job: &Job,
    game_dir: &Path,
    orig_dir: &Path,
    gen_dir: &Path,
    key: &[u8],
    iv: &[u8],
    prev: Option<StateEntry>,
) -> Result<(Option<Localized>, Option<StateEntry>), Error> {
    let (name, obf, tr) = (job.name, job.obf, job.tr);
    let disk_path = game_dir.join(obf);
    let Ok(disk) = std::fs::read(&disk_path) else {
        return Ok((None, None)); // not on this device: nothing to replace
    };
    let disk_md5 = md5_hex(&disk);
    let orig_path = orig_dir.join(obf);
    let ours = |md5: &str| {
        prev.as_ref()
            .is_some_and(|p| p.out_md5 == md5 || p.outputs.iter().any(|o| o == md5))
    };
    // Which bytes are the original? A file we produced (now or earlier) never is; anything else on
    // disk is (a first run, or the game's files restored from a backup).
    let source = if ours(&disk_md5) {
        std::fs::read(&orig_path).map_err(|e| {
            format!("original copy missing ({e}); restore the game's files to rebuild")
        })?
    } else {
        std::fs::create_dir_all(orig_dir).map_err(|e| e.to_string())?;
        let kept = std::fs::read(&orig_path).ok();
        if kept.as_deref() == Some(&disk[..]) {
            disk
        } else if let Some(kept) = kept.filter(|k| {
            // A bundle without Japanese text (art only) cannot be told apart by its strings: there
            // any file that parses counts as an original.
            let text_bundle = looks_original(k, name, key, iv);
            if text_bundle {
                !looks_original(&disk, name, key, iv)
            } else {
                !parses(&disk, key, iv)
            }
        }) {
            // A half-written download, or an English copy from a backup: keep our original.
            kept
        } else {
            jdr_core::storage::write_atomic(&orig_path, &disk).map_err(|e| e.to_string())?;
            disk
        }
    };
    let src_md5 = md5_hex(&source);
    let gen_path = gen_dir.join(obf);
    if let Some(p) = prev
        .clone()
        .filter(|p| p.src_md5 == src_md5 && p.digest == job.digest)
    {
        let same_size =
            std::fs::metadata(&gen_path).map(|m| m.len() as i64).ok() == Some(p.out_size);
        let same_time = p.out_mtime != 0 && mtime_secs(&gen_path) == Some(p.out_mtime);
        if same_size && same_time {
            let md5_on_device = p.out_md5.clone();
            return Ok((
                Some(Localized {
                    name: name.into(),
                    obf: obf.into(),
                    file: gen_path,
                    md5: p.out_md5,
                    size: p.out_size,
                    encrypted: p.encrypted,
                    stats: Stats::default(),
                    on_device: disk_md5 == md5_on_device,
                }),
                None,
            ));
        }
    }
    let plain = if Jjec::is_jjec(&source) {
        Jjec::decrypt(&source, key, iv).map_err(|e| format!("decrypt: {e}"))?
    } else {
        source.clone()
    };
    let (rewritten, stats) = translate_bundle(&plain, name, tr, job.art)?;
    // Nothing to change: serve the original as is (also reverts a copy we replaced earlier).
    let (bytes, encrypted) = match rewritten {
        Some(b) => (b, false),
        None => (source.clone(), Jjec::is_jjec(&source)),
    };
    std::fs::create_dir_all(gen_dir).map_err(|e| e.to_string())?;
    jdr_core::storage::write_atomic(&gen_path, &bytes).map_err(|e| e.to_string())?;
    let out_md5 = md5_hex(&bytes);
    let on_device = disk_md5 == out_md5;
    let mut outputs = prev.map(|p| p.outputs).unwrap_or_default();
    // The original itself is never recorded as "ours" (a revert serves it unchanged).
    if out_md5 != src_md5 && !outputs.contains(&out_md5) {
        outputs.push(out_md5.clone());
        if outputs.len() > MAX_OUTPUTS {
            outputs.remove(0);
        }
    }
    let entry = StateEntry {
        name: name.to_string(),
        src_md5,
        digest: job.digest.to_string(),
        out_md5: out_md5.clone(),
        out_size: bytes.len() as i64,
        out_mtime: mtime_secs(&gen_path).unwrap_or(0),
        encrypted,
        outputs,
    };
    Ok((
        Some(Localized {
            name: name.into(),
            obf: obf.into(),
            file: gen_path,
            md5: out_md5,
            size: bytes.len() as i64,
            encrypted,
            stats,
            on_device,
        }),
        Some(entry),
    ))
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

    fn label(h_overflow: i64, v_overflow: i64) -> V {
        let fd = V::Obj(vec![
            ("m_FontSize".into(), V::I(20)),
            ("m_BestFit".into(), V::U(0)),
            ("m_MinSize".into(), V::I(10)),
            ("m_MaxSize".into(), V::I(40)),
            ("m_HorizontalOverflow".into(), V::I(h_overflow)),
            ("m_VerticalOverflow".into(), V::I(v_overflow)),
        ]);
        V::Obj(vec![
            ("m_Name".into(), V::Str(Vec::new())),
            ("m_FontData".into(), fd),
            ("m_Text".into(), V::Str(b"123".to_vec())),
        ])
    }

    #[test]
    fn layout_hints_fit_labels() {
        let doc: Value = serde_json::from_str(LAYOUT).unwrap();
        let first = |k: &str| doc[k][0].as_str().unwrap().to_owned();
        let parse = |id: &str| {
            let p: Vec<&str> = id.split(':').collect();
            (
                p[0].to_owned(),
                p[1].parse::<usize>().unwrap(),
                p[2].parse::<i64>().unwrap(),
            )
        };
        let fd = |v: &V, k: &str| {
            v.get("m_FontData")
                .unwrap()
                .get(k)
                .unwrap()
                .as_i64()
                .unwrap()
        };
        let mut st = Stats::default();
        let mut none = |_: &str, _: &str| None;
        // A runtime label that wraps: best fit, down to half size.
        let (b, f, p) = parse(&first("bestFit"));
        let mut v = label(0, 0);
        assert!(visit_object(&b, f, p, &mut v, &mut none, &mut st));
        assert_eq!(
            (
                fd(&v, "m_BestFit"),
                fd(&v, "m_MinSize"),
                fd(&v, "m_MaxSize")
            ),
            (1, 10, 20)
        );
        // A label that spilled sideways: wrap and truncate first.
        let (b, f, p) = parse(&first("wrap"));
        let mut v = label(1, 1);
        assert!(visit_object(&b, f, p, &mut v, &mut none, &mut st));
        assert_eq!(
            (
                fd(&v, "m_HorizontalOverflow"),
                fd(&v, "m_VerticalOverflow"),
                fd(&v, "m_BestFit")
            ),
            (0, 0, 1)
        );
        // Not listed and not translated: untouched.
        let mut v = label(1, 1);
        assert!(!visit_object("ui_none", 0, 1, &mut v, &mut none, &mut st));
        assert_eq!(fd(&v, "m_BestFit"), 0);
        // Every hinted label is in a bundle the server rebuilds.
        let targets = targets(&Translations::embedded().unwrap());
        for k in ["bestFit", "wrap"] {
            for id in doc[k].as_array().unwrap() {
                let (b, _, _) = parse(id.as_str().unwrap());
                assert!(targets.contains(&b), "{b} is rebuilt");
            }
        }
    }

    #[test]
    fn translates_story_script_lines_by_index() {
        let mut v = V::Obj(vec![
            ("m_Name".into(), V::Str(b"r_ev_x_script_text_JP".to_vec())),
            (
                "text".into(),
                V::Arr(vec![
                    V::Str("波紋だ".as_bytes().to_vec()),
                    V::Str(b"...".to_vec()),
                    V::Str("無駄".as_bytes().to_vec()),
                ]),
            ),
        ]);
        let mut seen = Vec::new();
        let mut st = Stats::default();
        let mut edit = |k: &str, _: &str| {
            seen.push(k.to_string());
            (k.ends_with(":2")).then(|| "Useless!".to_string())
        };
        assert!(visit_object(
            "r_ev_x_script_text_jp",
            0,
            1,
            &mut v,
            &mut edit,
            &mut st
        ));
        assert_eq!(
            seen,
            [
                "script:r_ev_x_script_text_jp:0",
                "script:r_ev_x_script_text_jp:2"
            ],
            "lines without Japanese are skipped but keep their index"
        );
        assert_eq!((st.strings, st.replaced), (2, 1));
        let V::Arr(lines) = v.get("text").unwrap() else {
            panic!()
        };
        assert_eq!(lines[2].as_str(), Some("Useless!"));
        assert_eq!(lines[0].as_str(), Some("波紋だ"));
        let t = Translations::parse(&[(
            "story",
            r#"{"sources":[{"source":"jdr-default","note":"n"}],"entries":{"script:r_ev_x_script_text_jp:2":["0011aabb","Useless!"]}}"#,
        )])
        .unwrap();
        assert_eq!(t.script_bundles(), ["r_ev_x_script_text_jp"]);
        assert!(targets(&t).contains(&"r_ev_x_script_text_jp".to_string()));
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
        let (out, st) = translate_bundle(&b, "text.jp", &t, None).unwrap();
        assert_eq!((st.strings, st.replaced, st.stale), (2, 1, 1));
        let out = out.unwrap();
        let again = collect(&out, "text.jp").unwrap();
        assert_eq!(again.len(), 1, "only the stale row is still Japanese");
        // Nothing to translate: no rewrite.
        assert!(translate_bundle(&b, "text.jp", &tr(&[]), None)
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
        let (l, errs) = prepare(&game, &state, key, key, &t, None);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].stats.replaced, 1);
        let english = std::fs::read(&l[0].file).unwrap();
        assert_eq!(md5_hex(&english), l[0].md5);
        // The client downloads ours: the disk now holds the English file.
        std::fs::write(game.join(&obf), &english).unwrap();
        let (l2, _) = prepare(&game, &state, key, key, &t, None);
        assert_eq!(
            l2[0].md5, l[0].md5,
            "rebuilt from the kept original, same output"
        );
        assert_eq!(
            std::fs::read(state.join("l10n/orig").join(&obf)).unwrap(),
            original
        );
        // New translations (v2) while the client still holds v1: v1 must not become the original.
        let t2 = tr(&[("text:MyPage_JP:A", "決定", "Confirm")]);
        let (v2, _) = prepare(&game, &state, key, key, &t2, None);
        assert_ne!(v2[0].md5, l[0].md5);
        let (v2b, _) = prepare(&game, &state, key, key, &t2, None); // rebuilt again, disk still v1
        assert_eq!(v2b[0].md5, v2[0].md5);
        assert_eq!(
            std::fs::read(state.join("l10n/orig").join(&obf)).unwrap(),
            original
        );
        // Translations removed: the original is served again (revert), flagged as it is.
        let (l3, _) = prepare(&game, &state, key, key, &tr(&[]), None);
        assert_eq!(l3[0].md5, md5_hex(&original));
        let (l4, _) = prepare(&game, &state, key, key, &tr(&[]), None); // cached
        assert_eq!(
            (l4[0].md5.as_str(), l4[0].encrypted),
            (l3[0].md5.as_str(), l3[0].encrypted)
        );
        // A half-written download (or an English copy) on disk never replaces the kept original.
        std::fs::write(game.join(&obf), b"partial").unwrap();
        let (l5, _) = prepare(&game, &state, key, key, &t, None);
        assert_eq!(l5[0].md5, l[0].md5);
        assert_eq!(
            std::fs::read(state.join("l10n/orig").join(&obf)).unwrap(),
            original
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
