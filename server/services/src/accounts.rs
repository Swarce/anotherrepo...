//! Local accounts: one per device, created by `/user/create_user`, keyed by the uuid the client
//! stores in PlayerPrefs. Stored as JSON under `<data>/accounts/<uuid>.json` with backups.

use jdr_core::{clock, storage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub user_id: String,
    pub uuid: String,
    pub username: String,
    pub rank: i64,
    pub exp: i64,
    pub created_at: i64,
    pub last_login_at: i64,
    /// Tutorial flags as last posted by the client (positional, 0/1).
    pub tutorial_step: Vec<u8>,
    #[serde(default)]
    pub stone_free: i64,
    #[serde(default)]
    pub gold: i64,
    #[serde(default = "default_comment")]
    pub comment: String,
}

fn default_comment() -> String {
    String::new()
}

pub struct Accounts {
    dir: PathBuf,
    cache: Mutex<HashMap<String, Profile>>,
}

fn valid_uuid(u: &str) -> bool {
    !u.is_empty() && u.len() <= 64 && u.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

impl Accounts {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            dir: data_dir.join("accounts"),
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn path(&self, uuid: &str) -> PathBuf {
        self.dir.join(format!("{uuid}.json"))
    }

    pub fn get(&self, uuid: &str) -> Option<Profile> {
        if !valid_uuid(uuid) {
            return None;
        }
        if let Some(p) = self.cache.lock().unwrap().get(uuid) {
            return Some(p.clone());
        }
        let b = std::fs::read(self.path(uuid)).ok()?;
        let p: Profile = serde_json::from_slice(&b).ok()?;
        self.cache
            .lock()
            .unwrap()
            .insert(uuid.to_string(), p.clone());
        Some(p)
    }

    pub fn save(&self, p: &Profile) -> Result<(), String> {
        let data = serde_json::to_vec_pretty(p).map_err(|e| e.to_string())?;
        storage::save_guarded(&self.path(&p.uuid), &data, |b| {
            serde_json::from_slice::<Profile>(b)
                .map(|_| ())
                .map_err(|e| e.to_string())
        })
        .map_err(|e| e.to_string())?;
        self.cache.lock().unwrap().insert(p.uuid.clone(), p.clone());
        Ok(())
    }

    /// Create a fresh account. Ids are derived from the clock and a counter (no randomness needed).
    pub fn create(&self, default_tutorial: &[u8]) -> Result<Profile, String> {
        let now = clock::now();
        let n = std::fs::read_dir(&self.dir).map(|r| r.count()).unwrap_or(0) as i64 + 1;
        let uuid = format!("jdr-{now:x}-{n:x}");
        let p = Profile {
            user_id: format!("{}", 100_000_000 + n),
            uuid,
            username: "Player".into(),
            rank: 1,
            exp: 0,
            created_at: now,
            last_login_at: now,
            tutorial_step: default_tutorial.to_vec(),
            stone_free: 0,
            gold: 0,
            comment: String::new(),
        };
        self.save(&p)?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_get_save() {
        let d = std::env::temp_dir().join(format!("jdr-acc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let a = Accounts::new(&d);
        let mut p = a.create(&[1, 1]).unwrap();
        assert_eq!(a.get(&p.uuid).unwrap(), p);
        p.username = "Jotaro".into();
        a.save(&p).unwrap();
        // fresh instance reads from disk
        assert_eq!(Accounts::new(&d).get(&p.uuid).unwrap().username, "Jotaro");
        assert!(a.get("../etc").is_none());
        assert!(a.get("").is_none());
        let q = a.create(&[]).unwrap();
        assert_ne!(q.uuid, p.uuid);
    }
}
