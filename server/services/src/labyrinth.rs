//! The Labyrinth of the Mind (`/dungeon/*`, docs/research/22-labyrinth.md): content, run state and
//! the rules the server keeps. The routes are in `services/dungeon.rs`; battles go through
//! `bizarre_start`/`bizarre_end` like story stages.
//!
//! Client facts (3.12.0, docs/research/22 §1): a run has no "start" route, it begins at
//! `stage_list`; the dungeon in progress is the first one with `floorNum >= 0`; a dungeon whose
//! `floorNum` is past `floorMax` is cleared; 3 Souls and a Mentality cap of 300 (GameConst);
//! a TarotInfo shows `name`/`description` labels and card sprite `imgNo` 0..21.
//!
//! Research pack facts (`v2.labyrinth.*`): 20 floors; room costs 22–35 Mentality; rewards earned
//! per floor are paid only on a full clear or an escape; a defeat or a retire costs a Soul and
//! losing every Soul forfeits the pending rewards; escaping restores the Souls; 22 Tarot
//! identities, three offered per room, each used once until the deck resets (after all 22, an
//! escape or losing every Soul); reversing the offer costs 20 Mentality.
//!
//! Wiki facts (Build 43, research-b43, `quests_extra.json` `labyrinth`, docs/research/22 §2.1): room
//! costs by room label from the Bahamut floor screenshots (2018-04) and their cost-by-slot pattern
//! from floor 5 on; the ShoutWiki floor rewards of floors 1-4.
//!
//! Ours: the three dungeons (the game's Beginner/Intermediate/Advanced names of one mansion) and
//! their rooms (our quest data has no labyrinth stages, so rooms reuse tactical story stages picked
//! by floor, stronger in the harder dungeons), the costs of rooms no wiki covers (floors 1-4), the
//! rewards of floors 5-20, the Tarot effects mapped onto existing affects, and the Mentality refill
//! (full at the 04:00 JST day roll).

use crate::missions::Reward;
use crate::quests::{self, ChapterDef, StageDef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One labyrinth of the dungeon list (docs/research/22 §3.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dungeon {
    /// `DUnn`; its rooms are `STAGE_DUnn_<floor:02>_<room:02>`.
    pub id: &'static str,
    /// Name label: the game's `CHAPTER_NAME_D_01..03` ("Beginner/Intermediate/Advanced: The
    /// Hidden and Sealed Mansion", DungeonName_JP).
    pub name: &'static str,
    /// Summary label (`DUNGEON_MES_001..003`, all "Explore the Mind Labyrinth.").
    pub summary: &'static str,
    /// `Difficluty` E 0 … SSS+ 10 (client enum; the grade is ours).
    pub difficulty: i64,
    /// Levels added to the room's story-stage enemies (ours; capped at Lv99).
    pub level_add: i64,
    /// Floor rewards are multiplied by this (ours).
    pub reward_mult: i64,
}

/// The three dungeons: the game names one mansion in three grades (`CHAPTER_NAME_D_01..03`); its
/// room labels `STAGE_DU01_<floor>_<room>` (floors 1–20) name the rooms of all three (ours: which
/// labels the original paired, the grades and their strength).
pub const DUNGEONS: [Dungeon; 3] = [
    Dungeon {
        id: "DU01",
        name: "CHAPTER_NAME_D_01",
        summary: "DUNGEON_MES_001",
        difficulty: 2, // C
        level_add: 0,
        reward_mult: 1,
    },
    Dungeon {
        id: "DU02",
        name: "CHAPTER_NAME_D_02",
        summary: "DUNGEON_MES_002",
        difficulty: 4, // A
        level_add: 15,
        reward_mult: 2,
    },
    Dungeon {
        id: "DU03",
        name: "CHAPTER_NAME_D_03",
        summary: "DUNGEON_MES_003",
        difficulty: 7, // SS
        level_add: 30,
        reward_mult: 3,
    },
];
/// The Beginner dungeon (the one Build 23 served; an empty `dungeonId` is taken as it).
pub const DUNGEON_ID: &str = "DU01";
/// `bizarre_dungeon_002.png` (bundles 000–004 exist): the mansion's picture for all three (ours).
const DUNGEON_IMG: i64 = 2;
/// The room name labels the client has (`DungeonStageName_JP`): the Beginner dungeon's.
const ROOM_LABEL_PREFIX: &str = "STAGE_DU01_";

/// The dungeon behind an id.
pub fn dungeon(id: &str) -> Option<&'static Dungeon> {
    DUNGEONS.iter().find(|d| d.id == id)
}
/// Floors (research pack, 20 shown on every screenshot).
pub const FLOORS: i64 = 20;
/// GameConst DUNGEON_SOUL_MAX (client).
pub const SOUL_MAX: i64 = 3;
/// GameConst DUNGEON_MENTARITY_MAX (client display clamp).
pub const MENTALITY_MAX: i64 = 300;
/// Reversing the offer: the client's texts say 20 (GameConst says 30 but only feeds an unused
/// placeholder), and so does the research pack.
pub const FLIP_COST: i64 = 20;
/// Tarot identities (`TAROT_NAME_000..021`; card sprites 0..21).
pub const TAROT_CARDS: usize = 22;
/// Cards offered per room (client shows up to 3; pack).
pub const OFFER_SIZE: usize = 3;
/// Client error codes.
pub const E_STARTED_OTHER_DUNGEON: i64 = 52101;
pub const E_LACK_MENTALITY: i64 = 52102;
/// A request that does not fit the run (client shows ERROR_TEXT_00600001).
pub const E_INVALID: i64 = 10103;

/// One room of a floor: a tactical story stage under the room's own id and name label.
#[derive(Debug, Clone)]
pub struct Room {
    /// `STAGE_DUnn_<floor:02>_<room:02>`: the stage id the client sends back.
    pub id: String,
    /// `STAGE_DU01_<floor:02>_<room:02>`: the name label the room banner shows.
    pub label: String,
    pub dungeon: &'static Dungeon,
    pub floor: i64,
    pub index: usize,
    pub cost: i64,
    pub chapter: &'static ChapterDef,
    /// The story stage, its enemies raised by the dungeon's `level_add`.
    pub stage: StageDef,
}

/// Tactical story stages, easiest first (by enemy level, then id).
fn pool() -> &'static [(&'static ChapterDef, &'static StageDef)] {
    static CELL: std::sync::OnceLock<Vec<(&'static ChapterDef, &'static StageDef)>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let mut v: Vec<_> = quests::chapters()
            .iter()
            .filter(|c| c.stage_type == 1 && c.category == quests::STORY_CATEGORY)
            .flat_map(|c| c.stages.iter().map(move |s| (c, s)))
            .collect();
        v.sort_by(|a, b| (a.1.enemy_level, &a.1.stage_id).cmp(&(b.1.enemy_level, &b.1.stage_id)));
        v
    })
}

