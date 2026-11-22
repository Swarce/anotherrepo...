# 10. Content spec: My Page, ownership, story quests, events (JoJo DR 3.12.0)

Scope: what our server must return, and which `in_memory_master_data` rows must exist, so that
(a) My Page shows its background and the leader model, (b) the player owns statues and minds that
display in the team, list and detail screens, (c) story quests can be listed, started and finished,
(d) events appear.

Evidence: `game/research/dump.cs` (signatures), arm64 disassembly (`tools/armdis.py`, addresses are
`libil2cpp.so` addresses as printed in dump.cs), UnityPy listings of the player's bundles (decrypted
with `jdr-tool`), and the label column of `local/l10n/work2.tsv`. Field offsets were derived from
declaration order (object header 0x10, 8-byte alignment for references) and checked against the
loads seen in the disassembly.
Confidence: **H** read directly from code or files; **M** one step of inference; **L** guess.
No game text appears here; only ids, labels and asset names.

Companion: `research/E-content-ids.json` (id inventory; section 6 gives the counts).
Earlier notes used: D-masterdata.md / docs/research/09 (table schemas), B-boot.md (boot sequence,
JsonUtility rules), A-envelope.md.

Reminder (B-boot §1, H): responses are parsed with `UnityEngine.JsonUtility`. Missing keys keep
their defaults, nested `[Serializable]` objects and lists are never null, and `DateTime`, `TimeSpan`
and `Dictionary` fields are never read. "Required" below therefore means "must hold a meaningful
value", not "must not be null".

---------------------------------------------------------------------------------------------------

## 0. TL;DR

1. **The background is a 2D image, not a 3D scene, and it comes from the login response.**
   `MyPageController/<StartLoad>` reads `SkinChangeManager.leaderStatueInfo`. That value is set only
   from `LoginResponse.teamInfo` (and later by formation edits), and only when that team passes
   `IsMainTeam(dtMode=true)`, i.e. **`stageType == 1` (DTBattle) and `mainFlg == 1`**. Today the
   login sends `teamInfo: {}`, so the leader is null and the BG load is skipped (H).
2. Minimal fix: in `/user/login` (existStatus 0) send a DT main team whose leader `statueInfo.statueId`
   is, for example, `"1000_01"`. Add a `statue_setting_table` row `{statueId:"1000_01", myPageId:10}`.
   The client then loads `bg_0010.bytes` from bundle `ui_mypage_bg_00`, which is present. The 21 valid
   myPageIds are 10–13, 20–23, 30–35, 40–45 and 50 (H).
3. The **3D leader model** comes from `/team/team_info`, using the same test (DT team with
   `mainFlg 1`). It needs `statue_<psid>` plus `animation_statue_common_<psid>` (and the other MyPage
   families when present), where `psid = int(statueId[0:4])`. Its position is
   `anchor + myPagePos*0.01` with rotation `Euler(myPageRotation)` from the same setting row (H).
4. Statue, mind and item data is **server-driven**: the client keeps no statue, mind or stage master,
   only skill, ability and level tables. `name`/`description` fields carry **text labels**
   (`STATUE_NAME_1000_01`, `MIND_NAME_0001`, `CHAPTER_NAME_ST_01`, …), which the client resolves with
   `TextManager.GetString` (H for statues and chapters).
5. **Stages are server-defined.** `bizarre_start` returns the full battle: `areaList[]` with
   `mapId`, the area scene name, waves and enemies with stats (Battle mode), or `areaSettingList` plus
   `formation` (DT mode). The client provides only maps (`mapNN_xxx_{base,hiru,yoru,collider[_dt]}`),
   area spawn scenes (`area_st_CC_SS_AA`), AI tables (`aipack`), chapter-map images and talk scripts.
6. What must come from wikis or be invented: statue and mind stats and rarity, skill and ability
   numbers, level curves, enemy line-ups, rewards, stamina costs, rank curve, and DT formation
   positions.

---------------------------------------------------------------------------------------------------

## 1. My Page background and leader model

### 1.1 Background chain (H)

| Step | Code (address) | What happens |
|---|---|---|
| 1 | `LoginChecker/<OnCheckLogin>` 0xB7C994 | `new TeamInfoEventParameter(loginResponse.teamInfo)` is posted as UI event type 10 |
| 2 | `SkinChangeManager.UIEventCallback` 0xF9A6A8 | if `EventType==10` and `teamInfo.IsMainTeam(true)` and `teamOrder != null`, it sets `leaderStatueInfo = teamInfo.GetStatueInfo(Leader=1)` = `teamOrder.leaderStatue.statueInfo` (0xB867C8) |
| 2b | `FormationDataBase.SetTeamInfo` 0xC2830C | posts the same event after formation edits (the only other caller of the ctor 0xF9A7F8) |
| 3 | `MyPageController/<StartLoad>.MoveNext` 0xC367A4 | `CommonUIManager.InitPool(ImagePoolType.MyPageBG=4, …)`, then `leader = SkinChangeManager.leaderStatueInfo` (0xC368D4). **null → skip** (`cbz` 0xC368DC) |
| 4 | same, 0xC368E8 | `param = StatueSettingTable.GetSettingParametor(leader.statueId)`. **null → skip** (0xC368F0) |
| 5 | `ResourceConst.GetMyPageBGAssetName` 0xE2C1A4 | `MyPageBGPrefix ("bg_") + String.Format("{0:0000}", param.myPageId) + ".bytes"` |
| 6 | `CommonUIManager.LoadResource(4, bgImageObj, name, …)` 0xC3697C | loads from `mypageBGAssetBundleNameList` = {`ui_mypage_bg_00.bytes`} (cctor 0xE28CCC, H). The pool-to-bundle mapping inside LoadResource was not traced (M) |

`GetSettingParametor` (0xE77984) → `GetParametor` (0xE77AD0) does an exact, case-sensitive
`Dictionary<string,Param>` lookup on `statueId`. It returns null for an empty id or an unknown key (H).

**Valid myPageIds (H, `ui_mypage_bg_00` container):** 10, 11, 12, 13, 20, 21, 22, 23, 30, 31, 32, 33,
34, 35, 40, 41, 42, 43, 44, 45, 50 (21 entries). Each is a TextAsset
`assets/resources/common/assetbundleconverted/mypagebg_convert/android/ui_mypage_bg_00/bg_00NN.bytes`
whose payload is itself JJEC-encrypted image bytes. The bundle file itself is plain UnityFS.

`SkinChangeManager.GetPart()` (0xF9A7E0) returns `leaderStatueInfo.appearedPart` (+0x2C), or 4 when
there is no leader. It selects the UI skin by JoJo part (M). Send `appearedPart = psid / 1000`. That
matches the model bundles (1xxx = Part 1 … 5xxx = Part 5) (M).

### 1.2 Leader model chain (H)

`MypageUIController/<OnLoad>c__Iterator2` (0xC37C34):

1. Queues user_info, team_info, check_forced_notify, event_list, bizarre_resumption and banner_list.
2. Scans `TeamInfoResponse.teamList` (+0x28) and takes the **first** entry for which
   `IsMainTeam(true)` holds (0xC38708).
3. `statueInfo = team.teamOrder(+0x20).leaderStatue(+0x10).statueInfo(+0x18)` (0xC388D8..0xC38900).
4. `CharacterLoadCommand{personalSituationIdList=[statueInfo.personalSituationId], loadResourceType=MyPage(0)}`
   is sent as event 1 (0xC389E0). Then the cue sheet `StatueCueSheetKey(psid, 7)` is added and event 6
   sent (0xC382A8..0xC38304). Then a create command is sent as event 8 with callback `<>m__0` (0xC383A8).
