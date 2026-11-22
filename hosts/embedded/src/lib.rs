//! Embedded host: `libjdr.so`.
//!
//! The patcher renames `libmain.so`'s `liblog.so` dependency to `libjdr.so` (same length), and this
//! library depends on `liblog.so` itself, so everything libmain imported still resolves. The
//! dynamic linker runs our constructor while Unity loads `libmain`; the constructor only spawns a
//! thread, so the game never waits on us and a failure here never takes the game down.
//!
//! Paths (user `n` = uid / 100000):
//! - state (accounts, saves): `/data/user/<n>/<pkg>/jdr/`
//! - logs and save backups (reachable with adb): `/storage/emulated/<n>/Android/data/<pkg>/jdr/{logs,saves}/`
//! - extra CDN files (import folder): `/storage/emulated/<n>/Android/data/<pkg>/jdr/cdn/` — never
//!   the game's own `files/` folder, which the client downloads into.

pub mod apkzip;
pub mod import;

use jdr_core::settings::{Keys, Settings, DEFAULT_PORT};
use jdr_core::{http, log};
use jdr_services::{app, Ctx};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Marker the patcher checks to make sure it embeds this library and not something else.
#[used]
#[no_mangle]
pub static JDR_EMBEDDED_MARKER: [u8; 24] = *b"JDR_EMBEDDED_SERVER_b044";

pub const KEYS_ASSET: &str = "assets/jdr/keys.json";
/// The player's redrawn UI art, stored in their APK by the patcher (optional).
pub const ART_ASSET: &str = "assets/jdr/art.bin";

#[cfg(target_os = "android")]
#[used]
#[link_section = ".init_array"]
static INIT: extern "C" fn() = jdr_init;

#[cfg(target_os = "android")]
extern "C" fn jdr_init() {
    let _ = std::panic::catch_unwind(|| {
        let _ = std::thread::Builder::new()
            .name("jdr-server".into())
            .spawn(|| {
                let _ = std::panic::catch_unwind(run);
            });
    });
}

#[cfg(target_os = "android")]
mod alog {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};
    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }
    pub fn write(tag: &str, msg: &str) {
        let t = CString::new("JDR").unwrap();
        let m = CString::new(format!("[{tag}] {msg}").replace('\0', " ")).unwrap_or_default();
        unsafe {
            __android_log_write(4, t.as_ptr(), m.as_ptr());
        }
    }
}

pub struct Paths {
    pub pkg: String,
    pub state: PathBuf,
    pub external: PathBuf,
}

fn valid_pkg(p: &str) -> bool {
    !p.is_empty()
        && p.len() < 256
        && p.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'_')
}

pub fn detect_paths() -> Result<Paths, String> {
    let cmd = std::fs::read("/proc/self/cmdline").map_err(|e| e.to_string())?;
    let pkg = String::from_utf8_lossy(cmd.split(|&b| b == 0 || b == b':').next().unwrap_or(&[]))
        .to_string();
    if !valid_pkg(&pkg) {
        return Err(format!("unexpected process name {pkg:?}"));
    }
    let uid = status_uid().ok_or("cannot read uid from /proc/self/status")?;
    let user = uid / 100_000;
    // Internal storage normally lives at /data/user/<n>/<pkg>; apps moved to adoptable storage live
    // under /mnt/expand/<uuid>/user/<n>/<pkg>. Use the first one that exists.
    let mut cands = vec![
        PathBuf::from(format!("/data/user/{user}/{pkg}")),
        PathBuf::from(format!("/data/data/{pkg}")),
    ];
    if let Ok(rd) = std::fs::read_dir("/mnt/expand") {
        cands.extend(
            rd.flatten()
                .map(|e| e.path().join(format!("user/{user}/{pkg}"))),
        );
    }
    let base = cands
        .into_iter()
        .find(|p| p.is_dir())
        .ok_or_else(|| format!("no data folder found for {pkg} (user {user})"))?;
    Ok(Paths {
        state: base.join("jdr"),
        external: PathBuf::from(format!("/storage/emulated/{user}/Android/data/{pkg}/jdr")),
        pkg,
    })
}

