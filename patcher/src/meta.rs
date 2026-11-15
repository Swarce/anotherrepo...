//! IL2CPP `global-metadata.dat` (v24) string literal table.
//!
//! Header (all u32 LE): magic 0xFAB11BAF, version, then (offset, size) pairs. Pair 0 is the
//! string literal table (entries of `{u32 length, u32 data_index}`), pair 1 the literal data blob.
//! Literals are UTF-8 with an explicit length, so a literal can be shortened in place by writing
//! the new bytes at its data index and lowering its length; nothing else moves.
//!
//! A longer literal is relocated: its bytes are appended at the end of the file and its entry's
//! data index points there (past the blob; the index is relative to the blob's start). The
//! runtime maps the whole file and reads `data_offset + data_index` with no bounds check
//! (`MetadataCache::GetStringLiteralFromIndex`), and nothing else in the file moves.

pub const MAGIC: u32 = 0xFAB1_1BAF;

#[derive(Debug, PartialEq, Eq)]
pub enum MetaError {
    TooShort,
    BadMagic(u32),
    UnsupportedVersion(i32),
    BadIndex(usize),
    TooLong { have: usize, want: usize },
    Shared(usize),
}
impl std::fmt::Display for MetaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MetaError::TooShort => write!(f, "metadata file too short"),
            MetaError::BadMagic(m) => write!(f, "bad metadata magic {m:#x}"),
            MetaError::UnsupportedVersion(v) => write!(f, "unsupported metadata version {v}"),
            MetaError::BadIndex(i) => write!(f, "string literal index {i} out of range"),
            MetaError::TooLong { have, want } => {
                write!(f, "replacement is {want} bytes, only {have} available")
            }
            MetaError::Shared(i) => write!(f, "literal {i} shares bytes with another literal"),
        }
    }
}
impl std::error::Error for MetaError {}

fn rd(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

pub struct Metadata<'a> {
    pub data: &'a [u8],
    pub version: i32,
    lit_off: usize,
    lit_count: usize,
    data_off: usize,
    data_size: usize,
}

