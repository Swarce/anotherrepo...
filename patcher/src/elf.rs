//! Minimal ELF64 (little-endian) reading for one job: find a `DT_NEEDED` string in a shared
//! library's dynamic string table and replace it with another name of the same length.

pub type Error = String;

fn u16at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn u64at(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

pub const EM_AARCH64: u16 = 183;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_STRTAB: u64 = 5;
const DT_STRSZ: u64 = 10;
const DT_SONAME: u64 = 14;
const DT_VERNEED: u64 = 0x6fff_fffe;
const DT_VERNEEDNUM: u64 = 0x6fff_ffff;

pub struct Elf<'a> {
    pub data: &'a [u8],
    pub machine: u16,
    phdrs: Vec<(u32, u64, u64, u64, u64)>, // type, offset, vaddr, filesz, align
}

impl<'a> Elf<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, Error> {
        if data.len() < 64 || &data[..4] != b"\x7fELF" || data[4] != 2 || data[5] != 1 {
            return Err("not a 64-bit little-endian ELF".into());
        }
        let machine = u16at(data, 18);
        let phoff = u64at(data, 32) as usize;
        let phentsize = u16at(data, 54) as usize;
        let phnum = u16at(data, 56) as usize;
        if phentsize < 56 || phoff + phnum * phentsize > data.len() {
            return Err("bad program headers".into());
        }
        let phdrs = (0..phnum)
            .map(|i| {
                let o = phoff + i * phentsize;
                (
                    u32at(data, o),
                    u64at(data, o + 8),
                    u64at(data, o + 16),
                    u64at(data, o + 32),
                    u64at(data, o + 48),
                )
            })
            .collect();
        Ok(Self {
            data,
            machine,
            phdrs,
        })
    }

    pub fn load_alignments(&self) -> Vec<u64> {
        self.phdrs
            .iter()
            .filter(|p| p.0 == PT_LOAD)
            .map(|p| p.4)
            .collect()
    }

    fn vaddr_to_off(&self, va: u64) -> Option<usize> {
        self.phdrs
            .iter()
            .filter(|p| p.0 == PT_LOAD)
            .find(|p| va >= p.2 && va < p.2 + p.3)
            .map(|p| (va - p.2 + p.1) as usize)
    }

    /// (tag, value, file offset of the entry) for each dynamic entry up to DT_NULL.
    fn dynamic(&self) -> Result<Vec<(u64, u64, usize)>, Error> {
        let d = self
            .phdrs
            .iter()
            .find(|p| p.0 == PT_DYNAMIC)
            .ok_or("no PT_DYNAMIC")?;
        let mut v = vec![];
        let mut o = d.1 as usize;
        let end = (d.1 + d.3) as usize;
        while o + 16 <= end.min(self.data.len()) {
            let (t, val) = (u64at(self.data, o), u64at(self.data, o + 8));
            if t == DT_NULL {
                break;
            }
            v.push((t, val, o));
            o += 16;
        }
        Ok(v)
    }

    fn strtab(&self) -> Result<(usize, usize), Error> {
        let dy = self.dynamic()?;
        let va = dy
            .iter()
            .find(|e| e.0 == DT_STRTAB)
            .ok_or("no DT_STRTAB")?
            .1;
        let sz = dy.iter().find(|e| e.0 == DT_STRSZ).ok_or("no DT_STRSZ")?.1 as usize;
        let off = self.vaddr_to_off(va).ok_or("DT_STRTAB not in a PT_LOAD")?;
        if off + sz > self.data.len() {
            return Err("string table out of range".into());
        }
        Ok((off, sz))
    }

    fn str_at(&self, idx: usize) -> Result<&'a str, Error> {
        let (off, sz) = self.strtab()?;
        if idx >= sz {
            return Err("string index out of range".into());
        }
        let s = &self.data[off + idx..off + sz];
        let end = s
            .iter()
            .position(|&c| c == 0)
            .ok_or("unterminated string")?;
        std::str::from_utf8(&s[..end]).map_err(|_| "non-UTF-8 string".into())
    }

    pub fn needed(&self) -> Result<Vec<String>, Error> {
        self.dynamic()?
            .iter()
            .filter(|e| e.0 == DT_NEEDED)
            .map(|e| self.str_at(e.1 as usize).map(str::to_owned))
            .collect()
    }

    /// Every dynamic-string offset referenced from the dynamic table, version needs, and symbols
    /// (to prove a string is not shared before editing it).
    fn string_refs(&self) -> Result<Vec<usize>, Error> {
        let dy = self.dynamic()?;
        let mut refs: Vec<usize> = dy
            .iter()
            .filter(|e| e.0 == DT_NEEDED || e.0 == DT_SONAME)
            .map(|e| e.1 as usize)
            .collect();
        if let (Some(vn), Some(num)) = (
            dy.iter().find(|e| e.0 == DT_VERNEED),
            dy.iter().find(|e| e.0 == DT_VERNEEDNUM),
        ) {
            let mut o = self.vaddr_to_off(vn.1).ok_or("DT_VERNEED not mapped")?;
            for _ in 0..num.1 {
                let cnt = u16at(self.data, o + 2) as usize;
                refs.push(u32at(self.data, o + 4) as usize); // vn_file
                let mut a = o + u32at(self.data, o + 8) as usize;
                for _ in 0..cnt {
                    refs.push(u32at(self.data, a + 8) as usize); // vna_name
                    let next = u32at(self.data, a + 12) as usize;
                    if next == 0 {
                        break;
                    }
                    a += next;
                }
                let next = u32at(self.data, o + 12) as usize;
                if next == 0 {
                    break;
                }
                o += next;
            }
        }
        // symbol names: DT_SYMTAB entries until the string table (symtab precedes strtab here)
        if let Some(st) = dy.iter().find(|e| e.0 == 6) {
            let so = self.vaddr_to_off(st.1).ok_or("DT_SYMTAB not mapped")?;
            let (str_off, _) = self.strtab()?;
            if so < str_off {
                let mut o = so;
                while o + 24 <= str_off {
                    refs.push(u32at(self.data, o) as usize);
                    o += 24;
                }
            }
        }
        Ok(refs)
    }
}

