//! Local accounts: one per device, created by `/user/create_user`, keyed by the uuid the client
//! stores in PlayerPrefs. Stored as JSON under `<data>/accounts/<uuid>.json` with backups.
//!
//! Safety rules: a file that exists but cannot be read is an **error**, never "new user" (that
//! would hand the device a fresh account); fields written by a newer build are preserved
//! (`extra`) and a save with a newer schema is refused rather than rewritten.

use jdr_core::{clock, storage};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Save schema written by this build. Bump when the meaning of a field changes.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    #[serde(default)]
    pub schema: u32,
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
    #[serde(default)]
    pub comment: String,
    /// Owned statues and teams (added in Build 10; older saves get them on their next login).
    #[serde(default)]
    pub collection: crate::collection::Collection,
    /// Story progress (Build 10).
    #[serde(default)]
    pub progress: crate::quests::Progress,
    /// Fields this build does not know (written by a newer build); kept verbatim.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub struct Accounts {
    dir: PathBuf,
    cache: Mutex<HashMap<String, Profile>>,
    /// Serialises writes of account files (the client may resend a slow request).
    write_lock: Mutex<()>,
    /// Held across read, change and write in `update`, so concurrent requests cannot lose each
    /// other's changes.
    update_lock: Mutex<()>,
}

/// What a change to a profile was, which decides how it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    /// Nothing to write.
    None,
    /// Bookkeeping (login time, play counts): written without rotating backups.
    Bookkeeping,
    /// Real progress: written with a backup rotation.
    Progress,
}

fn valid_uuid(u: &str) -> bool {
    !u.is_empty() && u.len() <= 64 && u.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
}

fn parse(b: &[u8]) -> Result<Profile, String> {
    let p: Profile = serde_json::from_slice(b).map_err(|e| e.to_string())?;
    if p.schema > SCHEMA {
        return Err(format!(
            "save schema {} is newer than this build ({SCHEMA}); update the server",
            p.schema
        ));
    }
    Ok(p)
}

impl Accounts {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            dir: data_dir.join("accounts"),
            cache: Mutex::new(HashMap::new()),
            write_lock: Mutex::new(()),
            update_lock: Mutex::new(()),
        }
    }

    /// Read one account, let `f` change it, and write it as `f` says, all under one lock.
    /// `Ok(None)`: no such account.
    pub fn update<R>(
        &self,
        uuid: &str,
        f: impl FnOnce(&mut Profile) -> (R, Change),
    ) -> Result<Option<(Profile, R)>, String> {
        let _g = self.update_lock.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut p) = self.get(uuid)? else {
            return Ok(None);
        };
        let (r, change) = f(&mut p);
        match change {
            Change::None => {}
            Change::Bookkeeping => self.write(&p, false)?,
            Change::Progress => self.write(&p, true)?,
        }
        Ok(Some((p, r)))
    }

    fn path(&self, uuid: &str) -> PathBuf {
        self.dir.join(format!("{uuid}.json"))
    }

    /// `Ok(None)`: no such account. `Err`: the account exists but cannot be read.
    pub fn get(&self, uuid: &str) -> Result<Option<Profile>, String> {
        if !valid_uuid(uuid) {
            return Ok(None);
        }
        if let Some(p) = self.cache.lock().unwrap().get(uuid) {
            return Ok(Some(p.clone()));
        }
        let path = self.path(uuid);
        let b = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let p = parse(&b).map_err(|e| format!("{}: {e}", path.display()))?;
        self.cache
            .lock()
            .unwrap()
            .insert(uuid.to_string(), p.clone());
        Ok(Some(p))
    }

    fn write(&self, p: &Profile, rotate: bool) -> Result<(), String> {
        let _g = self.write_lock.lock().unwrap();
        let mut p = p.clone();
        p.schema = SCHEMA;
        let data = serde_json::to_vec_pretty(&p).map_err(|e| e.to_string())?;
        storage::save_guarded_opts(&self.path(&p.uuid), &data, |b| parse(b).map(|_| ()), rotate)
            .map_err(|e| e.to_string())?;
        self.cache.lock().unwrap().insert(p.uuid.clone(), p);
        Ok(())
    }

    /// Save with a backup rotation (real progress).
    pub fn save(&self, p: &Profile) -> Result<(), String> {
        self.write(p, true)
    }

    /// Save without rotating backups (bookkeeping such as the login timestamp).
    pub fn touch(&self, p: &Profile) -> Result<(), String> {
        self.write(p, false)
    }

    /// Create a fresh account. Ids are derived from the clock and a counter (no randomness needed).
    pub fn create(&self, default_tutorial: &[u8]) -> Result<Profile, String> {
        let now = clock::now();
        let n = std::fs::read_dir(&self.dir).map(|r| r.count()).unwrap_or(0) as i64 + 1;
        let p = Profile {
            schema: SCHEMA,
            user_id: format!("{}", 100_000_000 + n),
            uuid: format!("jdr-{now:x}-{n:x}"),
            username: "Player".into(),
            rank: 1,
            exp: 0,
            created_at: now,
            last_login_at: now,
            tutorial_step: default_tutorial.to_vec(),
            stone_free: 0,
            gold: 0,
            comment: String::new(),
            collection: Default::default(),
            progress: Default::default(),
            extra: Map::new(),
        };
        if self.path(&p.uuid).exists() {
            return Err(format!("account id collision for {}", p.uuid));
        }
        self.save(&p)?;
        Ok(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jdr-acc-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn create_get_save() {
        let d = tmp("basic");
        let a = Accounts::new(&d);
        let mut p = a.create(&[1, 1]).unwrap();
        assert_eq!(a.get(&p.uuid).unwrap().unwrap(), p);
        p.username = "Jotaro".into();
        a.save(&p).unwrap();
        assert_eq!(
            Accounts::new(&d).get(&p.uuid).unwrap().unwrap().username,
            "Jotaro"
        );
        assert_eq!(a.get("../etc").unwrap(), None);
        assert_eq!(a.get("").unwrap(), None);
        assert_eq!(a.get("nobody").unwrap(), None);
    }

    #[test]
    fn unreadable_save_is_an_error_not_a_new_user() {
        let d = tmp("bad");
        let a = Accounts::new(&d);
        let p = a.create(&[]).unwrap();
        std::fs::write(
            d.join("accounts").join(format!("{}.json", p.uuid)),
            b"{broken",
        )
        .unwrap();
        assert!(Accounts::new(&d).get(&p.uuid).is_err());
    }

    #[test]
    fn unknown_fields_survive_and_newer_schema_is_refused() {
        let d = tmp("extra");
        let a = Accounts::new(&d);
        let p = a.create(&[]).unwrap();
        let f = d.join("accounts").join(format!("{}.json", p.uuid));
        let mut v: Value = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
        v["statues"] = serde_json::json!([{"id": "s1"}]);
        std::fs::write(&f, serde_json::to_vec(&v).unwrap()).unwrap();
        let a2 = Accounts::new(&d);
        let mut q = a2.get(&p.uuid).unwrap().unwrap();
        q.username = "X".into();
        a2.save(&q).unwrap();
        let back: Value = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
        assert_eq!(back["statues"][0]["id"], "s1");

        v["schema"] = serde_json::json!(SCHEMA + 1);
        std::fs::write(&f, serde_json::to_vec(&v).unwrap()).unwrap();
        let a3 = Accounts::new(&d);
        assert!(a3.get(&p.uuid).is_err());
        assert!(a3.save(&q).is_err(), "must not overwrite a newer save");
    }
}