fn status_uid() -> Option<u32> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    s.lines()
        .find_map(|l| l.strip_prefix("Uid:"))
        .and_then(|v| v.split_whitespace().next())
        .and_then(|v| v.parse().ok())
}

/// Keys: an override file in the state folder wins, else the asset the patcher put in the APK.
pub fn load_keys(state: &Path, pkg: &str) -> Result<Keys, String> {
    let over = state.join("keys.json");
    if over.is_file() {
        return Settings::load_keys(&over);
    }
    let maps =
        std::fs::read_to_string("/proc/self/maps").map_err(|e| format!("/proc/self/maps: {e}"))?;
    let cands = apkzip::own_apk_candidates(&maps, pkg);
    let mut errs = vec![];
    for apk in &cands {
        match apkzip::read_stored(apk, KEYS_ASSET) {
            Ok(b) => {
                return serde_json::from_slice(&b).map_err(|e| format!("{apk} {KEYS_ASSET}: {e}"))
            }
            Err(e) => errs.push(e),
        }
    }
    Err(format!("keys asset not found; tried {cands:?}: {errs:?}"))
}

/// Identity of the installed APK (path, size, modification time): Android installs an update
/// under a new path with a new time, so this changes exactly when the APK is updated.
pub fn apk_identity(pkg: &str) -> Option<String> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    let apk = apkzip::own_apk_candidates(&maps, pkg).into_iter().next()?;
    let md = std::fs::metadata(&apk).ok()?;
    let t = md
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(format!("{apk}|{}|{t}", md.len()))
}

/// The art pack from our own APK, if the patcher stored one.
pub fn load_art(pkg: &str) -> Option<Result<jdr_services::art::ArtPack, String>> {
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    for apk in apkzip::own_apk_candidates(&maps, pkg) {
        if let Ok(b) = apkzip::read_stored(&apk, ART_ASSET) {
            return Some(jdr_services::art::ArtPack::from_bin(&b));
        }
    }
    None
}

/// Copy `src` to `dst_dir` if its size or mtime changed since the last copy.
fn copy_if_changed(
    src: &Path,
    dst_dir: &Path,
    seen: &mut std::collections::HashMap<PathBuf, (u64, std::time::SystemTime)>,
) {
    let Ok(md) = std::fs::metadata(src) else {
        return;
    };
    let stamp = (md.len(), md.modified().unwrap_or(std::time::UNIX_EPOCH));
    if seen.get(src) == Some(&stamp) {
        return;
    }
    let Some(name) = src.file_name() else { return };
    if std::fs::create_dir_all(dst_dir).is_ok() && std::fs::copy(src, dst_dir.join(name)).is_ok() {
        seen.insert(src.to_path_buf(), stamp);
    }
}

/// Keep only the newest `keep` `server-*.log` files in `dir`.
fn prune(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut v: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("server-") && n.ends_with(".log"))
                .unwrap_or(false)
        })
        .collect();
    v.sort();
    while v.len() > keep {
        let _ = std::fs::remove_file(v.remove(0));
    }
}

/// Mirror the log (every 5 s) and the account saves (every 30 s) to external storage, where the
/// player can reach them with adb. Only changed files are copied. External storage only becomes
/// writable once the game has created its folder there, so failures are retried silently.
fn mirror_loop(state: PathBuf, external: PathBuf) {
    let mut seen = std::collections::HashMap::new();
    let mut tick: u64 = 0;
    loop {
        std::thread::sleep(Duration::from_secs(5));
        // Never create Android/data/<pkg> ourselves: wait for the framework/game to do it.
        if !external.parent().map(|p| p.is_dir()).unwrap_or(false) {
            continue;
        }
        if let Some(src) = log::current_path() {
            copy_if_changed(&src, &external.join("logs"), &mut seen);
            if tick % 60 == 0 {
                prune(&external.join("logs"), log::KEEP_LOGS);
            }
        }
        if tick % 6 == 0 {
            if let Ok(rd) = std::fs::read_dir(state.join("accounts")) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("json") {
                        copy_if_changed(&p, &external.join("saves"), &mut seen);
                    }
                }
            }
        }
        tick += 1;
    }
}

