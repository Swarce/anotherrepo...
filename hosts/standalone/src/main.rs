//! `jdr-server` — the server as a PC process for the dev loop.
//! The stock-but-URL-patched game reaches it over `adb reverse tcp:18080 tcp:18080`.

use jdr_core::settings::{Settings, DEFAULT_PORT, FALLBACK_PORT};
use jdr_core::{http, log};
use jdr_services::{app, Ctx};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

const USAGE: &str = "usage: jdr-server --keys <keys.json> [--files <game files/ dir>] [--data <state dir>] [--port N] [--trace] [--clock-offset SECS] [--tutorial skip|play]";

fn opt(a: &[String], n: &str) -> Option<String> {
    a.iter()
        .position(|x| x == n)
        .and_then(|i| a.get(i + 1).cloned())
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.iter().any(|x| x == "-h" || x == "--help") {
        println!("{USAGE}");
        return;
    }
    let data = PathBuf::from(opt(&a, "--data").unwrap_or_else(|| "local/server".into()));
    let mut s = Settings::new(data);
    if let Some(p) = opt(&a, "--port") {
        let p: u16 = p.parse().unwrap_or_else(|_| die("bad --port"));
        s.ports = vec![p, p.wrapping_add(1)];
    } else {
        s.ports = vec![DEFAULT_PORT, FALLBACK_PORT];
    }
    let keys = opt(&a, "--keys").unwrap_or_else(|| die(USAGE));
    s.keys = Some(Settings::load_keys(&PathBuf::from(&keys)).unwrap_or_else(|e| die(&e)));
    s.files_dir = opt(&a, "--files").map(PathBuf::from);
    if let Some(off) = opt(&a, "--clock-offset") {
        jdr_core::clock::set_offset(off.parse().unwrap_or_else(|_| die("bad --clock-offset")));
    }
    let logp = log::init(Some(&s.log_dir()), true);
    log::line(
        "boot",
        &format!(
            "jdr-server build {} starting; log {}",
            jdr_core::BUILD,
            logp.map(|p| p.display().to_string()).unwrap_or_default()
        ),
    );
    if s.keys.as_ref().and_then(|k| k.http_key.as_ref()).is_none() {
        log::line(
            "boot",
            "WARNING: keys.json has no http_key; API requests cannot be read",
        );
    }
    match &s.files_dir {
        Some(d) if d.is_dir() => log::line("boot", &format!("CDN serves {}", d.display())),
        Some(d) => die(&format!("--files {} is not a directory", d.display())),
        None => log::line("boot", "no --files: the CDN will answer 404"),
    }
    let (listener, port) =
        http::bind(&s.ports).unwrap_or_else(|e| die(&format!("cannot bind {:?}: {e}", s.ports)));
    let mut ctx = Ctx::new(s, a.iter().any(|x| x == "--trace"));
    ctx.skip_tutorial = match opt(&a, "--tutorial").as_deref() {
        None | Some("skip") => true,
        Some("play") => false,
        Some(_) => die("--tutorial must be skip or play"),
    };
    log::line(
        "boot",
        &format!(
            "tutorial: {}",
            if ctx.skip_tutorial {
                "skipped (all flags set)"
            } else {
                "played"
            }
        ),
    );
    let ctx = Arc::new(ctx);
    ctx.port.store(port, Ordering::Relaxed);
    log::line(
        "boot",
        &format!("listening on http://127.0.0.1:{port} (adb reverse tcp:{port} tcp:{port})"),
    );
    let c2 = ctx.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        log::line("status", &c2.counters.status_line());
    });
    let h: http::Handler = Arc::new(move |r| app::handle(&ctx, r));
    if let Err(e) = http::serve(listener, h) {
        die(&format!("server stopped: {e}"));
    }
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2)
}