5. The callback (0xC38B70) calls `GetSettingParametor(statueId)`. If the row exists:
   `position = anchor.position + myPagePos(+0x28) * 0.01`
   (constant at 0x2089A84 = 0.01) and `rotation = anchor.rotation * Euler(myPageRotation(+0x34))`.
   If the row is missing, both are zero (0xC38C9C).

If no team qualifies, the model step is skipped and nothing crashes (B-boot, H).

`TeamInfo.IsMainTeam(dt)` (0xB86718, H): `dt ? (stageType==1 && mainFlg==1) : (stageType==0 && mainFlg==1)`.
**My Page and SkinChangeManager both call it with `dt=true`.**

`BaseStatueInfo.personalSituationId` (0xBFE588 → `ResourceConst.GetStatuePersonalSituationId`
0xE2C5A8): empty id → 0 / -1, otherwise `Convert.ToInt32(statueId.Substring(0,4))` (H).
So statueId must start with 4 digits.

### 1.3 Fields read (H unless marked)

| Class | Field (offset) | Use |
|---|---|---|
| TeamInfo | `stageType` (+0x14), `mainFlg` (+0x18) | IsMainTeam |
| TeamInfo | `teamOrder` (+0x20) | null check (SkinChangeManager 0xF9A768) and slot access |
| TeamOrder | `leaderStatue` (+0x10); `memberLeftStatue` (+0x18) role 2; `memberRightStatue` (+0x20) role 3 | GetStatueInfo(role) |
| StatueSetting | `statueInfo` (+0x18); `IsExistsStatue` = `statueInfo.IsValid()` = statueId non-empty (0xE6C7C8, 0xBFFE4C) | empty slot = statueInfo with "" id |
| StatueInfo | `statueId` (+0x10) | setting-table key and personalSituationId |
| StatueInfo | `appearedPart` (+0x2C) | UI skin part |
| StatueInfo | `modelDataList` (+0x60) | `ModelName(type)` returns `modelDataList[type-1]`, Human=1, Stand=2 (0xBFF798). Callers are the dtbattle `CharacterProxyManager` (0xEF35A4, 0xEF8BF0…), i.e. character creation and skill characters. Not read on My Page itself (M) |
| StatueSettingTable/Param | `statueId` +0x10, `myPageId` +0x18, `myPageCameraType` +0x20 (reader not found, L), `myPagePos` +0x28, `myPageRotation` +0x34 | BG and model placement |

### 1.4 Statue bundles for a statueId (H for names; M for which ones are optional)

`psid = int(statueId[0:4])`. Bundle-name prefixes come from the `ResourceConst` cctor
(String[][] tables at static +0x1A0..+0x1C0, indexed by `LoadStatueResourceType`
MyPage=0, Home=1, Battle=2, Clan=3, Talk=4, Gasha=5, All=6; 0xE2697C..0xE28434).

| Family | MyPage | Battle | Talk | Notes |
|---|---|---|---|---|
| `statue_<psid>` (model) | yes | yes | yes | `GetStatueModelAssetBundleName` 0xE2AFA8. Contains `<psid>_0_<name>.fbx` (human) and, for 68 psids, `<psid>_1_<name>.fbx` (stand) |
| `animation_statue_common_<psid>` | yes | yes | – | |
| `animation_statue_battle_<psid>` | – | yes | – | |
| `animation_statue_talk_<psid>` | – | – | yes | |
| `actionset_statue_<psid>` / `dt_actionset_statue_<psid>` | yes | yes | yes | which one depends on BattleType (`GetStatueActionSetAssetBundleNameList` 0xE2B238) |
| `frameevents_statue_ste_<psid>` | yes | – | – | battle uses `frameevents_statue_battle_`, talk uses `_talk_` |
| `effect_statue_common_<psid>`, `effect_statue_ste_<psid>`, `statue_accessory_<psid>` | yes | battle: common + `effect_statue_battle_` | `effect_statue_talk_` | only 24, 97 and 16 psids have these, so the loader must skip missing names (M) |
| `<psid>` (no prefix) | sound CPK | | | `GetStatueSoundCPKName` 0xE2C508 (M: 137 numeric bundles match the psids) |

`modelDataList` for a statue = `["<psid>_0_<name>", "<psid>_1_<name>"]` (the stand entry only when it
exists). The values are the .fbx asset names inside `statue_<psid>`. They are listed per statue in
E-content-ids.json (`modelDataList`). The fbx path prefix is
`Assets/Resources/Common/Graphics/Character/` (ResourceConst +0x1F8, M).

Thumbnails (H): `st_<statueId>.bytes` in `ui_statue_00`/`ui_statue_01`
(`GetStatueThumnailAssetName` 0xE15CE8); `mt_<digits after last '_' of mindId>` in
`ui_mind_00/01` (0xE2BDDC); `it_{itemId:D4}` in `ui_item_00` (0xE2BCF4).

### 1.5 Minimal My Page fix (M; H for every individual step)

```jsonc
// /user/login (existStatus 0) and /team/team_info. Show the same DT main team in both.
"teamInfo": {
  "teamNum": 1, "stageType": 1, "mainFlg": 1, "useFlg": 1,
  "teamOrder": {
    "leaderStatue":      { "statueuId": 1, "statueInfo": { /* StatueInfo, §2.2 */ "statueId": "1000_01", "statueuId": 1,
                           "name": "STATUE_NAME_1000_01", "appearedPart": 1, "rare": 3, "level": 1,
                           "modelDataList": ["1000_0_jonathan"] },
                           "equipMindList": [], "mindLinkList": [], "assistStatueList": [], "AiId": "" },
    "memberLeftStatue":  { "statueuId": 0, "statueInfo": { "statueId": "" } },
    "memberRightStatue": { "statueuId": 0, "statueInfo": { "statueId": "" } }
  },
  "jojoLinkList": []
}
```
statue_setting_table row: `{statueId:"1000_01", myPageId:10, myPageCameraType:"", myPagePos:(0,0,0),
myPageRotation:(0,0,0), teamScaleScalar:1.0, all others 0/""}`.
Check: `statue_1000`, `animation_statue_common_1000`, `actionset_statue_1000`,
`dt_actionset_statue_1000`, `frameevents_statue_ste_1000`, `effect_statue_ste_1000` and `st_1000_01`
are all present. 584 statueIds pass the same check (section 6).

---------------------------------------------------------------------------------------------------

## 2. Ownership responses

### 2.1 Endpoints (paths from PROTOCOL.md / B-api-paths.tsv, H)

