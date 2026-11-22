# 11 — Statue positions and what a tactical battle needs (Build 11)

Our own analysis of the 3.12.0 client (il2cpp metadata and arm64 disassembly). Addresses are RVAs in
`libil2cpp.so`. Confidence: H = read in the code, M = inferred, L = a guess or a value we chose.
No game text is reproduced here.

## 1. `statue_setting_table` positions

What the Build 10 device log showed: My Page drew the right background, but the leader model was
off frame. The Team Setup statues were invisible, and `StatueAndPedestalObjectManager.SetStatueState`
logged CharacterController "Step Offset" warnings.

- The client multiplies every position field and `teamScaleScalar` by 0.01, the constant at
  0x2089A84. Positions are therefore centimetres and the scale is a percentage (H).
- **Team view** (`GetStatueStandardTransform` @0xCBC6D4, view type 1): scale = anchor scale ×
  teamScaleScalar × 0.01 (@0xCBCA68, 0xCBCB98, `SetTransform` @0xCBCBE0) (H).
  - A value of 1.0 made the statue and its pedestal 1% size. The CharacterController then rejected
    its step offset (M). **100** is full size.
  - The three slot anchors (centre, left, right) already face the camera with scale 1, so
    `teamPos`/`teamRotation` (0,0,0) suits every statue (H).
  - The detail views use a fixed scale of 1 (@0xCBC840), which is why they looked right.
- **My Page** (@0xC38B70): the model is placed at the show point + `myPagePos` × 0.01, turned by
  Euler(`myPageRotation`). The show point and the default camera both sit at the origin, and the
  camera looks along +Z with a 40° field of view, so zero offsets put the camera inside the model (H).
- `myPageCameraType` names a scene object. The client calls `GameObject.Find(name)` and moves the
  camera to that object (@0xC38E90–0xC39154); there is no camera table (H).
  - The My Page scene has `CameraIdSet0` (the origin), `CameraIdSet1` and `CameraIdSet2` (lowered).
- Values we send for every statue (M, worth tuning on the device):

  | Field | Value |
  |---|---|
  | `myPageCameraType` | `CameraIdSet0` |
  | `myPagePos` | per statue (35, 25 − head height, 180) (Build 33, docs/research/31 §5; was (72, -113, 322)) |
  | `myPageRotation` | (0, 192.5, 0) |
  | `teamScaleScalar`, `teamPos` | per statue (docs/research/12 §7) |

  Since Build 33 the My Page leader stands 1.8 m from the camera, framed thighs-up as in the
  original's footage, with its head at the same place whatever its height.
- Other fields:

  | Field | Effect | Value we send |
  |---|---|---|
  | `mindFrameID` | the client uses it + 1 (@0xEA4C24): 0 picks the info-style frame, 1–4 the equip frames (H) | 1 |
  | `pvpAiId`, `pvpAiAutoBattleAiId` | any row overrides the built-in default AI id of 1 (@0xDB42A4), even when the row says 0 (H) | 1 each |
  | `dododoGaugeType` | 0–3 select an existing gauge asset (@0xBF9210) | 0 |
  | `detailViewPos`, `jojoViewPos`, `gashaTopViewPos` | offsets from their anchors | (0,0,0) |

## 2. Tactical battle: BattleInit to the first turn

What the Build 10 device log showed: after `bizarre_start`, `ReversalManager.Init` (0xD8A0E0)
threw a NullReferenceException from `BattleManager.BattleInit`. After that, `BattleManager.Update`
threw on every frame.

The reads that are not null-safe, in the order they fail:

1. **`dtbattle_damage_param`** (`Jjs.dtbattle.DtBattleDamageParam`, `param: List<Param>`).
   - `GetParametor` 0xCD0338 loads the asset on every call. It returns null when the asset is
     missing or its list is empty.
   - `ReversalManager.Init` uses that result without a check (0xD8A120) (H).
   - `CharacterStatus.ctor` 0xEFBB28 caches it; the damage code deals 0 damage when it is null.
   - Param has 68 fields of SecurityInt/FloatValue `{seed, securityValue}`. The client reads
     `seed ^ securityValue`; for floats it reads the bits of that XOR (0xA99694, 0xA9967C) (H).
   - From `CalcSkillHpDamage` 0xEFDDD0..0xEFEB90, some values must not be zero (H):
     - `limitRateMin..Max` must bracket 1.0;
     - `multiDealDamageMin..Max` must bracket 1;
     - `damageCap` must be non-zero.
   - We send one row of chosen values, with the reversal mode off (L).
2. **`dtbattle_skill_data_table`** (`Jjs.dtbattle.dtSkillDataTable`).
   - The client attaches `SKILL_000_02` (Cancel) and `SKILL_000_01` (UnitChange) to every unit
     (static list, cctor 0xD8F650; loop 0xD8E7C4) (H).
   - It looks up every attached skill with `GetSkillData` without a null check (0xD8D664) (H).
   - Each skill's `mainSkillCategory` must name a category row. Otherwise `CanUseSkill` 0xF011F8
     dereferences null on the first turn (M-H).
   - A unit needs a combo-type (1) skill in `dtSkillList` with `skillLevel` ≥ 1, or it has no Attack
     button (button jump table 0x223E35C; `SetupFromSkillData` 0xF98D78 keeps the level) (H).
   - We send the client's two fixed skills, one basic attack `JDR_ATK_00` given to every statue,
     and three categories.
   - String fields are SecurityStringValue `{seed, securityString}`, decoded at 0x105FAA4 (H):
     split on '-', parse each part with `Convert.ToInt32(part, 16)`, XOR byte i with byte i%4 of
     `BitConverter.GetBytes(seed)`, then ASCII-decode. With seed 0 we write the ASCII bytes as
     uppercase hex joined by '-'.
3. **`dtbattle_damage_rank_table`** (`List<int> table`). If it is missing, damage is skipped. If it
   is present but empty, the client reads index Count-1 = -1 and throws (0xEFE36C..0xEFE418) (H).
   We send rates in 1/10000 (M for the unit, L for the values).
4. **`AreaInfo.formation`** names an `area_position_pattern_table` row (0xEC1A8C → 0xC91BC4).
   - With no row, units stand on the pivot with a zero quaternion; this does not crash (H).
   - A set that exists but has too few positions throws (0xC91EF0) (H).
   - Sets are picked by list position: players with 1–3 front units use sets 0–2, enemies 3–5.
   - The player's files contain no pattern names, so we name our own row `JDR_DT_DEFAULT`.

These are null-safe or already satisfied (H unless marked):
- `AiId` "" falls back to `playerauto` (0xBBBEAC, constant set at 0xDB4A0C). Any name in the
  player's aipack works, and an unknown name gives no AI rather than a crash.
- `modelDataList` must be a list (0xD93530) and `stEffRateInfo` non-null (0xD8FAC0); our values
  already satisfy both.
- Leave `dtbattle_statue_affinity_table` out. Its readers treat a missing table as neutral, but a
  present one is indexed 6×6 without bounds checks.
- `levelUpTable` is not read by the battle.
- An enemy `bossFlag` of 1 may start a boss intro, so we send 0 for now (L).

Not traced yet: the action and timeline assets a skill needs when it plays (`viewSkillId` is empty
for our basic attack), and what the AI behaviour data needs beyond the lookup.

## 3. Battle mechanics: damage, turns, animations, stage window (Build 12)

What the Build 11 device log showed: the tutorial battle loaded and units were placed. But no
animation played, every hit did 1 damage, turns never ended, and the stage window had no
description or rewards. No errors were logged.