/// Rooms on a floor (ours): one on floors 10 and 20 (the game's labels also have a single room
/// there), three elsewhere (the labels have 3–6; screenshots show at least two).
pub fn rooms_on(floor: i64) -> usize {
    if floor % 10 == 0 {
        1
    } else {
        3
    }
}

/// Mentality per room: the wiki's cost for the room's label where one is known (Build 43: the
/// rooms on the Bahamut screenshots, and rooms 01-03 from floor 5 on by the cost-by-slot pattern
/// 35/27/25; both single rooms 30). Otherwise ours, inside the pack's 22–35: 22 rising to 31 by
/// floor, +3 per room down the list (floors 1-4 only, in practice).
pub fn room_cost(floor: i64, index: usize) -> i64 {
    let label = format!("{ROOM_LABEL_PREFIX}{floor:02}_{:02}", index + 1);
    if let Some(r) = quests::labyrinth().rooms.iter().find(|r| r.label == label) {
        return r.use_mentality;
    }
    if rooms_on(floor) == 1 {
        25 + floor / 2
    } else {
        (22 + (floor - 1) * 10 / 19 + 3 * index as i64).min(35)
    }
}

/// The rooms of a floor (1..=FLOORS) of a dungeon. Each floor takes its twentieth of the stage
/// pool, so the enemies get stronger floor by floor; a single room takes the hardest stage of its
/// slice. The harder dungeons raise the enemies' level.
pub fn rooms_in(d: &'static Dungeon, floor: i64) -> Vec<Room> {
    let pool = pool();
    if pool.is_empty() || !(1..=FLOORS).contains(&floor) {
        return Vec::new();
    }
    let n = pool.len();
    let span = (n / FLOORS as usize).max(1);
    let base = (floor as usize - 1) * n / FLOORS as usize;
    let k = rooms_on(floor);
    (0..k)
        .map(|r| {
            let off = if k == 1 {
                span - 1
            } else {
                r * span / k + span / (2 * k)
            };
            let (chapter, stage) = pool[(base + off).min(n - 1)];
            let mut stage = stage.clone();
            stage.enemy_level = (stage.enemy_level + d.level_add).min(crate::collection::MAX_LEVEL);
            Room {
                id: format!("STAGE_{}_{floor:02}_{:02}", d.id, r + 1),
                label: format!("{ROOM_LABEL_PREFIX}{floor:02}_{:02}", r + 1),
                dungeon: d,
                floor,
                index: r,
                cost: room_cost(floor, r),
                chapter,
                stage,
            }
        })
        .collect()
}

