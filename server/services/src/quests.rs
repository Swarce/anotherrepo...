//! Story quests: chapters, stages, battle start and end.
//!
//! Chapters, stages, area scenes and maps are identifiers from the player's own files
//! (`server/content/quests.json`, built by `tools/content/build_quests.py`); stamina, rewards and
//! enemies come from the wikis where merged, marked defaults elsewhere. The battle itself runs on
//! the client: the server describes it (areas, waves, enemies) and records the result.

use crate::collection::{self, Collection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const QUESTS_JSON: &str = include_str!("../../content/quests.json");
/// Daily and event chapters (`tools/content/build_extra_quests.py`).
pub const QUESTS_EXTRA_JSON: &str = include_str!("../../content/quests_extra.json");

#[derive(Debug, Clone, Deserialize)]
pub struct StageDef {
    #[serde(rename = "stageId")]
    pub stage_id: String,
    #[serde(default)]
    pub areas: Vec<String>,
    #[serde(rename = "mapId")]
    pub map_id: String,
    #[serde(rename = "useStamina")]
    pub use_stamina: i64,
    #[serde(rename = "gainRankExp")]
    pub gain_rank_exp: i64,
    pub gold: i64,
    #[serde(rename = "enemyLevel")]
    pub enemy_level: i64,
    pub enemies: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChapterDef {
    #[serde(rename = "chapterId")]
    pub chapter_id: String,
    pub num: i64,
    #[serde(rename = "stageType")]
    pub stage_type: i64,
    #[serde(rename = "chapterMap")]
    pub chapter_map: Option<i64>,
    /// BizarreCategoryType: 1 story, 6-12 daily (Sunday to Saturday), 13+ event.
    #[serde(default = "story_category")]
    pub category: i64,
    pub stages: Vec<StageDef>,
}

fn story_category() -> i64 {
    STORY_CATEGORY
}

/// One weekday's daily tile, or one event tile (from `quests_extra.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct DailyDef {
    pub category: i64,
    pub banner: String,
}

/// An event with a tile (docs/research/25-events-raids.md). `kind`: "exchange", "point",
/// "high_score", "high_score_shop" or "quiz"; `currency` the item its quests drop (0 = none).
#[derive(Debug, Clone, Deserialize)]
pub struct EventDef {
    #[serde(rename = "eventId")]
    pub event_id: String,
    pub category: i64,
    pub banner: String,
    #[serde(default = "exchange_kind")]
    pub kind: String,
    #[serde(default)]
    pub currency: i64,
    /// Event bonus units (ours): statues in the team and Minds on it raise the event's drops.
    #[serde(default, rename = "bonusStatues")]
    pub bonus_statues: Vec<String>,
    #[serde(default, rename = "bonusMinds")]
    pub bonus_minds: Vec<String>,
    /// High-score stages and their scoring rule (DTEventPointType: 2 turns, 3 max damage,
    /// 4 total damage), from the stage names.
    #[serde(default)]
    pub scoring: std::collections::BTreeMap<String, Vec<i64>>,
    #[serde(default)]
    pub quiz: Vec<QuizDef>,
}

fn exchange_kind() -> String {
    "exchange".into()
}

/// A quiz chapter: the question stage, the three answer stages and the right one (from the
/// event's own answer scripts).
#[derive(Debug, Clone, Deserialize)]
pub struct QuizDef {
    #[serde(rename = "chapterId")]
    pub chapter_id: String,
    pub question: String,
    pub answers: Vec<String>,
    pub correct: String,
}

/// A campaign banner (Campaign Info): `ec_<id>_..._start` with a name label.
#[derive(Debug, Clone, Deserialize)]
pub struct CampaignDef {
    #[serde(rename = "campaignId")]
    pub campaign_id: String,
    pub banner: String,
    pub name: String,
}

#[derive(Deserialize)]
struct ExtraDoc {
    daily: std::collections::BTreeMap<String, DailyDef>,
    events: Vec<EventDef>,
    #[serde(default)]
    campaigns: Vec<CampaignDef>,
    chapters: Vec<ChapterDef>,
}

fn extra() -> &'static ExtraDoc {
    static CELL: std::sync::OnceLock<ExtraDoc> = std::sync::OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str(QUESTS_EXTRA_JSON).expect("quests_extra.json"))
}

/// The events with a tile, in order.
pub fn events() -> &'static [EventDef] {
    &extra().events
}

