//! Real-data test: needs JDR_DATA (the game's files/ folder) and JDR_KEYS (local keys.json).

use jdr_services::versionfile::{self, md5_hex};

fn env() -> Option<(std::path::PathBuf, String, String)> {
    let d = std::path::PathBuf::from(std::env::var("JDR_DATA").ok()?);
    let k: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::var("JDR_KEYS").ok()?).ok()?).ok()?;
    Some((
        d,
        k["asset_key"].as_str()?.to_string(),
        k["asset_iv"].as_str()?.to_string(),
    ))
}

#[test]
fn builds_versionfile_from_player_files() {
    let Some((dir, key, iv)) = env() else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    if std::env::var("JDR_PRINT_TYPE_MD5").is_ok() {
        // development helper: print the fingerprints of the first template candidate
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let Ok(d) = std::fs::read(e.path()) else {
                continue;
            };
            let Ok(nodes) = jdr_unity::bundle::read(&d) else {
                continue;
            };
            for n in nodes {
                if let Ok(sf) = jdr_unity::serialized::parse_types(&n.data) {
                    let c: Vec<_> = sf
                        .types
                        .iter()
                        .map(|t| (t.class_id, md5_hex(&t.raw)))
                        .collect();
                    if c.iter().any(|x| x.0 == 115) && c.iter().any(|x| x.0 == 142) {
                        eprintln!("TYPES {:?}", c);
                        return;
                    }
                }
            }
        }
    }
    let entries = versionfile::entries_from_files(&dir, key.as_bytes(), iv.as_bytes()).unwrap();
    assert!(!entries.is_empty());
    assert!(entries.iter().any(|e| e.encrypted) && entries.iter().any(|e| !e.encrypted));
    let (ms, ab) = versionfile::find_template_types(&dir, key.as_bytes(), iv.as_bytes()).unwrap();
    let b = versionfile::build(&entries, &ms, &ab, "2026/11/14 00:00:00");
    let out = std::env::var("JDR_VERSIONFILE_OUT").ok();
    if let Some(o) = out {
        std::fs::write(o, &b).unwrap();
    }
    eprintln!("{} entries, bundle {} bytes", entries.len(), b.len());
}

#[test]
fn builds_masterdata_with_player_engine_types() {
    let Some((dir, key, iv)) = env() else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let (ms, ab) = versionfile::find_template_types(&dir, key.as_bytes(), iv.as_bytes()).unwrap();
    let (b, tables) = jdr_services::masterdata::build_embedded(&ms, &ab).unwrap();
    if let Ok(o) = std::env::var("JDR_MASTERDATA_OUT") {
        std::fs::write(o, &b).unwrap();
    }
    eprintln!(
        "{} tables, bundle {} bytes, md5 {}",
        tables.len(),
        b.len(),
        md5_hex(&b)
    );
}

/// Through the server context, as on the phone: the manifest lists the master-data bundle with the
/// md5 and size of exactly the bytes `/cdn` serves.
#[test]
fn manifest_lists_the_served_masterdata() {
    let Some((dir, _, _)) = env() else {
        eprintln!("skipped: set JDR_DATA and JDR_KEYS");
        return;
    };
    let mut s = jdr_core::settings::Settings::new(std::env::temp_dir().join("jdr-md-test"));
    s.game_files_dir = Some(dir);
    s.keys = Some(
        jdr_core::settings::Settings::load_keys(std::path::Path::new(
            &std::env::var("JDR_KEYS").unwrap(),
        ))
        .unwrap(),
    );
    let ctx = jdr_services::Ctx::new(s, false);
    let g = ctx.generated().unwrap();
    let obf = ctx.masterdata_obf_name().unwrap();
    let vf = jdr_unity::bundle::read(&g.versionfile).unwrap();
    // The manifest stores each string length-prefixed; find our entry's fields in order.
    let body = &vf[0].data;
    let find = |needle: &[u8]| body.windows(needle.len()).position(|w| w == needle);
    let at = find(b"in_memory_master_data.bytes").expect("entry listed");
    let rest = &body[at..];
    let pos = |n: &[u8]| rest.windows(n.len()).position(|w| w == n);
    assert!(pos(obf.as_bytes()).is_some(), "obfuscated name follows");
    let md5 = md5_hex(&g.masterdata);
    let m = pos(md5.as_bytes()).expect("md5 of the served bytes follows");
    // Strings are padded to 4 bytes from the start of the file (objects start 8-aligned).
    let size_at = (at + m + 32).div_ceil(4) * 4;
    let size = i64::from_le_bytes(body[size_at..size_at + 8].try_into().unwrap());
    assert_eq!(size, g.masterdata.len() as i64);
}