/// The rooms of a floor of the Beginner dungeon.
pub fn rooms(floor: i64) -> Vec<Room> {
    rooms_in(&DUNGEONS[0], floor)
}

/// The room behind a room id (`STAGE_DUnn_ff_rr`), if it is one.
pub fn find_room(id: &str) -> Option<Room> {
    let rest = id.strip_prefix("STAGE_")?;
    let (d, rest) = rest.split_once('_')?;
    let d = dungeon(d)?;
    let (f, r) = rest.split_once('_')?;
    let (f, r): (i64, usize) = (f.parse().ok()?, r.parse().ok()?);
    rooms_in(d, f).into_iter().nth(r.checked_sub(1)?)
}

/// `StageInfo` of a room: the stage's battle data under the room's id and name label, its
/// Mentality cost, and the floor's reward as the "possible rewards" row (shown while clearFlg is
/// not 1). Star missions start unlit and play counts at 0: room progress is not story progress.
pub fn room_stage_info(r: &Room) -> Value {
    let mut v = quests::stage_info(r.chapter, &r.stage, r.index, &quests::Progress::default());
    let reward = floor_reward(r.dungeon, r.floor);
    let (data_type, data_id) = crate::presents::reward_kind(reward.item_id);
    for (k, x) in [
        ("stageId", json!(r.id)),
        ("name", json!(r.label)),
        ("text", json!(r.label)),
        ("useMentality", json!(r.cost)),
        ("useStamina", json!(0)),
        ("nextStageIdList", json!([])),
        // The stage gold is not paid in the labyrinth: the floor reward is.
        ("steadyItemSet", json!([])),
        (
            "rewardItemSet",
            json!([{ "dataType": data_type, "dataId": data_id.to_string(),
                     "name": format!("ITEM_{:04}_NAME", reward.item_id), "count": reward.count }]),
        ),
    ] {
        v[k] = x;
    }
    v
}

/// The reward a floor earns, one item per floor so any list row shows it: the wiki's for floors
/// 1-4 (Build 43, ShoutWiki: Blue, Green and Red Stone ×30, a Green Brooch whose count is ours);
/// ours on the others: Memory Rings every fifth floor, gold on the other odd floors, trophies
/// (enhancement material for any statue, 6112–6115 by tier) on the other even floors. Times the
/// dungeon's `reward_mult` (ours). Item ids as missions give them (20 = gold).
pub fn floor_reward(d: &Dungeon, floor: i64) -> Reward {
    let r = base_floor_reward(floor);
    Reward {
        item_id: r.item_id,
        count: r.count * d.reward_mult,
    }
}

fn base_floor_reward(floor: i64) -> Reward {
    if let Some(r) = quests::labyrinth()
        .floor_rewards
        .iter()
        .find(|r| r.floor == floor)
    {
        return Reward {
            item_id: r.item_id,
            count: r.count,
        };
    }
    let (item_id, count) = match floor {
        f if f % 5 == 0 => (crate::items::MEMORY_RING, 100 * (f / 5)),
        f if f % 2 == 1 => (20, 2_000 + 1_000 * f),
        f if f < 5 => (6112, 5),
        f if f < 10 => (6113, 5),
        f if f < 15 => (6114, 3),
        _ => (6115, 2),
    };
    Reward { item_id, count }
}

/// A Tarot card as dealt: identity 0..21 and orientation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Card {
    pub id: usize,
    #[serde(default)]
    pub reversed: bool,
}

pub fn tarot_id(id: usize) -> String {
    format!("T{id:03}")
}

pub fn parse_tarot_id(s: &str) -> Option<usize> {
    s.strip_prefix('T')?
        .parse()
        .ok()
        .filter(|n| *n < TAROT_CARDS)
}

