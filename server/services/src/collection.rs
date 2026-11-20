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
    /// Stats with the growth tree complete (wiki), when known.
    #[serde(rename = "maxTree", default)]
    pub max_tree: Option<Stats>,
    /// Tactical basic attack skill id, chosen from the statue's action sets in the player's files
    /// (`tools/content/build_statues.py`).
    #[serde(rename = "dtAttack", default)]
    pub dt_attack: Option<String>,
    /// Tactical skills (normal, special) from `tools/content/build_statues.py`.
    #[serde(rename = "dtSkills", default)]
    pub dt_skills: Vec<DtSkill>,
    /// Label key of the win-screen quote (`Result_Chara_Comment_<statueId>`), when the game has one.
    #[serde(rename = "resultComment", default)]
    pub result_comment: Option<String>,
    /// Team-view scale in percent (the bust shot frames 1.55-2.05 m; tools/content/build_statues.py).
    #[serde(rename = "teamScale", default = "full_scale")]
    pub team_scale: i64,
    /// Team-view offset in cm (x, y, z), centring the posed head over the slot
    /// (tools/content/build_statues.py, docs/research/12 §7).
    #[serde(rename = "teamPos", default)]
    pub team_pos: [i64; 3],
    /// My Page offset in cm from the `CameraIdSet0` camera: right of centre, 1.8 m ahead, the head
    /// bone a quarter metre above the camera (tools/content/build_statues.py, docs/research/31 §5).
    #[serde(rename = "myPagePos", default = "my_page_pos")]
    pub my_page_pos: [i64; 3],
    /// An assist-only statue (`_6x`/`_7x` with their own `ABI_AST_<id>_N` label and no leader
    /// ability; docs/research/19-assists.md §6): the client's `assistAttr` OnlyAssist (1). It may
    /// be an assist but never a team member.
    #[serde(rename = "assistOnly", default)]
    pub assist_only: bool,
}

fn full_scale() -> i64 {
    100
}

fn my_page_pos() -> [i64; 3] {
    [35, -140, 180]
}

/// One tactical skill of a statue: label keys from the player's files, our numbers.
#[derive(Debug, Clone, Deserialize)]
pub struct DtSkill {
    pub id: String,
    pub special: bool,
    pub name: String,
    pub text: String,
    pub button: i64,
    #[serde(rename = "damageRank")]
    pub damage_rank: i64,
    pub category: String,
    #[serde(rename = "targetType")]
    pub target_type: i64,
    #[serde(rename = "processType")]
    pub process_type: i64,
    pub attr: String,
}

impl DtSkill {
    /// The skill's own key `SKILL_<p>_<NN>`: the action set (the number after the last `_`) and
    /// the classic skill row. A Mind's copy of a statue skill of the other kind has the row id
    /// `MSKILL_<p>_<NN>` (tools/content/build_mind_skills.py).
    pub fn view_id(&self) -> String {
        self.id.strip_prefix('M').unwrap_or(&self.id).to_string()
    }
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

/// The statues that may be team members (every statue but the assist-only ones), in content order.
pub fn members() -> impl Iterator<Item = &'static StatueDef> {
    statues().iter().filter(|d| !d.assist_only)
}

/// Whether an owned statue may sit in a team seat (known and not assist-only).
pub fn can_be_member(statue_id: &str) -> bool {
    find(statue_id).is_some_and(|d| !d.assist_only)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OwnedStatue {
    pub uid: i64,
    pub statue_id: String,
    pub level: i64,
    pub exp: i64,
    #[serde(default)]
    pub get_date: String,
    /// Growth tree squares opened (ids from `tree_squares`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tree: Vec<String>,
    /// Duplicate boosts (turtle items; shown as `convergence`).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub boost: i64,
}

fn is_zero(n: &i64) -> bool {
    *n == 0
}

/// One growth tree square: id, grid position, effect (1 ATK, 2 DEF, 3 HP; 0 = start), value, and
/// the square before it.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeSquare {
    pub id: String,
    pub pos: (i64, i64),
    pub effect: i64,
    pub param: i64,
    pub before: String,
}

/// The growth tree of a statue (docs/research/13-menus.md §3). The client has no tree data and
/// none survives, so the layout is ours: from the start square three branches run right, ATK on
/// top, HP in the middle and DEF below; the ATK and DEF branches fork halfway into a second line
/// and the HP branch runs further. Each stat's squares share that stat's total: the wiki's
/// tree-max minus Lv99 stats (else 20% of Lv99). Grid rows 0..4, the start square on row 2.
pub fn tree_squares(d: &StatueDef) -> Vec<TreeSquare> {
    let base = d.stats_at(MAX_LEVEL);
    let total = match &d.max_tree {
        Some(m) => Stats {
            hp: (m.hp - base.hp).max(0),
            atk: (m.atk - base.atk).max(0),
            def: (m.def - base.def).max(0),
        },
        None => Stats {
            hp: base.hp / 5,
            atk: base.atk / 5,
            def: base.def / 5,
        },
    };
    // (id, x, y, effect 1 ATK / 2 DEF / 3 HP, parent id)
    let mut layout: Vec<(String, i64, i64, i64, String)> = Vec::new();
    for (tag, row, fork_row, effect, len) in
        [("A", 1, 0, 1, 5), ("H", 2, -1, 3, 7), ("D", 3, 4, 2, 5)]
    {
        let mut parent = "S".to_string();
        for x in 1..=len {
            let id = format!("{tag}{x}");
            layout.push((id.clone(), x, row, effect, parent.clone()));
            if x == 3 && fork_row >= 0 {
                // The fork: two squares one row out, from the third square of the branch.
                layout.push((format!("{tag}f1"), 4, fork_row, effect, id.clone()));
                layout.push((format!("{tag}f2"), 5, fork_row, effect, format!("{tag}f1")));
            }
            parent = id;
        }
    }
    let counts: Vec<i64> = (0..4)
        .map(|e| layout.iter().filter(|s| s.3 == e).count() as i64)
        .collect();
    let share = |stat: i64, e: usize| stat / counts[e].max(1);
    let mut out = vec![TreeSquare {
        id: "S".into(),
        pos: (0, 2),
        effect: 0,
        param: 0,
        before: String::new(),
    }];
    for (id, x, y, effect, before) in layout {
        let param = match effect {
            1 => share(total.atk, 1),
            2 => share(total.def, 2),
            _ => share(total.hp, 3),
        };
        out.push(TreeSquare {
            id,
            pos: (x, y),
            effect,
            param,
            before,
        });
    }
    // Skill level squares (docs/research/26 §4): after the ATK line and its fork, effect 4 (the
    // game's "raises Skill Level" tile), after the DEF line and its fork effect 5 ("raises SP
    // Skill Level"), each +2 levels, for the skills the statue has. Placement and amounts are ours.
    let has = |special: bool| d.dt_skills.iter().any(|s| s.special == special);
    for (id, x, y, effect, before, special) in [
        ("A6", 6, 1, TREE_SKILL, "A5", false),
        ("Af3", 6, 0, TREE_SKILL, "Af2", false),
        ("D6", 6, 3, TREE_SP_SKILL, "D5", true),
        ("Df3", 6, 4, TREE_SP_SKILL, "Df2", true),
    ] {
        if has(special) {
            out.push(TreeSquare {
                id: id.into(),
                pos: (x, y),
                effect,
                param: TREE_SKILL_LEVELS,
                before: before.into(),
            });
        }
    }
    out
}

/// Tree square effects that raise a skill's level (client tile texts Tree_Message_08 / _09).
pub const TREE_SKILL: i64 = 4;
pub const TREE_SP_SKILL: i64 = 5;
/// Levels one skill square adds (ours).
const TREE_SKILL_LEVELS: i64 = 2;
/// Skill level cap (the client interpolates cool turns over levels 0..10).
pub const SKILL_MAX_LEVEL: i64 = 10;

/// The statue's (normal, special) skill levels: 1 plus the opened skill squares.
pub fn skill_levels(o: &OwnedStatue) -> (i64, i64) {
    let mut lv = (1, 1);
    if let Some(d) = find(&o.statue_id) {
        for sq in tree_squares(d).iter().filter(|s| o.tree.contains(&s.id)) {
            match sq.effect {
                TREE_SKILL => lv.0 += sq.param,
                TREE_SP_SKILL => lv.1 += sq.param,
                _ => {}
            }
        }
    }
    (lv.0.min(SKILL_MAX_LEVEL), lv.1.min(SKILL_MAX_LEVEL))
}

