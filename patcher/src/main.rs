//! `jdr-patch` — turn the player's own APK into the offline build.

use jdr_patch::apk::{ApkEdit, Error};
use jdr_patch::meta::Metadata;
use jdr_patch::patches::{self, Config, METADATA_PATH};
use jdr_patch::report::Report;
use jdr_patch::sign;
use std::path::PathBuf;

const USAGE: &str = "usage:
  jdr-patch patch --apk <in.apk> --out <out.apk> [--port N] [--disable <patch-id>]... [--keystore <p12>]
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
            let ks = opt(&args[1..], "--keystore")
                .map(PathBuf::from)
                .unwrap_or_else(sign::default_keystore);
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

fn cmd_patch(args: &[String]) -> Result<(), Error> {
    let input = PathBuf::from(opt(args, "--apk").ok_or("--apk is required")?);
    let out = PathBuf::from(opt(args, "--out").ok_or("--out is required")?);
    let keystore = opt(args, "--keystore")
        .map(PathBuf::from)
        .unwrap_or_else(sign::default_keystore);
    let mut cfg = Config::default();
    if let Some(p) = opt(args, "--port") {
        let port: u16 = p.parse().map_err(|_| "bad --port")?;
        cfg.server_url = format!("http://127.0.0.1:{port}");
    }
    let disabled: Vec<&str> = args
        .windows(2)
        .filter(|w| w[0] == "--disable")
        .map(|w| w[1].as_str())
        .collect();
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
            .apply(&mut apk, &cfg)
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
    if sign::ensure_key(&keystore)? {
        eprintln!("created signing key {}", keystore.display());
    }
    let sr = sign::align_and_sign(&unsigned, &out, &keystore)?;
    let _ = std::fs::remove_file(&unsigned);
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