/// A card effect: (affect id, per-mille amount, Jjs.StatueType for the *_STYPE_* affects).
type Effect = (&'static str, i64, i64);

/// Upright effects: the client's card texts (3.12.0) where an affect in our affect_table does the
/// same thing; `None` where none does (evasion, speed, accuracy, crit rate, attribute and HP
/// threshold conditions). Damage rates are per mille, positive = more damage, on both sides
/// (docs/research/12 §4). Statue types: 1 Courage, 2 Wisdom, 3 Spirit, 4 Unity, 5 Solitude.
const UPRIGHT: [Option<Effect>; TAROT_CARDS] = [
    None,                                               // 000 Fool: evasion +40
    Some(("DTAF_INTELLI_STYPE_DMG_RATEADD", 1000, 2)),  // 001 vs Wisdom +100%
    None,                                               // 002 vs Vampires
    None,                                               // 003 vs Pillar Men
    None,                                               // 004 vs Major Arcana
    None,                                               // 005 vs Stand Users
    Some(("DTAF_UNITY_STYPE_DMG_RATEADD", 1000, 4)),    // 006 vs Unity +100%
    None,                                               // 007 speed +50%
    Some(("DTAF_TENACITY_STYPE_DMG_RATEADD", 1000, 3)), // 008 vs Spirit +100%
    Some(("DTAF_BRAVE_STYPE_DMG_RATEADD", 1000, 1)),    // 009 vs Courage +100%
    None,                                               // 010 crit rate +20
    // 011 Strength: the client has only the reversed text (ATK -50%); upright ATK +50% is the
    // research pack's upright value.
    Some(("DTAF_ATK_RATEADD", 500, 0)),
    Some(("DTAF_MAVERICK_STYPE_VCT_DAM_RATESUB", 1000, 5)), // 012 taken from Solitude +100%
    Some(("DTAF_VCT_DAM_RATESUB", 500, 0)),                 // 013 damage taken +50%
    None,                                                   // 014 vs Hamon users
    None,                                                   // 015 accuracy -40
    Some(("DTAF_DEF_RATEADD", -500, 0)),                    // 016 DEF -50%
    None,                                                   // 017 vs Joestar family
    None,                                                   // 018 HP <= 50%: taken +100%
    None,                                                   // 019 HP >= 50%: dealt +100%
    Some(("DTAF_DMG_RATEADD", 500, 0)),                     // 020 damage dealt +50%
    Some(("DTAF_CRT_DMG_RATEADD", 300, 0)),                 // 021 crits dealt x1.3
];

/// Reversed effects (the client's `_REV` texts), same rules.
const REVERSED: [Option<Effect>; TAROT_CARDS] = [
    None,                                                // 000 evasion -40
    None,                                                // 001 taken from Wisdom (no such affect)
    None,                                                // 002
    None,                                                // 003
    None,                                                // 004
    None,                                                // 005
    None,                                                // 006 taken from Unity (no such affect)
    None,                                                // 007 speed -50%
    None,                                                // 008 taken from Spirit (no such affect)
    Some(("DTAF_BRAVE_STYPE_VCT_DAM_RATESUB", 1000, 1)), // 009 taken from Courage +100%
    None,                                                // 010 crit rate -20
    Some(("DTAF_ATK_RATEADD", -500, 0)),                 // 011 ATK -50%
    Some(("DTAF_MAVERICK_STYPE_DMG_RATEADD", 1000, 5)),  // 012 vs Solitude +100%
    Some(("DTAF_VCT_DAM_RATESUB", -500, 0)),             // 013 damage taken -50%
    None,                                                // 014
    None,                                                // 015 accuracy +40
    // 016 Tower: the client has only the upright text; reversed DEF +50% is ours.
    Some(("DTAF_DEF_RATEADD", 500, 0)),
    None,                                       // 017
    None,                                       // 018 HP <= 50%: dealt +100%
    None,                                       // 019 HP >= 50%: taken +100%
    Some(("DTAF_DMG_RATEADD", -500, 0)),        // 020 damage dealt -50%
    Some(("DTAF_VCT_CRT_DMG_RATESUB", 300, 0)), // 021 crits taken x1.3
];

/// The card's effect, if one maps onto an existing affect.
pub fn effect(c: Card) -> Option<Effect> {
    let table = if c.reversed { &REVERSED } else { &UPRIGHT };
    table.get(c.id).copied().flatten()
}

/// `TarotInfo.abilityInfo` (an AbilityInfo, applied to every team member as source Tarot,
/// docs/research/22 §1.5). `isInit` true keeps the parameters we send: the client otherwise
/// replaces them from the action-battle ability table, which has no Tarot rows (M). A card with
/// no mapped effect sends `{}` (no effect; the card still shows).
pub fn ability_info(c: Card) -> Value {
    let Some((affect, amount, stype)) = effect(c) else {
        return json!({});
    };
    let side = if c.reversed { "RV" } else { "UP" };
    let mut p = serde_json::Map::new();
    p.insert("affectId".into(), json!(affect));
    p.insert("giveLimit".into(), json!(-1)); // 0 would expire at turn end
    for lv in [1, 10] {
        for i in 0..8 {
            let v = match i {
                0 => amount,
                1 => stype,
                _ => 0,
            };
            p.insert(format!("paramListLv{lv}_{i}"), json!(v));
        }
    }
    json!({ "abilityId": format!("DT_TAROT_{side}_{:03}", c.id), "abilityLevel": 1,
            "abilityName": format!("DT_ABI_TAROT_{side}_NAME_{:03}", c.id),
            "abilityParamList": [Value::Object(p)], "isInit": true })
}

/// `TarotInfo` as the card scene reads it. Two orientations have no description label in the
/// client (Strength upright, Tower reversed); we send the key the pattern gives.
pub fn tarot_info(c: Card) -> Value {
    let rev = if c.reversed { "_REV" } else { "" };
    json!({ "tarotId": tarot_id(c.id), "name": format!("TAROT_NAME_{:03}", c.id),
            "description": format!("TAROT_DESCRIPTION_{:03}{rev}", c.id),
            "abilityInfo": ability_info(c), "position": c.reversed as i64, "imgNo": c.id })
}

/// Mentality and the run in progress (saved with the story progress).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DungeonState {
    /// Mentality spent since the refill of `mentality_day` (so a new save starts full).
    #[serde(default)]
    pub mentality_used: i64,
    /// The game day (04:00 JST) of the last refill.
    #[serde(default)]
    pub mentality_day: i64,
    #[serde(default)]
    pub run: Option<Run>,
    /// The deepest floor ever cleared in the Beginner dungeon (saves before Build 25; ours).
    #[serde(default)]
    pub best_floor: i64,
    /// The deepest floor ever cleared, per dungeon (shown as `floorClearNum`; ours).
    #[serde(default)]
    pub best_floors: std::collections::BTreeMap<String, i64>,
    /// The suspended state the player declined to resume (`/bizarre/destruction` after the
    /// resume dialog): the room whose Tarot pick is not offered again.
    #[serde(default)]
    pub declined: Option<String>,
    /// Full clears (bookkeeping).
    #[serde(default)]
    pub clears: i64,
}

