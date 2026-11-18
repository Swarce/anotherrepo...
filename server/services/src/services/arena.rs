//! The Arena (`/pvp/*`, docs/research/23-arena.md): the 2019 rule set (PvP2HomeController,
//! scene 1600), the only one client 3.12.0 can reach, played against AI-controlled teams the
//! server builds from our roster. The original's opponents were other players' teams run by the
//! AI (research pack `v2.arena.ai_opponent`), which an offline server can do on its own.
//!
//! Client facts (H, read in the 3.12.0 code) are marked "client"; facts from the research pack's
//! dated guides are marked with their note id; everything else is ours. The campaign is always
//! open; seasons, point rewards and opponent strength are ours.
//!
//! - My Page shows the Arena button for the first `/info/event_list` entry of eventType 6/7 and
//!   reads `userBattleInfo.fieverEndTime` as a date (client).
//! - Home: `get_pvp_config`, `/event/point_reward_list`, `/item/heal_item_list {type 2}` and the
//!   polled `/pvp/home` (`pvpUserInfo` must never be null, client).
//! - A battle: `enemy_player_list` (three opponents, `reChoiceFlg` 1 shuffles), `pvp_start`
//!   (battleModeInfo battleType 2 = the AI arena, client), the tactical battle on the client's
//!   own map (`map02_under_colos01_PVP`, formation `PatternPVP`, client), then `pvp_end`.
//! - Each team has a durability bar 0..10000 (`PVP_TEAM_HP_MAX`, client); `recover_hp_pvp`
//!   refills it.
//! - The formation routes (`*_pvp`) edit the Arena's own five teams like `/team/*` edits the
//!   story teams (services/team.rs).

use crate::accounts::{Change, Profile};
use crate::app::{ApiError, ApiResult};
use crate::collection::{self, Collection, OwnedStatue, Team};
use crate::missions::{self, Reward};
use crate::presents;
use crate::services::ok;
use crate::services::statue::with_profile;
use crate::services::team::{self, Roster};
use crate::Ctx;
use jdr_core::envelope::RequestEnvelope;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// The Arena campaign we list: one of the client's Arena campaign ids (its name labels exist,
/// `EQ_510027_Name`); which one is ours. Every pvp request sends it back as `eventCampaignId`.
pub const CAMPAIGN_ID: &str = "510027";
/// EventCampaignType.PvpAuto (client): the AI arena.
const EVENT_TYPE_PVP_AUTO: i64 = 7;
/// PvpBattleType.PvpAuto (client): `VSBattleManager.AIArenaMode`.
const BATTLE_TYPE_PVP_AUTO: i64 = 2;
/// GameConst.PVP_TEAM_HP_MAX (client).
pub const TEAM_HP_MAX: i64 = 10_000;
/// GameConst.feverGaugeMax (client).
pub const FEVER_GAUGE_MAX: i64 = 10_000;
/// GameConst.PVP_TIME_LIMIT_SEC (client).
const TIME_LIMIT_SECS: i64 = 120;
/// The campaign never closes offline (ours).
const OPEN_UNTIL: &str = "2099-12-31 23:59:59";
/// A date in the past: no Fever running.
const PAST: &str = "2000-01-01 00:00:00";
/// Battle turn limit (ours; the client needs a battleModeInfo and reads the limit from it).
pub const TURN_LIMIT: i64 = 20;
/// A team's BP cost (`fatiguCount`, client: shown and charged as the BP cost). Ours: BP never
/// runs out offline (UserInfo always reports 5 of 5), so every cost stays at one, which also
/// keeps the 2019 edit gate (no edits until the cost is back to one, `arena.edit_gate`) open.
const TEAM_COST: i64 = 1;
/// Durability a lost battle costs (ours: four defeats empty the bar).
pub const LOSS_DAMAGE: i64 = 2_500;
/// A team at 0 durability rests for thirty minutes (research pack `arena.durability_2019`).
pub const REST_SECS: i64 = 30 * 60;
/// Battles that fill the Fever gauge (research pack `arena.fever`: Fever after four battles).
const FEVER_BATTLES: i64 = 4;
/// Fever triples the points (`arena.fever`, `v2.arena.early_fever`); sent as a percentage (ours).
pub const FEVER_RATE: i64 = 300;
/// Fever lasts ten minutes (`v2.arena.early_fever`).
const FEVER_SECS: i64 = 10 * 60;
/// Team rank letters by FormationTeamRank 0..10 (client enum; our team_rank_table has 11 rows).
const RANKS: [&str; 11] = [
    "E", "D", "C", "B", "A", "S", "S+", "SS", "SS+", "SSS", "SSS+",
];
/// Points a win adds by the opponent's team rank (`teamPowerBasePointList`, ours).
const RANK_ADD_POINTS: [i64; 11] = [5, 8, 10, 12, 15, 18, 21, 24, 27, 30, 35];
/// Win-streak bonus in percent by streak length (`winningStreakPointRateList`, ours); the last
/// row applies to longer streaks.
const STREAK_RATES: [(i64, i64); 5] = [(1, 0), (2, 10), (3, 20), (4, 30), (5, 50)];
/// The three opponents: (level offset, rarity offset, base win points) (ours).
const TIERS: [(i64, i64, i64); 3] = [(-3, -1, 10), (0, 0, 15), (5, 1, 25)];
/// HP bonus for a win: up to this many points for a team that ends unhurt (ours).
const HP_BONUS_MAX: i64 = 20;
/// Points per statue still standing after a win (ours; survivors earn a bonus,
/// research pack `arena.survivor_bonus`).
const SURVIVOR_POINTS: i64 = 5;
/// Point rewards of a season (points, item, count), sent to the Message Box (ours). Item ids as
/// missions give them: 10 Diamond Coins, 20 gold, 31 Memory Rings, 3017 Fight Coins.
const LADDER: &[(i64, i64, i64)] = &[
    (100, 10, 50),
    (300, 31, 300),
    (600, 20, 100_000),
    (1_000, 10, 100),
    (1_500, crate::items::EVENT_COIN, 100),
    (2_500, 10, 200),
    (4_000, 31, 1_000),
    (6_000, 10, 300),
];
/// Season-end rewards by final ranking position (at most this position, item, count) (ours).
const SEASON_REWARDS: &[(i64, i64, i64)] = &[
    (10, 10, 500),
    (100, 10, 300),
    (500, 10, 150),
    (i64::MAX, 10, 50),
];
/// Ranking positions are against a virtual field (ours): position 1 at this many points, one
/// place lower for every RANKING_STEP points short of it.
const RANKING_TOP: i64 = 10_000;
const RANKING_STEP: i64 = 10;
/// Opponent handles (ours; never game text).
const HANDLES: [&str; 8] = [
    "Rival",
    "Challenger",
    "Duelist",
    "Contender",
    "Ace",
    "Veteran",
    "Drifter",
    "Wanderer",
];
/// Opponent ids start here (ours; `enemyuId` is an Int32).
const ENEMY_UID_BASE: i64 = 100_000;

/// Status codes (client E_PVP_*).
const E_PVP_ENEMY_NOT_FOUND: i64 = 52002;
const E_PVP_FEVER_LACK: i64 = 52003;
const E_PVP_NO_LEADER: i64 = 52006;
const E_PVP_TEAM_INTERVAL: i64 = 52015;

/// A team's durability below full (teams at full are not stored).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TeamHp {
    pub hp: i64,
    /// When a team at 0 is back (unix seconds; 0 while it still has durability).
    #[serde(default)]
    pub until: i64,
}

/// One AI-controlled opponent, kept until the next battle or shuffle so `pvp_start` fights the
/// team the list showed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Opponent {
    pub uid: i64,
    pub name: String,
    pub user_rank: i64,
    pub points: i64,
    pub level: i64,
    pub statues: Vec<String>,
    pub win_points: i64,
}

/// The battle `pvp_start` began; `pvp_end` settles it once.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingArena {
    pub enemy_uid: i64,
    pub team_num: i64,
    pub started: i64,
}

/// Arena progress (saved with the story progress).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ArenaState {
    /// The season (ours: a week, turning on Monday at 04:00 JST) these numbers belong to.
    #[serde(default)]
    pub season: i64,
    #[serde(default)]
    pub points: i64,
    #[serde(default)]
    pub streak: i64,
    #[serde(default)]
    pub season_battles: i64,
    /// Point rewards of this season already sent (by their point threshold).
    #[serde(default)]
    pub claimed: BTreeSet<i64>,
    #[serde(default)]
    pub wins: i64,
    #[serde(default)]
    pub losses: i64,
    /// The game day `todays_points` counts (days since the epoch, turning at 04:00 JST).
    #[serde(default)]
    pub day: i64,
    #[serde(default)]
    pub todays_points: i64,
    #[serde(default)]
    pub fever_gauge: i64,
    /// When the running Fever ends (unix seconds; in the past when none runs).
    #[serde(default)]
    pub fever_end: i64,
    /// Durability of damaged teams, by team number.
    #[serde(default)]
    pub hp: BTreeMap<i64, TeamHp>,
    #[serde(default)]
    pub opponents: Vec<Opponent>,
    #[serde(default)]
    pub next_enemy: i64,
    #[serde(default)]
    pub pending: Option<PendingArena>,
    #[serde(default)]
    pub selected_team: i64,
}