/// Stat bonus from the opened squares.
pub fn tree_bonus(o: &OwnedStatue) -> Stats {
    let mut b = Stats {
        hp: 0,
        atk: 0,
        def: 0,
    };
    if let Some(d) = find(&o.statue_id) {
        for sq in tree_squares(d).iter().filter(|s| o.tree.contains(&s.id)) {
            match sq.effect {
                1 => b.atk += sq.param,
                2 => b.def += sq.param,
                3 => b.hp += sq.param,
                _ => {}
            }
        }
    }
    b
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Team {
    /// 0 = Battle (action), 1 = DtBattle (tactical).
    pub stage_type: i64,
    pub team_num: i64,
    pub main: bool,
    /// Leader, left, right: statue uids (0 = empty).
    pub slots: [i64; 3],
    /// Minds equipped per seat (leader, left, right): (slot seq 1..8, mind uid). They belong to
    /// the seat, not the statue (docs/research/16-minds.md).
    #[serde(default, skip_serializing_if = "seats_empty")]
    pub minds: [Vec<(i64, i64)>; 3],
    /// Assist statues per seat: (seq 1..3, statue uid); seq 1 is the leader assist
    /// (docs/research/19-assists.md). They belong to the seat, like the Minds.
    #[serde(default, skip_serializing_if = "seats_empty")]
    pub assists: [Vec<(i64, i64)>; 3],
}

/// Assists per seat (client GameConst.MaxEquipAssistNum).
pub const MAX_ASSISTS: i64 = 3;

fn seats_empty(m: &[Vec<(i64, i64)>; 3]) -> bool {
    m.iter().all(Vec::is_empty)
}

/// A statue's Mind capacity (CP; `StatueInfo.mindCapacity`, ours: three 4-star or four 3-star
/// Minds at cost 3 x rarity).
pub const MIND_CAPACITY: i64 = 40;
/// Mind slots open per seat: the four golden frames (seq 1-4), the skill frame (5) and the SP
/// skill frame (6) (client `GameConst.GetMindSeqType`: 1-4 golden, 5 skill, 6 SP skill, 7-8 sub;
/// docs/research/26-links-and-skills.md §3).
pub const MIND_SLOTS_OPEN: i64 = 6;
/// The golden frames: ability Minds (mindType 1) only.
pub const MIND_GOLDEN_SLOTS: i64 = 4;
/// The skill frame takes a Mind whose skill is a normal skill (SkillType 2)...
pub const MIND_SKILL_SLOT: i64 = 5;
/// ...and the SP skill frame one whose skill is special (SkillType 3)
/// (`MindSelectUIController.IsPossibleEquip` 0xD66E50).
pub const MIND_SP_SKILL_SLOT: i64 = 6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Collection {
    #[serde(default)]
    pub statues: Vec<OwnedStatue>,
    #[serde(default)]
    pub teams: Vec<Team>,
    /// The Arena's own five teams (docs/research/23-arena.md; stage type 1, the Arena battle is
    /// tactical). Older saves get them on their next login.
    #[serde(default)]
    pub arena_teams: Vec<Team>,
    #[serde(default)]
    pub minds: Vec<OwnedMind>,
    /// Item counts by item id (Build 21; `server/content/items.json`).
    #[serde(default)]
    pub items: std::collections::BTreeMap<i64, i64>,
    /// The starting item stock was granted (once; consumed items are not refilled).
    #[serde(default)]
    pub items_granted: bool,
}

impl Collection {
    pub fn item_count(&self, id: i64) -> i64 {
        self.items.get(&id).copied().unwrap_or(0)
    }

    /// Add (or with a negative `n`, take) items; never below zero.
    pub fn add_item(&mut self, id: i64, n: i64) {
        let c = self.items.entry(id).or_insert(0);
        *c = (*c + n).max(0);
    }

    /// Grant every obtainable statue not owned yet and make sure the teams exist. Returns true if
    /// anything changed (the caller saves).
    pub fn ensure(&mut self, now: &str) -> bool {
        let mut changed = false;
        if !self.items_granted {
            for (id, n) in crate::items::starting_stock() {
                self.add_item(id, n);
            }
            self.items_granted = true;
            changed = true;
        }
        let mut next = self.statues.iter().map(|s| s.uid).max().unwrap_or(0) + 1;
        for d in statues() {
            if !self.statues.iter().any(|s| s.statue_id == d.statue_id) {
                self.statues.push(OwnedStatue {
                    uid: next,
                    statue_id: d.statue_id.clone(),
                    level: 1,
                    exp: 0,
                    get_date: now.to_string(),
                    tree: Vec::new(),
                    boost: 0,
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
                    protected: false,
                    level: 1,
                    exp: 0,
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
                    minds: Default::default(),
                    assists: Default::default(),
                });
                changed = true;
            }
        }
        // The Arena roster: team 1 starts as a copy of the tactical main team's members (ours).
        for team_num in 1..=TEAMS_PER_TYPE {
            if self.arena_teams.iter().any(|t| t.team_num == team_num) {
                continue;
            }
            let slots = match team_num {
                1 => main_team(self, 1).map(|t| t.slots).unwrap_or([0; 3]),
                _ => [0; 3],
            };
            self.arena_teams.push(Team {
                stage_type: 1,
                team_num,
                main: team_num == 1,
                slots,
                minds: Default::default(),
                assists: Default::default(),
            });
            changed = true;
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
                    .find(|s| s.statue_id == *id && can_be_member(&s.statue_id))
                    .map(|s| s.uid)
            })
            .take(3)
            .collect();
        for s in &self.statues {
            if picked.len() >= 3 {
                break;
            }
            if !picked.contains(&s.uid) && can_be_member(&s.statue_id) {
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

    /// An owned statue that may sit in a team seat (known and not assist-only).
    pub fn member_statue(&self, uid: i64) -> Option<&OwnedStatue> {
        self.statue(uid).filter(|o| can_be_member(&o.statue_id))
    }

    /// Clear team slots that point at nothing usable and refill a main team without a leader.
    fn repair_teams(&mut self) -> bool {
        let mut changed = false;
        let usable: std::collections::HashSet<i64> = self
            .statues
            .iter()
            .filter(|o| can_be_member(&o.statue_id))
            .map(|o| o.uid)
            .collect();
        let starter = self.starter_slots();
        for t in &mut self.arena_teams {
            for s in &mut t.slots {
                if *s != 0 && !usable.contains(s) {
                    *s = 0;
                    changed = true;
                }
            }
        }
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
    let mut st = d.map(|d| d.stats_at(o.level)).unwrap_or(Stats {
        hp: 1,
        atk: 1,
        def: 1,
    });
    let tb = tree_bonus(o);
    st.hp += tb.hp;
    st.atk += tb.atk;
    st.def += tb.def;
    let sid = &o.statue_id;
    json!({
        "statueId": sid, "name": format!("STATUE_NAME_{sid}"), "nameSortIndex": 0,
        "rare": rare, "rareMax": rare, "appearedPart": part,
        "description": format!("PROF_TEXT_{sid}"), "specialActionText": "",
        "charaTypeId": d.map(|d| d.psid()).unwrap_or(0),
        "tribeTypeList": [], "statueType": stype, "oraGauge": 0, "standRange": 0,
        "modelDataList": models, "mindCapacity": MIND_CAPACITY, "useAttr": 1, "attckType": 0,
        "addProfileName": "", "addProfile": "",
        "maxLevel": MAX_LEVEL, "sellVm": 0, "level": o.level, "bounds": 0, "exp": o.exp,
        "hp": st.hp, "eng": 0, "atk": st.atk, "def": st.def,
        "skillList": [], "abilityList": [], "leaderAbilityInfo": {},
        "dtSkillList": [], "dtAbilityList": [], "dtLeaderAbilityInfo": leader_ability(sid),
        "characteristicList": [], "assistAttr": d.is_some_and(|d| d.assist_only) as i64, "assistSkillInfo": {},
        "assistAbilityList": assist_ability_list(sid), "dtAssistSkillInfo": assist_skill(sid),
        "dtAssistAbilityList": dt_assist_ability_list(sid),
        "speed": 100, "evasion": 0, "hitrate": 100, "criticalRate": 5,
        "stEffRateInfo": {}, "maxSp": 100, "useStageType": 0, "levelUpTable": [],
        "statueuId": o.uid, "treeuId": "", "newFlg": 0, "useFlg": 0, "protectedFlg": 0,
        "limitReleaseCount": 0, "convergence": o.boost, "getDate": o.get_date,
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
    v["skillList"] = json!(action_skill_list(o));
    let mut list = vec![basic_attack(&attack_skill(&o.statue_id))];
    let (normal, special) = skill_levels(o);
    for s in find(&o.statue_id)
        .map(|d| d.dt_skills.as_slice())
        .unwrap_or(&[])
    {
        list.push(dt_skill_info(s, if s.special { special } else { normal }));
    }
    v["dtSkillList"] = json!(list);
    v
}

/// Action-battle skills per statue (server/content/action_skills.json, docs/research/30).
pub const ACTION_SKILLS_JSON: &str = include_str!("../../content/action_skills.json");

#[derive(Debug, Clone, Deserialize)]
pub struct ActionSkillRef {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ActionSkills {
    pub attack: String,
    pub skill: Option<ActionSkillRef>,
    pub sp: Option<ActionSkillRef>,
    pub finisher: Option<ActionSkillRef>,
}

pub fn action_skills(statue_id: &str) -> Option<&'static ActionSkills> {
    static CELL: std::sync::OnceLock<std::collections::HashMap<String, ActionSkills>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            statues: std::collections::HashMap<String, ActionSkills>,
        }
        serde_json::from_str::<Doc>(ACTION_SKILLS_JSON)
            .map(|d| d.statues)
            .unwrap_or_default()
    })
    .get(statue_id)
}

/// The action battle's `skillList`, read by index (`CharacterStatus.SetStatueInfo` 0xF17DC4 with
/// `GameConst.GetSkillTypeFromSkillArray` 0xDB5254): 0 attack, 1 skill 1, 2 skill 2, 3 SP 1, 4 SP 2.
/// Ids only (`isInit` false): the client fills name, type, cool-down and uses from
/// skill_data_table. An entry without a skill id is skipped by the client (an empty name or a level
/// under 1 is, `SetSkillInfo` 0xF18838), so unused slots are `{}`. The finisher is the second SP
/// skill, unlocked at rarity 5 and above (docs/research/30).
pub fn action_skill_list(o: &OwnedStatue) -> Vec<Value> {
    let Some(a) = action_skills(&o.statue_id) else {
        return Vec::new();
    };
    let (normal, special) = skill_levels(o);
    let info = |id: &str, level: i64| {
        json!({ "skillId": id, "skillLevel": level.clamp(1, SKILL_MAX_LEVEL), "name": "", "explanation": "",
                "skillType": 0, "bottunType": 0, "coolDown": 0, "limitCount": 0,
                "magnificationLv01": 0, "magnificationLv10": 0, "pvpMagnificationLv01": 0,
                "pvpMagnificationLv10": 0, "skillParamList": [], "isInit": false })
    };
    let rare = find(&o.statue_id).map(|d| d.rarity()).unwrap_or(4);
    vec![
        info(&a.attack, 1),
        a.skill.as_ref().map_or(json!({}), |x| info(&x.id, normal)),
        json!({}),
        a.sp.as_ref().map_or(json!({}), |x| info(&x.id, special)),
        a.finisher
            .as_ref()
            .filter(|_| rare >= 5)
            .map_or(json!({}), |x| info(&x.id, special)),
    ]
}

/// A tactical skill as a `SkillInfo`. The client takes type, button and name from the table row;
/// the level must be >= 1. The level sets the cool turn (`SkillParameter` ctor 0xD8C894:
/// dtMinCoolTurn + CeilToInt((dtMaxCoolTurn - dtMinCoolTurn) / 10 x level)).
fn dt_skill_info(s: &DtSkill, level: i64) -> Value {
    json!({ "skillId": s.id, "skillLevel": level.clamp(1, SKILL_MAX_LEVEL), "name": "", "explanation": "",
            "skillType": if s.special { 3 } else { 2 }, "bottunType": s.button, "coolDown": 0,
            "limitCount": 0, "magnificationLv01": 0, "magnificationLv10": 0,
            "pvpMagnificationLv01": 0, "pvpMagnificationLv10": 0, "skillParamList": [],
            "isInit": false })
}

/// The statue's own attack (its `dtAttack`), else our fallback `JDR_ATK_00`.
pub fn attack_skill(statue_id: &str) -> String {
    find(statue_id)
        .and_then(|d| d.dt_attack.clone())
        .unwrap_or_else(|| BASIC_ATTACK.to_string())
}

/// Characters whose attacks fire bullets (their models' bullet FSMs; docs/research/12 §2): the
/// 3-digit skill prefixes of statues 3110, 4160, 4180/4181, 4240, 5220 and 5300.
const RANGED_PREFIXES: &[&str] = &["311", "416", "418", "424", "522", "530"];

/// Whether a statue fights at range (its skill prefix is one of the bullet users').
pub fn is_ranged(statue_id: &str) -> bool {
    statue_id
        .get(..3)
        .is_some_and(|p| RANGED_PREFIXES.contains(&p))
}

/// Where a skill's user stops, as the client's (dtRangeMin, dtRangeMax) in centimetres
/// (`CharacterProxy.SetSkillTargetPos` 0xEE9304: farther than max walks to max, nearer than min
/// backs off to min; 0/0 walks into the target). The attack itself then closes in to a fixed
/// 1.05 m from the target (constant 0x223E5A4): a Stand appears on its user and rushes up to
/// about 2.1 m forward, so Stand users stop 2-2.5 m away (closer, the Stand overlapped its
/// user); units without a Stand lunge themselves and stop 1.2-1.5 m away, outside the snap
/// distance; bullet users 4-8 m; heals on allies (target types 3-5) move only when over 10 m
/// away. docs/research/12 §6. Values are ours.
pub fn skill_range(skill_id: &str, target_type: i64) -> (i64, i64) {
    let prefix = skill_id
        .strip_prefix("SKILL_")
        .and_then(|s| s.get(..3))
        .unwrap_or("");
    if (3..=5).contains(&target_type) {
        (0, 1000)
    } else if RANGED_PREFIXES.contains(&prefix) {
        (400, 800)
    } else if stand_prefix(prefix) {
        (200, 250)
    } else {
        (120, 150)
    }
}

/// Whether the statues sharing this 3-digit skill prefix fight with a Stand (a Stand model in
/// their modelDataList).
fn stand_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && statues()
            .iter()
            .any(|d| d.statue_id.starts_with(prefix) && d.model_data_list.len() > 1)
}

/// The base skill table (server/content/dt_skills_base.json) plus one attack row per distinct
/// statue attack id: a copy of `JDR_ATK_00` with `dtSkillId` and `viewSkillId` set to the id, so the
/// client plays the statue's own attack action set (skill number 99 means ATTACK).
pub const DT_SKILLS_BASE: &str = include_str!("../../content/dt_skills_base.json");

pub fn dt_skill_table() -> Value {
    let mut doc: Value = serde_json::from_str(DT_SKILLS_BASE).unwrap_or(Value::Null);
    if let Some(src) = doc["sources"].as_array_mut() {
        src.push(json!({ "source": "jdr-default",
            "note": "attack rows: one per dtAttack id in server/content/statues.json (picked from the player's own action sets by tools/content/build_statues.py); the id prefix is the first 3 digits of the statueId, shared by models with the same prefix (the client plays the action set of the unit's own model)" }));
    }
    let template = doc["data"]["skillDataList"]
        .as_array()
        .and_then(|l| l.iter().find(|r| r["dtSkillId"] == BASIC_ATTACK))
        .cloned()
        .unwrap_or(Value::Null);
    let ids: std::collections::BTreeSet<String> = statues()
        .iter()
        .filter_map(|d| d.dt_attack.clone())
        .collect();
    if let Some(list) = doc["data"]["skillDataList"].as_array_mut() {
        for id in ids {
            let mut row = template.clone();
            row["dtSkillId"] = json!(id);
            row["viewSkillId"] = json!(id);
            let (min, max) = skill_range(&id, 1);
            row["dtRangeMin"] = json!(min);
            row["dtRangeMax"] = json!(max);
            list.push(row);
        }
    }
    // Normal and special skills (docs/research/12 §3): one row per skill id, one category row per
    // category label. Numbers are ours: wait 100 (ends the turn), cool-down 2 turns (special 3,
    // with 3 uses), the same hit and critical rates as the attack, no SP cost, and Reversal gauge
    // 15% (special 20%; the attack's is 10%, docs/research/11 §5). Skill Minds add the rows of
    // their skills that no statue has (docs/research/26 §3).
    if let Some(src) = doc["sources"].as_array_mut() {
        src.push(json!({ "source": "jdr-research", "doc": "docs/research/26-links-and-skills.md",
            "note": "Mind skill rows (server/content/mind_skills.json), time-stop sub categories (the client's DTSC_WORLD_* categories: ProcessType 4, turn 1/2) and cool turns by skill level (dtMinCoolTurn at level 0, dtMaxCoolTurn at level 10; the values are ours)" }));
    }
    let mut skills = std::collections::BTreeMap::new();
    let mut cats = std::collections::BTreeMap::new();
    for s in statues()
        .iter()
        .flat_map(|d| d.dt_skills.iter())
        .chain(mind_skills().iter().map(|m| &m.skill))
    {
        skills.entry(s.id.clone()).or_insert(s);
        cats.entry(s.category.clone()).or_insert(s);
    }
    let mut worlds = std::collections::BTreeSet::new();
    if let Some(list) = doc["data"]["skillDataList"].as_array_mut() {
        for s in skills.values() {
            let mut row = template.clone();
            let (cool, uses) = dt_cool_turn(s);
            let view = s.view_id();
            let sub = time_stop(&s.id).unwrap_or("");
            if !sub.is_empty() {
                worlds.insert(sub);
            }
            for (k, v) in [
                ("dtSkillId", json!(s.id)),
                ("viewSkillId", json!(view)),
                ("dtSkillName", json!(s.name)),
                ("dtSkillProfileText", json!(s.text)),
                ("dtSkillType", json!(if s.special { 3 } else { 2 })),
                ("dtButtonType", json!(s.button)),
                ("damageRank", json!(s.damage_rank)),
                ("dtLimitCount", json!(uses)),
                // Reversal gauge, in percent, added when a player unit picks the skill (ours).
                ("dododoRate", json!(if s.special { 20 } else { 15 })),
                // Cool turn = min + CeilToInt((max - min) / 10 x level): the documented value at
                // level 10 (dt_cool_turn), COOL_TURN_LEVEL_GAIN more at level 0.
                ("dtMinCoolTurn", json!(cool + COOL_TURN_LEVEL_GAIN)),
                ("dtMaxCoolTurn", json!(cool)),
                ("mainSkillCategory", json!(s.category)),
                ("subSkillCategory", json!(sub)),
                ("dtRangeMin", json!(skill_range(&view, s.target_type).0)),
                ("dtRangeMax", json!(skill_range(&view, s.target_type).1)),
            ] {
                row[k] = v;
            }
            list.push(row);
        }
    }
    if let Some(list) = doc["data"]["categoryList"].as_array_mut() {
        for (id, s) in cats {
            list.push(
                json!({ "dtSkillCategoryId": id, "TargetType": s.target_type,
                "ProcessType": s.process_type, "turn": 0, "paramList": [s.attr] }),
            );
        }
        // Time stop (`Character.OnTurnMain` 0xFABE54: a category with ProcessType 4 WORLD calls
        // `TimeStopManager.StartTimeStop(user, turn)`), as the client's own labels name them.
        for id in worlds {
            let turn = match id {
                "DTSC_WORLD_LIGHT" => 1,
                "DTSC_WORLD_MIDDLE" => 2,
                "DTSC_WORLD_HEAVY" => 3,
                _ => 5,
            };
            list.push(
                json!({ "dtSkillCategoryId": id, "TargetType": 5, "ProcessType": 4,
                              "turn": turn, "paramList": [] }),
            );
        }
    }
    doc
}

/// Cool turns a skill loses from level 0 to level 10 (ours; docs/research/26 §4).
pub const COOL_TURN_LEVEL_GAIN: i64 = 2;

/// Documented tactical cool turns per skill id (server/content/dt_cool_turns.json).
pub const DT_COOL_TURNS_JSON: &str = include_str!("../../content/dt_cool_turns.json");

/// A tactical skill's cool turn at the highest skill level and its uses (0 = unlimited).
/// Players listed the cool turn at the highest level (the tactical-era atwiki and the Bahamut forum,
/// docs/research/26 §4): the skill's own value where its name matched, else its family's usual one
/// there: attack skills 3, time stops 6/9/10 for 1/2/3 stopped turns, heals 8, special (SP) skills 5
/// with 3 uses. How much the levels below take off is not documented (COOL_TURN_LEVEL_GAIN, ours).
pub fn dt_cool_turn(s: &DtSkill) -> (i64, i64) {
    static CELL: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    let doc = CELL.get_or_init(|| serde_json::from_str(DT_COOL_TURNS_JSON).unwrap_or(Value::Null));
    let known = &doc["skills"][&s.id];
    let default_uses = if s.special { 3 } else { 0 };
    let uses = known["uses"].as_i64().unwrap_or(default_uses);
    if let Some(ct) = known["coolTurn"].as_i64() {
        return (ct, uses);
    }
    let ct = if s.special {
        5
    } else if let Some(sub) = time_stop(&s.id) {
        match sub {
            "DTSC_WORLD_LIGHT" => 6,
            "DTSC_WORLD_MIDDLE" => 9,
            _ => 10,
        }
    } else if s.category.contains("HEAL") {
        8
    } else {
        3
    };
    (ct, uses)
}

/// The cool turn the client computes for a skill row at a level (`SkillParameter` ctor 0xD8C894).
pub fn cool_turn(min: i64, max: i64, level: i64) -> i64 {
    min + ((max - min) as f64 / 10.0 * level as f64).ceil() as i64
}

// ---------------------------------------------------------------------------------------------
// Skill Minds and time-stop skills (docs/research/26-links-and-skills.md §3, §5).

pub const MIND_SKILLS_JSON: &str = include_str!("../../content/mind_skills.json");

/// A Mind that grants a skill (mindType 2): the skill row it names and the statues that can use
/// it (`limitCharaList`, statueIds; the unit plays the skill's action set on its own model).
#[derive(Debug, Clone, Deserialize)]
pub struct MindSkill {
    #[serde(rename = "mindId")]
    pub mind_id: String,
    pub skill: DtSkill,
    pub statues: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MindSkillsDoc {
    minds: Vec<MindSkill>,
    #[serde(rename = "timeStop", default)]
    time_stop: std::collections::BTreeMap<String, String>,
}

fn mind_skills_doc() -> &'static MindSkillsDoc {
    static CELL: std::sync::OnceLock<MindSkillsDoc> = std::sync::OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str(MIND_SKILLS_JSON).unwrap_or_default())
}

pub fn mind_skills() -> &'static [MindSkill] {
    &mind_skills_doc().minds
}