/// One run: from the first `stage_list` to a full clear, an escape or the last Soul lost.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub dungeon_id: String,
    /// Current floor, 1..=FLOORS; FLOORS + 1 once the last floor is cleared.
    pub floor: i64,
    #[serde(default)]
    pub souls_lost: i64,
    /// The room paid for at `tarot_select` (until its battle ends).
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub team_no: i64,
    /// The cards on offer (the server keeps them for `tarot_flip` and `bizarre_start`).
    #[serde(default)]
    pub offer: Vec<Card>,
    /// The card picked for the battle in progress.
    #[serde(default)]
    pub chosen: Option<Card>,
    /// Cards used since the deck last reset.
    #[serde(default)]
    pub used: Vec<Card>,
    /// Floors cleared whose rewards wait for a full clear or an escape.
    #[serde(default)]
    pub earned: Vec<i64>,
}

/// What a battle's end did to the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// Next floor (win) or the same floor with a Soul fewer (loss).
    Continue,
    /// The last floor was cleared: the rewards wait for `earn_reward`.
    Cleared,
    /// The last Soul was lost: the run and its pending rewards are gone.
    GameOver,
}

impl DungeonState {
    /// Mentality now: full again once the game day has turned since the last refill (ours).
    pub fn mentality(&self, now: i64) -> i64 {
        if crate::missions::game_day(now) != self.mentality_day {
            MENTALITY_MAX
        } else {
            (MENTALITY_MAX - self.mentality_used).clamp(0, MENTALITY_MAX)
        }
    }

    /// Spend Mentality, or `Err(52102)` when short (nothing spent).
    pub fn spend(&mut self, cost: i64, now: i64) -> Result<(), i64> {
        let today = crate::missions::game_day(now);
        if self.mentality_day != today {
            self.mentality_day = today;
            self.mentality_used = 0;
        }
        if self.mentality(now) < cost {
            return Err(E_LACK_MENTALITY);
        }
        self.mentality_used += cost;
        Ok(())
    }

    /// The deepest floor cleared in a dungeon.
    pub fn best(&self, dungeon_id: &str) -> i64 {
        let legacy = if dungeon_id == DUNGEON_ID {
            self.best_floor
        } else {
            0
        };
        self.best_floors
            .get(dungeon_id)
            .copied()
            .unwrap_or(0)
            .max(legacy)
    }

    /// Souls left (full when no run is in progress).
    pub fn souls(&self) -> i64 {
        self.run
            .as_ref()
            .map(|r| (SOUL_MAX - r.souls_lost).clamp(0, SOUL_MAX))
            .unwrap_or(SOUL_MAX)
    }