- **Damage** (`CalcSkillHpDamage` 0xEFDCE4) (H):
  - base = baseDamageCorr × rankRate × ATK / 1e8;
  - then × max(ofRatioCurve × ln(ATK/DEF) + 1, minimumGuaranteedMag) (`logf` at 0xEFE610);
  - then × the random spread × 0.0001 (constant 0x2089AAC, 0xEFE69C–0xEFE7CC).

  So `damageGitterMin/Max` are in 1/10000. We had sent 95/105, which gives about 1% damage and
  rounds to 1; Build 12 sends 9500/10500. `healFluctuationsMin/Max` use the same unit (0xEFF05C).
  The DAMAGE category's `paramList[0]` is the hit attribute name, not a power (`CheckHpConfigAttr`
  0xD8CD5C), so we send `PHYSICAL` (L). A skill's power comes only from `damageRank` through the
  rank table.
- **Turns** (H for the mechanism):
  - Action gauges start at 50–52.
  - Time to act = (100 − gauge) × 10000 / speed (0xBA31F0).
  - After acting, the unit's gauge drops by the skill's `skillWait` (0xBA338C).
  - With `skillWait` 0 the first unit keeps acting forever. Build 12 sends 100 for attacks and 50
    for the fixed skills (L).
  - Clear: `clearAreaType` 1 ends the area when every enemy is dead (0xEBD730).
  - `limitTurn`/`endureTurn` 0 means no turn limit.
  - Story enemies now use the attack-only AI `zako01_cmn` from the player's aipack. An empty id
    falls back to the player's auto-play AI.
- **Animations** (H):
  - The action set comes from `viewSkillId`: the client parses the number after the last `_` as
    the skill number (0xF992F0); an empty id gives −1.
  - It then names the action set (0xFB3C28): 0 or 90–99 → `ATTACK`, otherwise `SKILL_%02d`.
  - Action sets are in `dt_actionset_statue_<psid>` as `<model>_<NNN>_<ACTION>`.
  - Build 12 gives every statue `SKILL_<first 3 digits>_99`, which maps to ATTACK. It uses the
    lowest `SKILL_NN` when the model has no ATTACK action (3 statues). `tools/content/scan_statues.py`
    records each model's actions, `build_statues.py` writes `dtAttack`, and the server adds one row
    per id to `dtbattle_skill_data_table`, with viewSkillId equal to the id.
- **Stage window** (0xE0EDD4) (H for the fields):
  - title = label `text`;
  - name = label `stageId`;
  - reward list = `steadyItemSet`;
  - first-clear reward = `rewardItemSet[0]`;
  - special reward = `reward3StarsItemSet[0]`;
  - conditions = `starList[].comment`;
  - enemies = `enemyList`.

  RewardItemSet is `{dataType (1 statue, 2 mind, 3 item, 4 stone, 5 money, 6 friend points),
  dataId, name, count}`. Build 12 lists the stage's gold as the steady reward. Chapter
  descriptions use the game's own `ChapterText_DT_ST_<nnn>` labels (000 and 401–478). Tactical
  stages have no description label of their own in the game's text (M).

## 4. Stage missions and loading tips (Build 19)

What the Build 19 device test showed: the stage window and the tactical results screen had three
empty star rows, and the loading screen's tip board was blank.

- **One source (H).** The BizarreStart handler (0xE0D690) keeps `response.stageInfo` as
  `BizarreQuestSceneManager.selectedStageInfo`. The battle, the loading screen, the end request
  and the result screen all read that copy. So `quests::stage_info()` feeds every symptom.
- **Stage window rows** (loop 0xE0F378) (H).
  - It shows min(3, `starList.Count`) rows in list order; `starNum` is not read.
  - Each row sets the `{starParam}` tag to `param` (tag provider → `GetStarParamText` 0xE0C914),
    shows `TextManager.GetString(comment)`, and lights its star when `clearFlg` is 1.
  - `comment` is a label: `DTB_QuestMission_JP:DT_QM_Text_<nnn>`.
- **Missions at the end of a battle** (H).
  - `RequestBattleEnd` 0xEC5A3C calls `GetClearMissionStr(starList)` 0xBB6B88, which turns each
    row into {`starTarget`, `param`}. `GetClearMissionList` 0xBB5F18 then checks each one against
    DTBattleQuestMissionClearType 1..16, for example 1 quest won, 2 everyone alive, 10 turns ≤ `param`.
  - The client sends the cleared types as `starClearList` ("1,2,10"). For tactical stages,
    `starTarget` must be the DT type, not QuestMissionClearType.