| Screen | Path | Request | Response (top level) |
|---|---|---|---|
| boot / My Page | `/user/login` | model, os, osVersion, backupId, ad_id | `teamInfo: TeamInfo`, `buyUncompleteFlg`, `userInfoMini: UserInfoMini`, `existStatus`, `existUser[]`, `is_ca`, `ca_html` |
| My Page, formation, support select | `/team/team_info` | – | `teamList: List<TeamInfo>`, `mindSeqInfo: MindSeqInfo` |
| formation edits | `/team/set_team` | teamNum, mainFlg, leaderstatueuId, memberLeftStatueuId, memberRightStatueuId | `teamInfo: TeamInfo` |
| | `/team/statue_select` | teamNum, statueSeq, statueuId | `teamInfo` |
| | `/team/leader_change` | teamNum, statueSeq (TeamStatueRole) | `teamInfo` |
| | `/team/equip_mind` | teamNum, statueSeq, equipSeq, minduId | `teamInfo` |
| | `/team/equip_multiple_minds` | teamNum, statueSeq, mindList[{equipSeq, minduId}] | `teamInfo` |
| | `/team/equip_multiple_assist` | teamNum, statueSeq, statueList[{equipSeq, statueuId}] | `teamInfo` |
| | `/team/auto_set_and_equip` | teamNum, changePlan | `teamInfo` |
| | `/team/set_team_all` | teamList[EquipTeamInfo] | `teamInfo` (types.py: same 355-line tree) |
| | `/team/save_all_flg` | statueSortType, mindSortType, releaseStatueList[int], releaseMindList[int] | (base only) |
| statue list | `/statue/statue_list` | – | `allStatueNum:int`, `statueList: List<StatueInfo>`, `sortType:int` |
| statue detail | `/statue/statue_info` | statueuId:int | `statueInfo: StatueInfo` |
| | `/statue/new_flg_release`, `/statue/protect`, `/statue/save_sort` | see B-api-paths | base only (M) |
| mind list | `/mind/mind_list` | – | `mindMax`, `mindList: List<MindInfo>`, `mindOverFlg`, `sortType`, `materialSortType`, `goldRatioInteger`, `goldAddition`, `storageInfoList: List<{storageId,count,capacity}>` |
| mind detail | `/mind/mind_info` | minduId, storageId | `mindInfo: MindInfo` |
| mind storage | `/mind/mind_storage_list` | storageId | (not dumped; same MindInfo list, M) |
| items | `/item/item_list` | – | `itemList: List<ItemInfo>` |
| exp items | `/item/exp_item_list` | – | `ExpItemListResponse` (ExpItemInfo: ItemInfo + stack, exp, bonusCondition, bonusRate, useGold) |
| book | `/book/statue_list`, `/book/mind_list`, `/book/item_list` | – | see t_*Book trees (not needed for a–d) |
| user | `/user/user_info` | id | `userInfo: UserInfo`, `halt:int`, `backgroundId:int` |

### 2.2 StatueInfo (BaseStatueInfo + StatueInfo), full tree

Offsets from 0x10. `levelUpTable` sits at +0x130 (H, `get_LevelUpTable` 0xBFF8A4).
"Req" = must be meaningful for (a)/(b). "MD" = the client fills or overrides it from master data.

