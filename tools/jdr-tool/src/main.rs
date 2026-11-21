//! `jdr-tool` — work with the player's own copy: recover keys, decode names, decrypt files.
//!
//! Keys are written to a local `keys.json` (git-ignored) and never into the repo.

use jdr_crypto::{keyrec, names, Jjec, Vcs};
use std::path::{Path, PathBuf};

mod l10n;

type Error = Box<dyn std::error::Error + Send + Sync>;

const USAGE: &str = "usage:
  jdr-tool keys --data <files dir> --apk <apk> [--soundkey <hex>] --out <keys.json>
  jdr-tool names --data <files dir> --keys <keys.json>            decoded name <TAB> file <TAB> size
  jdr-tool decrypt --keys <keys.json> --in <file> --out <file>     JJEC bundle or vcs.bytes
  jdr-tool l10n export --data <files dir> --keys <keys.json> [--apk <apk>] --out <work.tsv>
  jdr-tool l10n import --in <work.tsv> [--expand-with <export.tsv>] [--repo <checkout>]
                                         merge English into the repo (expand: same source, same English;
                                         --replace: also overwrite rows already translated)";

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let r = match a.first().map(String::as_str) {
        Some("keys") => cmd_keys(&a[1..]),
        Some("names") => cmd_names(&a[1..]),
        Some("decrypt") => cmd_decrypt(&a[1..]),
        Some("l10n") => l10n::cmd(&a[1..]),
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

fn opt(a: &[String], n: &str) -> Option<String> {
    a.iter()
        .position(|x| x == n)
        .and_then(|i| a.get(i + 1).cloned())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Keys {
    pub asset_key: String,
    pub asset_iv: String,
    #[serde(default)]
    pub sound_key: Option<String>,
    #[serde(default)]
    pub http_key: Option<String>,
}

pub fn load_keys(p: &Path) -> Result<Keys, Error> {
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}

/// Obfuscated stems (raw bytes) of every `<hex>.bytes`/`.cpk`-style file in the data dir.
fn stems(dir: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, Error> {
    let mut v = vec![];
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if !p.is_file() {
            continue;
        }
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            if let Some(raw) = names::unhex(stem) {
                v.push((p, raw));
            }
        }
    }
    Ok(v)
}

pub fn recover(dir: &Path) -> Result<(keyrec::NameKeyResult, [u8; 16]), Error> {
    let st = stems(dir)?;
    let raws: Vec<Vec<u8>> = st.iter().map(|(_, r)| r.clone()).collect();
    let nk = keyrec::recover_name_key(&raws)
        .ok_or("not enough obfuscated files to recover the asset key (need 50+)")?;
    if nk.score < 0.99 {
        return Err(format!(
            "asset key recovery is unsure (score {:.4}); is this the game's files/ folder?",
            nk.score
        )
        .into());
    }
    // Solve the IV from the first JJEC files and require agreement.
    let mut ivs = vec![];
    for (p, _) in &st {
        let d = std::fs::read(p)?;
        if Jjec::is_jjec(&d) {
            if let Some(iv) = keyrec::recover_iv(&nk.key, &d) {
                ivs.push(iv);
                if ivs.len() == 5 {
                    break;
                }
            }
        }
    }
    let iv = *ivs
        .first()
        .ok_or("no encrypted bundle matched the recovered key")?;
    if ivs.iter().any(|x| *x != iv) {
        return Err("encrypted bundles disagree on the IV".into());
    }
    Ok((nk, iv))
}

fn cmd_keys(a: &[String]) -> Result<(), Error> {
    let dir = PathBuf::from(opt(a, "--data").ok_or("--data is required")?);
    let out = PathBuf::from(opt(a, "--out").ok_or("--out is required")?);
    let (nk, iv) = recover(&dir)?;
    let printable = |b: &[u8]| b.iter().all(|c| c.is_ascii_graphic());
    if !printable(&nk.key) || !printable(&iv) {
        return Err("recovered key/IV are not printable text; refusing".into());
    }
    let sound_key = opt(a, "--soundkey");
    if let Some(s) = &sound_key {
        u64::from_str_radix(s.trim_start_matches("0x"), 16)
            .map_err(|_| "--soundkey must be hex")?;
    }
    let http_key = match opt(a, "--apk") {
        Some(apk) => Some(http_key_from_apk(Path::new(&apk))?),
        None => None,
    };
    let k = Keys {
        asset_key: String::from_utf8(nk.key.to_vec())?,
        asset_iv: String::from_utf8(iv.to_vec())?,
        sound_key,
        http_key,
    };
    if let Some(d) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&out, serde_json::to_vec_pretty(&k)?)?;
    completeness(&dir, &nk.key, &iv);
    eprintln!(
        "recovered asset key from {} names (score {:.4}), IV verified on encrypted bundles; wrote {}",
        nk.names_used, nk.score, out.display()
    );
    if k.http_key.is_none() {
        eprintln!("note: no --apk given; the server cannot read requests until http_key is set");
    }
    if k.sound_key.is_none() {
        eprintln!("note: no --soundkey given; audio will stay silent until one is set (docs/research/08-assets-cdn.md)");
    }
    Ok(())
}

