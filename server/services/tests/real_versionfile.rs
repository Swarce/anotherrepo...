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
