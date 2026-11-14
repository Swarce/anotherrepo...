//! The player's collection: owned statues (characters) and teams.
//!
//! Offline, the player owns every obtainable statue (a preservation sandbox; the dead server's
//! gacha economy comes later). Statue definitions are `server/content/statues.json`: identifiers
//! and model names from the player's own files, stats from the community wikis where matched,
//! marked defaults elsewhere. Names and descriptions are text labels the client resolves itself.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const STATUES_JSON: &str = include_str!("../../content/statues.json");

/// Highest statue level offered (the wikis give Lv99 for every rarity).
pub const MAX_LEVEL: i64 = 99;
/// Teams per battle type (the original count is unknown; 5 is a guess).
pub const TEAMS_PER_TYPE: i64 = 5;
/// Preferred starting leader and members (first ones present are used).
const STARTERS: &[&str] = &["1000_01", "3000_01", "4000_01", "2000_01", "5000_01"];

#[derive(Debug, Clone, Deserialize)]
pub struct Stats {
    pub hp: i64,
    pub atk: i64,
    pub def: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatueDef {
    #[serde(rename = "statueId")]
    pub statue_id: String,
    pub part: i64,
    #[serde(rename = "modelDataList")]
    pub model_data_list: Vec<String>,
    pub rarity: Option<i64>,
    #[serde(rename = "statueType")]
    pub statue_type: Option<i64>,
    pub lv1: Option<Stats>,
    pub lv99: Option<Stats>,
}

impl StatueDef {
    pub fn psid(&self) -> i64 {
        self.statue_id
            .get(..4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }
    /// Rarity (wiki), else 4 (jdr-default).
    pub fn rarity(&self) -> i64 {
        self.rarity.unwrap_or(4)
    }
    /// StatueType 1..5 (wiki), else a stable placeholder from the id (jdr-default).
    pub fn statue_type(&self) -> i64 {
        self.statue_type
            .unwrap_or_else(|| 1 + self.psid().rem_euclid(5))
    }
    /// Stats at a level: linear between Lv1 and Lv99 (the wikis' Lv99 is 10x Lv1 for 195 of 200
    /// statues, consistent with a linear curve). Defaults: a mid 4-star statue.
    pub fn stats_at(&self, level: i64) -> Stats {
        let (a, b) = match (&self.lv1, &self.lv99) {
            (Some(a), Some(b)) => (a.clone(), b.clone()),
            _ => (
                Stats {
                    hp: 800,
                    atk: 450,
                    def: 380,
                },
                Stats {
                    hp: 8000,
                    atk: 4500,
                    def: 3800,
                },
            ),
        };
        let t = (level.clamp(1, MAX_LEVEL) - 1) as f64 / (MAX_LEVEL - 1) as f64;
        let lerp = |x: i64, y: i64| x + ((y - x) as f64 * t).round() as i64;
        Stats {
            hp: lerp(a.hp, b.hp),
            atk: lerp(a.atk, b.atk),
            def: lerp(a.def, b.def),
        }
    }
}

/// Cumulative EXP to reach a level (jdr-default curve: no source gives the original one).
pub fn exp_for_level(level: i64) -> i64 {
    let l = level.clamp(1, MAX_LEVEL) - 1;
    l * l * 25 + l * 75
}

pub fn statues() -> &'static [StatueDef] {
    static CELL: std::sync::OnceLock<Vec<StatueDef>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            statues: Vec<StatueDef>,
        }
        serde_json::from_str::<Doc>(STATUES_JSON)
            .map(|d| d.statues)
            .unwrap_or_default()
    })
}

