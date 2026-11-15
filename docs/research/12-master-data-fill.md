# 12 — Filling the master data (Build 13)

Our own analysis of the 3.12.0 client. Addresses are RVAs in `libil2cpp.so`. Confidence: H = read in
the code, M = inferred, L = a guess or a value we chose. No game text is reproduced; content cites
label keys only.

The client's `in_memory_master_data` holds about 45 tables (doc 09). The original rows died with the
server. Each table here is filled from the best source available, in this order:
1. the player's own files: label keys, model, bullet and action-set names;
2. the community wikis (numbers only);
3. a neutral value we choose, marked `jdr-default`.

Every content file says which applies.

## 1. Tables the client reads at startup (TitleLoad)

| Table | How the client reads it | What we send |
|---|---|---|
| `yogo_table` (glossary) | null-safe: a missing table shows an empty glossary (0xDEBED8) | 167 terms by their `YOGO_*` label keys; a term without a reading key uses the client's own "―――" marker, which hides the reading box |
| `characteristic_data` | **`Init` throws on a duplicate `id` (0xF1D188); battle NREs if it is missing once minds carry characteristics (0xF13D7C)** | the 6 characteristics and their strong/weak pairs from label keys; Lv1 +10% (ShoutWiki), higher levels ours |
| `dtbattle_statue_affinity_table`, `statue_affinity_table` | `affinityMatrix[attacker].paramList[receiver]` is indexed without bounds checks, and short percent lists give 0 damage. So the table is either missing (neutral) or a full 6×6 | full 6×6 matrices. The chart (Brave > Tenacity > Intelli > Brave, Unity ⇄ Maverick) comes from the game's own tips; the percentages (150 / 75 / 100) are ours |
| `oraoramode_setting` | an empty asset replaces the client's fallback rates with 0 | the client's own fallback constants (2.0 damage dealt, 0.5 taken, its speed-ups) |
| `affect_table` | read by name; nothing uses it until abilities or buffs exist | the 3 affect ids the client names in code (`DTAF_SYS_OVERKILL`, `DTAF_SYS_TUTORIAL_FINISHLOCK`, `DTAF_HP1`) |
| `clan_table`, `clan_object_table` | Towns (hidden offline) | levels and objects by their label keys; exp ours; models empty |
| `effectReductionParam`, `state_abnormality_table`, `characterestic_skill_parameter` | null-safe; empty behaves like missing (`LoadSkillUseConditionParameter` throws on a duplicate `skillIdHash`, 0xF1BECC) | empty |

Hashes: `buffIdHash` and `skillIdHash` are `h = 31·h + c` over the string (H). A label lookup
returns the key itself when the label is missing (H).

## 2. Battle tables

- **Reactions** (`dtbattle_reaction_table_{normal,mini,mini_no_blow}`, `Jjs.ReactionTable`).
  - A killed tactical unit is retired only by its DIE action set. `OnDead` 0xEE3270 adds it to a
    retire list, `WaitCharacterRetired` 0xEFB538 waits on that list with no timeout, and only
    `DoRetireCharacter` inside `common_112_DIE`/`common_115_DIE_DOWN` clears it.
  - Without the table the DIE action name hashes to 0 and `RequestChangeActionSet` returns early
    (0xEE20C4). So a battle could stall after the first kill, and hit reactions never played (M-H).
  - We send row i = `AttackReactionType` i, mapped to the common action sets (DAMAGE_S … DIE,
    DIE_DOWN), entry 0. Each row has 3 items, because the action battle picks by receiver status.
    An empty `item` array throws (0xEE76D4).
  - The action battle's `reaction_table_*` get the same rows.
- **`model_table`.**
  - `modelIdHash` is `String.GetHashCode(prefab name)`, the Java-style `31·h + c` (0x13D8EA8) (H).
    The 210 model names from the player's statue bundles have no collisions.
  - The tactical battle reads three fields: the reaction table (0), the Stand flag (turns the
    shadow off) and the height type (Small for the 12 models under 1 m).
- **`bullet_parameter`.**
  - Without a row a bullet dies at launch; that affected the basic attack of 7 statues.
  - We send 394 rows from the player's `bullet_<id>` FSMs. The owner model comes from
    `frameevents_statue_battle` BLETList, or every model of that statue when unknown.
  - Pool and lifetime values are ours; effect ids are 0.
- **Action-battle and PvP only:**
  - `battle_damage_param` (3 rows: normal, PvP, AI arena; ours);
  - `battle_combo_param` (read at Count−1 without a check, so non-empty: `[1.0]`);
  - `battle_pvp_statue_status_parameter` (all rates 1.0).
