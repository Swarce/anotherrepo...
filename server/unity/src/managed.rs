//! Type trees and binary data for managed (C#) serialized fields, following the node shapes the
//! engine itself writes in 2018.4 (observed in the player's downloaded bundles):
//!
//! | C# type | nodes |
//! |---|---|
//! | int / enum | `int 4 0x0` |
//! | long | `SInt64 8 0x0` |
//! | float | `float 4 0x0` |
//! | bool | `UInt8 1 0x4100` (one byte, then align to 4) |
//! | string | `string -1 0x8000` > `Array -1 0x4001` > `int size`, `char data` |
//! | Vector3 / Quaternion | `Vector3f 12` / `Quaternionf 16`, flags 0x200000, float children |
//! | [Serializable] class/struct | `<ShortName> size flags` with its fields inline; size is the sum when every field is fixed-size, else -1; 0x8000 when any field aligns |
//! | List/array of a class/struct | `<ShortName> -1 f` > `Array -1 f` > `int size`, `<ShortName> data` (f = 0x8000 if the element aligns) |
//! | List/array of a primitive | `vector -1 0x8000` > `Array -1 0x4000(|0x8000)` > `int size`, `<elem> data` |
//!
//! 0x4000 means "align the stream to 4 after this node"; the others are informational.

use crate::serialized::TypeEntry;
use crate::typetree::{blob, mono_behaviour_nodes, TNode};
use crate::writer::W;
use crate::Error;

const ALIGN: i32 = 0x4000;
const VAR: i32 = 0x8000;
const UNITY_STRUCT: i32 = 0x20_0000;

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Long,
    Float,
    Bool,
    Str,
    Vector3,
    Quaternion,
    /// A `[Serializable]` class or struct: short type name and its fields in declaration order.
    Struct {
        name: String,
        fields: Vec<(String, Ty)>,
    },
    List(Box<Ty>),
}

/// A value of a [`Ty`].
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// Vector3 (3) or Quaternion (4) components.
    Floats(Vec<f32>),
    Struct(Vec<Val>),
    List(Vec<Val>),
}

impl Ty {
    /// True when reading this value realigns the stream (so the parent gets 0x8000).
    pub fn aligns(&self) -> bool {
        match self {
            Ty::Bool | Ty::Str => true,
            // A list of fixed-size structs (no 0x4000 on its Array) does not realign.
            Ty::List(e) => !matches!(e.as_ref(), Ty::Struct { .. }) || e.aligns(),
            Ty::Struct { fields, .. } => fields.iter().any(|(_, t)| t.aligns()),
            _ => false,
        }
    }

    /// Byte size when fixed (bools count 1, as the engine reports them).
    pub fn fixed_size(&self) -> Option<i32> {
        match self {
            Ty::Int | Ty::Float => Some(4),
            Ty::Long => Some(8),
            Ty::Bool => Some(1),
            Ty::Vector3 => Some(12),
            Ty::Quaternion => Some(16),
            Ty::Str | Ty::List(_) => None,
            Ty::Struct { fields, .. } => fields.iter().map(|(_, t)| t.fixed_size()).sum(),
        }
    }

    /// The zero value (empty strings and lists, nested structs filled with zeros).
    pub fn default_val(&self) -> Val {
        match self {
            Ty::Int | Ty::Long => Val::Int(0),
            Ty::Float => Val::Float(0.0),
            Ty::Bool => Val::Bool(false),
            Ty::Str => Val::Str(String::new()),
            Ty::Vector3 => Val::Floats(vec![0.0; 3]),
            Ty::Quaternion => Val::Floats(vec![0.0, 0.0, 0.0, 1.0]),
            Ty::Struct { fields, .. } => {
                Val::Struct(fields.iter().map(|(_, t)| t.default_val()).collect())
            }
            Ty::List(_) => Val::List(Vec::new()),
        }
    }
}

fn push(out: &mut Vec<TNode>, level: u8, ty: &str, name: &str, size: i32, flags: i32, arr: bool) {
    out.push(TNode {
        ty: ty.into(),
        name: name.into(),
        size,
        flags,
        level,
        is_array: arr,
    });
}