pub fn find(statue_id: &str) -> Option<&'static StatueDef> {
    statues().iter().find(|s| s.statue_id == statue_id)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OwnedStatue {
    pub uid: i64,
    pub statue_id: String,
    pub level: i64,
    pub exp: i64,
    #[serde(default)]
    pub get_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Team {
    /// 0 = Battle (action), 1 = DtBattle (tactical).
    pub stage_type: i64,
    pub team_num: i64,
    pub main: bool,
    /// Leader, left, right: statue uids (0 = empty).
    pub slots: [i64; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Collection {
    #[serde(default)]
    pub statues: Vec<OwnedStatue>,
    #[serde(default)]
    pub teams: Vec<Team>,
    #[serde(default)]
    pub minds: Vec<OwnedMind>,
}

impl Collection {
    /// Grant every obtainable statue not owned yet and make sure the teams exist. Returns true if
    /// anything changed (the caller saves).
    pub fn ensure(&mut self, now: &str) -> bool {
        let mut changed = false;
        let mut next = self.statues.iter().map(|s| s.uid).max().unwrap_or(0) + 1;
        for d in statues() {
            if !self.statues.iter().any(|s| s.statue_id == d.statue_id) {
                self.statues.push(OwnedStatue {
                    uid: next,
                    statue_id: d.statue_id.clone(),
                    level: 1,
                    exp: 0,
                    get_date: now.to_string(),
                });
                next += 1;
                changed = true;
            }
        }
        let mut next_mind = self.minds.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
        for d in minds() {
            if !self.minds.iter().any(|m| m.mind_id == d.mind_id) {
                self.minds.push(OwnedMind {
                    uid: next_mind,
                    mind_id: d.mind_id.clone(),
                    get_date: now.to_string(),
                });
                next_mind += 1;
                changed = true;
            }
        }
        for stage_type in [0, 1] {
            for team_num in 1..=TEAMS_PER_TYPE {
                if self
                    .teams
                    .iter()
                    .any(|t| t.stage_type == stage_type && t.team_num == team_num)
                {
                    continue;
                }
                let slots = if team_num == 1 {
                    self.starter_slots()
                } else {
                    [0; 3]
                };
                self.teams.push(Team {
                    stage_type,
                    team_num,
                    main: team_num == 1,
                    slots,
                });
                changed = true;
            }
        }
        changed |= self.repair_teams();
        changed
    }

    fn starter_slots(&self) -> [i64; 3] {
        let mut picked: Vec<i64> = STARTERS
            .iter()
            .filter_map(|id| {
                self.statues
                    .iter()
                    .find(|s| s.statue_id == *id && find(&s.statue_id).is_some())
                    .map(|s| s.uid)
            })
            .take(3)
            .collect();
        for s in &self.statues {
            if picked.len() >= 3 {
                break;
            }
            if !picked.contains(&s.uid) && find(&s.statue_id).is_some() {
                picked.push(s.uid);
            }
        }
        [
            picked.first().copied().unwrap_or(0),
            picked.get(1).copied().unwrap_or(0),
            picked.get(2).copied().unwrap_or(0),
        ]
    }

    pub fn statue(&self, uid: i64) -> Option<&OwnedStatue> {
        self.statues.iter().find(|s| s.uid == uid)
    }

    /// An owned statue the content still knows (a statue dropped from statues.json stays in the
    /// save but is not shown or used).
    pub fn known_statue(&self, uid: i64) -> Option<&OwnedStatue> {
        self.statue(uid).filter(|o| find(&o.statue_id).is_some())
    }

    /// Clear team slots that point at nothing usable and refill a main team without a leader.
    fn repair_teams(&mut self) -> bool {
        let mut changed = false;
        let usable: std::collections::HashSet<i64> = self
            .statues
            .iter()
            .filter(|o| find(&o.statue_id).is_some())
            .map(|o| o.uid)
            .collect();
        let starter = self.starter_slots();
        for t in &mut self.teams {
            for s in &mut t.slots {
                if *s != 0 && !usable.contains(s) {
                    *s = 0;
                    changed = true;
                }
            }
            if t.main && t.slots[0] == 0 {
                t.slots = starter;
                changed = true;
            }
        }
        changed
    }

    /// Add battle EXP to the given statues, levelling them up along `exp_for_level` (capped at
    /// MAX_LEVEL). Returns how many levels were gained in total.
    pub fn add_statue_exp(&mut self, uids: &[i64], exp: i64) -> i64 {
        let mut gained = 0;
        for o in self
            .statues
            .iter_mut()
            .filter(|o| o.uid != 0 && uids.contains(&o.uid))
        {
            o.exp = (o.exp + exp.max(0)).min(exp_for_level(MAX_LEVEL));
            while o.level < MAX_LEVEL && o.exp >= exp_for_level(o.level + 1) {
                o.level += 1;
                gained += 1;
            }
        }
        gained
    }
}

/// `StatueInfo` as the client reads it (see docs/research/10-content.md §2.2). Skill and ability
/// details are left to the master tables (`isInit` false).
pub fn statue_info(o: &OwnedStatue) -> Value {
    let d = find(&o.statue_id);
    let (part, rare, stype, models) = match d {
        Some(d) => (
            d.part,
            d.rarity(),
            d.statue_type(),
            d.model_data_list.clone(),
        ),
        None => (1, 4, 1, Vec::new()),
    };
    let st = d.map(|d| d.stats_at(o.level)).unwrap_or(Stats {
        hp: 1,
        atk: 1,
        def: 1,
    });
    let sid = &o.statue_id;
    json!({
        "statueId": sid, "name": format!("STATUE_NAME_{sid}"), "nameSortIndex": 0,
        "rare": rare, "rareMax": rare, "appearedPart": part,
        "description": format!("PROF_TEXT_{sid}"), "specialActionText": "",
        "charaTypeId": d.map(|d| d.psid()).unwrap_or(0),
        "tribeTypeList": [], "statueType": stype, "oraGauge": 0, "standRange": 0,
        "modelDataList": models, "mindCapacity": 99, "useAttr": 1, "attckType": 0,
        "addProfileName": "", "addProfile": "",
        "maxLevel": MAX_LEVEL, "sellVm": 0, "level": o.level, "bounds": 0, "exp": o.exp,
        "hp": st.hp, "eng": 0, "atk": st.atk, "def": st.def,
        "skillList": [], "abilityList": [], "leaderAbilityInfo": {},
        "dtSkillList": [], "dtAbilityList": [], "dtLeaderAbilityInfo": {},
        "characteristicList": [], "assistAttr": 0, "assistSkillInfo": {}, "assistAbilityList": [],
        "dtAssistSkillInfo": {}, "dtAssistAbilityList": [],
        "speed": 100, "evasion": 0, "hitrate": 100, "criticalRate": 5,
        "stEffRateInfo": {}, "maxSp": 100, "useStageType": 0, "levelUpTable": [],
        "statueuId": o.uid, "treeuId": "", "newFlg": 0, "useFlg": 0, "protectedFlg": 0,
        "limitReleaseCount": 0, "convergence": 0, "getDate": o.get_date,
        "evolutionFlg": 0, "openEvolutionTreeFlg": 0, "evolutionLevel": 0,
        "evolutionConvergence": 0, "evolutionUseGold": 0, "evolutionUseItemList": []
    })
}

/// Our basic tactical attack (`JDR_ATK_00` in dtbattle_skill_data_table), given to every statue in
/// a team (and to enemies) until real skill data exists: without a combo-type skill a unit has no
/// Attack button. The client takes type, button and name from the table row but keeps
/// `skillLevel`, which must be at least 1. Statue lists and detail views leave it out (their skill
/// views were not traced).
pub const BASIC_ATTACK: &str = "JDR_ATK_00";

/// `statue_info` for a battle unit: with the basic attack.
pub fn battle_statue_info(o: &OwnedStatue) -> Value {
    let mut v = statue_info(o);
    v["dtSkillList"] = json!([basic_attack()]);
    v
}

fn basic_attack() -> Value {
    json!({ "skillId": BASIC_ATTACK, "skillLevel": 1, "name": "", "explanation": "",
            "skillType": 1, "bottunType": 1, "coolDown": 0, "limitCount": 0,
            "magnificationLv01": 0, "magnificationLv10": 0, "pvpMagnificationLv01": 0,
            "pvpMagnificationLv10": 0, "skillParamList": [], "isInit": false })
}

fn empty_setting() -> Value {
    json!({ "statueuId": 0, "statueInfo": { "statueId": "" }, "equipMindList": [],
            "mindLinkList": [], "assistStatueList": [], "AiId": "" })
}

fn setting(c: &Collection, uid: i64) -> Value {
    match c.known_statue(uid) {
        Some(o) => {
            json!({ "statueuId": uid, "statueInfo": battle_statue_info(o), "equipMindList": [],
                           "mindLinkList": [], "assistStatueList": [], "AiId": "" })
        }
        None => empty_setting(),
    }
}

/// `TeamInfo` for one team.
pub fn team_info(c: &Collection, t: &Team) -> Value {
    json!({
        "teamNum": t.team_num, "stageType": t.stage_type, "mainFlg": t.main as i64, "useFlg": 1,
        "teamOrder": {
            "leaderStatue": setting(c, t.slots[0]),
            "memberLeftStatue": setting(c, t.slots[1]),
            "memberRightStatue": setting(c, t.slots[2])
        },
        "jojoLinkList": []
    })
}

/// The main team of a battle type (My Page and the UI skin use the tactical one, type 1).
pub fn main_team(c: &Collection, stage_type: i64) -> Option<&Team> {
    c.teams
        .iter()
        .find(|t| t.stage_type == stage_type && t.main)
}

/// Rows for `statue_setting_table`: every statue gets a My Page background by its JoJo part
/// (backgrounds bg_0010..bg_0050 exist in the player's files; which one the original used per
/// statue is unknown: jdr-default).
pub fn statue_setting_rows() -> Vec<Value> {
    const BG: &[(i64, &[i64])] = &[
        (1, &[10, 11, 12, 13]),
        (2, &[20, 21, 22, 23]),
        (3, &[30, 31, 32, 33, 34, 35]),
        (4, &[40, 41, 42, 43, 44, 45]),
        (5, &[50]),
    ];
    statues()
        .iter()
        .map(|d| {
            let ids = BG
                .iter()
                .find(|(p, _)| *p == d.part)
                .map(|(_, ids)| *ids)
                .unwrap_or(&[10]);
            // By a hash of the id, so a statue keeps its background when statues.json changes.
            let h = d.statue_id.bytes().fold(0x811c_9dc5u32, |h, b| {
                (h ^ b as u32).wrapping_mul(0x0100_0193)
            });
            // Positions are centimetres and teamScaleScalar a percentage (the client multiplies all of
            // them by 0.01). My Page: the camera at the `CameraIdSet0` scene object (the origin,
            // facing +Z) and the leader 3.22 m ahead, right of centre (Build 12: 12% closer at the owner's request), turned towards the camera,
            // which frames models 1.0-2.3 m tall. Team view: the slot anchors already face the
            // camera, so no offset, at full size. (docs/research/11-battle-and-positions.md)
            json!({
                "statueId": d.statue_id, "myPageId": ids[h as usize % ids.len()],
                "myPageCameraType": "CameraIdSet0",
                "myPagePos": { "x": 72.0, "y": -113.0, "z": 322.0 },
                "myPageRotation": { "x": 0.0, "y": 192.5, "z": 0.0 },
                "teamScaleScalar": 100.0,
                "mindFrameID": 1, "pvpAiId": 1, "pvpAiAutoBattleAiId": 1
            })
        })
        .collect()
}

/// Rows for `statue_level_table`.
pub fn statue_level_rows() -> Vec<Value> {
    statues()
        .iter()
        .map(|d| {
            let levels: Vec<Value> = (1..=MAX_LEVEL)
                .map(|l| {
                    let s = d.stats_at(l);
                    json!({ "level": l, "exp": exp_for_level(l), "hp": s.hp, "atk": s.atk, "def": s.def })
                })
                .collect();
            json!({ "statueId": d.statue_id, "levelTableList": levels })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_loads_and_grants() {
        assert!(statues().len() > 200, "statues.json loaded");
        let mut c = Collection::default();
        assert!(c.ensure("2026-11-14 00:00:00"));
        assert_eq!(c.statues.len(), statues().len());
        assert!(!c.ensure("x"), "idempotent");
        let main = main_team(&c, 1).expect("tactical main team");
        assert!(main.slots.iter().all(|&u| u > 0));
        let t = team_info(&c, main);
        assert_eq!(t["mainFlg"], 1);
        assert_eq!(t["stageType"], 1);
        let leader = &t["teamOrder"]["leaderStatue"]["statueInfo"];
        assert_eq!(leader["statueId"], "1000_01", "Jonathan leads when present");
        assert!(leader["modelDataList"][0]
            .as_str()
            .unwrap()
            .starts_with("1000_0_"));
        assert_eq!(statue_setting_rows().len(), statues().len());
    }

    #[test]
    fn stats_are_linear_between_lv1_and_lv99() {
        let d = StatueDef {
            statue_id: "1234_01".into(),
            part: 1,
            model_data_list: vec![],
            rarity: Some(3),
            statue_type: Some(2),
            lv1: Some(Stats {
                hp: 100,
                atk: 10,
                def: 20,
            }),
            lv99: Some(Stats {
                hp: 1000,
                atk: 100,
                def: 200,
            }),
        };
        assert_eq!(d.stats_at(1).hp, 100);
        assert_eq!(d.stats_at(99).hp, 1000);
        assert_eq!(d.stats_at(50).atk, 55);
        assert!(exp_for_level(2) > exp_for_level(1));
    }
}

// ---------------------------------------------------------------------------------------------
// Player rank (from the wiki's user-rank table).

pub const RANKS_JSON: &str = include_str!("../../content/ranks.json");

#[derive(Debug, Clone, Deserialize)]
pub struct RankRow {
    pub rank: i64,
    pub exp: i64,
    #[serde(rename = "apMax")]
    pub ap_max: i64,
}

pub fn ranks() -> &'static [RankRow] {
    static CELL: std::sync::OnceLock<Vec<RankRow>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            ranks: Vec<RankRow>,
        }
        let mut v = serde_json::from_str::<Doc>(RANKS_JSON)
            .map(|d| d.ranks)
            .unwrap_or_default();
        v.sort_by_key(|r| r.rank);
        v
    })
}

/// Rank reached with `exp` total rank EXP (1 if the table is empty).
pub fn rank_for_exp(exp: i64) -> i64 {
    ranks()
        .iter()
        .filter(|r| r.exp <= exp)
        .map(|r| r.rank)
        .max()
        .unwrap_or(1)
}

/// AP (stamina) maximum at a rank (20 if unknown, the wiki's rank-1 value).
pub fn ap_max(rank: i64) -> i64 {
    ranks()
        .iter()
        .find(|r| r.rank == rank)
        .map(|r| r.ap_max)
        .unwrap_or(20)
}

/// Total EXP needed for the next rank. At the top rank there is none: one past the top threshold,
/// so a client computing `(exp - cur) / (next - cur)` never divides by zero.
pub fn next_rank_exp(rank: i64) -> i64 {
    match ranks().iter().find(|r| r.rank == rank + 1) {
        Some(r) => r.exp,
        None => ranks().last().map(|r| r.exp + 1).unwrap_or(100),
    }
}

/// Rows for `rank_table` (`PlayerRankTable.Param {rank, exp}`).
pub fn rank_rows() -> Vec<Value> {
    ranks()
        .iter()
        .map(|r| json!({ "rank": r.rank, "exp": r.exp }))
        .collect()
}

#[cfg(test)]
mod rank_tests {
    use super::*;
    #[test]
    fn rank_table() {
        assert!(ranks().len() >= 99);
        assert_eq!(rank_for_exp(0), 1);
        assert_eq!(rank_for_exp(39), 1);
        assert_eq!(rank_for_exp(40), 2);
        assert_eq!(ap_max(1), 20);
        assert_eq!(next_rank_exp(1), 40);
        let top = ranks().last().unwrap();
        assert!(
            next_rank_exp(top.rank) > top.exp,
            "no zero-width bar at the top rank"
        );
    }

    #[test]
    fn statue_exp_levels_up_and_caps() {
        let mut c = Collection::default();
        c.ensure("2026-01-01 00:00:00");
        let uid = c.statues[0].uid;
        let other = c.statues[1].uid;
        assert_eq!(c.add_statue_exp(&[uid, 0], exp_for_level(3)), 2);
        assert_eq!(c.statue(uid).unwrap().level, 3);
        assert_eq!(c.statue(other).unwrap().level, 1, "only the team gains EXP");
        c.add_statue_exp(&[uid], i64::MAX / 2);
        assert_eq!(c.statue(uid).unwrap().level, MAX_LEVEL);
        assert_eq!(c.statue(uid).unwrap().exp, exp_for_level(MAX_LEVEL));
    }

    #[test]
    fn teams_drop_statues_the_content_no_longer_has() {
        let mut c = Collection::default();
        c.ensure("2026-01-01 00:00:00");
        let main = c
            .teams
            .iter()
            .position(|t| t.main && t.stage_type == 1)
            .unwrap();
        let leader = c.teams[main].slots[0];
        c.statues
            .iter_mut()
            .find(|s| s.uid == leader)
            .unwrap()
            .statue_id = "9999_99".into();
        assert!(c.ensure("2026-01-01 00:00:00"), "repair counts as a change");
        let t = &c.teams[main];
        assert_ne!(t.slots[0], 0, "main team gets a leader back");
        assert!(t
            .slots
            .iter()
            .all(|s| *s == 0 || c.known_statue(*s).is_some()));
        assert!(!c.ensure("2026-01-01 00:00:00"), "and is stable afterwards");
    }
}

// ---------------------------------------------------------------------------------------------
// Minds (cards).

pub const MINDS_JSON: &str = include_str!("../../content/minds.json");
/// Mind box size offered (the player owns every listed mind).
pub const MIND_MAX: i64 = 999;

#[derive(Debug, Clone, Deserialize)]
pub struct MindDef {
    #[serde(rename = "mindId")]
    pub mind_id: String,
    pub rarity: i64,
    pub cost: i64,
    pub hp: i64,
    pub atk: i64,
    pub def: i64,
}

pub fn minds() -> &'static [MindDef] {
    static CELL: std::sync::OnceLock<Vec<MindDef>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            minds: Vec<MindDef>,
        }
        serde_json::from_str::<Doc>(MINDS_JSON)
            .map(|d| d.minds)
            .unwrap_or_default()
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OwnedMind {
    pub uid: i64,
    pub mind_id: String,
    #[serde(default)]
    pub get_date: String,
}

