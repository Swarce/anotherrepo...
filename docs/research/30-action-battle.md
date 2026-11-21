# 30 — The action battle (Build 33)

Read in the 3.12.0 client and the player's files: H = read in the disassembly or measured in the
files, M = inferred. Code: `server/services/src/quests.rs` (`area_list`, `has_action_set`),
`server/services/src/collection.rs` (`action_skill_list`), tools
`tools/content/scan_action_areas.py`, `scan_action_cinematics.py`, `build_action_skills.py`,
`build_skill_table.py`.

## 1. What the Build 32 device test showed

- An action story chapter loaded, but no enemy appeared and the player could not attack. The log
  had no error.

## 2. Enemies (H)

- `GameObjectSpawnManager.SetupSpawnData` 0xDB9CAC walks the area's `waveList`:
  - a row whose `waveNum` differs from the previous one starts a new spawn group (`BaseSpawn`);
  - a row of `type` `RESPAWN` sets the group up as a respawner (`intervalTime` ms, `spawnNum`), and
    one of `type` `TRIGGER` as a trigger zone (`pivot` = the zone, `cutinEvent`, `cameraTarget`);
  - only a row of `type` `NORMAL_SET` creates an enemy (`SpawnData`).
- `SpawnData.Init` 0xCB1E9C finds the row's `pivot` with `GameObject.Find`. Its children are the
  spawn points, and the enemy appears at child number `offset`.
- The area scenes (`area_<id>`) hold a `root` with groups `WAVE_A`, `WAVE_B`, … (1–20 spawn points
  each; 388 of 389 action areas have one), `TRIGGER_<X>` zones, `PlayerStart`, `PlayerResult` and
  camera targets.
- We sent `type` `WAVE_A` and `pivot` "01": no enemy was ever created.
- **Now:** every enemy is a `NORMAL_SET` row on the area's first `WAVE_<X>` group, one spawn point
  each (`server/content/action_areas.json`).
- AI (`WaveInfo.AIId`) indexes aipack's `AIType` list (623 entries): 7 a balanced melee mob with
  frequent attacks, 13 the ranged one, 14 the generic boss (H for the list, the choice is ours).
- Ten late statues have no action-mode action set (`actionset_statue_<psid>`; 128 models have
  one). They are left out of action waves (M: their spawn would have nothing to play).

## 3. Skills (H)

- `CharacterStatus.SetStatueInfo` 0xF17DC4 fills the action skill slots from
  `statueInfo.skillList` by index, through `GameConst.GetSkillTypeFromSkillArray` 0xDB5254
  (table 0x223DC30): 0 attack, 1 skill 1, 2 skill 2, 3 SP 1, 4 SP 2.
- `SetSkillInfo` 0xF18838 skips an entry with an empty name or a level under 1. `SkillInfo`'s
  getters run `SetupFromSkillData` 0xF98D78, which fills name, type, button, cool-down and uses from
  `skill_data_table` by `skillId` (the action battle; the tactical battle uses dtSkillDataTable).
- Which action-set entry a skill id plays (Build 41 pass):
  - `SetSkillInfo` 0xF18838 stores 0 for slot 0, the attack (`cbz w19`), so its `_99` stays the
    combo attack (`DoAttack`).
  - The other slots call `SkillInfo.GetSkillIntId` 0xF992CC, then
    `GameConst.VariationSkillIdToSkillId` 0xDB627C (calls at 0xF18940/0xF18974). That function
    returns ids under 1 and extended ids (`IsExtendSkillId`, 61..70) unchanged; any other id
    becomes NN % 10, with 0 -> 10 (`umull`/`msub`, then `csel`).
  - `FsmEventDoSkill.GetDoSkillEventName` 0xF8370C maps 1..10 through a jump table (`sub 1`,
    `cmp 9`), and extended ids through `GameConst.GetExtendSkillEventName` 0xDB5460. The action
    set hub (`<model>_800_SKILL_HUB`) answers the event with its `SKILL_<entry>` action set.
  - So `SKILL_401_16` plays entry 6 and `SKILL_400_61` plays the extended entry 61.
