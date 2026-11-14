# 09. in_memory_master_data: schema of the master-data ScriptableObjects

Scope: JoJo Diamond Records 3.12.0, Unity 2018.4.1f1, IL2CPP, arm64. Evidence comes from `dump.cs`, field attributes read from global-metadata (`tools/fattr`), arm64 disassembly (`tools/dis.py`), and real type trees in downloaded bundles (UnityPy).
Confidence tags: **H** means read directly from code or metadata. **M** means inferred from code with one step of reasoning, or by analogy with a real type tree. **L** means a guess.
The analysis covered 18 core assets, 27 extra assets and 147 types. The repo keeps only what our content needs: `server/content/masterdata/schema.json` holds the 17 core assets with a class and the 57 types they reach (enum value names only where content uses them); battle-table types are added with their content. No key values, no game text. The server builds the bundle from it: `server/services/src/masterdata.rs`.

## 0. Key findings

1. **The bundle holds about 45 assets, not 18 (H).** The 18 full literal paths are only part of it. `ResourceConst` also builds 27 more paths at runtime as `InMemoryMasterDataDirectoryPath + "/x.asset"` (cctor @0xE29134..0xE29728). Every one of them is read with `GetResources(InMemoryMasterDataAssetBundleName, path)`, which is the same bundle. Examples: affect_table, characteristic_data, state_abnormality_table, model_table, clan_table, the six reaction tables, and the dtbattle_* tables. Eleven of these extras are read by TitleLoad (see section 2b). They are all null-guarded, so boot still works without them (M; checked in Build 06 for the two read earliest: `AbilityDataTable.Load` guards `dtTable` with `op_Inequality` before `OnInit` @0x205E2A4, and `EffectManager.SetupReductionSetting` @0x1038CF0 only stores the reference, which `checkReduction` @0x1039270 null-checks before use — H), but battles need them.
2. **`signboard_table` has no consumer (M).** Nothing reads `ResourceConst+0x410` as a static field (binary scan of every `ldr [x,#0x410]` that follows a ResourceConst access; all hits were GOT false positives). The dump has no `SignboardTable` class. Leave it out, or ship any ScriptableObject in its place.
3. **Every loader follows the same pattern (H):** `ResourceManager.GetResources(bundle=ResourceConst+0x58, assetPath)` @0x10C51AC, then an **exact-class** `isinst` check (class-hierarchy depth compare), then `if (obj == null) return;`, then `Init()`. `GetResources` lowercases the asset name (`checkAssetName` @0x10D370C → `String.ToLower`). Container keys must therefore be the lowercase full path, including `.asset`.
4. Every `Init()` builds its dictionary with a `ContainsKey` guard before `Add` (H, one ContainsKey and one Add per Init; e.g. TutorialFlagTable.Init @0xB9D0C8/@0xB9D0F4). **Duplicate ids are ignored rather than crashing**, and **empty lists are safe in every Load/Init**.
5. **No downloaded bundle uses `SecurityIntValue` (H).** All 13,336 non-CPK bundles were scanned for MonoBehaviour type trees containing SecurityInt/Long/Float types and found none. Its form is derived from metadata instead: a `[Serializable]` struct `{ [SerializeField] int seed; [SerializeField] int securityValue; }`, where `Value = seed ^ securityValue` (`get_Value` @0xA99694). **Write seed=0 and securityValue=value (H).**
6. For tutorials, use a table whose ids run 0..N-1 and which contains every label the client queries (section 6). Together with tutorialStep = all 1s (length ≥ N), the client treats every tutorial as done. An empty table also skips the tutorials, but then every `IsFlagOn(label)` returns false, which keeps the daily and event quest categories locked.

## 1. Bundle and loading mechanics

| Item | Value | Evidence | Conf |
|---|---|---|---|
| Bundle | `ResourceConst.InMemoryMasterDataAssetBundleName` (static +0x58) | TitleLoad binds it on-memory (08-assets-cdn) | H |
| Bulk load | `ResourceUtility.LoadAllAssets(bundle, true, false)` | TitleLoad MoveNext @0xB8EDC8 | H |
| Per-asset fetch | `ResourceManager.GetResources(string bundle, string assetName)` → `checkBundleName`, `checkAssetName` (ToLower), `ResourceManagerExec.GetResources` → `assetDepot.Get` | @0x10C51AC, @0x10D370C, @0x10D494C | H |
| Asset names registered | from `AssetBundle.GetAllAssetNames()` in `ResourceManagerExec.CompleteBind` @0x10D690C, i.e. the `m_Container` keys | callers of 0x202CA5C | M |
| Type check | exact class (`klass->typeHierarchy[depth-1] == Jjs.X`), so a subclass or a different class gives null | e.g. TutorialFlagTable.Load @0xB8E288-0xB8E2C0 | H |
| Object model | Unity runs the class ctor before deserializing, so the non-serialized Dictionary fields exist (TutorialFlagTable..ctor @0xB9CF54 news both the List and the Dictionary) | | H |

Each asset needs a MonoBehaviour object (class 114) whose `m_Script` points to a MonoScript in the same file. Set `m_ClassName` to the class name without namespace, `m_Namespace` to `Jjs`, `Jjs.dtbattle` or `ScSystem.ScGraphics`, and `m_AssemblyName` to `Assembly-CSharp.dll`. text.jp's MonoScript has exactly this shape (`TextData`/`ScSystem.ScText`/`Assembly-CSharp.dll`) (H). All root classes are top-level, never nested (H). text.jp holds 111 assets and a preload table of 222 entries: two per asset, the MB plus its MonoScript (H, observed).

## 2. The 18 assets