/// Append the nodes of one field.
pub fn field_nodes(out: &mut Vec<TNode>, level: u8, name: &str, ty: &Ty) {
    match ty {
        Ty::Int => push(out, level, "int", name, 4, 0, false),
        Ty::Long => push(out, level, "SInt64", name, 8, 0, false),
        Ty::Float => push(out, level, "float", name, 4, 0, false),
        Ty::Bool => push(out, level, "UInt8", name, 1, ALIGN | 0x100, false),
        Ty::Str => {
            push(out, level, "string", name, -1, VAR, false);
            push(out, level + 1, "Array", "Array", -1, ALIGN | 1, true);
            push(out, level + 2, "int", "size", 4, 1, false);
            push(out, level + 2, "char", "data", 1, 1, false);
        }
        Ty::Vector3 | Ty::Quaternion => {
            let (t, comps): (&str, &[&str]) = if *ty == Ty::Vector3 {
                ("Vector3f", &["x", "y", "z"])
            } else {
                ("Quaternionf", &["x", "y", "z", "w"])
            };
            push(
                out,
                level,
                t,
                name,
                comps.len() as i32 * 4,
                UNITY_STRUCT,
                false,
            );
            for c in comps {
                push(out, level + 1, "float", c, 4, UNITY_STRUCT, false);
            }
        }
        Ty::Struct { name: tn, fields } => {
            let flags = if ty.aligns() { VAR } else { 0 };
            push(
                out,
                level,
                tn,
                name,
                ty.fixed_size().unwrap_or(-1),
                flags,
                false,
            );
            for (n, t) in fields {
                field_nodes(out, level + 1, n, t);
            }
        }
        Ty::List(elem) => {
            if let Ty::Struct { name: tn, .. } = elem.as_ref() {
                let f = if elem.aligns() { VAR } else { 0 };
                push(out, level, tn, name, -1, f, false);
                push(out, level + 1, "Array", "Array", -1, f, true);
            } else {
                let f = ALIGN | if elem.aligns() { VAR } else { 0 };
                push(out, level, "vector", name, -1, VAR, false);
                push(out, level + 1, "Array", "Array", -1, f, true);
            }
            push(out, level + 2, "int", "size", 4, 0, false);
            field_nodes(out, level + 2, "data", elem);
        }
    }
}

fn mismatch(ty: &Ty, v: &Val) -> Error {
    format!("value {v:?} does not fit type {ty:?}")
}

/// Write `v` as `ty` in the engine's binary layout.
pub fn write_val(w: &mut W, ty: &Ty, v: &Val) -> Result<(), Error> {
    match (ty, v) {
        (Ty::Int, Val::Int(x)) => {
            w.i32(i32::try_from(*x).map_err(|_| format!("{x} out of int range"))?)
        }
        (Ty::Long, Val::Int(x)) => w.i64(*x),
        (Ty::Float, Val::Float(x)) => w.u32((*x as f32).to_bits()),
        (Ty::Float, Val::Int(x)) => w.u32((*x as f32).to_bits()),
        (Ty::Bool, Val::Bool(b)) => w.bool_aligned(*b),
        (Ty::Str, Val::Str(s)) => w.string(s),
        (Ty::Vector3, Val::Floats(f)) if f.len() == 3 => f.iter().for_each(|x| w.u32(x.to_bits())),
        (Ty::Quaternion, Val::Floats(f)) if f.len() == 4 => {
            f.iter().for_each(|x| w.u32(x.to_bits()))
        }
        (Ty::Struct { fields, .. }, Val::Struct(vals)) if vals.len() == fields.len() => {
            for ((_, t), x) in fields.iter().zip(vals) {
                write_val(w, t, x)?;
            }
        }
        (Ty::List(elem), Val::List(items)) => {
            w.i32(items.len() as i32);
            for x in items {
                write_val(w, elem, x)?;
            }
            if !matches!(elem.as_ref(), Ty::Struct { .. }) {
                w.align(4);
            }
        }
        _ => return Err(mismatch(ty, v)),
    }
    Ok(())
}

/// A type-table entry for a MonoBehaviour class with these fields; `script_index` points into the
/// file's script table.
pub fn mono_behaviour_type(
    fields: &[(String, Ty)],
    script_index: i16,
    hash: [u8; 16],
) -> TypeEntry {
    let mut nodes = mono_behaviour_nodes(&[]);
    for (n, t) in fields {
        field_nodes(&mut nodes, 1, n, t);
    }
    let mut w = W::new();
    w.i32(crate::CLASS_MONOBEHAVIOUR);
    w.u8(0);
    w.i16(script_index);
    w.bytes(&hash); // script id
    w.bytes(&hash); // properties hash
    w.bytes(&blob(&nodes));
    TypeEntry {
        class_id: crate::CLASS_MONOBEHAVIOUR,
        raw: w.buf,
    }
}

/// MonoBehaviour object data: the ScriptableObject header, then the fields.
pub fn mono_behaviour_data(
    script_path_id: i64,
    name: &str,
    fields: &[(String, Ty)],
    vals: &[Val],
) -> Result<Vec<u8>, Error> {
    if fields.len() != vals.len() {
        return Err("field/value count differs".into());
    }
    let mut w = W::new();
    w.pptr(0, 0); // m_GameObject: none for a ScriptableObject
    w.bool_aligned(true); // m_Enabled
    w.pptr(0, script_path_id);
    w.string(name);
    for ((_, t), v) in fields.iter().zip(vals) {
        write_val(&mut w, t, v)?;
    }
    Ok(w.buf)
}

