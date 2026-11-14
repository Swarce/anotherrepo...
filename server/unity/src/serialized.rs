//! SerializedFile version 17 (Unity 2018.4), little-endian metadata.
//!
//! Header (big-endian): u32 metadata size, u32 file size, u32 version, u32 data offset,
//! u8 endianness, 3 reserved. Metadata: engine version, platform, type-tree flag, types, objects,
//! script references, externals, user information. Object data follows at `data offset`.

use crate::writer::W;
use crate::{Error, SERIALIZED_VERSION};

/// One entry of the type table, kept as raw bytes so engine types can be copied verbatim.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeEntry {
    pub class_id: i32,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub path_id: i64,
    /// Index into the type table.
    pub type_index: i32,
    pub data: Vec<u8>,
}

struct R<'a> {
    b: &'a [u8],
    o: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let s = self
            .b
            .get(self.o..self.o + n)
            .ok_or("serialized file truncated")?;
        self.o += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn i16(&mut self) -> Result<i16, Error> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u32be(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn cstr(&mut self) -> Result<String, Error> {
        let rest = self.b.get(self.o..).ok_or("serialized file truncated")?;
        let end = rest
            .iter()
            .position(|&c| c == 0)
            .ok_or("unterminated string")?;
        let s = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.o += end + 1;
        Ok(s)
    }
}

pub struct Parsed {
    pub engine: String,
    pub platform: i32,
    pub types: Vec<TypeEntry>,
}

/// Parse the header and the type table (enough to copy engine type definitions).
pub fn parse_types(b: &[u8]) -> Result<Parsed, Error> {
    let mut r = R { b, o: 0 };
    let _meta = r.u32be()?;
    let _size = r.u32be()?;
    let version = r.u32be()?;
    let _data = r.u32be()?;
    if version != SERIALIZED_VERSION {
        return Err(format!(
            "serialized file version {version}, expected {SERIALIZED_VERSION}"
        ));
    }
    if r.u8()? != 0 {
        return Err("big-endian metadata not supported".into());
    }
    r.take(3)?;
    let engine = r.cstr()?;
    let platform = r.i32()?;
    let tree = r.u8()? != 0;
    if !tree {
        return Err("file has no type trees".into());
    }
    let n = r.i32()?;
    if !(0..=1024).contains(&n) {
        return Err("bad type count".into());
    }
    let mut types = Vec::new();
    for _ in 0..n {
        let start = r.o;
        let class_id = r.i32()?;
        let _stripped = r.u8()?;
        let _script_index = r.i16()?;
        if class_id == crate::CLASS_MONOBEHAVIOUR {
            r.take(16)?; // script id
        }
        r.take(16)?; // old type hash
        let nodes = r.i32()?;
        let strbuf = r.i32()?;
        if nodes < 0 || strbuf < 0 {
            return Err("bad type tree".into());
        }
        r.take(nodes as usize * 24 + strbuf as usize)?;
        types.push(TypeEntry {
            class_id,
            raw: b[start..r.o].to_vec(),
        });
    }
    Ok(Parsed {
        engine,
        platform,
        types,
    })
}

/// Write a SerializedFile with the given type table, objects and script references.
pub fn write(
    engine: &str,
    platform: i32,
    types: &[TypeEntry],
    objects: &[Object],
    scripts: &[(i32, i64)],
) -> Vec<u8> {
    write_full(
        engine,
        platform,
        types,
        objects,
        scripts,
        &0i32.to_le_bytes(),
        "",
    )
}

/// As [`write`], with a verbatim externals table (count included) and user information string.
pub fn write_full(
    engine: &str,
    platform: i32,
    types: &[TypeEntry],
    objects: &[Object],
    scripts: &[(i32, i64)],
    externals_raw: &[u8],
    user_info: &str,
) -> Vec<u8> {
    let order: Vec<usize> = (0..objects.len()).collect();
    write_layout(
        engine,
        platform,
        types,
        objects,
        &order,
        scripts,
        externals_raw,
        user_info,
        0,
    )
}

/// As [`write_full`], placing object data in `order` (indices into `objects`; the engine's own
/// files do not always store data in object-table order), and starting the data no earlier than
/// `min_data_offset` (the engine sometimes pads it, e.g. to 4096).
#[allow(clippy::too_many_arguments)]
pub fn write_layout(
    engine: &str,
    platform: i32,
    types: &[TypeEntry],
    objects: &[Object],
    order: &[usize],
    scripts: &[(i32, i64)],
    externals_raw: &[u8],
    user_info: &str,
    min_data_offset: usize,
) -> Vec<u8> {
    let mut m = W::new();
    m.cstr(engine);
    m.i32(platform);
    m.u8(1); // type trees enabled
    m.i32(types.len() as i32);
    for t in types {
        m.bytes(&t.raw);
    }
    // Object table entries are 4-byte aligned relative to the start of the file; the metadata
    // starts at offset 20 (4-aligned), so aligning the metadata buffer is equivalent.
    m.i32(objects.len() as i32);
    let mut offsets = vec![0usize; objects.len()];
    let mut data = W::new();
    for &i in order {
        data.align(8);
        offsets[i] = data.len();
        data.bytes(&objects[i].data);
    }
    for (o, off) in objects.iter().zip(&offsets) {
        let pad = (4 - (20 + m.len()) % 4) % 4;
        for _ in 0..pad {
            m.u8(0);
        }
        m.i64(o.path_id);
        m.u32(*off as u32);
        m.u32(o.data.len() as u32);
        m.i32(o.type_index);
    }
    m.i32(scripts.len() as i32);
    for (file, id) in scripts {
        m.i32(*file);
        let pad = (4 - (20 + m.len()) % 4) % 4;
        for _ in 0..pad {
            m.u8(0);
        }
        m.i64(*id);
    }
    m.bytes(externals_raw);
    m.cstr(user_info);
    let header = 20usize;
    let data_offset = ((header + m.len()).div_ceil(16) * 16).max(min_data_offset);
    let file_size = data_offset + data.len();
    let mut w = W::new();
    w.u32be(m.len() as u32);
    w.u32be(file_size as u32);
    w.u32be(SERIALIZED_VERSION);
    w.u32be(data_offset as u32);
    w.u8(0);
    w.bytes(&[0, 0, 0]);
    w.bytes(&m.buf);
    while w.len() < data_offset {
        w.u8(0);
    }
    w.bytes(&data.buf);
    w.buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typetree::{mono_behaviour_type, Field};

    #[test]
    fn write_then_parse_types() {
        let mb = mono_behaviour_type(&[Field::string("A"), Field::i64("B")], [1; 16], [2; 16]);
        let t = vec![mb.clone()];
        let f = write(
            "2018.4.1f1",
            13,
            &t,
            &[Object {
                path_id: 1,
                type_index: 0,
                data: vec![1, 2, 3],
            }],
            &[(0, 5)],
        );
        let p = parse_types(&f).unwrap();
        assert_eq!(p.engine, "2018.4.1f1");
        assert_eq!(p.platform, 13);
        assert_eq!(p.types, t);
        assert!(parse_types(&f[..30]).is_err());
    }
}