/// The campaign banners, in order.
pub fn campaigns() -> &'static [CampaignDef] {
    &extra().campaigns
}

pub fn event_by_id(id: &str) -> Option<&'static EventDef> {
    events().iter().find(|e| e.event_id == id)
}

pub fn event_for_category(category: i64) -> Option<&'static EventDef> {
    events().iter().find(|e| e.category == category)
}

/// The scoring rule of a high-score stage (None for every other stage).
pub fn scoring_of(stage_id: &str) -> Option<&'static [i64]> {
    events()
        .iter()
        .find_map(|e| e.scoring.get(stage_id))
        .map(Vec::as_slice)
}

/// Today's daily tile: the game's day turns at 04:00 JST (research pack `economy.daily_reset`).
pub fn daily_today() -> Option<&'static DailyDef> {
    const CODES: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "STA"];
    let t = jdr_core::clock::now() + jdr_core::clock::TZ_OFFSET_SECS - 4 * 3600;
    let weekday = (t.div_euclid(86400) + 4).rem_euclid(7) as usize; // 1970-01-01 was a Thursday
    extra().daily.get(CODES[weekday])
}

pub fn chapters() -> &'static [ChapterDef] {
    static CELL: std::sync::OnceLock<Vec<ChapterDef>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            chapters: Vec<ChapterDef>,
        }
        let mut all = serde_json::from_str::<Doc>(QUESTS_JSON)
            .map(|d| d.chapters)
            .unwrap_or_default();
        all.extend(extra().chapters.iter().cloned());
        all
    })
}

pub fn find_stage(stage_id: &str) -> Option<(&'static ChapterDef, &'static StageDef)> {
    chapters().iter().find_map(|c| {
        c.stages
            .iter()
            .find(|s| s.stage_id == stage_id)
            .map(|s| (c, s))
    })
}

/// Story progress kept in the save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Progress {
    #[serde(default)]
    pub cleared: BTreeSet<String>,
    #[serde(default)]
    pub play_counts: std::collections::BTreeMap<String, i64>,
    /// The battle started and not yet ended: `bizarre_end` pays out only for it, once.
    #[serde(default)]
    pub pending: Option<PendingBattle>,
    /// Tactical missions cleared per stage (DT_QM types, from the clients' `starClearList`).
    #[serde(default)]
    pub stars: std::collections::BTreeMap<String, BTreeSet<i64>>,
    /// Mission counters and claims (Build 21).
    #[serde(default)]
    pub missions: crate::missions::MissionState,
    /// The Message Box (Build 22).
    #[serde(default)]
    pub presents: crate::presents::PresentBox,
    /// Box and Count Up summon progress (Build 22).
    #[serde(default)]
    pub gasha: crate::services::gasha::GashaState,
    /// Event points per event (point events, Build 23).
    #[serde(default)]
    pub event_points: std::collections::BTreeMap<String, i64>,
    /// Mentality and the Labyrinth run in progress (docs/research/22-labyrinth.md).
    #[serde(default)]
    pub dungeon: crate::labyrinth::DungeonState,
    /// Arena standing, team durability and opponents (docs/research/23-arena.md).
    #[serde(default)]
    pub arena: crate::services::arena::ArenaState,
    /// Best result per high-score stage (docs/research/25-events-raids.md).
    #[serde(default)]
    pub high_scores: std::collections::BTreeMap<String, crate::services::event::HighScore>,
    /// The answer stage first cleared per quiz chapter (the player's answer).
    #[serde(default)]
    pub quiz_answers: std::collections::BTreeMap<String, String>,
}