/// The season of a moment (ours): weeks of game days, starting on Monday.
pub fn season_of(now: i64) -> i64 {
    (missions::game_day(now) + 3).div_euclid(7) // 1970-01-01 (day 0) was a Thursday
}

/// The ranking position for a point total (ours; see RANKING_TOP).
pub fn ranking(points: i64) -> i64 {
    1 + (RANKING_TOP - points).max(0) / RANKING_STEP
}

fn date(t: i64) -> String {
    let (y, mo, d, h, mi, s) = jdr_core::clock::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

impl ArenaState {
    /// Durability of a team; a rested team is back at full.
    pub fn hp_of(&self, team_num: i64, now: i64) -> i64 {
        match self.hp.get(&team_num) {
            Some(h) if h.hp <= 0 && h.until <= now => TEAM_HP_MAX,
            Some(h) => h.hp,
            None => TEAM_HP_MAX,
        }
    }

    /// When a team at 0 durability is back, while it rests.
    pub fn resting_until(&self, team_num: i64, now: i64) -> Option<i64> {
        self.hp
            .get(&team_num)
            .filter(|h| h.hp <= 0 && h.until > now)
            .map(|h| h.until)
    }

    fn damage(&mut self, team_num: i64, n: i64, now: i64) {
        let h = self.hp.entry(team_num).or_insert(TeamHp {
            hp: TEAM_HP_MAX,
            until: 0,
        });
        h.hp = (h.hp - n).max(0);
        if h.hp == 0 {
            h.until = now + REST_SECS;
        }
    }

    /// Refill one team (or every team); returns whether anything changed.
    fn heal(&mut self, team_num: Option<i64>) -> bool {
        let before = self.hp.len();
        match team_num {
            Some(n) => {
                self.hp.remove(&n);
            }
            None => self.hp.clear(),
        }
        self.hp.len() != before
    }

    pub fn fever_on(&self, now: i64) -> bool {
        self.fever_end > now
    }

    /// Turn the day and the season; rested teams come back. Returns whether anything changed
    /// and, when a season with battles finished, its final ranking position.
    fn roll(&mut self, now: i64) -> (bool, Option<i64>) {
        let mut changed = false;
        let day = missions::game_day(now);
        if self.day != day {
            self.day = day;
            self.todays_points = 0;
            changed = true;
        }
        let before = self.hp.len();
        self.hp.retain(|_, h| !(h.hp <= 0 && h.until <= now));
        changed |= self.hp.len() != before;
        let season = season_of(now);
        if self.season == season {
            return (changed, None);
        }
        let finished = (self.season != 0 && self.season_battles > 0).then(|| ranking(self.points));
        self.season = season;
        self.points = 0;
        self.streak = 0;
        self.season_battles = 0;
        self.claimed.clear();
        self.fever_gauge = 0;
        self.fever_end = 0;
        self.opponents.clear();
        (true, finished)
    }
}

/// The reward for a season's final ranking position (ours).
pub fn season_reward(position: i64) -> (i64, i64) {
    SEASON_REWARDS
        .iter()
        .find(|(top, ..)| position <= *top)
        .map(|(_, item, n)| (*item, *n))
        .unwrap_or((10, 50))
}

fn send_reward(p: &mut Profile, item_id: i64, count: i64, comment: String, now: i64) {
    let (data_type, data_id) = presents::reward_kind(item_id);
    p.progress
        .presents
        .add(data_type, data_id, count, comment, now);
}

/// Every Arena route first turns the day and the season; a finished season's reward goes to the
/// Message Box.
fn touch(p: &mut Profile, now: i64) -> Change {
    let (changed, finished) = p.progress.arena.roll(now);
    if let Some(pos) = finished {
        let (item, n) = season_reward(pos);
        send_reward(
            p,
            item,
            n,
            format!("Arena season reward: final position {pos}."),
            now,
        );
        return Change::Progress;
    }
    if changed {
        Change::Bookkeeping
    } else {
        Change::None
    }
}

fn with_arena<R>(
    ctx: &Ctx,
    env: &RequestEnvelope,
    f: impl FnOnce(&mut Profile, i64) -> (R, Change),
) -> Result<(Profile, R), ApiError> {
    let now = jdr_core::clock::now();
    with_profile(ctx, env, |p| {
        let touched = touch(p, now);
        let (r, change) = f(p, now);
        (r, change.max(touched))
    })
}

fn int(env: &RequestEnvelope, k: &str) -> Option<i64> {
    env.param.get(k).and_then(Value::as_i64)
}

// ---- JSON shapes ----

/// The My Page entry of `/info/event_list` (EventListResponse/EventCampaignInfo, client).
pub fn campaign_entry() -> Value {
    json!({ "eventCampaignId": CAMPAIGN_ID, "eventType": EVENT_TYPE_PVP_AUTO,
            "eventEndDate": OPEN_UNTIL, "eventFinishedDate": OPEN_UNTIL })
}

/// `userBattleInfo.fieverEndTime`: My Page parses it as a date, so it is always one.
pub fn fever_end_time(st: &ArenaState, now: i64) -> String {
    if st.fever_on(now) {
        date(st.fever_end)
    } else {
        PAST.to_string()
    }
}

/// PvpTeamInfo: a TeamInfo plus `hp`, `recoveryTime` (a date the client counts down to while a
/// team rests) and `fatiguCount` (the BP cost). `st` None = an opponent, at full durability.
fn pvp_team_info(c: &Collection, st: Option<&ArenaState>, t: &Team, now: i64) -> Value {
    let mut v = collection::team_info(c, t);
    let hp = st.map(|s| s.hp_of(t.team_num, now)).unwrap_or(TEAM_HP_MAX);
    let back = st.and_then(|s| s.resting_until(t.team_num, now));
    v["hp"] = json!(hp);
    v["recoveryTime"] = json!(date(back.unwrap_or(now)));
    v["fatiguCount"] = json!(TEAM_COST);
    v
}

fn arena_team(c: &Collection, team_num: i64) -> Team {
    c.arena_teams
        .iter()
        .find(|t| t.team_num == team_num)
        .cloned()
        .unwrap_or(Team {
            stage_type: 1,
            team_num,
            ..Default::default()
        })
}

fn arena_team_info(p: &Profile, team_num: i64, now: i64) -> Value {
    let t = arena_team(&p.collection, team_num);
    pvp_team_info(&p.collection, Some(&p.progress.arena), &t, now)
}

fn team_list(p: &Profile, now: i64) -> Vec<Value> {
    let mut teams: Vec<&Team> = p.collection.arena_teams.iter().collect();
    teams.sort_by_key(|t| t.team_num);
    teams
        .iter()
        .map(|t| pvp_team_info(&p.collection, Some(&p.progress.arena), t, now))
        .collect()
}

fn empty_set() -> Value {
    json!({ "itemSetIcon": "", "itemSetNum": 0, "statueList": [], "mindList": [], "itemList": [],
            "clanObjectList": [] })
}

fn reward_set(item_id: i64, count: i64) -> Value {
    missions::reward_set(&Reward { item_id, count })
}

/// PvpUserInfo (client: must never be null; `nextEventItem` is an ItemSetInfo).
fn pvp_user_info(st: &ArenaState, now: i64) -> Value {
    let next = LADDER.iter().find(|(need, ..)| !st.claimed.contains(need));
    let fever = st.fever_on(now);
    json!({
        "rankingPoint": st.points, "feverEndDate": fever_end_time(st, now),
        "feverGauge": st.fever_gauge, "feverAddTime": 0, "feverAddFlag": 0,
        "feverRate": if fever { FEVER_RATE } else { 0 }, "rank": ranking(st.points),
        "winningStreak": st.streak, "nextEventPoint": next.map(|l| l.0).unwrap_or(0),
        "nextEventItem": next.map(|l| reward_set(l.1, l.2)).unwrap_or_else(empty_set),
        "crazyDiamondPoint": 0, "crazyDiamondNotifyFlg": false
    })
}

// ---- Team rank and points ----

/// Our team_rank_table: (rank, passingScore) rows.
fn rank_rows() -> &'static [(i64, i64)] {
    static ROWS: std::sync::OnceLock<Vec<(i64, i64)>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| {
        let js = crate::masterdata::TABLES
            .iter()
            .find(|(n, _)| *n == "team_rank_table")
            .map(|(_, j)| *j)
            .unwrap_or("{}");
        serde_json::from_str::<Value>(js)
            .ok()
            .and_then(|v| v["data"]["paramList"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|r| Some((r["rank"].as_i64()?, r["passingScore"].as_i64()?)))
            .collect()
    })
}

/// The team rank the client shows and scores with (`TeamRankTable.AnalyzeTeamRank`, client):
/// score = floor(hp/1000 + atk/2000 + def/2000 + ora + 0.003 x Mind capacity) over the team, and
/// the highest rank whose passingScore is reached. Minds' stat effects are left out (opponents
/// carry none).
pub fn team_rank(c: &Collection, t: &Team) -> usize {
    let (mut hp, mut atk, mut def, mut cap) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for o in t.slots.iter().filter_map(|u| c.known_statue(*u)) {
        let v = collection::statue_info(o);
        let g = |k: &str| v[k].as_f64().unwrap_or(0.0);
        hp += g("hp");
        atk += g("atk");
        def += g("def");
        cap += g("mindCapacity");
    }
    let score =
        (3.0 * hp / 3000.0 + 3.0 * atk / 6000.0 + 3.0 * def / 6000.0 + 3.0 * cap / 1000.0).floor();
    rank_rows()
        .iter()
        .filter(|(_, pass)| *pass as f64 <= score)
        .map(|(r, _)| *r)
        .max()
        .unwrap_or(0)
        .clamp(0, RANKS.len() as i64 - 1) as usize
}