Each path is `Assets/Resources/Common/InMemoryMasterData/<name>.asset` and each container key is the lowercase of it. The `RC+off` column is the ResourceConst static-field offset holding the path. It was mapped from the cctor stores @0xE29198..0xE292F4 and @0xE2A438..0xE2A454 (H). Note that the two `Int32` statics `DefaultMaxCachePedestalStatueNum`/`GasyaMaxCachePedestalStatueNum` share one 8-byte slot, so the field index × 8 is off by 8 after them.

| # | name | container key | class (ScriptableObject) | RC+off | loader (address) | Conf |
|---|---|---|---|---|---|---|
| 1 | rank_table | assets/resources/common/inmemorymasterdata/rank_table.asset | `Jjs.PlayerRankTable` | 0x3C0 | BattleResultUIController/<SetUI>.MoveNext: GetResources @0xC0F890, isinst @0xC0F89C | H |
| 2 | yogo_table | …/yogo_table.asset | `Jjs.YogoTable` | 0x3C8 | GlossaryUIController.GetYogoList @0xDEB408/@0xDEB414 | H |
| 3 | statue_setting_table | …/statue_setting_table.asset | `Jjs.StatueSettingTable` | 0x3D0 | StatueSettingTable.Load @0xE776F4 (TitleLoad @0xB8F2FC); GetSettingParametor @0xE77984 | H |
| 4 | statue_special_scene_data_table | …/statue_special_scene_data_table.asset | `Jjs.StatueSpecialSceneDataTable` | 0x3D8 | Load @0xE78D64 (TitleLoad @0xB8F36C); GetSpecialSceneData @0xE78FF4 | H |
| 5 | team_rank_table | …/team_rank_table.asset | `Jjs.TeamRankTable` | 0x3E0 | TeamRankUtil.get_teamRankTable @0xB87CB4 (cached) | H |
| 6 | tutorial_flag_table | …/tutorial_flag_table.asset | `Jjs.TutorialFlagTable` | 0x3E8 | Load @0xB8E1E8 (TitleLoad @0xB8F300); TutorialManager.InitFlagList @0xB8E328, HasMainTutorial @0xB8E5EC, IsClearAllTutorial @0xB9D540, DownloadMainTutorialData @0xB90254, TitleLoad.CheckMainTutorial (LoadAsset @0xB8E07C) | H |
| 7 | statue_level_table | …/statue_level_table.asset | `Jjs.LevelTableData` | 0x3F0 | Load @0xB7116C (TitleLoad @0xB8F2EC); BaseStatueInfo.GetLevelTable @0xBFF8E0 | H |
| 8 | mind_level_table | …/mind_level_table.asset | `Jjs.MindLevelTableData` | 0x3F8 | Load @0xD6114C (TitleLoad @0xB8F2F4); BaseMindInfo.GetLevelTableList @0xBFBC64 | H |
| 9 | skill_data_table | …/skill_data_table.asset | `Jjs.SkillDataTable` | 0x400 | Load @0xF988C0 (TitleLoad @0xB8F354), stores static `table` | H |
| 10 | ability_data_table | …/ability_data_table.asset | `Jjs.AbilityDataTable` | 0x408 | Load @0x205E094 (TitleLoad @0xB8F35C); the same Load also reads dtbattle_ability_data_table (+0x510) into static `dtTable` | H |
| 11 | signboard_table | …/signboard_table.asset | **none found** | 0x410 | no reader | M |
| 12 | assist_ability_condition_table | …/assist_ability_condition_table.asset | `Jjs.AssistAbilityConditionTableData` | 0x418 | Load @0xC9208C (TitleLoad @0xB8F364) | H |
| 13 | map_gimmick_pattern_table | …/map_gimmick_pattern_table.asset | `Jjs.MapGimmickPatternTable` | 0x420 | Load @0xD4EB84 (TitleLoad @0xB8F37C) | H |
| 14 | affect_text_rule_table | …/affect_text_rule_table.asset | `Jjs.AffectTextRuleTable` | 0x428 | Load @0xC8F6C4 (TitleLoad @0xB8F3BC) | H |
| 15 | action_branch_data_table | …/action_branch_data_table.asset | `Jjs.ActionBranchDataTable` | 0x430 | Load @0x2060AEC (TitleLoad @0xB8F3DC) | H |
| 16 | area_position_pattern_table | …/area_position_pattern_table.asset | `Jjs.AreaPositionPatternTable` | 0x448 | Load @0xC917B8 (TitleLoad @0xB8F38C) | H |
| 17 | Set_el_camera_set_common | …/set_el_camera_set_common.asset | `Jjs.ScriptCameraParamSetAsset` | 0x538 | ScriptCamera.ResourceSet @0xD70968 (isinst @0xD70A18), called from CameraManagerProxy.Init (TitleLoad @0xB8F414) | H |
| 18 | Node_el_camera_node_common | …/node_el_camera_node_common.asset | `Jjs.ScriptCameraParamNodeAsset` | 0x540 | ScriptCamera.ResourceSet (isinst @0xD70A90) | H |


## 2b. The 27 extra assets in the same bundle (H: path, bundle and class all read from code)

Path = `Assets/Resources/Common/InMemoryMasterData` + `/` + name + `.asset`. The reaction tables use `"/" + "reaction_table_" + {normal,mini,mini_no_blow} + ".asset"` (cctor @0xE29320-0xE29398) and `dtbattle_reaction_table_` likewise. Their schemas were mapped in the analysis and are added to the repo schema together with their content. **TitleLoad reads these** (all null-safe): affect_table (AffectTableManager.SetupExternal @0xC8F310), characteristic_data (@0xF1D048), state_abnormality_table, characterestic_skill_parameter (@0xF1BD9C), dtbattle_skill_data_table (@0xD9B0D4), oraoramode_setting (@0xC42918), statue_affinity_table (@0xCBA50C), dtbattle_statue_affinity_table (@0xCD145C), clan_table (@0xCE5AF8), clan_object_table (@0xCE18D4) and effectReductionParam (`ScSystem.ScGraphics.EffectSettingParam`, @0x1038CF0). **Battle-only:** bullet_parameter (`Jjs.BulletParameter`), additional_statue_load_table, reaction tables ×6 (`Jjs.ReactionTable`), model_table, skill_create_character_table, battle_damage_param (`Jjs.BattleDamageParam`), battle_combo_param (`Jjs.BattlePvpComboParam`), battle_pvp_statue_status_parameter, dtbattle_damage_param, dtbattle_damage_rank_table and dtbattle_ability_data_table (`Jjs.AbilityDataTable`).

