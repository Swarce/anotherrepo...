//! Server core shared by the embedded and standalone hosts.
//!
//! Shared-code rules (docs/MODULES.md): std + the workspace dependencies only, no async
//! runtime, no global process state beyond the logger/clock, must build for
//! `aarch64-linux-android`.

pub mod clock;
pub mod envelope;
pub mod http;
pub mod log;
pub mod settings;
pub mod storage;

/// Build number, shown in the log and on the settings page.
pub const BUILD: u32 = 33;
