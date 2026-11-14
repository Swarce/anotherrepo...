//! `jdr-patch` — turn the player's own APK into the offline build.

use jdr_patch::apk::{ApkEdit, Error};
use jdr_patch::meta::Metadata;
use jdr_patch::patches::{self, Config, METADATA_PATH};
use jdr_patch::report::Report;
use jdr_patch::sign;
use std::path::PathBuf;

const USAGE: &str = "usage:
  jdr-patch patch --apk <in.apk> --out <out.apk> --keys <keys.json> [--server-lib libjdr.so] [--port N] [--disable <patch-id>]... [--keystore <p12>] [--force]
        [--art <your redrawn art pack.zip> --data <your game files folder>]
        (--disable embed-server: dev-loop APK that talks to jdr-server on a PC)
  jdr-patch list                 list patches
  jdr-patch init-key [--keystore <p12>]   create the signing key if missing

env: JDR_BUILD_TOOLS (folder with zipalign+apksigner), JDR_KEYSTORE, JDR_KS_PASS";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("patch") => cmd_patch(&args[1..]),
        Some("list") => {
            for p in patches::all() {
                println!("{:<12} {}", p.id(), p.description());
            }
            Ok(())
        }
        Some("init-key") => {
            let ks = match opt(&args[1..], "--keystore")
                .map(PathBuf::from)
                .map(Ok)
                .unwrap_or_else(sign::default_keystore)
            {
                Ok(k) => k,
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(1)
                }
            };
            sign::ensure_key(&ks).map(|created| {
                println!(
                    "{} {}",
                    if created { "created" } else { "exists" },
                    ks.display()
                )
            })
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2)
        }
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

/// The first valid aarch64 `libjdr.so`: next to the jdr-patch binary (release bundle), then the
/// cross-build output in this checkout. (A host build next to the binary is skipped.)
fn default_server_lib() -> Option<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("libjdr.so")));
    let built = PathBuf::from("target/aarch64-linux-android/release/libjdr.so");
    beside.into_iter().chain(std::iter::once(built)).find(|p| {
        std::fs::read(p)
            .map(|b| patches::validate_server_lib(&b).is_ok())
            .unwrap_or(false)
    })
}

/// Never overwrite the player's original APK, and never clobber an existing output silently.
fn check_paths(input: &std::path::Path, out: &std::path::Path, force: bool) -> Result<(), Error> {
    let inp = std::fs::canonicalize(input).map_err(|e| format!("{}: {e}", input.display()))?;
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let canon_out = |p: &std::path::Path| -> Result<PathBuf, Error> {
        let dir = p
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        Ok(std::fs::canonicalize(dir)?.join(p.file_name().ok_or("--out has no file name")?))
    };
    for p in [
        out.to_path_buf(),
        out.with_extension("unsigned.apk"),
        out.with_extension("aligned.apk"),
        out.with_extension("unsigned.aligned.apk"),
    ] {
        if canon_out(&p)? == inp {
            return Err("--out (or a temporary file next to it) would overwrite the input APK; choose another output path".into());
        }
    }
    if out.exists() && !force {
        return Err(format!("{} exists; pass --force to replace it", out.display()).into());
    }
    Ok(())
}

