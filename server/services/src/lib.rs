//! Backend services the game talks to, plus the shared server context.

pub mod accounts;
pub mod app;
pub mod cdn;
pub mod services;
pub mod versionfile;
pub mod web;

use jdr_core::envelope::RequestEnvelope;
use jdr_core::log::Counters;
use jdr_core::settings::Settings;
use std::sync::atomic::{AtomicU16, Ordering};

/// A generated bundle and the (size, mtime) of the input it was built from.
pub type CachedBundle = ((u64, std::time::SystemTime), std::sync::Arc<Vec<u8>>);

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
    /// Generated `versionfile` bundle, keyed by the (size, mtime) of `vcs.bytes`.
    pub versionfile_cache: std::sync::Mutex<Option<CachedBundle>>,
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

    /// The manifest bundle, generated from the game's own files (cached until vcs.bytes changes).
    pub fn versionfile(&self) -> Result<std::sync::Arc<Vec<u8>>, String> {
        let dir = self
            .settings
            .game_files_dir
            .as_ref()
            .ok_or("no game files folder configured")?;
        let k = self.settings.keys.as_ref().ok_or("no keys")?;
        let md = std::fs::metadata(dir.join("vcs.bytes")).map_err(|e| format!("vcs.bytes: {e}"))?;
        let stamp = (md.len(), md.modified().unwrap_or(std::time::UNIX_EPOCH));
        let mut g = self.versionfile_cache.lock().unwrap();
        if let Some((s, b)) = g.as_ref() {
            if *s == stamp {
                return Ok(b.clone());
            }
        }
        let t0 = std::time::Instant::now();
        let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
        let entries = versionfile::entries_from_files(dir, key, iv)?;
        let (ms, ab) = versionfile::find_template_types(dir, key, iv)?;
        let b = std::sync::Arc::new(versionfile::build(
            &entries,
            &ms,
            &ab,
            &jdr_core::clock::format(jdr_core::clock::now()),
        ));
        jdr_core::log::line(
            "assets",
            &format!(
                "versionfile built: {} entries, {} KiB, {} ms",
                entries.len(),
                b.len() / 1024,
                t0.elapsed().as_millis()
            ),
        );
        *g = Some((stamp, b.clone()));
        Ok(b)
    }

    /// Data version; constant so the client never thinks assets changed mid-session.
    pub fn datal_ver(&self) -> String {
        "1".into()
    }
}