pub fn mind_skill(mind_id: &str) -> Option<&'static MindSkill> {
    mind_skills().iter().find(|m| m.mind_id == mind_id)
}

/// The time-stop category of a skill (`DTSC_WORLD_<n>`), when its text says the user acts in
/// stopped time.
pub fn time_stop(skill_id: &str) -> Option<&'static str> {
    mind_skills_doc()
        .time_stop
        .get(skill_id)
        .map(String::as_str)
}

/// A skill Mind's skill as a `SkillInfo` at the Mind's level ("Raising the Mind Level improves
/// the Skill", MIND_TEXT_<id>). The client reads `skillType` from here, not from the table: 2
/// puts the skill on the second normal button, 3 on the second special one
/// (`SkillStatusTable.CreateSkillStatus` 0xD8E46C), and the formation lets it into slot 5 or 6.
/// `skillInfo` (the action battle's) names the classic row `SKILL_<p>_<NN>`.
fn mind_skill_info(m: &MindSkill, level: i64, dt: bool) -> Value {
    let id = if dt {
        m.skill.id.clone()
    } else {
        m.skill.view_id()
    };
    json!({ "skillId": id, "skillLevel": level.clamp(1, SKILL_MAX_LEVEL), "name": "",
            "explanation": "", "skillType": if m.skill.special { 3 } else { 2 },
            "bottunType": m.skill.button, "coolDown": 0, "limitCount": 0,
            "magnificationLv01": 0, "magnificationLv10": 0, "pvpMagnificationLv01": 0,
            "pvpMagnificationLv10": 0, "skillParamList": [], "isInit": false })
}

