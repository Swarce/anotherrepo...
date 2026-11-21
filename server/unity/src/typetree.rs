//! Type trees for our own MonoBehaviour classes (the runtime reads the object by these field
//! names and types when the script hash differs from the running build).

use crate::serialized::TypeEntry;
use crate::writer::W;

const ALIGN: i32 = 0x4000;

#[derive(Debug, Clone)]
pub struct TNode {
    pub ty: String,
    pub name: String,
    pub size: i32,
    pub flags: i32,
    pub level: u8,
    pub is_array: bool,
}

/// A serialized field of one of our classes.
#[derive(Debug, Clone)]
pub enum Field {
    String(String),
    I32(String),
    I64(String),
    Bool(String),
    /// `List<T>` of a struct with the given fields.
    List {
        name: String,
        elem_type: String,
        fields: Vec<Field>,
    },
}

impl Field {
    pub fn string(n: &str) -> Self {
        Field::String(n.into())
    }
    pub fn i32(n: &str) -> Self {
        Field::I32(n.into())
    }
    pub fn i64(n: &str) -> Self {
        Field::I64(n.into())
    }
    pub fn bool(n: &str) -> Self {
        Field::Bool(n.into())
    }
}

fn node(
    out: &mut Vec<TNode>,
    level: u8,
    ty: &str,
    name: &str,
    size: i32,
    flags: i32,
    is_array: bool,
) {
    out.push(TNode {
        ty: ty.into(),
        name: name.into(),
        size,
        flags,
        level,
        is_array,
    });
}

/// Unity marks variable-size containers with 0x8000 (and 0xC000 on an Array whose elements
/// contain aligned fields); only 0x4000 affects reading, but we match the engine's own trees.
const VAR: i32 = 0x8000;

fn string_nodes(out: &mut Vec<TNode>, level: u8, name: &str, outer_flags: i32) {
    node(out, level, "string", name, -1, outer_flags | VAR, false);
    node(out, level + 1, "Array", "Array", -1, ALIGN | 1, true);
    node(out, level + 2, "int", "size", 4, 1, false);
    node(out, level + 2, "char", "data", 1, 1, false);
}

fn field_nodes(out: &mut Vec<TNode>, level: u8, f: &Field) {
    match f {
        Field::String(n) => string_nodes(out, level, n, 0),
        Field::I32(n) => node(out, level, "int", n, 4, 0, false),
        Field::I64(n) => node(out, level, "SInt64", n, 8, 0, false),
        Field::Bool(n) => node(out, level, "bool", n, 1, ALIGN, false),
        Field::List {
            name,
            elem_type,
            fields,
        } => {
            node(out, level, "vector", name, -1, VAR, false);
            node(out, level + 1, "Array", "Array", -1, ALIGN | VAR, true);
            node(out, level + 2, "int", "size", 4, 0, false);
            node(out, level + 2, elem_type, "data", -1, VAR, false);
            for c in fields {
                field_nodes(out, level + 3, c);
            }
        }
    }
}

/// The MonoBehaviour base fields (as in every MonoBehaviour of this engine) plus `fields`.
pub fn mono_behaviour_nodes(fields: &[Field]) -> Vec<TNode> {
    let mut v = Vec::new();
    node(&mut v, 0, "MonoBehaviour", "Base", -1, 0x8000, false);
    node(
        &mut v,
        1,
        "PPtr<GameObject>",
        "m_GameObject",
        12,
        0x41,
        false,
    );
    node(&mut v, 2, "int", "m_FileID", 4, 0x41, false);
    node(&mut v, 2, "SInt64", "m_PathID", 8, 0x41, false);
    node(&mut v, 1, "UInt8", "m_Enabled", 1, 0x4101, false);
    node(&mut v, 1, "PPtr<MonoScript>", "m_Script", 12, 0, false);
    node(&mut v, 2, "int", "m_FileID", 4, 0x80_0001, false);
    node(&mut v, 2, "SInt64", "m_PathID", 8, 0x80_0001, false);
    node(&mut v, 1, "string", "m_Name", -1, 0x8_8001, false);
    node(&mut v, 2, "Array", "Array", -1, 0x8_4001, true);
    node(&mut v, 3, "int", "size", 4, 0x8_0001, false);
    node(&mut v, 3, "char", "data", 1, 0x8_0001, false);
    for f in fields {
        field_nodes(&mut v, 1, f);
    }
    v
}

/// Type-tree blob: node count, string buffer size, 24-byte nodes, local string buffer.
pub fn blob(nodes: &[TNode]) -> Vec<u8> {
    let mut strings = W::new();
    let mut offs = std::collections::HashMap::new();
    let mut off = |s: &str, strings: &mut W| -> u32 {
        if let Some(&o) = offs.get(s) {
            return o;
        }
        let o = strings.len() as u32;
        strings.cstr(s);
        offs.insert(s.to_string(), o);
        o
    };
    let mut w = W::new();
    let mut body = W::new();
    for (i, n) in nodes.iter().enumerate() {
        let t = off(&n.ty, &mut strings);
        let nm = off(&n.name, &mut strings);
        body.buf.extend_from_slice(&1u16.to_le_bytes()); // version
        body.u8(n.level);
        body.u8(n.is_array as u8);
        body.u32(t);
        body.u32(nm);
        body.i32(n.size);
        body.i32(i as i32);
        body.i32(n.flags);
    }
    w.i32(nodes.len() as i32);
    w.i32(strings.len() as i32);
    w.bytes(&body.buf);
    w.bytes(&strings.buf);
    w.buf
}

/// A complete type-table entry for a MonoBehaviour class (script index 0).
pub fn mono_behaviour_type(
    fields: &[Field],
    script_id: [u8; 16],
    old_type_hash: [u8; 16],
) -> TypeEntry {
    let mut w = W::new();
    w.i32(crate::CLASS_MONOBEHAVIOUR);
    w.u8(0); // not stripped
    w.i16(0); // script type index
    w.bytes(&script_id);
    w.bytes(&old_type_hash);
    w.bytes(&blob(&mono_behaviour_nodes(fields)));
    TypeEntry {
        class_id: crate::CLASS_MONOBEHAVIOUR,
        raw: w.buf,
    }
}

/// A stable 16-byte hash for our own classes (namespace.class), used as script id and
/// properties hash. It cannot equal the build's own hash, which makes the runtime read the object
/// through the type tree.
pub fn class_hash(ns: &str, class: &str, fields: &[Field]) -> [u8; 16] {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(format!("jdr:{ns}.{class}:{fields:?}").as_bytes());
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nodes_have_levels_and_indices() {
        let f = [
            Field::string("CreateTime"),
            Field::List {
                name: "L".into(),
                elem_type: "X".into(),
                fields: vec![Field::bool("b")],
            },
        ];
        let n = mono_behaviour_nodes(&f);
        assert_eq!(n[0].level, 0);
        assert!(n.iter().skip(1).all(|x| x.level >= 1));
        let b = blob(&n);
        assert_eq!(
            i32::from_le_bytes(b[..4].try_into().unwrap()) as usize,
            n.len()
        );
        let last = n.last().unwrap();
        assert_eq!((last.ty.as_str(), last.flags), ("bool", ALIGN));
    }
}
