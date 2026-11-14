//! Unity 2018.4 asset bundle support, just enough to build the bundles the dead server used to
//! send (`versionfile`, later `in_memory_master_data`).
//!
//! - [`bundle`]: UnityFS container read (LZ4/LZ4HC) and write (uncompressed).
//! - [`serialized`]: SerializedFile v17: read type entries, write a file from parts.
//! - [`typetree`]: build type trees for our own MonoBehaviour classes.
//! - [`writer`]: little-endian writer with Unity's 4-byte alignment rules.
//!
//! Type definitions for engine classes (AssetBundle, MonoScript) are taken at runtime from the
//! player's own downloaded bundles, never stored here.

pub mod bundle;
pub mod serialized;
pub mod typetree;
pub mod writer;

pub type Error = String;

pub const UNITY_VERSION: &str = "2018.4.1f1";
pub const SERIALIZED_VERSION: u32 = 17;
pub const PLATFORM_ANDROID: i32 = 13;

pub const CLASS_MONOBEHAVIOUR: i32 = 114;
pub const CLASS_MONOSCRIPT: i32 = 115;
pub const CLASS_ASSETBUNDLE: i32 = 142;
