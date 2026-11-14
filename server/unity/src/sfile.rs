//! A whole SerializedFile v17, parsed so that objects can be decoded through their type trees,
//! edited and written back. Everything not edited (types, script table, externals, user info,
//! other objects) is kept byte-for-byte.

use crate::common_strings;
use crate::serialized::{self, Object, TypeEntry};
use crate::typetree::TNode;
use crate::writer::W;
use crate::{Error, SERIALIZED_VERSION};

#[derive(Debug, Clone)]
pub struct SType {
    pub entry: TypeEntry,
    pub nodes: Vec<TNode>,
}

#[derive(Debug, Clone)]
pub struct SFile {
    pub engine: String,
    pub platform: i32,
    pub types: Vec<SType>,
    pub objects: Vec<Object>,
    pub scripts: Vec<(i32, i64)>,
    /// Whether the file carries type trees (Resources files in the APK do not).
    pub type_trees: bool,
    /// Where the object data started in the original file.
    pub data_offset: usize,
    /// Object indices in the order their data is stored.
    pub layout: Vec<usize>,
    /// The externals table, verbatim (count included).
    pub externals_raw: Vec<u8>,
    pub user_info: String,
}

/// Caps against corrupt input (the game's own files are far smaller).
const MAX_TYPES: i32 = 4096;
const MAX_OBJECTS: i32 = 1 << 20;
const MAX_NODES: i32 = 1 << 16;

struct R<'a> {
    b: &'a [u8],
    o: usize,
    base: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let s = self
            .b
            .get(self.o..self.o.checked_add(n).ok_or("overflow")?)
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
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, Error> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
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
    /// Align to 4 relative to `base` (the start of the file).
    fn align4(&mut self) {
        let rel = self.o - self.base;
        self.o += (4 - rel % 4) % 4;
    }
}