/// A stable hash of a class layout (never equal to the build's own hash, so the runtime reads
/// our objects through the type tree we ship).
pub fn layout_hash(namespace: &str, class: &str, fields: &[(String, Ty)]) -> [u8; 16] {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(format!("jdr:{namespace}.{class}:{fields:?}").as_bytes());
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tutorial_param() -> Ty {
        Ty::Struct {
            name: "Param".into(),
            fields: vec![
                ("id".into(), Ty::Int),
                ("flagLabel".into(), Ty::Str),
                ("autoSave".into(), Ty::Bool),
                ("launchType".into(), Ty::Int),
                ("scriptPackName".into(), Ty::Str),
                ("continueHome".into(), Ty::Bool),
                ("resourceDownloadStopForMainTutorial".into(), Ty::Bool),
            ],
        }
    }

    fn render(nodes: &[TNode]) -> Vec<String> {
        nodes
            .iter()
            .map(|n| {
                format!(
                    "{}{} {} {} {:#x}",
                    "  ".repeat(n.level as usize),
                    n.ty,
                    n.name,
                    n.size,
                    n.flags
                )
            })
            .collect()
    }

    /// The tree from docs/research/09-master-data.md §4.2 (derived from real engine trees).
    #[test]
    fn tutorial_flag_table_tree() {
        let mut n = Vec::new();
        field_nodes(
            &mut n,
            1,
            "paramList",
            &Ty::List(Box::new(tutorial_param())),
        );
        let want = [
            "  Param paramList -1 0x8000",
            "    Array Array -1 0x8000",
            "      int size 4 0x0",
            "      Param data -1 0x8000",
            "        int id 4 0x0",
            "        string flagLabel -1 0x8000",
            "          Array Array -1 0x4001",
            "            int size 4 0x1",
            "            char data 1 0x1",
            "        UInt8 autoSave 1 0x4100",
            "        int launchType 4 0x0",
            "        string scriptPackName -1 0x8000",
            "          Array Array -1 0x4001",
            "            int size 4 0x1",
            "            char data 1 0x1",
            "        UInt8 continueHome 1 0x4100",
            "        UInt8 resourceDownloadStopForMainTutorial 1 0x4100",
        ];
        assert_eq!(render(&n), want);
        assert!(n[1].is_array && n[6].is_array);
    }

    fn sec2() -> Ty {
        Ty::Struct {
            name: "SecurityIntValue".into(),
            fields: vec![("seed".into(), Ty::Int), ("securityValue".into(), Ty::Int)],
        }
    }

    #[test]
    fn primitive_and_fixed_lists() {
        let sec = Ty::Struct {
            name: "SecurityIntValue".into(),
            fields: vec![("seed".into(), Ty::Int), ("securityValue".into(), Ty::Int)],
        };
        let mut n = Vec::new();
        field_nodes(&mut n, 1, "p", &Ty::List(Box::new(sec)));
        field_nodes(&mut n, 1, "v", &Ty::List(Box::new(Ty::Vector3)));
        field_nodes(&mut n, 1, "s", &Ty::List(Box::new(Ty::Str)));
        let r = render(&n);
        assert_eq!(r[0], "  SecurityIntValue p -1 0x0");
        assert_eq!(r[3], "      SecurityIntValue data 8 0x0");
        assert_eq!(r[6], "  vector v -1 0x8000");
        assert_eq!(r[7], "    Array Array -1 0x4000");
        assert_eq!(r[9], "      Vector3f data 12 0x200000");
        assert_eq!(r[14], "    Array Array -1 0xc000");
        // A struct holding only a list of fixed-size structs does not align (engine: flags 0).
        let holder = Ty::Struct {
            name: "SkillData".into(),
            fields: vec![("p".into(), Ty::List(Box::new(sec2())))],
        };
        assert!(!holder.aligns());
    }

    #[test]
    fn binary_layout() {
        let t = Ty::List(Box::new(tutorial_param()));
        let row = Val::Struct(vec![
            Val::Int(7),
            Val::Str("AB".into()),
            Val::Bool(true),
            Val::Int(3),
            Val::Str(String::new()),
            Val::Bool(false),
            Val::Bool(false),
        ]);
        let mut w = W::new();
        write_val(&mut w, &t, &Val::List(vec![row])).unwrap();
        #[rustfmt::skip]
        let want: Vec<u8> = [
            &1i32.to_le_bytes()[..], &7i32.to_le_bytes(),
            &2i32.to_le_bytes(), b"AB\0\0",
            &[1, 0, 0, 0], &3i32.to_le_bytes(),
            &0i32.to_le_bytes(), &[0, 0, 0, 0], &[0, 0, 0, 0],
        ].concat();
        assert_eq!(w.buf, want);
        let mut w = W::new();
        write_val(
            &mut w,
            &Ty::List(Box::new(Ty::Int)),
            &Val::List(vec![Val::Int(1)]),
        )
        .unwrap();
        assert_eq!(w.len(), 8);
        assert!(write_val(&mut w, &Ty::Int, &Val::Str("x".into())).is_err());
        assert!(write_val(&mut w, &Ty::Int, &Val::Int(1 << 40)).is_err());
    }

    #[test]
    fn defaults_fit_their_types() {
        let t = Ty::Struct {
            name: "X".into(),
            fields: vec![
                ("a".into(), tutorial_param()),
                ("q".into(), Ty::Quaternion),
                ("l".into(), Ty::List(Box::new(Ty::Float))),
            ],
        };
        let mut w = W::new();
        write_val(&mut w, &t, &t.default_val()).unwrap();
        assert_eq!(w.len() % 4, 0);
    }
}
