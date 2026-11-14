//! IL2CPP `global-metadata.dat` (v24) string literal table.
//!
//! Header (all u32 LE): magic 0xFAB11BAF, version, then (offset, size) pairs. Pair 0 is the
//! string literal table (entries of `{u32 length, u32 data_index}`), pair 1 the literal data blob.
//! Literals are UTF-8 with an explicit length, so a literal can be shortened in place by writing
//! the new bytes at its data index and lowering its length; nothing else moves.

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

    pub fn literal(&self, i: usize) -> Result<&'a [u8], MetaError> {
        let (len, idx) = self.entry(i)?;
        if idx + len > self.data_size {
            return Err(MetaError::BadIndex(i));
        }
        Ok(&self.data[self.data_off + idx..self.data_off + idx + len])
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