## 3. Serialized field trees (core assets)

Rules (H): a field is serialized when it is an instance field, is not static, const, readonly or `[NonSerialized]`, is public or `[SerializeField]`, and has a Unity-serializable type. All Dictionary fields are private and have no `[SerializeField]`, so they are skipped, as are the static singleton fields (`table`, `dtTable`, `<Instance>k__BackingField`) and `ScUtil.Random posRandom`. Private `[SerializeField]` fields that **are** serialized: `MapGimmickPatternTable.paramList`, `AreaPositionPatternTable.paramList/positionList`, `AreaPositionPatternTable/PatternData.posSetList`, and `SecurityIntValue.seed/securityValue`. Field order is declaration order, which IL2CPP metadata preserves. **No root has a user base class**: each derives directly from `UnityEngine.ScriptableObject`, so the serialized object is the Unity header followed by these fields. `List<T>` and `T[]` serialize identically. Every nested element class is `[Serializable]` (flags 0x2000) and is stored inline; Unity 2018 has no SerializeReference, so nested class values are never null.

Small enums (all int-backed, H):
`TutorialFlagTable/LaunchType` Manual=0, Tutorial=1, AreaStart=2, Home=3 · `FormationTeamRank` E=0,D,C,B,A,S,SPlus,SS,SSPlus,SSS,SSSPlus=10 · `SpecialRetireCondition` None=0, Enemy=1, Map=2 · `SkillType` NONE=0..SKILL_TYPE_DEAD_CHANGE=6 · `AffectTextRuleTable/RuleType` Invalid=0, Combine=1, Solo=2 · `AreaPositionPatternTable/PatternData/SetType` Player1..3=0..2, Enemy1..3=3..5 · `StatueType` None=0, Brave, Intelli, Tenacity, Unity, Maverick=5. The large enums `SkillButtonType` (106, Normal=1..DT_SP_KC_ATK=106), `AffectCondition` (30), `BaseAffectType` (170) and `CharacterBoneList/Bone` (72) are listed in full in the JSON.

The value structs `Jjs.LevelTable {int level, exp, hp, atk, def}` and `Jjs.MindLevelTable {int level, exp}` are `[Serializable]` structs. Their static `InvalidValue` is not serialized (H).

### rank_table -> `Jjs.PlayerRankTable`
ScriptableObject, [Serializable]=False; not serialized: -
```
- paramList: List<Jjs.PlayerRankTable/Param>
  - rank: int
  - exp: int
```
Nested types: `Jjs.PlayerRankTable/Param` class [Serializable]=True

### yogo_table -> `Jjs.YogoTable`
ScriptableObject, [Serializable]=False; not serialized: -
```
- paramList: List<Jjs.YogoTable/Param>
  - yogo_Id: string
  - yogo_Title: string
  - yogo_Yomi: string
  - yogo_Text: string
  - yogo_Picture: string
```
Nested types: `Jjs.YogoTable/Param` class [Serializable]=True

### statue_setting_table -> `Jjs.StatueSettingTable`
ScriptableObject, [Serializable]=False; not serialized: paramTable
```
- paramList: List<Jjs.StatueSettingTable/Param>
  - statueId: string
  - myPageId: int
  - myPageCameraType: string
  - myPagePos: Vector3
  - myPageRotation: Vector3
  - teamPos: Vector3
  - teamRotation: Vector3
  - teamScaleScalar: float
  - detailViewPos: Vector3
  - jojoViewPos: Vector3
  - gashaTopViewPos: Vector3
  - mindFrameID: int
  - battleResultComment: string
  - dododoGaugeType: int
  - gashaScriptCamNo01: int
  - pvpAiId: int
  - pvpAiAutoBattleAiId: int
```
Nested types: `Jjs.StatueSettingTable/Param` class [Serializable]=True

### statue_special_scene_data_table -> `Jjs.StatueSpecialSceneDataTable`
ScriptableObject, [Serializable]=False; not serialized: dataTable
```
- dataList: List<Jjs.StatueSpecialSceneDataTable/Data>
  - statueId: string
  - specialRetireData: Jjs.SpecialRetireData
    - condition: Jjs.SpecialRetireCondition (enum:int)
    - conditionParam1: string
    - conditionParam2: string
    - longExcelCameraId: int
    - shortExcelCameraId: int
  - dtSpecialRetireData: Jjs.SpecialRetireData
    - condition: Jjs.SpecialRetireCondition (enum:int)
    - conditionParam1: string
    - conditionParam2: string
    - longExcelCameraId: int
    - shortExcelCameraId: int
  - spcialSkillSceneData: Jjs.SpecialSkillSceneData
    - spSkillVariation: int
    - spSkillSceneParam1: string
    - spSkillSceneParam2: string
```
Nested types: `Jjs.SpecialRetireData` class [Serializable]=True; `Jjs.SpecialSkillSceneData` class [Serializable]=True; `Jjs.StatueSpecialSceneDataTable/Data` class [Serializable]=True

### team_rank_table -> `Jjs.TeamRankTable`
ScriptableObject, [Serializable]=False; not serialized: -
```
- paramList: List<Jjs.TeamRankTable/Param>
  - rank: Jjs.FormationTeamRank (enum:int)
  - passingScore: int
```
Nested types: `Jjs.TeamRankTable/Param` class [Serializable]=True

