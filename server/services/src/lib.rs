#![recursion_limit = "256"]
//! Backend services the game talks to, plus the shared server context.

pub mod accounts;
pub mod app;
pub mod art;
pub mod cdn;
pub mod collection;
pub mod items;
pub mod l10n;
pub mod labyrinth;
pub mod masterdata;
pub mod missions;
pub mod presents;
pub mod quests;
pub mod services;
pub mod town;
pub mod town_socket;
pub mod versionfile;
pub mod web;

use jdr_core::envelope::RequestEnvelope;
use jdr_core::log::Counters;
use jdr_core::settings::Settings;
use std::sync::atomic::{AtomicU16, Ordering};

type Stamp = (u64, std::time::SystemTime);

/// The two bundles we generate: the master tables, and the manifest that lists every file
/// (including the master tables, with their md5 and size).
pub struct Generated {
    pub versionfile: Vec<u8>,
    /// The master tables, or why they could not be built (the manifest is still served).
    pub masterdata: Result<Vec<u8>, String>,
    /// English versions of the game's text bundles: obfuscated name -> file we serve instead.
    pub localized: std::collections::HashMap<String, std::path::PathBuf>,
}

/// The generated bundles, or the last failure (with the vcs.bytes stamp it was computed for).
#[derive(Clone)]
pub enum ManifestState {
    Built(Stamp, std::sync::Arc<Generated>),
    Failed(Option<Stamp>, std::time::Instant, String),
}

pub struct Ctx {
    pub settings: Settings,
    pub counters: Counters,
    /// Port actually bound (logged; used to build the URLs we hand out).
    pub port: AtomicU16,
    pub trace: bool,
    pub accounts: accounts::Accounts,
    /// Answer `get_tutorial_step` with all flags set (Build 01 default: the tutorial needs
    /// master data that is not rebuilt yet).
    pub skip_tutorial: bool,
    /// Generated `versionfile` and `in_memory_master_data`, keyed by the (size, mtime) of `vcs.bytes`.
    pub versionfile_cache: std::sync::Mutex<Option<ManifestState>>,
    /// True while the embedded host imports staged game files (the key request waits for it).
    pub importing: std::sync::atomic::AtomicBool,
    /// The player's redrawn UI art, if their APK carries a pack (applied with the English rebuild).
    pub art: Option<art::ArtPack>,
    /// The last successful build, kept even when a later rebuild fails.
    pub last_built: std::sync::Mutex<Option<std::sync::Arc<Generated>>>,
    /// Serialises manifest builds.
    pub versionfile_build: std::sync::Mutex<()>,
}

/// Length of the all-ones tutorial list used to skip every tutorial (the real flag table is
/// unknown; extra entries are ignored by the client).
pub const TUTORIAL_FLAGS: usize = 256;

impl Ctx {
    pub fn new(settings: Settings, trace: bool) -> Self {
        let accounts = accounts::Accounts::new(&settings.data_dir);
        Self {
            accounts,
            skip_tutorial: true,
            versionfile_cache: std::sync::Mutex::new(None),
            versionfile_build: std::sync::Mutex::new(()),
            last_built: std::sync::Mutex::new(None),
            art: None,
            importing: std::sync::atomic::AtomicBool::new(false),
            settings,
            counters: Counters::default(),
            port: AtomicU16::new(0),
            trace,
        }
    }

    pub fn http_key(&self) -> Option<&[u8]> {
        self.settings
            .keys
            .as_ref()?
            .http_key
            .as_deref()
            .map(str::as_bytes)
    }

