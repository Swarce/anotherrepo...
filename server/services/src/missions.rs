//! Missions (`trophy/*`, docs/research/17-missions.md). The client never reports progress, so the
//! server counts: quest clears, EXP and gold earned, and a handful of first-time actions. Daily
//! missions reset at 04:00 JST and only today's are listed (the client does not filter by day).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MISSIONS_JSON: &str = include_str!("../../content/missions.json");
/// `acEnd` meaning "no end" (the screen's own constant).
pub const NO_END: &str = "2036-01-01 00:00:00";

#[derive(Debug, Clone, Deserialize)]
pub struct MissionDef {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub counter: String,
    pub param: i64,
    pub reward: Reward,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Reward {
    #[serde(rename = "itemId")]
    pub item_id: i64,
    pub count: i64,
}

pub fn missions() -> &'static [MissionDef] {
    static CELL: std::sync::OnceLock<Vec<MissionDef>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            missions: Vec<MissionDef>,
        }
        serde_json::from_str::<Doc>(MISSIONS_JSON)
            .map(|d| d.missions)
            .expect("missions.json")
    })
}

/// Per-account mission progress (saved with the story progress).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MissionState {
    /// The game day the daily counters belong to (days since the epoch, turning at 04:00 JST).
    #[serde(default)]
    pub day: i64,
    #[serde(default)]
    pub daily: BTreeMap<String, i64>,
    #[serde(default)]
    pub totals: BTreeMap<String, i64>,
    #[serde(default)]
    pub claimed_daily: BTreeSet<String>,
    #[serde(default)]
    pub claimed: BTreeSet<String>,
}

/// The game day: the day turns at 04:00 JST (research pack `economy.daily_reset`).
pub fn game_day(now: i64) -> i64 {
    (now + jdr_core::clock::TZ_OFFSET_SECS - 4 * 3600).div_euclid(86400)
}

fn date(t: i64) -> String {
    let (y, mo, d, h, mi, s) = jdr_core::clock::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

impl MissionState {
    /// Start a new day's daily counters when the day turned.
    pub fn roll(&mut self, now: i64) {
        let today = game_day(now);
        if self.day != today {
            self.day = today;
            self.daily.clear();
            self.claimed_daily.clear();
        }
    }

    /// Count `n` more of `counter` (daily and total).
    pub fn bump(&mut self, counter: &str, n: i64, now: i64) {
        if n <= 0 {
            return;
        }
        self.roll(now);
        *self.daily.entry(counter.into()).or_insert(0) += n;
        *self.totals.entry(counter.into()).or_insert(0) += n;
    }

    fn count(&self, m: &MissionDef, rank: i64) -> i64 {
        match m.counter.as_str() {
            "rank" => rank,
            c if m.kind == "daily" => self.daily.get(c).copied().unwrap_or(0),
            c => self.totals.get(c).copied().unwrap_or(0),
        }
    }

    fn claimed(&self, m: &MissionDef) -> bool {
        if m.kind == "daily" {
            self.claimed_daily.contains(&m.id)
        } else {
            self.claimed.contains(&m.id)
        }
    }

    /// TrophyInfo status: 0 not achieved, 1 claimable, 2 received.
    pub fn status(&self, m: &MissionDef, rank: i64) -> i64 {
        if self.claimed(m) {
            2
        } else if self.count(m, rank) >= m.param {
            1
        } else {
            0
        }
    }

    pub fn claimable(&self, rank: i64) -> usize {
        missions()
            .iter()
            .filter(|m| self.status(m, rank) == 1)
            .count()
    }

    /// Mark claimed; returns the reward when the mission was claimable.
    pub fn claim(&mut self, id: &str, rank: i64) -> Option<&'static MissionDef> {
        let m = missions().iter().find(|m| m.id == id)?;
        if self.status(m, rank) != 1 {
            return None;
        }
        if m.kind == "daily" {
            self.claimed_daily.insert(m.id.clone());
        } else {
            self.claimed.insert(m.id.clone());
        }
        Some(m)
    }

    /// The `trophyList`: today's dailies (day 1..7 = Mon..Sun) and the regular missions.
    pub fn trophy_list(&self, rank: i64, now: i64) -> Vec<Value> {
        let day = game_day(now);
        let weekday = (day + 3).rem_euclid(7) + 1; // 1970-01-01 was a Thursday; Mon = 1
        let (start, end) = (
            date(day * 86400 + 4 * 3600 - jdr_core::clock::TZ_OFFSET_SECS),
            date((day + 1) * 86400 + 4 * 3600 - jdr_core::clock::TZ_OFFSET_SECS),
        );
        missions()
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let daily = m.kind == "daily";
                json!({ "achievementId": m.id, "id": m.id, "count": self.count(m, rank).min(m.param),
                    "status": self.status(m, rank), "achievementType": if daily { 0 } else { 1 },
                    "param": m.param, "day": if daily { weekday } else { 0 },
                    "achievementGroupNo": if daily { 1 } else { 2 }, "achievementOrder": i + 1,
                    "achievementTitle": "", "achievementText": m.text,
                    // Every list must exist and acEnd must parse: the row drawing throws otherwise.
                    "itemSetInfo": reward_set(&m.reward),
                    "acStart": if daily { start.clone() } else { "2020-01-01 00:00:00".into() },
                    "acEnd": if daily { end.clone() } else { NO_END.into() } })
            })
            .collect()
    }
}

/// The reward as an ItemSetInfo (the row shows `itemList[0]`).
pub fn reward_set(r: &Reward) -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 1, "statueList": [], "mindList": [], "clanObjectList": [],
            "itemList": [crate::items::reward_item(r.item_id, r.count)] })
}
