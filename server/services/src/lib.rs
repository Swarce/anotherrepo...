//! Backend services the game talks to, plus the shared server context.

pub mod accounts;
pub mod app;
pub mod cdn;
pub mod services;
pub mod web;

use jdr_core::envelope::RequestEnvelope;
use jdr_core::log::Counters;
use jdr_core::settings::Settings;
use std::sync::atomic::{AtomicU16, Ordering};

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

    /// Data version; constant so the client never thinks assets changed mid-session.
    pub fn datal_ver(&self) -> String {
        "1".into()
    }
}
