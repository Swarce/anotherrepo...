# 19 — Assists (Builds 22, 24, 25)

Assist statues back up a team seat. Read in the 3.12.0 client: H = read in the disassembly,
M = inferred.

## 1. Data (H)

- Each seat (`StatueSetting`) has `assistStatueList`: a list of whole StatueInfo records, each
  with a `seq` (1–3). Seq 1 is the leader assist. Up to 3 per seat (`MaxEquipAssistNum`).
- Assist fields on StatueInfo: `assistAttr` (0 Common, 1 OnlyAssist), `assistSkillInfo`,
  `assistAbilityList`, `dtAssistSkillInfo`, `dtAssistAbilityList`.
- `[]` when empty, never null (the select screen copies it).

## 2. Route (H)

`/team/equip_multiple_assist {teamNum, statueSeq (1 leader, 2 left, 3 right), statueList:
[{equipSeq, statueuId}]}` → `{teamInfo}`.

- The request is a diff: only changed slots; a cleared slot is sent with statueuId 0.
- The reply's team replaces the cached team `teamNum`, so the whole edited team is returned.
- The select screen offers any statue that is not a main member or already an assist in the team.
- `/team/auto_set_and_equip {teamNum, changePlan}` → `{teamInfo}` (the auto formation popup;
  changePlan 1 total power, 2 ATK, 3 DEF, 4 HP; H). Built in Build 23: the three best owned
  statues by the plan, one per character, and per seat the highest-rarity Minds not on the team's
  other seats or other teams that fit slots 1–4 and the capacity (ours). Assists are kept.

## 3. Battle (H)

