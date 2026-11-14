//! Crypto formats used by the game client.
//!
//! Nothing in this crate contains a key. Every key is either recovered from the
//! player's own files ([`keyrec`]) or read from their own APK by the tools.

pub mod aescbc;
pub mod container;
pub mod keyrec;
pub mod names;

pub use container::{Jjec, Vcs};