/// `MindInfo` (level 1; abilities left to the master tables).
pub fn mind_info(o: &OwnedMind) -> Value {
    let d = minds().iter().find(|m| m.mind_id == o.mind_id);
    let (rare, cost, hp, atk, def) = d
        .map(|d| (d.rarity, d.cost, d.hp, d.atk, d.def))
        .unwrap_or((3, 9, 0, 0, 0));
    let id = &o.mind_id;
    json!({
        "mindId": id, "name": format!("MIND_NAME_{id}"), "nameSortIndex": 0, "rare": rare, "cost": cost,
        "description": format!("MIND_TEXT_{id}"), "mindType": 1,
        "abilityInfo": {}, "skillInfo": {}, "dtAbilityInfo": {}, "dtSkillInfo": {},
        "tokkouInfo": {}, "dtTokkouInfo": {}, "pvpAbilityInfo": {}, "dtPvpAbilityInfo": {},
        "maxLevel": 10, "sellVm": 0, "linkParam": "", "tipsText": "", "level": 1, "exp": 0,
        "limitCharaList": [], "limitTribeList": [], "limitStatueType": 0, "limitCount": 0,
        "limitCharacteristicList": [], "characteristicList": [], "levelUpTable": [],
        "minduId": o.uid, "sellRing": 0, "sellGet": 0, "stack": 1, "materialExp": 0,
        "bonusCondition": 0, "bonusRate": 0, "hp": hp, "eng": 0, "atk": atk, "def": def,
        "newFlg": 0, "useFlg": 0, "protectedFlg": 0, "getDate": o.get_date
    })
}
