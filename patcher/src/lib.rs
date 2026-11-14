//! APK in -> patched APK out. See docs/ARCHITECTURE.md and CONTRIBUTING.md (patch rules).

pub mod apk;
pub mod axml;
pub mod elf;
pub mod known;
pub mod meta;
pub mod patches;
pub mod report;
pub mod sign;

pub use apk::Error;