/// Read the request-envelope key literal from the player's own APK (verified builds only).
pub fn http_key_from_apk(apk: &Path) -> Result<String, Error> {
    use jdr_patch::{apk::ApkEdit, meta::Metadata, patches};
    let mut e = ApkEdit::open(apk)?;
    let build = patches::identify(&mut e)?;
    let data = e.read(patches::METADATA_PATH)?;
    let m = Metadata::parse(&data)?;
    let lit = m.literal(build.http_key_literal)?;
    if lit.len() != 16 || !lit.iter().all(|c| c.is_ascii_graphic()) {
        return Err("envelope key literal has an unexpected shape; refusing".into());
    }
    Ok(String::from_utf8(lit.to_vec())?)
}

/// Compare the folder with the client's own record (`vcs.bytes`): warns if the copy is partial,
/// e.g. after an incomplete MTP copy. A partial copy works for key recovery but not for play.
fn completeness(dir: &Path, key: &[u8], iv: &[u8]) {
    let Ok(raw) = std::fs::read(dir.join("vcs.bytes")) else {
        eprintln!(
            "WARNING: no vcs.bytes in {}; cannot check that your copy is complete",
            dir.display()
        );
        return;
    };
    let Ok((_, _, plain)) = Vcs::decrypt(&raw, key, iv) else {
        eprintln!("WARNING: vcs.bytes did not decrypt; cannot check completeness");
        return;
    };
    let text = String::from_utf8_lossy(&plain);
    let Some(keys) = text
        .find("\"keys\":[")
        .map(|i| &text[i + 8..])
        .and_then(|t| t.split(']').next())
    else {
        eprintln!("WARNING: vcs.bytes has no file list; cannot check completeness");
        return;
    };
    let names: Vec<&str> = keys
        .split(',')
        .map(|n| n.trim().trim_matches('"'))
        .filter(|n| !n.is_empty())
        .collect();
    let missing = names.iter().filter(|n| !dir.join(n).is_file()).count();
    if missing == 0 {
        eprintln!(
            "copy is complete: all {} files recorded by the game are present",
            names.len()
        );
    } else {
        eprintln!("WARNING: {missing} of {} files recorded by the game are MISSING from {}. Copy the folder again (adb pull) before uninstalling anything.", names.len(), dir.display());
    }
}

fn cmd_names(a: &[String]) -> Result<(), Error> {
    let dir = PathBuf::from(opt(a, "--data").ok_or("--data is required")?);
    let k = load_keys(Path::new(&opt(a, "--keys").ok_or("--keys is required")?))?;
    for (p, raw) in stems(&dir)? {
        let n = String::from_utf8_lossy(&names::xor(&raw, k.asset_key.as_bytes())).to_string();
        println!(
            "{n}\t{}\t{}",
            p.file_name().unwrap().to_string_lossy(),
            std::fs::metadata(&p)?.len()
        );
    }
    Ok(())
}

fn cmd_decrypt(a: &[String]) -> Result<(), Error> {
    let k = load_keys(Path::new(&opt(a, "--keys").ok_or("--keys is required")?))?;
    let inp = std::fs::read(opt(a, "--in").ok_or("--in is required")?)?;
    let out = opt(a, "--out").ok_or("--out is required")?;
    let (key, iv) = (k.asset_key.as_bytes(), k.asset_iv.as_bytes());
    let plain = if Jjec::is_jjec(&inp) {
        Jjec::decrypt(&inp, key, iv)?
    } else {
        // vcs.bytes carries a size and an md5 that are checked, so other files are rejected.
        Vcs::decrypt(&inp, key, iv)
            .map_err(|e| format!("not a JJEC bundle or a valid vcs.bytes ({e})"))?
            .2
    };
    std::fs::write(out, plain)?;
    Ok(())
}
