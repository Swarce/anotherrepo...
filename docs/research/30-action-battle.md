# 30 — The action battle (Build 33)

Read in the 3.12.0 client and the player's files: H = read in the disassembly or measured in the
files, M = inferred. Code: `server/services/src/quests.rs` (`area_list`, `has_action_set`),
`server/services/src/collection.rs` (`action_skill_list`), tools
`tools/content/scan_action_areas.py`, `build_action_skills.py`, `build_skill_table.py`.

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
- The skill id `SKILL_<p>_<NN>` gives the event `DoSkill<NN>` (`FsmEventDoSkill`), which the
  action set hub (`<model>_800_SKILL_HUB`) turns into its `SKILL_<NN>` action set; 99 is the combo
  attack (`DoAttack`).
- We sent `skillList` empty, so nothing could attack.
- **Now:** `battle_statue_info` sends `[attack, skill, {}, SP, finisher]`.
  - From the Gamerch statue pages (live wiki, action era) where the skill names match the game's
    labels: 109 statues, 26 with a finisher.
  - Otherwise the statue's tactical skills (normal as the skill, special as the SP skill).
  - The finisher only from rarity 5.
- `skill_data_table` rows take those roles (skill type 2; SP and finisher type 3, button 4), with
  the wiki's cool-downs and uses (skill 5 s, SP 3 s with 3 uses, finisher 10 s with 2 uses where
  it gives none).

## 4. Open (M)

- Action-stage star missions, the callable support, JoJo Links and Mind Links in action battles.
- Waves beyond the first group of each area; respawning and trigger groups.
