//! Storage primitives: atomic writes, rotating backups, and a refusal to overwrite a file we
//! cannot read back (so a parse bug never destroys a save).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const BACKUPS: usize = 5;

/// Write via `<file>.tmp` + fsync + rename.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

fn backup_path(path: &Path, n: usize) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(format!(".bak{n}"));
    PathBuf::from(s)
}

/// Rotate `<file>.bak1..bakN` (bak1 newest) and copy the current file to bak1.
pub fn rotate_backups(path: &Path, keep: usize) -> std::io::Result<()> {
    if !path.exists() || keep == 0 {
        return Ok(());
    }
    let _ = fs::remove_file(backup_path(path, keep));
    for n in (1..keep).rev() {
        let a = backup_path(path, n);
        if a.exists() {
            fs::rename(&a, backup_path(path, n + 1))?;
        }
    }
    fs::copy(path, backup_path(path, 1))?;
    Ok(())
}

#[derive(Debug)]
pub enum SaveError {
    Io(std::io::Error),
    /// The existing file failed `validate`; we refuse to overwrite it.
    ExistingUnreadable(String),
}
impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "{e}"),
            SaveError::ExistingUnreadable(w) => {
                write!(f, "refusing to overwrite unreadable save: {w}")
            }
        }
    }
}
impl std::error::Error for SaveError {}

/// Save with backups. `validate` must accept the existing file before it is replaced.
pub fn save_guarded(
    path: &Path,
    data: &[u8],
    validate: impl Fn(&[u8]) -> Result<(), String>,
) -> Result<(), SaveError> {
    if path.exists() {
        let cur = fs::read(path).map_err(SaveError::Io)?;
        validate(&cur).map_err(SaveError::ExistingUnreadable)?;
        rotate_backups(path, BACKUPS).map_err(SaveError::Io)?;
    }
    write_atomic(path, data).map_err(SaveError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jdr-storage-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn backups_rotate_and_cap() {
        let d = dir("rot");
        let p = d.join("save.json");
        let ok = |_: &[u8]| Ok(());
        for i in 0..8 {
            save_guarded(&p, format!("{{\"v\":{i}}}").as_bytes(), ok).unwrap();
        }
        assert_eq!(fs::read_to_string(&p).unwrap(), "{\"v\":7}");
        assert_eq!(fs::read_to_string(backup_path(&p, 1)).unwrap(), "{\"v\":6}");
        assert_eq!(fs::read_to_string(backup_path(&p, 5)).unwrap(), "{\"v\":2}");
        assert!(!backup_path(&p, 6).exists());
    }

    #[test]
    fn refuses_to_overwrite_unreadable() {
        let d = dir("bad");
        let p = d.join("save.json");
        fs::write(&p, b"garbage").unwrap();
        let v = |b: &[u8]| {
            serde_json::from_slice::<serde_json::Value>(b)
                .map(|_| ())
                .map_err(|e| e.to_string())
        };
        assert!(matches!(
            save_guarded(&p, b"{}", v),
            Err(SaveError::ExistingUnreadable(_))
        ));
        assert_eq!(fs::read(&p).unwrap(), b"garbage");
    }
}