/// The three tactical missions of a stage (docs/research/11 §4): clear the quest, every member
/// survives, clear within a number of turns. `starTarget` is the DTBattleQuestMissionClearType the
/// client checks itself when the battle ends; `comment` is the mission's text label, with
/// `{starParam}` replaced by `param`.
pub fn dt_missions(s: &StageDef) -> [(i64, &'static str, String); 3] {
    // Turn limit: ours (jdr-default), 10 turns for the first area and 5 more for each further one.
    let turns = 10 + 5 * (s.areas.len().max(1) as i64 - 1);
    [
        (1, "DT_QM_Text_001", String::new()),
        (2, "DT_QM_Text_002", String::new()),
        (10, "DT_QM_Text_010", turns.to_string()),
    ]
}

/// Loading-screen tips with a title, a text and a picture in the player's files
/// (`TipsTitle_<nnn>`, `TipsText_<nnn>`, bundle `ui_tips_<nnn>`; docs/research/11 §4).
const TIPS: &[u16] = &[
    1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 24, 25, 26, 27, 28, 29, 30,
    31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 50, 51, 52, 53, 55, 56, 57, 58,
    59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 72, 73, 74, 100,
];

/// The stage's loading tip: a stable pick by the stage id.
pub fn quest_tips(stage_id: &str) -> Value {
    let h = stage_id.bytes().fold(0x811c_9dc5u32, |h, b| {
        (h ^ b as u32).wrapping_mul(0x0100_0193)
    });
    let n = TIPS[h as usize % TIPS.len()];
    json!({
        "tipsId": n.to_string(), "tipsType": 0,
        "tipsTitleText": format!("TipsTitle_{n:03}"), "tipsText": format!("TipsText_{n:03}"),
        "tipsPicture": format!("ui_tips_{n:03}")
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingBattle {
    pub stage_id: String,
    pub stage_type: i64,
    pub team_num: i64,
}

const OPEN: &str = "2020-01-01 00:00:00";
const CLOSE: &str = "2099-12-31 23:59:59";
/// BizarreCategoryType.STORY_01: the single story category we list.
pub const STORY_CATEGORY: i64 = 1;

/// `QuestInfo` for the story button.
pub fn story_quest_info() -> Value {
    json!({ "questHomeId": "ST", "category": STORY_CATEGORY, "bannerType": 1, "bannerFileName": "",
            "day": 0, "Priority": 1, "eventMasterId": "", "startDate": OPEN, "endDate": CLOSE })
}

/// The chapter title/name labels: `CHAPTER_ST_01` / `CHAPTER_NAME_ST_01` (and the DT forms).
fn name_label(c: &ChapterDef) -> String {
    c.chapter_id.replacen("CHAPTER_", "CHAPTER_NAME_", 1)
}

/// The chapter description label: the game has `ChapterText_DT_ST_<nnn>` for the tactical story
/// chapters (000 and 401-478); other chapters keep their id (shown as is).
fn summary_label(c: &ChapterDef) -> String {
    if c.stage_type == 1 && (c.num == 0 || (401..=478).contains(&c.num)) {
        format!("ChapterText_DT_ST_{:03}", c.num)
    } else {
        c.chapter_id.clone()
    }
}

pub fn chapter_info(c: &ChapterDef, p: &Progress) -> Value {
    let stages: Vec<Value> = c
        .stages
        .iter()
        .map(|s| {
            json!({ "stageId": s.stage_id, "clearFlg": p.cleared.contains(&s.stage_id) as i64,
                    "specialRewardFlg": 0, "playCount": p.play_counts.get(&s.stage_id).copied().unwrap_or(0),
                    "isRaid": 0 })
        })
        .collect();
    json!({
        "chapterId": c.chapter_id, "name": name_label(c), "category": c.category,
        "summary": summary_label(c), "chapterNum": c.num,
        "startStageId": c.stages.first().map(|s| s.stage_id.as_str()).unwrap_or(""),
        "lastStageId": c.stages.last().map(|s| s.stage_id.as_str()).unwrap_or(""),
        "chapterMapType": c.chapter_map.unwrap_or(0), "selectMapType": 0, "locationId": 0,
        "startTalkId": "", "start": OPEN, "end": CLOSE, "stageType": c.stage_type,
        "startConditionItemList": [], "startConditionFlg": true, "startConditionText": "",
        "stageList": stages
    })
}

fn enemy_info(statue_id: &str, level: i64) -> Value {
    let mut v = collection::battle_statue_info(&collection::OwnedStatue {
        uid: 0,
        statue_id: statue_id.to_string(),
        level,
        exp: 0,
        get_date: String::new(),
        tree: Vec::new(),
        boost: 0,
    });
    v["useAttr"] = json!(3); // EnemyOnly
                             // Enemy teams have no assists, so no assist skill or abilities either.
    v["dtAssistSkillInfo"] = json!({});
    v["assistAbilityList"] = json!([]);
    v["dtAssistAbilityList"] = json!([]);
    v
}

fn star_list(c: &ChapterDef, s: &StageDef, p: &Progress) -> Vec<Value> {
    if c.stage_type != 1 {
        return Vec::new(); // action-battle missions use another type list; not served
    }
    let done = p.stars.get(&s.stage_id);
    dt_missions(s)
        .iter()
        .enumerate()
        .map(|(i, (ty, label, param))| {
            json!({
                "starNum": i + 1, "starTarget": ty, "comment": label, "param": param,
                "clearFlg": done.is_some_and(|d| d.contains(ty)) as i64
            })
        })
        .collect()
}

pub fn stage_info(c: &ChapterDef, s: &StageDef, n: usize, p: &Progress) -> Value {
    let cleared = p.cleared.contains(&s.stage_id);
    json!({
        "stageId": s.stage_id, "name": s.stage_id, "stageNo": n + 1, "stageDifficulty": 0, "type": 0,
        "text": s.stage_id, "startTalkId": "", "endTalkId": "",
        "startConditionItemList": [], "startConditionList": [], "startConditionFlg": true,
        "startConditionText": "", "areaMax": s.areas.len().max(1), "useMentality": 0, "openSetting": 1,
        "nextStageIdList": c.stages.get(n + 1).map(|x| vec![x.stage_id.clone()]).unwrap_or_default(),
        // The stage window lists steadyItemSet as the clear reward: the gold a clear pays.
        // (RewardItemSet {dataType 5 = Money, dataId: string, name, count}. dataId is a string
        // the stage window parses with Int32.Parse (ItemIconController.SetThumnail), so it must
        // hold a number: "" threw, the window never filled in and the battle could not start.)
        // Mission rows: stage window, battle end and result screen all read this list.
        "starList": star_list(c, s, p), "rewardItemSet": [], "reward3StarsItemSet": [],
        "steadyItemSet": [{ "dataType": 5, "dataId": "0", "name": "", "count": s.gold }],
        "bossDropItemSet": [], "useStamina": s.use_stamina, "gainRankExp": s.gain_rank_exp,
        "mindGashaPoint": 0, "playCount": p.play_counts.get(&s.stage_id).copied().unwrap_or(0),
        "clearFlg": cleared as i64, "clearCount": cleared as i64, "clearRank": if cleared { 4 } else { 0 },
        "specialRewardFlg": 0, "questTips": quest_tips(&s.stage_id), "meisyoId": 0, "limitTime": 0, "endureTime": 0,
        "enemyList": s.enemies.iter().map(|e| enemy_info(e, s.enemy_level)).collect::<Vec<_>>(),
        "staminaCoefficientList": { "rate": 1.0, "add": 0 },
        "expCoefficientList": { "rate": 1.0, "add": 0 },
        "goldCoefficientList": { "rate": 1.0, "add": 0 },
        // High-score stages list their scoring rule (the battle's condition text, "Scored on:
        // ...") and show the best score on the stage banner; -1 hides the banner's high-score box
        // (BizarreStageBannerController.SetDtBattleIcon, docs/research/25-events-raids.md).
        "bizarreEffectList": [], "limitTurn": 0, "endureTurn": 0,
        "eventPointSettingList": crate::services::event::point_settings(&s.stage_id),
        "dayNight": 0, "clearStarList": [], "useStatueCount": 3,
        "highScore": crate::services::event::stage_high_score(p, &s.stage_id),
        "cannotContinue": 0, "presetTeamId": ""
    })
}

/// AI for story enemies: an attack-only AI from the player's aipack (an empty id would fall back to
/// the player's auto-play AI).
pub const DT_ENEMY_AI: &str = "zako01_cmn";

/// Our pattern in area_position_pattern_table (server/content/masterdata).
pub const DT_FORMATION: &str = "JDR_DT_DEFAULT";

/// The battle description: one AreaInfo per area scene, one wave of the stage's enemies each.
pub fn area_list(c: &ChapterDef, s: &StageDef) -> Vec<Value> {
    let areas: Vec<String> = if s.areas.is_empty() {
        vec![String::new()]
    } else {
        s.areas.clone()
    };
    areas
        .iter()
        .enumerate()
        .map(|(i, area)| {
            let waves: Vec<Value> = s
                .enemies
                .iter()
                .enumerate()
                .map(|(k, e)| {
                    json!({ "areaId": area, "waveNum": 1, "type": "WAVE_A", "pivot": format!("{:02}", k + 1),
                            "offset": 0, "enemyStatueInfo": enemy_info(e, s.enemy_level), "AIId": 2,
                            "dropList": [], "label": "", "cutinEvent": 0, "cameraTarget": "",
                            "intervalTime": 0, "spawnNum": 1, "disappear": 0,
                            "bossFlag": (i + 1 == areas.len() && k == 0) as i64 })
                })
                .collect();
            let dt_settings: Vec<Value> = s
                .enemies
                .iter()
                .enumerate()
                .map(|(k, e)| {
                    json!({ "enemyStatueInfo": enemy_info(e, s.enemy_level), "seq": k + 1,
                            "difficultyTable": "", "difficultyLevel": 1, "AiId": DT_ENEMY_AI, "fastStrike": false,
                            "enemySkillTable": "", "dropList": [], "disappear": 0, "hideEnemyInfo": 0,
                            // No boss intro until it is known to work offline.
                            "bossFlag": 0, "backFlag": 0 })
                })
                .collect();
            json!({
                "areaNum": i + 1, "areaId": area, "areaScene": area.to_uppercase(),
                "areaEventId": "", "endEventId": "", "bizarreEffectId": "", "mapId": s.map_id,
                "dayNight": 0, "vampireDayNight": 0, "mapCamera": 0, "startCamera": 0, "bgmCueId": 0,
                "rotate": 0, "levelEditId": "", "instanceObjType": 0, "startWipe": 1, "endWipe": 1,
                "clearWaveNum": 1, "limitTime": 0, "endureTime": 0,
                "waveList": if c.stage_type == 0 { waves } else { Vec::new() },
                "mapGimmickInfo": {}, "mapPoint": 0,
                // Tactical areas place units by this area_position_pattern_table row.
                "formation": if c.stage_type == 1 { DT_FORMATION } else { "" },
                "clearAreaType": 1, "limitTurn": 0,
                "areaSettingList": if c.stage_type == 1 { dt_settings } else { Vec::new() }
            })
        })
        .collect()
}

/// Team by number and battle type; a missing team or one without a leader falls back to the main
/// team of that type.
pub fn team_for(c: &Collection, stage_type: i64, team_no: i64) -> Option<&collection::Team> {
    c.teams
        .iter()
        .find(|t| t.stage_type == stage_type && t.team_num == team_no && t.slots[0] != 0)
        .or_else(|| collection::main_team(c, stage_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missions_and_tips_name_real_labels() {
        let tr = crate::l10n::Translations::embedded().unwrap();
        let has = |label: &str| {
            ["DTB_QuestMission_JP", "TipsTitle_JP", "TipsText_JP"]
                .iter()
                .any(|t| tr.english(&format!("text:{t}:{label}")).is_some())
        };
        let c = chapters().iter().find(|c| c.stage_type == 1).unwrap();
        let s = &c.stages[0];
        let mut p = Progress::default();
        let info = stage_info(c, s, 0, &p);
        let stars = info["starList"].as_array().unwrap();
        assert_eq!(stars.len(), 3);
        for st in stars {
            assert!(has(st["comment"].as_str().unwrap()), "{st}");
            assert!((1..=16).contains(&st["starTarget"].as_i64().unwrap()));
            assert_eq!(st["clearFlg"], 0);
        }
        assert_eq!(stars[2]["starTarget"], 10, "turn limit mission");
        assert!(stars[2]["param"].as_str().unwrap().parse::<i64>().unwrap() >= 10);
        let tips = &info["questTips"];
        assert!(
            has(tips["tipsTitleText"].as_str().unwrap()) && has(tips["tipsText"].as_str().unwrap())
        );
        assert!(tips["tipsPicture"]
            .as_str()
            .unwrap()
            .starts_with("ui_tips_"));
        // Every tip number we pick has both labels translated.
        for n in TIPS {
            assert!(
                has(&format!("TipsTitle_{n:03}")) && has(&format!("TipsText_{n:03}")),
                "{n}"
            );
        }
        // Cleared missions light their stars.
        p.stars
            .insert(s.stage_id.clone(), [1, 10].into_iter().collect());
        let info = stage_info(c, s, 0, &p);
        let lit: Vec<i64> = info["starList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["clearFlg"].as_i64().unwrap())
            .collect();
        assert_eq!(lit, [1, 0, 1]);
    }

    #[test]
    fn content_loads() {
        assert!(chapters().len() > 50);
        let (c, s) = find_stage(&chapters()[1].stages[0].stage_id).unwrap();
        let info = chapter_info(c, &Progress::default());
        assert_eq!(info["stageList"].as_array().unwrap().len(), c.stages.len());
        assert!(info["name"].as_str().unwrap().starts_with("CHAPTER_NAME_"));
        let a = area_list(c, s);
        assert!(!a.is_empty());
        assert!(!s.enemies.is_empty());
    }
}
