//! Builds the `in_memory_master_data` bundle: the game's master tables, which the client fetches
//! into memory at every boot (`TitleLoad`) and never stores, so no player copy has them.
//!
//! The field layouts come from `server/content/masterdata/schema.json` (our analysis of the
//! client's type metadata, docs/research/09-master-data.md); the rows come from our own content
//! files in the same folder. A table without a content file is written empty, which every core
//! table's loader accepts. Engine type definitions (MonoScript, AssetBundle) are copied from the
//! player's own bundles, as for `versionfile`.

use crate::versionfile::md5_hex;
use jdr_unity::bundle::{self, Node};
use jdr_unity::managed::{self, Ty, Val};
use jdr_unity::objects::{self, ContainerEntry};
use jdr_unity::serialized::{self, Object, TypeEntry};
use jdr_unity::{PLATFORM_ANDROID, UNITY_VERSION};
use serde_json::{Map, Value};

pub const BUNDLE_NAME: &str = "in_memory_master_data";
pub const ASSET_DIR: &str = "Assets/Resources/Common/InMemoryMasterData";

pub const SCHEMA_JSON: &str = include_str!("../../content/masterdata/schema.json");

/// Content files (asset name, JSON). Each has `asset`, `sources` and `data` (root field -> value).
pub const TABLES: &[(&str, &str)] = &[(
    "tutorial_flag_table",
    include_str!("../../content/masterdata/tutorial_flag_table.json"),
)];

type Error = String;

/// Nesting limit for type resolution (the deepest real table nests 5 levels).
const MAX_DEPTH: usize = 16;

pub struct AssetDef {
    pub name: String,
    pub class: String,
    /// One of the 18 assets the client names directly (the others are battle tables).
    pub core: bool,
}

pub struct Schema {
    pub assets: Vec<AssetDef>,
    types: Map<String, Value>,
}

/// `Jjs.dtbattle.X` -> (`Jjs.dtbattle`, `X`). Root classes are never nested.
pub fn split_class(full: &str) -> (&str, &str) {
    full.rsplit_once('.').unwrap_or(("", full))
}

/// The type name the engine writes in type trees: `Jjs.A/B` -> `B`, `Ns.C` -> `C`.
pub fn short_name(full: &str) -> &str {
    let tail = full.rsplit('/').next().unwrap_or(full);
    tail.rsplit('.').next().unwrap_or(tail)
}

fn prim(name: &str) -> Option<Ty> {
    Some(match name {
        "int" => Ty::Int,
        "long" => Ty::Long,
        "float" => Ty::Float,
        "bool" => Ty::Bool,
        "string" => Ty::Str,
        "Vector3" => Ty::Vector3,
        "Quaternion" => Ty::Quaternion,
        _ => return None,
    })
}

const SEC_INT: &str = "ScSystem.ScSecurity.SecurityIntValue";
const SEC_FLOAT: &str = "ScSystem.ScSecurity.SecurityFloatValue";
const SEC_STRING: &str = "ScSystem.ScSecurity.SecurityStringValue";

