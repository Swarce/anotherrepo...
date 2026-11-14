//! Unity 2018.4 asset bundle support, just enough to build the bundles the dead server used to
//! send (`versionfile`, `in_memory_master_data`).
//!
//! - [`bundle`]: UnityFS container read (LZ4/LZ4HC) and write (uncompressed).
//! - [`sfile`]: a whole SerializedFile, parsed for decode/edit/write of any object through its
//!   type tree (localization).
//! - [`serialized`]: SerializedFile v17: read type entries, write a file from parts.
//! - [`typetree`]: build type trees for our own MonoBehaviour classes.
//! - [`managed`]: generic managed field types (type-tree nodes + binary values) for the game's own
//!   ScriptableObject classes.
//! - [`objects`]: AssetBundle and MonoScript object data.
//! - [`writer`]: little-endian writer with Unity's 4-byte alignment rules.
//!
//! Type definitions for engine classes (AssetBundle, MonoScript) are taken at runtime from the
//! player's own downloaded bundles, never stored here.

pub mod bundle;
pub mod common_strings;
pub mod managed;
pub mod objects;
pub mod serialized;
pub mod sfile;
pub mod texture;
pub mod typetree;
pub mod writer;

pub type Error = String;

pub const UNITY_VERSION: &str = "2018.4.1f1";
pub const SERIALIZED_VERSION: u32 = 17;
pub const PLATFORM_ANDROID: i32 = 13;

pub const CLASS_MONOBEHAVIOUR: i32 = 114;
pub const CLASS_MONOSCRIPT: i32 = 115;
pub const CLASS_ASSETBUNDLE: i32 = 142;