fn cmd_patch(args: &[String]) -> Result<(), Error> {
    let input = PathBuf::from(opt(args, "--apk").ok_or("--apk is required")?);
    let out = PathBuf::from(opt(args, "--out").ok_or("--out is required")?);
    let keystore = match opt(args, "--keystore") {
        Some(k) => PathBuf::from(k),
        None => sign::default_keystore()?,
    };
    let force = args.iter().any(|a| a == "--force");
    check_paths(&input, &out, force)?;
    if !keystore.exists() {
        return Err(format!(
            "no signing key at {}. First time: run `jdr-patch init-key`. If you patched before, point JDR_KEYSTORE at that same key: a new key cannot install over your existing build.",
            keystore.display()
        )
        .into());
    }
    let pinned = sign::check_pin(&keystore)?;
    let mut cfg = Config::default();
    if let Some(p) = opt(args, "--port") {
        let port: u16 = p.parse().map_err(|_| "bad --port")?;
        cfg.server_url = format!("http://127.0.0.1:{port}");
    }
    cfg.keys_json = match opt(args, "--keys") {
        Some(k) => Some(std::fs::read(&k).map_err(|e| format!("--keys {k}: {e}"))?),
        None => None,
    };
    cfg.art_zip = opt(args, "--art").map(PathBuf::from);
    cfg.data_dir = opt(args, "--data").map(PathBuf::from);
    cfg.server_lib = opt(args, "--server-lib")
        .map(PathBuf::from)
        .or_else(default_server_lib);
    let disabled: Vec<&str> = args
        .windows(2)
        .filter(|w| w[0] == "--disable")
        .map(|w| w[1].as_str())
        .collect();
    if cfg.server_url != Config::default().server_url && !disabled.contains(&"embed-server") {
        return Err("--port only works for the dev-loop APK (--disable embed-server): the in-game server always listens on 18080".into());
    }
    for d in &disabled {
        if !patches::all().iter().any(|p| p.id() == *d) {
            return Err(format!("unknown patch id {d:?} (see `jdr-patch list`)").into());
        }
    }

    let mut rep = Report {
        input: input.display().to_string(),
        ..Default::default()
    };
    rep.input_sha256 = patches::sha256_hex(&std::fs::read(&input)?);
    let mut apk = ApkEdit::open(&input)?;
    let build = patches::identify(&mut apk)?;
    rep.build = format!("{} ({}) — verified", build.version_name, build.version_code);

    for p in patches::all() {
        if disabled.contains(&p.id()) {
            rep.lines.push((
                p.id().into(),
                p.description().into(),
                "disabled".into(),
                "-".into(),
            ));
            continue;
        }
        let detail = p
            .apply(&mut apk, &cfg, build)
            .map_err(|e| format!("patch {} refused: {e}", p.id()))?;
        rep.lines.push((
            p.id().into(),
            p.description().into(),
            "applied".into(),
            detail,
        ));
    }

    let unsigned = out.with_extension("unsigned.apk");
    let stats = apk.write(&unsigned)?;
    rep.stats = format!("{stats:?}");
    let sr = sign::align_and_sign(&unsigned, &out, &keystore)?;
    let _ = std::fs::remove_file(&unsigned);
    if sr.cert_sha256.to_ascii_lowercase() != pinned {
        let _ = std::fs::remove_file(&out);
        return Err("post-check failed: output certificate differs from the pinned key".into());
    }
    rep.checks
        .push("certificate matches the pinned signing key".into());
    rep.schemes = sr.schemes;
    rep.cert_sha256 = sr.cert_sha256;
    rep.output = out.display().to_string();
    rep.output_sha256 = patches::sha256_hex(&std::fs::read(&out)?);

    // Post-checks on the signed output.
    let mut chk = ApkEdit::open(&out)?;
    if !disabled.contains(&"server-url") {
        let m = chk.read(METADATA_PATH)?;
        let md = Metadata::parse(&m)?;
        let lit = md.literal(build.server_url_literal.0)?;
        if lit != cfg.server_url.as_bytes() {
            return Err("post-check failed: server URL literal not rewritten".into());
        }
        rep.checks.push(format!(
            "metadata literal #{} = {}",
            build.server_url_literal.0, cfg.server_url
        ));
    }
    if !disabled.contains(&"cleartext") {
        let x = jdr_patch::axml::Axml::parse(&chk.read(patches::MANIFEST)?)?;
        if x.get_android_bool("application", patches::ATTR_USES_CLEARTEXT) != Some(true) {
            return Err("post-check failed: usesCleartextTraffic not set".into());
        }
        rep.checks
            .push("manifest: application usesCleartextTraffic=true".into());
    }
    if !disabled.contains(&"drop-armv7") {
        if chk
            .names()
            .iter()
            .any(|n| n.starts_with("lib/armeabi-v7a/"))
        {
            return Err("post-check failed: armeabi-v7a still present".into());
        }
        rep.checks.push("no lib/armeabi-v7a entries".into());
    }
    if !disabled.contains(&"keep-files") {
        let lib = chk.read(patches::LIBIL2CPP_ARM64)?;
        let (off, _) = build.clear_flag_fn;
        let want: Vec<u8> = patches::RETURN_TRUE
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect();
        if lib.get(off..off + 8) != Some(&want[..]) {
            return Err("post-check failed: keep-files patch not present".into());
        }
        rep.checks.push(format!(
            "libil2cpp.so @{off:#x} = return true (no download-folder wipe)"
        ));
    }
    if !disabled.contains(&"unity-heap") {
        let lib = chk.read(patches::LIBUNITY_ARM64)?;
        let (off, _) = build.unity_default_alloc_ctor;
        if lib.get(off..off + 96) != Some(&patches::unity_heap_bytes()[..]) {
            return Err("post-check failed: libunity.so allocator patch not present".into());
        }
        rep.checks.push(format!(
            "libunity.so @{off:#x} = DynamicHeapAllocator constructor"
        ));
    }
    if !disabled.contains(&"embed-server") {
        let main = chk.read(patches::LIBMAIN_ARM64)?;
        let needed = jdr_patch::elf::needed_of(&main)?;
        if !needed.iter().any(|n| n == "libjdr.so") || needed.iter().any(|n| n == "liblog.so") {
            return Err(format!("post-check failed: libmain.so NEEDED is {needed:?}").into());
        }
        patches::validate_server_lib(&chk.read(patches::SERVER_LIB_ENTRY)?)?;
        if chk.is_compressed(patches::KEYS_ASSET) {
            return Err("post-check failed: keys asset must be stored".into());
        }
        rep.checks.push(format!(
            "libmain.so NEEDED = {needed:?}; libjdr.so valid; keys asset stored"
        ));
    }
    if !disabled.contains(&"english-clientmes") {
        // Expected: every row whose original source matches a translation now holds that English.
        let original = ApkEdit::open(&input)?.read(jdr_patch::clientmes::ENTRY)?;
        let (_, expected, total) = patches::translate_clientmes(&original)?;
        let got = patches::clientmes_english_rows(&chk.read(jdr_patch::clientmes::ENTRY)?)?;
        if got != expected {
            return Err(format!(
                "post-check failed: ClientMes has {got} English rows, expected {expected}"
            )
            .into());
        }
        rep.checks.push(format!(
            "ClientMes: {got} of {total} messages are our English"
        ));
    }
    if cfg.art_zip.is_some() && !disabled.contains(&"ui-art") {
        let msg = match chk.read(jdr_patch::art::ART_ASSET) {
            Ok(b) => {
                if chk.is_compressed(jdr_patch::art::ART_ASSET) {
                    return Err("post-check failed: art pack asset must be stored".into());
                }
                let p = jdr_services::art::ArtPack::from_bin(&b)?;
                format!(
                    "art pack stored ({} images, {} KiB)",
                    p.len(),
                    b.len() / 1024
                )
            }
            Err(_) => "no menu art in the pack".into(),
        };
        rep.checks.push(msg);
    }
    rep.checks.push("zipalign -c -P 16 4 passed".into());
    rep.checks
        .push(format!("apksigner verify: {}", rep.schemes.join("+")));

    let text = rep.render();
    let rp = out.with_extension("report.txt");
    std::fs::write(&rp, &text)?;
    print!("{text}");
    println!("\nreport: {}", rp.display());
    Ok(())
}