impl<'a> Metadata<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, MetaError> {
        if data.len() < 24 {
            return Err(MetaError::TooShort);
        }
        let magic = rd(data, 0);
        if magic != MAGIC {
            return Err(MetaError::BadMagic(magic));
        }
        let version = rd(data, 4) as i32;
        if !(24..=24).contains(&version) {
            return Err(MetaError::UnsupportedVersion(version));
        }
        let lit_off = rd(data, 8) as usize;
        let lit_size = rd(data, 12) as usize;
        let data_off = rd(data, 16) as usize;
        let data_size = rd(data, 20) as usize;
        if lit_off + lit_size > data.len() || data_off + data_size > data.len() {
            return Err(MetaError::TooShort);
        }
        Ok(Self {
            data,
            version,
            lit_off,
            lit_count: lit_size / 8,
            data_off,
            data_size,
        })
    }

    pub fn literal_count(&self) -> usize {
        self.lit_count
    }

    /// (length, data_index) of literal `i`.
    pub fn entry(&self, i: usize) -> Result<(usize, usize), MetaError> {
        if i >= self.lit_count {
            return Err(MetaError::BadIndex(i));
        }
        let o = self.lit_off + i * 8;
        Ok((rd(self.data, o) as usize, rd(self.data, o + 4) as usize))
    }

    /// The bytes of literal `i` (in the blob, or past it for a relocated literal).
    pub fn literal(&self, i: usize) -> Result<&'a [u8], MetaError> {
        let (len, idx) = self.entry(i)?;
        let start = self.data_off + idx;
        if start + len > self.data.len() || (idx < self.data_size && idx + len > self.data_size) {
            return Err(MetaError::BadIndex(i));
        }
        Ok(&self.data[start..start + len])
    }

    /// True if literal `i` lies past the blob (relocated by [`set_literal`]).
    pub fn is_relocated(&self, i: usize) -> Result<bool, MetaError> {
        Ok(self.entry(i)?.1 >= self.data_size)
    }

    /// Indices of all literals equal to `value`.
    pub fn find(&self, value: &[u8]) -> Vec<usize> {
        (0..self.lit_count)
            .filter(|&i| self.literal(i).map(|l| l == value).unwrap_or(false))
            .collect()
    }

    /// True if no other literal overlaps literal `i`'s bytes.
    pub fn is_exclusive(&self, i: usize) -> Result<bool, MetaError> {
        let (len, idx) = self.entry(i)?;
        for j in 0..self.lit_count {
            if j == i {
                continue;
            }
            let (l2, i2) = self.entry(j)?;
            if l2 > 0 && i2 < idx + len && i2 + l2 > idx {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Return a copy of `data` with literal `i` replaced by `new` (which must fit in the old bytes).
pub fn replace_literal(data: &[u8], i: usize, new: &[u8]) -> Result<Vec<u8>, MetaError> {
    let m = Metadata::parse(data)?;
    let (len, idx) = m.entry(i)?;
    m.literal(i)?;
    if new.len() > len {
        return Err(MetaError::TooLong {
            have: len,
            want: new.len(),
        });
    }
    if !m.is_exclusive(i)? {
        return Err(MetaError::Shared(i));
    }
    let mut out = data.to_vec();
    let start = m.data_off + idx;
    out[start..start + new.len()].copy_from_slice(new);
    for b in &mut out[start + new.len()..start + len] {
        *b = 0;
    }
    let o = m.lit_off + i * 8;
    out[o..o + 4].copy_from_slice(&(new.len() as u32).to_le_bytes());
    Ok(out)
}

/// Return a copy of `data` with literal `i` set to `new`: in place when it fits (as
/// [`replace_literal`]), otherwise relocated to the end of the file. Returns (file, relocated).
pub fn set_literal(data: &[u8], i: usize, new: &[u8]) -> Result<(Vec<u8>, bool), MetaError> {
    let m = Metadata::parse(data)?;
    let (len, _) = m.entry(i)?;
    m.literal(i)?;
    if new.len() <= len {
        return Ok((replace_literal(data, i, new)?, false));
    }
    let idx = data.len() - m.data_off;
    if u32::try_from(idx + new.len()).is_err() {
        return Err(MetaError::TooLong {
            have: u32::MAX as usize - idx,
            want: new.len(),
        });
    }
    let mut out = data.to_vec();
    out.extend_from_slice(new);
    let o = m.lit_off + i * 8;
    out[o..o + 4].copy_from_slice(&(new.len() as u32).to_le_bytes());
    out[o + 4..o + 8].copy_from_slice(&(idx as u32).to_le_bytes());
    Ok((out, true))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Build a tiny synthetic v24 metadata file with the given literals.
    pub fn synth(lits: &[&[u8]]) -> Vec<u8> {
        let header = 24usize;
        let table = lits.len() * 8;
        let mut blob = Vec::new();
        let mut entries = Vec::new();
        for l in lits {
            entries.push((l.len() as u32, blob.len() as u32));
            blob.extend_from_slice(l);
        }
        let mut d = Vec::new();
        for v in [
            MAGIC,
            24,
            header as u32,
            table as u32,
            (header + table) as u32,
            blob.len() as u32,
        ] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        for (l, i) in entries {
            d.extend_from_slice(&l.to_le_bytes());
            d.extend_from_slice(&i.to_le_bytes());
        }
        d.extend_from_slice(&blob);
        d
    }

    #[test]
    fn find_and_replace() {
        let d = synth(&[b"alpha", b"https://example.invalid", b"omega"]);
        let m = Metadata::parse(&d).unwrap();
        assert_eq!(m.find(b"https://example.invalid"), vec![1]);
        let out = replace_literal(&d, 1, b"http://127.0.0.1:1").unwrap();
        let m2 = Metadata::parse(&out).unwrap();
        assert_eq!(m2.literal(1).unwrap(), b"http://127.0.0.1:1");
        assert_eq!(m2.literal(0).unwrap(), b"alpha");
        assert_eq!(m2.literal(2).unwrap(), b"omega");
        assert_eq!(out.len(), d.len());
    }

    #[test]
    fn relocates_longer_literals() {
        let d = synth(&[b"ab", b"cd", b"ef"]);
        let (out, moved) = set_literal(&d, 1, b"a longer literal").unwrap();
        assert!(moved);
        // Header, the other entries and the blob are untouched; entry 1 is at 32..40.
        assert_eq!(out[..32], d[..32]);
        assert_eq!(out[40..d.len()], d[40..]);
        assert_eq!(&out[d.len()..], b"a longer literal");
        let m = Metadata::parse(&out).unwrap();
        assert_eq!(m.literal(1).unwrap(), b"a longer literal");
        assert!(m.is_relocated(1).unwrap() && !m.is_relocated(0).unwrap());
        assert_eq!(m.literal(0).unwrap(), b"ab");
        assert_eq!(m.literal(2).unwrap(), b"ef");
        // A second relocation of the same literal appends again; a short one stays in place.
        let (out2, moved2) = set_literal(&out, 1, b"an even longer literal").unwrap();
        assert!(moved2);
        let m2 = Metadata::parse(&out2).unwrap();
        assert_eq!(m2.literal(1).unwrap(), b"an even longer literal");
        let (out3, moved3) = set_literal(&out2, 2, b"x").unwrap();
        assert!(!moved3);
        assert_eq!(out3.len(), out2.len());
        assert_eq!(Metadata::parse(&out3).unwrap().literal(2).unwrap(), b"x");
        // Index out of range, and an entry that straddles the end of the blob.
        assert_eq!(set_literal(&d, 3, b"x"), Err(MetaError::BadIndex(3)));
        let mut bad = d.clone();
        let o = 24 + 8 * 2 + 4;
        bad[o..o + 4].copy_from_slice(&5u32.to_le_bytes());
        assert_eq!(
            Metadata::parse(&bad).unwrap().literal(2),
            Err(MetaError::BadIndex(2))
        );
    }

    #[test]
    fn refuses_longer_and_bad_input() {
        let d = synth(&[b"abc"]);
        assert_eq!(
            replace_literal(&d, 0, b"abcd"),
            Err(MetaError::TooLong { have: 3, want: 4 })
        );
        assert_eq!(replace_literal(&d, 5, b"a"), Err(MetaError::BadIndex(5)));
        assert!(matches!(
            Metadata::parse(&[0u8; 30]),
            Err(MetaError::BadMagic(0))
        ));
    }
}