/// Whether a Mind may sit in this seat for this statue: a skill Mind only when the statue is on
/// its list (the client compares `limitCharaList` with the statueId,
/// `GameConst.IsEnableEquipMind` 0xDB5720), and only in the frame of its kind; an ability Mind
/// only in the golden frames.
pub fn mind_fits(mind_id: &str, seq: i64, statue_id: &str) -> bool {
    match mind_skill(mind_id) {
        Some(m) => {
            let frame = if m.skill.special {
                MIND_SP_SKILL_SLOT
            } else {
                MIND_SKILL_SLOT
            };
            seq == frame && m.statues.iter().any(|s| s == statue_id)
        }
        None => (1..=MIND_GOLDEN_SLOTS).contains(&seq),
    }
}

fn basic_attack(id: &str) -> Value {
    json!({ "skillId": id, "skillLevel": 1, "name": "", "explanation": "",
            "skillType": 1, "bottunType": 1, "coolDown": 0, "limitCount": 0,
            "magnificationLv01": 0, "magnificationLv10": 0, "pvpMagnificationLv01": 0,
            "pvpMagnificationLv10": 0, "skillParamList": [], "isInit": false })
}

fn empty_setting() -> Value {
    json!({ "statueuId": 0, "statueInfo": { "statueId": "" }, "equipMindList": [],
            "mindLinkList": [], "assistStatueList": [], "AiId": "" })
}

fn setting(c: &Collection, t: &Team, seat: usize) -> Value {
    let uid = t.slots[seat];
    match c.known_statue(uid) {
        Some(o) => {
            // Only filled slots: the client counts the list against the open slots. A skill Mind
            // left in a seat whose statue cannot use it (the seat's statue changed) is not sent:
            // the battle would add its skill to a model without that action set.
            let minds = seat_minds(c, &t.minds[seat], &o.statue_id);
            let equipped: Vec<Value> = minds
                .iter()
                .map(|(seq, m)| {
                    let mut v = mind_info(m);
                    v["seq"] = json!(seq);
                    v["useFlg"] = json!(1);
                    v
                })
                .collect();
            let links = mind_link_list(&minds);
            // Each assist is a whole StatueInfo plus its seq; a statue that is now a main member of
            // the team is not shown as an assist.
            let assists: Vec<Value> = t.assists[seat]
                .iter()
                .filter(|(_, a)| !t.slots.contains(a))
                .filter_map(|(seq, a)| {
                    let mut v = battle_statue_info(c.known_statue(*a)?);
                    v["seq"] = json!(seq);
                    Some(v)
                })
                .collect();
            // The tactical battle never applies assist abilities or Mind links itself
            // (19-assists §5, 26 §2), so the active ones ride on the seat statue's own ability
            // list.
            let mut info = battle_statue_info(o);
            let mut abilities = active_assist_abilities(&assists);
            abilities.extend(
                links
                    .iter()
                    .filter_map(|l| Some(dt_ability(l["abilityInfo"]["abilityId"].as_str()?))),
            );
            info["dtAbilityList"] = json!(abilities);
            json!({ "statueuId": uid, "statueInfo": info, "equipMindList": equipped,
                    "mindLinkList": links, "assistStatueList": assists, "AiId": "" })
        }
        None => empty_setting(),
    }
}

