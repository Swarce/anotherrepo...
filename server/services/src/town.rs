//! The Town (the client's "clan"; docs/research/24-town.md), offline: one Town the player
//! belongs to at a time, whose other members are AI characters. The player creates a Town (and
//! leads it) or joins one of the AI Towns the recommend and search screens list. Saved with the
//! story progress (`Progress::town`).
//!
//! Client facts (H, 3.12.0): the Town level comes from `clan_table` (`exp` is the cumulative
//! total of a level); `clanFunctionList` holds the `ClanFunctionType` names the level unlocks;
//! Town Medals (item 41) are donated, at most 500 at a time, and the donation gift goes to the
//! Message Box (label `Clan_Foundation_10`); a background is a `clan_object_table` row whose
//! model is the map name. The object unlock ranks (`OBJECT_UNLOCKS`) are design borrowed from the
//! Diamond Records PC playtest. Everything else here (AI Towns, AI members, numbers, rewards) is
//! ours.

use crate::collection::{self, OwnedStatue};
use crate::services::ranking::{hash, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The player's own Town (ours: there is only one player).
pub const OWN_TOWN_ID: i64 = 1001;
/// AI Towns: ids `AI_TOWN_FIRST..AI_TOWN_FIRST + AI_TOWNS` (ours).
pub const AI_TOWN_FIRST: i64 = 2001;
pub const AI_TOWNS: i64 = 12;
/// Members per Town, the player included (research pack: "groups of up to fifteen").
pub const MEMBER_MAX: usize = 15;
/// Sub-Masters per Town (label `ClanSubMasterMax`).
pub const SUB_MASTER_MAX: usize = 3;
/// Town Medal (labels `ITEM_0041_NAME` / `_DETAIL`).
pub const MEDAL_ITEM: i64 = 41;
/// Town points per Town Medal (ours).
pub const MEDAL_POINTS: i64 = 10;
/// Medals per donation (client `MaxDonationAmount`, label `Clan_Foundation_11`).
pub const DONATION_MAX: i64 = 500;
/// Gold sent to the Message Box per Town Medal donated (ours).
pub const GIFT_GOLD_PER_MEDAL: i64 = 100;
/// Town Medals the Town sends once per game day, and once when the player first joins (ours).
pub const DAILY_MEDALS: i64 = 100;
pub const WELCOME_MEDALS: i64 = 300;
/// Seconds an AI Town's Master takes to approve the player's request (ours).
pub const APPROVAL_WAIT: i64 = 60;
/// Days of AI activity caught up at once (ours).
pub const CATCH_UP_DAYS: i64 = 7;

/// `ClanCommonUtility.ClanPosition` and `ClanStatus` (client).
pub const POS_APPLYING: i64 = -1;
pub const POS_MEMBER: i64 = 0;
pub const POS_SUB: i64 = 1;
pub const POS_LEADER: i64 = 2;
pub const ADD_NONE: i64 = 0;
pub const ADD_APPLYING: i64 = 1;
pub const ADD_MEMBER: i64 = 2;
/// `ClanApproval_02` is "Approval Required"; any other value joins at once (M: the dropdowns
/// count from 1, like the `ClanApproval_{0:00}` label).
pub const APPROVAL_REQUIRED: i64 = 2;

/// Levels that unlock the client's `ClanFunctionType`s (labels: object placement at Town Rank
/// 8, background at 10; Sub-Masters at 3 from the research pack's guide).
pub const FUNCTIONS: [(i64, &str); 3] = [
    (3, "TOWN_FUNCTION_SUBMASTER"),
    (8, "TOWN_FUNCTION_OBJECT_SET"),
    (10, "TOWN_FUNCTION_MAP_CHANGE"),
];

/// Background rows of `clan_object_table` (ids ours; maps from the player's files, names from
/// `ClanBGTitle_0001..0004`).
pub const BACKGROUNDS: [i64; 4] = [9001, 9002, 9003, 9004];
pub const DEFAULT_BACKGROUND: i64 = 9001;

/// The 20 Town objects with a model in the player's `clan_instance_object_00`, each with the
/// Town Rank that unlocks it, in unlock order. Ids and models are ours (the player's files); the
/// unlock order and ranks are design borrowed from the Diamond Records PC playtest (its trophy
/// list, matched to our ids by model). Rank 21 (`cl_roadroller`) is above our top level and
/// unlocks at `level_max()`. The 3.12.0 client has no per-object rank or cost
/// (`ClanObjectTable/Param` has none; `/clan/clan_object_list` is only ids), so objects stay free.
/// Objects below Rank 8 are owned but, by the client's rule, can only be placed from Rank 8
/// (`TOWN_FUNCTION_OBJECT_SET`, label `Clan_Object_Object_Unlock`); docs/research/24-town.md.
pub const OBJECT_UNLOCKS: [(i64, i64); 20] = [
    (52, 2),
    (42, 3),
    (27, 4),
    (8017, 5),
    (44, 6),
    (34, 7),
    (21, 8),
    (22, 9),
    (70, 10),
    (67, 11),
    (82, 12),
    (89, 13),
    (79, 14),
    (75, 15),
    (24, 16),
    (91, 17),
    (8019, 18),
    (8018, 19),
    (93, 20),
    (62, 21),
];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TownState {
    /// The Town the player belongs to.
    #[serde(default)]
    pub town: Option<Town>,
    /// The AI Town the player asked to join (approval required), with when.
    #[serde(default)]
    pub applying: Option<Applying>,
    /// The one-time welcome Town Medals were sent.
    #[serde(default)]
    pub welcomed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Applying {
    pub town: Town,
    pub since: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Town {
    pub id: i64,
    pub name: String,
    pub policy: i64,
    pub activity_time: i64,
    pub approval: i64,
    pub emblem: String,
    pub comment: String,
    /// User id of the Master (the player's, or an AI member's).
    pub leader: String,
    #[serde(default)]
    pub sub_masters: Vec<String>,
    /// Town points (cumulative; the level follows from `clan_table`).
    #[serde(default)]
    pub point: i64,
    /// The highest level whose reward was settled.
    #[serde(default)]
    pub rewarded_level: i64,
    #[serde(default)]
    pub background: i64,
    /// Town objects the Town owns, and where they stand (`[objectId, slotIndex]`).
    #[serde(default)]
    pub objects: Vec<i64>,
    #[serde(default)]
    pub placed: Vec<[i64; 2]>,
    #[serde(default)]
    pub members: Vec<AiMember>,
    /// AI players asking to join the player's Town.
    #[serde(default)]
    pub applicants: Vec<AiMember>,
    #[serde(default)]
    pub next_ai: i64,
    /// The last game day the AI members were simulated for.
    #[serde(default)]
    pub day: i64,
    /// The last game day the daily Town Medals went out.
    #[serde(default)]
    pub gift_day: i64,
    #[serde(default)]
    pub joined_at: i64,
    /// The player's donations, in Town points.
    #[serde(default)]
    pub my_donation: i64,
    #[serde(default)]
    pub chat: Vec<ChatLine>,
    #[serde(default)]
    pub seq: i64,
    #[serde(default)]
    pub read_seq: i64,
    /// The player's last position in the Town home (saved when the socket closes).
    #[serde(default)]
    pub my_pos: [f64; 3],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AiMember {
    pub id: String,
    pub name: String,
    pub statue: String,
    pub rank: i64,
    #[serde(default)]
    pub donation: i64,
    #[serde(default)]
    pub joined: i64,
    #[serde(default)]
    pub pos: [f64; 3],
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ChatLine {
    pub user: String,
    pub message: String,
    pub at: i64,
    pub seq: i64,
}

/// Chat lines kept (the client keeps 100, `ChatRecordMaxCount`).
pub const CHAT_KEEP: usize = 100;
/// Walked positions kept.
pub const WALKED_KEEP: usize = 32;

/// The player as the Town screens show them.
#[derive(Debug, Clone)]
pub struct Me {
    pub id: String,
    pub name: String,
    pub rank: i64,
    pub comment: String,
    pub last_login: i64,
    /// The main team leader's StatueInfo (or `{statueId: ""}`).
    pub statue: Value,
    pub statue_id: String,
    pub models: Vec<String>,
}

impl Me {
    pub fn of(p: &crate::accounts::Profile) -> Me {
        let leader = collection::main_team(&p.collection, 1)
            .and_then(|t| p.collection.known_statue(t.slots[0]))
            .or_else(|| p.collection.statues.first());
        let statue_id = leader.map(|o| o.statue_id.clone()).unwrap_or_default();
        Me {
            id: p.user_id.clone(),
            name: p.username.clone(),
            rank: p.rank.max(1),
            comment: p.comment.clone(),
            last_login: p.last_login_at,
            statue: leader
                .map(collection::statue_info)
                .unwrap_or_else(|| json!({ "statueId": "" })),
            models: collection::find(&statue_id)
                .map(|d| d.model_data_list.clone())
                .unwrap_or_default(),
            statue_id,
        }
    }
}

pub fn date(t: i64) -> String {
    let (y, mo, d, h, mi, s) = jdr_core::clock::civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

// ---------------------------------------------------------------------------------------------
// Levels

fn level_exps() -> &'static Vec<i64> {
    static CELL: std::sync::OnceLock<Vec<i64>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let v: Value =
            serde_json::from_str(include_str!("../../content/masterdata/clan_table.json"))
                .expect("clan_table.json");
        let mut rows: Vec<(i64, i64)> = v["data"]["paramList"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|r| {
                        (
                            r["level"].as_i64().unwrap_or(0),
                            r["exp"].as_i64().unwrap_or(0),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        rows.sort();
        rows.into_iter().map(|(_, e)| e).collect()
    })
}

pub fn level_max() -> i64 {
    level_exps().len().max(1) as i64
}

/// The total points a level needs.
pub fn level_total(level: i64) -> i64 {
    level_exps()
        .get((level.clamp(1, level_max()) - 1) as usize)
        .copied()
        .unwrap_or(0)
}

pub fn level_of(point: i64) -> i64 {
    let mut l = 1;
    for (i, e) in level_exps().iter().enumerate() {
        if point >= *e {
            l = i as i64 + 1;
        }
    }
    l
}

pub fn functions(level: i64) -> Vec<&'static str> {
    FUNCTIONS
        .iter()
        .filter(|(l, _)| level >= *l)
        .map(|(_, f)| *f)
        .collect()
}

/// The Town Rank that unlocks an object (`OBJECT_UNLOCKS`, capped at `level_max()`), if it is one.
pub fn object_rank(id: i64) -> Option<i64> {
    OBJECT_UNLOCKS
        .iter()
        .find(|(o, _)| *o == id)
        .map(|(_, r)| (*r).min(level_max()))
}

/// Objects a Town of this level owns, in unlock order (`OBJECT_UNLOCKS`).
pub fn objects_for(level: i64) -> Vec<i64> {
    OBJECT_UNLOCKS
        .iter()
        .filter(|(_, r)| (*r).min(level_max()) <= level)
        .map(|(o, _)| *o)
        .collect()
}

/// Backgrounds a Town can choose (ours: all four once it may change the map).
pub fn backgrounds_for(level: i64, current: i64) -> Vec<i64> {
    if functions(level).contains(&"TOWN_FUNCTION_MAP_CHANGE") {
        BACKGROUNDS.to_vec()
    } else {
        vec![if BACKGROUNDS.contains(&current) {
            current
        } else {
            DEFAULT_BACKGROUND
        }]
    }
}

// ---------------------------------------------------------------------------------------------
// AI players and Towns

const TOWN_WORDS_A: [&str; 8] = [
    "Golden", "Crazy", "Silver", "Star", "Purple", "Echo", "Sticky", "Stone",
];
const TOWN_WORDS_B: [&str; 8] = [
    "Wind",
    "Diamonds",
    "Platinum",
    "Crusaders",
    "Ocean",
    "Rain",
    "Hermits",
    "Heaven",
];

/// The statues AI members wear: every statue with a model.
fn statue_pool() -> Vec<&'static collection::StatueDef> {
    collection::statues()
        .iter()
        .filter(|d| !d.model_data_list.is_empty())
        .collect()
}

/// An AI player, stable for its id.
pub fn ai_member(id: i64, joined: i64) -> AiMember {
    let mut rng = Rng(hash(&format!("town-member-{id}")));
    let pool = statue_pool();
    let statue = pool
        .get(rng.below(pool.len()))
        .map(|d| d.statue_id.clone())
        .unwrap_or_default();
    let h = crate::services::ranking::HANDLES;
    AiMember {
        id: id.to_string(),
        name: format!("{} {}", h[rng.below(h.len())], 100 + rng.below(900)),
        statue,
        rank: 10 + rng.below(140) as i64,
        donation: 0,
        joined,
        pos: spot(&mut rng),
    }
}

/// A spot near the origin of the map (M: the maps' spawn points are unknown).
fn spot(rng: &mut Rng) -> [f64; 3] {
    let f = |r: &mut Rng| (r.below(201) as f64 - 100.0) / 100.0 * 1.5;
    [f(rng), 0.0, f(rng)]
}

/// One of the AI Towns, as it stands on game day `day`.
pub fn ai_town(id: i64, now: i64) -> Option<Town> {
    if !(AI_TOWN_FIRST..AI_TOWN_FIRST + AI_TOWNS).contains(&id) {
        return None;
    }
    let mut rng = Rng(hash(&format!("town-{id}")));
    let n = id - AI_TOWN_FIRST;
    let name = format!(
        "{} {}",
        TOWN_WORDS_A[(n as usize) % 8],
        TOWN_WORDS_B[(n as usize * 3 + 1) % 8]
    );
    let size = 4 + rng.below(10); // 4..13 AI members, so there is room for the player
    let base = 900_000_000 + id * 1000;
    let members: Vec<AiMember> = (1..=size as i64)
        .map(|i| ai_member(base + i, now - 86400 * (30 + rng.below(300) as i64)))
        .collect();
    let level = 2 + rng.below(11) as i64; // Town Rank 2..12
    let point = level_total(level) + rng.below(400) as i64;
    let background = if level >= 10 {
        BACKGROUNDS[rng.below(4)]
    } else {
        DEFAULT_BACKGROUND
    };
    let objects = objects_for(level_of(point));
    Some(Town {
        id,
        name,
        policy: 1 + rng.below(5) as i64,
        activity_time: 1 + rng.below(6) as i64,
        approval: 1 + rng.below(2) as i64,
        emblem: format!("ce_{:04}.bytes", rng.below(11)),
        comment: String::new(),
        leader: members[0].id.clone(),
        sub_masters: Vec::new(),
        point,
        rewarded_level: level_of(point),
        background,
        objects: objects.clone(),
        placed: Vec::new(),
        members,
        applicants: Vec::new(),
        next_ai: size as i64,
        day: crate::missions::game_day(now),
        gift_day: 0,
        joined_at: 0,
        my_donation: 0,
        chat: Vec::new(),
        seq: 0,
        read_seq: 0,
        my_pos: [0.0; 3],
    })
}

pub fn ai_towns(now: i64) -> Vec<Town> {
    (AI_TOWN_FIRST..AI_TOWN_FIRST + AI_TOWNS)
        .filter_map(|id| ai_town(id, now))
        .collect()
}

/// A new AI player for the player's own Town.
fn recruit(t: &mut Town, now: i64) -> AiMember {
    t.next_ai += 1;
    ai_member(950_000_000 + t.next_ai, now)
}

// ---------------------------------------------------------------------------------------------
// The player's Town

impl Town {
    pub fn level(&self) -> i64 {
        level_of(self.point)
    }

    /// The player's ClanPosition.
    pub fn position_of(&self, user: &str) -> i64 {
        if self.leader == user {
            POS_LEADER
        } else if self.sub_masters.iter().any(|s| s == user) {
            POS_SUB
        } else {
            POS_MEMBER
        }
    }

    pub fn member_count(&self) -> usize {
        self.members.len() + 1
    }

    pub fn is_full(&self) -> bool {
        self.member_count() >= MEMBER_MAX
    }

    pub fn member(&self, id: &str) -> Option<&AiMember> {
        self.members.iter().find(|m| m.id == id)
    }

    /// May the player manage members (approve, expel)?
    pub fn can_manage(&self, user: &str) -> bool {
        self.position_of(user) >= POS_SUB
    }

    /// The Town as the player creates it: they lead it alone; three AI players ask to join
    /// (and join at once when the Town is open).
    pub fn create(me: &Me, f: TownFields, now: i64) -> Town {
        let mut t = Town {
            id: OWN_TOWN_ID,
            name: f.name,
            policy: f.policy,
            activity_time: f.activity_time,
            approval: f.approval,
            emblem: f.emblem,
            comment: f.comment,
            leader: me.id.clone(),
            rewarded_level: 1,
            background: DEFAULT_BACKGROUND,
            day: crate::missions::game_day(now),
            joined_at: now,
            ..Default::default()
        };
        for _ in 0..3 {
            let m = recruit(&mut t, now);
            t.applicants.push(m);
        }
        t.admit_open(now);
        t
    }

    /// An open Town takes its applicants while there is room.
    fn admit_open(&mut self, now: i64) {
        if self.approval == APPROVAL_REQUIRED {
            return;
        }
        while !self.is_full() && !self.applicants.is_empty() {
            let mut m = self.applicants.remove(0);
            m.joined = now;
            self.members.push(m);
        }
    }

    pub fn approve(&mut self, id: &str, now: i64) -> Result<(), i64> {
        let i = self
            .applicants
            .iter()
            .position(|m| m.id == id)
            .ok_or(E_CLAN_NOT_REQUEST)?;
        if self.is_full() {
            return Err(E_CLAN_MEMBER_MAX);
        }
        let mut m = self.applicants.remove(i);
        m.joined = now;
        self.members.push(m);
        Ok(())
    }

    pub fn deny(&mut self, id: &str) -> Result<(), i64> {
        let i = self
            .applicants
            .iter()
            .position(|m| m.id == id)
            .ok_or(E_CLAN_NOT_REQUEST)?;
        self.applicants.remove(i);
        Ok(())
    }

    pub fn expel(&mut self, id: &str) -> Result<(), i64> {
        if id == self.leader {
            return Err(E_CLAN_LEADER);
        }
        let i = self
            .members
            .iter()
            .position(|m| m.id == id)
            .ok_or(E_CLAN_USER_NOT_FOUND)?;
        self.members.remove(i);
        self.sub_masters.retain(|s| s != id);
        Ok(())
    }

    /// Appoint, or dismiss when already one (the client sends the same route for both).
    pub fn toggle_sub_master(&mut self, id: &str) -> Result<(), i64> {
        if self.member(id).is_none() {
            return Err(E_CLAN_USER_NOT_FOUND);
        }
        if let Some(i) = self.sub_masters.iter().position(|s| s == id) {
            self.sub_masters.remove(i);
            return Ok(());
        }
        if !functions(self.level()).contains(&"TOWN_FUNCTION_SUBMASTER") {
            return Err(E_CLAN_NOT_AUTHORITY);
        }
        if self.sub_masters.len() >= SUB_MASTER_MAX {
            return Err(E_CLAN_SUBLEADER_NUM_MAX);
        }
        self.sub_masters.push(id.to_string());
        Ok(())
    }

    /// Town levels reached and not yet settled: their objects join the Town, and the player gets
    /// the level reward in the Message Box. Returns the objects that joined.
    pub fn settle(&mut self, presents: &mut crate::presents::PresentBox, now: i64) -> Vec<i64> {
        let mut got = Vec::new();
        let level = self.level();
        while self.rewarded_level < level {
            self.rewarded_level += 1;
            let l = self.rewarded_level;
            presents.add(
                crate::presents::DATA_STONE,
                10,
                level_reward(l),
                format!("Town Rank {l} reward"),
                now,
            );
        }
        for o in objects_for(level) {
            if !self.objects.contains(&o) {
                self.objects.push(o);
                got.push(o);
            }
        }
        got
    }

    /// The AI members' days since the last visit: donations, and AI players asking to join the
    /// player's own Town (ours: one or two a day while there is room).
    pub fn simulate(&mut self, me: &str, now: i64) {
        let today = crate::missions::game_day(now);
        if self.day == 0 {
            self.day = today;
        }
        let days = (today - self.day).clamp(0, CATCH_UP_DAYS);
        self.day = today;
        for d in 0..days {
            let mut rng = Rng(hash(&format!("town-day-{}-{}", self.id, today - d)));
            for m in &mut self.members {
                let medals = 5 + rng.below(26) as i64;
                m.donation += medals * MEDAL_POINTS;
                self.point += medals * MEDAL_POINTS;
            }
            self.point = self.point.min(level_total(level_max()));
            if self.leader == me {
                for _ in 0..1 + rng.below(2) {
                    if self.member_count() + self.applicants.len() < MEMBER_MAX {
                        let m = recruit(self, now);
                        self.applicants.push(m);
                    }
                }
                self.admit_open(now);
            }
        }
    }

    pub fn add_chat(&mut self, user: &str, message: &str, now: i64) -> ChatLine {
        self.seq += 1;
        let line = ChatLine {
            user: user.to_string(),
            message: message.chars().take(200).collect(),
            at: now,
            seq: self.seq,
        };
        self.chat.push(line.clone());
        if self.chat.len() > CHAT_KEEP {
            let cut = self.chat.len() - CHAT_KEEP;
            self.chat.drain(..cut);
        }
        line
    }

    pub fn unread_chat(&self) -> bool {
        self.chat.iter().any(|c| c.seq > self.read_seq)
    }
}

/// Diamond Coins for reaching a Town level (ours).
pub fn level_reward(level: i64) -> i64 {
    10 * level
}

/// Fields of `CreateClanRequest` / `ChangeClanInfoRequest`.
#[derive(Debug, Clone, Default)]
pub struct TownFields {
    pub name: String,
    pub policy: i64,
    pub activity_time: i64,
    pub approval: i64,
    pub emblem: String,
    pub comment: String,
}

impl TownFields {
    pub fn from_param(p: &Value) -> TownFields {
        let s = |k: &str| p.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let i = |k: &str| p.get(k).and_then(Value::as_i64).unwrap_or(0);
        TownFields {
            name: s("clanName").chars().take(24).collect(),
            policy: i("policy"),
            activity_time: i("activityTime"),
            approval: i("approval"),
            emblem: s("emblem"),
            comment: s("comment").chars().take(200).collect(),
        }
    }
}

/// Client `ResponseParameterCode` statuses for Towns.
pub const E_CLAN_AFFILIATION: i64 = 50101;
pub const E_CLAN_NOT_AFFILIATION: i64 = 50102;
pub const E_CLAN_LEADER: i64 = 50103;
pub const E_CLAN_NOT_AUTHORITY: i64 = 50104;
pub const E_CLAN_NOT_EXIST: i64 = 50111;
pub const E_CLAN_NOT_REQUEST: i64 = 50112;
pub const E_CLAN_MEMBER_MAX: i64 = 50113;
pub const E_CLAN_USER_NOT_FOUND: i64 = 50114;
pub const E_CLAN_SUBLEADER_NUM_MAX: i64 = 50117;
pub const E_CLAN_NOT_DONATION_ITEM: i64 = 50118;
pub const E_CLAN_LEVEL_MAX: i64 = 50119;

// ---------------------------------------------------------------------------------------------
// State changes that are not tied to one route

impl TownState {
    /// Turn the clock: an AI Town approves the player's request after `APPROVAL_WAIT`; the
    /// Town's AI days; level rewards; the daily Town Medals. Returns whether anything changed.
    pub fn tick(&mut self, me: &str, presents: &mut crate::presents::PresentBox, now: i64) -> bool {
        let before = self.clone();
        if let Some(a) = &self.applying {
            if now - a.since >= APPROVAL_WAIT {
                let t = a.town.clone();
                self.applying = None;
                if !t.is_full() {
                    self.join(t, presents, now);
                }
            }
        }
        if let Some(t) = &mut self.town {
            t.simulate(me, now);
            t.settle(presents, now);
            let today = crate::missions::game_day(now);
            if t.gift_day < today {
                t.gift_day = today;
                presents.add(
                    crate::presents::DATA_ITEM,
                    MEDAL_ITEM,
                    DAILY_MEDALS,
                    "Town Medals from your Town".into(),
                    now,
                );
            }
        }
        *self != before
    }

    /// Join a Town (created or AI): the first time, the welcome Town Medals.
    pub fn join(&mut self, mut t: Town, presents: &mut crate::presents::PresentBox, now: i64) {
        t.joined_at = now;
        t.rewarded_level = t.level();
        t.day = crate::missions::game_day(now);
        t.gift_day = t.day;
        self.applying = None;
        self.town = Some(t);
        if !self.welcomed {
            self.welcomed = true;
            presents.add(
                crate::presents::DATA_ITEM,
                MEDAL_ITEM,
                WELCOME_MEDALS,
                "Welcome to your Town".into(),
                now,
            );
        }
    }

    /// The `user_info` Town fields: clanId, clanName, position, addStatus.
    pub fn user_fields(&self, me: &str) -> (i64, String, i64, i64) {
        if let Some(t) = &self.town {
            (t.id, t.name.clone(), t.position_of(me), ADD_MEMBER)
        } else if let Some(a) = &self.applying {
            (a.town.id, a.town.name.clone(), POS_APPLYING, ADD_APPLYING)
        } else {
            (0, String::new(), POS_MEMBER, ADD_NONE)
        }
    }

    pub fn background(&self) -> i64 {
        self.town.as_ref().map(|t| t.background).unwrap_or(0)
    }

    /// A Town by id: the player's own, the one applied to, or an AI Town.
    pub fn find(&self, id: i64, now: i64) -> Option<Town> {
        if let Some(t) = self.town.as_ref().filter(|t| t.id == id) {
            return Some(t.clone());
        }
        if let Some(a) = self.applying.as_ref().filter(|a| a.town.id == id) {
            return Some(a.town.clone());
        }
        ai_town(id, now)
    }
}

// ---------------------------------------------------------------------------------------------
// Client shapes

fn ai_statue(m: &AiMember) -> Value {
    let level = (m.rank / 2).clamp(1, collection::MAX_LEVEL);
    collection::statue_info(&OwnedStatue {
        uid: m.id.parse().unwrap_or(0),
        statue_id: m.statue.clone(),
        level,
        exp: collection::exp_for_level(level),
        ..Default::default()
    })
}

/// Minutes since an AI member's last login (ours, stable per member and hour).
fn ai_idle_minutes(m: &AiMember, now: i64) -> i64 {
    let mut rng = Rng(hash(&format!("idle-{}-{}", m.id, now / 3600)));
    rng.below(600) as i64
}

/// `UserInfoClan` of an AI member.
pub fn ai_user_info(t: &Town, m: &AiMember, now: i64) -> Value {
    json!({
        "id": m.id, "username": m.name, "userRank": m.rank, "clanId": t.id, "clanName": t.name,
        "clanLevel": t.level().to_string(), "position": t.position_of(&m.id),
        "clanDonationAmount": m.donation, "comment": "",
        "lastLoginAt": date(now - 60 * ai_idle_minutes(m, now)), "statueInfo": ai_statue(m)
    })
}

/// `UserInfoClan` of the player.
pub fn my_user_info(t: Option<&Town>, me: &Me) -> Value {
    json!({
        "id": me.id, "username": me.name, "userRank": me.rank,
        "clanId": t.map(|t| t.id).unwrap_or(0), "clanName": t.map(|t| t.name.clone()).unwrap_or_default(),
        "clanLevel": t.map(|t| t.level()).unwrap_or(0).to_string(),
        "position": t.map(|t| t.position_of(&me.id)).unwrap_or(POS_MEMBER),
        "clanDonationAmount": t.map(|t| t.my_donation).unwrap_or(0), "comment": me.comment,
        "lastLoginAt": date(me.last_login), "statueInfo": me.statue
    })
}

/// `ClanInfo`; `me` is set when the player is a member. Only a member's view lists the members
/// (the Town home and its edit screen read the list; the entrance's info window reads only the
/// counts and names, ClanCommonUtility.SetupClanInfoWindow 0xF56D90), which keeps the search
/// reply small.
pub fn clan_info(t: &Town, me: Option<&Me>, now: i64) -> Value {
    let mut list = Vec::new();
    if let Some(me) = me {
        list.push(json!({ "clanUserInfo": my_user_info(Some(t), me), "date": date(t.joined_at) }));
        for m in &t.members {
            list.push(json!({ "clanUserInfo": ai_user_info(t, m, now), "date": date(m.joined) }));
        }
    }
    let leader = if me.is_some_and(|me| me.id == t.leader) {
        me.map(|me| me.name.clone()).unwrap_or_default()
    } else {
        t.member(&t.leader)
            .map(|m| m.name.clone())
            .unwrap_or_default()
    };
    let level = t.level();
    json!({
        "clanId": t.id, "clanName": t.name,
        "clanMemberNum": t.members.len() + usize::from(me.is_some()), "clanMemberMax": MEMBER_MAX,
        "clanLevel": level, "clanLevelMax": level_max(), "clanPoint": t.point,
        "policy": t.policy, "activityTime": t.activity_time, "approval": t.approval,
        "emblem": t.emblem, "comment": t.comment, "clanLeaderName": leader,
        "clanMemberList": list,
        "eventInfo": { "eventCampaignName": "", "clanEventPoint": 0 },
        "clanFunctionList": functions(level), "backgroundId": t.background
    })
}

/// `UserInfoClan` rows of the player's applicants.
pub fn request_list(t: &Town, now: i64) -> Vec<Value> {
    t.applicants
        .iter()
        .map(|m| {
            let mut v = ai_user_info(t, m, now);
            v["clanId"] = json!(0);
            v["clanName"] = json!("");
            v["clanLevel"] = json!("0");
            v["position"] = json!(POS_APPLYING);
            v
        })
        .collect()
}

/// `ClanObjectInfo` for an ItemSetInfo (labels by key).
pub fn object_info(id: i64) -> Value {
    json!({
        "clanObjectId": id.to_string(), "clanObjectModel": object_model(id),
        "clanObjectAbility": "", "clanObjectTitle": format!("ClanObjectTitle_{id:04}"),
        "clanObjectText": format!("ClanObjectText_{id:04}")
    })
}

/// The model of a Town object (from the player's `clan_instance_object_00`, matched by title;
/// M), or "".
pub fn object_model(id: i64) -> &'static str {
    match id {
        21 => "cl_fmegacar_green",
        22 => "cl_fmegacar_red",
        24 => "cl_stonemask",
        27 => "cl_frog",
        34 => "cl_doll_noriaki",
        42 => "cl_roadsign",
        44 => "cl_turtle",
        52 => "cl_paperbag_sige",
        62 => "cl_roadroller",
        67 => "cl_jojo_paint",
        70 => "cl_tvgame",
        75 => "cl_anjero_rock",
        79 => "cl_surface",
        82 => "cl_echoesskin_a",
        89 => "cl_catgrass",
        91 => "cl_bike",
        93 => "cl_ambulance",
        8017 => "cl_scoop",
        8018 => "cl_lion",
        8019 => "cl_zip_pillar",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> Me {
        Me {
            id: "100000001".into(),
            name: "Me".into(),
            rank: 10,
            comment: String::new(),
            last_login: 0,
            statue: json!({ "statueId": "" }),
            statue_id: String::new(),
            models: Vec::new(),
        }
    }

    #[test]
    fn levels_follow_the_table() {
        assert_eq!(level_total(1), 0);
        assert_eq!(level_of(0), 1);
        assert_eq!(level_of(level_total(8)), 8);
        assert_eq!(level_of(level_total(8) - 1), 7);
        assert_eq!(level_of(i64::MAX), level_max());
        assert_eq!(functions(2), Vec::<&str>::new());
        assert_eq!(functions(10).len(), 3);
        assert_eq!(objects_for(1), Vec::<i64>::new());
        assert_eq!(objects_for(2), vec![52]);
        // Below Rank 8 objects are owned but not placeable (the client's rule is kept).
        assert_eq!(objects_for(7).len(), 6);
        assert!(!functions(7).contains(&"TOWN_FUNCTION_OBJECT_SET"));
        assert!(functions(8).contains(&"TOWN_FUNCTION_OBJECT_SET"));
        // Rank 8 (placement unlocks): the seven objects of ranks 2..8, in unlock order.
        assert_eq!(objects_for(8), vec![52, 42, 27, 8017, 44, 34, 21]);
        assert_eq!(objects_for(level_max() - 1).len(), OBJECT_UNLOCKS.len() - 2);
        // Rank 21 is capped at the top level: every object by then.
        assert_eq!(object_rank(62), Some(level_max()));
        assert_eq!(object_rank(1), None);
        let all = objects_for(level_max());
        assert_eq!(all.len(), OBJECT_UNLOCKS.len());
        assert_eq!(objects_for(level_max() + 5), all);
        let mut ids = all.clone();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len(), "unique ids");
        let mut prev = 0;
        for (o, r) in OBJECT_UNLOCKS {
            assert!(!object_model(o).is_empty(), "{o}");
            assert!(r > prev, "one object per rank, ascending: {o}");
            prev = r;
            let rank = object_rank(o).unwrap();
            assert!(objects_for(rank).contains(&o), "{o}");
            assert!(!objects_for(rank - 1).contains(&o), "{o}");
        }
    }

    #[test]
    fn ai_towns_are_stable_and_have_room() {
        let a = ai_towns(1_800_000_000);
        let b = ai_towns(1_800_000_000);
        assert_eq!(a.len(), AI_TOWNS as usize);
        assert_eq!(a, b);
        for t in &a {
            assert!(t.member_count() < MEMBER_MAX, "{}", t.name);
            assert!(t.member(&t.leader).is_some());
            assert!(t.emblem.starts_with("ce_00"));
            assert!((1..=5).contains(&t.policy));
        }
    }

    #[test]
    fn own_town_recruits_and_manages() {
        let now = 1_800_000_000;
        let me = me();
        let f = TownFields {
            name: "Mine".into(),
            approval: APPROVAL_REQUIRED,
            policy: 1,
            activity_time: 1,
            ..Default::default()
        };
        let mut t = Town::create(&me, f, now);
        assert_eq!(t.position_of(&me.id), POS_LEADER);
        assert_eq!(t.applicants.len(), 3);
        let id = t.applicants[0].id.clone();
        t.approve(&id, now).unwrap();
        assert_eq!(t.members.len(), 1);
        assert_eq!(t.toggle_sub_master(&id), Err(E_CLAN_NOT_AUTHORITY));
        t.point = level_total(3);
        t.toggle_sub_master(&id).unwrap();
        assert_eq!(t.position_of(&id), POS_SUB);
        t.expel(&id).unwrap();
        assert!(t.sub_masters.is_empty());
        let other = t.applicants[0].id.clone();
        t.deny(&other).unwrap();
        assert_eq!(t.deny(&other), Err(E_CLAN_NOT_REQUEST));
        // An open Town takes applicants at once.
        let open = Town::create(
            &me,
            TownFields {
                approval: 1,
                ..Default::default()
            },
            now,
        );
        assert_eq!(open.members.len(), 3);
        assert!(open.applicants.is_empty());
    }

    #[test]
    fn days_bring_donations_rewards_and_medals() {
        let now = 1_800_000_000;
        let me = me();
        let mut st = TownState::default();
        let mut pb = crate::presents::PresentBox::default();
        let t = Town::create(
            &me,
            TownFields {
                approval: 1,
                ..Default::default()
            },
            now,
        );
        st.join(t, &mut pb, now);
        assert_eq!(pb.list.len(), 1, "welcome medals");
        assert!(!st.tick(&me.id, &mut pb, now));
        let later = now + 3 * 86400;
        assert!(st.tick(&me.id, &mut pb, later));
        let t = st.town.as_ref().unwrap();
        assert!(t.point > 0);
        // The first three were members on day one; recruits join later.
        assert!(t.members[..3].iter().all(|m| m.donation > 0));
        assert!(pb
            .list
            .iter()
            .any(|p| p.data_id == MEDAL_ITEM && p.count == DAILY_MEDALS));
        // Level rewards and objects.
        let t = st.town.as_mut().unwrap();
        t.point = t.point.max(level_total(4));
        t.settle(&mut pb, later);
        assert_eq!(t.objects, objects_for(t.level()));
        assert!(t.objects.len() >= 3);
        assert!(t.objects.starts_with(&[52, 42, 27]));
        assert!(pb.list.iter().any(|p| p.comment == "Town Rank 4 reward"));
    }

    #[test]
    fn applying_is_approved_after_a_wait() {
        let now = 1_800_000_000;
        let mut st = TownState::default();
        let mut pb = crate::presents::PresentBox::default();
        let t = ai_town(AI_TOWN_FIRST, now).unwrap();
        st.applying = Some(Applying {
            town: t,
            since: now,
        });
        assert_eq!(st.user_fields("1").3, ADD_APPLYING);
        st.tick("1", &mut pb, now + 10);
        assert!(st.town.is_none());
        st.tick("1", &mut pb, now + APPROVAL_WAIT);
        assert_eq!(
            st.user_fields("1"),
            (
                AI_TOWN_FIRST,
                st.town.as_ref().unwrap().name.clone(),
                POS_MEMBER,
                ADD_MEMBER
            )
        );
    }
}