- We sent `skillList` empty, so nothing could attack.
- **Now:** `battle_statue_info` sends `[attack, skill, {}, SP, finisher]`, with the finisher only
  from rarity 5. §4 says where each id comes from.
- `skill_data_table` rows take those roles (skill type 2; SP and finisher type 3, button 4), with
  the wiki's cool-downs and uses (skill 5 s, SP 3 s with 3 uses, finisher 10 s with 2 uses where
  it gives none).

## 4. Which skill fills each slot (wiki H, policy M)

`tools/content/build_action_skills.py` writes `server/content/action_skills.json` (Build 41):

- **Wiki (113 statues, 48 with a finisher).** The skill, SP and finisher names on the Gamerch
  statue pages (research b33, plus five SP2 blocks read again in b34), matched to the game's
  `SKILL_NAME_<p>_<NN>` labels, per slot:
  - strict: equal after a wide normalisation (NFKC; brackets, dots, dash variants, tildes, `!`/`?`
    and one trailing ッ dropped; `［＋］` kept, since 304_14 and 304_18 differ only by it);
  - several labels with the same text: a finisher takes the one at the retail cinematic entry
    (§5). This fixes 3040_03/3040_13, which got `SKILL_304_03` (no cinematic) instead of `304_08`.
    A skill or SP keeps the first label (the 4011, 3001, 4040, 4190, 3050 and 3321 groups are
    unresolved);
  - fuzzy: a difflib ratio of at least 0.80 with the runner-up under 0.60 (wiki typos and
    kana/kanji variants; 25 slots);
  - an SP2 block with a blank name (3330_03) takes the single label of the statue's NN decade at
    the cinematic entry.

  A statue with any matched slot counts as wiki-sourced; each slot's match method is in its note.
- **Otherwise (jdr-default, 179 statues).** The statue's tactical skills: the normal one as the
  skill and the special one as the SP skill. A skill that would play the cinematic entry, or has
  no action-set entry (5290_04), is skipped. From rarity 5 the finisher is the single label at the
  cinematic entry (57 statues). None is set when several labels sit there (28 statues held, e.g.
  3041 304_06/304_16, 4011 401_06/16/26), and none for 4000_18, whose ★6 page has no SP2 block.
- The wiki supports this policy: across the Gamerch statues the cinematic never sits in the skill
  or SP slot, and every wiki finisher but 3090_03 is the cinematic (3090 has no cinematic set).
  Whether 6★ awakenings had a cinematic finisher in the action era is unknown (M).

## 5. The SP cinematic (H)

- The statue's own action set starts it: its `SKILL_<entry>` FSM runs `DoChangeBattleSceneMode`
  (or the `CameraKeep` variant) with `mode` 2. The chain is:
  - `DoChangeBattleSceneMode.OnEnter` 0xCFF29C;
  - `ChangeModeActionBattle` 0xCFF374;
  - `BattleManager.ChangeMode` 0xC072B8;
  - `StartSpecialSkill` 0xC07694;
  - `SpecialSkillSceneControl`;
  - back to battle with `ReturnToBattleMode` 0xCB2FD4.