/// A seat's Minds that the statue can use, in slot order: owned, in an open slot, and fitting
/// it ([`mind_fits`]).
pub fn seat_minds<'a>(
    c: &'a Collection,
    seat: &[(i64, i64)],
    statue_id: &str,
) -> Vec<(i64, &'a OwnedMind)> {
    seat.iter()
        .filter_map(|(seq, u)| {
            let m = c.minds.iter().find(|m| m.uid == *u)?;
            mind_fits(&m.mind_id, *seq, statue_id).then_some((*seq, m))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Mind links (docs/research/26-links-and-skills.md §2).

pub const MIND_LINKS_JSON: &str = include_str!("../../content/mind_links.json");

#[derive(Debug, Clone, Deserialize)]
pub struct MindLinkDef {
    #[serde(rename = "mindLinkId")]
    pub mind_link_id: String,
    /// Ability id by number of linked Minds ("2".."4").
    pub abilities: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct MindLinksDoc {
    links: Vec<MindLinkDef>,
    /// Mind id -> link id.
    minds: std::collections::BTreeMap<String, String>,
}

fn mind_links_doc() -> &'static MindLinksDoc {
    static CELL: std::sync::OnceLock<MindLinksDoc> = std::sync::OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str(MIND_LINKS_JSON).unwrap_or_default())
}

pub fn mind_link_defs() -> &'static [MindLinkDef] {
    &mind_links_doc().links
}

/// The link a Mind belongs to, if any.
pub fn mind_link_of(mind_id: &str) -> Option<&'static MindLinkDef> {
    let doc = mind_links_doc();
    let id = doc.minds.get(mind_id)?;
    doc.links.iter().find(|l| &l.mind_link_id == id)
}

/// A seat's `mindLinkList`: one `MindLinkInfo` per link with at least two equipped Minds:
/// `dataList` holds their minduIds (the formation draws its link lines between the frames whose
/// Mind has one of them, `StatueMindUIController.SetupEnableMindLinkEffect` 0xE6CA64), and the
/// ability for that many Minds goes inline (`isInit` true): `MindLinkInfo.abilityInfo` is an
/// action-battle `AbilityInfo`, which looks in a table we do not ship. `MindLinkInfo.IsValid`
/// 0xD61530 needs the id, a non-empty `dataList` and a valid ability; an empty id with a null
/// `dataList` throws.
pub fn mind_link_list(minds: &[(i64, &OwnedMind)]) -> Vec<Value> {
    let mut by_link: std::collections::BTreeMap<&str, (&MindLinkDef, Vec<i64>)> =
        Default::default();
    for (_, m) in minds {
        if let Some(l) = mind_link_of(&m.mind_id) {
            by_link
                .entry(l.mind_link_id.as_str())
                .or_insert((l, Vec::new()))
                .1
                .push(m.uid);
        }
    }
    by_link
        .into_values()
        .filter_map(|(l, uids)| {
            let n = uids.len().min(4);
            let ability = l.abilities.get(&n.to_string())?;
            Some(json!({ "mindLinkId": l.mind_link_id, "dataList": uids,
                         "abilityInfo": inline_ability(ability) }))
        })
        .collect()
}

impl Collection {
    /// Mind uids equipped anywhere (story and Arena teams).
    pub fn equipped_minds(&self) -> std::collections::HashSet<i64> {
        self.teams
            .iter()
            .chain(&self.arena_teams)
            .flat_map(|t| t.minds.iter().flatten().map(|(_, u)| *u))
            .collect()
    }
}

/// The statue's tactical leader ability, when dtbattle_ability_data_table has its row
/// `DT_LEAD_<statueId>` (docs/research/12 §4). The client takes the name and parameters from the
/// table (`isInit` false) but needs a list here, and a level of 1..10 (values are interpolated from
/// the wiki's level-1 value to its MAX value at level 10); level 1 is the wiki's base value.
pub fn leader_ability(statue_id: &str) -> Value {
    static IDS: std::sync::OnceLock<std::collections::HashSet<String>> = std::sync::OnceLock::new();
    let ids = IDS.get_or_init(|| {
        let js = crate::masterdata::TABLES
            .iter()
            .find(|(n, _)| *n == "dtbattle_ability_data_table")
            .map(|(_, j)| *j)
            .unwrap_or("{}");
        serde_json::from_str::<Value>(js)
            .ok()
            .and_then(|v| v["data"]["paramList"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|r| r["abilityId"].as_str().map(str::to_string))
            .collect()
    });
    let id = format!("DT_LEAD_{statue_id}");
    if ids.contains(&id) {
        json!({ "abilityId": id, "abilityLevel": 1, "abilityName": "", "abilityParamList": [],
                "isInit": false })
    } else {
        json!({})
    }
}

// ---------------------------------------------------------------------------------------------
// Assist abilities and the assist skill (docs/research/19-assists.md §5).

pub const ASSIST_ABILITIES_JSON: &str = include_str!("../../content/assist_abilities.json");

/// One `{statueType, count}` term of an assist ability's activation condition (`Jjs.CondData`):
/// met when the seat's assists hold at least `count` statues of the type.
#[derive(Debug, Clone, Deserialize)]
pub struct AssistCond {
    #[serde(rename = "statueType")]
    pub statue_type: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistAbility {
    #[serde(rename = "abilityId")]
    pub ability_id: String,
    pub conditions: Vec<AssistCond>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistDef {
    #[serde(rename = "statueId")]
    pub statue_id: String,
    pub abilities: Vec<AssistAbility>,
}

pub fn assist_defs() -> &'static [AssistDef] {
    static CELL: std::sync::OnceLock<Vec<AssistDef>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            statues: Vec<AssistDef>,
        }
        serde_json::from_str::<Doc>(ASSIST_ABILITIES_JSON)
            .map(|d| d.statues)
            .unwrap_or_default()
    })
}

/// A statue's assist abilities, in the order both client lists carry them.
pub fn assist_abilities(statue_id: &str) -> &'static [AssistAbility] {
    assist_defs()
        .iter()
        .find(|d| d.statue_id == statue_id)
        .map(|d| d.abilities.as_slice())
        .unwrap_or(&[])
}

/// The rows of dtbattle_ability_data_table by id.
fn ability_rows() -> &'static std::collections::HashMap<String, Value> {
    static CELL: std::sync::OnceLock<std::collections::HashMap<String, Value>> =
        std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        let js = crate::masterdata::TABLES
            .iter()
            .find(|(n, _)| *n == "dtbattle_ability_data_table")
            .map(|(_, j)| *j)
            .unwrap_or("{}");
        serde_json::from_str::<Value>(js)
            .ok()
            .and_then(|v| v["data"]["paramList"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| Some((r["abilityId"].as_str()?.to_string(), r)))
            .collect()
    })
}

/// `assistAbilityList`: action-battle `AbilityInfo`s, read by the ability cards and the action
/// battle. Their `SetupFromAbilityData` looks in the action-battle ability table, which we do not
/// ship, so the name and parameters go inline (`isInit` true; parameters named `paramListLv…`).
pub fn assist_ability_list(statue_id: &str) -> Vec<Value> {
    assist_abilities(statue_id)
        .iter()
        .map(|a| inline_ability(&a.ability_id))
        .collect()
}

/// An action-battle `AbilityInfo` with the name and parameters of a dtbattle_ability_data_table
/// row inline (`isInit` true; the parameters are named `paramListLv…`).
pub fn inline_ability(ability_id: &str) -> Value {
    let row = ability_rows().get(ability_id);
    let params: Vec<Value> = row
        .and_then(|r| r["abilityParamList"].as_array())
        .into_iter()
        .flatten()
        .map(|p| {
            let o: serde_json::Map<String, Value> = p
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, v)| {
                    let k = match k.strip_prefix("paramLv") {
                        Some(rest) => format!("paramListLv{rest}"),
                        None => k.clone(),
                    };
                    (k, v.clone())
                })
                .collect();
            Value::Object(o)
        })
        .collect();
    let name = row.map(|r| r["abilityName"].clone()).unwrap_or(json!(""));
    json!({ "abilityId": ability_id, "abilityLevel": 1, "abilityName": name,
            "abilityParamList": params, "isInit": true })
}

/// A tactical `DtAbilityInfo`; the client fills it from dtbattle_ability_data_table.
fn dt_ability(id: &str) -> Value {
    json!({ "abilityId": id, "abilityLevel": 1, "abilityName": "", "abilityParamList": [],
            "isInit": false })
}

/// `dtAssistAbilityList`: the same abilities in the same order, as tactical `DtAbilityInfo`s.
/// The client finds a condition's index in this list and reads that index from
/// `assistAbilityList`, so the two lists must match.
pub fn dt_assist_ability_list(statue_id: &str) -> Vec<Value> {
    assist_abilities(statue_id)
        .iter()
        .map(|a| dt_ability(&a.ability_id))
        .collect()
}

/// `dtAssistSkillInfo`: the skill the statue uses as the team's assist character (slot 6, the
/// Assist button). It must name a dtbattle_skill_data_table row (the client throws on a missing
/// one), so it is the statue's own normal skill, else its special, else none (`{}`: no button).
pub fn assist_skill(statue_id: &str) -> Value {
    let skills = find(statue_id)
        .map(|d| d.dt_skills.as_slice())
        .unwrap_or(&[]);
    skills
        .iter()
        .find(|s| !s.special)
        .or_else(|| skills.first())
        .map(|s| dt_skill_info(s, 1))
        .unwrap_or_else(|| json!({}))
}