    /// `DungeonInfo.floorNum`: the current floor, -1 when not started.
    pub fn floor_num(&self, dungeon_id: &str) -> i64 {
        self.run
            .as_ref()
            .filter(|r| r.dungeon_id == dungeon_id)
            .map(|r| r.floor)
            .unwrap_or(-1)
    }

    /// The run in `dungeon_id`, started on floor 1 if none is in progress; `Err(52101)` when
    /// another dungeon's run is. The bool says whether a run started.
    pub fn enter(&mut self, dungeon_id: &str) -> Result<(&mut Run, bool), i64> {
        let started = self.run.is_none();
        let run = self.run.get_or_insert_with(|| Run {
            dungeon_id: dungeon_id.to_string(),
            floor: 1,
            ..Run::default()
        });
        if run.dungeon_id != dungeon_id {
            return Err(E_STARTED_OTHER_DUNGEON);
        }
        Ok((run, started))
    }

    /// A won battle: the floor's reward joins the pending ones and the run goes one floor down.
    pub fn battle_won(&mut self) -> Outcome {
        let Some(run) = self.run.as_mut() else {
            return Outcome::Continue;
        };
        run.room = None;
        run.chosen = None;
        if run.floor > FLOORS {
            return Outcome::Cleared;
        }
        run.earned.push(run.floor);
        let best = self.best_floors.entry(run.dungeon_id.clone()).or_insert(0);
        *best = (*best).max(run.floor);
        run.floor += 1;
        if run.floor > FLOORS {
            Outcome::Cleared
        } else {
            Outcome::Continue
        }
    }

    /// A lost or retired battle: one Soul; the last one ends the run and forfeits its rewards
    /// (the deck resets with it).
    pub fn battle_lost(&mut self) -> Outcome {
        let Some(run) = self.run.as_mut() else {
            return Outcome::Continue;
        };
        run.room = None;
        run.chosen = None;
        run.souls_lost += 1;
        if run.souls_lost >= SOUL_MAX {
            self.run = None;
            Outcome::GameOver
        } else {
            Outcome::Continue
        }
    }

    /// End the run (full clear or escape): returns the floors whose rewards are now paid. Souls
    /// and the deck start fresh with the next run.
    pub fn finish(&mut self) -> Vec<i64> {
        match self.run.take() {
            Some(run) => {
                if run.floor > FLOORS {
                    self.clears += 1;
                }
                run.earned
            }
            None => Vec::new(),
        }
    }
}

impl Run {
    /// Deal a new offer: up to three cards not used since the deck last reset (all 22 used =
    /// the deck resets first). Cards are dealt upright.
    pub fn deal(&mut self, seed: u64) {
        if self.used.len() >= TAROT_CARDS {
            self.used.clear();
        }
        let mut deck: Vec<usize> = (0..TAROT_CARDS)
            .filter(|i| !self.used.iter().any(|c| c.id == *i))
            .collect();
        let mut x = seed | 1;
        self.offer.clear();
        while self.offer.len() < OFFER_SIZE && !deck.is_empty() {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let id = deck.swap_remove((x % deck.len() as u64) as usize);
            self.offer.push(Card {
                id,
                reversed: false,
            });
        }
    }

    /// Reverse every card on offer.
    pub fn flip(&mut self) {
        for c in &mut self.offer {
            c.reversed = !c.reversed;
        }
    }

    /// Take the offered card `id` for the battle: it is used until the deck resets.
    pub fn choose(&mut self, id: usize) -> Option<Card> {
        let c = *self.offer.iter().find(|c| c.id == id)?;
        self.offer.clear();
        self.used.push(c);
        self.chosen = Some(c);
        Some(c)
    }
}

/// `DungeonInfo` for the dungeon list.
pub fn dungeon_info(d: &Dungeon, s: &DungeonState) -> Value {
    json!({ "dungeonId": d.id, "name": d.name, "summary": d.summary,
            "floorMax": FLOORS, "floorNum": s.floor_num(d.id), "floorClearNum": s.best(d.id),
            "difficulty": d.difficulty, "imgNo": DUNGEON_IMG, "stageType": 1 })
}

