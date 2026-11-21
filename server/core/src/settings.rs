//! Server settings: where things live, which ports to try, and the keys recovered from the
//! player's own copy (`keys.json`, produced by `jdr-tool keys`).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keys {
    pub asset_key: String,
    pub asset_iv: String,
    #[serde(default)]
    pub sound_key: Option<String>,
    /// Request envelope key (read from the player's APK by `jdr-tool keys --apk`).
    #[serde(default)]
    pub http_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Settings {
    /// Ports tried in order; the first free one is used and logged.
    pub ports: Vec<u16>,
    /// Server state: accounts, saves, logs.
    pub data_dir: PathBuf,
    /// Folder the CDN serves extra files from (never the game's own download folder on a device).
    pub files_dir: Option<PathBuf>,
    /// The game's own downloaded files (`files/`), read-only: source for the generated manifest.
    pub game_files_dir: Option<PathBuf>,
    pub keys: Option<Keys>,
}

pub const DEFAULT_PORT: u16 = 18080;
pub const FALLBACK_PORT: u16 = 18081;

impl Settings {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            ports: vec![DEFAULT_PORT, FALLBACK_PORT],
            data_dir,
            files_dir: None,
            game_files_dir: None,
            keys: None,
        }
    }

    pub fn log_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    pub fn load_keys(path: &Path) -> Result<Keys, String> {
        let b = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", path.display()))
    }
}
