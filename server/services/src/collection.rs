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
}

fn full_scale() -> i64 {
    100
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
    out
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
}

fn seats_empty(m: &[Vec<(i64, i64)>; 3]) -> bool {
    m.iter().all(Vec::is_empty)
}

/// A statue's Mind capacity (CP; `StatueInfo.mindCapacity`, ours: three 4-star or four 3-star
/// Minds at cost 3 x rarity).
pub const MIND_CAPACITY: i64 = 40;
/// Mind slots open per seat (the four golden frames, as at launch).
pub const MIND_SLOTS_OPEN: i64 = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Collection {
    #[serde(default)]
    pub statues: Vec<OwnedStatue>,
    #[serde(default)]
    pub teams: Vec<Team>,
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
        "characteristicList": [], "assistAttr": 0, "assistSkillInfo": {}, "assistAbilityList": [],
        "dtAssistSkillInfo": {}, "dtAssistAbilityList": [],
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
    let mut list = vec![basic_attack(&attack_skill(&o.statue_id))];
    for s in find(&o.statue_id)
        .map(|d| d.dt_skills.as_slice())
        .unwrap_or(&[])
    {
        // The client takes type, button and name from the table row; the level must be >= 1.
        list.push(
            json!({ "skillId": s.id, "skillLevel": 1, "name": "", "explanation": "",
            "skillType": if s.special { 3 } else { 2 }, "bottunType": s.button, "coolDown": 0,
            "limitCount": 0, "magnificationLv01": 0, "magnificationLv10": 0,
            "pvpMagnificationLv01": 0, "pvpMagnificationLv10": 0, "skillParamList": [],
            "isInit": false }),
        );
    }
    v["dtSkillList"] = json!(list);
    v
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
    // 15% (special 20%; the attack's is 10%, docs/research/11 §5).
    let mut skills = std::collections::BTreeMap::new();
    let mut cats = std::collections::BTreeMap::new();
    for s in statues().iter().flat_map(|d| d.dt_skills.iter()) {
        skills.entry(s.id.clone()).or_insert(s);
        cats.entry(s.category.clone()).or_insert(s);
    }
    if let Some(list) = doc["data"]["skillDataList"].as_array_mut() {
        for s in skills.values() {
            let mut row = template.clone();
            let cool = if s.special { 3 } else { 2 };
            for (k, v) in [
                ("dtSkillId", json!(s.id)),
                ("viewSkillId", json!(s.id)),
                ("dtSkillName", json!(s.name)),
                ("dtSkillProfileText", json!(s.text)),
                ("dtSkillType", json!(if s.special { 3 } else { 2 })),
                ("dtButtonType", json!(s.button)),
                ("damageRank", json!(s.damage_rank)),
                ("dtLimitCount", json!(if s.special { 3 } else { 0 })),
                // Reversal gauge, in percent, added when a player unit picks the skill (ours).
                ("dododoRate", json!(if s.special { 20 } else { 15 })),
                ("dtMinCoolTurn", json!(cool)),
                ("dtMaxCoolTurn", json!(cool)),
                ("mainSkillCategory", json!(s.category)),
                ("dtRangeMin", json!(skill_range(&s.id, s.target_type).0)),
                ("dtRangeMax", json!(skill_range(&s.id, s.target_type).1)),
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
    }
    doc
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
            // Only filled slots: the client counts the list against the open slots.
            let equipped: Vec<Value> = t.minds[seat]
                .iter()
                .filter_map(|(seq, muid)| {
                    let m = c.minds.iter().find(|m| m.uid == *muid)?;
                    let mut v = mind_info(m);
                    v["seq"] = json!(seq);
                    v["useFlg"] = json!(1);
                    Some(v)
                })
                .collect();
            json!({ "statueuId": uid, "statueInfo": battle_statue_info(o), "equipMindList": equipped,
                           "mindLinkList": [], "assistStatueList": [], "AiId": "" })
        }
        None => empty_setting(),
    }
}

impl Collection {
    /// Mind uids equipped anywhere.
    pub fn equipped_minds(&self) -> std::collections::HashSet<i64> {
        self.teams
            .iter()
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
            // facing +Z) and the leader 3.22 m ahead, right of centre (Build 12: 12% closer at the owner's request), turned towards the camera,
            // which frames models 1.0-2.3 m tall. Team view: the slot anchors already face the
            // camera; each statue's scale and offset frame its posed head (docs/research/11 §1, 12 §7).
            json!({
                "statueId": d.statue_id, "myPageId": ids[h as usize % ids.len()],
                "myPageCameraType": "CameraIdSet0",
                "myPagePos": { "x": 72.0, "y": -113.0, "z": 322.0 },
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OwnedMind {
    pub uid: i64,
    pub mind_id: String,
    #[serde(default)]
    pub get_date: String,
    /// The heart lock (protects from sale and use as fodder).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub protected: bool,
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
        "newFlg": 0, "useFlg": 0, "protectedFlg": o.protected as i64, "getDate": o.get_date
    })
}
