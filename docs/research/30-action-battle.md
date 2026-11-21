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

- Action-stage star missions, the callable support, JoJo Links and Mind Links in action battles.
- Waves beyond the first group of each area; respawning and trigger groups.