    pub fn settings_trace(&self) -> bool {
        self.trace
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port.load(Ordering::Relaxed))
    }

    /// Sessions are local and never expire: echo the client's, or issue one per device.
    pub fn session_for(&self, env: &RequestEnvelope) -> String {
        if !env.session.is_empty() {
            return env.session.clone();
        }
        if env.uuid.is_empty() {
            "local-session".into()
        } else {
            format!("local-{}", env.uuid)
        }
    }

    /// `yyyy-MM-dd HH:mm:ss` (JST), parsed by the client with DateTime.TryParse.
    pub fn server_date(&self) -> String {
        let (y, mo, d, h, mi, s) = jdr_core::clock::civil(jdr_core::clock::now());
        format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
    }

    /// Obfuscated file name of the `versionfile` bundle (what `getfiledata` asks for).
    pub fn versionfile_obf_name(&self) -> Option<String> {
        let k = self.settings.keys.as_ref()?;
        Some(format!(
            "{}.bytes",
            jdr_crypto::names::obfuscate(versionfile::BUNDLE_NAME, k.asset_key.as_bytes())
        ))
    }

    /// Obfuscated file name of the `in_memory_master_data` bundle.
    pub fn masterdata_obf_name(&self) -> Option<String> {
        let k = self.settings.keys.as_ref()?;
        Some(format!(
            "{}.bytes",
            jdr_crypto::names::obfuscate(masterdata::BUNDLE_NAME, k.asset_key.as_bytes())
        ))
    }

    /// The last successfully built bundles, without rebuilding (used to serve the English files
    /// while the client downloads: it rewrites vcs.bytes as it goes, which must not trigger a
    /// rebuild mid-download).
    pub fn last_generated(&self) -> Option<std::sync::Arc<Generated>> {
        self.last_built
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The generated bundles (cached until vcs.bytes changes). The build runs outside the cache
    /// lock; a failure is remembered for 30 s so client retries do not rescan the whole folder
    /// each time.
    pub fn generated(&self) -> Result<std::sync::Arc<Generated>, String> {
        let lock = |m: &std::sync::Mutex<Option<ManifestState>>| {
            m.lock().unwrap_or_else(|e| e.into_inner()).clone()
        };
        let dir = self
            .settings
            .game_files_dir
            .as_ref()
            .ok_or("no game files folder configured")?;
        let k = self.settings.keys.as_ref().ok_or("no keys")?;
        let stamp = std::fs::metadata(dir.join("vcs.bytes"))
            .ok()
            .map(|md| (md.len(), md.modified().unwrap_or(std::time::UNIX_EPOCH)));
        match lock(&self.versionfile_cache) {
            Some(ManifestState::Built(s, b)) if Some(s) == stamp => return Ok(b),
            Some(ManifestState::Failed(s, at, e))
                if s == stamp && at.elapsed() < std::time::Duration::from_secs(30) =>
            {
                return Err(e)
            }
            _ => {}
        }
        // One builder at a time; others wait here, then find the fresh result.
        let _b = self
            .versionfile_build
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match lock(&self.versionfile_cache) {
            Some(ManifestState::Built(s, b)) if Some(s) == stamp => return Ok(b),
            Some(ManifestState::Failed(s, at, e))
                if s == stamp && at.elapsed() < std::time::Duration::from_secs(30) =>
            {
                return Err(e)
            }
            _ => {}
        }
        let t0 = std::time::Instant::now();
        let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
        let built = (|| {
            let stamp = stamp.ok_or_else(|| {
                format!("{}/vcs.bytes not found: the game's downloaded files are not on this device (restore files/, see PLAYER-SETUP)", dir.display())
            })?;
            let mut entries = versionfile::entries_from_files(dir, key, iv)?;
            let (ms, ab) = versionfile::find_template_types(dir, key, iv)?;
            let md_name = masterdata::BUNDLE_NAME;
            entries.retain(|e| e.path != format!("{md_name}.bytes"));
            let md = masterdata::build_embedded(&ms, &ab);
            match &md {
                // Listed like any other file (expected to match the dead server; to be confirmed
                // on a device): the client fetches it into memory, never stores it or records it in
                // vcs.bytes, and is expected to check the download against this md5.
                Ok((md, _)) => entries.push(versionfile::Entry {
                    path: format!("{md_name}.bytes"),
                    obf: format!("{}.bytes", jdr_crypto::names::obfuscate(md_name, key)),
                    md5: versionfile::md5_hex(md),
                    size: md.len() as i64,
                    encrypted: false,
                }),
                Err(e) => {
                    jdr_core::log::line("assets", &format!("in_memory_master_data ERROR: {e}"))
                }
            }
            entries.sort_by(|a, b| a.path.cmp(&b.path));
            // English text bundles: listed with their own md5/size, so the client fetches them once.
            let t_l10n = std::time::Instant::now();
            let (loc, l10n_errors) = match l10n::Translations::embedded() {
                Ok(tr) => l10n::prepare(
                    dir,
                    &self.settings.data_dir,
                    key,
                    iv,
                    &tr,
                    self.art.as_ref(),
                ),
                Err(e) => (Vec::new(), vec![e]),
            };
            for e in &l10n_errors {
                jdr_core::log::line("l10n", &format!("ERROR {e}"));
            }
            let mut localized = std::collections::HashMap::new();
            for l in &loc {
                if let Some(e) = entries.iter_mut().find(|e| e.obf == l.obf) {
                    e.md5 = l.md5.clone();
                    e.size = l.size;
                    e.encrypted = l.encrypted;
                    localized.insert(l.obf.clone(), l.file.clone());
                }
            }
            let (strings, replaced, stale): (usize, usize, usize) =
                loc.iter().fold((0, 0, 0), |a, l| {
                    (
                        a.0 + l.stats.strings,
                        a.1 + l.stats.replaced,
                        a.2 + l.stats.stale,
                    )
                });
            for l in &loc {
                for e in l.stats.art_errors.iter().take(5) {
                    jdr_core::log::line("art", &format!("{}: skipped: {e}", l.name));
                }
            }
            if let Some(a) = &self.art {
                let (done, unmatched) = loc.iter().fold((0, 0), |x, l| {
                    (x.0 + l.stats.art, x.1 + l.stats.art_unmatched)
                });
                jdr_core::log::line(
                    "art",
                    &format!(
                        "art pack: {} image(s); {done} sprite(s) replaced in this rebuild, {unmatched} unmatched",
                        a.len()
                    ),
                );
            }
            jdr_core::log::line(
                "l10n",
                &format!(
                    "{} English file(s) ready in {} ms, {} already on the phone ({} rebuilt now: {} of {} strings replaced, {} stale)",
                    localized.len(),
                    t_l10n.elapsed().as_millis(),
                    loc.iter().filter(|l| l.on_device).count(),
                    loc.iter().filter(|l| l.stats.strings > 0).count(),
                    replaced,
                    strings,
                    stale
                ),
            );
            let vf = versionfile::build(
                &entries,
                &ms,
                &ab,
                &jdr_core::clock::format(jdr_core::clock::now()),
            );
            jdr_core::log::line(
                "assets",
                &format!(
                    "versionfile built: {} entries, {} KiB; in_memory_master_data: {}; {} ms",
                    entries.len(),
                    vf.len() / 1024,
                    match &md {
                        Ok((b, tables)) => format!(
                            "{} tables ({}), {} KiB",
                            tables.len(),
                            tables
                                .iter()
                                .filter(|t| t.rows > 0)
                                .map(|t| format!("{} {} rows", t.name, t.rows))
                                .collect::<Vec<_>>()
                                .join(", "),
                            b.len() / 1024
                        ),
                        Err(_) => "FAILED (see above)".into(),
                    },
                    t0.elapsed().as_millis()
                ),
            );
            let b = std::sync::Arc::new(Generated {
                versionfile: vf,
                masterdata: md.map(|(b, _)| b),
                localized,
            });
            Ok::<_, String>((stamp, b))
        })();
        let mut g = self
            .versionfile_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match built {
            Ok((s, b)) => {
                *g = Some(ManifestState::Built(s, b.clone()));
                *self.last_built.lock().unwrap_or_else(|e| e.into_inner()) = Some(b.clone());
                Ok(b)
            }
            Err(e) => {
                *g = Some(ManifestState::Failed(
                    stamp,
                    std::time::Instant::now(),
                    e.clone(),
                ));
                Err(e)
            }
        }
    }

    /// Log what the game's files folder holds (boot diagnostics).
    pub fn log_files_report(&self) {
        let (Some(dir), Some(k)) = (
            self.settings.game_files_dir.as_ref(),
            self.settings.keys.as_ref(),
        ) else {
            return;
        };
        let r = versionfile::files_report(dir, k.asset_key.as_bytes(), k.asset_iv.as_bytes());
        let line = if !r.vcs_present {
            format!("game files: {} file(s) in {}, vcs.bytes MISSING: restore your downloaded files (PLAYER-SETUP)", r.files_on_disk, dir.display())
        } else if r.missing.is_empty() {
            format!(
                "game files: {} file(s), all {} recorded files present",
                r.files_on_disk, r.recorded
            )
        } else {
            format!(
                "game files: {} file(s); {} of {} recorded files MISSING (copy incomplete; first: {:?})",
                r.files_on_disk,
                r.missing.len(),
                r.recorded,
                r.missing.iter().take(3).collect::<Vec<_>>()
            )
        };
        jdr_core::log::line("assets", &line);
    }

    /// Data version; constant so the client never thinks assets changed mid-session.
    pub fn datal_ver(&self) -> String {
        "1".into()
    }
}