- **Stat bonus (tactical):** for each assist,
  `rate = base/1000 + min(boost, limitR)/limitR × addR/1000` (R = the assist's rarity 3–6), times
  `same or different statue type rate /1000`, applied to the **assist's own** HP/ATK/DEF and added
  to the seat. The constants live in `dtbattle_damage_param`; ours were missing, so 0.
- **Abilities** need `assist_ability_condition_table` rows keyed by statueId. Trap: the unused
  tactical reader finds the index in `dtAssistAbilityList` and reads that index from
  `assistAbilityList`, so both lists keep the same length and order. The tactical battle itself
  never applies them (§5.1).
- **Assist character:** in tactical battle the leader seat's seq-1 assist joins as the team's
  assist character, using `dtAssistSkillInfo` as its skill (§5.2). No skill is made while that is
  empty.

## 4. Server (Build 22)

- `Team.assists`: per seat, (seq, uid), the same for both battle types of a team, like Minds.
- Every TeamInfo sends the seat's assists as full statue records plus `seq`. A statue that later
  becomes a main member of the team is not shown as an assist.
- equip_multiple_assist applies the diff in order, moves an assist off the team's other seats, and
  refuses a main member, a seq outside 1–3 and an unknown statue (51702/51709).
- **Bonus constants (ours):** base 10%, plus up to 10/10/15/20% for rarity 3/4/5/6 at 10 boosts,
  ×1.2 when the assist shares the seat statue's type. So a fully boosted 5-star assist adds 30% of
  its own stats.
- Abilities and the assist skill: Build 24 (§5).

## 5. Abilities and the assist skill (Build 24)

### 5.1 Abilities in the client

- **Two lists (H).** `assistAbilityList` (0xF8) holds action-battle `AbilityInfo`s;
  `dtAssistAbilityList` (0x108) holds `DtAbilityInfo`s.
- **The reader that runs (H).** `CreateAssistAbilityData` 0x157EE1C takes, for each assist of a
  seat whose `assistAbilityList` is not null:
  1. `assist_ability_condition_table.GetParameter(statueId)`;
  2. for each condition, the index of `assistAbilityId` in **`assistAbilityList`**, and the
     `AbilityInfo` at that index;
  3. active when `CheckAssistAbilityActive` 0xC93910 passes: for every `{statueType, count}`
     term, the seat's assists (the owner included; `CountType` 0x157F3AC) hold at least `count` of
     that type. No terms means always active.
- **Its callers (H):** the statue screen (`StatueInfoUIController.UpdateUI` 0xF477D8), the
  ability window (`AssistAbilityWindowUIController.UpdateUI` 0xFE2E1C) and the action battle
  (`AffectStatus.ApplyAssistAbilityAffect` 0x2062264, from `CharacterStatus.SetStatueInfo`
  0xF17DC4 and `UIStatusCalclator.ApplyOldBattle`).
- **The tactical battle never applies assist abilities (H).**
  - `dtbattle.CharacterStatus.ApplyAssistAbilityAffect` 0xF01B4C and
    `UIStatusCalclator.ApplyDtAssistAbilityAffect` 0xDB2F10 have no caller: no BL or B to them, and
    no method pointer names them.
  - `DtCreateAssistAbilityData` 0x157F0E4 (the reader with the index trap) has no caller either.
  - The tactical `CharacterStatus.SetStatueInfo` 0xEFC570 passes the assists only to
    `CharacterAffectGroup.Setup` 0xECEFA4, which adds the stat bonus (`DTCalcAddParam`) and nothing
    else.
- **The condition table (H).**
  - Row shape: `{statueId, conditionTableData: [{assistAbilityId, assistAbilityCondition:
    [{statueType, count}]}]}`.
  - `Init` 0xC921EC skips a duplicate statueId (ContainsKey).
  - `GetParameter` 0xC9233C returns a new empty list for an unknown statueId. With no row, the
    statue shows no ability cards.
- **Card text (H).** `AbilityInfo.SetupFromAbilityData` 0x205E7D8 returns at once when `isInit`
  is true. Otherwise it looks in the action-battle `ability_data_table`, which we do not ship.
  `IsValid` 0x205EBFC needs an id and at least one parameter with an affectId. The card shows the
  `abilityName` label only when it is valid (`UpdateAbilityName` 0xF3E37C).
- **Crash traps.**
  - A condition with a null term list → NRE in `CheckAssistAbilityActive` (H).
  - A null `dtAssistAbilityList` → NRE in `GetAssistAbilityIndex` 0xD316C8, only on the unused
    reader (H).

### 5.2 The assist skill in the client

- **Creation (H).** Every tactical unit builds its skills from its own statueInfo
  (`SkillStatusTable.SetStatueInfo` 0xD8E3FC). It then calls `CreateAssistSkillStatus`
  0xD8E9AC with `dtAssistSkillInfo`, which adds a slot-6 skill when `SkillInfo.IsValid` holds
  (a non-empty skillId).
- **Crash trap (H).** `CreateSkillParameter` 0xD8D5FC looks the skillId up in
  `dtbattle_skill_data_table` and uses the row without a null check: an unknown skillId throws.
- **Use (H for the routing, M for the whole flow).**
  - Button 6 (Assist) on any member is routed to the team's assist character:
    `Character.CanUseSkill` 0xFAEBEC and `SkillAttached` 0xFB5454 call `GetAssistCharacter`
    0xFAED0C. With no assist character the button is off.
  - In `OnTurnMain` 0xFABE54 the assist character pays the cost and
    `CreateAssistSkillCommand` 0xFB0DB4 plays the skill.
  - `CharacterProxyManager.StartAssistCharacter` 0xEF40B8 and `CharacterProxy.StartAssist`
    0xEE9894 place the hidden assist next to the target and show it.
  - A slot-6 skill on a statue that is not the assist character is never used.
- **Which skill (M).** Assist-only statues (suffixes _6x and _7x) have `ABI_AST_<id>_N` labels.
  Their only tactical action is often `SKILL_09`, so their assist skill is probably
  `SKILL_<p>_09`. None of them is in our roster: they have no leader-ability label.

### 5.3 What the server sends (Build 24)

- **Ability rows (ours).** 16 shared rows `DT_AST_<KIND><tier>` in `dtbattle_ability_data_table`:
  - 4 kinds × tiers 1–4, named by the game's generic label keys `ABI_AST_<KIND><tier>_N`;
  - per mille, the same at Lv1 and Lv10:

    | Kind | Affect | Tiers 1–4 |
    |---|---|---|
    | HP_RATEADD | DTAF_HP_RATEADD | 5 / 6 / 10 / 20 % |
    | ATK_RATEADD | DTAF_ATK_RATEADD | 4 / 6 / 8 / 20 % |
    | DEF_RATEADD | DTAF_DEF_RATEADD | 6 / 8 / 10 / 20 % |
    | DMG_RATEADD | DTAF_DMG_RATEADD | 3 / 5 / 7 / 18 % |

  - The tier values follow ShoutWiki's assist-ability numbers where it has them (HP 5/6/10,
    ATK 4/6/8/20, DEF 6/8, damage dealt 3/5/7/18). Those numbers date from the action battle.
- **Per statue** (`server/content/assist_abilities.json`, `tools/content/build_assist_abilities.py`):
  - 35 statues take ShoutWiki's assist stat. The entry must match exactly one roster statue: by
    the statue's page, or by JP name label plus wiki rarity and type. The tier is the one whose
    value is nearest.
  - The other 231 get a default (ours): the stat their stats lean to (HP counted at two thirds),
    at tier rarity − 2.
  - Result: 156 ATK, 85 HP, 18 DEF and 7 damage abilities, one per statue.
  - Effects ShoutWiki lists without a tactical affect are skipped: move speed, cool-down,
    DoDoDo charge, KO timer, attribute targets.
- **Both lists.** Every StatueInfo sends `assistAbilityList` and `dtAssistAbilityList` with the
  same ids in the same order:
  - the action-battle list carries the name and parameters inline (`isInit` true,
    `paramListLv…`);
  - the tactical list carries ids only, and the client fills them from the table.