### tutorial_flag_table -> `Jjs.TutorialFlagTable`
ScriptableObject, [Serializable]=False; not serialized: paramTable
```
- paramList: List<Jjs.TutorialFlagTable/Param>
  - id: int
  - flagLabel: string
  - autoSave: bool
  - launchType: Jjs.TutorialFlagTable/LaunchType (enum:int)
  - scriptPackName: string
  - continueHome: bool
  - resourceDownloadStopForMainTutorial: bool
```
Nested types: `Jjs.TutorialFlagTable/Param` class [Serializable]=True

### statue_level_table -> `Jjs.LevelTableData`
ScriptableObject, [Serializable]=False; not serialized: paramTable
```
- paramList: List<Jjs.StatueLevelTableList>
  - statueId: string
  - levelTableList: List<Jjs.LevelTable>
    - level: int
    - exp: int
    - hp: int
    - atk: int
    - def: int
```
Nested types: `Jjs.LevelTable` struct [Serializable]=True; `Jjs.StatueLevelTableList` class [Serializable]=True

### mind_level_table -> `Jjs.MindLevelTableData`
ScriptableObject, [Serializable]=False; not serialized: paramTable
```
- paramList: List<Jjs.MindLevelTableList>
  - rare: int
  - levelTableList: List<Jjs.MindLevelTable>
    - level: int
    - exp: int
```
Nested types: `Jjs.MindLevelTable` struct [Serializable]=True; `Jjs.MindLevelTableList` class [Serializable]=True

### skill_data_table -> `Jjs.SkillDataTable`
ScriptableObject, [Serializable]=False; not serialized: table, paramTable
```
- paramList: List<Jjs.SkillData>
  - skillId: string
  - skillName: string
  - skillProfileText: string
  - skillType: Jjs.SkillType (enum:int)
  - bottunType: Jjs.SkillButtonType (enum:int)
  - coolDownTime: ScSystem.ScSecurity.SecurityIntValue
  - limitCount: ScSystem.ScSecurity.SecurityIntValue
  - skillRateLv01: ScSystem.ScSecurity.SecurityIntValue
  - skillRateLv10: ScSystem.ScSecurity.SecurityIntValue
  - pvpSkillRateLv01: ScSystem.ScSecurity.SecurityIntValue
  - pvpSkillRateLv10: ScSystem.ScSecurity.SecurityIntValue
  - skillParamList: List<ScSystem.ScSecurity.SecurityIntValue>
```
Nested types: `Jjs.SkillData` class [Serializable]=True; `ScSystem.ScSecurity.SecurityIntValue` struct [Serializable]=True

### ability_data_table -> `Jjs.AbilityDataTable`
ScriptableObject, [Serializable]=False; not serialized: table, dtTable, paramTable
```
- paramList: List<Jjs.AbilityData>
  - abilityId: string
  - abilityName: string
  - abilityParamList: List<Jjs.AbilityData/AbilityParam>
    - affectId: string
    - giveLimit: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_0: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_1: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_2: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_3: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_4: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_5: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_6: ScSystem.ScSecurity.SecurityIntValue
    - paramLv1_7: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_0: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_1: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_2: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_3: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_4: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_5: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_6: ScSystem.ScSecurity.SecurityIntValue
    - paramLv10_7: ScSystem.ScSecurity.SecurityIntValue
```
Nested types: `Jjs.AbilityData` class [Serializable]=True; `Jjs.AbilityData/AbilityParam` class [Serializable]=True; `ScSystem.ScSecurity.SecurityIntValue` struct [Serializable]=True

### assist_ability_condition_table -> `Jjs.AssistAbilityConditionTableData`
ScriptableObject, [Serializable]=False; not serialized: paramTable, table
```
- paramList: List<Jjs.AbilityConditionTable>
  - statueId: string
  - conditionTableData: List<Jjs.AssistAbilityCondition>
    - assistAbilityId: string
    - assistAbilityCondition: List<Jjs.CondData>
      - statueType: Jjs.StatueType (enum:int)
      - count: int
```
Nested types: `Jjs.AbilityConditionTable` class [Serializable]=True; `Jjs.AssistAbilityCondition` class [Serializable]=True; `Jjs.CondData` class [Serializable]=True

### map_gimmick_pattern_table -> `Jjs.MapGimmickPatternTable`
ScriptableObject, [Serializable]=False; not serialized: <Instance>k__BackingField, paramTable, posRandom
```
- paramList: List<Jjs.MapGimmickPatternTable/PatternData>
  - patternName: string
  - bodyPos: Vector3
  - positionList: List<Vector3>
```
Nested types: `Jjs.MapGimmickPatternTable/PatternData` class [Serializable]=True

### affect_text_rule_table -> `Jjs.AffectTextRuleTable`
ScriptableObject, [Serializable]=True; not serialized: pvrsRuleTable, baseAffectRuleTable, affectRuleTable, table
```
- pvrsRuleList: List<Jjs.AffectTextRuleTable/AffectTextRuleParam>
  - pvrs: Jjs.AffectCondition (enum:int)
  - baseAffectType: Jjs.BaseAffectType (enum:int)
  - paramRuleTypeList: Jjs.AffectTextRuleTable/RuleType (enum:int)[]
- baseAffectRuleList: List<Jjs.AffectTextRuleTable/AffectTextRuleParam>
  - pvrs: Jjs.AffectCondition (enum:int)
  - baseAffectType: Jjs.BaseAffectType (enum:int)
  - paramRuleTypeList: Jjs.AffectTextRuleTable/RuleType (enum:int)[]
```
Nested types: `Jjs.AffectTextRuleTable/AffectTextRuleParam` class [Serializable]=True

