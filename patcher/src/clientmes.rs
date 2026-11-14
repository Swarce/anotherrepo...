//! The in-APK `ClientMes_JP` table (boot, download and error messages shown before any download).
//!
//! It is a Resources file without type trees holding one `ScSystem.ScText.TextData`
//! MonoBehaviour: the ScriptableObject header, `m_Name`, then `Contents: [{Label, Text}]`.

use jdr_unity::sfile;
use jdr_unity::writer::W;

pub type Error = String;

/// Resources file holding `Text/ClientMes_JP` (see docs/research/08-assets-cdn.md).
pub const ENTRY: &str = "assets/bin/Data/fe7671e3f0ccf804aafdca1a0c25d15f";

pub struct Table {
    pub header: Vec<u8>,
    pub name: String,
    pub rows: Vec<(String, String)>,
}

struct R<'a> {
    b: &'a [u8],
    o: usize,
}

impl R<'_> {
    fn i32(&mut self) -> Result<i32, Error> {
        let v = self
            .b
            .get(self.o..self.o + 4)
            .ok_or("ClientMes truncated")?;
        self.o += 4;
        Ok(i32::from_le_bytes(v.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String, Error> {
        let n = self.i32()?;
        if n < 0 {
            return Err("ClientMes: negative string length".into());
        }
        let s = self
            .b
            .get(self.o..self.o + n as usize)
            .ok_or("ClientMes truncated")?;
        self.o += n as usize;
        self.o = self.o.div_ceil(4) * 4;
        String::from_utf8(s.to_vec()).map_err(|_| "ClientMes: invalid UTF-8".into())
    }
}

/// Decode the TextData object (it must be consumed exactly).
pub fn parse_object(d: &[u8]) -> Result<Table, Error> {
    // m_GameObject (12) + m_Enabled (1, aligned to 4) + m_Script (12)
    let header = d.get(..28).ok_or("ClientMes truncated")?.to_vec();
    let mut r = R { b: d, o: 28 };
    let name = r.string()?;
    let n = r.i32()?;
    if !(0..=100_000).contains(&n) {
        return Err("ClientMes: bad row count".into());
    }
    let mut rows = Vec::with_capacity(n as usize);
    for _ in 0..n {
        rows.push((r.string()?, r.string()?));
    }
    if r.o != d.len() {
        return Err(format!("ClientMes: {} trailing bytes", d.len() - r.o));
    }
    Ok(Table { header, name, rows })
}

pub fn write_object(t: &Table) -> Vec<u8> {
    let mut w = W::new();
    w.bytes(&t.header);
    w.string(&t.name);
    w.i32(t.rows.len() as i32);
    for (l, x) in &t.rows {
        w.string(l);
        w.string(x);
    }
    w.buf
}

/// The table from the Resources file.
pub fn read(file: &[u8]) -> Result<Table, Error> {
    let f = sfile::parse(file)?;
    match f.objects.as_slice() {
        [o] => parse_object(&o.data),
        _ => Err("ClientMes: expected exactly one object".into()),
    }
}

/// Replace row texts (`label -> English`) and re-serialize the file. Returns (file, replaced).
pub fn rewrite(
    file: &[u8],
    english: &dyn Fn(&str, &str) -> Option<String>,
) -> Result<(Vec<u8>, usize), Error> {
    let mut f = sfile::parse(file)?;
    if f.objects.len() != 1 {
        return Err("ClientMes: expected exactly one object".into());
    }
    let mut t = parse_object(&f.objects[0].data)?;
    let mut n = 0;
    for (label, text) in t.rows.iter_mut() {
        if let Some(en) = english(label, text) {
            *text = en;
            n += 1;
        }
    }
    f.objects[0].data = write_object(&t);
    Ok((f.write(), n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_round_trip() {
        let t = Table {
            header: vec![0; 28],
            name: "ClientMes_JP".into(),
            rows: vec![("A".into(), "はい".into()), ("B".into(), "x".into())],
        };
        let b = write_object(&t);
        let back = parse_object(&b).unwrap();
        assert_eq!(
            (back.name.as_str(), back.rows.clone()),
            ("ClientMes_JP", t.rows)
        );
        assert!(parse_object(&b[..b.len() - 4]).is_err());
    }

    /// Real APK (JDR_APK): the table parses and an unchanged rewrite is byte-identical.
    #[test]
    fn real_clientmes() {
        let Ok(apk) = std::env::var("JDR_APK") else {
            eprintln!("skipped: set JDR_APK");
            return;
        };
        let mut z = zip::ZipArchive::new(std::fs::File::open(apk).unwrap()).unwrap();
        let mut file = Vec::new();
        std::io::Read::read_to_end(&mut z.by_name(ENTRY).unwrap(), &mut file).unwrap();
        let t = read(&file).unwrap();
        assert_eq!(t.name, "ClientMes_JP");
        assert!(t.rows.len() > 200);
        let (same, n) = rewrite(&file, &|_, _| None).unwrap();
        assert_eq!((same == file, n), (true, 0));
    }
}