```
statueId            string   Req  "PPPP_VV" (st_ thumbnail, setting key, psid)          H
name                string   Req  text label "STATUE_NAME_<statueId>" (TextManager.GetString, 0xDC5D18)  H
nameSortIndex       int           sort key                                              M
rare / rareMax      int      Req  rarity stars (UI, pedestal "daiza" GetPedestalAssetPathName(part, rank)) M
appearedPart        int      Req  JoJo part 1..8 (UI skin; pedestal)                    M
description         string        label, e.g. "PROF_TEXT_<statueId>"                    M
specialActionText   string        label                                                 L
charaTypeId         int           character grouping (same chara, different versions)    L
tribeTypeList       List<{tribeTypeId: StatueAttributeType}>  Vampire=1..ResistKinetic=24
statueType          StatueType    None=0, Brave=1, Intelli=2, Tenacity=3, Unity=4, Maverick=5
oraGauge, standRange int
modelDataList       List<string> Req  ["<psid>_0_x", "<psid>_1_y"]                      H (ModelName)
mindCapacity        int           mind cost cap
useAttr             UseAttribute  None=0, Playable=1, EventOnly=2, EnemyOnly=3          (send 1)
attckType           int
addProfileName, addProfile string
maxLevel, sellVm, level, bounds, exp   int   Req level/maxLevel
hp, eng, atk, def   int      Req  current stats (shown and used in battle)
skillList           List<SkillInfo>       MD (see 2.4)
abilityList         List<AbilityInfo>     MD
leaderAbilityInfo   AbilityInfo           MD
dtSkillList         List<SkillInfo>       DT mode, MD from dtbattle_skill_data_table
dtAbilityList       List<DtAbilityInfo>
dtLeaderAbilityInfo DtAbilityInfo
characteristicList  List<{id:int, level:int}>
assistAttr          AssistAttribute Common=0, OnlyAssist=1
assistSkillInfo     SkillInfo; assistAbilityList List<AbilityInfo>
dtAssistSkillInfo   SkillInfo; dtAssistAbilityList List<DtAbilityInfo>
speed, evasion, hitrate, criticalRate int   (DT stats; Build 44: statues.json `tactical` where a
                    wiki gives them, 17 statues, else our 100/0/100/5)
stEffRateInfo       dtbattle.StEffRateInfo {stRateId, text, hamonAttack, bind, atkDown, defDown,
                    hitDown, avoDown, ctDown, spdDown, sripDamage, stun, vampireAttack, mindAttack,
                    aging, nodef, changeSType, stEffRateConfig}
maxSp               int
useStageType        UseStageType Both=0, Battle=1, DtBattle=2
levelUpTable        List<LevelTable{level,exp,hp,atk,def}>  if non-empty it overrides statue_level_table (0xBFF8A4..0xBFF8CC) H
--- StatueInfo ---
statueuId           int      Req  unique instance id (team, set_team, statue_info)      H
treeuId             string
newFlg, useFlg, protectedFlg int
limitReleaseCount, convergence int
getDate             string        "yyyy-MM-dd HH:mm:ss"
evolutionFlg        EvolutionFlag Disable=0/Enable=1; openEvolutionTreeFlg likewise
evolutionLevel, evolutionConvergence, evolutionUseGold int
evolutionUseItemList List<ItemInfo>
```
The full nested trees (AbilityParam's 16 ints, SkillParam, StEffRateInfo) are in
`e_work/t_StatueListResponse.txt` (generated with `bscripts/types.py`).

### 2.3 TeamInfo / StatueSetting / mind slots

```
TeamInfo { teamNum:int, stageType:BattleType(-1/0/1), mainFlg:int, useFlg:int,
           teamOrder: TeamOrder{ leaderStatue, memberLeftStatue, memberRightStatue : StatueSetting },
           jojoLinkList: List<JojoLinkInfo{charaLinkId, type, dataList[{dataId}], name, matchType, abilityId, abilityInfo}> }
StatueSetting { statueuId:int, statueInfo:StatueInfo, equipMindList:List<EquipMindInfo>,
                mindLinkList:List<MindLinkInfo{mindLinkId, dataList[int], abilityInfo}>,
                assistStatueList:List<AssistStatueEquipInfo(StatueInfo + seq)>, AiId:string }
EquipMindInfo = MindInfo + seq:int.  IsValid = mindId non-empty && seq != 0 (0xD12CD8)       H
MindSeqInfo { seqInfoList: List<SeqInfo{seq, openFlg(1=open), goldenFlameFlg, onlySkillFlg}>,
              unlockInfo: {openRank, openMaterialList[MindMaterialInfo], itemList[MindMaterialInfo]} }
```
Rules:
- Send one team per stageType with `mainFlg: 1`. The **DT team (stageType 1)** drives My Page and
  SkinChangeManager (H). The Battle team (stageType 0) is the one `IsMainTeam(false)` finds for
  action-mode screens (M).
- `teamNum` 1..N per type. The original team count per type is unknown; N=5 is a guess (L).
- `StatueSetting.statueuId` must equal `statueInfo.statueuId` (M).
- An empty slot is a StatueSetting with `statueuId 0` and `statueInfo.statueId ""` (IsExistsStatue false, H).
- `mindSeqInfo.seqInfoList`: seq 1..N with `openFlg 1` (M; IsOpen = openFlg==1, 0xC3DA70).

### 2.4 SkillInfo / AbilityInfo are filled from master data (H)

`SkillInfo` getters call `SetupFromSkillData` (0xF98D78). It runs once (`isInit` +0x58) and only
if `skillId` is non-empty:
- In DT mode (`BizarreQuestSceneManager.CurrentBattleType==1`) it reads
  `dtSkillDataTable.GetSkillData(skillId)` (0xD8D958) and overwrites coolDown, limitCount and name
  (SecurityStringValue).
- Otherwise it reads `SkillDataTable.GetParametor(skillId)`. If the row `IsValid`, it overwrites
  name (+0x20), explanation (+0x28), skillType, bottunType, coolDown, limitCount, the four
  magnifications and skillParamList. If the row is missing, the server values are kept.
- A server that sends `isInit: true` makes the client use the server values as they are.

`GetSkillIntId` = `int.Parse(skillId after last '_')` (0xF992F0). Skill labels look like
`SKILL_NAME_100_01`, so skillId is `"<group>_<NN>"` (M). AbilityInfo/DtAbilityInfo have the same
`isInit` + `SetupFromAbilityData` pattern (0xD0EE00, M by analogy).
**So for display the server can send either `{skillId, skillLevel}` and rely on skill_data_table, or
full values with `isInit:true`. Battles read the tables directly (M), so the rows are needed anyway.**

### 2.5 MindInfo (BaseMindInfo + MindInfo)

```
mindId string Req  4-digit "NNNN" (mt_/mim_ assets use the part after the last '_';
                   shop code strips "MIND_", 0xBE15E4). Plain digits are safe either way.       M
name string  Req  label "MIND_NAME_<mindId>"                                                    M
nameSortIndex, rare(Req), cost(Req) int; description string (label "MIND_TEXT_<id>", M)
mindType MindType None=0, Ability=1, Skill=2, Boost=3, Kakusei=4, PvP=5
abilityInfo, skillInfo, dtAbilityInfo, dtSkillInfo, tokkouInfo(+tokkouStart/End), dtTokkouInfo,
pvpAbilityInfo, dtPvpAbilityInfo
maxLevel, sellVm int; linkParam, tipsText string; level, exp int
limitCharaList List<string>; limitTribeList List<StatueAttributeType>; limitStatueType; limitCount
limitCharacteristicList List<int>; characteristicList List<{id,level}>; levelUpTable List<MindLevelTable{level,exp}>
--- MindInfo ---
minduId int Req (unique); sellRing, sellGet, stack, materialExp int; bonusCondition, bonusRate
hp, eng, atk, def int; newFlg, useFlg, protectedFlg int; getDate string
```

### 2.6 ItemInfo

`{itemId:int, name, comment, icon:string, bg, flame:int, type:ItemType, value:int, unit:string,
sell, priority, count:int}`. ItemType: CHARGE_MONEY=1, FREE_MONEY=2, GOLD=3, STAR_TIP=4,
GASYA_POINT=5, GASYA_TICKET=6, ABILITY_TREE_MATERIAL=7, STATUE_LIMIT_LV_UP=8,
STAMINA_RECOVER_RATIO=9, STAMINA_RECOVER_ADDITION=10, EVENT_ITEM=11, EVENT_POINT=12,
ENERGY_RECOVER_RATIO=13, ENERGY_RECOVER_ADDITION=14, MIND_BOX_EXTEND=15, AP_RECOVER=16,
CLAN_PERSONAL_ITEM=17, CLAN_PUBLIC_ITEM=18.
The client builds the item label itself: `ITEM_{itemId:0000}_NAME` (`GetItemTextLabel` 0xE2D188)
and the thumbnail `it_{itemId:D4}`. Required: itemId, count, type (M). Send `name` = the label anyway.

### 2.7 UserInfo fields that matter

| Field | Use | Note |
|---|---|---|
| username, userRank, exp, nextExp | header, rank bar; rank_table for the battle result | H fields; rank ≥ 1 |
| staminaPt, staminaMax, staminaUseTime, staminaMaxTime | `GetCurrentStamina` / `TryUseStamina` (0xBC7C98/0xBC7D68) with `apOneRecoveryTime` from system config | time strings must parse with `DateTime.TryParse`; use "yyyy-MM-dd HH:mm:ss" |
| battlePoint*, raidPoint*, mentality, mentalityUseTime | PvP, raid, DT "mentality" (StageInfo.useMentality) | |
| stoneNum, stoneNumFree | paid and free gems (`GetStoneNum` 0xBC7AC8) | |
| vm (long) | gold | |
| mindGashaPoint (long), soul | gasha points | |
| statueNum, mindNum, mindMax | box counts | keep consistent with the lists |
| apUnlimitedFlg | BoolType | 0 |
| `UserInfo.IsValid` | `username != null` (0xBC7AB8) | |

`UserInfoResponse.halt` / `backgroundId`: the readers were not traced. `backgroundId` is probably the
clan-room BG (ClanInfo has the same field) (L). Send 0.

### 2.8 Where the client caches these (H)

- `HttpResponseCacheManager` (owned by RequestResponseManager): `userInfo`, `userInfoMini`,
  `teamInfoList` and `supportMemberList`. `CacheResponse` (0x2038B0C) updates `userInfo` from
  UserInfoResponse, LoginResponse, RecoverStamina, BizarreStart, BizarreEnd, BizarreResumption,
  BizarreContinue, Statue/MindStrengthening, AddStorage, TreeSquare, Sell, GetPresent, GashaPlay,
  ShopBuy, BuyComplete, EventShopBuy, Maintenance, PvP and Raid responses. It updates `teamInfoList`
  from TeamInfoResponse and `supportMemberList` from SupportUserResponse. **Every response that
  carries `userInfo` must carry the complete current one.**
- `SkinChangeManager.leaderStatueInfo`: My Page BG and UI part (section 1.1).
- `FormationDataBase`: teamList, pvpTeamList, statueList, mindList, mindSeqInfo, itemList, plus the
  `StatueSettingTable` reference. It is filled by team_info, statue_list, mind_list and item_list.
- `UserDataManager.userInfoMini` (profile).

---------------------------------------------------------------------------------------------------

## 3. Story quests

### 3.1 Flow (H for the calls; ids are server-chosen)

```
MyPage → Bizarre button
  /bizarre/get_bizarre_select {}                       BizarreCategorySelectUIController/<OnLoad> 0xD3E398
     questList[] is sorted by bannerType: 1 Story → storyCategoryQuestInfoList (+0x70),
     2 Daily → week list (+0x78), 3 Event → event list (+0x80) (0xD3E8E8..0xD3E95C)
     dungeonLock = (dungeonLockFlg == 1) (0xD3E9EC)
  Story button: if storyCategoryQuestInfoList.Count == 0 → nothing happens (0xD3FD70);
     else selectCategoryId = storyList[0].category → GoToBizarreChapterScene (0xD3FDF8)
  /bizarre/bizarre_list {category}                     BizarreChapterSelectUIController/<OnLoad> 0xD42C44
     chapterList is split by ChapterInfo.stageType (+0x70) into Battle and DT lists, with a
     mode-toggle button (chapterListModeSelect 0xD42284)
     chapter image = bundle "bizarre_chapter_map_{chapterMapType:D3}", asset
     ".../BizarreChapterMap/bizarre_chapter_map_NNN.png" (LoadMapImage 0xD4267C reads +0x48)
  /bizarre/stage_list {chapterId, stageType}           BizarreStageSelectUI(2)Controller
  /support/support_user_s + /team/team_info            BizarreSupportSelectUIController
  /bizarre/unlock {chapterId, stageId}                 only for item-gated stages
  /bizarre/bizarre_start {...}                         BizarreQuestSceneManager/<RequestStartBizarreQuest> 0xE0D0F0
  battle (client-simulated)
  /bizarre/bizarre_end {...}                           BizarreQuestManager/<OnBizarreQuestEnd> 0xE05D58 (Battle),
                                                       dtbattle.BattleSceneController/<RequestBattleEnd> 0xEC58A0 (DT)
  /bizarre/bizarre_continue (on defeat, optional), /bizarre/destruction (abandon)
```
Tutorial flags (D/09 §6): `OPEN_00` and `OPEN_01` must be on for the daily and event categories
(`SetQuestLock` 0xD3D818). Story is not gated by them.

### 3.2 get_bizarre_select

```
BizarreSelectResponse {
  questList: List<QuestInfo{ questHomeId:string, category:BizarreCategoryType, bannerType:BizarreBannerType(1 Story,2 Daily,3 Event),
              bannerFileName:string, day:int, Priority:int, eventMasterId:string, startDate:string, endDate:string }>,
  eventCampaignList: List<EventCampaignInfo{ eventCampaignId, banner, htmlLink:string, signboardPriority:int,
              bizarreHomeFlag:bool, signboardParam:string }>,
  dungeonLockFlg: BoolType }
```
- BizarreCategoryType (248 values): STORY_01..05 = 1..5, DAILY_SUN..STA = 6..12, EVENT_001.. = 13...
- `QuestInfo.BannerFileName` = `bannerFileName` with a trailing `_quest` removed (0xE196B8). The banner
  is loaded from **bundle `<BannerFileName>`**, asset
  `Assets/Resources/Common/Graphics/GUI/Banners/<BannerFileName>.png` (0xD3E5C8..0xD3E6B8, H).
  The player has only the 7 daily banners (`bizarre_type_daily_{sun,mon,tue,wed,thu,fri,sta}`). These
  names equal `GetBizarreBannerName(category)` = `category.ToString().ToLower()` (0xE2B7D8, H), so a
  daily entry should send `bannerFileName = "bizarre_type_daily_mon"` etc. (M). **There are no event banner bundles in
  the player's files**, so an event entry must reuse an existing banner bundle (M).
- Event click: `selectCategoryId = eventList[idx].category`, then `GoToBizarreChapterSceneEvent`. It
  also uses `BannerFileName + "_large"` (0xD3FA50..0xD3FA88; the bundle for it is absent, effect not traced, L).

Minimal (story only):
```json
{"questList":[{"questHomeId":"ST","category":1,"bannerType":1,"bannerFileName":"","day":0,"Priority":1,
  "eventMasterId":"","startDate":"2020-01-01 00:00:00","endDate":"2099-12-31 23:59:59"}],
 "eventCampaignList":[],"dungeonLockFlg":1}
```

### 3.3 bizarre_list (category) → chapterList

```
BizarreListResponse { chapterList: List<ChapterInfo>, eventInfo: EventInfo, raidInfo: RaidInfo }
ChapterInfo { chapterId:string, name:string(label), category:int, summary:string(label), chapterNum:int,
  startStageId, lastStageId:string, chapterMapType:int (→ bizarre_chapter_map_NNN), selectMapType:int,
  locationId:int, startTalkId:string, start, end:string(dates), stageType:BattleType,
  startConditionItemList: List<{itemId,count,possessionCount}>, startConditionFlg:bool, startConditionText:string,
  stageList: List<StageCurrentStateInfo{stageId, clearFlg, specialRewardFlg, playCount, isRaid}> }
```
Text: the chapter banner calls `TextManager.GetString` on the name fields (0xD406E4, 0xD40D3C, M for
which field holds which label). Labels: `CHAPTER_ST_NN` (chapter number, e.g. "Chapter 1") and
`CHAPTER_NAME_ST_NN` (title); for DT `CHAPTER_DT_ST_NNN` / `CHAPTER_NAME_DT_ST_NNN`.
Use `summary = CHAPTER_ST_01`-style and `name = CHAPTER_NAME_ST_01`-style (M).

### 3.4 stage_list (chapterId, stageType) → StageInfo

```
StageInfo { stageId, name(label), stageNo:int, stageDifficulty:int, type:int, text(label), startTalkId, endTalkId:string,
  startConditionItemList, startConditionList: List<{type:StartConditionType, optionType, value:string, count}>,
  startConditionFlg:bool, startConditionText, areaMax:int, useMentality:int, openSetting:BizarreOpenSetting,
  nextStageIdList:string[], starList: List<StarInfo{starNum, starTarget:QuestMissionClearType, comment, param, clearFlg}>,
  steadyItemSet, rewardItemSet, reward3StarsItemSet: List<RewardItemSet{dataType:ServerDataType, dataId, name, count}>,
  bossDropItemSet: List<DropItemSet{…, lot}>, useStamina, gainRankExp, mindGashaPoint, playCount:int,
  clearFlg:BoolType, clearCount, clearRank:int, specialRewardFlg:BoolType, questTips:TipsInfo, meisyoId,
  limitTime, endureTime:int, enemyList: List<EnemyStatueInfo>, stamina/exp/goldCoefficientList:{rate:float, add:int},
  bizarreEffectList, limitTurn, endureTurn:int, eventPointSettingList, dayNight:DayNightRistrictionsType,
  clearStarList: List<{clearType, value}>, useStatueCount, highScore:int, cannotContinue:BoolType, presetTeamId:string }
```
ServerDataType: Statue=1, Mind=2, Item=3, Stone=4, Money=5, FrendPoint=6.
Stage labels: `STAGE_ST_CC_SS` (name, ChapterName_JP) and the same key in StageText_JP (`text`);
DT: `DT_STAGE_ST_NNN_SS`.

**Stage ids.** No client code builds or parses stage ids. The only literal is the debug default
`"DT_STAGE_ST_401_01"` in `DebugButtonController..cctor` (0xCF9FE4), which suggests the original ids
**equal the stage labels** (L–M). Recommended: `stageId = "STAGE_ST_01_01"`,
`chapterId = "CHAPTER_ST_01"` (L); any consistent strings work.

### 3.5 bizarre_start

Request:
```
BizarreStartRequest { stageId:string, teamNo:int, supportId:string, staminaCoefficientList, expCoefficientList,
  goldCoefficientList:{rate,add}, campaignSkipFlg:BoolType, tarotInfo:{tarotId, tarotPosition},
  dungeonStartFlg:DungeonStartFlg(0 normal), dungeonId:string, floorNum:int, stageType:BattleType, presetTeamId:string }
```
Response:
```
BizarreStartResponse { userInfo:UserInfo (stamina already paid), areaList: List<AreaInfo>, minigameInfo:{},
  supportUserInfo: UserInfoMini, chapterInfo: ChapterInfo, teamInfo: TeamInfo, stageInfo: StageInfo,
  continueCount:int, tarotInfo:TarotInfo, dungeonId:string, floorNum:int, seed:int }
AreaInfo { areaNum:int, areaId:string, areaScene:string, areaEventId, endEventId, bizarreEffectId:string,
  mapId:string, dayNight, vampireDayNight:AreaMapTimeType(Day=0,Night=1,RoomDay=2,RoomNight=3),
  mapCamera, startCamera, bgmCueId, rotate:int, levelEditId:string, instanceObjType:int,
  startWipe:InWipeType(1..12), endWipe:OutWipeType(1..12), clearWaveNum, limitTime, endureTime:int,
  waveList: List<WaveInfo>, mapGimmickInfo: MapGimmickInfo, mapPoint:int, formation:string,
  clearAreaType: DTBattleClearAreaType(KILL_ALL=1, KILL_BOSS=2), limitTurn:int, areaSettingList: List<DTBattleAreaSettingElement> }
WaveInfo { areaId, waveNum:int, type, pivot:string, offset:int, enemyStatueInfo: EnemyStatueInfo, AIId:int,
  dropList: List<DropInfo{dataType, dataId, count}>, label, cutinEvent:int, cameraTarget:string, intervalTime, spawnNum:int,
  disappear, bossFlag: BoolType }
DTBattleAreaSettingElement { enemyStatueInfo: EnemyStatueInfo, seq:int, difficultyTable:string, difficultyLevel:int,
  AiId:string, fastStrike:bool, enemySkillTable:string, dropList: List<DropInfo>, disappear, hideEnemyInfo, bossFlag, backFlag: BoolType }
MapGimmickInfo { gimmickId, cycle, prob, predictMindId, breakOutTime, triggerMaxNum, patternId, bulletId, bulletList[], assistStatueMasterInfo, … }
EnemyStatueInfo = BaseStatueInfo (all fields of 2.2 up to levelUpTable)
```
How the client uses AreaInfo (H unless marked):
- **Battle mode** scenes (`BizarreQuest.GetUseSceneNameList` 0xE00384): `mapId+"_base"`,
  `mapId+"_collider"`, `mapId+"_hiru"` or `"_yoru"` (by `GetMapTimeType`), plus `AreaSceneName` =
  `areaScene` if non-empty, else `areaId` (0xC90914). Area scene bundles are `area_st_CC_SS_AA` and
  contain the scene `Assets/AppData/Scenes/Area/AREA_ST_CC_SS_AA.unity` with only spawn transforms:
  `PlayerStart/01..03`, `WAVE_A/01..`, `WAVE_B/…`, sometimes `PlayerResult` and `mapCameraTarget`
  (files, H). `AreaPosData.GetPosData(root, pos)` looks positions up by name (0xC912F4);
  "WAVE_" is a known root prefix (0xC90FA0). So a WaveInfo names its spawn as
  root `type`/`pivot` = "WAVE_A" and slot "01" (M for which field holds which). `AIId` indexes
  `aipack/aitype.asset` (621 ids, file H; link M).
- **DT mode** scenes (`BattleSceneBaseControllerData.GetSceneNameList` 0xEC14CC): `mapId+"_base"`,
  `mapId+"_hiru"/"_yoru"`, `mapId+"_collider_dt"`. No area scene. The pivot is taken from the
  `_collider_dt` scene (0xEC19D8). Unit positions come from
  **`area_position_pattern_table` row `patternName == AreaInfo.formation`**
  (`GetPositionPattern` 0xEC1A8C → `AreaPositionPatternTable.GetParametor` 0xC91BC4). A missing row
  gives an empty PatternData. `AiId`/`enemySkillTable` name the `aipack/ai/dtbattle/*.asset` tables
  (146 names, M).
- Map ids in the files: 113, of which 109 are complete for Battle and 102 for DT (E-content-ids.json `maps`).

### 3.6 bizarre_end

Request (DT path 0xEC58A0, Battle path 0xE05D58):
```
BizarreEndRequest { result:int (BizarreEndResultType Success=0, Fail=1; M), stageId:string,
  starClearList:string (BattleLogManager.GetClearMissionStr), dropList: List<DropResultInfo{dataType, dataId, count, areaId, waveNum, type, pivot, offset}>,
  defeatedEnemyList: List<{statueId, count}>, dungeonStartFlg:int, dungeonId:string, floorNum:int,
  dtDropList: List<{areaNum, seq}>, stageType:BattleType (from selectedChapterInfo), eventRankingSendInfo:{turn, maxDamage, totalDamage}, enableCheatCheck:bool }
```
Response:
```
BizarreEndResponse { clearRank: BizarreClearRank(Failed=0,C=1,B=2,A=3,S=4), starClearList:string, userInfo:UserInfo,
  rewardList, dropList, firstRewardList, starRewardList: ItemSetInfo{itemSetIcon, itemSetNum, statueList[StatueInfo], mindList[MindInfo], itemList[ItemInfo], clanObjectList[]},
  newRecordFlg:int, teamInfo:TeamInfo (post-battle exp/levels), gold, rankExp, levelExp, mindGashaPoint:int,
  supportUserInfo:UserInfoMini, eventResult:EventResultInfo{eventType, userEventPointAll, getUserEventPoint, eventRewardList[], getEventItemList[]},
  dungeonClearFlag, dungeonGameoverFlag:int, nowEventRankingPoint:long, oldEventRankingResultInfo, raidStageInfo, raidBossInfo }
```
The drops are identified by spawn (`areaId/waveNum/type/pivot/offset` in Battle mode, `areaNum/seq` in
DT mode). **The server owns the drop tables** (WaveInfo/DTBattleAreaSettingElement `dropList`) and
must grant the rewards to its own inventory as well as echoing them (M).
The battle result screen reads `rank_table` (`BattleResultUIController/<SetUI>` 0xC0F890), so the
asset must exist (D/09 §5, H).

### 3.7 What the battle needs from the server vs the client

| Data | Source | Conf |
|---|---|---|
| Enemy line-up, stats, skills, AI id, drops, waves | server (`areaList`) | H (no stage master in client) |
| Map, lighting and colliders | client `mapNN_*` bundles, chosen by `mapId` | H |
| Spawn points (Battle) | client `area_*` scene, chosen by areaId/areaScene | H |
| DT unit positions | master data `area_position_pattern_table` keyed by `formation` | H |
| AI behaviour | client `aipack` | M |
| Skill and ability numbers | master data (`skill_data_table`, `ability_data_table`, `dtbattle_*`) or server SkillInfo with isInit | H/M |
| Statue models, animations, action sets, effects | client statue bundles by psid | H |
| Talk scripts (startTalkId/areaEventId/…) | client script bundles `r_ev_*`, `e_ev_eq_*`; empty ids should skip them | L |
| Damage formulas, reactions, affects | master data extras (section 5) | M |

### 3.8 Minimal chapter 1 / stage 1 (Battle mode; M as a whole, untested)

- get_bizarre_select: the story entry from 3.2.
- bizarre_list `{category:1}` →
  `chapterList:[{chapterId:"CHAPTER_ST_01", name:"CHAPTER_NAME_ST_01", summary:"CHAPTER_ST_01", category:1,
  chapterNum:1, startStageId:"STAGE_ST_01_01", lastStageId:"STAGE_ST_01_02", chapterMapType:1, selectMapType:0,
  locationId:0, startTalkId:"", start:"2020-01-01 00:00:00", end:"2099-12-31 23:59:59", stageType:0,
  startConditionFlg:true, stageList:[{stageId:"STAGE_ST_01_01", clearFlg:0, playCount:0}]}]`.
  The `startConditionFlg` semantics were not traced; try true = condition met (L).
- stage_list → one StageInfo `{stageId:"STAGE_ST_01_01", name:"STAGE_ST_01_01", text:"STAGE_ST_01_01",
  stageNo:1, areaMax:1, openSetting:1, useStamina:5, gainRankExp:10, enemyList:[EnemyStatueInfo…],
  starList:[], rewardItemSet:[], clearFlg:0, startConditionFlg:true}`.
- support_user_s → `{supportUserList:[]}`. It may need one dummy entry; `useDummySupportMember`
  exists (L).
- team_info → a Battle team (stageType 0, mainFlg 1) with owned statues.
- bizarre_start → `userInfo` (stamina reduced), `chapterInfo`, `stageInfo`, `teamInfo` (the chosen
  team), `seed`, and
  `areaList:[{areaNum:1, areaId:"area_st_01_01_01", areaScene:"AREA_ST_01_01_01", mapId:"<a battleOk map, e.g. map04_town01>",
  dayNight:0, startWipe:1, endWipe:1, clearWaveNum:1, waveList:[{areaId:"area_st_01_01_01", waveNum:1, type:"WAVE_A", pivot:"01",
  enemyStatueInfo:{statueId:"…", level, hp, atk, def, modelDataList:[…], skillList:[…], useAttr:3}, AIId:2, dropList:[]}]}]`.
  The map choice and the type/pivot naming are L/M.
- bizarre_end → `{clearRank:4, starClearList:<echo>, userInfo, rewardList:{}, dropList:{}, firstRewardList:{}, starRewardList:{},
  teamInfo:<same team, exp added>, gold:0, rankExp:10, levelExp:10}`, plus record the clear for the next bizarre_list.
- Master data: rows for every skillId/abilityId used, statue_setting rows (battle uses
  `pvpAiId`/`pvpAiAutoBattleAiId` for player auto-AI, `BattleSceneUtil.CreateUnit` 0xC11AE4/0xC11B20,
  null-safe), rank_table, and the battle extras of section 5.

DT variant: stageType 1 chapters (`CHAPTER_NAME_DT_ST_401…`). Same calls, but an AreaInfo with
`mapId` (dtBattleOk), `formation` = a pattern defined in area_position_pattern_table, and
`areaSettingList[{seq:1, enemyStatueInfo, AiId:<dtbattle table>, difficultyLevel:1, dropList:[]}]`.
It needs the dtbattle_* tables. The DT team (stageType 1) already exists for My Page.

### 3.9 Story stage data from the wikis (Build 43)

`tools/content/build_quests.py --research` merges the stages_story passes
(`local/wiki-raw/research-b33/stages_story`, `research-b43/stages_story`; high and medium records
only) into `server/content/quests.json`. Each value names its page, capture and access date (the
stage's `source`, or an `also` entry with the `fields` it gives).

- **CHAPTER_ST_23** (Gamerch 第24章, live page): AP 18 and recommended level 78 on every stage.
  Build 33 held them because the page looked copied from 第23章; the atwiki EXP table
  (その他のデータ, row 旧モードストーリー 23章以降: AP 18, EXP 5000, 難易度 78) corroborates them.
  EXP and gold stay defaults (low confidence).
- **CHAPTER_ST_24..28**: AP 18 and recommended level 78 per stage from the same atwiki row (old-mode
  in-game numbering = Gamerch numbering = our ST_(N-1)). Its EXP 5000 is the top stage's, so the
  stages keep default EXP and the chapter lists `gainRankExpMax` in `wiki`.
- **Star missions** on 116 action story stages (docs/research/11 §4). Boss-stage conflict: the
  Bahamut thread snA=969 (2019) says the old-mode final stage wants a regular (blue) skill, JoJo
  Wiki and Gamerch 第24章 say an SP skill. Each stage keeps its own source (ST_23_05 and ST_24_05
  JoJo Wiki, SP; ST_25_05..ST_28_05 Bahamut, regular) and the conflict is noted on all six.
- **Tactical rewards** (atwiki ストーリークエスト): atwiki 第N章 = CHAPTER_DT_ST_4NN by number,
  used only where the stage counts agree (38 chapters, 170 stages): first clear Diamonds (no
  quantity) and one special reward per stage (`nameJp` as written; `itemId` only for an exact
  ITEM_<id>_NAME label match, 67 of 170). Not served yet. Held in the chapter's `wiki`: the
  prologue and chapters 1-4 (the 2018-09-28 maintenance cut their stages, Bahamut snA=726; our
  files list the stages from before the cut) and 7, 21, 33, 43 (counts swapped 3/5, unexplained).
  Atwiki chapters 47-48 have no id in the client.
- **Not applied**: one DT_ST_438 stage's AP 15 / EXP 4185 / 難易度 30 (which stage is not
  stated; recorded in `wiki`); drops are fixed when the area loads (atwiki FAQ; our tactical
  drops are empty).
- Enemies are drawn from team statues only (assist-only statues were excluded; this reproduces
  the Build 10 enemy picks). Output dates come from the inputs, so a rerun is byte for byte.

Build 44 (`--research` also takes the four research-b44 family files; only topic `stages_story`
is read; all read 2026-11-22, cited per row):
- **Action chapters' last-stage reward**: the official beginner guide (jdr.channel.or.jp tab /3,
  Wayback capture 2018-02-12, high) gives a special reward of 1 Diamond Coin on every chapter's last
  stage except chapter 1 (in-game 第1章 = CHAPTER_ST_00). 24 last stages get it in `rewards.special`
  (`itemId` null: two item labels read ダイヤモンドコイン). The four chapters whose stage count Gamerch
  contradicts (CHAPTER_ST_01, 05, 07, 16) get none and note why in `wiki`: their last stage in our
  files is not the original's. Data only, as the tactical rewards.