/// Replace the `DT_NEEDED` entry `old` with `new` (same byte length) in place.
pub fn replace_needed(data: &[u8], old: &str, new: &str) -> Result<Vec<u8>, Error> {
    if old.len() != new.len() {
        return Err("replacement name must have the same length".into());
    }
    if !new.ends_with(".so")
        || !new
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.+-".contains(&c))
    {
        return Err(format!("{new:?} is not a plain library name"));
    }
    let elf = Elf::parse(data)?;
    let dy = elf.dynamic()?;
    let hits: Vec<_> = dy
        .iter()
        .filter(|e| e.0 == DT_NEEDED && elf.str_at(e.1 as usize).ok() == Some(old))
        .collect();
    if hits.len() != 1 {
        return Err(format!(
            "expected exactly one DT_NEEDED {old}, found {}",
            hits.len()
        ));
    }
    let idx = hits[0].1 as usize;
    let span = idx..idx + old.len() + 1;
    let shared = elf
        .string_refs()?
        .iter()
        .filter(|&&r| span.contains(&r) && r != 0)
        .count();
    if shared != 1 {
        return Err(format!(
            "string {old} is shared by {} references; refusing to edit",
            shared
        ));
    }
    if elf.needed()?.iter().any(|n| n == new) {
        return Err(format!("{new} is already a dependency"));
    }
    let (str_off, _) = elf.strtab()?;
    let mut out = data.to_vec();
    out[str_off + idx..str_off + idx + new.len()].copy_from_slice(new.as_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic ELF64: one PT_LOAD covering everything, PT_DYNAMIC with NEEDED a/b + STRTAB/STRSZ.
    fn synth(shared: bool) -> Vec<u8> {
        let strtab = b"\0liblog.so\0libc.so\0".to_vec();
        let ph_off = 64usize;
        let dyn_off = ph_off + 2 * 56;
        let mut dynv: Vec<(u64, u64)> = vec![
            (DT_NEEDED, 1),
            (DT_NEEDED, 11),
            (DT_STRTAB, 0),
            (DT_STRSZ, strtab.len() as u64),
        ];
        if shared {
            dynv.push((DT_SONAME, 4)); // points inside "liblog.so"
        }
        dynv.push((DT_NULL, 0));
        let str_off = dyn_off + dynv.len() * 16;
        dynv[2].1 = str_off as u64;
        let total = str_off + strtab.len();
        let mut d = vec![0u8; total];
        d[..4].copy_from_slice(b"\x7fELF");
        d[4] = 2;
        d[5] = 1;
        d[18..20].copy_from_slice(&EM_AARCH64.to_le_bytes());
        d[32..40].copy_from_slice(&(ph_off as u64).to_le_bytes());
        d[54..56].copy_from_slice(&56u16.to_le_bytes());
        d[56..58].copy_from_slice(&2u16.to_le_bytes());
        let ph = |d: &mut Vec<u8>, i: usize, t: u32, off: u64, sz: u64, al: u64| {
            let o = ph_off + i * 56;
            d[o..o + 4].copy_from_slice(&t.to_le_bytes());
            d[o + 8..o + 16].copy_from_slice(&off.to_le_bytes());
            d[o + 16..o + 24].copy_from_slice(&off.to_le_bytes());
            d[o + 32..o + 40].copy_from_slice(&sz.to_le_bytes());
            d[o + 48..o + 56].copy_from_slice(&al.to_le_bytes());
        };
        ph(&mut d, 0, PT_LOAD, 0, total as u64, 0x4000);
        ph(
            &mut d,
            1,
            PT_DYNAMIC,
            dyn_off as u64,
            (dynv.len() * 16) as u64,
            8,
        );
        for (i, (t, v)) in dynv.iter().enumerate() {
            let o = dyn_off + i * 16;
            d[o..o + 8].copy_from_slice(&t.to_le_bytes());
            d[o + 8..o + 16].copy_from_slice(&v.to_le_bytes());
        }
        d[str_off..].copy_from_slice(&strtab);
        d
    }

    #[test]
    fn swaps_needed_in_place() {
        let d = synth(false);
        let out = replace_needed(&d, "liblog.so", "libjdr.so").unwrap();
        assert_eq!(out.len(), d.len());
        assert_eq!(
            Elf::parse(&out).unwrap().needed().unwrap(),
            vec!["libjdr.so", "libc.so"]
        );
        assert_eq!(Elf::parse(&out).unwrap().load_alignments(), vec![0x4000]);
    }

    #[test]
    fn refuses_shared_wrong_length_missing_and_duplicate() {
        assert!(replace_needed(&synth(true), "liblog.so", "libjdr.so")
            .unwrap_err()
            .contains("shared"));
        assert!(replace_needed(&synth(false), "liblog.so", "libjdr2.so").is_err());
        assert!(replace_needed(&synth(false), "libz.so", "liby.so").is_err());
        assert!(replace_needed(&synth(false), "liblog.so", "libc.so\0\0").is_err());
        assert!(
            Elf::parse(b"not an elf at all, definitely not, no way, nope, nope, nope, nope!")
                .is_err()
        );
    }
}