### action_branch_data_table -> `Jjs.ActionBranchDataTable`
ScriptableObject, [Serializable]=False; not serialized: dtTable, paramTable4Hash
```
- dtParamList: List<Jjs.ActionBranchData>
  - branchId: string
  - actionId_None: string
  - actionId_DamageSmall: string
  - actionId_DamageBig: string
  - actionId_DamageHeavy: string
  - actionId_DamageBlow: string
  - actionId_DamageBind: string
  - actionId_DamageParalysis: string
  - actionId_Down: string
  - actionId_DamageBindDown: string
  - actionId_DamageParalysisDown: string
  - actionId_DamagePull: string
  - actionId_DamageInvidible: string
  - actionId_DamageVidible: string
  - actionId_Die: string
  - actionId_DieDown: string
```
Nested types: `Jjs.ActionBranchData` class [Serializable]=True

### area_position_pattern_table -> `Jjs.AreaPositionPatternTable`
ScriptableObject, [Serializable]=False; not serialized: <Instance>k__BackingField, paramTable, positionTable
```
- paramList: List<Jjs.AreaPositionPatternTable/PatternData>
  - patternName: string
  - posSetList: List<Jjs.AreaPositionPatternTable/PatternData/PositionSet>
    - setType: Jjs.AreaPositionPatternTable/PatternData/SetType (enum:int)
    - positionList: List<Jjs.AreaPositionPatternTable/PatternData/Position>
      - pos: Vector3
      - rotate: Quaternion
      - startPos: Vector3
      - startRotate: Quaternion
- positionList: List<Jjs.AreaPositionPatternTable/PositionData>
  - name: string
  - pos: Vector3
  - rotate: Quaternion
```
Nested types: `Jjs.AreaPositionPatternTable/PatternData` class [Serializable]=True; `Jjs.AreaPositionPatternTable/PatternData/Position` class [Serializable]=True; `Jjs.AreaPositionPatternTable/PatternData/PositionSet` class [Serializable]=True; `Jjs.AreaPositionPatternTable/PositionData` class [Serializable]=True

### Set_el_camera_set_common -> `Jjs.ScriptCameraParamSetAsset`
ScriptableObject, [Serializable]=True; not serialized: -
```
- listData: List<Jjs.ScriptCameraParamSet>
  - cameraID: int
  - beginInter: float
  - endInter: float
  - flagMaintain: bool
  - flagBindBase: bool
  - flagBindLookat: bool
  - flagNowConnect: bool
  - flagWorldSet: bool
  - flagForcusAhead: bool
  - flagNotUpdateBonePosition: bool
  - flagNotInter: bool
  - dataList: List<Jjs.ScriptCameraParamSetData>
    - cameraNode: string
    - sec: float
    - inter: float
    - targetIDBase: int
    - targetIDLookat: int
```
Nested types: `Jjs.ScriptCameraParamSet` class [Serializable]=True; `Jjs.ScriptCameraParamSetData` class [Serializable]=True

### Node_el_camera_node_common -> `Jjs.ScriptCameraParamNodeAsset`
ScriptableObject, [Serializable]=True; not serialized: -
```
- listData: List<Jjs.ScriptCameraParamNode>
  - id: string
  - baseBoneID: ScSystem.ScGraphics.CharacterBoneList/Bone (enum:int)
  - lookatBoneID: ScSystem.ScGraphics.CharacterBoneList/Bone (enum:int)
  - rotate: Vector3
  - perspective: float
  - length: float
  - baseOffset: Vector3
  - lookatOffset: Vector3
```
Nested types: `Jjs.ScriptCameraParamNode` class [Serializable]=True


## 4. Unity serialized form of the special types

### 4.1 Real type-tree evidence (downloaded bundles, Unity 2018.4.1f1, SerializedFile v17, type trees present)
- `map03_enya03_yoru` (`Jjs.CameraSettings` MB): `Vector3f CameraPos size=12 flags=0x200000 {float x,y,z size=4 flags=0x200000}`, `int clearFlag size=4 flags=0x0` (an enum field), `UInt8 normalFog size=1 flags=0x4100` (a bool) (H).
- `frameevents_statue_talk_3320` (`Jjs.FrameEventAssetData`): a managed nested-class list is written as `TFrameInfo FrameMax size=-1 flags=0x8000 / Array Array size=-1 flags=0x8000 / int size 4 0x0 / TFrameInfo data -1 0x8000 {...}`. The field node's type is the **element class short name** (`TFrameInfo` is `FrameEventAssetData/TFrameInfo`, so the outer class is dropped), and this Array node has **no 0x4000**. A managed string is `string x size=-1 flags=0x8000 / Array Array -1 0x4001 / int size 4 0x1 / char data 1 0x1` (H).
- `actionset_common` (PlayMaker, managed): `vector actionStartIndex -1 0x8000 / Array -1 0x4000 / int size 4 0x0 / int data 4 0x0` (int[]); `vector vector4Values -1 0x8000 / Array -1 0x4000 / Vector4f data 16 0x200000`; string[] → `vector -1 0x8000 / Array -1 0xC000 / string data -1 0x8000`; `Quaternionf value size=16 flags=0x200000 {float x,y,z,w 0x200000}` (H).
- A managed class with only fixed-size fields has a positive byte size and flags 0 (`MathManipulatorProperty mathProperty size=52 flags=0x0`, effect_gsh). Any aligned child (bool or string) raises 0x8000 on the parent (`FontData m_FontData size=47 flags=0x8000`) (H).
- Header shared by all MBs: `MonoBehaviour Base -1 0x8000`, `PPtr<GameObject> m_GameObject 12 0x41 {int m_FileID 0x41, SInt64 m_PathID 0x41}`, `UInt8 m_Enabled 1 0x4101`, `PPtr<MonoScript> m_Script 12 0x0 {int m_FileID 0x800001, SInt64 m_PathID 0x800001}`, `string m_Name -1 0x88001 {Array -1 0x84001 {int size 0x80001, char data 0x80001}}` (H).