fn streak_rate(streak: i64) -> i64 {
    STREAK_RATES
        .iter()
        .rev()
        .find(|(n, _)| streak >= *n)
        .map(|(_, r)| *r)
        .unwrap_or(0)
}

/// Points for a win: (winPoints + teamPowerBase[rank].addPoint) x (1 + streakRate/100), the
/// formula the opponent banner shows (client, the divisor 100 inferred); `streak` counts this win.
pub fn battle_points(win_points: i64, rank: usize, streak: i64) -> i64 {
    (win_points + RANK_ADD_POINTS[rank.min(RANK_ADD_POINTS.len() - 1)])
        * (100 + streak_rate(streak))
        / 100
}

// ---- Opponents ----

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Three statues of different characters, of one rarity (any lower rarity fills in when the
/// rarity has too few characters).
fn pick_three(rng: &mut Rng, rarity: i64) -> Vec<String> {
    let all = collection::statues();
    let distinct = |pool: &[&collection::StatueDef]| {
        pool.iter().map(|d| d.psid()).collect::<HashSet<_>>().len()
    };
    let mut pool: Vec<&collection::StatueDef> =
        all.iter().filter(|d| d.rarity() == rarity).collect();
    if distinct(&pool) < 3 {
        pool = all.iter().filter(|d| d.rarity() <= rarity).collect();
    }
    if distinct(&pool) < 3 {
        pool = all.iter().collect();
    }
    let mut out = Vec::new();
    let mut charas = Vec::new();
    for _ in 0..1000 {
        if out.len() == 3 || pool.is_empty() {
            break;
        }
        let d = pool[rng.below(pool.len())];
        if !charas.contains(&d.psid()) {
            charas.push(d.psid());
            out.push(d.statue_id.clone());
        }
    }
    out
}

/// A fresh list of three opponents, scaled on the player's chosen team (average level and
/// rarity) and season points: an easier, an even and a harder one (ours).
fn make_opponents(st: &mut ArenaState, c: &Collection, team_num: i64, user_rank: i64, now: i64) {
    let team = arena_team(c, team_num);
    let members: Vec<&OwnedStatue> = team
        .slots
        .iter()
        .filter_map(|u| c.known_statue(*u))
        .collect();
    let n = members.len().max(1) as i64;
    let level = (members.iter().map(|o| o.level).sum::<i64>() / n).max(1);
    let rarity = if members.is_empty() {
        3
    } else {
        members
            .iter()
            .map(|o| {
                collection::find(&o.statue_id)
                    .map(|d| d.rarity())
                    .unwrap_or(3)
            })
            .sum::<i64>()
            / n
    };
    let mut rng = Rng(now as u64 ^ (st.next_enemy as u64).rotate_left(29) ^ 0xa7e4_a5ee_d000_0001);
    let mut list = Vec::new();
    for (i, (dl, dr, win)) in TIERS.iter().enumerate() {
        st.next_enemy += 1;
        list.push(Opponent {
            uid: ENEMY_UID_BASE + st.next_enemy,
            name: format!(
                "{} {}",
                HANDLES[rng.below(HANDLES.len())],
                100 + rng.below(900)
            ),
            user_rank: (user_rank + dl + rng.below(5) as i64 - 2).max(1),
            points: (st.points + (i as i64 - 1) * 40 + rng.below(30) as i64).max(0),
            level: (level + st.points / 100 + dl).clamp(1, collection::MAX_LEVEL),
            statues: pick_three(&mut rng, (rarity + st.points / 2000 + dr).clamp(3, 6)),
            win_points: *win,
        });
    }
    st.opponents = list;
}

/// An opponent as a one-team collection, so its units are built exactly like the player's.
fn opponent_team(o: &Opponent) -> (Collection, Team) {
    let statues: Vec<OwnedStatue> = o
        .statues
        .iter()
        .enumerate()
        .map(|(i, id)| OwnedStatue {
            uid: i as i64 + 1,
            statue_id: id.clone(),
            level: o.level,
            exp: collection::exp_for_level(o.level),
            ..Default::default()
        })
        .collect();
    let slots = [0, 1, 2].map(|i| statues.get(i).map(|s| s.uid).unwrap_or(0));
    (
        Collection {
            statues,
            ..Default::default()
        },
        Team {
            stage_type: 1,
            team_num: 1,
            main: true,
            slots,
            ..Default::default()
        },
    )
}

/// PvpOpponentTeamInfo (client: the banner needs three StatueSettings in `teamOrder`).
fn opponent_info(o: &Opponent, now: i64) -> Value {
    let (c, t) = opponent_team(o);
    json!({ "pvpTeamInfo": pvp_team_info(&c, None, &t, now), "enemyName": o.name,
            "enemyRank": o.user_rank, "enemyEventPoint": o.points, "winPoints": o.win_points,
            "enemyuId": o.uid })
}

/// A team can fight when it has a leader and is not resting.
fn check_team(c: &Collection, st: &ArenaState, team_num: i64, now: i64) -> Result<(), i64> {
    let t = arena_team(c, team_num);
    if c.known_statue(t.slots[0]).is_none() {
        return Err(E_PVP_NO_LEADER);
    }
    if st.resting_until(team_num, now).is_some() {
        return Err(E_PVP_TEAM_INTERVAL);
    }
    Ok(())
}

fn arena_team_num(env: &RequestEnvelope, key: &str) -> Result<i64, ApiError> {
    match int(env, key) {
        Some(n) if (1..=collection::TEAMS_PER_TYPE).contains(&n) => Ok(n),
        other => Err(ApiError::Internal(format!("{key} {other:?} out of range"))),
    }
}

// ---- Routes: home ----

/// `/pvp/get_pvp_config {eventCampaignId}`: the scoring tables the home and the banners use
/// (an out-of-range rank index scores 0 in the client, so all 11 ranks are listed).
pub fn get_pvp_config(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    let bases: Vec<Value> = RANKS
        .iter()
        .zip(RANK_ADD_POINTS)
        .map(|(r, p)| json!({ "Rank": r, "addPoint": p }))
        .collect();
    let streaks: Vec<Value> = STREAK_RATES
        .iter()
        .map(|(n, r)| json!({ "winningStreakNum": n, "pointRate": r }))
        .collect();
    ok(json!({ "teamPowerBasePointList": bases, "winningStreakPointRateList": streaks }))
}

/// `/event/point_reward_list {eventCampaignId}`: the season's point rewards (EventPointInfo,
/// `getFlg` 1 once sent). Other campaigns have none.
pub fn point_reward_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let id = env
        .param
        .get("eventCampaignId")
        .and_then(Value::as_str)
        .unwrap_or(CAMPAIGN_ID);
    if id != CAMPAIGN_ID {
        return ok(
            json!({ "userEventPoint": 0, "clanEventPoint": 0, "userPointRewardList": [],
                          "clanPointRewardList": [] }),
        );
    }
    let (p, ()) = with_arena(ctx, env, |_, _| ((), Change::None))?;
    let st = &p.progress.arena;
    let list: Vec<Value> = LADDER
        .iter()
        .map(|(need, item, n)| {
            json!({ "rewardTitle": "", "rewardTitleReplace": "", "rewardText": "",
                    "rewardTextReplace": "", "necessaryEventPoint": need,
                    "itemSetInfo": reward_set(*item, *n),
                    "getFlg": st.claimed.contains(need) as i64 })
        })
        .collect();
    ok(
        json!({ "userEventPoint": st.points, "clanEventPoint": 0, "userPointRewardList": list,
               "clanPointRewardList": [] }),
    )
}

/// `/item/heal_item_list {type}` (2 = BP items in the Arena home): held recovery items. Our item
/// catalog has none (no cola or recovery item ids are recorded), so the list is empty and the
/// home offers its Diamond Coin option, which is free offline.
pub fn heal_item_list(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "itemList": [] }))
}

/// `/pvp/home_acqusition_check {eventCampaignId}` (polled): nothing to collect offline.
pub fn home_acqusition_check(_ctx: &Ctx, _env: &RequestEnvelope) -> ApiResult {
    ok(json!({ "result": 0 }))
}

/// `/pvp/home {eventCampaignId, pollingFlg}`: teams, the player's Arena standing and today's
/// points. Nobody attacks offline, so `damageInfo` is empty; lists are never null (client).
pub fn home(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (p, ()) = with_arena(ctx, env, |_, _| ((), Change::None))?;
    let now = jdr_core::clock::now();
    let st = &p.progress.arena;
    ok(json!({
        "endDate": OPEN_UNTIL, "finishDate": OPEN_UNTIL, "pvpTeamList": team_list(&p, now),
        "pvpUserInfo": pvp_user_info(st, now), "todaysPoint": st.todays_points.max(0),
        "recoveryTimes": 0, "selectedTeamId": st.selected_team.clamp(1, collection::TEAMS_PER_TYPE),
        "pollingTimes": 0, "damageInfo": [], "eventCampaignList": [], "forceDefeatFlg": false,
        "battleType": BATTLE_TYPE_PVP_AUTO
    }))
}