- **Result screen** (`BattleResultUIController.SetUI`, loop 0xC0EB90) (H).
  - The rows come from the same `starList`.
  - A star lights when `BizarreEndResponse.starClearList`, split on commas, contains the row's
    `starTarget`. We echo the request's list.
  - `clearStarList` has no reader we found (M).
- **What we send.**
  - Three missions per tactical stage: clear the quest (1), all members survive (2), and clear
    within 10 + 5 × (areas − 1) turns (10; the limit is ours).
  - `clearFlg` comes from the saved set of cleared types per stage (`Progress.stars`, the union
    of `starClearList` over wins).
  - Action stages (Build 42): see "Action stages" below.
- **Action stages** (Build 42) (H unless marked).
  - `StarInfo.starTarget` is typed `QuestMissionClearType` (0 None .. 47 NoContinue), the
    action enum; tactical stages put DT types in the same field.
  - The action end request (`BizarreQuestManager.OnBizarreQuestEnd` 0xE05D58) sends
    `starClearList` from the stage's `starList` through `BattleLogManager.GetClearMissionListStr`
    0xC069DC → 0xC0681C (comma-joined) → `GetClearMissionList` 0xC05DFC.
  - `GetClearMissionList` switches on types 1..46 (jump table 0x223CB90); 0 and 47 never clear.
    Thresholds are built into the type, so `param` is not read: 7 `CheckNobodyDied` 0xC06390
    (team size = log `ClearMemberNum`), 8 `CheckKillAllEnemy` 0xC06424 (log `KillEnemy` =
    log `EnemyNum`), 20 log `UseSkill` (11) ≥ 1, 21 ≥ 2, 9–13 clear time, 15–17 combo. All are
    the client's own counts; the server only stores what it reports.
  - The result screen (`BattleResultUIController`, shared; `isClear(List<QuestMissionClearType>,
    …)`) lights rows from the echoed `starClearList` as for tactical stages.
  - Labels: `QuestMission_JP:QuestMissionText_<type + 2>` (texts 0–2 are counters; 3 is type 1
    UseSupport, 16 type 14 NoCondition, 49 type 47 NoContinue) (M for the offset rule: every row
    matches its enum name).
  - What we send (Build 43): a story stage a wiki lists sends the wiki's three (`missions` in
    quests.json, 116 of 130 stages; docs/research/10 §3.9), e.g. 14 NoCondition (クエストをクリアする,
    text 16), 7, 22 UseSPSkill (text 24), 26/27 boss by skill / SP skill (texts 28/29), 4 and 25
    DoDoDo. Other action stages: 7 all team members survive, 8 defeat all enemies, 20 use a Skill
    (our translations of the labels; the choice: design borrowed from the Diamond Records PC
    playtest). `param` empty. Wins keep types 1..46 in `Progress.stars`.
  - Open (M): how the client checks 14 NoCondition (expected: met by any win) and 26/27 (the
    boss kill's last hit) is not traced yet.
  - Open (M): not yet seen on a device (whether every served wave raises `EnemyNum` as it
    spawns, so 8 can be met).
- **Loading tips** (H).
  - `BattleSceneController.CreateSceneChangeOption` 0xEC36F0 passes the stage's `questTips` to
    the loading screen; there is no random pick.
  - `LoadUIController.SetTipsInfo` 0xB74170 shows `GetString(tipsTitleText)` and
    `GetString(tipsText)`.
  - The picture is bundle `tipsPicture`, file
    `Assets/Resources/Common/Graphics/GUI/Tips/<tipsPicture>.png` (`GetTipsPath` 0xB7440C).
  - With `{}` the strings are null, so the board is blank. A missing `questTips` shows the
    prefab's defaults. `tipsType`/`tipsId` are not read.
  - We send `TipsTitle_<nnn>`, `TipsText_<nnn>` and `ui_tips_<nnn>`, picked by a hash of the stage
    id from the 66 numbers that have a real title, text and picture (M for the numbering: every
    `ui_tips_*` bundle matches such a title).

## 5. Reversal (Build 20)

Reversal is a core tactical mechanic (research pack `tactical.reversal`, documented), and two of
the game's missions use it. Since Build 11 we had sent `_reversalEnable 0`. All of the following
is H unless marked, read in `Jjs.dtbattle.ReversalManager` and its callers.

- **Gauge.** Internally 0..10000 (`.cctor` 0xD8B8B0); the UI shows gauge/100 as a percent. Only
  the player team has a gauge (`CheckReversalTeam`). It fills from three sources
  (`AddGaugePower` 0xD8AA20, capped at 10000):
  - a player unit picks a skill: the skill's `dododoRate`, as a percent (0xFAC6C0);
  - a player unit takes a hit of at least 1 HP: `_reversalAddGaugeDamage`, raw 1/10000 (0xFB01B0);
  - a player unit dies: `_reversalAddGaugeDead`, raw (`OnDied` 0xFABB4C).

  A continue fills it completely. Gauge gained during Reversal is lost: the gauge is reset when
  Reversal ends.
- **Fields.**
  - `_reversalEnable` is on only at exactly 1.
  - `_reversalStartGauge` is a percent with no clamp.
  - `_reversalAddSkillStock` gives skills with no uses left that many more when Reversal starts.
  - `_reversalMaxTurn` is the number of actions the team gets; enemy actions do not count.
  - `_reversalAddSpeedUpRate` **multiplies** speed by rate/10000 (`AddSpeed` 0xD8A4E4). Units
    under speed 1 never act (0xBA38D4), so a rate of 0 freezes the team, and the Reversal would
    never end.
  - `_reversalAddSkillCoolTimeRate` multiplies cool turns by rate/10000 (0 = none).
- **Activation and end.**
  - The button works with a full gauge while the battle waits for input; auto play uses it by
    itself.
  - Reversal ends when the actions run out. When the area changes first, the unused share returns
    as gauge.
  - Overkill: during Reversal in the last area, killing a boss turns the remaining actions into
    overkill hits (0xBBC3F0, 0xEE4504).
  - No other table or stage, area or statue field is read; the effects and BGM are the client's.
- **What we send** (the meaning of each field is H; the last three values are documented by the
  tactical-era atwiki and the Bahamut forum since Build 33, the gauge values are ours):

  | Field | Value | Effect |
  |---|---|---|
  | `_reversalEnable` | 1 | Reversal on |
  | `_reversalStartGauge` | 0 | The gauge starts empty |
  | `_reversalAddGaugeDamage` | 1000 | +10% per hit taken |
  | `_reversalAddGaugeDead` | 2500 | +25% per statue lost |
  | `_reversalAddSkillStock` | 1 | +1 use for spent limited skills |
  | `_reversalMaxTurn` | 4 | 4 turns, so up to 3 overkill hits |
  | `_reversalAddSpeedUpRate` | 30000 | Speed +200% (×3) during Reversal |
  | `_reversalAddSkillCoolTimeRate` | 5000 | Cool turns halved during Reversal |

  Skills now carry `dododoRate`: attack 10, normal skill 15, special 20. With three statues the
  gauge fills in about two rounds.
- **Chain (Build 33).** `ChainManager.GetChainRate` 0xFAABF0 multiplies an action by
  1 + `_chainNRate` / 100, N being the number of consecutive player actions before it (0: ×1,
  1..4: `_chain1Rate`..`_chain4Rate`, 5 and more: `_chain5Rate`). The tactical-era atwiki gives
  ×1.1, ×1.35, ×1.5 and ×2.0, so we send 10, 35, 50, 100, 100 (we sent 1.0..1.4, i.e. ×1.01).
- **Type chart (Build 33).** ×1.5 for an advantaged hit, ×0.7 for a disadvantaged one (atwiki).

  The tests reject a speed rate under 10000, fewer than 2
  actions, a start gauge outside 0..100, and attacks without gauge.
