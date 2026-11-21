# 26 — JoJo links, Mind links, skill Minds, skill levels and time stop (Build 25)

Read in the 3.12.0 client (libil2cpp.so arm64; addresses are RVAs). H = read in the code,
M = inferred. No game text is reproduced; labels are cited by key.

## 1. JoJo links (action mode only)

- **Data (H).** `TeamInfo.jojoLinkList` (and `PvpTeamInfo.jojoLinkList`) is a list of
  `JojoLinkInfo {charaLinkId, type, dataList[{dataId}], name, matchType, abilityId,
  abilityInfo}`. `abilityInfo` is an action-battle `AbilityInfo`. `IsValid` 0xB70F44 is just
  `abilityInfo.IsValid()`. The client has no link table: the server computed the links.
- **The tactical battle ignores them (H).**
  - `CharacterManager.CreateCharacter` and `Character.Initialize` 0xFAB5BC pass the list down.
  - `dtbattle.CharacterStatus.SetStatueInfo` 0xEFC570 receives `mindLinkInfoList` (x5) and
    `jojoLinkInfoList` (x6) and never reads either register.
  - It applies only: the statue's `dtAbilityList` (source 2), the leader ability (6), per Mind
    `dtAbilityInfo` (3), `dtTokkouInfo` in seq 1–4 (9) and `dtPvpAbilityInfo` in PvP (10), the
    support leader (7), tarot (12) and sally abilities (15).
  - The tactical formation calculator ignores them too. `UIStatusCalclator.ApplyDtBizzare`
    0xDB1B34 never reads `mindLinkList` (+0x68) or `oldJojoLinkList` (+0x70). Only
    `ApplyOldBattle` 0xDB216C reads them, and `SetOldJojoLinkList` 0xDB2DB4 has no caller.
- **The tactical formation hides them (H).** `FormationUIController.DtUpdateAbilityInfoWindowUI`
  0xF26E70 switches every `JojoLinkUIGroup` off and sets the link button's animator to "Off".
  The action-mode `UpdateAbilityInfoWindowUI` 0xF274A4 fills the groups from
  `teamInfo.jojoLinkList` (+0x28): the `abilityInfo` name label and its effect lines.
- **Battle use (H):** only the action battle, through `Jjs.CharacterStatus.ApplyJojoLinkAbilityAffect`
  0xF17A30 (source 4, JojoLink).
- **What survives.** The 78 labels `ABI_JOLINK_NAME_<001..078>` hold the link names. An archived
  Gamerch table lists 42 links (one row a duplicate); 39 of its names match a label exactly
  (compared locally after NFKC).
  - Each link names 2–6 characters.
  - The effects are action-era: SP skill uses, DoDoDo damage and charge, crit rate, cool time,
    move speed, gold and EXP; only a few are plain HP/ATK/DEF.
- **Traits (Build 41).** Trait links need the statues' traits: `StatueInfo.tribeTypeList` is now
  sent from `server/content/statue_traits.json` (docs/research/30-action-battle.md §7), so the
  formation and detail screens know them.
