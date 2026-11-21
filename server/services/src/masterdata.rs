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
pub const TABLES: &[(&str, &str)] = &[
    (
        "tutorial_flag_table",
        include_str!("../../content/masterdata/tutorial_flag_table.json"),
    ),
    (
        "dtbattle_damage_param",
        include_str!("../../content/masterdata/dtbattle_damage_param.json"),
    ),
    (
        "dtbattle_damage_rank_table",
        include_str!("../../content/masterdata/dtbattle_damage_rank_table.json"),
    ),
    (
        "area_position_pattern_table",
        include_str!("../../content/masterdata/area_position_pattern_table.json"),
    ),
    (
        "Set_el_camera_set_common",
        include_str!("../../content/masterdata/Set_el_camera_set_common.json"),
    ),
    (
        "Node_el_camera_node_common",
        include_str!("../../content/masterdata/Node_el_camera_node_common.json"),
    ),
    (
        "team_rank_table",
        include_str!("../../content/masterdata/team_rank_table.json"),
    ),
    (
        "mind_level_table",
        include_str!("../../content/masterdata/mind_level_table.json"),
    ),
    (
        "dtbattle_reaction_table_normal",
        include_str!("../../content/masterdata/dtbattle_reaction_table_normal.json"),
    ),
    (
        "dtbattle_reaction_table_mini",
        include_str!("../../content/masterdata/dtbattle_reaction_table_mini.json"),
    ),
    (
        "dtbattle_reaction_table_mini_no_blow",
        include_str!("../../content/masterdata/dtbattle_reaction_table_mini_no_blow.json"),
    ),
    (
        "reaction_table_normal",
        include_str!("../../content/masterdata/reaction_table_normal.json"),
    ),
    (
        "reaction_table_mini",
        include_str!("../../content/masterdata/reaction_table_mini.json"),
    ),
    (
        "reaction_table_mini_no_blow",
        include_str!("../../content/masterdata/reaction_table_mini_no_blow.json"),
    ),
    (
        "model_table",
        include_str!("../../content/masterdata/model_table.json"),
    ),
    (
        "bullet_parameter",
        include_str!("../../content/masterdata/bullet_parameter.json"),
    ),
    (
        "battle_damage_param",
        include_str!("../../content/masterdata/battle_damage_param.json"),
    ),
    (
        "battle_combo_param",
        include_str!("../../content/masterdata/battle_combo_param.json"),
    ),
    (
        "battle_pvp_statue_status_parameter",
        include_str!("../../content/masterdata/battle_pvp_statue_status_parameter.json"),
    ),
    (
        "yogo_table",
        include_str!("../../content/masterdata/yogo_table.json"),
    ),
    (
        "characteristic_data",
        include_str!("../../content/masterdata/characteristic_data.json"),
    ),
    (
        "dtbattle_statue_affinity_table",
        include_str!("../../content/masterdata/dtbattle_statue_affinity_table.json"),
    ),
    (
        "statue_affinity_table",
        include_str!("../../content/masterdata/statue_affinity_table.json"),
    ),
    (
        "oraoramode_setting",
        include_str!("../../content/masterdata/oraoramode_setting.json"),
    ),
    (
        "affect_table",
        include_str!("../../content/masterdata/affect_table.json"),
    ),
    (
        "affect_text_rule_table",
        include_str!("../../content/masterdata/affect_text_rule_table.json"),
    ),
    (
        "clan_table",
        include_str!("../../content/masterdata/clan_table.json"),
    ),
    (
        "clan_object_table",
        include_str!("../../content/masterdata/clan_object_table.json"),
    ),
    (
        "effectReductionParam",
        include_str!("../../content/masterdata/effectReductionParam.json"),
    ),
    (
        "state_abnormality_table",
        include_str!("../../content/masterdata/state_abnormality_table.json"),
    ),
    (
        "characterestic_skill_parameter",
        include_str!("../../content/masterdata/characterestic_skill_parameter.json"),
    ),
    (
        "skill_data_table",
        include_str!("../../content/masterdata/skill_data_table.json"),
    ),
    (
        "dtbattle_ability_data_table",
        include_str!("../../content/masterdata/dtbattle_ability_data_table.json"),
    ),
];

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
        if list && t == Ty::Bool {
            // The engine packs bool arrays at one byte each; not needed yet, so not written.
            return Err(format!("schema: List<bool> ({name}) is not supported yet"));
        }
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
            // {seed, securityString}: the client splits the string on '-', parses each part as a
            // hex byte, XORs byte i with byte i%4 of the little-endian seed and reads ASCII
            // (SecurityStringValue.get_Value -> 0x105FAA4). With seed 0 that is the ASCII bytes as
            // uppercase hex joined by '-'; "" stays "".
            SEC_STRING => {
                let s = v.as_str().ok_or_else(bad)?;
                if !s.is_ascii() {
                    return Err(format!("{at}: {name} must be ASCII"));
                }
                return Ok(Val::Struct(vec![Val::Int(0), Val::Str(security_string(s))]));
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

/// The `securityString` of a SecurityStringValue with seed 0.
pub fn security_string(s: &str) -> String {
    s.bytes()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join("-")
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

/// CONTRIBUTING: every content file names its sources. `jdr-research` (our own analysis, with the
/// doc it is in) and `jdr-default` (a value we chose) need a `doc`/`note`; any other source is an
/// outside site and needs its `page` and the date it was `accessed`.
pub fn check_sources(v: &Value) -> Result<(), Error> {
    let list = v
        .get("sources")
        .and_then(Value::as_array)
        .filter(|s| !s.is_empty())
        .ok_or("a non-empty \"sources\" list is required")?;
    for (i, s) in list.iter().enumerate() {
        let field = |k: &str| s.get(k).and_then(Value::as_str).filter(|x| !x.is_empty());
        let src = field("source").ok_or_else(|| format!("sources[{i}] needs a \"source\""))?;
        let ok = match src {
            "jdr-research" => field("doc").is_some(),
            "jdr-default" => field("note").is_some(),
            _ => field("page").is_some() && field("accessed").is_some(),
        };
        if !ok {
            return Err(format!(
                "sources[{i}] ({src}): jdr-research needs \"doc\", jdr-default needs \"note\", an outside source needs \"page\" and \"accessed\""
            ));
        }
    }
    Ok(())
}

/// Every core asset (empty unless a content file fills it) plus any other asset with content.
pub fn tables(schema: &Schema, content: &[(&str, &str)]) -> Result<Vec<Table>, Error> {
    let mut parsed = Vec::new();
    for (name, js) in content {
        let v: Value = serde_json::from_str(js).map_err(|e| format!("{name}.json: {e}"))?;
        if v.get("asset").and_then(Value::as_str) != Some(name) {
            return Err(format!("{name}.json: \"asset\" must be {name:?}"));
        }
        check_sources(&v).map_err(|e| format!("{name}.json: {e}"))?;
        if parsed.iter().any(|(n, _): &(String, Value)| n == name) {
            return Err(format!("{name}.json: listed twice"));
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
            continue; // battle tables: absent until we have content (their boot readers null-check)
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
        // Object, then its script: the order the engine writes (text.jp).
        preload.push(table_id(ti));
        preload.push(script_id(ci));
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
/// Tables generated from other content (the statue roster): (asset, content-file JSON).
pub fn generated_tables() -> Vec<(String, String)> {
    // Each generated table carries the sources of the content file its rows come from.
    let sources_of = |content: &str| -> Value {
        serde_json::from_str::<Value>(content)
            .ok()
            .and_then(|v| v.get("sources").cloned())
            .unwrap_or(Value::Null)
    };
    let mut statue_src = sources_of(crate::collection::STATUES_JSON);
    if let Some(list) = statue_src.as_array_mut() {
        list.push(serde_json::json!({ "source": "jdr-research",
            "doc": "docs/research/11-battle-and-positions.md §1",
            "note": "statue_setting_table: position units, camera object, team scale" }));
        list.push(serde_json::json!({ "source": "jdr-default",
            "note": "statue_setting_table: the My Page position/rotation, background choice, mindFrameID and PvP AI ids are our values" }));
    }
    let rank_src = sources_of(crate::collection::RANKS_JSON);
    let assist_src = sources_of(crate::collection::ASSIST_ABILITIES_JSON);
    let skills = crate::collection::dt_skill_table();
    let doc = |asset: &str, src: &Value, rows: Vec<Value>| {
        serde_json::json!({ "asset": asset, "sources": src, "data": { "paramList": rows } })
            .to_string()
    };
    vec![
        (
            "statue_setting_table".into(),
            doc(
                "statue_setting_table",
                &statue_src,
                crate::collection::statue_setting_rows(),
            ),
        ),
        (
            "statue_level_table".into(),
            doc(
                "statue_level_table",
                &statue_src,
                crate::collection::statue_level_rows(),
            ),
        ),
        (
            "dtbattle_skill_data_table".into(),
            serde_json::json!({ "asset": "dtbattle_skill_data_table", "sources": skills["sources"],
                                "data": skills["data"] })
            .to_string(),
        ),
        (
            "rank_table".into(),
            doc("rank_table", &rank_src, crate::collection::rank_rows()),
        ),
        (
            "assist_ability_condition_table".into(),
            doc(
                "assist_ability_condition_table",
                &assist_src,
                crate::collection::assist_condition_rows(),
            ),
        ),
    ]
}

/// Build from the embedded schema and content (plus the generated tables).
pub fn build_embedded(ms: &TypeEntry, ab: &TypeEntry) -> Result<(Vec<u8>, Vec<Table>), Error> {
    let generated = generated_tables();
    let mut all: Vec<(&str, &str)> = TABLES.to_vec();
    all.extend(generated.iter().map(|(a, b)| (a.as_str(), b.as_str())));
    let t = tables(&Schema::embedded()?, &all)?;
    Ok((build(&t, ms, ab)?, t))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jdr_unity::serialized::parse_types;
    use jdr_unity::writer::W;

    /// The content files the server reads directly (not through the schema) follow the same rule
    /// as master-data content: a valid file-level `sources` list, and every record cites a source
    /// that is valid by itself.
    #[test]
    fn content_files_cite_their_sources() {
        let files = [
            ("statues", crate::collection::STATUES_JSON, "statues"),
            ("minds", crate::collection::MINDS_JSON, "minds"),
            ("ranks", crate::collection::RANKS_JSON, ""),
            ("quests", crate::quests::QUESTS_JSON, "chapters"),
            (
                "action_abilities",
                crate::collection::ACTION_ABILITIES_JSON,
                "leaders",
            ),
            ("statue_traits", crate::collection::STATUE_TRAITS_JSON, ""),
            ("jojo_links", crate::collection::JOJO_LINKS_JSON, "links"),
        ];
        for (name, js, records) in files {
            let v: Value = serde_json::from_str(js).unwrap();
            check_sources(&v).unwrap_or_else(|e| panic!("{name}.json: {e}"));
            if records.is_empty() {
                continue;
            }
            let mut list: Vec<&Value> = v[records].as_array().unwrap().iter().collect();
            if name == "quests" {
                list = list
                    .iter()
                    .flat_map(|c| c["stages"].as_array().unwrap())
                    .collect();
            }
            assert!(!list.is_empty(), "{name}.json has no records");
            for (i, r) in list.iter().enumerate() {
                let one = serde_json::json!({ "sources": [r["source"].clone()] });
                check_sources(&one).unwrap_or_else(|e| panic!("{name}.json record {i}: {e}"));
            }
        }
        for (asset, js) in generated_tables() {
            let v: Value = serde_json::from_str(&js).unwrap();
            check_sources(&v).unwrap_or_else(|e| panic!("generated {asset}: {e}"));
        }
    }
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

    /// What the tactical battle dereferences without null checks (docs/research/11): a damage
    /// row, a non-empty rank table, a skill row for every skill a unit gets (the client's two
    /// fixed ones and our basic attack), a category row for every skill, and the formation row.
    #[test]
    fn tactical_battle_tables_are_consistent() {
        let doc = |name: &str| -> Value {
            let js = TABLES.iter().find(|(n, _)| *n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        assert_eq!(
            doc("dtbattle_damage_param")["param"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(!doc("dtbattle_damage_rank_table")["table"]
            .as_array()
            .unwrap()
            .is_empty());
        let sk = crate::collection::dt_skill_table()["data"].clone();
        let ids: Vec<&str> = sk["skillDataList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["dtSkillId"].as_str().unwrap())
            .collect();
        for need in [
            "SKILL_000_01",
            "SKILL_000_02",
            crate::collection::BASIC_ATTACK,
        ] {
            assert!(ids.contains(&need), "{need}");
        }
        // Every statue's attack has a row whose viewSkillId is the id itself (the action set).
        for d in crate::collection::statues() {
            let id = crate::collection::attack_skill(&d.statue_id);
            let row = sk["skillDataList"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["dtSkillId"] == id.as_str())
                .unwrap_or_else(|| panic!("no row for {id}"));
            assert!(id == crate::collection::BASIC_ATTACK || row["viewSkillId"] == id.as_str());
            assert_eq!(row["skillWait"], 100, "an attack ends the unit's turn");
            // attacks stay on the client's Overview two-shot (docs/research/31 §8)
            assert_eq!(row["forceNormalCamera"], 1, "{id} on the two-shot");
            assert!(
                row["dtRangeMax"].as_i64().unwrap() >= 100,
                "attackers stop in front, not inside"
            );
        }
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "skill ids are unique");
        // every other row (skills, SPs, Mind skills) keeps its script cameras
        let attacks: std::collections::HashSet<String> = crate::collection::statues()
            .iter()
            .map(|d| crate::collection::attack_skill(&d.statue_id))
            .chain([crate::collection::BASIC_ATTACK.to_string()])
            .collect();
        for r in sk["skillDataList"].as_array().unwrap() {
            let id = r["dtSkillId"].as_str().unwrap();
            if !attacks.contains(id) {
                assert_eq!(r["forceNormalCamera"], 0, "{id} plays its script cameras");
            }
        }
        let cats: Vec<&str> = sk["categoryList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["dtSkillCategoryId"].as_str().unwrap())
            .collect();
        for r in sk["skillDataList"].as_array().unwrap() {
            assert!(cats.contains(&r["mainSkillCategory"].as_str().unwrap()));
        }
        // Values that would zero damage (docs/research/11 §2.1).
        let p = &doc("dtbattle_damage_param")["param"][0];
        let f = |k: &str| p[k].as_f64().unwrap();
        assert!(f("_limitRateMin") <= 1.0 && 1.0 <= f("_limitRateMax"));
        assert!(f("_multiDealDamageMin") <= 1.0 && 1.0 <= f("_multiDealDamageMax"));
        assert!(f("_addDamageMin") <= 0.0 && 0.0 <= f("_addDamageMax"));
        assert!(f("_damageCap") > 0.0);
        // Reversal (docs/research/11 §5): the speed rate multiplies speed (in 1/10000), so below
        // 10000 the team slows down and at 0 it never acts and Reversal never ends; the action
        // count is a divisor; the start gauge is a percent with no clamp; the gauge must be able
        // to fill.
        if f("_reversalEnable") == 1.0 {
            assert!(f("_reversalAddSpeedUpRate") >= 10000.0);
            assert!(
                f("_reversalMaxTurn") >= 2.0,
                "at least one overkill hit after the kill"
            );
            assert!((0.0..=100.0).contains(&f("_reversalStartGauge")));
            assert!(f("_reversalAddGaugeDamage") > 0.0 && f("_reversalAddGaugeDead") > 0.0);
            let skills = crate::collection::dt_skill_table();
            let attacks: Vec<&Value> = skills["data"]["skillDataList"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["dtSkillId"].as_str().unwrap().ends_with("_99"))
                .collect();
            assert!(!attacks.is_empty());
            assert!(attacks
                .iter()
                .all(|r| r["dododoRate"].as_i64().unwrap_or(0) > 0));
        }
        // Move ranges are centimetres: an approach of a few metres must not count as "far".
        assert!(f("_moveRangeFar") >= 300.0 && f("_moveEscapeRenge") >= 50.0);
        // The random spread is in 1/10000 (docs/research/11 §3): it must bracket 10000.
        assert!(f("_damageGitterMin") <= 10000.0 && 10000.0 <= f("_damageGitterMax"));
        assert!(f("_healFluctuationsMin") <= 10000.0 && 10000.0 <= f("_healFluctuationsMax"));
        let pat = doc("area_position_pattern_table");
        let row = pat["paramList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["patternName"] == crate::quests::DT_FORMATION)
            .expect("formation row");
        let sets = row["posSetList"].as_array().unwrap();
        assert_eq!(sets.len(), 6);
        for (i, s) in sets.iter().enumerate() {
            let list = s["positionList"].as_array().unwrap();
            assert_eq!(list.len(), i % 3 + 1);
            for q in list.iter().flat_map(|p| [&p["rotate"], &p["startRotate"]]) {
                let n: f64 = ["x", "y", "z", "w"]
                    .iter()
                    .map(|k| q[*k].as_f64().unwrap().powi(2))
                    .sum();
                assert!((n - 1.0).abs() < 1e-6, "unit quaternion");
            }
        }
        // Statue rows: full-size team statues, the My Page camera object, positions in cm.
        let rows = crate::collection::statue_setting_rows();
        assert!(rows.iter().all(|r| {
            let scale = r["teamScaleScalar"].as_f64().unwrap();
            (50.0..=300.0).contains(&scale)
                && r["myPageCameraType"] == "CameraIdSet0"
                && r["mindFrameID"] == 1
                && r["myPagePos"]["x"] == 35.0
                && r["myPagePos"]["z"] == 180.0
                && (-180.0..=-50.0).contains(&r["myPagePos"]["y"].as_f64().unwrap())
        }));
        // Most models keep their size in the team view; offsets centre a head over its slot and only
        // ever lift it (to the same height in every slot, as in the original's footage).
        let full = rows
            .iter()
            .filter(|r| r["teamScaleScalar"] == 100.0)
            .count();
        assert!(full * 2 > rows.len());
        assert!(rows.iter().all(|r| {
            let p = |k: &str| r["teamPos"][k].as_f64().unwrap();
            p("x").abs() <= 100.0 && (0.0..=120.0).contains(&p("y")) && p("z") == 0.0
        }));
        // Reward rows: dataId is a string the client parses as an int (Build 12 device log).
        for c in crate::quests::chapters().iter().take(40) {
            let s = crate::quests::stage_info(c, &c.stages[0], 0, &Default::default());
            for r in s["steadyItemSet"].as_array().unwrap() {
                assert!(r["dataId"].as_str().unwrap().parse::<i32>().is_ok());
            }
        }
        // Tactical areas name the formation and send no boss; action areas name none.
        for c in crate::quests::chapters().iter().take(40) {
            let a = &crate::quests::area_list(c, &c.stages[0])[0];
            let want = if c.stage_type == 1 {
                crate::quests::DT_FORMATION
            } else {
                ""
            };
            assert_eq!(a["formation"], want);
            for e in a["areaSettingList"].as_array().unwrap() {
                // only the last area of a chapter's last stage has a boss (with its intro)
                if c.stages.len() > 1 {
                    assert_eq!(e["bossFlag"], 0);
                }
                let sid = e["enemyStatueInfo"]["statueId"].as_str().unwrap();
                assert_eq!(
                    e["enemyStatueInfo"]["dtSkillList"][0]["skillId"],
                    crate::collection::attack_skill(sid).as_str()
                );
                assert_eq!(e["AiId"], crate::quests::DT_ENEMY_AI);
            }
        }
    }

    /// Every camera id the client hard-codes has a set that ends on its own (flagMaintain false),
    /// and every node a set names exists (docs/research/12 §1).
    #[test]
    fn script_cameras_cover_the_hard_coded_ids() {
        let doc = |name: &str| -> Value {
            let js = TABLES.iter().find(|(n, _)| *n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        let sets = doc("Set_el_camera_set_common");
        let nodes = doc("Node_el_camera_node_common");
        let node_ids: Vec<&str> = nodes["listData"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        // The win shot follows the winner's own chest and neck (CharacterBoneList.Bone SPINE2 4,
        // NECK 5), so every statue is framed whatever its size or pose (docs/research/12 §5).
        let result = nodes["listData"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "jdr_result")
            .unwrap();
        assert_eq!(
            (
                result["baseBoneID"].as_i64(),
                result["lookatBoneID"].as_i64()
            ),
            (Some(4), Some(5))
        );
        let list = sets["listData"].as_array().unwrap();
        let ids: Vec<i64> = list
            .iter()
            .map(|s| s["cameraID"].as_i64().unwrap())
            .collect();
        for id in [41, 42, 201, 301, 350, 351, 100300, 100301, 100310, 100311]
            .into_iter()
            .chain(101005..=101028)
        {
            assert!(ids.contains(&id), "camera {id}");
        }
        let node = |id: &str| {
            nodes["listData"]
                .as_array()
                .unwrap()
                .iter()
                .find(|n| n["id"] == id)
                .unwrap()
                .clone()
        };
        for s in list {
            assert_eq!(s["flagMaintain"], false);
            // The client adds every later step's node to the first one (CreateNodeParam 0xD70D44,
            // docs/research/31 §2): the summed distance, field of view and pitch of each step stay
            // those of a close shot (Build 33 first sent absolute nodes: doubled values).
            let (mut length, mut fov, mut pitch) = (0.0, 0.0, 0.0);
            let id = s["cameraID"].as_i64().unwrap();
            for d in s["dataList"].as_array().unwrap() {
                let name = d["cameraNode"].as_str().unwrap();
                assert!(node_ids.contains(&name));
                assert!(d["sec"].as_f64().unwrap() > 0.0);
                // `inter` is the per-frame share of the way to the pose (SetCameraInfo 0xD6FE60):
                // 0 leaves the camera where it was (Builds 33-36), so every step moves it.
                assert!(d["inter"].as_f64().unwrap() > 0.0, "camera {id} inter 0");
                let n = node(name);
                length += n["length"].as_f64().unwrap();
                fov += n["perspective"].as_f64().unwrap();
                pitch += n["rotate"]["x"].as_f64().unwrap();
                assert!(
                    (1.0..=10.0).contains(&length),
                    "camera {id} length {length}"
                );
                assert!((20.0..=45.0).contains(&fov), "camera {id} fov {fov}");
                // (low shots look up from about 30 degrees below; the measured near-top-down impact
                // view of big moves looks 65 degrees down, docs/research/31 §3)
                assert!((-70.0..=32.0).contains(&pitch), "camera {id} pitch {pitch}");
            }
        }
    }

    /// Some loaders throw on a duplicate key at startup (CharacteristicTable.Init @0xF1D188,
    /// LoadSkillUseConditionParameter @0xF1BECC; docs/research/12 §1), and duplicate model hashes,
    /// bullets or camera ids would shadow each other: every keyed table has unique keys.
    #[test]
    fn keyed_tables_have_unique_keys() {
        let doc = |name: &str| -> Value {
            let js = TABLES.iter().find(|(n, _)| *n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        for (name, list, key) in [
            ("characteristic_data", "paramList", &["id"][..]),
            ("yogo_table", "paramList", &["yogo_Id"][..]),
            ("model_table", "paramList", &["modelIdHash"][..]),
            (
                "bullet_parameter",
                "param",
                &["ownerModelIdHash", "bulletId"][..],
            ),
            ("skill_data_table", "paramList", &["skillId"][..]),
            ("Set_el_camera_set_common", "listData", &["cameraID"][..]),
            ("Node_el_camera_node_common", "listData", &["id"][..]),
        ] {
            let rows = doc(name)[list].as_array().unwrap().clone();
            let keys: std::collections::HashSet<String> = rows
                .iter()
                .map(|r| {
                    key.iter()
                        .map(|k| r[*k].to_string())
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .collect();
            assert_eq!(keys.len(), rows.len(), "{name}: duplicate keys");
        }
    }

    /// Leader abilities (docs/research/12 §4): every affect a row uses exists in affect_table
    /// (the client drops unknown ones silently), giveLimit is never 0 (an affect with no uses is
    /// deleted at turn end), and the statues that send an ability have its row.
    #[test]
    fn leader_abilities_are_consistent() {
        let doc = |name: &str| -> Value {
            let js = TABLES.iter().find(|(n, _)| *n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        let affects: Vec<String> = doc("affect_table")["paramList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["affectId"].as_str().unwrap().to_string())
            .collect();
        let rows = doc("dtbattle_ability_data_table")["paramList"]
            .as_array()
            .unwrap()
            .clone();
        assert!(!rows.is_empty());
        for r in &rows {
            for p in r["abilityParamList"].as_array().unwrap() {
                assert!(affects.contains(&p["affectId"].as_str().unwrap().to_string()));
                assert_ne!(p["giveLimit"], 0);
            }
        }
        let with = crate::collection::statues()
            .iter()
            .filter(|d| !crate::collection::leader_ability(&d.statue_id)["abilityId"].is_null())
            .count();
        let lead = rows
            .iter()
            .filter(|r| r["abilityId"].as_str().unwrap().starts_with("DT_LEAD_"))
            .count();
        assert_eq!(
            with, lead,
            "every leader row belongs to a statue in the roster"
        );
        // Every Mind's ability names a row (Build 22).
        let ids: std::collections::HashSet<&str> = rows
            .iter()
            .map(|r| r["abilityId"].as_str().unwrap())
            .collect();
        for m in crate::collection::minds() {
            let a = crate::collection::mind_ability(&m.mind_id, 1);
            assert!(
                ids.contains(a["abilityId"].as_str().unwrap()),
                "{}",
                m.mind_id
            );
            assert_eq!(a["abilityParamList"], serde_json::json!([]), "never null");
        }
        let assist = rows
            .iter()
            .filter(|r| r["abilityId"].as_str().unwrap().starts_with("DT_AST_"))
            .count();
        // 4 kinds x 4 tiers, and one own row per assist-only statue (its ABI_AST_<id>_N label).
        let own = crate::collection::statues()
            .iter()
            .filter(|d| d.assist_only)
            .count();
        assert_eq!(assist, 16 + own, "4 kinds x 4 tiers + assist-only rows");
        for d in crate::collection::statues()
            .iter()
            .filter(|d| d.assist_only)
        {
            let id = format!("DT_AST_{}", d.statue_id);
            let row = rows.iter().find(|r| r["abilityId"] == id.as_str()).unwrap();
            assert_eq!(row["abilityName"], format!("ABI_AST_{}_N", d.statue_id));
        }
        let links = rows
            .iter()
            .filter(|r| r["abilityId"].as_str().unwrap().starts_with("DT_MLINK_"))
            .count();
        assert_eq!(links, 9, "3 Mind links x 2..4 Minds (docs/research/26 §2)");
        for l in crate::collection::mind_link_defs() {
            for a in l.abilities.values() {
                assert!(ids.contains(a.as_str()), "{a}");
            }
        }
        assert_eq!(
            rows.len(),
            lead + 12 + assist + links,
            "only leader, Mind, assist and Mind link rows"
        );
    }

    /// Assist abilities (docs/research/19-assists.md §5): every id names a row, the condition
    /// table has one row per statue (keyed by statueId, unique) whose ids are in the statue's
    /// dtAssistAbilityList, which matches assistAbilityList; the assist skill names a tactical row.
    #[test]
    fn assist_abilities_are_consistent() {
        let doc = |name: &str| -> Value {
            let js = TABLES.iter().find(|(n, _)| *n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        let rows = doc("dtbattle_ability_data_table")["paramList"]
            .as_array()
            .unwrap()
            .clone();
        let ids: std::collections::HashSet<&str> = rows
            .iter()
            .map(|r| r["abilityId"].as_str().unwrap())
            .collect();
        let generated = generated_tables();
        let gen = |name: &str| -> Value {
            let js = &generated.iter().find(|(n, _)| n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        let cond = gen("assist_ability_condition_table")["paramList"]
            .as_array()
            .unwrap()
            .clone();
        let skill_ids: std::collections::HashSet<String> = gen("dtbattle_skill_data_table")
            ["skillDataList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["dtSkillId"].as_str().unwrap().to_string())
            .collect();
        let mut seen = std::collections::HashSet::new();
        for r in &cond {
            let sid = r["statueId"].as_str().unwrap();
            assert!(seen.insert(sid), "{sid}: one row per statue");
            let o = crate::collection::OwnedStatue {
                statue_id: sid.into(),
                level: 1,
                ..Default::default()
            };
            let v = crate::collection::statue_info(&o);
            let dt: Vec<&Value> = v["dtAssistAbilityList"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| &a["abilityId"])
                .collect();
            let cl: Vec<&Value> = v["assistAbilityList"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| &a["abilityId"])
                .collect();
            assert_eq!(dt, cl, "{sid}: both lists equal (the client's index trap)");
            for c in r["conditionTableData"].as_array().unwrap() {
                let id = c["assistAbilityId"].as_str().unwrap();
                assert!(ids.contains(id), "{sid}: {id} names a row");
                assert!(dt.contains(&&c["assistAbilityId"]), "{sid}: {id} is listed");
                assert!(c["assistAbilityCondition"].is_array(), "never null");
            }
            let s = &v["dtAssistSkillInfo"];
            if let Some(k) = s["skillId"].as_str() {
                assert!(skill_ids.contains(k), "{sid}: {k} is a tactical row");
            }
        }
        assert_eq!(seen.len(), crate::collection::statues().len());
        // The generated table goes through the schema.
        let t = tables(&Schema::embedded().unwrap(), &[]).unwrap();
        assert!(t.iter().any(|t| t.name == "assist_ability_condition_table"));
        let all: Vec<(&str, &str)> = generated
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let t = tables(&Schema::embedded().unwrap(), &all).unwrap();
        let row = t
            .iter()
            .find(|t| t.name == "assist_ability_condition_table")
            .unwrap();
        assert_eq!(row.rows, cond.len());
    }

    /// Effect lines (docs/research/16-minds.md §8): every affect row a Mind or leader ability uses
    /// names a game label with English that shows the amount, and its (prvs, baseAffect) has a
    /// text rule whose amount slot is not Invalid (an Invalid slot shows -1, "0.1"). Every rule
    /// row has the 8 slots the client copies (fewer throws). The real-data test `real_l10n`
    /// checks the labels against the player's text.jp.
    #[test]
    fn effect_lines_have_labels_and_rules() {
        let doc = |name: &str| -> Value {
            let js = TABLES.iter().find(|(n, _)| *n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        let rules = doc("affect_text_rule_table");
        let slots = |list: &str, key: &str| -> Vec<(String, Vec<String>)> {
            rules[list]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    let k = r[key].as_str().unwrap().to_string();
                    let s: Vec<String> = r["paramRuleTypeList"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect();
                    assert_eq!(s.len(), 8, "{k}: 8 slots");
                    (k, s)
                })
                .collect()
        };
        let by_prvs = slots("pvrsRuleList", "pvrs");
        let by_base = slots("baseAffectRuleList", "baseAffectType");
        // The client's lookup: the condition's row, then the base row's non-Invalid slots.
        let rule = |prvs: &str, base: &str| -> Vec<String> {
            let mut r = by_prvs
                .iter()
                .find(|(k, _)| k == prvs)
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| vec!["Invalid".to_string(); 8]);
            if let Some((_, s)) = by_base.iter().find(|(k, _)| k == base) {
                for (i, x) in s.iter().enumerate() {
                    if x != "Invalid" {
                        r[i] = x.clone();
                    }
                }
            }
            r
        };
        let tr = crate::l10n::Translations::embedded().unwrap();
        // These rows hold no Japanese (`<STAT>+{decimal_Param0}％`), so they need no translation
        // and are not in our files; real_l10n checks them in the player's text.jp.
        let untranslated = [
            "AFFECT_HP_RATEADD_TEXT",
            "AFFECT_ATK_RATEADD_TEXT",
            "AFFECT_DEF_RATEADD_TEXT",
        ];
        let english = |label: &str| -> Option<String> {
            if untranslated.contains(&label) {
                return Some("{decimal_Param0}".into());
            }
            tr.english(&format!("text:affectText_JP:{label}"))
                .map(str::to_owned)
        };
        let used: std::collections::HashSet<String> = doc("dtbattle_ability_data_table")
            ["paramList"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|r| r["abilityParamList"].as_array().unwrap().clone())
            .map(|p| p["affectId"].as_str().unwrap().to_string())
            .collect();
        let affects = doc("affect_table")["paramList"].as_array().unwrap().clone();
        let mut lines = 0;
        for a in &affects {
            let id = a["affectId"].as_str().unwrap();
            if !used.contains(id) {
                continue;
            }
            let text = a["affectTextId"].as_str().unwrap();
            let en = english(text).unwrap_or_else(|| panic!("{id}: no English for {text:?}"));
            assert!(
                en.contains("{decimal_Param0}") || en.contains("{mag_Param0}"),
                "{id}: the line shows the amount"
            );
            let r = rule(
                a["prvs"].as_str().unwrap(),
                a["baseAffect"].as_str().unwrap(),
            );
            assert_ne!(r[0], "Invalid", "{id}: the amount slot has a rule");
            if id.contains("_STYPE_") {
                assert_eq!(r[1], "Solo", "{id}: one line per StatueType");
            }
            lines += 1;
        }
        assert_eq!(lines, used.len(), "every used affect is in affect_table");
        // Mind abilities: one plain stat line each.
        for stat in ["HP", "ATK", "DEF"] {
            let row = affects
                .iter()
                .find(|a| a["affectId"] == format!("DTAF_{stat}_RATEADD"))
                .unwrap();
            assert_eq!(row["affectTextId"], format!("AFFECT_{stat}_RATEADD_TEXT"));
        }
    }

    /// Action-battle affects (docs/research/30-action-battle.md §7): every affectId the action
    /// leader abilities, the assist "action" params, the JoJo Links and the tactical-to-action map
    /// (ACTION_AFFECT) send has an affect_table row (the client drops unknown ones silently) whose base the action
    /// battle reads (UpdateActiveAffectParameter cases, BASE_SPSTC_ADD 27, 88), HP-type bases only
    /// with a condition known at battle start, a label with English that shows the amount, and a
    /// text rule that shows slot 0, slot 1 per target type and slot 2 for the conditions that use it.
    #[test]
    fn action_affects_have_rows_labels_and_rules() {
        let doc = |name: &str| -> Value {
            let js = TABLES.iter().find(|(n, _)| *n == name).unwrap().1;
            serde_json::from_str::<Value>(js).unwrap()["data"].clone()
        };
        let schema: Value = serde_json::from_str(SCHEMA_JSON).unwrap();
        let base_value = |b: &str| -> i64 {
            schema["types"]["Jjs.BaseAffectType"]["values"][b]
                .as_i64()
                .unwrap_or_else(|| panic!("{b}: a BaseAffectType"))
        };
        let consumer = |n: i64| {
            (1..=20).contains(&n)
                || (23..=27).contains(&n)
                || (45..=57).contains(&n)
                || n == 60
                || n == 61
                || (74..=79).contains(&n)
                || n == 81
                || (88..=91).contains(&n)
        };
        let mut used: Vec<String> = Vec::new();
        for l in crate::collection::action_leaders().values() {
            used.extend(l.params.iter().map(|p| p.affect_id.clone()));
        }
        for d in crate::collection::assist_defs() {
            for a in &d.abilities {
                if let Some(x) = &a.action {
                    used.extend(x.params.iter().map(|p| p.affect_id.clone()));
                }
            }
        }
        for l in crate::collection::jojo_links() {
            used.extend(l.params.iter().map(|p| p.affect_id.clone()));
        }
        used.extend(
            crate::collection::ACTION_AFFECT
                .iter()
                .map(|(_, af)| af.to_string()),
        );
        used.sort();
        used.dedup();
        assert!(used.len() >= 100, "{}", used.len());
        let rules = doc("affect_text_rule_table");
        let slots = |list: &str, key: &str| -> Vec<(String, Vec<String>)> {
            rules[list]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    let s: Vec<String> = r["paramRuleTypeList"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect();
                    assert_eq!(s.len(), 8, "8 slots");
                    (r[key].as_str().unwrap().to_string(), s)
                })
                .collect()
        };
        let by_prvs = slots("pvrsRuleList", "pvrs");
        let by_base = slots("baseAffectRuleList", "baseAffectType");
        let rule = |prvs: &str, base: &str| -> Vec<String> {
            let mut r = by_prvs
                .iter()
                .find(|(k, _)| k == prvs)
                .map(|(_, s)| s.clone())
                .unwrap_or_else(|| vec!["Invalid".to_string(); 8]);
            if let Some((_, s)) = by_base.iter().find(|(k, _)| k == base) {
                for (i, x) in s.iter().enumerate() {
                    if x != "Invalid" {
                        r[i] = x.clone();
                    }
                }
            }
            r
        };
        let tr = crate::l10n::Translations::embedded().unwrap();
        let untranslated = [
            "AFFECT_HP_RATEADD_TEXT",
            "AFFECT_ATK_RATEADD_TEXT",
            "AFFECT_DEF_RATEADD_TEXT",
        ];
        let affects = doc("affect_table")["paramList"].as_array().unwrap().clone();
        for id in &used {
            let a = affects
                .iter()
                .find(|a| a["affectId"] == id.as_str())
                .unwrap_or_else(|| panic!("{id}: an affect_table row"));
            let (prvs, base) = (
                a["prvs"].as_str().unwrap(),
                a["baseAffect"].as_str().unwrap(),
            );
            assert!(consumer(base_value(base)), "{id}: {base} has a consumer");
            if [
                "BASE_HP_CON",
                "BASE_HP_RATECON",
                "BASE_ATK_BOOST_HP",
                "BASE_DEF_BOOST_HP",
            ]
            .contains(&base)
            {
                assert!(
                    prvs == "PRVS_NONE" || prvs == "PRVS_ATRBT",
                    "{id}: max HP is fixed at battle start"
                );
            }
            let text = a["affectTextId"].as_str().unwrap();
            if !untranslated.contains(&text) {
                let en = tr
                    .english(&format!("text:affectText_JP:{text}"))
                    .unwrap_or_else(|| panic!("{id}: no English for {text}"));
                assert!(
                    [
                        "{decimal_Param0}",
                        "{mag_Param0}",
                        "{Param0}",
                        "{permil_Param0}"
                    ]
                    .iter()
                    .any(|p| en.contains(p)),
                    "{id}: the line shows the amount"
                );
            }
            let r = rule(prvs, base);
            assert_ne!(r[0], "Invalid", "{id}: slot 0");
            if base.contains("_ATRBT_") || base.contains("_STYPE_") {
                assert_eq!(r[1], "Solo", "{id}: one line per target type");
            }
            if [
                "PRVS_DEFEAT",
                "PRVS_ASSIST_USE",
                "PRVS_ATRBT",
                "PRVS_OVER_HP",
                "PRVS_UNDER_HP",
            ]
            .contains(&prvs)
            {
                assert_ne!(r[2], "Invalid", "{id}: slot 2");
            }
        }
    }

    /// statue_special_scene_data_table stays empty. The action battle's SP cinematic does not
    /// read it: DoChangeBattleSceneMode mode 2 in the statue's own action set starts it, and
    /// SpecialSkillSceneControl.GetCameraAnimatorController 0xCB3040 builds the camera's bundle
    /// path from the model id (docs/research/30 §5). Its rows (special retire cameras and a second
    /// actor, `spcialSkillSceneData`) have no retail values, and CreateActorObject 0xCB3624 calls
    /// Substring on the scene params, so a guessed or short row could throw.
    #[test]
    fn statue_special_scene_table_stays_empty() {
        let name = "statue_special_scene_data_table";
        assert!(TABLES.iter().all(|(n, _)| *n != name));
        assert!(generated_tables().iter().all(|(n, _)| n != name));
        let mut all: Vec<(&str, &str)> = TABLES.to_vec();
        let generated = generated_tables();
        all.extend(generated.iter().map(|(a, b)| (a.as_str(), b.as_str())));
        let t = tables(&Schema::embedded().unwrap(), &all).unwrap();
        assert!(t.iter().filter(|t| t.name == name).all(|t| t.rows == 0));
    }

    #[test]
    fn tutorial_table_content() {
        let s = Schema::embedded().unwrap();
        let t = tables(&s, TABLES).unwrap();
        // 17 core tables plus the battle tables from content files (the skill table is
        // generated from the statues, see generated_tables).
        assert_eq!(t.len(), 19 + 11 + 10 + 1); // yogo_table is core
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
        // SecurityStringValue: seed 0, ASCII bytes as uppercase hex joined by '-'.
        assert_eq!(
            s.val(SEC_STRING, false, &json!("SKILL_000_01"), "x")
                .unwrap(),
            Val::Struct(vec![
                Val::Int(0),
                Val::Str("53-4B-49-4C-4C-5F-30-30-30-5F-30-31".into())
            ])
        );
        assert_eq!(
            s.val(SEC_STRING, false, &json!(""), "x").unwrap(),
            Val::Struct(vec![Val::Int(0), Val::Str(String::new())])
        );
        assert!(s.val(SEC_STRING, false, &json!("é"), "x").is_err());
        // SecurityFloatValue: seed 0, the float's bits.
        assert_eq!(
            s.val(SEC_FLOAT, false, &json!(1.5), "x").unwrap(),
            Val::Struct(vec![Val::Int(0), Val::Int(1.5f32.to_bits() as i64)])
        );
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
        let unknown =
            r#"{"asset":"not_a_table","sources":[{"source":"jdr-default","note":"n"}],"data":{}}"#;
        assert!(tables(&s, &[("not_a_table", unknown)])
            .unwrap_err()
            .contains("no such asset"));
        for (src, ok) in [
            (r#"{"source":"jdr-research"}"#, false),
            (
                r#"{"source":"jdr-research","doc":"docs/research/09-master-data.md"}"#,
                true,
            ),
            (r#"{"source":"jdr-default"}"#, false),
            (
                r#"{"source":"some-wiki","page":"https://example.org/x"}"#,
                false,
            ),
            (
                r#"{"source":"some-wiki","page":"https://example.org/x","accessed":"2026-11-14"}"#,
                true,
            ),
        ] {
            let v: Value = serde_json::from_str(&format!(r#"{{"sources":[{src}]}}"#)).unwrap();
            assert_eq!(check_sources(&v).is_ok(), ok, "{src}");
        }
    }

    /// The semantics from research 09 §6 that make "every tutorial done" work.
    #[test]
    fn tutorial_table_invariants() {
        let v: Value = serde_json::from_str(TABLES[0].1).unwrap();
        let rows = v["data"]["paramList"].as_array().unwrap();
        assert!(
            rows.len() <= crate::TUTORIAL_FLAGS,
            "get_tutorial_step must cover every row"
        );
        let labels: Vec<&str> = rows
            .iter()
            .map(|r| r["flagLabel"].as_str().unwrap())
            .collect();
        let unique: std::collections::BTreeSet<_> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len(), "labels unique");
        for i in 0..12 {
            assert!(labels.contains(&format!("TUTORIAL_{i:02}").as_str()));
        }
        for i in (0..18).filter(|i| *i != 9) {
            assert!(labels.contains(&format!("HINT_{i:02}").as_str()));
        }
        for i in 0..5 {
            assert!(labels.contains(&format!("OPEN_{i:02}").as_str()));
        }
        for r in rows {
            let l = r["flagLabel"].as_str().unwrap();
            if !l.starts_with("TUTORIAL_") {
                assert_eq!(r["launchType"], "Manual", "{l}: hints/unlocks never launch");
            }
            assert!(r.get("scriptPackName").is_none_or(|p| p == ""), "{l}");
        }
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