pub fn run() {
    let t0 = Instant::now();
    #[cfg(target_os = "android")]
    log::set_mirror(alog::write);
    let paths = match detect_paths() {
        Ok(p) => p,
        Err(e) => {
            log::line("boot", &format!("FATAL: {e}; embedded server not started"));
            return;
        }
    };
    let logp = log::init(Some(&paths.state.join("logs")), false);
    let ms = || t0.elapsed().as_millis();
    log::line(
        "boot",
        &format!(
            "jdr embedded build {} in {} (+{}ms); log {:?}",
            jdr_core::BUILD,
            paths.pkg,
            ms(),
            logp
        ),
    );
    {
        let ext = paths.external.clone();
        let st = paths.state.clone();
        let _ = std::thread::Builder::new()
            .name("jdr-mirror".into())
            .spawn(move || mirror_loop(st, ext));
    }
    let keys = match load_keys(&paths.state, &paths.pkg) {
        Ok(k) => k,
        Err(e) => {
            log::line(
                "boot",
                &format!("FATAL: no keys ({e}); embedded server not started"),
            );
            return;
        }
    };
    log::line(
        "boot",
        &format!(
            "keys loaded (+{}ms); sound key {}",
            ms(),
            if keys.sound_key.is_some() {
                "present"
            } else {
                "MISSING"
            }
        ),
    );
    serve_embedded(&paths, keys, DEFAULT_PORT, u32::MAX, t0);
}

/// The portable part of the embedded host: bind (with retries), build the context, serve.
/// Returns when the listener fails. Host tests call this directly.
pub fn serve_embedded(paths: &Paths, keys: Keys, port: u16, bind_retries: u32, t0: Instant) {
    let ms = || t0.elapsed().as_millis();
    let mut s = Settings::new(paths.state.clone());
    s.ports = vec![port];
    s.keys = Some(keys);
    let cdn = paths.external.join("cdn");
    s.files_dir = Some(cdn.clone());
    // The game's own download folder, read-only (manifest source): <external>/../files
    s.game_files_dir = paths.external.parent().map(|p| p.join("files"));
    // The previous game process may still be dying; retry the bind for a while.
    let mut tries = 0;
    let (listener, port) = loop {
        match http::bind(&s.ports) {
            Ok(x) => break x,
            Err(e) if tries < bind_retries => {
                if tries % 120 == 0 {
                    // first failure, then once a minute
                    log::line(
                        "boot",
                        &format!("FATAL: cannot bind 127.0.0.1:{port} ({e}): another app, an old game process, or a leftover `adb reverse tcp:18080` (run `adb reverse --remove-all`) holds it; retrying"),
                    );
                }
                tries += 1;
                std::thread::sleep(Duration::from_millis(500));
            }
            Err(e) => {
                log::line("boot", &format!("FATAL: cannot bind 127.0.0.1:{port}: {e}"));
                return;
            }
        }
    };
    let mut ctx = Ctx::new(s, false);
    // The generated bundles are kept between launches and rebuilt only when this APK changes.
    ctx.build_identity = apk_identity(&paths.pkg);
    match load_art(&paths.pkg) {
        Some(Ok(a)) => {
            log::line("art", &format!("art pack loaded: {} image(s)", a.len()));
            ctx.art = Some(a);
        }
        Some(Err(e)) => log::line("art", &format!("art pack ERROR: {e}")),
        None => {}
    }
    let ctx = Arc::new(ctx);
    ctx.port.store(port, Ordering::Relaxed);
    {
        // Build the manifest in the background so the game's first request is answered at once.
        let c = ctx.clone();
        // Set before the thread starts, so a key request that arrives first already waits.
        ctx.importing.store(import_pending(&ctx), Ordering::Relaxed);
        let _ = std::thread::Builder::new()
            .name("jdr-manifest".into())
            .spawn(move || {
                run_import(&c);
                c.log_files_report();
                if let Err(e) = c.generated() {
                    log::line("assets", &format!("generated bundles not built yet: {e}"));
                }
            });
    }
    BOUND_PORT.store(port, Ordering::Relaxed);
    log::line(
        "boot",
        &format!(
            "listening on 127.0.0.1:{port} (+{}ms); CDN import folder {}",
            ms(),
            cdn.display()
        ),
    );
    let c2 = ctx.clone();
    let _ = std::thread::Builder::new()
        .name("jdr-status".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(60));
            log::line("status", &c2.counters.status_line());
        });
    // The Town home's socket shares the port (docs/research/24-town.md).
    let town = jdr_services::town_socket::hook(ctx.clone());
    let h: http::Handler = Arc::new(move |r| app::handle(&ctx, r));
    if let Err(e) = http::serve_with(listener, h, Some(town)) {
        log::line("boot", &format!("server stopped: {e}"));
    }
}

