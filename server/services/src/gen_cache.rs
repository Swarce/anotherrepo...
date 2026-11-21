//! The generated bundles (versionfile, in_memory_master_data, the English text bundles' file
//! list) kept on disk between launches, so they are built once per installed APK instead of at
//! every start. A saved build is used only when its key matches: the key covers the installed
//! build, the server's build number and the game files (`Ctx::disk_cache_key`).
//!
//! Layout of the folder: `versionfile.bin`, `masterdata.bin` (or `masterdata.err` with the
//! reason it failed), `localized.tsv` (obfuscated name, tab, served file) and `key.txt`, written
//! last so an interrupted save never matches.

use crate::Generated;
use std::path::{Path, PathBuf};

/// Bump when the saved layout or what goes into a build changes in a way the key does not cover.
pub const FORMAT: u32 = 1;

pub fn load(dir: &Path, key: &str) -> Option<Generated> {
    if std::fs::read_to_string(dir.join("key.txt")).ok()?.trim() != key {
        return None;
    }
    let versionfile = std::fs::read(dir.join("versionfile.bin")).ok()?;
    let masterdata = match std::fs::read(dir.join("masterdata.bin")) {
        Ok(b) => Ok(b),
        Err(_) => Err(std::fs::read_to_string(dir.join("masterdata.err")).ok()?),
    };
    let mut localized = std::collections::HashMap::new();
    for line in std::fs::read_to_string(dir.join("localized.tsv"))
        .ok()?
        .lines()
    {
        let (obf, file) = line.split_once('\t')?;
        let file = PathBuf::from(file);
        // the English files live in the state folder; a missing one means rebuild
        if !file.is_file() {
            return None;
        }
        localized.insert(obf.to_string(), file);
    }
    Some(Generated {
        versionfile,
        masterdata,
        localized,
    })
}

pub fn save(dir: &Path, key: &str, g: &Generated) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    // an older save stops matching before any of its files is replaced
    let _ = std::fs::remove_file(dir.join("key.txt"));
    let write = |name: &str, b: &[u8]| -> std::io::Result<()> {
        let tmp = dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, b)?;
        std::fs::rename(&tmp, dir.join(name))
    };
    write("versionfile.bin", &g.versionfile)?;
    match &g.masterdata {
        Ok(b) => {
            write("masterdata.bin", b)?;
            let _ = std::fs::remove_file(dir.join("masterdata.err"));
        }
        Err(e) => {
            write("masterdata.err", e.as_bytes())?;
            let _ = std::fs::remove_file(dir.join("masterdata.bin"));
        }
    }
    let mut rows: Vec<String> = g
        .localized
        .iter()
        .map(|(o, f)| format!("{o}\t{}", f.display()))
        .collect();
    rows.sort();
    write("localized.tsv", rows.join("\n").as_bytes())?;
    write("key.txt", key.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jdr-gencache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn saved_build_is_reused_only_with_its_key_and_files() {
        let d = tmp("reuse");
        let en = d.join("en.bytes");
        std::fs::write(&en, b"english").unwrap();
        let g = Generated {
            versionfile: b"vf".to_vec(),
            masterdata: Ok(b"md".to_vec()),
            localized: [("abc.bytes".to_string(), en.clone())]
                .into_iter()
                .collect(),
        };
        let dir = d.join("generated");
        save(&dir, "k1", &g).unwrap();
        let l = load(&dir, "k1").expect("same key reuses the build");
        assert_eq!(l.versionfile, b"vf");
        assert_eq!(l.masterdata.unwrap(), b"md");
        assert_eq!(l.localized["abc.bytes"], en);
        // an updated APK (or changed game files) gives another key
        assert!(load(&dir, "k2").is_none());
        // a missing English file means rebuild
        std::fs::remove_file(&en).unwrap();
        assert!(load(&dir, "k1").is_none());
        // a failed master-data build is kept as such
        let g2 = Generated {
            versionfile: b"vf2".to_vec(),
            masterdata: Err("no template".into()),
            localized: Default::default(),
        };
        save(&dir, "k3", &g2).unwrap();
        let l = load(&dir, "k3").unwrap();
        assert_eq!(l.masterdata.unwrap_err(), "no template");
        assert!(!dir.join("masterdata.bin").exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