- **Conditions.** `assist_ability_condition_table` is generated from `assist_abilities.json`: one
  row per statue. Until Build 33 every row had no terms (always active).
  - Build 33: 116 statues take the condition their research record lists (`--research`).
  - Build 41: 175 statues.
    - The ShoutWiki parser now reads the requirement icons: `{{Type}}` or `File:Type.png`, after
      "Requirement:", "Activation Condition:" or "Activation Type Requirements:", or a bare icon
      line. It also reads the h2 `== Assist Ability ==` inside `{{Article|…|…}}` tabs.
    - 34 statues that cite a ShoutWiki revision get the condition from that same revision.
    - 8 defaults get a ShoutWiki entry. Three of these come from "Old Mode" templates (oldids 3832,
      3840, 3855, kept in `local/wiki-raw/shoutwiki/oldmode_pages.json`). With the entries,
      4190_13 becomes DEF +8% (DEF2) and 5050_04 damage dealt +18% (DMG4).
    - 17 defaults get the condition from the research records: `research-b33/action` (live Gamerch
      pages, read for the first time) and `research-b33/statues`. Gamerch's 発動条件 text and its
      JP damage line are now parsed. Records also count as agreeing when what they parse to agrees,
      so the wording no longer has to match. With these, 2081_13 becomes DMG3, 4000_18 becomes
      DMG4, and the assist-only row DT_AST_1030_62 becomes HP +10%.
  - Held, with the reasons in `HELD` in the builder:
    - 3180_62, 3230_62 and 4220_72 were rejected;
    - 4030_04, 4090_72 and 4170_16 have a ShoutWiki requirement that has not been checked yet.
  - Effects with no tactical affect keep the default kind (the leaning stat) under the wiki
    condition. These are conversions, crit, cool time, move speed and damage to a race.
  - Statues with no wiki type are counted under the server's placeholder type, so a condition
    on them depends on that guess (collection.rs `StatueDef::statue_type`).
- **Tactical effect (ours).** Since the client never applies assist abilities in a tactical
  battle, every TeamInfo puts a seat's active assist abilities into that seat statue's
  `dtAbilityList`. The battle applies that list as statue abilities (source Statue 2,
  `SetStatueInfo` 0xEFC570, H). The server runs the client's own condition check (§5.1). The
  formation status therefore includes them too.
- **Assist skill.** `dtAssistSkillInfo` is the statue's own normal skill (258 statues), else its
  special (8): an existing tactical row, so the assist plays its own model's action.
- **Enemies** send no assist skill or abilities.

### 5.4 Not verified

- None of this has run on a device yet:
  - the Assist button, the assist character's entry and its skill animation;
  - the cards in the assist window with inline parameters (`isInit` parsed from JSON is M);
  - how folded abilities look on the seat statue's ability list.
- The action battle's use of `assistAbilityList` (DTAF_* affect ids) is untested. The action
  battle is not served.
- The assist-only statues: see §6.

## 6. Assist-only statues (Build 25)

### 6.1 In the client and the player's files

- **The flag (H).** `BaseStatueInfo.assistAttr` is a `Jjs.AssistAttribute`: Common 0,
  OnlyAssist 1. The thumbnail lists have their own messages for it (`ThumbnailGroupController`
  MessageType AssistOnly 22, AssistOnlyMember 30; `isAssistOnlyText`), so the member selection
  shows such a statue as assist-only (M for the exact screen behaviour).
- **Which (H, files).** 32 statueIds have their own `ABI_AST_<id>_N` label and no leader-ability
  label (a 33rd, 2100_03, has both and is a roster statue). 26 of the 32 have a model, its
  animations and an action set in the player's files (the roster rule); the other 6 (3140_62,
  3310_61, 4010_62, 5070_61, 5090_61, 5110_71) have none and stay out. All 26 are `_6x`/`_7x`
  ids; 20 of them have a thumbnail (as 217 of the 266 roster statues do).
- **Their actions (H, files).** 18 have a single tactical action, SKILL_09 (SKILL_01 for
  3230/4090/4220); the rest have more (4000_62, 5000_62 and 4181_72 a full set).

### 6.2 What the server sends

- **Roster.** `tools/content/build_statues.py` appends them to statues.json with `assistOnly`
  true (after the 266, so nothing else changes). Stats: Gamerch by assist-ability name where it
  matches exactly (1: 4260_72), else the defaults. Rarity from their pose clips where the wiki has
  none.
- **One skill (M for the choice).** `dtSkills` holds one normal skill: `SKILL_<p>_09` when the
  model has it, else its lowest skill with a skill text. It is their assist skill
  (`dtAssistSkillInfo`, §5.3). No `dtAttack` (an assist never acts as a unit; an attack id could
  also collide with a roster skill id of the same prefix).
- **Own ability.** `tools/content/build_assist_abilities.py` gives each one its own row
  `DT_AST_<id>` named by its own `ABI_AST_<id>_N` label, with the kind and tier the defaults (or
  the wiki) give (ours: the label names the ability, the effect is a guess). 142 ability rows in
  all.
- **assistAttr 1** on every StatueInfo of theirs.
- **Never a member (ours, server side too):** not on the starting teams, refused in a seat
  (`select_statue`, `clean_members`: the client's own check comes first), skipped by the auto
  formation, cleared from seats by the save repair, never an Arena opponent or a ranking rival's
  leader (`collection::members`). They can be equipped as assists on any seat, and summons can
  draw them.

### 6.3 Not verified

- Not run on a device: the assist-only marking on the team screens, their assist skill animation
  (an action set exists for each skill sent) and their ability cards.