/// The seat's active assist abilities, found as the client's own check does
/// (`CreateAssistAbilityData` 0x157EE1C): each assist's abilities whose `{statueType, count}`
/// terms the types of the seat's assists meet. They go into the seat statue's `dtAbilityList`.
pub fn active_assist_abilities(assists: &[Value]) -> Vec<Value> {
    assists
        .iter()
        .flat_map(|a| assist_abilities(a["statueId"].as_str().unwrap_or("")))
        .filter(|ab| conditions_met(&ab.conditions, assists))
        .map(|ab| dt_ability(&ab.ability_id))
        .collect()
}

/// `CheckAssistAbilityActive` 0xC93910: every term's count of that statueType among the seat's
/// assists (`CountType` 0x157F3AC, the owner included) is reached; no terms = always active.
fn conditions_met(conds: &[AssistCond], assists: &[Value]) -> bool {
    conds.iter().all(|c| {
        let n = assists
            .iter()
            .filter(|a| a["statueType"].as_i64() == Some(c.statue_type))
            .count() as i64;
        n >= c.count
    })
}

/// `assist_ability_condition_table` rows: one per statue with assist abilities, keyed by statueId.
pub fn assist_condition_rows() -> Vec<Value> {
    assist_defs()
        .iter()
        .filter(|d| !d.abilities.is_empty())
        .map(|d| {
            let conds: Vec<Value> = d
                .abilities
                .iter()
                .map(|a| {
                    let terms: Vec<Value> = a
                        .conditions
                        .iter()
                        .map(|c| json!({ "statueType": c.statue_type, "count": c.count }))
                        .collect();
                    json!({ "assistAbilityId": a.ability_id, "assistAbilityCondition": terms })
                })
                .collect();
            json!({ "statueId": d.statue_id, "conditionTableData": conds })
        })
        .collect()
}