/// `DungeonRewardInfo` rows for the reward review: one per floor. The client marks a row
/// achieved while the run's floor is past it.
pub fn reward_rows(d: &Dungeon) -> Vec<Value> {
    (1..=FLOORS)
        .map(|f| {
            json!({ "rewardItemSet": crate::missions::reward_set(&floor_reward(d, f)), "floorNum": f,
                    "reward_title": "", "reward_text": "" })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twenty_floors_of_rooms_get_harder() {
        let mut last = 0;
        for f in 1..=FLOORS {
            let rs = rooms(f);
            assert_eq!(rs.len(), rooms_on(f));
            for r in &rs {
                assert!((22..=35).contains(&r.cost), "{} costs {}", r.id, r.cost);
                assert_eq!(r.chapter.stage_type, 1, "tactical stages only");
                let back = find_room(&r.id).unwrap();
                assert_eq!(back.stage.stage_id, r.stage.stage_id);
                let info = room_stage_info(r);
                assert_eq!(info["stageId"], r.id.as_str());
                assert_eq!(info["useMentality"], r.cost);
                assert!(info["rewardItemSet"][0]["dataId"]
                    .as_str()
                    .unwrap()
                    .parse::<i32>()
                    .is_ok());
            }
            let top = rs.iter().map(|r| r.stage.enemy_level).max().unwrap();
            assert!(top >= last, "floor {f} is not easier than the one above");
            last = top;
        }
        assert!(find_room("STAGE_DU01_21_01").is_none());
        assert!(find_room("STAGE_DU01_10_02").is_none());
        assert!(find_room("STAGE_DU04_01_01").is_none());
        assert!(find_room("DT_STAGE_ST_00_01").is_none());
    }

    /// The Intermediate and Advanced dungeons: the same rooms (and labels) with stronger enemies
    /// and larger rewards; their room ids find their own dungeon.
    #[test]
    fn harder_dungeons_raise_levels_and_rewards() {
        for f in [1, 10, 20] {
            let easy = rooms_in(&DUNGEONS[0], f);
            for d in &DUNGEONS[1..] {
                let hard = rooms_in(d, f);
                assert_eq!(hard.len(), easy.len());
                for (e, h) in easy.iter().zip(&hard) {
                    assert_eq!(
                        h.label, e.label,
                        "one label set names every dungeon's rooms"
                    );
                    assert_eq!(
                        h.id,
                        format!("STAGE_{}_{}", d.id, &e.id["STAGE_DU01_".len()..])
                    );
                    assert_eq!(
                        h.stage.enemy_level,
                        (e.stage.enemy_level + d.level_add).min(99)
                    );
                    assert!(h.stage.enemy_level >= e.stage.enemy_level);
                    let back = find_room(&h.id).unwrap();
                    assert_eq!(back.dungeon.id, d.id);
                    let info = room_stage_info(h);
                    assert_eq!(info["name"], e.label.as_str());
                    assert_eq!(
                        info["enemyList"][0]["level"].as_i64(),
                        Some(h.stage.enemy_level)
                    );
                }
                assert_eq!(
                    floor_reward(d, f).count,
                    floor_reward(&DUNGEONS[0], f).count * d.reward_mult
                );
            }
        }
        assert!(DUNGEONS
            .windows(2)
            .all(|w| w[0].difficulty < w[1].difficulty));
    }

    #[test]
    fn labels_exist_for_names_rooms_and_cards() {
        let tr = crate::l10n::Translations::embedded().unwrap();
        let has = |table: &str, key: &str| tr.english(&format!("text:{table}:{key}")).is_some();
        for d in &DUNGEONS {
            assert!(has("DungeonName_JP", d.name), "{}", d.name);
            assert!(has("DungeonSceneText_JP", d.summary), "{}", d.summary);
            for f in 1..=FLOORS {
                for r in rooms_in(d, f) {
                    assert!(has("DungeonStageName_JP", &r.label), "{}", r.label);
                }
            }
        }
        for id in 0..TAROT_CARDS {
            for reversed in [false, true] {
                let t = tarot_info(Card { id, reversed });
                assert!(has("TarotName_JP", t["name"].as_str().unwrap()));
                let d = t["description"].as_str().unwrap();
                // Strength upright and Tower reversed have no text in the client.
                assert!(
                    has("TarotDescription_JP", d) || [11, 16].contains(&id),
                    "{d}"
                );
                if let Some(n) = t["abilityInfo"]["abilityName"].as_str() {
                    assert!(has("Tarot_Name_JP", n), "{n}");
                }
            }
        }
    }

    #[test]
    fn tarot_effects_use_existing_affects() {
        let affects: Vec<String> = serde_json::from_str::<Value>(
            crate::masterdata::TABLES
                .iter()
                .find(|(n, _)| *n == "affect_table")
                .unwrap()
                .1,
        )
        .unwrap()["data"]["paramList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["affectId"].as_str().unwrap().to_string())
            .collect();
        let mut mapped = 0;
        for id in 0..TAROT_CARDS {
            for reversed in [false, true] {
                let a = ability_info(Card { id, reversed });
                if a == json!({}) {
                    continue;
                }
                mapped += 1;
                let p = &a["abilityParamList"][0];
                assert!(affects.contains(&p["affectId"].as_str().unwrap().to_string()));
                assert_eq!(p["giveLimit"], -1);
                assert_eq!(p["paramListLv1_0"], p["paramListLv10_0"]);
                assert_eq!(a["isInit"], true);
                let stype = p["paramListLv1_1"].as_i64().unwrap();
                assert_eq!(
                    stype != 0,
                    p["affectId"].as_str().unwrap().contains("_STYPE_")
                );
            }
        }
        assert!(mapped >= 15, "{mapped}");
    }

    #[test]
    fn mentality_refills_at_the_day_roll() {
        let mut s = DungeonState::default();
        // 2026-11-18 12:00 JST.
        let noon = 1_794_970_800;
        assert_eq!(s.mentality(noon), MENTALITY_MAX, "a new save is full");
        for _ in 0..10 {
            s.spend(30, noon).unwrap();
        }
        assert_eq!(s.mentality(noon), 0);
        assert_eq!(s.spend(1, noon), Err(E_LACK_MENTALITY));
        // 03:59 the next day: still the same game day.
        assert_eq!(s.mentality(noon + 16 * 3600 - 60), 0);
        // 04:00: full again.
        assert_eq!(s.mentality(noon + 16 * 3600), MENTALITY_MAX);
        s.spend(25, noon + 16 * 3600).unwrap();
        assert_eq!(s.mentality(noon + 16 * 3600 + 1), MENTALITY_MAX - 25);
    }

    #[test]
    fn the_deck_uses_each_card_once_until_it_resets() {
        let mut run = Run::default();
        let mut seen = std::collections::BTreeSet::new();
        let mut seed: u64 = 0x1234_5678;
        for _ in 0..8 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            run.deal(seed);
            assert!(!run.offer.is_empty() && run.offer.len() <= OFFER_SIZE);
            let pick = run.offer[0].id;
            assert!(
                seen.insert(pick),
                "card {pick} offered again before the reset"
            );
            for c in &run.offer {
                assert!(!run.used.iter().any(|u| u.id == c.id));
            }
            run.choose(pick).unwrap();
        }
        // 8 picks: 22 - 8 = 14 left; use them all, then the deck resets.
        while run.used.len() < TAROT_CARDS {
            run.deal(seed);
            let id = run.offer[0].id;
            run.choose(id).unwrap();
        }
        run.deal(seed);
        assert_eq!(run.used.len(), 0, "the deck reset after all 22");
        assert_eq!(run.offer.len(), OFFER_SIZE);
        assert!(run.choose(99).is_none());
    }

    /// Build 43: the wiki's room costs (Bahamut screenshots, the slot pattern) and floor rewards
    /// (ShoutWiki) replace ours; rooms no wiki covers keep ours.
    #[test]
    fn wiki_room_costs_and_floor_rewards_apply() {
        assert!(quests::labyrinth().rooms.len() >= 36);
        // On a screenshot: STAGE_DU01_01_02 22, STAGE_DU01_02_01 30, both single rooms 30.
        assert_eq!(room_cost(1, 1), 22);
        assert_eq!(room_cost(2, 0), 30);
        assert_eq!(room_cost(10, 0), 30);
        assert_eq!(room_cost(20, 0), 30);
        // The slot pattern from floor 5 on (floor 17 has no screenshot).
        assert_eq!(
            (room_cost(17, 0), room_cost(17, 1), room_cost(17, 2)),
            (35, 27, 25)
        );
        // Floor 4 has no screenshot and comes before the pattern: ours.
        assert_eq!(room_cost(4, 0), 23);
        for (floor, item, count) in [(1, 1002, 30), (2, 1003, 30), (3, 1004, 30), (4, 1006, 10)] {
            let r = floor_reward(&DUNGEONS[0], floor);
            assert_eq!((r.item_id, r.count), (item, count), "floor {floor}");
        }
        assert_eq!(
            floor_reward(&DUNGEONS[0], 5).item_id,
            crate::items::MEMORY_RING
        );
    }

    #[test]
    fn rewards_are_one_known_item_per_floor() {
        for d in &DUNGEONS {
            for f in 1..=FLOORS {
                let r = floor_reward(d, f);
                assert!(r.count > 0);
                assert!(r.item_id == 20 || crate::items::find(r.item_id).is_some());
            }
            assert_eq!(reward_rows(d).len(), FLOORS as usize);
        }
    }
}