fn node_name(off: u32, local: &[u8]) -> Result<String, Error> {
    if off & 0x8000_0000 != 0 {
        return common_strings::lookup(off & 0x7fff_ffff)
            .map(str::to_owned)
            .ok_or_else(|| format!("unknown common string {off:#x}"));
    }
    let rest = local.get(off as usize..).ok_or("bad type-tree string")?;
    let end = rest
        .iter()
        .position(|&c| c == 0)
        .ok_or("bad type-tree string")?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

pub fn parse(b: &[u8]) -> Result<SFile, Error> {
    let mut r = R { b, o: 0, base: 0 };
    let _meta = r.u32be()?;
    let size = r.u32be()? as usize;
    let version = r.u32be()?;
    let data_off = r.u32be()? as usize;
    if version != SERIALIZED_VERSION {
        return Err(format!(
            "serialized file version {version}, expected {SERIALIZED_VERSION}"
        ));
    }
    if r.u8()? != 0 {
        return Err("big-endian metadata not supported".into());
    }
    r.take(3)?;
    if size > b.len() || data_off > b.len() {
        return Err("serialized file header out of range".into());
    }
    let engine = r.cstr()?;
    let platform = r.i32()?;
    let type_trees = r.u8()? != 0;
    let ntypes = r.i32()?;
    if !(0..=MAX_TYPES).contains(&ntypes) {
        return Err("bad type count".into());
    }
    let mut types = Vec::with_capacity(ntypes as usize);
    for _ in 0..ntypes {
        let start = r.o;
        let class_id = r.i32()?;
        let _stripped = r.u8()?;
        let script_index = r.i16()?;
        let _ = script_index;
        if class_id == crate::CLASS_MONOBEHAVIOUR {
            r.take(16)?; // script id
        }
        r.take(16)?;
        if !type_trees {
            types.push(SType {
                entry: TypeEntry {
                    class_id,
                    raw: b[start..r.o].to_vec(),
                },
                nodes: Vec::new(),
            });
            continue;
        }
        let n = r.i32()?;
        let sb = r.i32()?;
        if !(0..=MAX_NODES).contains(&n) || sb < 0 {
            return Err("bad type tree".into());
        }
        let raw_nodes = r.take(n as usize * 24)?;
        let local = r.take(sb as usize)?;
        let mut nodes = Vec::with_capacity(n as usize);
        for c in raw_nodes.chunks_exact(24) {
            let u32at = |i: usize| u32::from_le_bytes(c[i..i + 4].try_into().unwrap());
            nodes.push(TNode {
                ty: node_name(u32at(4), local)?,
                name: node_name(u32at(8), local)?,
                size: u32at(12) as i32,
                flags: u32at(20) as i32,
                level: c[2],
                is_array: c[3] != 0,
            });
        }
        types.push(SType {
            entry: TypeEntry {
                class_id,
                raw: b[start..r.o].to_vec(),
            },
            nodes,
        });
    }
    let nobj = r.i32()?;
    if !(0..=MAX_OBJECTS).contains(&nobj) {
        return Err("bad object count".into());
    }
    let mut objects = Vec::with_capacity(nobj as usize);
    let mut starts = Vec::with_capacity(nobj as usize);
    for _ in 0..nobj {
        r.align4();
        let path_id = r.i64()?;
        let start = r.u32()? as usize;
        let len = r.u32()? as usize;
        let type_index = r.i32()?;
        let at = data_off.checked_add(start).ok_or("overflow")?;
        let data = b
            .get(at..at.checked_add(len).ok_or("overflow")?)
            .ok_or("object out of range")?
            .to_vec();
        if type_index < 0 || type_index >= ntypes {
            return Err("object type index out of range".into());
        }
        starts.push(start);
        objects.push(Object {
            path_id,
            type_index,
            data,
        });
    }
    let nscripts = r.i32()?;
    if !(0..=MAX_OBJECTS).contains(&nscripts) {
        return Err("bad script count".into());
    }
    let mut scripts = Vec::with_capacity(nscripts as usize);
    for _ in 0..nscripts {
        let f = r.i32()?;
        r.align4();
        scripts.push((f, r.i64()?));
    }
    let ext_start = r.o;
    let next = r.i32()?;
    if !(0..=MAX_TYPES).contains(&next) {
        return Err("bad externals count".into());
    }
    for _ in 0..next {
        r.cstr()?;
        r.take(16)?;
        r.i32()?;
        r.cstr()?;
    }
    let externals_raw = b[ext_start..r.o].to_vec();
    let mut layout: Vec<usize> = (0..objects.len()).collect();
    layout.sort_by_key(|&i| starts[i]);
    let user_info = r.cstr().unwrap_or_default();
    Ok(SFile {
        engine,
        platform,
        types,
        objects,
        scripts,
        layout,
        type_trees,
        data_offset: data_off,
        externals_raw,
        user_info,
    })
}

impl SFile {
    /// Serialize back (objects 8-aligned, data 16-aligned, as the engine does).
    pub fn write(&self) -> Vec<u8> {
        let entries: Vec<TypeEntry> = self.types.iter().map(|t| t.entry.clone()).collect();
        serialized::write_layout(
            &self.engine,
            self.platform,
            &entries,
            &self.objects,
            &self.layout,
            &self.scripts,
            &self.externals_raw,
            &self.user_info,
            self.data_offset,
            self.type_trees,
        )
    }
}

// ---------------------------------------------------------------------------------------------
// Generic values through a type tree.

#[derive(Debug, Clone, PartialEq)]
pub enum V {
    I(i64),
    U(u64),
    F32(f32),
    F64(f64),
    /// `string` (raw bytes; the engine stores UTF-8).
    Str(Vec<u8>),
    /// Byte arrays (`vector` of UInt8/char, `TypelessData`), kept raw.
    Bytes(Vec<u8>),
    Arr(Vec<V>),
    Obj(Vec<(String, V)>),
}

impl V {
    pub fn get(&self, k: &str) -> Option<&V> {
        match self {
            V::Obj(f) => f.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn get_mut(&mut self, k: &str) -> Option<&mut V> {
        match self {
            V::Obj(f) => f.iter_mut().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            V::Str(b) => std::str::from_utf8(b).ok(),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            V::I(x) => Some(*x),
            V::U(x) => i64::try_from(*x).ok(),
            _ => None,
        }
    }
}

const ALIGN: i32 = 0x4000;

/// Index one past the last descendant of node `i`.
fn subtree_end(nodes: &[TNode], i: usize) -> usize {
    let lvl = nodes[i].level;
    let mut j = i + 1;
    while j < nodes.len() && nodes[j].level > lvl {
        j += 1;
    }
    j
}

fn children(nodes: &[TNode], i: usize) -> Vec<usize> {
    let end = subtree_end(nodes, i);
    let lvl = nodes[i].level + 1;
    (i + 1..end).filter(|&j| nodes[j].level == lvl).collect()
}

fn prim_size(ty: &str) -> Option<(usize, u8)> {
    // (byte size, kind: 0 signed, 1 unsigned, 2 float)
    Some(match ty {
        "SInt8" => (1, 0),
        "UInt8" | "char" | "bool" => (1, 1),
        "SInt16" | "short" => (2, 0),
        "UInt16" | "unsigned short" => (2, 1),
        "SInt32" | "int" => (4, 0),
        "UInt32" | "unsigned int" | "Type*" => (4, 1),
        "SInt64" | "long long" => (8, 0),
        "UInt64" | "unsigned long long" | "FileSize" => (8, 1),
        "float" => (4, 2),
        "double" => (8, 2),
        _ => return None,
    })
}

/// A 2^24-element cap on any array we decode (corrupt counts must fail, not allocate).
const MAX_ELEMS: usize = 1 << 24;

fn read_node(nodes: &[TNode], i: usize, r: &mut R) -> Result<V, Error> {
    let n = &nodes[i];
    let v = if let Some((sz, kind)) = prim_size(&n.ty) {
        let b = r.take(sz)?;
        let mut a = [0u8; 8];
        a[..sz].copy_from_slice(b);
        let u = u64::from_le_bytes(a);
        match (kind, sz) {
            (2, 4) => V::F32(f32::from_bits(u as u32)),
            (2, _) => V::F64(f64::from_bits(u)),
            (0, _) => {
                let shift = 64 - 8 * sz as u32;
                V::I(((u << shift) as i64) >> shift)
            }
            _ => V::U(u),
        }
    } else if n.ty == "string" {
        let len = r.i32()?;
        if len < 0 {
            return Err("negative string length".into());
        }
        let s = r.take(len as usize)?.to_vec();
        // The Array child carries the align flag.
        if children(nodes, i)
            .first()
            .is_some_and(|&c| nodes[c].flags & ALIGN != 0)
        {
            r.align4();
        }
        V::Str(s)
    } else if n.ty == "TypelessData" {
        let len = r.i32()?;
        if len < 0 {
            return Err("negative data length".into());
        }
        V::Bytes(r.take(len as usize)?.to_vec())
    } else {
        let ch = children(nodes, i);
        if ch.len() == 1 && nodes[ch[0]].is_array {
            let arr = ch[0];
            let ac = children(nodes, arr);
            if ac.len() != 2 {
                return Err(format!("array node {} without size/data", n.name));
            }
            let count = r.i32()?;
            if count < 0 || count as usize > MAX_ELEMS {
                return Err("bad array count".into());
            }
            let data = ac[1];
            let v = if matches!(nodes[data].ty.as_str(), "UInt8" | "char")
                && subtree_end(nodes, data) == data + 1
            {
                V::Bytes(r.take(count as usize)?.to_vec())
            } else {
                let mut items = Vec::with_capacity((count as usize).min(4096));
                for _ in 0..count {
                    items.push(read_node(nodes, data, r)?);
                }
                V::Arr(items)
            };
            if nodes[arr].flags & ALIGN != 0 {
                r.align4();
            }
            v
        } else {
            let mut fields = Vec::with_capacity(ch.len());
            for c in ch {
                fields.push((nodes[c].name.clone(), read_node(nodes, c, r)?));
            }
            V::Obj(fields)
        }
    };
    if n.flags & ALIGN != 0 {
        r.align4();
    }
    Ok(v)
}

/// Decode an object's data through its type tree; the whole object must be consumed.
pub fn read_object(nodes: &[TNode], data: &[u8]) -> Result<V, Error> {
    if nodes.is_empty() {
        return Err("empty type tree".into());
    }
    let mut r = R {
        b: data,
        o: 0,
        base: 0,
    };
    let v = read_node(nodes, 0, &mut r)?;
    if r.o != data.len() {
        return Err(format!("type tree read {} of {} bytes", r.o, data.len()));
    }
    Ok(v)
}

fn write_node(nodes: &[TNode], i: usize, v: &V, w: &mut W) -> Result<(), Error> {
    let n = &nodes[i];
    let bad = || format!("value does not fit node {} {}", n.ty, n.name);
    if let Some((sz, _)) = prim_size(&n.ty) {
        let u: u64 = match v {
            V::I(x) => *x as u64,
            V::U(x) => *x,
            V::F32(f) => f.to_bits() as u64,
            V::F64(f) => f.to_bits(),
            _ => return Err(bad()),
        };
        w.bytes(&u.to_le_bytes()[..sz]);
    } else if n.ty == "string" {
        let V::Str(s) = v else { return Err(bad()) };
        w.i32(s.len() as i32);
        w.bytes(s);
        if children(nodes, i)
            .first()
            .is_some_and(|&c| nodes[c].flags & ALIGN != 0)
        {
            w.align(4);
        }
    } else if n.ty == "TypelessData" {
        let V::Bytes(b) = v else { return Err(bad()) };
        w.i32(b.len() as i32);
        w.bytes(b);
    } else {
        let ch = children(nodes, i);
        if ch.len() == 1 && nodes[ch[0]].is_array {
            let arr = ch[0];
            let data = children(nodes, arr)[1];
            match v {
                V::Bytes(b) => {
                    w.i32(b.len() as i32);
                    w.bytes(b);
                }
                V::Arr(items) => {
                    w.i32(items.len() as i32);
                    for it in items {
                        write_node(nodes, data, it, w)?;
                    }
                }
                _ => return Err(bad()),
            }
            if nodes[arr].flags & ALIGN != 0 {
                w.align(4);
            }
        } else {
            let V::Obj(fields) = v else { return Err(bad()) };
            if fields.len() != ch.len() {
                return Err(bad());
            }
            for (c, (name, fv)) in ch.iter().zip(fields) {
                if &nodes[*c].name != name {
                    return Err(bad());
                }
                write_node(nodes, *c, fv, w)?;
            }
        }
    }
    if n.flags & ALIGN != 0 {
        w.align(4);
    }
    Ok(())
}

/// Encode a value through a type tree (the inverse of [`read_object`]).
pub fn write_object(nodes: &[TNode], v: &V) -> Result<Vec<u8>, Error> {
    let mut w = W::new();
    write_node(nodes, 0, v, &mut w)?;
    Ok(w.buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed::{self, Ty, Val};

    fn sample() -> (Vec<(String, Ty)>, Vec<Val>) {
        let row = Ty::Struct {
            name: "Row".into(),
            fields: vec![
                ("Label".into(), Ty::Str),
                ("Text".into(), Ty::Str),
                ("on".into(), Ty::Bool),
            ],
        };
        let fields = vec![
            ("Contents".into(), Ty::List(Box::new(row))),
            ("v".into(), Ty::Vector3),
            ("n".into(), Ty::List(Box::new(Ty::Int))),
        ];
        let vals = vec![
            Val::List(vec![Val::Struct(vec![
                Val::Str("A".into()),
                Val::Str("xyz".into()),
                Val::Bool(true),
            ])]),
            Val::Floats(vec![1.0, 2.0, 3.0]),
            Val::List(vec![Val::Int(-5), Val::Int(7)]),
        ];
        (fields, vals)
    }

    /// Build a file with our own writer, parse it generically, edit a string, write, re-read.
    #[test]
    fn parse_edit_round_trip() {
        let (fields, vals) = sample();
        let hash = [7u8; 16];
        let ty = managed::mono_behaviour_type(&fields, 0, hash);
        let data = managed::mono_behaviour_data(5, "tbl", &fields, &vals).unwrap();
        let bytes = serialized::write(
            "2018.4.1f1",
            13,
            &[ty],
            &[Object {
                path_id: 9,
                type_index: 0,
                data: data.clone(),
            }],
            &[(0, 5)],
        );
        let f = parse(&bytes).unwrap();
        assert_eq!(f.objects.len(), 1);
        assert_eq!(f.write(), bytes, "unchanged file writes back identically");
        let nodes = &f.types[0].nodes;
        assert_eq!(nodes[0].ty, "MonoBehaviour");
        let mut v = read_object(nodes, &f.objects[0].data).unwrap();
        assert_eq!(
            write_object(nodes, &v).unwrap(),
            data,
            "decode/encode is lossless"
        );
        assert_eq!(v.get("m_Name").and_then(V::as_str), Some("tbl"));
        assert_eq!(v.get("n"), Some(&V::Arr(vec![V::I(-5), V::I(7)])));
        let V::Arr(rows) = v.get_mut("Contents").unwrap() else {
            panic!()
        };
        *rows[0].get_mut("Text").unwrap() = V::Str(b"longer text".to_vec());
        let mut f2 = f.clone();
        f2.objects[0].data = write_object(nodes, &v).unwrap();
        let back = parse(&f2.write()).unwrap();
        let v2 = read_object(&back.types[0].nodes, &back.objects[0].data).unwrap();
        let V::Arr(rows) = v2.get("Contents").unwrap() else {
            panic!()
        };
        assert_eq!(rows[0].get("Text").and_then(V::as_str), Some("longer text"));
        assert_eq!(rows[0].get("on"), Some(&V::U(1)));
    }

    #[test]
    fn corrupt_input_fails_cleanly() {
        let (fields, vals) = sample();
        let ty = managed::mono_behaviour_type(&fields, 0, [1; 16]);
        let data = managed::mono_behaviour_data(5, "t", &fields, &vals).unwrap();
        let f = parse(&serialized::write("2018.4.1f1", 13, &[ty], &[], &[])).unwrap();
        let nodes = &f.types[0].nodes;
        assert!(read_object(nodes, &data[..data.len() - 4]).is_err());
        let mut longer = data.clone();
        longer.extend_from_slice(&[0; 4]);
        assert!(read_object(nodes, &longer).is_err(), "must consume exactly");
        for cut in [0, 10, 40] {
            assert!(parse(&serialized::write("2018.4.1f1", 13, &[], &[], &[])[..cut]).is_err());
        }
    }

    #[test]
    fn common_strings_resolve() {
        assert_eq!(common_strings::lookup(0), Some("AABB"));
        assert_eq!(common_strings::lookup(49), Some("Array"));
        assert_eq!(
            common_strings::lookup(1),
            None,
            "mid-string offsets are refused"
        );
    }
}