- **Left out (no source, null-safe):**
  - `skill_create_character_table`: summon skills only;
  - `additional_statue_load_table`: extra preloads only.
- Rebuild: `tools/content/scan_battle_assets.py` (inventory of the player's files) then
  `tools/content/build_battle_tables.py`.

## 3. Skills and the core tables

- **Script cameras** (`Set_el_camera_set_common`, `Node_el_camera_node_common`).
  - Doc 09 was wrong that these are safe empty. `PlayStart` 0xD7020C sets playEnable before it
    fails on a missing camera id, and both CameraManagers ignore the failure (0xE973E8, 0xFA6CE8).
    The camera never reports its end, so flows that wait for it stall (M).
  - The client hard-codes these ids (GameConst cctor 0xDB3124):
    - PvP intro 41/42 and round finish 351;
    - boss intro 201 and end 350/351;
    - battle result 301;
    - tactical stage finish 100300/100301/100310/100311;
    - tactical result 101005–101028.
  - We send one set per id: a one-second shot on the node `ski_cam_00`, whose values come from the
    client's own `ScriptCameraTestData`. flagMaintain stays false, or the camera never finishes.
- **`team_rank_table`.**
  - The client computes score = hp/1000 + atk/2000 + def/2000 + ora + 0.003 × mind capacity
    (0xB87AF8) and shows the highest rank whose passingScore ≤ score (H).
  - Thresholds are ours (E 0 … SSS+ 185).
- **`mind_level_table`.** EXP is cumulative and indexed by level−1 (0xBFBDE4). The curve is ours.
- **`skill_data_table`** (classic).
  - Only the classic tabs, the dictionary and the old action battle read it; the tactical battle
    does not (0xF98D78, M).
  - We send one row per `SKILL_NAME_<X>` key (1,109). Names and texts are label keys
    (`CreateSkillInfo` 0xD04880 builds `SKILL_NAME_` + X); numbers are ours.
- **Tactical skills** (`dtbattle_skill_data_table`, generated).
  - `dtSkillName` is a label key (0xF99EA8). `dtSkillProfileText` is `DTbattle_SKILL_TEXT_<p>_<NN>`;
    the client fills in its damage rank, wait and category tags (0xD944A8). Category ids are the
    `DTSC_*` label keys.
  - No source says which SKILL_NN belongs to which variant. Variant k of a character takes
    SKILL_{2k−1} (normal) and SKILL_{2k} (special), else the k-th available pair, among the NN with
    both an action set and a tactical text key (M/L).
  - The text is classified locally into damage or heal, one target or all, and an element. Damage
    and heal skills are kept; buff skills wait for buff data.
  - Result: 382 skills (188 statues get two extra buttons, 73 one, 5 none) and 13 categories.
- **`dtbattle_damage_rank_table`.** The rank index is the damage rank clamped to Count−1
  (0xEFE344..0xEFE418). It now has 256 entries of 100 × rank: the attack is rank 100 (×1.0), normal
  skills 125, specials 150.
- **Left empty on purpose:**
  - `statue_special_scene_data_table`: rows trigger special death cameras and spawn a second actor;
    a guess can spawn the wrong one;
  - `assist_ability_condition_table`: no assist statues in the roster;
  - `map_gimmick_pattern_table`: old-mode gimmicks only;
  - `affect_text_rule_table`: every row needs exactly 8 rules or it throws, and there is no source;
  - `action_branch_data_table`: needs affect data first.

  All five are null-safe when empty.

## 4. Leader abilities in the tactical battle (Build 14)

Data path (H):
1. The battle reads the leader's (`teamOrder.leaderStatue`) `statueInfo.dtLeaderAbilityInfo`
   (0xEBCC30) and applies it to every player member as source Leader (0xEBDE14 → 0xEFC6C0).
   Enemies get none.
2. `DtAbilityInfo.SetupFromAbilityData` 0xD0EE00 looks the id up in `dtbattle_ability_data_table`.
   If it finds the row, it overwrites the name and parameters from the table. The server must
   still send `abilityParamList: []`: a null list throws (0xD0EEC8).
3. Each parameter becomes an affect, which is silently dropped if `affect_table` has no row for its
   id (0xECFD48).

Values (H):
- value(level) = RoundToInt(Lerp(Lv1_i, Lv10_i, (clamp(level, 1, 10) − 1)/9)) over 8 slots
  (0xEDA880, 0xEDAA44, GameConst 8/1/10).
- Every value is **per mille**.
  - Stat rates: fixed + RoundToInt(permille × 0.001 × base) (`CalcConfigValue` 0xCCA5C4).
  - Damage dealt and damage taken: rate = 1 + Σ permille/1000, clamped to 0.1–5.0. Positive means
    more damage on both sides, so "damage taken −9%" is −90 (0xEFDF34–0xEFE8B4).
  - Crit damage: base 1.0, × the 1.5 crit rate (0xEFF180).
  - Per-statue-type damage: slot 1 holds the StatueType (0xED4458).
- `giveLimit` 0 makes the affect expire at turn end (0xFB8F0C), so we send −1, which is never
  reduced (0xFB99C4).
- Conditions take no parameters: DYING (HP ≤ 30%), ALONE (all others dead), FULL_HP.

What we send:
- 88 statues get `DT_LEAD_<statueId>`, with the name label `ABI_LEAD_NAME_<statueId>`. Each
  ability carries the wiki's leader-ability numbers, the level-1 value and the MAX value as level 10.
- 21 new affect rows join the 3 existing ones.
- The server sends `abilityLevel` 1.
- Left out:
  - action-battle-only effects (move speed, cool-down seconds, gold…);
  - DoDoDo mode (reversal is off);
  - attribute targets (statues carry no `tribeTypeList` yet);
  - crit-rate multipliers (the client supports only a fixed add);
  - heal (capped by `_limitedHealRateMax`).
- Rebuild: `tools/content/build_leader_abilities.py`.
- Risk: the wiki numbers come from the action-battle era, so the balance is ours. The ability's
  description in the UI may be blank, because these stat affects have no text keys.

## 5. Battle camera and formation (Build 15)

What the Build 14 device log and owner showed: tactical battles were seen from behind the player
team, not from the side, and the win shot was wrong.

- **Turn camera** (H).
  - Overview and focus call `UpdateNearFarCamera` with `cameraOffsetInWorld = 1` (0xCD3BE4,
    0xCD3C2C). `CalculateNearFarCameraTargetPose` 0xCD4760 then looks at the midpoint of the two
    team centres from midpoint + (0, height, length) **in world axes**.
  - The built-in parameter asset has a negative length, so the camera always stands on world −Z
    looking +Z.
  - Our old formation (players at z = −3 facing +Z) put it directly behind the players.
  - No dtbattle code reads AreaInfo `mapCamera`/`startCamera`/`rotate` (M-H). `mapPoint` picks the
    `PosPivot/<n>` origin in `<mapId>_collider_dt`.
- **Formation.** The teams now face each other along X: players at x = −3 facing +X, enemies at
  x = +3 facing −X, spread in depth (z 0; ±1; −1.6/0/+1.6). The map arenas are about 11.2 × 7.7 m
  (measured across 102 collider scenes). Axis and facing are H; the exact numbers are ours.
- **Script cameras.**
  - The statues' battle animations request 252 camera ids (ids 1000–2999 get +100000). We served
    34, and on a missing id the camera stays in script mode and freezes on its last pose (M).
  - The set table now covers all 266 ids: the hard-coded ones plus every requested one, listed from
    the player's action sets.
  - Result shots (301–314, 101001–101028) use `jdr_result`, a three-quarter front shot 4 m from
    the winner, bound to the unit. The winner is moved to the map's `ResultPivot`.
  - Everything else uses `jdr_side`: world-fixed (`flagWorldSet`), on the −Z side of the acting
    unit, 9 m away. The client mirrors some ids for enemies; the world-fixed shot cannot be flipped.
  - Node values are ours (M).

## 6. Attack distance and Stands (Builds 16–17)

- **Where the user stops (H).** `CharacterProxy.SetSkillTargetPos` 0xEE9304 reads the skill's
  `dtRangeMin`/`dtRangeMax` × 0.01 (centimetres). A user farther than the maximum walks to it; a
  user nearer than the minimum backs off to it. With 0/0 the attacker walked into the target's
  centre.
- **Where the hit lands (H).** The attack then closes in to a fixed 1.05 m from the target
  (float 0x223E5A4).
  - A Stand appears on its user (`DoAdjustPairCharacterPose` 0xCFE298, offset 0 for almost all
    Stands) and rushes forward (`ProcessMoveAhead`, `OnUpdateDtBattle` 0xDC3540). The rush is
    typically 6 m/s for 0.35 s, about 2.1 m at most.
  - So with the user only 1–1.5 m away, the Stand stayed inside its user or snapped behind it.
- **`standRange` and model_table hit sizes play no part in the tactical battle (H).**
- **Ranges we send** (ours):

  | Unit | dtRangeMin/Max (cm) |
  |---|---|
  | Stand users | 200/250 |
  | Units without a Stand (they lunge themselves) | 120/150 |
  | Bullet users | 400/800 |
  | Heals on allies | 0/1000 |

  A 3-digit skill prefix counts as a Stand user when any statue sharing it has a Stand model.
