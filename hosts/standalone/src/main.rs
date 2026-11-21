//! `jdr-server` — the server as a PC process for the dev loop.
//! The stock-but-URL-patched game reaches it over `adb reverse tcp:18080 tcp:18080`.

use jdr_core::settings::{Settings, DEFAULT_PORT};
use jdr_core::{http, log};
use jdr_services::{app, Ctx};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

const USAGE: &str = "usage: jdr-server --keys <keys.json> --files <copy of the game's files/ dir> [--data <state dir>] [--port N] [--trace] [--clock-offset SECS] [--tutorial skip|play] [--art <replacements dir>]";

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
    // Standalone binds exactly one port: the patched game calls the port it was patched with,
    // so silently moving to a fallback port would point it at whatever else holds that port.
    let port: u16 = match opt(&a, "--port") {
        Some(p) => p
            .parse()
            .ok()
            .filter(|&p| p != 0)
            .unwrap_or_else(|| die("bad --port")),
        None => DEFAULT_PORT,
    };
    s.ports = vec![port];
    let keys = opt(&a, "--keys").unwrap_or_else(|| die(USAGE));
    s.keys = Some(Settings::load_keys(&PathBuf::from(&keys)).unwrap_or_else(|e| die(&e)));
    s.game_files_dir = opt(&a, "--files").map(PathBuf::from);
    s.files_dir = Some(PathBuf::from(opt(&a, "--files").unwrap_or_else(|| die("--files is required: a copy of the game's files/ folder (the CDN serves your files from it)"))));
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
    let sk = s.keys.as_ref().and_then(|k| k.sound_key.clone());
    if sk
        .as_deref()
        .and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .is_none()
    {
        log::line("boot", &format!("WARNING: sound_key missing or not hex ({sk:?}); sending 0, downloaded audio will not decode"));
    }
    if s.keys.as_ref().and_then(|k| k.http_key.as_ref()).is_none() {
        log::line(
            "boot",
            "WARNING: keys.json has no http_key; API requests cannot be read",
        );
    }
    match &s.files_dir {
        Some(d) if d.is_dir() => log::line("boot", &format!("CDN serves {}", d.display())),
        Some(d) => die(&format!("--files {} is not a directory", d.display())),
        None => unreachable!(),
    }
    let (listener, port) =
        http::bind(&s.ports).unwrap_or_else(|e| {
            die(&format!(
                "cannot bind 127.0.0.1:{port}: {e}\nIs another jdr-server still running? Stop it, or use --port N with an APK patched for the same port (jdr-patch --port N)."
            ))
        });
    let mut ctx = Ctx::new(s, a.iter().any(|x| x == "--trace"));
    if let Some(d) = opt(&a, "--art") {
        let pack =
            jdr_services::art::ArtPack::from_dir(&PathBuf::from(&d)).unwrap_or_else(|e| die(&e));
        log::line(
            "art",
            &format!("art pack loaded from {d}: {} image(s)", pack.len()),
        );
        ctx.art = Some(pack);
    }
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
                "played — WARNING: the master tables have no tutorial scripts yet, so the first tutorial starts with nothing to run (dev only)"
            }
        ),
    );
    let ctx = Arc::new(ctx);
    ctx.port.store(port, Ordering::Relaxed);
    log::line(
        "boot",
        &format!("listening on http://127.0.0.1:{port} (adb reverse tcp:{port} tcp:{port})"),
    );
    ctx.log_files_report();
    let c2 = ctx.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(60));
        log::line("status", &c2.counters.status_line());
    });
    // The Town home's socket shares the port (docs/research/24-town.md).
    let town = jdr_services::town_socket::hook(ctx.clone());
    let h: http::Handler = Arc::new(move |r| app::handle(&ctx, r));
    if let Err(e) = http::serve_with(listener, h, Some(town)) {
        die(&format!("server stopped: {e}"));
    }
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2)
}