/// Import staged game files (see [`import`]) before the manifest is built.
/// Clears `Ctx::importing` on every exit, panics included, so the key request never waits for a
/// dead import.
struct Importing<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for Importing<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// True when there is something to import (decided before the server answers its first request).
fn import_pending(ctx: &Ctx) -> bool {
    ctx.settings.game_files_dir.is_some() && Path::new(import::STAGING).is_dir()
}

fn run_import(ctx: &Ctx) {
    let _importing = Importing(&ctx.importing);
    let staging = Path::new(import::STAGING);
    let Some(dest) = ctx.settings.game_files_dir.clone() else {
        return;
    };
    if !staging.is_dir() {
        return;
    }
    ctx.importing.store(true, Ordering::Relaxed);
    let t0 = Instant::now();
    log::line(
        "import",
        &format!(
            "staging folder found: importing {} -> {}",
            staging.display(),
            dest.display()
        ),
    );
    // Keep what the game and the server updated on purpose, but only while it is really theirs:
    // an English file only while it is byte-identical to one the server produced (a broken or
    // foreign copy is restored), and vcs.bytes only while the staging copy is the one we already
    // imported (a newer backup in staging still replaces it).
    let ours: std::collections::HashMap<String, Vec<String>> =
        std::fs::read(ctx.settings.data_dir.join("l10n/state.json"))
            .ok()
            .and_then(|b| {
                serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&b).ok()
            })
            .map(|m| {
                m.into_iter()
                    .map(|(k, v)| {
                        let mut md5s: Vec<String> = v["outputs"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(str::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default();
                        if let Some(o) = v["out_md5"].as_str() {
                            md5s.push(o.to_owned());
                        }
                        (k, md5s)
                    })
                    .collect()
            })
            .unwrap_or_default();
    let record_path = ctx.settings.data_dir.join("import-record.json");
    let stamp = |p: &Path| {
        std::fs::metadata(p).ok().map(|m| {
            let t = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("{}:{t}", m.len())
        })
    };
    let recorded_vcs = std::fs::read_to_string(&record_path).ok();
    let staging_vcs = stamp(&staging.join("vcs.bytes"));
    let keep = |n: &str| {
        if n == "vcs.bytes" {
            return staging_vcs.is_some() && recorded_vcs == staging_vcs;
        }
        ours.get(n).is_some_and(|md5s| {
            std::fs::read(dest.join(n))
                .map(|b| md5s.contains(&jdr_services::versionfile::md5_hex(&b)))
                .unwrap_or(false)
        })
    };
    let r = import::import(staging, &dest, &keep, |done, total, st| {
        log::line(
            "import",
            &format!("{done}/{total} files ({} MiB copied)", st.bytes >> 20),
        );
    });
    let imported_ok = r.as_ref().is_ok_and(|st| st.failed == 0);
    match r {
        Ok(st) => log::line(
            "import",
            &format!(
                "import complete in {} s: {} copied ({} MiB), {} replaced unreadable, {} already fine, {} failed. Restart the game; then you may delete /data/local/tmp/jdr-import",
                t0.elapsed().as_secs(),
                st.copied,
                st.bytes >> 20,
                st.replaced_unreadable,
                st.already_ok,
                st.failed
            ),
        ),
        Err(e) => log::line("import", &format!("import FAILED: {e}")),
    }
    if let (true, Some(v)) = (imported_ok, &staging_vcs) {
        let _ = std::fs::write(&record_path, v);
    }
}

/// Port actually bound (0 until listening); for tests and diagnostics.
pub static BOUND_PORT: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_stored_entry_from_zip() {
        // build a zip with the patcher's writer conventions (stored entry)
        let d = std::env::temp_dir().join(format!("jdr-emb-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("base.apk");
        let raw = make_zip(&[
            ("a.txt", b"hello"),
            (KEYS_ASSET, br#"{"asset_key":"k","asset_iv":"i"}"#),
        ]);
        std::fs::write(&p, raw).unwrap();
        let ps = p.to_string_lossy().to_string();
        assert_eq!(apkzip::read_stored(&ps, "a.txt").unwrap(), b"hello");
        let k: Keys =
            serde_json::from_slice(&apkzip::read_stored(&ps, KEYS_ASSET).unwrap()).unwrap();
        assert_eq!(k.asset_key, "k");
        assert!(apkzip::read_stored(&ps, "missing").is_err());
    }

    /// Minimal stored-only zip writer for the test.
    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = vec![];
        let mut cd = vec![];
        for (name, data) in entries {
            let off = out.len() as u32;
            let mut lh = vec![];
            lh.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            lh.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            lh.extend_from_slice(&0u32.to_le_bytes()); // crc (not checked)
            lh.extend_from_slice(&(data.len() as u32).to_le_bytes());
            lh.extend_from_slice(&(data.len() as u32).to_le_bytes());
            lh.extend_from_slice(&(name.len() as u16).to_le_bytes());
            lh.extend_from_slice(&3u16.to_le_bytes()); // extra field (to test skipping)
            lh.extend_from_slice(name.as_bytes());
            lh.extend_from_slice(&[0, 0, 0]);
            out.extend_from_slice(&lh);
            out.extend_from_slice(data);
            cd.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            cd.extend_from_slice(&[20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            cd.extend_from_slice(&0u32.to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&[0; 12]);
            cd.extend_from_slice(&off.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&cd);
        out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[test]
    fn embedded_server_boots_from_apk_keys_and_answers() {
        use jdr_core::envelope::{decode_response_like_client, encode_request_like_client};
        use std::io::{Read, Write};
        let d = std::env::temp_dir().join(format!("jdr-emb-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let apk = d.join("base.apk");
        let keys = br#"{"asset_key":"AAAAAAAAAAAAAAAA","asset_iv":"BBBBBBBBBBBBBBBB","http_key":"Synthetic-Key-16","sound_key":null}"#;
        std::fs::write(&apk, make_zip(&[(KEYS_ASSET, keys)])).unwrap();
        let k: Keys = serde_json::from_slice(
            &apkzip::read_stored(&apk.to_string_lossy(), KEYS_ASSET).unwrap(),
        )
        .unwrap();
        let paths = Paths {
            pkg: "test.pkg".into(),
            state: d.join("state"),
            external: d.join("ext"),
        };
        std::thread::spawn(move || serve_embedded(&paths, k, 0, 0, Instant::now()));
        let mut port = 0;
        for _ in 0..100 {
            port = BOUND_PORT.load(Ordering::Relaxed);
            if port != 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_ne!(port, 0, "server did not start");
        let js = r#"{"uuid":"","session":"","language":1,"date":20261114103000,"platform":2,"version":"v","lzAccept":1,"lzEncode":0,"param":{}}"#;
        let body = encode_request_like_client(js, b"Synthetic-Key-16", b"0123456789abcdef");
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(format!("POST /common/get_version HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).unwrap();
        s.write_all(&body).unwrap();
        let mut out = vec![];
        s.read_to_end(&mut out).unwrap();
        let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let v = decode_response_like_client(&out[split + 4..], b"Synthetic-Key-16").unwrap();
        assert_eq!(v["param"]["url"], format!("http://127.0.0.1:{port}/api"));
    }

    #[test]
    fn apk_candidates_prefer_own_library_and_package() {
        let pkg = "com.bandainamcoent.jojodr";
        let maps = "\
7000-8000 r--p 00000000 fd:01 1 /data/app/~~x==/com.google.android.gms-AAA==/base.apk
7100-8100 r--p 00000000 fd:01 2 /data/app/~~y==/com.android.webview-BBB==/base.apk
7200-8200 r-xp 00000000 fd:01 3 /data/app/~~z==/com.bandainamcoent.jojodr-CCC==/lib/arm64/libjdr.so
7300-8300 r--p 00000000 fd:01 4 /data/app/~~z==/com.bandainamcoent.jojodr-CCC==/base.apk
";
        let c = apkzip::own_apk_candidates(maps, pkg);
        assert_eq!(
            c,
            vec!["/data/app/~~z==/com.bandainamcoent.jojodr-CCC==/base.apk".to_string()]
        );
        let from_apk = "7200-8200 r-xp 00000000 fd:01 3 /data/app/com.bandainamcoent.jojodr-1/base.apk!/lib/arm64-v8a/libjdr.so\n";
        assert_eq!(
            apkzip::own_apk_candidates(from_apk, pkg),
            vec!["/data/app/com.bandainamcoent.jojodr-1/base.apk".to_string()]
        );
        assert!(apkzip::own_apk_candidates(
            "7000-8000 r--p 0 0 1 /data/app/other-1/base.apk\n",
            pkg
        )
        .is_empty());
    }

    #[test]
    fn prune_keeps_newest() {
        let d = std::env::temp_dir().join(format!("jdr-emb-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for i in 0..15 {
            std::fs::write(d.join(format!("server-{:04}.log", i)), b"x").unwrap();
        }
        std::fs::write(d.join("other.txt"), b"x").unwrap();
        prune(&d, 10);
        let mut names: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names.len(), 11);
        assert_eq!(names[0], "other.txt");
        assert_eq!(names[1], "server-0005.log");
    }

    #[test]
    fn marker_matches_build() {
        assert_eq!(
            &JDR_EMBEDDED_MARKER[..],
            format!("JDR_EMBEDDED_SERVER_b{:03}", jdr_core::BUILD).as_bytes()
        );
    }

    #[test]
    fn copies_only_changed_files() {
        let d = std::env::temp_dir().join(format!("jdr-emb-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let src = d.join("a.json");
        std::fs::write(&src, b"1").unwrap();
        let mut seen = std::collections::HashMap::new();
        copy_if_changed(&src, &d.join("out"), &mut seen);
        assert_eq!(std::fs::read(d.join("out/a.json")).unwrap(), b"1");
        std::fs::write(d.join("out/a.json"), b"tampered").unwrap();
        copy_if_changed(&src, &d.join("out"), &mut seen);
        assert_eq!(
            std::fs::read(d.join("out/a.json")).unwrap(),
            b"tampered",
            "unchanged source is not copied again"
        );
        std::fs::write(&src, b"22").unwrap();
        copy_if_changed(&src, &d.join("out"), &mut seen);
        assert_eq!(std::fs::read(d.join("out/a.json")).unwrap(), b"22");
    }

    #[test]
    fn pkg_names() {
        assert!(valid_pkg("com.bandainamcoent.jojodr"));
        assert!(!valid_pkg("../x"));
        assert!(!valid_pkg(""));
    }
}