- `GetCameraAnimatorController` 0xCB3040 builds the camera's bundle and asset path from
  `CharacterObject.PersonalId` and `spSkillVariation`, then calls `ResourceManager.GetResources`.
  The bundles are `animation_statue_spskill` (83 in the player's files).
- No server data picks the camera, the scene or the clips.
- `server/content/action_cinematics.json` (`tools/content/scan_action_cinematics.py`) lists, per
  model id, the cinematic entries and every `SKILL_<NN>` entry of its action set. Entry numbers
  only.
  - 70 of 128 action sets have a cinematic entry. It is entry 6, 8, 9 or 10; 3321 has two (6 and 8).
  - 13 model ids have an spskill bundle but no mode-2 set: the 10 statue models without an action
    set (3323, 4031, 5001, 5030, 5080, 5100, 5150, 5170, 5220, 5320), 5140 (its action set has no
    FSM), and 5070 and 5110 (no action set, no statue in our roster).
- `statue_special_scene_data_table` stays empty (a test checks it). Its `Data` holds:
  - `statueId`;
  - `specialRetireData` / `dtSpecialRetireData`: a condition, two params and long/short
    `ExcelCameraId`;
  - `spcialSkillSceneData`: `spSkillVariation`, `spSkillSceneParam1`, `spSkillSceneParam2`.

  `IsEnableSpecialSkillSceneData` 0xCB4024 needs both params non-null, and `CreateActorObject`
  0xCB3624 calls `Substring` on them, so a short or blank param would throw. No retail rows exist
  for either part.
- Tests (`collection.rs`):
  - no served skill or SP id plays a cinematic entry;
  - every jdr-default finisher plays one;
  - wiki finishers are not required to (3090_03).

## 6. Open (M)

- Action-stage star missions, the callable support and Mind Links in action battles (JoJo Links:
  §7, Build 41).
- Waves beyond the first group of each area; respawning and trigger groups.

## 7. Leader and assist abilities, statue traits, JoJo Links (Build 41)

Read in the 3.12.0 client (H unless marked). Code: `collection.rs` (`action_leader_ability`,
`tribe_type_list`, `assist_ability_list`, `ACTION_AFFECT`, `jojo_link_list`), tools
`tools/content/action_vocab.py` (the shared parser), `build_action_abilities.py`,
`build_assist_abilities.py`, `build_statue_traits.py`, `build_jojo_links.py`.

- **AbilityInfo.** The action battle needs `isInit` true and the params inline:
  `{abilityId, abilityLevel, abilityName, abilityParamList [{affectId, giveLimit,
  paramListLv1_0..7, paramListLv10_0..7}], isInit}`. With `isInit` false it looks in the action
  AbilityDataTable, which we do not ship. `IsValid` 0x205EBFC needs an id, a param list and one
  affectId. AddAffect 0x2062884 clamps the level to 1..10; every slot is
  `RoundToInt(Lerp(Lv1, Lv10, (L-1)/9))` (GetAffectParamValue 0x205F328). From an affect_table
  row only `affectId`, `prvs` and `baseAffect` are read; an unknown id is dropped silently.
  `giveLimit` 0 is never active, -1 never runs out.
- **Units** (UpdateActiveAffectParameter 0x2066F84, jump table 0x236a85c). Every `*_CON` /
  `*_RATECON` is `fixed + RoundToInt(permil × 0.001 × base)`. p1 = target tribe (`ATRBT`) or
  StatueType (`STYPE`); p2 = seconds × 10 (DEFEAT, ASSIST_USE), the HP threshold per mille
  (OVER/UNDER_HP) or the holder tribe (ATRBT). The bases with a consumer: 1–20, 23–26, 27,
  45–57, 60, 61, 74–79, 81, 88–91. CONVERT 68–73, BARRIER 59 and gold / EXP 29–33 take the
  default case (no effect).

  | Wiki | p0 |
  |---|---|
  | `N%` | N × 10 (taken damage and "-" lines negative) |
  | `xX` | round((X − 1) × 1000) |
  | flat `+N` | the `*_CON` base, N |
  | SP uses +N | `BASE_SPSTC_ADD` (27, CalcAdditionalSpSkillUsableCount 0xF18E3C), N |
  | `X% of HP/DEF → ATK` etc. | `*_BOOST_*` (74–79, CalcParamBoostValue 0x206A22C), N × 10 |

- **Conditions** (UpdateAffectCondition, jump table 0x236a808): ORAORA = DoDoDo mode, DYING
  = HP ≤ 30%, ALONE, HALF ≤ 50%, FULL ≥ 100%, DEFEAT and ASSIST_USE stack, ATRBT = the holder's
  tribe flag has bit p2, OVER/UNDER_HP = rate ≥ / ≤ p2/1000.
- **Stacking.** AmplifyAffect 0x2066150 adds a timer of p2 × 0.1 s up to 15 stacks; the effect
  applies once per stack. Kills feed DEFEAT on the killer. ASSIST_USE is fed only by event 18
  (ChangeAssistTeamUnit) of the action assist character, which we do not send (assistSkillInfo
  `{}`): those lines show on the cards but do nothing yet (M on who raises the event).
- **Order** (CharacterStatus.SetStatueInfo 0xF17DC4): tribe flag and base stats, statue
  abilities, Minds, JoJo links (4), leader (6), support (7), Mind links, assist abilities (11),
  tarot, then max HP. So HP lines count at battle start only, and an HP-type base may only use
  PRVS_NONE or PRVS_ATRBT; ATK and DEF are live.
- **JoJo Links.** `teamInfo.jojoLinkList` (+0x28) goes to all three members:
  ApplyJojoLinkAbilityAffect 0xF17A30 applies each entry whose abilityInfo is valid (source 4,
  key charaLinkId, so the ids must be unique). type, dataList, name and matchType are never read,
  and the client checks no members: the server decides (doc 26 §1).
- **Sources.** BattleSceneUtil.CreateTeam 0xC11564 applies the leader seat statue's
  `leaderAbilityInfo` to all three members; a supporter's when `followStatus` is 3 (we send 3).
  Assist abilities: per seat statue, found by id in that assist's `assistAbilityList`
  (GetAssistAbilityIndex 0xD316C8), so the ids stay those of `dtAssistAbilityList`.
- **Crit.** probability = max(5 + dealer delta + receiver delta, 0), banker's rounding: x1.10
  adds nothing alone, x1.20 adds 1%.
- **tribeTypeList.** `[{tribeTypeId}]` is OR'd into the attribute flag (AffectStatus.SetStatueInfo
  0x2062124): the ATRBT conditions and targets, the detail screens' trait list and the tactical
  Vampire death effect read it. The tactical battle ignores `leaderAbilityInfo` and
  `assistAbilityList`.