### 4.2 Node templates to use (0x4000 = align stream to 4 after this node; 0x8000 = some child aligns; 0x200000 = Unity struct)
| C# type | Node(s) | Binary |
|---|---|---|
| int, enum (int-backed) | `int name 4 0x0` | int32 LE (H) |
| float | `float name 4 0x0` | float32 (H) |
| bool | `UInt8 name 1 0x4100` | 1 byte, then pad to 4 (H) |
| long (extras only) | `SInt64 name 8 0x0` | (H) |
| string | `string name -1 0x8000` → `Array Array -1 0x4001` → `int size 4 0x1`, `char data 1 0x1` | int32 byte length + UTF-8 bytes + pad to 4 (H) |
| Vector3 | `Vector3f name 12 0x200000` → `float x,y,z 4 0x200000` | 3×float (H) |
| Quaternion | `Quaternionf name 16 0x200000` → `float x,y,z,w 4 0x200000` | 4×float (H) |
| **SecurityIntValue** (struct) | `SecurityIntValue name 8 0x0` → `int seed 4 0x0`, `int securityValue 4 0x0` | two int32. Value = seed XOR securityValue (H for fields and decoding: fattr flags=0x102109 ser=True, both `[SerializeField]`; `get_Value` @0xA99694 `eor`. Node byte size and flags are M by analogy with MathManipulatorProperty) |
| SecurityFloatValue (extras) | same two ints; float bits = seed ^ securityValue (`get_Value` @0xA9967C) | (H) |
| SecurityStringValue (extras, dtSkillDataTable) | `int seed`, `string securityString`; decoding @0x105FAA4: split on '-', `Convert.ToInt32(part, 16)`, XOR byte i with byte i%4 of the seed, ASCII. An empty string decodes to empty (IsNullOrEmpty early-out). Implemented in Build 11, see 11-battle-and-positions.md §2 | (H) |
| nested [Serializable] class field | `<ShortName> name size 0x8000-if-any-child-aligns` with its fields inline | (H) |
| List<T>/T[] of int/enum/float/Vector3 | `vector name -1 0x8000` → `Array Array -1 0x4000` → `int size 4 0x0`, `<elem> data` | int32 count + elements, pad (H for int/float/Vector4; Vector3 and enum by analogy, M) |
| List<T> of managed class/struct | `<ElemShortName> name -1 (0x8000 if elem aligns, else 0x0)` → `Array Array -1 (same)` → `int size 4 0x0`, `<ElemShortName> data …` | int32 count + elements (H for class elems with strings; M for `List<SecurityIntValue>` → `SecurityIntValue skillParamList -1 0x0 / Array -1 0x0 / SecurityIntValue data 8 0x0`) |

Example (derived with these rules) for `tutorial_flag_table`:
```
MonoBehaviour Base  size=-1 flags=0x8000
  PPtr<GameObject> m_GameObject  size=12 flags=0x41
    int m_FileID  size=4 flags=0x41
    SInt64 m_PathID  size=8 flags=0x41
  UInt8 m_Enabled  size=1 flags=0x4101
  PPtr<MonoScript> m_Script  size=12 flags=0x0
    int m_FileID  size=4 flags=0x800001
    SInt64 m_PathID  size=8 flags=0x800001
  string m_Name  size=-1 flags=0x88001
    Array Array  size=-1 flags=0x84001
      int size  size=4 flags=0x80001
      char data  size=1 flags=0x80001
  Param paramList  size=-1 flags=0x8000
    Array Array  size=-1 flags=0x8000
      int size  size=4 flags=0x0
      Param data  size=-1 flags=0x8000
        int id  size=4 flags=0x0
        string flagLabel  size=-1 flags=0x8000
          Array Array  size=-1 flags=0x4001
            int size  size=4 flags=0x1
            char data  size=1 flags=0x1
        UInt8 autoSave  size=1 flags=0x4100
        int launchType  size=4 flags=0x0
        string scriptPackName  size=-1 flags=0x8000
          Array Array  size=-1 flags=0x4001
            int size  size=4 flags=0x1
            char data  size=1 flags=0x1
        UInt8 continueHome  size=1 flags=0x4100
        UInt8 resourceDownloadStopForMainTutorial  size=1 flags=0x4100
```
If the type tree written differs slightly from what the runtime generates (for example in flags or byte sizes), Unity 2018 falls back to its name- and type-matching (safe binary) reader, provided the bundle carries type trees. Field names and types are what must match (M, general Unity behaviour; not verified in this binary). The safest choice is to build the bundles with type trees, as all the existing downloaded bundles are.

## 5. Boot path: TitleLoad → MyPage

TitleLoad.Load MoveNext (@0xB8E8C0) runs, in order:
1. getkey, then SystemConfig, then `get_tutorial_step` (the request is built @0xB8FC9C).
2. `LoadAllAssets(in_memory_master_data)` @0xB8EDC8, then `TutorialManager.DownloadMainTutorialData` @0xB8F0C0.
3. The table loads @0xB8F2E4..0xB8F40C (list below).
4. `CameraManagerProxy.Init` @0xB8F414 (camera Set/Node assets), then `EffectManager.SetupReductionSetting` @0xB8F43C.
5. `TutorialManager.UpdateTutorialFlag(server list)` @0xB8F8A8.
6. `HasMainTutorial(Tutorial, checkEnable=false)` @0xB8F9E4. A non-null result starts the tutorial; null leads to user_info and then the scene change.
7. MyPage: `MypageUIController/<StartLoad>` calls `StartTutorials(Home=3)` @0xC39D90. `MyPageController/<StartLoad>` @0xC368E8 and `MypageUIController/<OnLoad>` @0xC38C18 call `StatueSettingTable.GetSettingParametor(leader.statueId)`.