/// `/pvp/enemy_player_list {eventCampaignId, teamNo, reChoiceFlg}`: three opponents for the
/// chosen team, kept until a battle or a shuffle (`reChoiceFlg` 1; free offline). A resting team
/// gets 52015 (the client handles it), a team without a leader 52006.
pub fn enemy_player_list(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = arena_team_num(env, "teamNo")?;
    let shuffle = int(env, "reChoiceFlg") == Some(1);
    let (p, r) = with_arena(ctx, env, |p, now| {
        if let Err(code) = check_team(&p.collection, &p.progress.arena, team_num, now) {
            return (Err(code), Change::None);
        }
        if shuffle || p.progress.arena.opponents.is_empty() {
            make_opponents(
                &mut p.progress.arena,
                &p.collection,
                team_num,
                p.rank.max(1),
                now,
            );
            return (Ok(()), Change::Bookkeeping);
        }
        (Ok(()), Change::None)
    })?;
    r.map_err(ApiError::Status)?;
    let now = jdr_core::clock::now();
    let list: Vec<Value> = p
        .progress
        .arena
        .opponents
        .iter()
        .map(|o| opponent_info(o, now))
        .collect();
    ok(json!({ "pvpTeamList": list, "userInfo": crate::services::user::user_info(&p) }))
}

/// `/pvp/pvp_start {eventCampaignId, teamNum, enemyuId, enemyTeamNum}`: both teams and the
/// battle mode (battleType 2 = the AI arena, `battleModeInfo` must not be null, client). The
/// client picks the map and the formation itself. BP is not charged (UserInfo keeps 5 of 5).
pub fn pvp_start(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = arena_team_num(env, "teamNum")?;
    let enemy = int(env, "enemyuId").unwrap_or(0);
    let (p, r) = with_arena(ctx, env, |p, now| {
        let st = &p.progress.arena;
        if let Err(code) = check_team(&p.collection, st, team_num, now) {
            return (Err(code), Change::None);
        }
        let Some(o) = st.opponents.iter().find(|o| o.uid == enemy).cloned() else {
            return (Err(E_PVP_ENEMY_NOT_FOUND), Change::None);
        };
        let st = &mut p.progress.arena;
        st.pending = Some(PendingArena {
            enemy_uid: enemy,
            team_num,
            started: now,
        });
        st.selected_team = team_num;
        (Ok(o), Change::Bookkeeping)
    })?;
    let o = r.map_err(ApiError::Status)?;
    let now = jdr_core::clock::now();
    let st = &p.progress.arena;
    let (oc, ot) = opponent_team(&o);
    ok(json!({
        "userInfo": crate::services::user::user_info(&p),
        "pvpTeamInfo": arena_team_info(&p, team_num, now),
        "pvpEnemyTeamInfo": pvp_team_info(&oc, None, &ot, now),
        "pvpEnemyInfo": { "enemyName": o.name, "enemyRank": o.user_rank, "rank": ranking(o.points),
                          "enemyEventPoint": o.points, "enemyComment": "" },
        "winningStreak": st.streak,
        "battleModeInfo": { "battleType": BATTLE_TYPE_PVP_AUTO, "battleTimelimit": TIME_LIMIT_SECS,
                            "battleTurnlimit": TURN_LIMIT },
        "userRank": p.rank.max(1), "winningStreakAtkRate": 0, "winningStreakDefRate": 0,
        "seed": now & 0x7fff_ffff, "dayNight": 0
    }))
}

/// What a settled battle paid.
#[derive(Debug, Default)]
struct EndResult {
    team_num: i64,
    enemy_uid: i64,
    points: i64,
    hp_bonus: i64,
    rest_bonus: i64,
    fever_points: i64,
    fever: bool,
    rewards: Vec<(i64, i64, i64)>,
}

/// `/pvp/pvp_end {result (0 win, 1 loss), defeatedEnemyList, teamNo, battleTime, battleTurn,
/// myTeamInfoList [{hp}], myTeamPower, enemyTeamPower}`: settles the battle `pvp_start` began,
/// once (a resent request pays nothing).
///
/// A win (ours, on the client's banner formula): battle points by the opponent's base points,
/// its team rank and the streak; an HP bonus for the share of HP left (`myTeamInfoList`) and a
/// bonus per statue standing; Fever triples the lot. A loss costs the team LOSS_DAMAGE
/// durability and ends the streak; points are never taken. Point rewards reached go to the
/// Message Box and are listed in `eventRewardList`.
pub fn pvp_end(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    // ResultType Win 0 / Lose 1 (client). A missing result counts as a loss (ours; the early
    // guide counts interrupted battles as defeats, `v2.arena.early_failure`).
    let won = int(env, "result") == Some(0);
    let hps: Vec<i64> = env
        .param
        .get("myTeamInfoList")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|m| m.get("hp")?.as_i64())
                .map(|h| h.max(0))
                .collect()
        })
        .unwrap_or_default();
    let (p, out) = with_arena(ctx, env, |p, now| {
        let Some(b) = p.progress.arena.pending.take() else {
            return (None, Change::None);
        };
        let team = arena_team(&p.collection, b.team_num);
        let mut r = EndResult {
            team_num: b.team_num,
            enemy_uid: b.enemy_uid,
            ..Default::default()
        };
        let opp = p
            .progress
            .arena
            .opponents
            .iter()
            .find(|o| o.uid == b.enemy_uid)
            .cloned();
        let max_hp: i64 = team
            .slots
            .iter()
            .filter_map(|u| p.collection.known_statue(*u))
            .map(|o| collection::statue_info(o)["hp"].as_i64().unwrap_or(0))
            .sum();
        let st = &mut p.progress.arena;
        st.season_battles += 1;
        if won {
            st.wins += 1;
            st.streak += 1;
            if let Some(o) = &opp {
                let (oc, ot) = opponent_team(o);
                r.points = battle_points(o.win_points, team_rank(&oc, &ot), st.streak);
            }
            let left: i64 = hps.iter().sum();
            if max_hp > 0 {
                r.hp_bonus = (HP_BONUS_MAX * left.min(max_hp) + max_hp / 2) / max_hp;
            }
            r.rest_bonus = SURVIVOR_POINTS * hps.iter().filter(|h| **h > 0).count().min(3) as i64;
        } else {
            st.losses += 1;
            st.streak = 0;
            st.damage(b.team_num, LOSS_DAMAGE, now);
        }
        if st.fever_on(now) {
            r.fever = true;
            r.fever_points = (r.points + r.hp_bonus + r.rest_bonus) * (FEVER_RATE - 100) / 100;
        } else {
            st.fever_gauge =
                (st.fever_gauge + FEVER_GAUGE_MAX / FEVER_BATTLES).min(FEVER_GAUGE_MAX);
        }
        let total = r.points + r.hp_bonus + r.rest_bonus + r.fever_points;
        st.points += total;
        st.todays_points += total;
        st.opponents.clear();
        st.selected_team = b.team_num;
        for l in LADDER {
            if l.0 <= st.points && st.claimed.insert(l.0) {
                r.rewards.push(*l);
            }
        }
        for (need, item, n) in r.rewards.clone() {
            send_reward(
                p,
                item,
                n,
                format!("Arena point reward: {need} points reached."),
                now,
            );
        }
        (Some(r), Change::Progress)
    })?;
    if out.is_none() {
        jdr_core::log::line(
            "arena",
            "pvp_end without a matching pvp_start: nothing paid (resent?)",
        );
    }
    let r = out.unwrap_or_default();
    let now = jdr_core::clock::now();
    let st = &p.progress.arena;
    let team_num = if r.team_num != 0 {
        r.team_num
    } else {
        int(env, "teamNo")
            .filter(|n| (1..=collection::TEAMS_PER_TYPE).contains(n))
            .unwrap_or(1)
    };
    let rewards: Vec<Value> = r
        .rewards
        .iter()
        .map(|(need, item, n)| {
            json!({ "itemSetInfo": reward_set(*item, *n), "necessaryEventPoint": need,
                    "rewardTitle": "", "rewardTitleReplace": "" })
        })
        .collect();
    ok(json!({
        "userInfo": crate::services::user::user_info(&p),
        "pvpTeamInfo": arena_team_info(&p, team_num, now),
        "getRankingPoint": r.points, "enemyuId": r.enemy_uid, "winningStreak": st.streak,
        "feverRate": if r.fever { FEVER_RATE } else { 0 }, "feverGauge": st.fever_gauge,
        "getFeverRankingPoint": r.fever_points, "totalWin": st.wins, "totalLose": st.losses,
        "totalHpBonusPoint": r.hp_bonus, "restStatueBonusPoint": r.rest_bonus,
        "eventRewardList": rewards
    }))
}

