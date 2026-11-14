//! Logger: one file per start (rotated, newest N kept), mirrored to stderr, plus helpers for the
//! per-request line and the periodic `[status]` line.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

struct Sink {
    file: Option<File>,
    path: Option<PathBuf>,
    echo: bool,
}

static SINK: OnceLock<Mutex<Sink>> = OnceLock::new();
static MIRROR: OnceLock<fn(&str, &str)> = OnceLock::new();

/// Also send every line to `f(tag, message)` (the embedded host uses this for logcat).
pub fn set_mirror(f: fn(&str, &str)) {
    let _ = MIRROR.set(f);
}

pub const KEEP_LOGS: usize = 10;

fn sink() -> &'static Mutex<Sink> {
    SINK.get_or_init(|| {
        Mutex::new(Sink {
            file: None,
            path: None,
            echo: true,
        })
    })
}

/// Start logging to `<dir>/server-<unix>.log`, deleting the oldest beyond [`KEEP_LOGS`].
pub fn init(dir: Option<&Path>, echo: bool) -> Option<PathBuf> {
    let mut s = sink().lock().unwrap();
    s.echo = echo;
    let dir = dir?;
    std::fs::create_dir_all(dir).ok()?;
    let mut old: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("server-") && n.ends_with(".log"))
                .unwrap_or(false)
        })
        .collect();
    old.sort();
    while old.len() >= KEEP_LOGS {
        let _ = std::fs::remove_file(old.remove(0));
    }
    let p = dir.join(format!("server-{}.log", unix_ms() / 1000));
    s.file = OpenOptions::new().create(true).append(true).open(&p).ok();
    s.path = Some(p.clone());
    Some(p)
}

pub fn current_path() -> Option<PathBuf> {
    sink().lock().unwrap().path.clone()
}

pub fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `HH:MM:SS.mmm` (UTC) for log lines.
pub fn stamp() -> String {
    let ms = unix_ms();
    let s = ms / 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (s / 3600) % 24,
        (s / 60) % 60,
        s % 60,
        ms % 1000
    )
}

pub fn line(tag: &str, msg: &str) {
    if let Some(f) = MIRROR.get() {
        f(tag, msg);
    }
    let l = format!("{} [{tag}] {msg}\n", stamp());
    let mut s = sink().lock().unwrap();
    if let Some(f) = s.file.as_mut() {
        let _ = f.write_all(l.as_bytes());
    }
    if s.echo {
        eprint!("{l}");
    }
}

#[macro_export]
macro_rules! log {
    ($tag:expr, $($a:tt)*) => { $crate::log::line($tag, &format!($($a)*)) };
}

/// Server-wide counters for the `[status]` line.
#[derive(Default)]
pub struct Counters {
    pub requests: AtomicU64,
    pub api: AtomicU64,
    pub cdn: AtomicU64,
    pub unknown: AtomicU64,
    pub errors: AtomicU64,
    pub bytes_out: AtomicU64,
}

impl Counters {
    pub fn status_line(&self) -> String {
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        format!(
            "requests={} api={} cdn={} unknown_routes={} errors={} out={}KB",
            g(&self.requests),
            g(&self.api),
            g(&self.cdn),
            g(&self.unknown),
            g(&self.errors),
            g(&self.bytes_out) / 1024
        )
    }
}