| Table | Read during boot? | Asset missing / list empty | Rows needed | Conf |
|---|---|---|---|---|
| tutorial_flag_table | yes (Load, InitFlagList, UpdateTutorialFlag, HasMainTutorial, IsClearAllTutorial, DownloadMainTutorialData) | Missing: Load returns, the flag list stays empty, HasMainTutorial returns null (@0xB8E718), DownloadMainTutorialData returns, and **IsClearAllTutorial returns false** (@0xB9D65C). Its callers are FooterUIController.OnEventCallback @0xC26448, BizarreCategorySelectUIController.SetQuestLock @0xD3D988, BattlePauseUIController.SetupUI, SignboardController and dtbattle EndBattle (effects not traced). Empty list: no tutorial runs and IsClearAllTutorial returns true, but every `IsFlagOn(label)` is false, so the daily and event quest categories stay locked. SetQuestLock reads OPEN_00/02/01/03 (GameConst+0x500/0x510/0x508/0x518 @0xD3D8B8..0xD3DABC), FooterUIController reads OPEN_00/01 (@0xC26478/@0xC264BC), and TrophyUIController reads OPEN_00/01/04 (@0xB9B20C..0xB9B31C). | see section 6 | H |
| statue_setting_table | yes (Load; MyPage BG and leader position) | Safe. `GetSettingParametor` returns null when the asset or the key is missing. MyPageController skips the BG load (`cbz x0` @0xC368F0) and MypageUIController uses Vector3.zero (@0xC38C28). You get no MyPage background and the leader at the origin. | One row per leader `statueId` (string; from server TeamInfo, main-team leader `statueInfo.statueId`, read via `SkinChangeManager.leaderStatueInfo`+0x10 @0xC368D4). `myPageId` selects the BG asset (`GetMyPageBGAssetName`). Also used by gasha, formation and battle-result screens. | H (null paths); M (visual impact) |
| statue_level_table | Load only | Safe. `LevelTableData.GetParametor` returns an empty new list on a miss (@0xB714AC); `BaseStatueInfo.GetLevelTableData` checks null and Count and returns `LevelTable.InvalidValue` (@0xBFFAAC/@0xBFFAE0). | Per statueId: levels 1..max with exp/hp/atk/def, for level-up and exp UIs | M |
| mind_level_table | Load only | Safe (the same pattern, @0xD6148C / BaseMindInfo @0xBFBD84) | per `rare` int | M |
| skill_data_table, ability_data_table | Load only | Safe. `GetParametor` returns `new SkillData()` / `new AbilityData()` when the table is null or the key is missing (@0xF98BFC, @0xF98D28, @0x205E4D4, @0x205E5F8) | skillId / abilityId strings referenced by server statue/mind info | H |
| assist_ability_condition_table, statue_special_scene_data_table, map_gimmick_pattern_table, affect_text_rule_table, action_branch_data_table, area_position_pattern_table | Load only | Safe: Load is guarded and Init loops 0 times. Getters return null or a default (`StatueSpecialSceneDataTable.GetData` returns null @0xE79210; `ActionBranchDataTable.GetParametor` returns null when the table is null (@0x2060F04); `AffectTextRuleTable.GetParametor` returns a new default param). Consumers are battle and detail screens. | battle content | M |
| Set_/Node_el_camera_* | CameraManagerProxy.Init → ResourceSet stores the reference (it may be null) | Safe. `ScriptCamera.PlayStart` checks both assets for null (op_Equality @0xD702F0/@0xD70320) and a failed `List.Find` (@0xD703C0), and returns false. Script cameras (gasha, special skill) then do nothing. | cameraID sets used by scripts and gasha | H |
| team_rank_table | no (formation screens) | Safe. `AnalyzeTeamRank` returns the default (E) when the list is null or Count==0 (@0xB87B58/@0xB87B6C) | 11 thresholds, E..SSSPlus | H |
| rank_table | no (battle result only) | **An empty list is safe** (`GetRankParam` returns `new Param()` when Count==0, @0xDC1930/@0xDC1988). **A missing asset is not**: BattleResult stores the null table (@0xC0F8F0) and later calls GetRankParam on it, so expect an NRE (M). | rank 1..max with cumulative exp; the user's rank comes from the server UserInfo | M |
| yogo_table | no (glossary) | Missing asset: `GetYogoList` returns null (@0xDEB47C), so the glossary may NRE (M). Empty list: an empty glossary. | - | M |
| signboard_table | never | - | - | M |

Summary (M): **no core table needs a non-empty row list to reach an interactive MyPage**. Every asset must exist with its exact class (or be safely absent), except as follows. tutorial_flag_table needs the rows from section 6. statue_setting_table needs a row for the leader statueId to get the BG and a sensible model position. rank_table must at least exist (it may be empty) before the first battle result. The extras behave the same way: they are needed for battles, not for boot.

## 6. tutorial_flag_table

`Jjs.TutorialFlagTable { List<Param> paramList; }`. `Param` is `[Serializable]` and its fields are, in order (offsets from Param+0x10):

| field | type | off | meaning (evidence) |
|---|---|---|---|
| id | int | 0x10 | **Index into the flag list** (IsFlagOn @0xB9BE74/0xB9BEA8, OnSetFlag @0xB9D4E4) |
| flagLabel | string | 0x18 | Dictionary key (Init @0xB9D0AC, ContainsKey guard); looked up by GetFlagParameter. Labels containing `"TUTORIAL_"` are "main" tutorials. |
| autoSave | bool | 0x20 | Saves to the server when the script ends (OnScriptEndFunc) (M) |
| launchType | LaunchType(int) | 0x24 | Manual=0/Tutorial=1/AreaStart=2/Home=3. HasMainTutorial compares it (@0xB8E7B0); IsClearAllTutorial ignores Manual rows (@0xB9D6E8) |
| scriptPackName | string | 0x28 | Script bundle name. **DownloadMainTutorialData downloads `<pack>.bytes` and its language pack for every TUTORIAL_ row with a non-empty pack, regardless of flags** (@0xB9044C..0xB90528) |
| continueHome | bool | 0x30 | (not traced, L) |
| resourceDownloadStopForMainTutorial | bool | 0x31 | (not traced, L) |