/// `/pvp/recover_hp_pvp {typeId (0 Diamond Coins, 1 item), itemId, eventCampaignId, teamNum}`:
/// refills the team's durability (every team when `teamNum` names none). Free offline: no coins
/// or items are taken (ours).
pub fn recover_hp_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let team_num = int(env, "teamNum").filter(|n| (1..=collection::TEAMS_PER_TYPE).contains(n));
    let (p, ()) = with_arena(ctx, env, |p, _| {
        let changed = p.progress.arena.heal(team_num);
        (
            (),
            if changed {
                Change::Bookkeeping
            } else {
                Change::None
            },
        )
    })?;
    ok(json!({ "userInfo": crate::services::user::user_info(&p), "itemList": [] }))
}

/// `/pvp/fever_mode {eventCampaignId}`: starts Fever once the gauge is full (52003 otherwise).
pub fn fever_mode(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (p, r) = with_arena(ctx, env, |p, now| {
        let st = &mut p.progress.arena;
        if st.fever_on(now) {
            return (Ok(()), Change::None);
        }
        if st.fever_gauge < FEVER_GAUGE_MAX {
            return (Err(E_PVP_FEVER_LACK), Change::None);
        }
        st.fever_gauge = 0;
        st.fever_end = now + FEVER_SECS;
        (Ok(()), Change::Bookkeeping)
    })?;
    r.map_err(ApiError::Status)?;
    let now = jdr_core::clock::now();
    let st = &p.progress.arena;
    ok(json!({
        "userInfo": crate::services::user::user_info(&p), "feverEndDate": fever_end_time(st, now),
        "feverGauge": st.fever_gauge, "feverAddTime": 0, "feverAddFlag": 0,
        "feverRate": if st.fever_on(now) { FEVER_RATE } else { 0 }
    }))
}

// ---- Routes: the Arena roster (formation scene 901) ----

/// `/pvp/team_info_pvp {}`: the five Arena teams and the open Mind slots.
pub fn team_info_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let (p, ()) = with_arena(ctx, env, |_, _| ((), Change::None))?;
    let now = jdr_core::clock::now();
    ok(json!({ "pvpTeamList": team_list(&p, now), "mindSeqInfo": team::mind_seq_info() }))
}

/// Run an edit on the Arena roster and reply with the team (`pvpTeamInfo`).
fn edit(
    ctx: &Ctx,
    env: &RequestEnvelope,
    team_num: i64,
    f: impl FnOnce(&mut Collection) -> Result<bool, i64>,
) -> ApiResult {
    let (p, r) = with_arena(ctx, env, |p, _| match f(&mut p.collection) {
        Ok(true) => (Ok(()), Change::Bookkeeping),
        Ok(false) => (Ok(()), Change::None),
        Err(code) => (Err(code), Change::None),
    })?;
    r.map_err(ApiError::Status)?;
    ok(json!({ "pvpTeamInfo": arena_team_info(&p, team_num, jdr_core::clock::now()) }))
}

/// `/pvp/set_team_pvp {teamNum, leaderstatueuId, memberLeftStatueuId, memberRightStatueuId}`
/// (no caller found in 3.12.0; kept like `/team/set_team`).
pub fn set_team_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let n = team::team_num(env)?;
    let wanted = team::members_of(env);
    edit(ctx, env, n, |c| {
        Ok(team::set_members(c, Roster::Arena, n, wanted).is_some())
    })
}

/// `/pvp/statue_select_pvp {teamNum, statueSeq, statueuId}`.
pub fn statue_select_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let n = team::team_num(env)?;
    let seq = int(env, "statueSeq").unwrap_or(0);
    let uid = int(env, "statueuId").unwrap_or(0);
    edit(ctx, env, n, |c| {
        Ok(team::select_statue(c, Roster::Arena, n, seq, uid))
    })
}

/// `/pvp/leader_change_pvp {teamNum, statueSeq}`.
pub fn leader_change_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let n = team::team_num(env)?;
    let seq = int(env, "statueSeq").unwrap_or(1);
    edit(ctx, env, n, |c| {
        Ok(team::change_leader(c, Roster::Arena, n, seq))
    })
}

/// `/pvp/equip_mind_pvp {teamNum, statueSeq, equipSeq, minduId}`.
pub fn equip_mind_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let n = team::team_num(env)?;
    let seat = team::seat_of(env)?;
    let change = team::one_change(env);
    edit(ctx, env, n, |c| {
        team::equip(c, Roster::Arena, n, seat, &[change]).map(|()| true)
    })
}

/// `/pvp/equip_multiple_minds_pvp {teamNum, statueSeq, mindList:[{equipSeq, minduId}]}`.
pub fn equip_multiple_minds_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let n = team::team_num(env)?;
    let seat = team::seat_of(env)?;
    let changes = team::changes_of(env.param.get("mindList"), "minduId");
    edit(ctx, env, n, |c| {
        team::equip(c, Roster::Arena, n, seat, &changes).map(|()| true)
    })
}

/// `/pvp/equip_multiple_assist_pvp {teamNum, statueSeq, statueList:[{equipSeq, statueuId}]}`.
pub fn equip_multiple_assist_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let n = team::team_num(env)?;
    let seat = team::seat_of(env)?;
    let changes = team::changes_of(env.param.get("statueList"), "statueuId");
    edit(ctx, env, n, |c| {
        team::equip_assists(c, Roster::Arena, n, seat, &changes).map(|()| true)
    })
}

/// `/pvp/auto_set_and_equip {teamNum, changePlan 1..4}` (the auto formation on an Arena team;
/// statues and Minds of the other Arena teams are left alone).
pub fn auto_set_and_equip_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let n = team::team_num(env)?;
    let plan = int(env, "changePlan").unwrap_or(1);
    edit(ctx, env, n, |c| {
        team::auto_formation(c, Roster::Arena, n, plan).map(|()| true)
    })
}

/// One seat of an EquipPvpTeamInfo: (statue uid, Mind changes, assist changes).
type SeatSetting = (i64, Vec<(i64, i64)>, Vec<(i64, i64)>);

fn seat_setting(v: &Value) -> SeatSetting {
    (
        v.get("statueuId").and_then(Value::as_i64).unwrap_or(0),
        team::changes_of(v.get("equipMindList"), "minduId"),
        team::changes_of(v.get("assistStatueList"), "statueuId"),
    )
}

