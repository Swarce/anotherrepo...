//! Backend services the game talks to, plus the shared server context.

pub mod accounts;
pub mod app;
pub mod cdn;
pub mod masterdata;
pub mod services;
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