What we send:
- **Leader** (`server/content/action_abilities.json`): 142 statues from the wiki leader numbers
  (136 Wayback dump, 6 live Gamerch pages), 328 lines, 131 statues with every line served. Lv10 =
  the wiki MAX when plausible (220 lines), else Lv1; a bare `(v)` after the value is read as the
  MAX (3 lines, M). `ACTION_LEADER_LEVEL` = 1 (the wiki's Lv1 values; raising it is an owner
  decision). Held: gold/EXP (1010_01/02/03, 4110_01/02/13), BARRIER (4190_13, 3321_03, 3100_12),
  DoDoDo-mode DoDoDo charge (2111_03, no label), PER_STYPE (3000_13, not traced), 4150_03
  "ATK+37.8" (not an integer). 4110_01 sends `{}`.
- **Assist** (`assist_abilities.json` "action", 104 statues): the cited record's lines (JP via
  the parser, ShoutWiki `{stat, value}` through an English table), else the inline tactical row;
  `DTAF_DMG_RATEADD` and the other tactical-only affects become their `AF_*` twins. The card name
  follows the first line's kind (`ABI_AST_<KIND><tier>_N`), so a move-speed card no longer says
  "ATK Assist". Held lines: KO timer, collision and explosion damage, invulnerability,
  always-crit, status procs, two conditions, extra text.
- **Traits** (`statue_traits.json`): 261 of 292 statues (134 live Gamerch pages, 61 ShoutWiki
  categories, 66 family fill); 31 statues in 17 families have none. Gamerch wins the 6
  disagreements (ShoutWiki misses a category). Assist-only Gamerch records have no leader record
  to match by, so they are not used.
- **JoJo Links** (`jojo_links.json`, doc 26 §1): 53 links served (48 by members, 5 by trait:
  11, 12, 13, 14, 33), 25 held; Lv1 = Lv10, sent at level 1.
  New rows `AF_SPSTC_ADD` (SP uses +1) and `AF_FNT_VCT_CRT_DMG_RATESUB` (near-death crit damage
  taken).
- Not verified on a device: inline `isInit` cards, the leader window, the trait screens, the link
  window.
