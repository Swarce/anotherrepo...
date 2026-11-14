//! Read an APK, replace/remove/add entries, and write it back. Untouched entries are copied
//! raw (same bytes, same compression). Old signature files are always dropped.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
enum Change {
    Remove,
    Put { data: Vec<u8>, compress: bool },
}

pub struct ApkEdit {
    archive: ZipArchive<File>,
    changes: BTreeMap<String, Change>,
    added_order: Vec<String>,
}

pub fn is_signature_file(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("META-INF/") else {
        return false;
    };
    if rest.contains('/') {
        return false;
    }
    let up = rest.to_ascii_uppercase();
    up == "MANIFEST.MF"
        || [".SF", ".RSA", ".DSA", ".EC"]
            .iter()
            .any(|e| up.ends_with(e))
}

impl ApkEdit {
    pub fn open(path: &Path) -> Result<Self, Error> {
        Ok(Self {
            archive: ZipArchive::new(File::open(path)?)?,
            changes: BTreeMap::new(),
            added_order: vec![],
        })
    }

    pub fn names(&self) -> Vec<String> {
        self.archive.file_names().map(str::to_owned).collect()
    }

    pub fn exists(&self, name: &str) -> bool {
        match self.changes.get(name) {
            Some(Change::Remove) => false,
            Some(Change::Put { .. }) => true,
            None => self.archive.index_for_name(name).is_some(),
        }
    }

    /// Current content of an entry (including pending changes).
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, Error> {
        match self.changes.get(name) {
            Some(Change::Remove) => Err(format!("{name} was removed").into()),
            Some(Change::Put { data, .. }) => Ok(data.clone()),
            None => {
                let mut f = self.archive.by_name(name)?;
                let mut v = Vec::with_capacity(f.size() as usize);
                f.read_to_end(&mut v)?;
                Ok(v)
            }
        }
    }

    pub fn is_compressed(&mut self, name: &str) -> bool {
        self.archive
            .by_name(name)
            .map(|f| f.compression() != CompressionMethod::Stored)
            .unwrap_or(true)
    }

    pub fn put(&mut self, name: &str, data: Vec<u8>, compress: bool) {
        if self.archive.index_for_name(name).is_none() && !self.changes.contains_key(name) {
            self.added_order.push(name.to_owned());
        }
        self.changes
            .insert(name.to_owned(), Change::Put { data, compress });
    }

    pub fn remove(&mut self, name: &str) {
        self.changes.insert(name.to_owned(), Change::Remove);
    }

    pub fn write(mut self, out: &Path) -> Result<WriteStats, Error> {
        let mut w = ZipWriter::new(File::create(out)?);
        let mut st = WriteStats::default();
        let opts = |compress: bool| {
            SimpleFileOptions::default()
                .compression_method(if compress {
                    CompressionMethod::Deflated
                } else {
                    CompressionMethod::Stored
                })
                .last_modified_time(zip::DateTime::default())
        };
        for i in 0..self.archive.len() {
            let name = self.archive.by_index_raw(i)?.name().to_owned();
            if is_signature_file(&name) {
                st.signatures_dropped += 1;
                continue;
            }
            match self.changes.get(&name).cloned() {
                Some(Change::Remove) => st.removed += 1,
                Some(Change::Put { data, compress }) => {
                    w.start_file(name.as_str(), opts(compress))?;
                    w.write_all(&data)?;
                    st.replaced += 1;
                }
                None => {
                    w.raw_copy_file(self.archive.by_index_raw(i)?)?;
                    st.copied += 1;
                }
            }
        }
        for name in &self.added_order {
            if let Some(Change::Put { data, compress }) = self.changes.get(name) {
                w.start_file(name.as_str(), opts(*compress))?;
                w.write_all(data)?;
                st.added += 1;
            }
        }
        w.finish()?;
        Ok(st)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct WriteStats {
    pub copied: usize,
    pub replaced: usize,
    pub removed: usize,
    pub added: usize,
    pub signatures_dropped: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("jdr-apk-test-{}-{name}", std::process::id()));
        d
    }

    fn make_zip(path: &Path) {
        let mut w = ZipWriter::new(File::create(path).unwrap());
        for (n, c, comp) in [
            ("AndroidManifest.xml", b"manifest".as_slice(), true),
            ("META-INF/MANIFEST.MF", b"x", true),
            ("META-INF/CERT.RSA", b"x", true),
            ("META-INF/services/keep.txt", b"keep", true),
            ("lib/armeabi-v7a/libfoo.so", b"arm32", true),
            ("lib/arm64-v8a/libfoo.so", b"arm64", true),
            ("resources.arsc", b"stored", false),
        ] {
            let o = SimpleFileOptions::default().compression_method(if comp {
                CompressionMethod::Deflated
            } else {
                CompressionMethod::Stored
            });
            w.start_file(n, o).unwrap();
            w.write_all(c).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn edit_round_trip() {
        let (a, b) = (tmp("in.zip"), tmp("out.zip"));
        make_zip(&a);
        let mut e = ApkEdit::open(&a).unwrap();
        e.remove("lib/armeabi-v7a/libfoo.so");
        e.put("AndroidManifest.xml", b"patched".to_vec(), true);
        e.put("assets/new.txt", b"new".to_vec(), false);
        assert!(!e.exists("lib/armeabi-v7a/libfoo.so"));
        assert_eq!(e.read("AndroidManifest.xml").unwrap(), b"patched");
        let st = e.write(&b).unwrap();
        assert_eq!(
            st,
            WriteStats {
                copied: 3,
                replaced: 1,
                removed: 1,
                added: 1,
                signatures_dropped: 2
            }
        );
        let mut r = ApkEdit::open(&b).unwrap();
        assert_eq!(r.read("AndroidManifest.xml").unwrap(), b"patched");
        assert_eq!(r.read("META-INF/services/keep.txt").unwrap(), b"keep");
        assert!(!r.exists("META-INF/CERT.RSA"));
        assert!(!r.is_compressed("resources.arsc"));
        assert_eq!(r.read("assets/new.txt").unwrap(), b"new");
        let _ = std::fs::remove_file(a);
        let _ = std::fs::remove_file(b);
    }

    #[test]
    fn signature_file_detection() {
        assert!(is_signature_file("META-INF/BANDAINA.SF"));
        assert!(is_signature_file("META-INF/x.rsa"));
        assert!(!is_signature_file("META-INF/services/a.SF"));
        assert!(!is_signature_file("assets/META-INF/x.RSA"));
    }
}