- **CHAPTER_ST_01's third stage**: a 2019 in-game stage screen (Bahamut snA=1046, high) shows
  第2章's 第3話 (boss; AP 5, EXP 80, the boss missions, special reward ダイヤ ×1), which the game's
  files lack (2 stages). Recorded in the chapter's `wiki`, not mapped; the chapter keeps defaults.
- **Tactical first-clear Diamonds**: 1 per stage (Bahamut snA=940: about 150 Diamonds for the 148
  stages of chapters 7-40; derived, medium) fills the quantity of the 170 tactical stages' ダイヤ.
- **Chapter 28's release** (1.8.0, 2018-01-29, official notice): in CHAPTER_ST_27's `wiki`, with the
  N-1 mapping noted (the client's own label reads 第２７章).
- **Not in the JSON**: AP came back every 5 minutes and every 3 from 3.3.0 (2018-12; Reddit, medium):
  our `apOneRecoveryTime` is already 180 s. Quests cost no AP up to rank 30 (1.8.0 notice; the 2019
  screen still shows 'Rank 30までAP無限'): AP is never charged offline. The tactical chapters'
  release dates (19-20 in 3.3.0, 33-34 in 3.4.0, 45-46 in 3.9.0) need the held 4NN mapping and are
  not attached to chapters.

---------------------------------------------------------------------------------------------------

## 4. Events

Two separate trees (H):

1. `/info/event_list` (My Page): 
   ```
   EventListResponse { eventList: List<EventListResponse/EventCampaignInfo{ eventCampaignId:string, eventType:EventCampaignType,
                         eventEndDate:string, eventFinishedDate:string }>,
                       userBattleInfo:{fieverEndTime:string}, raidInfo:{raidBossInfo, raidStageInfo, totalRescueCount:int} }
   EventCampaignType: GrowthTree=1, Stage=2, Strength=3, EventBizarre=4, Gasha=5, Pvp=6, PvpAuto=7, Raid=8
   ```
   This is a nested class, distinct from `Jjs.EventCampaignInfo`. It drives My Page event badges
   and timers (M).
2. Quest-side events: get_bizarre_select `questList` entries with `bannerType 3`,
   `category = BIZARRE_TYPE_EVENT_nnn (13+)`, `eventMasterId`, start/end dates and a `bannerFileName`
   whose bundle exists; plus `eventCampaignList` (`Jjs.EventCampaignInfo{eventCampaignId, banner,
   htmlLink, signboardPriority, bizarreHomeFlag, signboardParam}`) for the campaign slide panel and
   signboards (the campaign banner is also a bundle). `bizarre_list {category}` then returns
   `eventInfo: EventInfo{eventCampaignId, eventShopFlg, eventCollectType, eventRankingFlg,
   eventMissiongFlg, eventBg, htmlLink, eventBgm, eventTutorialImage, eventItemList[ItemInfo],
   eventPointImage, userEventPoint, clanEventPoint, endDate, finishedDate, banner}` and the event
   chapters (EventHomeUIController also calls item_list and stage_list).

What an event needs to show (M): OPEN_01 set and OPEN_03 set (tutorial table, D/09 §6); a questList
event entry with valid dates; a banner bundle that exists, which means reusing one of the 7 daily
banner bundles because the player's files hold no event banners; event chapters/stages
(labels `CHAPTER_NAME_EV_*`, `STAGE_EV_*`; area scenes `area_ev_*`, 1,403 groups). Event shop,
ranking and mission flags at 0 to hide those features. Campaign text labels are `GS_nnnnnn_Name/Info`
(515) and `EQ_nnnnnn_Name/Info` (255). `EQ_5000xx` lines up with the `e_ev_eq_5000xx_*` script
bundles (M).

---------------------------------------------------------------------------------------------------

## 5. Master-data rows these screens need

Schemas: D-masterdata.md §3 / docs/research/09. "Derivable" means it can be filled from the client's
own files; "wiki" means real numbers needed; "default" means a neutral value is fine.

| Table | Needed for | Fields that matter | Derivable | Wiki | Default |
|---|---|---|---|---|---|
| statue_setting_table | My Page BG/model (a), formation (`teamPos/teamRotation/teamScaleScalar`, via FormationDataBase.GetStatueSettingParameter 0xC2939C), gasha, battle result cut (0xC0D0F4), player auto-AI (`pvpAiId`/`pvpAiAutoBattleAiId`) | statueId (every owned statueId), myPageId (one of 21), positions | statueId list, myPageId set, mindFrameID (mind_frame_00 prefabs), battleResultComment = `Result_Chara_Comment_<id>` label (M) | myPageId per character (which BG suits whom) is not known (L) | positions 0, `teamScaleScalar 1.0` (M), cameraType "", AI ids from aitype (e.g. 1 "player auto", L) |
| statue_level_table | level-up/exp UI unless the server sends `levelUpTable` inline (0xBFF8A4) | per statueId: level, exp, hp, atk, def | ids | curves and stats | linear curve |
| skill_data_table | skill display and battle (2.4) | skillId, skillName (label `SKILL_NAME_<id>`), skillProfileText (`SKILL_TEXT_<id>`), skillType, bottunType, coolDown, limitCount, rates, params (SecurityIntValue: seed 0, value) | ids and labels (1,107 SKILL_NAME ids) | numbers | – |
| ability_data_table (+ dtbattle_ability_data_table) | abilities, leader skills, mind abilities | abilityId, abilityName (labels ABI_STAT/LEAD/MIND_NAME_*), affectId + params | ids and labels | affect ids and params | empty param list (no effect) |
| mind_level_table | mind level UI | per `rare`: level, exp | – | curve | linear |
| rank_table | battle result (must exist) | rank, exp | – | curve | linear |
| team_rank_table | formation rank badge | 11 thresholds | – | – | any increasing |
| tutorial_flag_table | quest categories | D/09 §6 table (34 rows) | – | – | – |
| area_position_pattern_table | DT battle positions | patternName = AreaInfo.formation, posSetList Player1..3/Enemy1..3 | – | – | invented positions (M) |
| assist_ability_condition_table, statue_special_scene_data_table, action_branch_data_table, affect_text_rule_table, map_gimmick_pattern_table | detail texts, special retire scenes, reactions, gimmicks | | | real rows | empty is null-safe (D/09 §5) |
| Battle extras: affect_table, state_abnormality_table, characteristic_data, characterestic_skill_parameter, bullet_parameter, reaction_table_{normal,mini,mini_no_blow}, model_table, skill_create_character_table, battle_damage_param, battle_combo_param, oraoramode_setting, statue_affinity_table, additional_statue_load_table | Battle-mode fights | see D schema JSON | – | all numbers | null-guarded at boot (D/09, M); whether a fight runs with them empty is **untested** |
| DT extras: dtbattle_skill_data_table (SecurityStringValue names), dtbattle_ability_data_table, dtbattle_damage_param, dtbattle_damage_rank_table, dtbattle_reaction_table_*, dtbattle_statue_affinity_table | DT fights | | – | all numbers | untested |
| Set_/Node_el_camera_* | gasha and special-skill script cameras | | – | – | empty is safe (D/09) |

Server-side (not master data) but needed: statue base stats, rarity, type and tribe per statueId;
mind stats, rarity and cost; stage definitions; enemy stats; rewards; stamina costs. All from wikis
or invented.

---------------------------------------------------------------------------------------------------

## 6. Id inventory (counts; details in E-content-ids.json)

Sources: 13,479 decoded bundle names (`names.tsv`), the thumbnail bundle containers, the `statue_*`
fbx names, `aipack`, and the work2.tsv labels.

| Set | Count | Notes |
|---|---|---|
| myPage BG ids | 21 | 10–13, 20–23, 30–35, 40–45, 50; bundle `ui_mypage_bg_00` |
| statueIds (label ∪ thumbnail) | 1,266 | 1,246 have `STATUE_NAME_` labels; 643 have `st_` thumbnails; 20 thumbnail-only |
| personalSituationIds | 185 | 141 `statue_<psid>` model bundles (140 with a human model, 68 with a stand model); 50 psids lack model or common animation |
| statueIds with model + common anim | 1,084 | minimum for My Page |
| "fully playable" statueIds | 584 (113 psids) | label + thumbnail + statue_ + animation common/battle + actionset + dt_actionset |
| statue suffixes | `_01.._04`, `_11.._28`, `_61.._97`, `_98`, `_99` | meaning (rarity or version) not derivable; `rarity: null` |
| mindIds | 1,384 | 1,373 `MIND_NAME_` labels; 1,021 `mt_` thumbnails; 1,021 `mim_` image bundles |
| itemIds | 573 | 544 `ITEM_NNNN_NAME` labels; 447 `it_` thumbnails |
| skill ids (from SKILL_NAME labels) | 1,107 | format `<group>_<NN>` |
| story chapters (Battle) | 31 (`CHAPTER_NAME_ST_00..30`) | 137 stage labels (`STAGE_ST_CC_SS`); 130 of them have `area_st_CC_SS_AA` scenes; 15 area-stage groups have no label; chapter maps `bizarre_chapter_map_000..028` |
| story chapters (DT) | 81 (`CHAPTER_NAME_DT_ST_00`, `401..480`) | 240 `DT_STAGE_ST_NNN_SS` labels; no area scenes; `bizarre_chapter_map_1401..1480` match 401..480 (M) |
| daily | 42 chapter labels (`CHAPTER_NAME_DY[N]_<DAY>_*`) | 108 area groups (`area_dy_*`, `area_dy2_*`); 7 daily banner bundles |
| event | 1,807 chapter-name labels, 6,220 stage labels | 1,403 `area_ev_*` groups; campaign labels GS 515, EQ 255 |
| chapter map images | 136 | 000–028, 500–506, 531, 532, 700, 701, 750, 900–913, 1000, 1401–1480 |
| maps | 113 map ids | 109 complete for Battle, 102 for DT |
| AI | 621 `aitype` aiIds; 146 DT AI tables | from `aipack` |

---------------------------------------------------------------------------------------------------

## 7. Open questions

1. `CommonUIManager.LoadResource` pool-to-bundle mapping for `MyPageBG` (M). Confirm on device that
   `bg_0010` appears.
2. `myPageCameraType`: no reader found.
3. Which ChapterInfo field holds `CHAPTER_ST_NN` and which holds `CHAPTER_NAME_ST_NN`; the semantics of
   `startConditionFlg`.
4. WaveInfo `type`/`pivot`/`offset` naming against the area scene's `WAVE_X/NN` transforms.
5. Whether Battle or DT fights run with the extras tables empty; the order of required rows.
6. The original team count per stageType and the number of mind slots.
7. Statue suffix meaning and rarity (wiki).
8. The event banner bundles needed (`<bannerFileName>`, `<bannerFileName>_large`) are absent from the
   player's files.