Runtime semantics (H):
- `InitFlagList` @0xB8E328 builds `tutorialFlagList` = `paramList.Count` entries of BoolType.False (0). Its length is the **row count**, not max(id)+1.
- `UpdateTutorialFlag(server)` @0xB8E4E4 copies `server[i]` for `i < min(server.Count, local.Count)`. Extra server entries are ignored.
- `IsFlagOn(label)` @0xB9BE28 returns false if the label is unknown, if `id < 0`, or if `id >= Count`. Otherwise it returns `flags[id] == 1`. `OnSetFlag` uses the same bounds check, so it never crashes.
- `HasMainTutorial(lt, checkEnable)` @0xB8E5EC walks the rows in list order and skips labels without "TUTORIAL_" and rows that are already on. At the **first unfinished TUTORIAL_ row**: if its launchType ≠ lt it returns null; if `!checkEnable` it returns the row; otherwise it returns the row when `IsEnableLaunchTutorial` holds, else it marks the row on and continues.
- `IsClearAllTutorial` @0xB9D540 returns true when every TUTORIAL_ row with launchType ≠ Manual is on. It returns false if the asset is missing.
- `CheckAndStartTutorial(label)` → `StartTutorial(label)` (@0xB9E844): an unknown label means no tutorial, which is safe.

Labels the client code queries (H, from GameConst cctor @0xDB3ED4..0xDB48C0 and literal xrefs):
- `MainTutorialFlagLabelList` = TUTORIAL_00, TUTORIAL_04. `MainHomeTutorialFlagLabeList` = TUTORIAL_05, 06, 07, 11.
- Literal uses: TUTORIAL_01, 02, 08, 09, 10, 11 (BattleUIController.SetForTutorial @0xD37FA8..), TUTORIAL_05 (StatueResultInfoUIController @0xE73C3C), TUTORIAL_00/01/07/08/09 (IsEnableLaunchTutorial).
- Hints (CheckAndStartTutorial from about 18 screens): HINT_00..HINT_08, HINT_10..HINT_17. HINT_09 does not exist.
- Feature unlocks (read by SetQuestLock, FooterUIController, TrophyUIController and IsBizarreCategoryUnLock): OPEN_00 (=OPEN_FLAG_BIZARRE_DAILY), OPEN_01 (EVENT), OPEN_02 (DAILY_ANIM), OPEN_03 (EVENT_ANIM), OPEN_04 (MISSION_DAILY_ANIM). `IsBizarreCategoryUnLock` = (OPEN_00 && !OPEN_02) || (OPEN_01 && !OPEN_03), meaning "an unlock animation is pending".
- TUTORIAL_03 is not referenced by code but is probably a real row (L). The real table almost certainly has more TUTORIAL_xx rows, driving the main_tutorial_NN scripts (03-progression-storage). Rows the code never names only matter for HasMainTutorial and IsClearAllTutorial, and all-ones covers them.

**Minimal "everything done" table (M; follows from the semantics above):**
- One row per label: TUTORIAL_00..TUTORIAL_11 (12), HINT_00..08 and HINT_10..17 (17), and OPEN_00..OPEN_04 (5). That is 34 rows with **ids 0..33, dense and unique**.
- Set `launchType` to Tutorial(1) for TUTORIAL_00..04 and Home(3) for TUTORIAL_05..11 (M; not critical when all flags are on). Use Manual(0) for HINT_ and OPEN_ rows.
- Set `scriptPackName = ""` so that DownloadMainTutorialData requests nothing. Set all bools false.
- The server returns `tutorialStep` as at least 34 entries of 1. Then: HasMainTutorial(Tutorial/Home) returns null, IsClearAllTutorial returns true, no hints appear, the daily and event categories are unlocked, and no unlock animation plays.
- If the server stores what the client posts back (`/user/tutorial`), the list length the client sends equals the row count (34).

Alternative: with an empty `paramList`, no tutorial runs and IsClearAllTutorial returns true. But OPEN_00/01 then read as false, so the daily and event quest categories stay locked (SetQuestLock, Footer and Trophy all read OPEN_00/01, H). This is not recommended.

## 7. Open questions
1. The real row content of every table: statue/skill/ability ids, level curves, the camera sets used by gasha and special-skill scripts, area/gimmick patterns. Captured traffic or another dump is needed. The schema alone does not make battles or gasha work.
2. signboard_table: is it really dead in 3.12? No reader was found (M). Its original class is unknown.
3. The exact runtime type-tree flags for `List<SecurityIntValue>`, enum arrays and `List<Vector3>` are inferred by analogy (M). Shipping type trees should make small mismatches harmless, but this is untested in this binary.
4. ~~SecurityStringValue encoding~~: solved in Build 11 (11-battle-and-positions.md §2).
5. Whether the in-memory bundle must be JJEC-encrypted or may be plain UnityFS (see 08-assets-cdn: versionfile is plain). Build 06 serves it plain and lists it in versionfile with isEncrypt=false; to be confirmed on device. This is not covered here.
6. The full original tutorial row list (TUTORIAL_12+ and the main_tutorial_NN mapping) is needed only to make the tutorials playable.
7. The meaning of `continueHome` and `resourceDownloadStopForMainTutorial` (not traced).