impl Schema {
    pub fn parse(json: &str) -> Result<Self, Error> {
        let v: Value = serde_json::from_str(json).map_err(|e| format!("schema: {e}"))?;
        let types = v
            .get("types")
            .and_then(Value::as_object)
            .ok_or("schema: no types")?
            .clone();
        let assets = v
            .get("assets")
            .and_then(Value::as_array)
            .ok_or("schema: no assets")?
            .iter()
            .map(|a| {
                Ok(AssetDef {
                    name: a["name"].as_str().ok_or("schema: asset name")?.into(),
                    class: a["class"].as_str().ok_or("schema: asset class")?.into(),
                    core: a["core"].as_bool().unwrap_or(false),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Self { assets, types })
    }

    pub fn embedded() -> Result<Self, Error> {
        Self::parse(SCHEMA_JSON)
    }

    fn def(&self, name: &str) -> Result<&Map<String, Value>, Error> {
        self.types
            .get(name)
            .and_then(Value::as_object)
            .ok_or_else(|| format!("schema: unknown type {name}"))
    }

    fn fields_of(&self, name: &str) -> Result<Vec<(String, String, bool)>, Error> {
        self.def(name)?
            .get("fields")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("schema: {name} has no fields"))?
            .iter()
            .map(|f| {
                Ok((
                    f["name"].as_str().ok_or("schema: field name")?.to_string(),
                    f["type"].as_str().ok_or("schema: field type")?.to_string(),
                    f["list"].as_bool().unwrap_or(false),
                ))
            })
            .collect()
    }

    fn is_enum(&self, name: &str) -> bool {
        self.def(name)
            .ok()
            .and_then(|d| d.get("kind"))
            .and_then(Value::as_str)
            == Some("enum")
    }

    /// Resolve a field type (`list` wraps it in a List).
    pub fn ty(&self, name: &str, list: bool, depth: usize) -> Result<Ty, Error> {
        if depth > MAX_DEPTH {
            return Err(format!("schema: {name} nests too deeply"));
        }
        let t = match prim(name) {
            Some(t) => t,
            None if self.is_enum(name) => Ty::Int,
            None => Ty::Struct {
                name: short_name(name).into(),
                fields: self.root_fields_at(name, depth + 1)?,
            },
        };
        Ok(if list { Ty::List(Box::new(t)) } else { t })
    }

    fn root_fields_at(&self, class: &str, depth: usize) -> Result<Vec<(String, Ty)>, Error> {
        self.fields_of(class)?
            .into_iter()
            .map(|(n, t, l)| Ok((n, self.ty(&t, l, depth)?)))
            .collect()
    }

    /// Serialized fields of a root ScriptableObject class.
    pub fn root_fields(&self, class: &str) -> Result<Vec<(String, Ty)>, Error> {
        self.root_fields_at(class, 0)
    }

    /// Convert a JSON value of type `name` (list: an array of them) to a [`Val`].
    pub fn val(&self, name: &str, list: bool, v: &Value, at: &str) -> Result<Val, Error> {
        if list {
            let a = v
                .as_array()
                .ok_or_else(|| format!("{at}: expected an array"))?;
            return a
                .iter()
                .enumerate()
                .map(|(i, x)| self.val(name, false, x, &format!("{at}[{i}]")))
                .collect::<Result<_, _>>()
                .map(Val::List);
        }
        let bad = || format!("{at}: {v} is not a {name}");
        let int = |v: &Value| v.as_i64().ok_or_else(bad);
        let floats = |n: usize, keys: &[&str]| -> Result<Val, Error> {
            let f: Option<Vec<f32>> = match v {
                Value::Array(a) if a.len() == n => {
                    a.iter().map(|x| x.as_f64().map(|f| f as f32)).collect()
                }
                Value::Object(o) if o.len() == n => keys
                    .iter()
                    .map(|k| o.get(*k).and_then(Value::as_f64).map(|f| f as f32))
                    .collect(),
                _ => None,
            };
            f.map(Val::Floats).ok_or_else(bad)
        };
        match name {
            "int" | "long" => return int(v).map(Val::Int),
            "float" => return v.as_f64().map(Val::Float).ok_or_else(bad),
            "bool" => return v.as_bool().map(Val::Bool).ok_or_else(bad),
            "string" => return v.as_str().map(|s| Val::Str(s.into())).ok_or_else(bad),
            "Vector3" => return floats(3, &["x", "y", "z"]),
            "Quaternion" => return floats(4, &["x", "y", "z", "w"]),
            // Stored as {seed, securityValue}; the client reads seed ^ securityValue.
            SEC_INT if v.is_number() => {
                return Ok(Val::Struct(vec![Val::Int(0), Val::Int(int(v)?)]))
            }
            SEC_FLOAT if v.is_number() => {
                let bits = (v.as_f64().ok_or_else(bad)? as f32).to_bits() as i32;
                return Ok(Val::Struct(vec![Val::Int(0), Val::Int(bits as i64)]));
            }
            // The string obfuscation is not decoded yet; an empty string reads as empty.
            SEC_STRING => {
                return match v.as_str() {
                    Some("") => Ok(Val::Struct(vec![Val::Int(0), Val::Str(String::new())])),
                    _ => Err(format!("{at}: only \"\" is supported for {name}")),
                }
            }
            _ => {}
        }
        if self.is_enum(name) {
            if let Some(s) = v.as_str() {
                let values = self.def(name)?.get("values").and_then(Value::as_object);
                return values
                    .and_then(|m| m.get(s))
                    .and_then(Value::as_i64)
                    .map(Val::Int)
                    .ok_or_else(|| format!("{at}: {s:?} is not a value of {name}"));
            }
            return int(v).map(Val::Int);
        }
        let obj = v
            .as_object()
            .ok_or_else(|| format!("{at}: expected an object ({name})"))?;
        let fields = self.fields_of(name)?;
        if let Some(k) = obj.keys().find(|k| !fields.iter().any(|(n, _, _)| n == *k)) {
            return Err(format!("{at}: {name} has no field {k:?}"));
        }
        fields
            .iter()
            .map(|(n, t, l)| match obj.get(n) {
                Some(x) => self.val(t, *l, x, &format!("{at}.{n}")),
                None => Ok(self.ty(t, *l, 0)?.default_val()),
            })
            .collect::<Result<_, _>>()
            .map(Val::Struct)
    }

    /// The values of a root object from a content file's `data` (missing fields: defaults).
    pub fn root_vals(&self, class: &str, data: &Value, at: &str) -> Result<Vec<Val>, Error> {
        match self.val(class, false, data, at)? {
            Val::Struct(v) => Ok(v),
            _ => Err(format!("{at}: not an object")),
        }
    }
}

/// One table in the bundle.
#[derive(Debug)]
pub struct Table {
    pub name: String,
    pub class: String,
    pub fields: Vec<(String, Ty)>,
    pub vals: Vec<Val>,
    /// Rows in the first list field (for the build log).
    pub rows: usize,
}

/// Every core asset (empty unless a content file fills it) plus any other asset with content.
pub fn tables(schema: &Schema, content: &[(&str, &str)]) -> Result<Vec<Table>, Error> {
    let mut parsed = Vec::new();
    for (name, js) in content {
        let v: Value = serde_json::from_str(js).map_err(|e| format!("{name}.json: {e}"))?;
        if v.get("asset").and_then(Value::as_str) != Some(name) {
            return Err(format!("{name}.json: \"asset\" must be {name:?}"));
        }
        if !v
            .get("sources")
            .and_then(Value::as_array)
            .is_some_and(|s| !s.is_empty())
        {
            return Err(format!(
                "{name}.json: a non-empty \"sources\" list is required"
            ));
        }
        if !schema.assets.iter().any(|a| a.name == *name) {
            return Err(format!("{name}.json: no such asset in the schema"));
        }
        parsed.push((name.to_string(), v));
    }
    let mut out = Vec::new();
    for a in &schema.assets {
        let data = parsed
            .iter()
            .find(|(n, _)| *n == a.name)
            .map(|(_, v)| &v["data"]);
        if !a.core && data.is_none() {
            continue; // battle tables: absent until we have content (their readers null-check)
        }
        let fields = schema.root_fields(&a.class)?;
        let vals = match data {
            Some(d) => schema.root_vals(&a.class, d, &a.name)?,
            None => fields.iter().map(|(_, t)| t.default_val()).collect(),
        };
        let rows = vals
            .iter()
            .find_map(|v| match v {
                Val::List(l) => Some(l.len()),
                _ => None,
            })
            .unwrap_or(0);
        out.push(Table {
            name: a.name.clone(),
            class: a.class.clone(),
            fields,
            vals,
            rows,
        });
    }
    Ok(out)
}

pub fn container_path(asset: &str) -> String {
    format!("{ASSET_DIR}/{asset}.asset").to_lowercase()
}

/// Build the bundle. The output depends only on the inputs, so its md5 is stable.
pub fn build(
    tables: &[Table],
    monoscript_type: &TypeEntry,
    assetbundle_type: &TypeEntry,
) -> Result<Vec<u8>, Error> {
    let mut sorted: Vec<&Table> = tables.iter().collect();
    sorted.sort_by_key(|t| container_path(&t.name));
    let mut classes: Vec<&str> = Vec::new();
    for t in &sorted {
        if !classes.contains(&t.class.as_str()) {
            classes.push(&t.class);
        }
    }
    // Path ids: 1 = AssetBundle, then one MonoScript per class, then one object per table.
    let script_id = |ci: usize| 2 + ci as i64;
    let table_id = |ti: usize| 2 + classes.len() as i64 + ti as i64;
    let mut types = vec![monoscript_type.clone(), assetbundle_type.clone()];
    let mut objects_out = Vec::new();
    let mut hashes = Vec::new();
    for (ci, class) in classes.iter().enumerate() {
        let t = sorted
            .iter()
            .find(|t| t.class == *class)
            .expect("class from tables");
        let (ns, cn) = split_class(class);
        let h = managed::layout_hash(ns, cn, &t.fields);
        hashes.push(h);
        types.push(managed::mono_behaviour_type(&t.fields, ci as i16, h));
        objects_out.push(Object {
            path_id: script_id(ci),
            type_index: 0,
            data: objects::monoscript_data(cn, ns, h),
        });
    }
    let mut preload = Vec::new();
    let mut container = Vec::new();
    for (ti, t) in sorted.iter().enumerate() {
        let ci = classes.iter().position(|c| *c == t.class).expect("class");
        objects_out.push(Object {
            path_id: table_id(ti),
            type_index: 2 + ci as i32,
            data: managed::mono_behaviour_data(script_id(ci), &t.name, &t.fields, &t.vals)
                .map_err(|e| format!("{}: {e}", t.name))?,
        });
        container.push(ContainerEntry {
            path: container_path(&t.name),
            preload_index: preload.len() as i32,
            preload_size: 2,
            path_id: table_id(ti),
        });
        preload.push(script_id(ci));
        preload.push(table_id(ti));
    }
    objects_out.insert(
        0,
        Object {
            path_id: 1,
            type_index: 1,
            data: objects::assetbundle_data(BUNDLE_NAME, &preload, &container),
        },
    );
    let scripts: Vec<(i32, i64)> = (0..classes.len()).map(|ci| (0, script_id(ci))).collect();
    let sf = serialized::write(
        UNITY_VERSION,
        PLATFORM_ANDROID,
        &types,
        &objects_out,
        &scripts,
    );
    Ok(bundle::write(
        &[Node {
            name: format!("CAB-{}", md5_hex(BUNDLE_NAME.as_bytes())),
            data: sf,
            flags: 4,
        }],
        UNITY_VERSION,
    ))
}

/// Build from the embedded schema and content.
pub fn build_embedded(ms: &TypeEntry, ab: &TypeEntry) -> Result<(Vec<u8>, Vec<Table>), Error> {
    let t = tables(&Schema::embedded()?, TABLES)?;
    Ok((build(&t, ms, ab)?, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jdr_unity::serialized::parse_types;
    use jdr_unity::writer::W;
    use serde_json::json;

    fn fake_type(class_id: i32) -> TypeEntry {
        let mut w = W::new();
        w.i32(class_id);
        w.u8(0);
        w.i16(-1);
        w.bytes(&[0u8; 16]);
        w.i32(0);
        w.i32(0);
        TypeEntry {
            class_id,
            raw: w.buf,
        }
    }

    #[test]
    fn names() {
        assert_eq!(
            split_class("Jjs.dtbattle.DtBattleDamageParam"),
            ("Jjs.dtbattle", "DtBattleDamageParam")
        );
        assert_eq!(short_name("Jjs.TutorialFlagTable/Param"), "Param");
        assert_eq!(
            short_name("ScSystem.ScSecurity.SecurityIntValue"),
            "SecurityIntValue"
        );
        assert_eq!(
            container_path("Set_el_camera_set_common"),
            "assets/resources/common/inmemorymasterdata/set_el_camera_set_common.asset"
        );
    }

    #[test]
    fn every_schema_class_resolves() {
        let s = Schema::embedded().unwrap();
        assert_eq!(
            s.assets.iter().filter(|a| a.core).count(),
            17,
            "18 core minus signboard_table"
        );
        for a in &s.assets {
            let f = s.root_fields(&a.class).unwrap();
            assert!(!f.is_empty(), "{}", a.class);
            let vals: Vec<Val> = f.iter().map(|(_, t)| t.default_val()).collect();
            managed::mono_behaviour_data(2, &a.name, &f, &vals).unwrap();
        }
    }

    #[test]
    fn tutorial_table_content() {
        let s = Schema::embedded().unwrap();
        let t = tables(&s, TABLES).unwrap();
        assert_eq!(t.len(), 17);
        let tut = t.iter().find(|t| t.name == "tutorial_flag_table").unwrap();
        assert_eq!(tut.rows, 34);
        let Val::List(rows) = &tut.vals[0] else {
            panic!()
        };
        for (i, r) in rows.iter().enumerate() {
            let Val::Struct(f) = r else { panic!() };
            assert_eq!(f[0], Val::Int(i as i64), "ids are dense 0..N-1");
            assert_eq!(f[4], Val::Str(String::new()), "no script pack downloads");
        }
        let labels: Vec<_> = rows
            .iter()
            .map(|r| match r {
                Val::Struct(f) => f[1].clone(),
                _ => unreachable!(),
            })
            .collect();
        for l in [
            "TUTORIAL_00",
            "TUTORIAL_11",
            "HINT_08",
            "HINT_10",
            "HINT_17",
            "OPEN_00",
            "OPEN_04",
        ] {
            assert!(labels.contains(&Val::Str(l.into())), "{l}");
        }
        assert!(!labels.contains(&Val::Str("HINT_09".into())));
        // Enum by name: TUTORIAL_05 is a Home (3) tutorial.
        let Val::Struct(f) = &rows[5] else { panic!() };
        assert_eq!(f[3], Val::Int(3));
    }

    #[test]
    fn json_conversion_rules() {
        let s = Schema::embedded().unwrap();
        // SecurityIntValue from a plain number: seed 0, value as is.
        let v = s.val(SEC_INT, false, &json!(42), "x").unwrap();
        assert_eq!(v, Val::Struct(vec![Val::Int(0), Val::Int(42)]));
        assert!(s.val(SEC_STRING, false, &json!("abc"), "x").is_err());
        assert!(s.val("Vector3", false, &json!([1, 2]), "x").is_err());
        assert_eq!(
            s.val("Vector3", false, &json!({"x":1,"y":2,"z":3}), "x")
                .unwrap(),
            Val::Floats(vec![1.0, 2.0, 3.0])
        );
        // Typos are refused, not dropped.
        let e = s
            .root_vals("Jjs.TutorialFlagTable", &json!({"paramLis": []}), "t")
            .unwrap_err();
        assert!(e.contains("paramLis"), "{e}");
        let e = s
            .val(
                "Jjs.TutorialFlagTable/LaunchType",
                false,
                &json!("Nope"),
                "t",
            )
            .unwrap_err();
        assert!(e.contains("Nope"));
    }

    #[test]
    fn content_file_rules() {
        let s = Schema::embedded().unwrap();
        let no_sources = r#"{"asset":"rank_table","sources":[],"data":{}}"#;
        assert!(tables(&s, &[("rank_table", no_sources)])
            .unwrap_err()
            .contains("sources"));
        let wrong = r#"{"asset":"x","sources":[1],"data":{}}"#;
        assert!(tables(&s, &[("rank_table", wrong)]).is_err());
        let extra = r#"{"asset":"model_table","sources":[{"source":"jdr-default"}],"data":{}}"#;
        assert_eq!(
            tables(&s, &[("model_table", extra)]).unwrap().len(),
            18,
            "extras join once they have content"
        );
    }

    #[test]
    fn bundle_structure() {
        let (b, t) = build_embedded(&fake_type(115), &fake_type(142)).unwrap();
        let (b2, _) = build_embedded(&fake_type(115), &fake_type(142)).unwrap();
        assert_eq!(b, b2, "deterministic");
        let nodes = bundle::read(&b).unwrap();
        let p = parse_types(&nodes[0].data).unwrap();
        let classes: std::collections::BTreeSet<_> = t.iter().map(|t| &t.class).collect();
        assert_eq!(p.types.len(), 2 + classes.len());
        assert_eq!(p.types[0].class_id, 115);
        assert_eq!(p.types[1].class_id, 142);
        assert!(p.types[2..].iter().all(|t| t.class_id == 114));
    }
}
