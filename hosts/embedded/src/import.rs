//! Imports the player's game files from a staging folder into the game's own `files/` folder,
//! writing them as the app itself.
//!
//! Why: on Android 11+ files that `adb push` writes into `Android/data/<pkg>/` belong to the shell
//! user and the game cannot read them (seen on device: `vcs.bytes: Permission denied`). Apps may
//! read world-readable files under `/data/local/tmp`, so the player pushes there and the server
//! copies them in. Each file is copied to a temporary name and renamed into place; an unreadable
//! (shell-owned) file already in `files/` is replaced. `vcs.bytes` is copied last so the game never
//! sees a half-imported folder as complete. Files that are already readable with the same size are
//! left alone. The staging folder is only read, never modified.

use std::io::Read;
use std::path::{Path, PathBuf};

pub const STAGING: &str = "/data/local/tmp/jdr-import/files";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImportStats {
    pub copied: usize,
    pub replaced_unreadable: usize,
    pub already_ok: usize,
    pub failed: usize,
    pub bytes: u64,
}

fn readable_with_size(p: &Path, size: u64) -> bool {
    match std::fs::File::open(p) {
        Ok(mut f) => {
            let mut b = [0u8; 1];
            f.metadata().map(|m| m.len() == size).unwrap_or(false)
                && (size == 0 || f.read(&mut b).map(|n| n == 1).unwrap_or(false))
        }
        Err(_) => false,
    }
}

fn copy_one(src: &Path, dst: &Path, stats: &mut ImportStats) -> Result<(), String> {
    let size = std::fs::metadata(src)
        .map_err(|e| format!("{}: {e}", src.display()))?
        .len();
    if readable_with_size(dst, size) {
        stats.already_ok += 1;
        return Ok(());
    }
    if dst.exists() {
        // Present but unreadable (or wrong size): we own the directory, so we may replace it.
        std::fs::remove_file(dst).map_err(|e| format!("cannot replace {}: {e}", dst.display()))?;
        stats.replaced_unreadable += 1;
    }
    let tmp = dst.with_extension("jdrimport.tmp");
    std::fs::copy(src, &tmp)
        .map_err(|e| format!("copy {} -> {}: {e}", src.display(), tmp.display()))?;
    std::fs::rename(&tmp, dst).map_err(|e| format!("rename {}: {e}", dst.display()))?;
    stats.copied += 1;
    stats.bytes += size;
    Ok(())
}

/// Copy everything from `staging` into `dest`; `progress` is called every 500 files.
pub fn import(
    staging: &Path,
    dest: &Path,
    mut progress: impl FnMut(usize, usize, &ImportStats),
) -> Result<ImportStats, String> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(staging)
        .map_err(|e| format!("{}: {e}", staging.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    names.sort();
    // vcs.bytes last
    names.sort_by_key(|p| p.file_name().map(|n| n == "vcs.bytes").unwrap_or(false));
    std::fs::create_dir_all(dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    // Leftovers of an interrupted import (temp names only; never the game's own files).
    if let Ok(rd) = std::fs::read_dir(dest) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with(".jdrimport.tmp") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let total = names.len();
    let mut stats = ImportStats::default();
    for (i, src) in names.iter().enumerate() {
        let Some(name) = src.file_name() else {
            continue;
        };
        if copy_one(src, &dest.join(name), &mut stats).is_err() {
            stats.failed += 1;
        }
        if (i + 1) % 500 == 0 {
            progress(i + 1, total, &stats);
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jdr-import-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn imports_replaces_and_skips() {
        let (s, d) = (dir("src"), dir("dst"));
        std::fs::write(s.join("a.bytes"), b"aaaa").unwrap();
        std::fs::write(s.join("b.bytes"), b"bbbb").unwrap();
        std::fs::write(s.join("vcs.bytes"), b"vcs").unwrap();
        std::fs::write(d.join("a.bytes"), b"aaaa").unwrap(); // already fine
        std::fs::write(d.join("b.bytes"), b"bb").unwrap(); // wrong size -> replaced
        std::fs::write(d.join("x.jdrimport.tmp"), b"leftover").unwrap(); // interrupted import
        let mut calls = 0;
        let st = import(&s, &d, |_, _, _| calls += 1).unwrap();
        assert_eq!(
            (st.copied, st.already_ok, st.replaced_unreadable, st.failed),
            (2, 1, 1, 0)
        );
        assert_eq!(std::fs::read(d.join("b.bytes")).unwrap(), b"bbbb");
        assert_eq!(std::fs::read(d.join("vcs.bytes")).unwrap(), b"vcs");
        assert!(std::fs::read_dir(&d).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        // source untouched
        assert_eq!(std::fs::read(s.join("b.bytes")).unwrap(), b"bbbb");
        // second run: everything already ok
        let st2 = import(&s, &d, |_, _, _| {}).unwrap();
        assert_eq!((st2.copied, st2.already_ok), (0, 3));
    }
}