- **What we send (Build 41).** `server/content/jojo_links.json` (`tools/content/build_jojo_links.py`)
  holds 53 links; `collection.rs jojo_link_list` computes the active ones for every team (so
  `TeamInfo` and the arena's `PvpTeamInfo`):
  - **Rule format.** A link has member `groups` (each a set of families, statueId[..4]) and
    `rules` `{groups, min}`: active when at least `min` of the named groups each have a seat
    statue in them; rules are OR'd. A trait link is `{tribe, min}`: at least `min` seat statues
    carry the tribe. Each link counts once. Only the three seat statues count; assists do not
    (M: the client checks no members, so this is our reading of the wiki's "in the team").
  - **Entry.** `{charaLinkId, type 1|2, dataList [{dataId: family}], name "", matchType 0,
    abilityId JOLINK_<nnn>, abilityInfo {abilityName ABI_JOLINK_NAME_<nnn>, inline params,
    isInit true}}`. type, dataList, name and matchType are never read (ours).
  - **Members.** Wiki names map to families through the `STATUE_NAME_<sid>` JP labels and
    statues.json part, plus three variant families (2011 Caesar, 2111 Stroheim, 3041
    Polnareff); 4192 (Kira as Kosaku) is not mapped. '?' members are dropped.
  - **Sources.** The research pass's Gamerch link page and comments and the Bahamut thread, per
    link; links 12 and 34 from the saved Gamerch table. Effects use the action parser
    (doc 30 §7); a part after "/" without a condition takes the previous part's condition, so
    "near-death ATK / DEF" is near-death for both.
  - **Hand tables (with reasons in the builder).** RULE_OVERRIDES for the free-text rules of
    links 19, 30 (the stricter Kakyoin + one), 32, 42 and 57 (Kars + Wamuu); RESOLVED for link 46
    (cool time +2.0%, where the Gamerch table and Bahamut agree).
  - **Held (25, never sent):** gold/EXP 8, 21, 26, 43 (no consumer); conflict 20; name only 25,
    51, 59, 69, 70, 71, 75, 77; effect unknown 53, 64, 76; members unknown 54; two rows under one
    name 35; 60 (crit-taken 0.07, Straizo uncertain); low confidence 36, 52, 62, 67, 74; no
    record 73.
  - Not verified on a device: the formation's link window (the prefab sets how many groups show;
    the wiki says 4) and the links in battle.

## 2. Mind links

- **Data (H).** `StatueSetting.mindLinkList` is a list of
  `MindLinkInfo {mindLinkId, dataList: List<int>, abilityInfo}`.
  - `abilityInfo` is an action-battle `AbilityInfo`.
  - `IsValid` 0xD61530 needs a non-empty id, a non-empty `dataList` and `abilityInfo.IsValid()`.
  - **Trap:** an empty id with a null `dataList` throws.
- **Where it shows (H).**
  - **Ability text box:** `FormationStatueInfoUIController.UpdateUI` 0xF23458 calls
    `StatueInfoUIController.UpdateMindLinkAbilityParts` 0xF4B2D8 in both modes. For each valid
    link it prints the `abilityInfo` parameters as effect lines (`AffectTextUtil`, doc 16 §8).
  - **Link lines:** `StatueMindUIController.SetupEnableMindLinkEffect` 0xE6CA64 draws lines
    between Mind frames, in action mode only (it returns when `isDtEditMode`). For each link it
    collects the seqs of the equipped Minds whose `minduId` is in `dataList`, then sorts them and
    draws a 2-, 3- or 4-frame pattern. So `dataList` holds **minduIds**.
- **Battle use (H):** the action battle only (`ApplyMindLinkAbilityAffect` 0xF17BEC, source 5).
  The tactical battle ignores it (§1).
- **Which Minds link.** The Gamerch Mind pages list a link for 12 Minds, as bonuses for 2, 3 and
  4 equipped Minds of the same link. Labels name the links: `ABI_MIND_LINK_NAME_001` ATK,
  `_002` DEF, `_003` HP, `_004` cool-down, `_005` crit rate, `_006` move speed, `_013` ATK
  (again); 007–012 are placeholders.
- **What we send (ours where marked).**
  - `server/content/mind_links.json` (`tools/content/build_mind_links.py`): 8 roster Minds with
    an ATK, DEF or HP link. The move-speed and crit links have no tactical affect and are left
    out.
  - A seat with at least 2 equipped Minds of one link gets a `MindLinkInfo`:
    - `mindLinkId` `MIND_LINK_<nnn>` (ours);
    - `dataList` = their minduIds;
    - the ability `DT_MLINK_<STAT>_<n>` inline (`isInit` true, `paramListLv…`), with n the
      number of Minds, capped at 4.
  - The amounts are ours: +3/5/8 % (`DTAF_<STAT>_RATEADD`, 30/50/80 per mille). The wiki's are
    flat action-era numbers that differ by rarity.
  - Since the tactical battle skips links, the same ability also rides on the seat statue's
    `dtAbilityList`, as the assist abilities do (doc 19 §5.3). It works in battle and in the
    formation's status.

## 3. Skill Minds (mindType 2) and the skill frames

- **Frames (H).** `GameConst.GetMindSeqType` 0xDB2D70: seq 1–4 → 1 (golden), 5 → 2 (skill),
  6 → 3 (SP skill), others → 4 (sub). `MindSelectUIController.IsPossibleEquip` 0xD66E50:
  - frame 5 needs `skillInfo.SkillType == 2`;
  - frame 6 needs `skillInfo.SkillType == 3`;
  - frames 1–4 need `mindType == 1`.

  It reads `skillInfo` (+0x48, the action-battle SkillInfo), **not** `dtSkillInfo`.
  `IsEnableEquipMind` 0xDB5720 compares each `limitCharaList` entry with the statueId
  (`String.CompareTo`).
- **Battle (H).** `SkillStatusTable.CreateSkillStatus` 0xD8E46C:
  - **Statue skills:** each skill in `dtSkillList` with level ≥ 1 gets a slot by the table's
    `dtSkillType` (else the SkillInfo's): type 1 → slot 0, type 2 → 1, 2, …, type 3 → 3, 4, …,
    type 4 → 7, type 5 → 8.
  - **Mind skills:** then, for every equipped Mind with **mindType 2**, its `dtSkillInfo` is
    slotted by **`dtSkillInfo.skillType`** alone, sharing the same counters.
  - **Slots:** `DTBattleButtonType` names the slots: Attack 0, Skill1 1, Skill2 2, SpSkill1 3,
    SpSkill2 4, SpSkill3 5, Assist 6, Cancel 7, UnitChange 8. The fixed Cancel (7) and UnitChange
    (8) skills and the assist skill (6) are added after.

  So a normal skill Mind becomes **Skill2** and an SP skill Mind **SpSkill2** (M for how the
  buttons look).
- **Crash traps (H).**
  - A mindType 2 Mind with a null `dtSkillInfo` → NRE (no check at 0xD8E6A0).
  - A skill id with no `dtbattle_skill_data_table` row → NRE (`CreateSkillParameter`
    0xD8D5FC).
  - A sub category that names no row makes the `SkillParameter` ctor 0xD8C894 return early.
  - Types 4/5 would collide with the fixed slots 7/8 in a `ListDictionary`, so we send only
    types 2 and 3.
- **Which Mind grants which skill (from the game's labels).** `MIND_TEXT_<mindId>` says "lets you
  use the (SP) Skill «name»". The name is the label `SKILL_NAME_<p>_<NN>` with the same text.
  - 94 roster Minds say this; 87 resolve to a skill that some roster statue of prefix `<p>`
    can play (an action set `SKILL_<NN>` on its model).
  - The "SP" in the text gives the kind: 44 SP, 43 normal.
  - Tool: `tools/content/build_mind_skills.py` → `server/content/mind_skills.json`.
  - 33 Minds reuse a statue's own row of the same kind.
  - The game's kind contradicts our statue skill guess (doc 12 §3) for 39 Minds. Such a Mind
    gets its own row `MSKILL_<p>_<NN>`, with `viewSkillId` `SKILL_<p>_<NN>`, because the client
    takes the action set and the classic row from `viewSkillId`.
  - 15 are skills no statue has. They get new rows, classified by our English reading of the
    skill text (all are damage-rank hits).
- **What we send.**
  - `mindSeqInfo` opens seq 1–6 (5 and 6 with `onlySkillFlg 1`, which has no reader we found).
  - A skill Mind's MindInfo:
    - `mindType 2`;
    - `dtSkillInfo` and `skillInfo` with `skillType` 2/3, `skillLevel` = the Mind's level, and
      ids: the dt row for `dtSkillInfo`, the classic `SKILL_<p>_<NN>` for `skillInfo`;
    - `dtAbilityInfo {}`;
    - `limitCharaList` = the statues that can play it.
  - Equip (`collection::mind_fits`): ability Minds go in 1–4 and skill Minds in the frame of
    their kind, for the listed statues only (else 51709).
  - A skill Mind left on a seat whose statue changed is not sent in the TeamInfo, so the battle
    never adds a skill the model cannot play. The next equip on that seat drops it.
  - The auto formation leaves skill Minds out.

## 4. Skill levels and cool turns

- **Cool turn (H, `SkillParameter` ctor 0xD8C894).**
  `coolTurn = dtMinCoolTurn + CeilToInt((dtMaxCoolTurn − dtMinCoolTurn) / 10 × skillLevel)`.
  "Min" is the value at level 0 and "Max" the value at level 10. The level is the SkillInfo's.
  Beyond this, the level is read only by counter skills (`registCounterSkill`).
- **Tree (labels).** Square effect 4 raises the skill level (`Tree_Message_08`), 5 the SP skill
  level (`_09`), 9 the finisher's (`_11`), 13 the assist skill's (`_18`); the full list is in
  docs/research/13 §3. Build 42: finisher squares set the action `skillList[4]` level (it used
  the SP skill level), assist squares the `dtAssistSkillInfo` level.
- **What we send (ours).**
  - Rows keep `dtMinCoolTurn` 2 (special 3) and get `dtMaxCoolTurn` 0 (special 1). Level 1–4
    gives the old 2/3 turns, level 5–9 one less, level 10 two less.
  - Up to four tree squares: `A6` and `Af3` (effect 4), `D6` and `Df3` (effect 5), each +2
    levels, only for skills the statue has.
  - A statue's skill level = 1 + its opened squares (cap 10); a skill Mind's = the Mind's level.
  - With no squares opened, every battle input is what Build 24 sent: a test dump of the main
    team and of all 266 statues alone showed no TeamInfo change. In the skill table only
    `dtMaxCoolTurn` changed, plus the new sub categories.

## 5. Time stop and other skill families

- **Time stop (H).** In `Character.OnTurnMain` 0xFABE54, each of a skill's two categories (main,
  then sub) is dispatched on `ProcessType` (jump table 0x2240BFC). ProcessType 4 (WORLD) calls
  `TimeStopManager.StartTimeStop(user, category.turn)` 0xD999E4. That stores `turn + 1` as the
  stopper's count, and `NextTurn` 0xD993EC counts it down on the stopper's turns.
  - Other units do not act while time is stopped (M, from `GetTimeStopTurn` in the battle
    loop).
  - The visual comes from the action set (`DoSetTimeStop` → `StartTimeStopEffect`).
- **The client's own family data (labels).** The category labels `DTSC_*` spell out each
  family's target and turns:
  - `DTSC_WORLD_LIGHT/MIDDLE/HEAVY/HEAVY2` = time stop for 1/2/3/5 turns;
  - `DTSC_BIRI_*`/`REST_*`/`ICE_REST_*` = action seals for 1–3 turns;
  - `DTSC_ATKBUFF_*`/`DEFBUFF_*` = up/down effects for 2–3 turns;
  - `DTSC_SLIP_*` = damage over time;
  - `DTSC_CTBUFF_*` = cool turn up/down;
  - `DTSC_ERACE_*` = removals;
  - `DTSC_REVIVE` / `DTSC_RECOVERY` = revival at 10 / 100 % HP;
  - `DTSC_SKILL_<p>_<NN>` = counters.
- **What we send.**
  - `mind_skills.json` `timeStop` lists the 7 skills whose text says the user acts in stopped
    time (DIO's and Jotaro's): 300_10, 300_14, 404_06, 404_10, 332_11, 532_01, and a Mind copy
    of 300_14.
  - Their row gets `subSkillCategory` `DTSC_WORLD_LIGHT` (normal, 1 turn) or `_MIDDLE` (special,
    2 turns). The category row is `{TargetType 5, ProcessType 4, turn 1/2, paramList []}`.
  - The main category (their hit) is unchanged. Skills that only show a time stop in their
    animation stay plain hits.
- **Not built:**
  - **Buff, debuff and seal families** need `dtBuff` rows (`dtBuffId, dtBuffText, resist,
    eraceId, icon, buffEffectId, eraceTiming, affectID, giveLimit, maxStackNum, paramList[]`,
    H) and the BUFF (2) process wiring, which we have not traced. No source gives the amounts
    behind "S/M/L".
  - **Summons** need `skill_create_character_table` (doc 12 §2).
  - **Per-skill cool turns** have no source. The wiki's few values are action-mode seconds.

## 6. Unverified (M)

- On a device: the Skill2/SpSkill2 buttons and the frame-5/6 UI, the skill Mind's animation, the
  time-stop sequence (who acts, how the effect ends, whether the AI uses it), the link text box,
  and the inline link ability (`isInit` parsed from JSON).
- Whether the original skill Minds kept their statue's normal/special kind (we follow the
  Mind's own text).
- `onlySkillFlg` / `goldenFlameFlg` have no reader we found.
