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
    pub stages: Vec<StageDef>,
}

pub fn chapters() -> &'static [ChapterDef] {
    static CELL: std::sync::OnceLock<Vec<ChapterDef>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| {
        #[derive(Deserialize)]
        struct Doc {
            chapters: Vec<ChapterDef>,
        }
        serde_json::from_str::<Doc>(QUESTS_JSON)
            .map(|d| d.chapters)
            .unwrap_or_default()
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
        "chapterId": c.chapter_id, "name": name_label(c), "category": STORY_CATEGORY,
        "summary": c.chapter_id, "chapterNum": c.num,
        "startStageId": c.stages.first().map(|s| s.stage_id.as_str()).unwrap_or(""),
        "lastStageId": c.stages.last().map(|s| s.stage_id.as_str()).unwrap_or(""),
        "chapterMapType": c.chapter_map.unwrap_or(0), "selectMapType": 0, "locationId": 0,
        "startTalkId": "", "start": OPEN, "end": CLOSE, "stageType": c.stage_type,
        "startConditionItemList": [], "startConditionFlg": true, "startConditionText": "",
        "stageList": stages
    })
}

fn enemy_info(statue_id: &str, level: i64) -> Value {
    let mut v = collection::statue_info(&collection::OwnedStatue {
        uid: 0,
        statue_id: statue_id.to_string(),
        level,
        exp: 0,
        get_date: String::new(),
    });
    v["useAttr"] = json!(3); // EnemyOnly
    v
}

pub fn stage_info(c: &ChapterDef, s: &StageDef, n: usize, p: &Progress) -> Value {
    let cleared = p.cleared.contains(&s.stage_id);
    json!({
        "stageId": s.stage_id, "name": s.stage_id, "stageNo": n + 1, "stageDifficulty": 0, "type": 0,
        "text": s.stage_id, "startTalkId": "", "endTalkId": "",
        "startConditionItemList": [], "startConditionList": [], "startConditionFlg": true,
        "startConditionText": "", "areaMax": s.areas.len().max(1), "useMentality": 0, "openSetting": 1,
        "nextStageIdList": c.stages.get(n + 1).map(|x| vec![x.stage_id.clone()]).unwrap_or_default(),
        "starList": [], "steadyItemSet": [], "rewardItemSet": [], "reward3StarsItemSet": [],
        "bossDropItemSet": [], "useStamina": s.use_stamina, "gainRankExp": s.gain_rank_exp,
        "mindGashaPoint": 0, "playCount": p.play_counts.get(&s.stage_id).copied().unwrap_or(0),
        "clearFlg": cleared as i64, "clearCount": cleared as i64, "clearRank": if cleared { 4 } else { 0 },
        "specialRewardFlg": 0, "questTips": {}, "meisyoId": 0, "limitTime": 0, "endureTime": 0,
        "enemyList": s.enemies.iter().map(|e| enemy_info(e, s.enemy_level)).collect::<Vec<_>>(),
        "staminaCoefficientList": { "rate": 1.0, "add": 0 },
        "expCoefficientList": { "rate": 1.0, "add": 0 },
        "goldCoefficientList": { "rate": 1.0, "add": 0 },
        "bizarreEffectList": [], "limitTurn": 0, "endureTurn": 0, "eventPointSettingList": [],
        "dayNight": 0, "clearStarList": [], "useStatueCount": 3, "highScore": 0,
        "cannotContinue": 0, "presetTeamId": ""
    })
}

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
                            "difficultyTable": "", "difficultyLevel": 1, "AiId": "", "fastStrike": false,
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