/// `/pvp/set_team_all_pvp {teamList:[{teamNum, leaderStatueInfo, memberLeftStatueInfo,
/// memberRightStatueInfo}]}` (EquipStatueSetting {statueuId, equipMindList [{equipSeq,
/// minduId}], assistStatueList [{equipSeq, statueuId}]}, client): each listed team is replaced
/// whole. A statue keeps to one Arena team (the 2019 guide, `arena.teams`): one already in an
/// unlisted team, or in an earlier listed one, leaves an empty seat. A team left without a
/// leader is not saved; Minds or assists that do not fit a seat are dropped. Replies with all
/// five teams.
pub fn set_team_all_pvp(ctx: &Ctx, env: &RequestEnvelope) -> ApiResult {
    let entries: Vec<(i64, [SeatSetting; 3])> = env
        .param
        .get("teamList")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|t| {
                    let n = t.get("teamNum")?.as_i64()?;
                    (1..=collection::TEAMS_PER_TYPE).contains(&n).then(|| {
                        let seat = |k: &str| seat_setting(t.get(k).unwrap_or(&Value::Null));
                        (
                            n,
                            [
                                seat("leaderStatueInfo"),
                                seat("memberLeftStatueInfo"),
                                seat("memberRightStatueInfo"),
                            ],
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let (p, ()) = with_arena(ctx, env, |p, _| {
        let c = &mut p.collection;
        let listed: HashSet<i64> = entries.iter().map(|(n, _)| *n).collect();
        let mut used: HashSet<i64> = c
            .arena_teams
            .iter()
            .filter(|t| !listed.contains(&t.team_num))
            .flat_map(|t| t.slots)
            .filter(|u| *u != 0)
            .collect();
        let mut changed = false;
        for (n, seats) in &entries {
            let wanted = [0, 1, 2].map(|i| {
                let u = seats[i].0;
                if used.contains(&u) {
                    0
                } else {
                    u
                }
            });
            let Some(slots) = team::clean_members(c, wanted) else {
                continue;
            };
            used.extend(slots.iter().filter(|u| **u != 0));
            if let Some(t) = c.arena_teams.iter_mut().find(|t| t.team_num == *n) {
                t.slots = slots;
                t.minds = Default::default();
                t.assists = Default::default();
            }
            for (seat, s) in seats.iter().enumerate() {
                if slots[seat] == 0 {
                    continue;
                }
                let minds: Vec<(i64, i64)> = s.1.iter().filter(|(_, u)| *u != 0).copied().collect();
                if team::equip(c, Roster::Arena, *n, seat, &minds).is_err() {
                    // Keep what fits: one at a time, in order.
                    for m in &minds {
                        let _ = team::equip(c, Roster::Arena, *n, seat, &[*m]);
                    }
                }
                for a in s.2.iter().filter(|(_, u)| *u != 0) {
                    let _ = team::equip_assists(c, Roster::Arena, *n, seat, &[*a]);
                }
            }
            changed = true;
        }
        (
            (),
            if changed {
                Change::Bookkeeping
            } else {
                Change::None
            },
        )
    })?;
    ok(json!({ "pvpTeamList": team_list(&p, jdr_core::clock::now()) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::dispatch;
    use jdr_core::settings::Settings;
    use serde_json::Map;

    fn env(uuid: &str, param: Value) -> RequestEnvelope {
        RequestEnvelope {
            uuid: uuid.into(),
            session: String::new(),
            language: 1,
            date: 20261118103000,
            platform: 2,
            version: "vtest".into(),
            lz_accept: 1,
            lz_encode: 0,
            param,
        }
    }

    fn setup(tag: &str) -> (Ctx, String) {
        let dir = std::env::temp_dir().join(format!("jdr-arena-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = Ctx::new(Settings::new(dir), false);
        let uuid = ctx.accounts.create(&[]).unwrap().uuid;
        (ctx, uuid)
    }

    /// Crash trap check: nothing the client reads may be null.
    fn no_nulls(path: &str, v: &Value) {
        match v {
            Value::Null => panic!("null at {path}"),
            Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    no_nulls(&format!("{path}[{i}]"), x);
                }
            }
            Value::Object(m) => {
                for (k, x) in m {
                    no_nulls(&format!("{path}.{k}"), x);
                }
            }
            _ => {}
        }
    }

    fn call(ctx: &Ctx, uuid: &str, path: &str, param: Value) -> Result<Map<String, Value>, i64> {
        let (r, known) = dispatch(ctx, path, &env(uuid, param));
        assert!(known, "{path} is routed");
        match r {
            Ok(m) => {
                no_nulls(path, &Value::Object(m.clone()));
                Ok(m)
            }
            Err(ApiError::Status(c)) => Err(c),
            Err(ApiError::Internal(e)) => panic!("{path}: {e}"),
        }
    }

    fn seats(t: &Value) -> Vec<String> {
        ["leaderStatue", "memberLeftStatue", "memberRightStatue"]
            .iter()
            .map(|k| {
                t["teamOrder"][*k]["statueInfo"]["statueId"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn my_page_entry_and_home() {
        let (ctx, uuid) = setup("home");
        let ev = call(&ctx, &uuid, "/info/event_list", json!({})).unwrap();
        let e = &ev["eventList"][0];
        assert_eq!(e["eventType"], 7);
        assert_eq!(e["eventCampaignId"], CAMPAIGN_ID);
        assert!(e["eventEndDate"].as_str().unwrap() > "2090");
        assert!(e["eventFinishedDate"].is_string());
        assert!(
            ev["userBattleInfo"]["fieverEndTime"].is_string(),
            "parsed as a date by My Page"
        );

        let cfg = call(&ctx, &uuid, "/pvp/get_pvp_config", json!({})).unwrap();
        assert_eq!(
            cfg["teamPowerBasePointList"].as_array().unwrap().len(),
            11,
            "one row per team rank E..SSS+"
        );
        assert!(!cfg["winningStreakPointRateList"]
            .as_array()
            .unwrap()
            .is_empty());
        let pr = call(
            &ctx,
            &uuid,
            "/event/point_reward_list",
            json!({"eventCampaignId": CAMPAIGN_ID}),
        )
        .unwrap();
        assert_eq!(
            pr["userPointRewardList"].as_array().unwrap().len(),
            LADDER.len()
        );
        assert_eq!(pr["clanPointRewardList"], json!([]));
        let other = call(
            &ctx,
            &uuid,
            "/event/point_reward_list",
            json!({"eventCampaignId": "999"}),
        )
        .unwrap();
        assert_eq!(other["userPointRewardList"], json!([]));
        let heal = call(&ctx, &uuid, "/item/heal_item_list", json!({"type": 2})).unwrap();
        assert!(heal["itemList"].is_array());
        let chk = call(&ctx, &uuid, "/pvp/home_acqusition_check", json!({})).unwrap();
        assert_eq!(chk["result"], 0);

        let h = call(
            &ctx,
            &uuid,
            "/pvp/home",
            json!({"eventCampaignId": CAMPAIGN_ID, "pollingFlg": 0}),
        )
        .unwrap();
        let teams = h["pvpTeamList"].as_array().unwrap();
        assert_eq!(teams.len(), 5);
        for t in teams {
            assert_eq!(t["hp"], TEAM_HP_MAX);
            assert_eq!(t["fatiguCount"], 1);
            assert!(t["recoveryTime"].is_string());
            assert!(t["teamOrder"]["memberRightStatue"].is_object());
        }
        assert!(
            !seats(&teams[0])[0].is_empty(),
            "Arena team 1 starts with the main team"
        );
        let u = &h["pvpUserInfo"];
        assert!(u.is_object(), "pvpUserInfo never null");
        assert!(u["nextEventItem"]["itemList"].is_array());
        assert!(u["feverEndDate"].is_string());
        assert_eq!(h["eventCampaignList"], json!([]));
        assert_eq!(h["damageInfo"], json!([]));
        assert_eq!(h["battleType"], 2);
    }

    #[test]
    fn opponents_battle_points_and_streak() {
        let (ctx, uuid) = setup("battle");
        let list = call(
            &ctx,
            &uuid,
            "/pvp/enemy_player_list",
            json!({"teamNo": 1, "reChoiceFlg": 0}),
        )
        .unwrap();
        let opps = list["pvpTeamList"].as_array().unwrap().clone();
        assert_eq!(opps.len(), 3);
        for o in &opps {
            let ids = seats(&o["pvpTeamInfo"]);
            assert!(ids.iter().all(|s| !s.is_empty()), "three statues: {ids:?}");
            let charas: HashSet<&str> = ids.iter().map(|i| i.split('_').next().unwrap()).collect();
            assert_eq!(charas.len(), 3, "three characters");
            let lead = &o["pvpTeamInfo"]["teamOrder"]["leaderStatue"]["statueInfo"];
            assert!(lead["modelDataList"].is_array() && lead["stEffRateInfo"].is_object());
            assert!(o["winPoints"].as_i64().unwrap() > 0);
            assert!(o["enemyName"].as_str().unwrap().is_ascii());
        }
        // The same list until a shuffle.
        let again = call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})).unwrap();
        assert_eq!(again["pvpTeamList"], json!(opps));
        let shuffled = call(
            &ctx,
            &uuid,
            "/pvp/enemy_player_list",
            json!({"teamNo": 1, "reChoiceFlg": 1}),
        )
        .unwrap();
        let ids = |l: &Value| -> Vec<i64> {
            l.as_array()
                .unwrap()
                .iter()
                .map(|o| o["enemyuId"].as_i64().unwrap())
                .collect()
        };
        let fresh = ids(&shuffled["pvpTeamList"]);
        assert!(fresh.iter().all(|u| !ids(&json!(opps)).contains(u)));

        // Unknown opponent.
        assert_eq!(
            call(
                &ctx,
                &uuid,
                "/pvp/pvp_start",
                json!({"teamNum": 1, "enemyuId": 1, "enemyTeamNum": 1})
            ),
            Err(E_PVP_ENEMY_NOT_FOUND)
        );
        // A win.
        let s = call(
            &ctx,
            &uuid,
            "/pvp/pvp_start",
            json!({"eventCampaignId": CAMPAIGN_ID, "teamNum": 1, "enemyuId": fresh[1], "enemyTeamNum": 1}),
        )
        .unwrap();
        let m = &s["battleModeInfo"];
        assert_eq!(
            (m["battleType"].as_i64(), m["battleTimelimit"].as_i64()),
            (Some(2), Some(120))
        );
        assert_eq!(m["battleTurnlimit"], TURN_LIMIT);
        assert!(seats(&s["pvpEnemyTeamInfo"]).iter().all(|x| !x.is_empty()));
        assert!(s["userInfo"]["username"].is_string());
        assert!(s["seed"].is_i64());
        let win = |hp: Value| {
            call(
                &ctx,
                &uuid,
                "/pvp/pvp_end",
                json!({"result": 0, "teamNo": 1, "defeatedEnemyList": [], "battleTime": "60",
                       "battleTurn": 5, "myTeamInfoList": hp, "myTeamPower": 10, "enemyTeamPower": 10}),
            )
            .unwrap()
        };
        let e1 = win(json!([{"hp": 500}, {"hp": 0}, {"hp": 300}]));
        assert!(e1["getRankingPoint"].as_i64().unwrap() > 0);
        assert_eq!(e1["winningStreak"], 1);
        assert_eq!(e1["restStatueBonusPoint"], 2 * SURVIVOR_POINTS);
        assert!(e1["totalHpBonusPoint"].as_i64().unwrap() >= 0);
        assert_eq!(e1["totalWin"], 1);
        assert!(e1["eventRewardList"].is_array());
        let p1 = ctx.accounts.get(&uuid).unwrap().unwrap().progress.arena;
        let paid = e1["getRankingPoint"].as_i64().unwrap()
            + e1["totalHpBonusPoint"].as_i64().unwrap()
            + e1["restStatueBonusPoint"].as_i64().unwrap();
        assert_eq!(p1.points, paid);
        assert!(p1.pending.is_none());
        assert!(p1.opponents.is_empty(), "a new list after each battle");
        // Resent: nothing more.
        let e1b = win(json!([]));
        assert_eq!(e1b["getRankingPoint"], 0);
        assert_eq!(
            ctx.accounts
                .get(&uuid)
                .unwrap()
                .unwrap()
                .progress
                .arena
                .points,
            p1.points
        );

        // A second win: streak 2.
        let l = call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})).unwrap();
        let uid = l["pvpTeamList"][0]["enemyuId"].as_i64().unwrap();
        let s2 = call(
            &ctx,
            &uuid,
            "/pvp/pvp_start",
            json!({"teamNum": 1, "enemyuId": uid, "enemyTeamNum": 1}),
        )
        .unwrap();
        assert_eq!(s2["winningStreak"], 1);
        let e2 = win(json!([{"hp": 1}, {"hp": 1}, {"hp": 1}]));
        assert_eq!(e2["winningStreak"], 2);
        assert_eq!(e2["restStatueBonusPoint"], 3 * SURVIVOR_POINTS);

        // A loss ends the streak and costs durability; points stay.
        let before = ctx
            .accounts
            .get(&uuid)
            .unwrap()
            .unwrap()
            .progress
            .arena
            .points;
        let l = call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})).unwrap();
        let uid = l["pvpTeamList"][2]["enemyuId"].as_i64().unwrap();
        call(
            &ctx,
            &uuid,
            "/pvp/pvp_start",
            json!({"teamNum": 1, "enemyuId": uid, "enemyTeamNum": 1}),
        )
        .unwrap();
        let e3 = call(
            &ctx,
            &uuid,
            "/pvp/pvp_end",
            json!({"result": 1, "teamNo": 1, "myTeamInfoList": [{"hp": 0}, {"hp": 0}, {"hp": 0}]}),
        )
        .unwrap();
        assert_eq!(e3["winningStreak"], 0);
        assert_eq!(e3["getRankingPoint"], 0);
        assert_eq!(e3["totalLose"], 1);
        assert_eq!(e3["pvpTeamInfo"]["hp"], TEAM_HP_MAX - LOSS_DAMAGE);
        let st = ctx.accounts.get(&uuid).unwrap().unwrap().progress.arena;
        assert_eq!(st.points, before);
        assert_eq!(st.fever_gauge, 3 * FEVER_GAUGE_MAX / FEVER_BATTLES);
        let h = call(&ctx, &uuid, "/pvp/home", json!({})).unwrap();
        assert_eq!(h["todaysPoint"], st.points);
        assert_eq!(h["pvpUserInfo"]["rankingPoint"], st.points);
        assert_eq!(h["pvpUserInfo"]["winningStreak"], 0);
    }

    #[test]
    fn durability_rest_and_recovery() {
        let (ctx, uuid) = setup("hp");
        for _ in 0..(TEAM_HP_MAX / LOSS_DAMAGE) {
            let l = call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})).unwrap();
            let uid = l["pvpTeamList"][0]["enemyuId"].as_i64().unwrap();
            call(
                &ctx,
                &uuid,
                "/pvp/pvp_start",
                json!({"teamNum": 1, "enemyuId": uid, "enemyTeamNum": 1}),
            )
            .unwrap();
            call(
                &ctx,
                &uuid,
                "/pvp/pvp_end",
                json!({"result": 1, "teamNo": 1}),
            )
            .unwrap();
        }
        let h = call(&ctx, &uuid, "/pvp/home", json!({})).unwrap();
        let t1 = &h["pvpTeamList"][0];
        assert_eq!(t1["hp"], 0);
        assert!(t1["recoveryTime"].as_str().unwrap() > ctx.server_date().as_str());
        assert_eq!(
            call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})),
            Err(E_PVP_TEAM_INTERVAL),
            "a resting team cannot pick opponents"
        );
        // The rest ends on its own...
        let st = ctx.accounts.get(&uuid).unwrap().unwrap().progress.arena;
        let now = jdr_core::clock::now();
        assert_eq!(st.hp_of(1, now + REST_SECS + 1), TEAM_HP_MAX);
        assert!(st.resting_until(1, now).is_some());
        // ...or the recovery refills it now.
        let r = call(
            &ctx,
            &uuid,
            "/pvp/recover_hp_pvp",
            json!({"typeId": 0, "itemId": 0, "eventCampaignId": CAMPAIGN_ID, "teamNum": 1}),
        )
        .unwrap();
        assert!(r["itemList"].is_array() && r["userInfo"].is_object());
        let h = call(&ctx, &uuid, "/pvp/home", json!({})).unwrap();
        assert_eq!(h["pvpTeamList"][0]["hp"], TEAM_HP_MAX);
        assert!(call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})).is_ok());
        // A team without a leader cannot fight.
        assert_eq!(
            call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 3})),
            Err(E_PVP_NO_LEADER)
        );
        // Fever: four battles filled the gauge.
        assert_eq!(
            call(&ctx, &uuid, "/pvp/fever_mode", json!({})).unwrap()["feverRate"],
            FEVER_RATE
        );
    }

    #[test]
    fn fever_triples_and_needs_a_full_gauge() {
        let (ctx, uuid) = setup("fever");
        assert_eq!(
            call(&ctx, &uuid, "/pvp/fever_mode", json!({})),
            Err(E_PVP_FEVER_LACK)
        );
        ctx.accounts
            .update(&uuid, |p| {
                p.progress.arena.fever_gauge = FEVER_GAUGE_MAX;
                ((), Change::Bookkeeping)
            })
            .unwrap();
        let f = call(&ctx, &uuid, "/pvp/fever_mode", json!({})).unwrap();
        assert_eq!(f["feverRate"], FEVER_RATE);
        assert_eq!(f["feverGauge"], 0);
        assert!(f["feverEndDate"].as_str().unwrap() > ctx.server_date().as_str());
        let ev = call(&ctx, &uuid, "/info/event_list", json!({})).unwrap();
        assert_eq!(ev["userBattleInfo"]["fieverEndTime"], f["feverEndDate"]);
        let l = call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})).unwrap();
        let uid = l["pvpTeamList"][1]["enemyuId"].as_i64().unwrap();
        call(
            &ctx,
            &uuid,
            "/pvp/pvp_start",
            json!({"teamNum": 1, "enemyuId": uid, "enemyTeamNum": 1}),
        )
        .unwrap();
        let e = call(
            &ctx,
            &uuid,
            "/pvp/pvp_end",
            json!({"result": 0, "teamNo": 1, "myTeamInfoList": [{"hp": 1}]}),
        )
        .unwrap();
        let base = e["getRankingPoint"].as_i64().unwrap()
            + e["totalHpBonusPoint"].as_i64().unwrap()
            + e["restStatueBonusPoint"].as_i64().unwrap();
        assert_eq!(e["getFeverRankingPoint"], base * 2);
        assert_eq!(e["feverRate"], FEVER_RATE);
    }

    #[test]
    fn point_rewards_and_season_end_reach_the_message_box() {
        let (ctx, uuid) = setup("rewards");
        ctx.accounts
            .update(&uuid, |p| {
                p.progress.arena.season = season_of(jdr_core::clock::now());
                p.progress.arena.points = 650;
                ((), Change::Bookkeeping)
            })
            .unwrap();
        let waiting = |ctx: &Ctx| {
            ctx.accounts
                .get(&uuid)
                .unwrap()
                .unwrap()
                .progress
                .presents
                .waiting()
        };
        call(&ctx, &uuid, "/pvp/home", json!({})).unwrap();
        let w0 = waiting(&ctx);
        let l = call(&ctx, &uuid, "/pvp/enemy_player_list", json!({"teamNo": 1})).unwrap();
        let uid = l["pvpTeamList"][0]["enemyuId"].as_i64().unwrap();
        call(
            &ctx,
            &uuid,
            "/pvp/pvp_start",
            json!({"teamNum": 1, "enemyuId": uid, "enemyTeamNum": 1}),
        )
        .unwrap();
        let e = call(
            &ctx,
            &uuid,
            "/pvp/pvp_end",
            json!({"result": 0, "teamNo": 1, "myTeamInfoList": [{"hp": 1}]}),
        )
        .unwrap();
        let got = e["eventRewardList"].as_array().unwrap();
        assert_eq!(got.len(), 3, "100, 300 and 600 points reached");
        assert_eq!(got[2]["necessaryEventPoint"], 600);
        assert!(got[0]["itemSetInfo"]["itemList"].is_array());
        assert_eq!(waiting(&ctx), w0 + 3);
        let pr = call(&ctx, &uuid, "/event/point_reward_list", json!({})).unwrap();
        let flags: Vec<i64> = pr["userPointRewardList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["getFlg"].as_i64().unwrap())
            .collect();
        assert_eq!(&flags[..4], &[1, 1, 1, 0]);
        let h = call(&ctx, &uuid, "/pvp/home", json!({})).unwrap();
        assert_eq!(h["pvpUserInfo"]["nextEventPoint"], 1_000);

        // The season turns: its reward is sent, points start again.
        ctx.accounts
            .update(&uuid, |p| {
                p.progress.arena.season -= 1;
                ((), Change::Bookkeeping)
            })
            .unwrap();
        let h = call(&ctx, &uuid, "/pvp/home", json!({})).unwrap();
        assert_eq!(h["pvpUserInfo"]["rankingPoint"], 0);
        assert_eq!(waiting(&ctx), w0 + 4);
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let last = p.progress.presents.list.last().unwrap();
        assert!(last.comment.starts_with("Arena season reward"));
        assert!(p.progress.arena.claimed.is_empty());
    }

    #[test]
    fn the_battle_formation_exists() {
        // The client starts the Arena battle with the formation `PatternPVP`.
        let js = crate::masterdata::TABLES
            .iter()
            .find(|(n, _)| *n == "area_position_pattern_table")
            .unwrap()
            .1;
        let v: Value = serde_json::from_str(js).unwrap();
        let row = v["data"]["paramList"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["patternName"] == "PatternPVP")
            .expect("PatternPVP row");
        assert_eq!(row["posSetList"].as_array().unwrap().len(), 6);
        assert_eq!(rank_rows().len(), RANKS.len(), "one team rank per letter");
    }

    #[test]
    fn season_roll_rules() {
        let mut st = ArenaState::default();
        let t = 1_800_000_000; // 2027-01-15
        assert_eq!(st.roll(t).1, None, "a first season has nothing to reward");
        st.points = 9_995;
        st.season_battles = 1;
        assert_eq!(st.roll(t + 3600).1, None, "same season");
        let next = st.roll(t + 8 * 86_400);
        assert_eq!(next, (true, Some(1)));
        assert_eq!(st.points, 0);
        st.season_battles = 0;
        assert_eq!(
            st.roll(t + 16 * 86_400).1,
            None,
            "no battles, no season reward"
        );
        assert_eq!(season_reward(1), (10, 500));
        assert_eq!(season_reward(5_000), (10, 50));
        assert_eq!(ranking(0), 1 + RANKING_TOP / RANKING_STEP);
        assert_eq!(ranking(RANKING_TOP * 2), 1);
        // Mondays at 04:00 JST start a season.
        let monday_4am_jst = 1_794_769_200; // 2026-11-16 04:00 JST, a Monday
        assert_eq!(season_of(monday_4am_jst), season_of(monday_4am_jst - 1) + 1);
        assert_eq!(battle_points(10, 0, 1), 15);
        assert_eq!(battle_points(10, 0, 2), 16);
        assert_eq!(battle_points(10, 99, 9), (10 + 35) * 150 / 100);
    }

    #[test]
    fn arena_formation_routes_edit_only_the_arena_roster() {
        let (ctx, uuid) = setup("team");
        let ti = call(&ctx, &uuid, "/pvp/team_info_pvp", json!({})).unwrap();
        assert_eq!(ti["pvpTeamList"].as_array().unwrap().len(), 5);
        assert!(ti["mindSeqInfo"]["seqInfoList"].is_array());
        let p = ctx.accounts.get(&uuid).unwrap().unwrap();
        let story = p.collection.teams.clone();
        let a1 = p.collection.arena_teams[0].slots;
        let free: Vec<i64> = p
            .collection
            .statues
            .iter()
            .map(|s| s.uid)
            .filter(|u| !a1.contains(u) && p.collection.known_statue(*u).is_some())
            .take(6)
            .collect();
        let slots = |r: &Map<String, Value>| -> [i64; 3] {
            let o = &r["pvpTeamInfo"]["teamOrder"];
            ["leaderStatue", "memberLeftStatue", "memberRightStatue"]
                .map(|k| o[k]["statueuId"].as_i64().unwrap())
        };
        // set_team_pvp, leader change, statue select.
        let r = call(
            &ctx,
            &uuid,
            "/pvp/set_team_pvp",
            json!({"teamNum": 2, "leaderstatueuId": free[0], "memberLeftStatueuId": free[1],
                   "memberRightStatueuId": free[1]}),
        )
        .unwrap();
        assert_eq!(slots(&r), [free[0], free[1], 0]);
        assert_eq!(r["pvpTeamInfo"]["hp"], TEAM_HP_MAX);
        let r = call(
            &ctx,
            &uuid,
            "/pvp/leader_change_pvp",
            json!({"teamNum": 2, "statueSeq": 2}),
        )
        .unwrap();
        assert_eq!(slots(&r), [free[1], free[0], 0]);
        let r = call(
            &ctx,
            &uuid,
            "/pvp/statue_select_pvp",
            json!({"teamNum": 2, "statueSeq": 3, "statueuId": free[2]}),
        )
        .unwrap();
        assert_eq!(slots(&r), [free[1], free[0], free[2]]);
        // Minds and assists.
        let mind = p.collection.minds.iter().map(|m| m.uid).next().unwrap();
        let r = call(
            &ctx,
            &uuid,
            "/pvp/equip_multiple_minds_pvp",
            json!({"teamNum": 2, "statueSeq": 1, "mindList": [{"equipSeq": 1, "minduId": mind}]}),
        )
        .unwrap();
        assert_eq!(
            r["pvpTeamInfo"]["teamOrder"]["leaderStatue"]["equipMindList"][0]["minduId"],
            mind
        );
        let r = call(
            &ctx,
            &uuid,
            "/pvp/equip_mind_pvp",
            json!({"teamNum": 2, "statueSeq": 1, "equipSeq": 1, "minduId": 0}),
        )
        .unwrap();
        assert_eq!(
            r["pvpTeamInfo"]["teamOrder"]["leaderStatue"]["equipMindList"],
            json!([])
        );
        assert_eq!(
            call(
                &ctx,
                &uuid,
                "/pvp/equip_multiple_minds_pvp",
                json!({"teamNum": 2, "statueSeq": 1, "mindList": [{"equipSeq": 9, "minduId": mind}]}),
            ),
            Err(51709),
            "locked slot"
        );
        let r = call(
            &ctx,
            &uuid,
            "/pvp/equip_multiple_assist_pvp",
            json!({"teamNum": 2, "statueSeq": 2, "statueList": [{"equipSeq": 1, "statueuId": free[3]}]}),
        )
        .unwrap();
        assert_eq!(
            r["pvpTeamInfo"]["teamOrder"]["memberLeftStatue"]["assistStatueList"][0]["statueuId"],
            free[3]
        );
        // Auto formation on team 3 leaves the statues of teams 1 and 2 alone.
        let r = call(
            &ctx,
            &uuid,
            "/pvp/auto_set_and_equip",
            json!({"teamNum": 3, "changePlan": 1}),
        )
        .unwrap();
        let auto = slots(&r);
        assert!(auto.iter().all(|u| *u != 0));
        let p2 = ctx.accounts.get(&uuid).unwrap().unwrap();
        for t in p2.collection.arena_teams.iter().filter(|t| t.team_num != 3) {
            assert!(t.slots.iter().all(|u| *u == 0 || !auto.contains(u)));
        }
        assert!(
            !r["pvpTeamInfo"]["teamOrder"]["leaderStatue"]["equipMindList"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        // set_team_all_pvp: a statue of an unlisted team leaves an empty seat; Minds come along.
        let in_arena: HashSet<i64> = p2
            .collection
            .arena_teams
            .iter()
            .flat_map(|t| t.slots)
            .collect();
        let spare: Vec<i64> = p2
            .collection
            .statues
            .iter()
            .map(|s| s.uid)
            .filter(|u| {
                !in_arena.contains(u) && *u != free[3] && p2.collection.known_statue(*u).is_some()
            })
            .take(2)
            .collect();
        let free = [free[0], free[1], free[2], free[3], spare[0], spare[1]];
        let all = call(
            &ctx,
            &uuid,
            "/pvp/set_team_all_pvp",
            json!({"teamList": [
                {"teamNum": 4,
                 "leaderStatueInfo": {"statueuId": free[4], "equipMindList": [{"equipSeq": 1, "minduId": mind}], "assistStatueList": []},
                 "memberLeftStatueInfo": {"statueuId": auto[0], "equipMindList": [], "assistStatueList": []},
                 "memberRightStatueInfo": {"statueuId": free[5], "equipMindList": [], "assistStatueList": [{"equipSeq": 1, "statueuId": free[3]}]}},
                {"teamNum": 5, "leaderStatueInfo": {"statueuId": free[4]}}
            ]}),
        )
        .unwrap();
        let teams = all["pvpTeamList"].as_array().unwrap();
        assert_eq!(teams.len(), 5);
        let o4 = &teams[3]["teamOrder"];
        assert_eq!(o4["leaderStatue"]["statueuId"], free[4]);
        assert_eq!(o4["memberLeftStatue"]["statueuId"], 0, "used by team 3");
        assert_eq!(o4["memberRightStatue"]["statueuId"], free[5]);
        assert_eq!(o4["leaderStatue"]["equipMindList"][0]["minduId"], mind);
        assert_eq!(
            o4["memberRightStatue"]["assistStatueList"][0]["statueuId"],
            free[3]
        );
        assert_eq!(
            teams[4]["teamOrder"]["leaderStatue"]["statueuId"], 0,
            "team 5's leader is team 4's: not saved"
        );
        // The story teams never moved; Arena Minds count as equipped.
        let p3 = ctx.accounts.get(&uuid).unwrap().unwrap();
        assert_eq!(p3.collection.teams, story);
        assert!(p3.collection.equipped_minds().contains(&mind));
        // Out-of-range team numbers are refused.
        let (r, _) = dispatch(
            &ctx,
            "/pvp/set_team_pvp",
            &env(&uuid, json!({"teamNum": 9, "leaderstatueuId": free[0]})),
        );
        assert!(r.is_err());
    }
}
