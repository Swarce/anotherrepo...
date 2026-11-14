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
  | `myPagePos` | (82, -115, 366) |
  | `myPageRotation` | (0, 192.5, 0) |
  | `teamScaleScalar` | 100 |

  These put models 1.0–2.3 m tall where the My Page leader touch area is. Six very tall models
  may need a per-height position later.
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