/// `TeamInfo` for one team.
pub fn team_info(c: &Collection, t: &Team) -> Value {
    json!({
        "teamNum": t.team_num, "stageType": t.stage_type, "mainFlg": t.main as i64, "useFlg": 1,
        "teamOrder": {
            "leaderStatue": setting(c, t, 0),
            "memberLeftStatue": setting(c, t, 1),
            "memberRightStatue": setting(c, t, 2)
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
            // facing +Z, 40 degrees) and the leader 1.8 m ahead, right of centre, turned towards the
            // camera: thighs up with the head in the upper quarter, as in the original's footage.
            // Team view: the slot anchors already face the camera; each statue's scale and offset put
            // its posed head at the same height in every slot (docs/research/11 §1, 12 §7, 31 §5).
            json!({
                "statueId": d.statue_id, "myPageId": ids[h as usize % ids.len()],
                "myPageCameraType": "CameraIdSet0",
                "myPagePos": {
                    "x": d.my_page_pos[0] as f64, "y": d.my_page_pos[1] as f64, "z": d.my_page_pos[2] as f64
                },
                "myPageRotation": { "x": 0.0, "y": 192.5, "z": 0.0 },
                "teamScaleScalar": d.team_scale as f64,
                "teamPos": { "x": d.team_pos[0] as f64, "y": d.team_pos[1] as f64, "z": d.team_pos[2] as f64 },
                "mindFrameID": 1, "pvpAiId": 1, "pvpAiAutoBattleAiId": 1,
                // The result screen shows this label (skipped when empty).
                "battleResultComment": d.result_comment.clone().unwrap_or_default()
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

    /// Assists (docs/research/19-assists.md §5): both ability lists match, the seat statue carries
    /// its assists' active abilities, and the assist skill is one of the statue's own skills.
    #[test]
    fn assist_abilities_and_skill() {
        let mut c = Collection::default();
        c.ensure("2026-11-14 00:00:00");
        for d in statues() {
            let o = OwnedStatue {
                statue_id: d.statue_id.clone(),
                level: 1,
                ..Default::default()
            };
            let v = statue_info(&o);
            let (cl, dt) = (
                v["assistAbilityList"].as_array().unwrap(),
                v["dtAssistAbilityList"].as_array().unwrap(),
            );
            assert!(
                !dt.is_empty(),
                "{}: every statue has an assist ability",
                d.statue_id
            );
            assert_eq!(cl.len(), dt.len(), "{}", d.statue_id);
            for (a, b) in cl.iter().zip(dt) {
                assert_eq!(a["abilityId"], b["abilityId"], "same order");
                assert_eq!(a["isInit"], true);
                assert!(!a["abilityParamList"].as_array().unwrap().is_empty());
                assert!(a["abilityParamList"][0]["paramListLv1_0"].as_i64().unwrap() > 0);
                assert!(a["abilityName"].as_str().unwrap().starts_with("ABI_AST_"));
                assert_eq!(b["abilityParamList"], json!([]), "never null");
            }
            let s = &v["dtAssistSkillInfo"];
            if d.dt_skills.is_empty() {
                assert_eq!(*s, json!({}));
            } else {
                assert!(d.dt_skills.iter().any(|k| s["skillId"] == k.id.as_str()));
                assert_eq!(s["skillLevel"], 1);
            }
        }
        // A team with two assists on the leader seat.
        let mut t = c.teams[0].clone();
        // (assists whose abilities have no activation condition, so they apply in any team)
        let free: Vec<i64> = c
            .statues
            .iter()
            .filter(|o| !t.slots.contains(&o.uid))
            .filter(|o| {
                let abs = assist_abilities(&o.statue_id);
                !abs.is_empty() && abs.iter().all(|a| a.conditions.is_empty())
            })
            .map(|o| o.uid)
            .take(2)
            .collect();
        t.assists[0] = vec![(1, free[0]), (2, free[1])];
        let info = team_info(&c, &t);
        let seat = &info["teamOrder"]["leaderStatue"];
        let got: Vec<&Value> = seat["statueInfo"]["dtAbilityList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| &a["abilityId"])
            .collect();
        let want: Vec<Value> = free
            .iter()
            .flat_map(|u| assist_abilities(&c.statue(*u).unwrap().statue_id))
            .map(|a| json!(a.ability_id))
            .collect();
        assert_eq!(got, want.iter().collect::<Vec<_>>());
        assert_eq!(seat["assistStatueList"].as_array().unwrap().len(), 2);
        // The other seats have no assists, so no assist abilities.
        let left = &info["teamOrder"]["memberLeftStatue"]["statueInfo"]["dtAbilityList"];
        assert_eq!(*left, json!([]));
        // An assist whose condition asks for two or more statues adds nothing when it is the
        // seat's only assist.
        let picky = c.statues.iter().find(|o| {
            !t.slots.contains(&o.uid)
                && assist_abilities(&o.statue_id)
                    .iter()
                    .any(|a| a.conditions.iter().map(|x| x.count).sum::<i64>() > 1)
        });
        if let Some(o) = picky {
            let mut t2 = c.teams[0].clone();
            t2.assists[0] = vec![(1, o.uid)];
            let info = team_info(&c, &t2);
            let list = &info["teamOrder"]["leaderStatue"]["statueInfo"]["dtAbilityList"];
            let ids: Vec<String> = assist_abilities(&o.statue_id)
                .iter()
                .filter(|a| a.conditions.iter().map(|x| x.count).sum::<i64>() > 1)
                .map(|a| a.ability_id.clone())
                .collect();
            for id in ids {
                assert!(
                    !list
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|a| a["abilityId"] == json!(id)),
                    "{id} needs more assists than one"
                );
            }
        }
    }

    #[test]
    fn action_skill_lists_follow_the_client_slots() {
        // skillList by index: 0 attack, 1 skill 1, 2 skill 2, 3 SP 1, 4 SP 2 (docs/research/30);
        // every id is a skill_data_table row, whose type matches the slot.
        let table: Value = serde_json::from_str(include_str!(
            "../../content/masterdata/skill_data_table.json"
        ))
        .unwrap();
        let ty: std::collections::HashMap<String, i64> = table["data"]["paramList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["skillId"].as_str().unwrap().to_owned(),
                    r["skillType"].as_i64().unwrap(),
                )
            })
            .collect();
        let mut with_sp = 0;
        for d in statues() {
            let o = OwnedStatue {
                uid: 1,
                statue_id: d.statue_id.clone(),
                level: 1,
                exp: 0,
                get_date: String::new(),
                tree: Vec::new(),
                boost: 0,
            };
            let list = action_skill_list(&o);
            assert_eq!(list.len(), 5, "{}", d.statue_id);
            let id = |i: usize| list[i]["skillId"].as_str().map(str::to_owned);
            assert!(id(0).unwrap().ends_with("_99"));
            assert_eq!(ty[&id(0).unwrap()], 1, "attack is a combo");
            assert_eq!(list[2], json!({}), "no second skill");
            for i in [1, 3, 4] {
                if let Some(x) = id(i) {
                    assert!(ty.contains_key(&x), "{x} is a table row");
                    assert!(list[i]["skillLevel"].as_i64().unwrap() >= 1);
                }
            }
            if id(3).is_some() {
                with_sp += 1;
            }
        }
        assert!(with_sp > 200, "{with_sp}");
    }

    #[test]
    fn assist_conditions_count_the_seats_assist_types() {
        let assists = [
            json!({ "statueType": 1 }),
            json!({ "statueType": 1 }),
            json!({ "statueType": 3 }),
        ];
        let cond = |statue_type, count| AssistCond { statue_type, count };
        assert!(conditions_met(&[], &assists), "no terms: always active");
        assert!(conditions_met(&[cond(1, 2)], &assists));
        assert!(conditions_met(&[cond(1, 1), cond(3, 1)], &assists));
        assert!(!conditions_met(&[cond(1, 3)], &assists));
        assert!(!conditions_met(&[cond(3, 1), cond(5, 1)], &assists));
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
            dt_attack: None,
            dt_skills: Vec::new(),
            result_comment: None,
            team_scale: 100,
            team_pos: [0, 0, 0],
            my_page_pos: my_page_pos(),
            assist_only: false,
            max_tree: None,
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

fn one() -> i64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OwnedMind {
    pub uid: i64,
    pub mind_id: String,
    #[serde(default)]
    pub get_date: String,
    /// The heart lock (protects from sale and use as fodder).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub protected: bool,
    /// Mind level (1..=10) and cumulative EXP (Build 23: Mind levelling).
    #[serde(default = "one")]
    pub level: i64,
    #[serde(default)]
    pub exp: i64,
}

/// A Mind's CP cost (minds.json; 9 when unknown, as mind_info shows).
pub fn mind_cost(mind_id: &str) -> i64 {
    minds()
        .iter()
        .find(|m| m.mind_id == mind_id)
        .map(|m| m.cost)
        .unwrap_or(9)
}

/// `MindInfo` (level 1; abilities left to the master tables).
/// The stat a Mind's ability raises: the one its wiki stats lean to (HP counts at two thirds,
/// being the larger number), ATK when the wiki has none.
pub fn mind_focus(mind_id: &str) -> &'static str {
    let Some(d) = minds().iter().find(|m| m.mind_id == mind_id) else {
        return "ATK";
    };
    let hp = d.hp * 2 / 3;
    if d.def > d.atk && d.def > hp {
        "DEF"
    } else if hp > d.atk {
        "HP"
    } else {
        "ATK"
    }
}

/// A Mind's tactical ability (docs/research/16 §6): the shared row `DT_MIND_<stat>_<rarity>`
/// in dtbattle_ability_data_table (ours). The client takes the parameters from the table
/// (`isInit` false) at `abilityLevel` 1..10, which is the Mind's level.
pub fn mind_ability(mind_id: &str, level: i64) -> Value {
    let rarity = minds()
        .iter()
        .find(|m| m.mind_id == mind_id)
        .map(|d| d.rarity)
        .unwrap_or(3)
        .clamp(1, 4);
    json!({ "abilityId": format!("DT_MIND_{}_{rarity}", mind_focus(mind_id)),
            "abilityLevel": level.clamp(1, 10), "abilityName": "", "abilityParamList": [],
            "isInit": false })
}

/// Mind level cap (MindInfo.maxLevel).
pub const MIND_MAX_LEVEL: i64 = 10;

/// The cumulative EXP to reach each Mind level 1..=10 for a rarity, from `mind_level_table`
/// (the client reads it by position: level L is entry L-1).
pub fn mind_curve(rarity: i64) -> &'static [i64] {
    static CELL: std::sync::OnceLock<Vec<(i64, Vec<i64>)>> = std::sync::OnceLock::new();
    let rows = CELL.get_or_init(|| {
        let js = crate::masterdata::TABLES
            .iter()
            .find(|(n, _)| *n == "mind_level_table")
            .map(|(_, j)| *j)
            .unwrap_or("{}");
        serde_json::from_str::<Value>(js)
            .ok()
            .and_then(|v| v["data"]["paramList"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(|r| {
                let exp = r["levelTableList"]
                    .as_array()
                    .map(|l| l.iter().filter_map(|e| e["exp"].as_i64()).collect())
                    .unwrap_or_default();
                (r["rare"].as_i64().unwrap_or(0), exp)
            })
            .collect()
    });
    rows.iter()
        .find(|(r, _)| *r == rarity.clamp(1, 6))
        .map(|(_, e)| e.as_slice())
        .unwrap_or(&[])
}

/// The level a cumulative EXP total reaches (capped at MIND_MAX_LEVEL).
pub fn mind_level_for(rarity: i64, exp: i64) -> i64 {
    let curve = mind_curve(rarity);
    (1..=MIND_MAX_LEVEL.min(curve.len() as i64))
        .rev()
        .find(|l| curve[*l as usize - 1] <= exp)
        .unwrap_or(1)
}

/// EXP a Mind gives as fodder (ours: 100 x rarity x level).
pub fn mind_material_exp(rarity: i64, level: i64) -> i64 {
    100 * rarity.max(1) * level.max(1)
}

pub fn mind_rarity(mind_id: &str) -> i64 {
    minds()
        .iter()
        .find(|m| m.mind_id == mind_id)
        .map(|m| m.rarity)
        .unwrap_or(3)
}

pub fn mind_info(o: &OwnedMind) -> Value {
    let d = minds().iter().find(|m| m.mind_id == o.mind_id);
    let (rare, cost, hp, atk, def) = d
        .map(|d| (d.rarity, d.cost, d.hp, d.atk, d.def))
        .unwrap_or((3, 9, 0, 0, 0));
    let id = &o.mind_id;
    let level_table: Vec<Value> = mind_curve(rare)
        .iter()
        .enumerate()
        .map(|(i, e)| json!({ "level": i + 1, "exp": e }))
        .collect();
    // A skill Mind (mindType 2) grants its skill instead of an ability (docs/research/26 §3).
    // The battle adds `dtSkillInfo` for every equipped mindType 2 Mind without a null check, so
    // it always carries a real row.
    let skill = mind_skill(id);
    let (mind_type, ability, skill_info, dt_skill, limit) = match skill {
        Some(m) => (
            2,
            json!({}),
            mind_skill_info(m, o.level, false),
            mind_skill_info(m, o.level, true),
            json!(m.statues),
        ),
        None => (
            1,
            mind_ability(id, o.level),
            json!({}),
            json!({}),
            json!([]),
        ),
    };
    json!({
        "mindId": id, "name": format!("MIND_NAME_{id}"), "nameSortIndex": 0, "rare": rare, "cost": cost,
        "description": format!("MIND_TEXT_{id}"), "mindType": mind_type,
        "abilityInfo": {}, "skillInfo": skill_info, "dtAbilityInfo": ability, "dtSkillInfo": dt_skill,
        "tokkouInfo": {}, "dtTokkouInfo": {}, "pvpAbilityInfo": {}, "dtPvpAbilityInfo": {},
        "maxLevel": MIND_MAX_LEVEL, "sellVm": 0, "linkParam": "", "tipsText": "", "level": o.level.max(1), "exp": o.exp,
        "limitCharaList": limit, "limitTribeList": [], "limitStatueType": 0, "limitCount": 0,
        "limitCharacteristicList": [], "characteristicList": [], "levelUpTable": level_table,
        "minduId": o.uid, "sellRing": 0, "sellGet": 0, "stack": 1,
        "materialExp": mind_material_exp(rare, o.level),
        "bonusCondition": 0, "bonusRate": 0, "hp": hp, "eng": 0, "atk": atk, "def": def,
        "newFlg": 0, "useFlg": 0, "protectedFlg": o.protected as i64, "getDate": o.get_date
    })
}

/// Skill Minds, Mind links, skill levels and time stop (docs/research/26-links-and-skills.md).
#[cfg(test)]
mod links_and_skills_tests {
    use super::*;
    use crate::services::shapes::assert_complete;

    fn rows() -> (Vec<Value>, Vec<Value>) {
        let t = dt_skill_table();
        (
            t["data"]["skillDataList"].as_array().unwrap().clone(),
            t["data"]["categoryList"].as_array().unwrap().clone(),
        )
    }

    fn owned(c: &Collection, statue_id: &str) -> i64 {
        c.statues
            .iter()
            .find(|o| o.statue_id == statue_id)
            .unwrap()
            .uid
    }

    fn mind_uid(c: &Collection, mind_id: &str) -> i64 {
        c.minds.iter().find(|m| m.mind_id == mind_id).unwrap().uid
    }

    /// Crash traps (CreateSkillStatus 0xD8E46C, CreateSkillParameter 0xD8D5FC): every mindType 2
    /// Mind carries a dtSkillInfo whose id has a row, with an existing category, and the type
    /// the client slots it by.
    #[test]
    fn skill_minds_name_real_rows() {
        let (skills, cats) = rows();
        let cat_ids: Vec<&str> = cats
            .iter()
            .map(|c| c["dtSkillCategoryId"].as_str().unwrap())
            .collect();
        assert!(mind_skills().len() > 50, "mind_skills.json loaded");
        let mut c = Collection::default();
        c.ensure("2026-11-18 00:00:00");
        let mut seen = 0;
        for m in &c.minds {
            let v = mind_info(m);
            assert_complete("Jjs.MindInfo", &v);
            let Some(ms) = mind_skill(&m.mind_id) else {
                assert_eq!(v["mindType"], 1);
                assert_eq!(v["dtSkillInfo"], json!({}));
                continue;
            };
            seen += 1;
            assert_eq!(v["mindType"], 2);
            let id = v["dtSkillInfo"]["skillId"].as_str().unwrap();
            let row = skills
                .iter()
                .find(|r| r["dtSkillId"] == id)
                .unwrap_or_else(|| panic!("{}: no row {id}", m.mind_id));
            let kind = if ms.skill.special { 3 } else { 2 };
            assert_eq!(v["dtSkillInfo"]["skillType"], kind);
            assert_eq!(
                v["skillInfo"]["skillType"], kind,
                "the formation checks skillInfo"
            );
            assert_eq!(row["dtSkillType"], kind);
            assert!(v["dtSkillInfo"]["skillLevel"].as_i64().unwrap() >= 1);
            assert!(cat_ids.contains(&row["mainSkillCategory"].as_str().unwrap()));
            let view = row["viewSkillId"].as_str().unwrap();
            assert!(view.starts_with("SKILL_") && v["skillInfo"]["skillId"] == view);
            assert!(!ms.statues.is_empty());
            for s in &ms.statues {
                assert!(find(s).is_some(), "{s} in the roster");
                assert!(view[6..].starts_with(&s[..3]), "{view} is {s}'s own skill");
            }
            assert_eq!(v["limitCharaList"], json!(ms.statues));
        }
        assert_eq!(seen, mind_skills().len());
    }

    /// Time stop: a WORLD category (ProcessType 4) with a positive turn count for every sub
    /// category used; a sub category that names no row would cut the skill short
    /// (SkillParameter ctor returns early).
    #[test]
    fn time_stop_skills_have_world_categories() {
        let (skills, cats) = rows();
        let mut n = 0;
        for r in &skills {
            let sub = r["subSkillCategory"].as_str().unwrap_or("");
            if sub.is_empty() {
                continue;
            }
            n += 1;
            let cat = cats
                .iter()
                .find(|c| c["dtSkillCategoryId"] == sub)
                .unwrap_or_else(|| panic!("no category {sub}"));
            assert_eq!(cat["ProcessType"], 4);
            assert!(cat["turn"].as_i64().unwrap() >= 1);
        }
        assert!(n >= 5, "DIO's and Jotaro's time stops");
        assert_eq!(time_stop("SKILL_300_10"), Some("DTSC_WORLD_MIDDLE"));
    }

    /// Cool turns by level: level 1 keeps the old 2 (special 3) turns, so existing battles are
    /// unchanged; level 10 is two turns shorter, never negative.
    #[test]
    fn cool_turns_by_skill_level() {
        let (skills, _) = rows();
        for r in skills.iter().filter(|r| {
            let t = r["dtSkillType"].as_i64().unwrap();
            t == 2 || t == 3
        }) {
            let (min, max) = (
                r["dtMinCoolTurn"].as_i64().unwrap(),
                r["dtMaxCoolTurn"].as_i64().unwrap(),
            );
            // The documented value at the highest level, two more at level 1 to 4, one more
            // from 5 (docs/research/26 §4).
            let id = r["dtSkillId"].as_str().unwrap();
            let top = max;
            assert!((2..=11).contains(&top), "{id}: {top} turns at level 10");
            assert_eq!(cool_turn(min, max, 1), top + 2, "{id}");
            assert_eq!(cool_turn(min, max, 4), top + 2);
            assert_eq!(cool_turn(min, max, 5), top + 1);
            assert_eq!(cool_turn(min, max, 10), top);
        }
        // Documented skills keep their own value; the families take theirs.
        let row = |id: &str| skills.iter().find(|r| r["dtSkillId"] == id).cloned();
        if let Some(r) = row("SKILL_332_11") {
            assert_eq!(
                r["dtMaxCoolTurn"], 9,
                "DIO's time stop: 9 turns at the highest level"
            );
        }
        if let Some(r) = row("SKILL_532_03") {
            assert_eq!(
                (r["dtMaxCoolTurn"].clone(), r["dtLimitCount"].clone()),
                (json!(4), json!(3))
            );
        }
    }

    /// The tree's skill squares raise the statue's skill levels; a statue without them keeps
    /// level 1 (the battle input of Build 24).
    #[test]
    fn tree_skill_squares_raise_skill_levels() {
        let d = statues()
            .iter()
            .find(|d| d.dt_skills.len() == 2)
            .expect("a statue with both skills");
        let squares = tree_squares(d);
        for id in ["A6", "Af3", "D6", "Df3"] {
            let sq = squares.iter().find(|s| s.id == id).unwrap();
            assert!(squares.iter().any(|b| b.id == sq.before));
        }
        let mut o = OwnedStatue {
            uid: 1,
            statue_id: d.statue_id.clone(),
            level: 1,
            exp: 0,
            get_date: String::new(),
            tree: Vec::new(),
            boost: 0,
        };
        assert_eq!(skill_levels(&o), (1, 1));
        let before = tree_bonus(&o).atk;
        o.tree = vec!["A6".into(), "Af3".into(), "D6".into()];
        assert_eq!(skill_levels(&o), (5, 3));
        assert_eq!(tree_bonus(&o).atk, before, "skill squares add no stats");
        let v = battle_statue_info(&o);
        let lv: Vec<(bool, i64)> = v["dtSkillList"]
            .as_array()
            .unwrap()
            .iter()
            .skip(1)
            .map(|s| (s["skillType"] == 3, s["skillLevel"].as_i64().unwrap()))
            .collect();
        assert!(lv.contains(&(false, 5)) && lv.contains(&(true, 3)));
        // Statues without a skill get no square for it.
        if let Some(none) = statues().iter().find(|d| d.dt_skills.is_empty()) {
            assert!(tree_squares(none).iter().all(|s| s.effect < TREE_SKILL));
        }
    }

    /// A seat with a skill Mind in the skill frame and three ATK-link Minds: the skill Mind is
    /// sent with its skill, the link lists the three minduIds with the 3-Mind ability, and the
    /// ability rides on the statue's ability list. Moving the seat to another character drops
    /// the skill Mind from the battle input.
    #[test]
    fn seat_with_skill_mind_and_link() {
        let mut c = Collection::default();
        c.ensure("2026-11-18 00:00:00");
        let ms = mind_skills()
            .iter()
            .find(|m| !m.skill.special && m.mind_id == "4001")
            .expect("Mind 4001 grants a normal skill");
        let statue = owned(&c, &ms.statues[0]);
        let links = ["3006", "7243", "7317"].map(|m| mind_uid(&c, m));
        let mut t = main_team(&c, 1).unwrap().clone();
        t.slots[0] = statue;
        t.minds[0] = vec![
            (1, links[0]),
            (2, links[1]),
            (3, links[2]),
            (MIND_SKILL_SLOT, mind_uid(&c, "4001")),
        ];
        let v = team_info(&c, &t);
        assert_complete("Jjs.TeamOrder", &v["teamOrder"]);
        let seat = &v["teamOrder"]["leaderStatue"];
        let minds = seat["equipMindList"].as_array().unwrap();
        assert_eq!(minds.len(), 4);
        assert!(minds
            .iter()
            .any(|m| m["mindType"] == 2 && m["seq"] == MIND_SKILL_SLOT));
        let l = seat["mindLinkList"].as_array().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0]["mindLinkId"], "MIND_LINK_001");
        assert_eq!(l[0]["dataList"], json!(links));
        assert_eq!(l[0]["abilityInfo"]["abilityId"], "DT_MLINK_ATK_3");
        assert_eq!(l[0]["abilityInfo"]["isInit"], true);
        assert!(!l[0]["abilityInfo"]["abilityParamList"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(seat["statueInfo"]["dtAbilityList"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["abilityId"] == "DT_MLINK_ATK_3"));
        // Another character in the seat: the skill Mind is not sent, the link stays.
        let other = c
            .statues
            .iter()
            .find(|o| !ms.statues.contains(&o.statue_id))
            .unwrap()
            .uid;
        t.slots[0] = other;
        let v = team_info(&c, &t);
        let seat = &v["teamOrder"]["leaderStatue"];
        assert_eq!(seat["equipMindList"].as_array().unwrap().len(), 3);
        assert_eq!(seat["mindLinkList"].as_array().unwrap().len(), 1);
        // One linked Mind is no link.
        t.minds[0] = vec![(1, links[0])];
        let v = team_info(&c, &t);
        assert_eq!(v["teamOrder"]["leaderStatue"]["mindLinkList"], json!([]));
    }

    #[test]
    fn mind_frames() {
        let ms = mind_skill("4001").unwrap();
        let s = &ms.statues[0];
        assert!(mind_fits("4001", MIND_SKILL_SLOT, s));
        assert!(!mind_fits("4001", MIND_SP_SKILL_SLOT, s), "wrong frame");
        assert!(!mind_fits("4001", 1, s), "no skill Mind in a golden frame");
        assert!(
            !mind_fits("4001", MIND_SKILL_SLOT, "9999_99"),
            "another character"
        );
        assert!(mind_fits("0001", 4, s));
        assert!(
            !mind_fits("0001", MIND_SKILL_SLOT, s),
            "no ability Mind in a skill frame"
        );
        let sp = mind_skills().iter().find(|m| m.skill.special).unwrap();
        assert!(mind_fits(&sp.mind_id, MIND_SP_SKILL_SLOT, &sp.statues[0]));
    }
}
