# 16 — Mind equipment (Build 21), abilities (Build 22) and levelling (Build 23)

Read in the 3.12.0 client (H read in code, M inferred).

- **Where Minds live (H).** Equipped Minds belong to a **team seat**:
  `TeamInfo.teamOrder.{leader,memberLeft,memberRight}Statue.equipMindList`, each a full MindInfo
  plus `seq`.
  - `/statue/statue_info` cannot carry them.
  - We store `(seq, mind uid)` per seat, the same for both battle types of a team number, like the
    statue slots.
  - Only filled slots are sent: the client counts the list against the open slots.
- **Slots (H).** There are 8. Seq 1–4 are golden frames (`mindType 1`), 5 the skill frame
  (`skillType 2`), 6 the special-skill frame (`skillType 3`), and 7–8 sub frames.
  - A slot is open when `mindSeqInfo.seqInfoList` (from `team/team_info`) lists it with `openFlg 1`.
  - We open 1–4, as at launch. `/team/unlock_mind_seq` answers with the same info.
- **Capacity (H).** `StatueInfo.mindCapacity` comes from the server; the client never derives it.
  Used CP is the sum of the equipped `cost`s. We send 40 (ours), where it used to be 99. Cost is
  3 × rarity, so three ★4 or four ★3 Minds fit.
- **Equip: `/team/equip_multiple_minds`** `{teamNum, statueSeq, mindList:[{equipSeq, minduId}]}`
  → `{teamInfo}`.
  - The request is the client's diff of changed slots. `minduId 0` clears a slot, and the clears
    come first; "remove all" sends a clear for every filled slot.
  - The server applies the entries in order. A Mind on another seat of the same team moves here,
    which the client asks the player to confirm.
  - Refusals use the client's codes: 51702 no statue, 51709 slot locked, 51103 Mind missing,
    51705 CP over.
  - `/team/equip_mind` (unused in 3.12) shares the same logic.
- **Lock: `/mind/protect`** `{minduId, protectedFlg}` → `{mindInfo}`. The client reads
  `mindInfo.minduId`, so the old `{}` reply crashed. It now stores the flag and returns the full
  MindInfo. `/mind/protect_multi` stores and replies `{}`.
- `mind_list` sets `useFlg 1` for Minds equipped anywhere.
- **Effects (H).** In 3.12 a Mind's `hp`/`atk`/`def` are never read: stats, team power and battle
  use the statue's alone. A Mind acts only through ability and skill infos (`dtAbilityInfo`,
  `dtTokkouInfo`, `dtSkillInfo`, link abilities).
  - Build 21 sent empty `{}` (skipped safely), so equipped Minds did nothing in battle.


## 6. Mind abilities (Build 22)

- **Battle path (H).** For each equipped Mind the client applies `dtAbilityInfo` with the same
  code as the leader ability: it looks the id up in `dtbattle_ability_data_table`, takes the name
  and parameters from the row (`isInit` false), clamps `abilityLevel` to 1–10 and interpolates
  Lv1→Lv10 (per mille). Abilities are always on (`giveLimit -1`). A Mind cap from
  `affect_table`'s limit lists follows; our lists are empty, so no cap.
- **Safe values (H).** An unknown or empty `abilityId` is skipped; `abilityParamList` must be
  `[]`, never null. Inline parameters (`isInit` true) use the names `paramListLv1_0…`, not the
  table's `paramLv1_0…`; we use the table.
- **No numbers survive** for Mind abilities. Only 124 of our 417 Minds have an
  `ABI_MIND_NAME_<id>` label, and the client never shows the name. So the rows are ours
  (`tools/content/build_mind_abilities.py`, jdr-default):
  - 12 shared rows `DT_MIND_<HP|ATK|DEF>_<rarity>`: Lv1 +2/3/5/8 % for rarity 1–4, doubled at
    Lv10.
  - Each Mind gets the stat its wiki stats lean to (`collection::mind_focus`: HP counted at two
    thirds; ATK when the wiki has no stats, which is 258 of 417 Minds).
  - `abilityLevel` is the Mind's level (1 until Mind levelling exists).
- **Detail screen (H).** It shows no ability name, only an effect line built from each affect
  row's `affectTextId` with `MindInfo.level`. Until Build 24 our affect rows had
  `affectTextId ""`, so that line was blank; see §8.
- Abilities apply in slots 1–4 (7–8 too, which stay locked offline).

## 7. Mind levelling (Build 23)

- **Routes (H).** `/mind/strengthening {minduIdBase, mindList:[{minduId}], goldRatioInteger,
  goldAddition}` (fodder Minds) and `/mind/strengthening_item {minduIdBase, useItemList:[{itemId,
  count}], …}` (items); both reply `{mindInfo, userInfo, mindList, composeType, itemList}`.
  Build 22 answered them with `{}`, and the result screen crashed reading `mindInfo.maxLevel`.
- **Curve (H).** `mind_level_table` has one row per rarity with the cumulative EXP of levels 1–10;
  the client reads it by position (level L = entry L-1), or the Mind's own `levelUpTable` when
  that is not empty. The cap is `MindInfo.maxLevel` (10).
- **EXP (H).** Items: `exp × count`, ×bonusRate/100 when bonusCondition is 3 (Minds) or 7/8/9 for
  rarity 3/4/5 — the Mind medals give ×1.2. Fodder: `materialExp` (a bonus only for EXP Minds,
  type 3; ours are type 1). Boost items cannot be used on Minds. At most 20 fodder or item kinds;
  protected or equipped Minds cannot be fodder. The client allows one item past the cap, so the
  server clamps.
- **Gold (H).** Fodder: `mindGrowthRare{r}Gold` (100/200/300/400) × target level, with the gold
  ratio; items: `useGold` per item.
- **Server.** Repeats that arithmetic, refuses with the client's codes (51201 protected, 51202
  equipped, 51203 missing, 51205 at the cap, 10102 gold, 10108 items), saves level and EXP on the
  Mind and sends `levelUpTable` and `dtAbilityInfo.abilityLevel` = level. `materialExp` is ours:
  100 × rarity × level. Every Mind stays owned offline, so a Mind used as fodder comes back as a
  fresh level-1 copy.

## 8. Effect lines (Build 24)

The Mind detail screen, the statue detail (leader ability), the formation ability window, the
battle pause screen and the support pick all print ability effects through
`AffectTextUtil.CreateAffectText` (H: callers of 0xDAB998/0xDABD54, among them
`MindInfoUIController.DtUpdateUI` through `UIUtil.SetAbilityText` 0xBA0808,
`StatueInfoUIController.DtUpdateUI`, `FormationUIController.DtUpdateAbilityInfoWindowUI`).

- **Inputs (H, `AddAffectTextParamList` 0xDABA5C).** Each ability parameter becomes one entry
  `{giveLimit, params[8], affectId, affectTextId, condition, baseAffectType, textSortIndex}`. The
  params are the values at the ability level (Lv1→Lv10 lerp); condition, base and text id come
  from the `affect_table` row. A parameter with an empty `affectId` or no `affect_table` row is
  skipped; an empty `affectTextId` is not.
- **Merging (H, `CreateAffectText` 0xDABD54).** The entries are grouped by `affectTextId`. The rule
  of a group is `AffectTextRuleTable.GetRuleParametor(condition, baseAffectType)` of its first
  entry (0xC8FA8C → `GetParametor` 0xC8FC2C, cached under `base × 1000 + condition`):
  - it starts as 8 slots of `Invalid`;
  - if `pvrsRuleList` has a row for the condition, all 8 slots are copied from it;
  - if `baseAffectRuleList` has a row for the base, its non-`Invalid` slots override;
  - a row with fewer than 8 slots throws (IndexOutOfRange); `GameConst.AffectParamCount` = 8
    (cctor 0xDB40F0). The lists are keyed by `pvrs` (+0x10) and `baseAffectType` (+0x14); the
    first row of a key wins (`OnInit` 0xC8F85C).
  - Inside a group, entries whose `Solo` slots hold the same values merge into one line
    (key = the `Solo` values, other slots `INVALID`); `giveLimit`s add up.
  - Per slot, `CombineParam` 0xDAD198: `Combine` (1) adds, `Solo` (2) takes the value, `Invalid`
    (0) gives `GameConst.InvalidAffectParam` = **−1** (cctor 0xDB3264).
  - **No rule does not skip the line (H).** An earlier note said it did. Instead every slot
    shows −1, so `{decimal_Param0}` prints "0.1" and `{mag_Param0}` "0.99". With the empty
    table we sent before Build 24, every effect line would have read that way.
- **Text (H).** Lines are sorted by condition, then `textSortIndex`, and joined with "\r\n". Each
  is `TextManager.GetString(affectTextId)` with the replace tags bound to the merged slots
  (`{Param<i>}` |p|, `{decimal_Param<i>}` |p|/10 as "0.0" 0xDAD7D8, `{mag_Param<i>}`
  ((p+1000)/10)/100 as "0.00" 0xDAD8F4, `{StatueType_Param<i>}`, …). `{BaseAffect}` is
  `GetString("BASE_AFFECT_TEXT_<base>")` and `{Prvs}` is `GetString("PRVS_TEXT_<condition>")`
  (0xDAE01C, 0xDAE174; the number is the enum's int). With a limit mode and a capped Mind
  ability, `AFFECT_UTIL_LIMIT` is appended.
- **Labels (H, read in the player's text.jp).** The client registers each label once across all
  tables, the first row winning (`RegistTextData` 0xB8926C, keyed by the label's hash).
  - `affectText_JP` has `AFFECT_HP_RATEADD_TEXT`, `AFFECT_ATK_RATEADD_TEXT` and
    `AFFECT_DEF_RATEADD_TEXT` = `<STAT>+{decimal_Param0}％`. The earlier note "no label says just
    ATK +x%" was wrong: their text holds no Japanese script (`％` is not in the detector's
    ranges), so they are absent from the translation export, and they need no English.
    `BASE_AFFECT_TEXT_2/4/6` are the same strings.
  - It also has `DTAF_<id>_TEXT` = `{Prvs}{BaseAffect}` for 16 of our 21 ability affect ids,
    the plain HP/ATK/DEF ones among them (our ids follow the client's naming).
    `DTAF_DYING_*` and `DTAF_FULL_HP_*` do not exist. That composite is probably the original
    wiring (M). We do not use it. In Japanese it gives the same text as the single label
    (`PRVS_TEXT_3` + `BASE_AFFECT_TEXT_4` equals `AFFECT_ALONE_ATK_RATEADD_TEXT`). In English, our
    `PRVS_TEXT_<n>` ends in a comma with no space before the stat ("…remains,ATK+5.0％").
  - Every other affect has an `AFFECT_[COND_]STAT_OP_TEXT` label of the same meaning, already in
    English:
    - `ALONE`: "When only 1 team member remains, …";
    - `FULL`: "At full HP, …";
    - `FNT`: "When near death, …";
    - the per-type `BRAVE/INTELLI/TENACITY/UNITY/MAVERICK` damage lines;
    - damage dealt and taken;
    - critical damage, `{mag_Param0}x`.
  - The condition labels match the conditions. In the Japanese source, the `AFFECT_FNT_*` texts
    begin with exactly `PRVS_TEXT_2` (DYING), `AFFECT_FULL_*` with `PRVS_TEXT_7` (FULL_HP), and
    `AFFECT_ALONE_*` with `PRVS_TEXT_3` (ALONE).
- **What we send (ours).**
  - `affect_table`: each ability affect `DTAF_X` names `AFFECT_X_TEXT`, with `DYING` → `FNT` and
    `FULL_HP` → `FULL`. Every one is a real label (`real_l10n` checks them against the player's
    text.jp), so no rows are added to text.jp.
  - `affect_text_rule_table`: one `baseAffectRuleList` row per BaseAffectType we use, with slot 0
    `Combine`. For the two `*_STYPE_*` bases, slot 1 (the StatueType) is `Solo`, so different
    types never merge. There are no condition rows. The rules are ours: there is no source.
    `Combine` sums the same line across Minds, as the battle sums the affects.
  - Lines now show, e.g. "ATK+5.0％" for a ★3 ATK Mind at Lv1 (50 per mille). The plain stat
    labels have no Japanese, so they look the same in both languages. A leader ability gives one
    line per effect, e.g. "When only 1 team member remains, ATK +x%", and a crit line of 200 per
    mille reads "Critical damage dealt 1.20x".
- **Unverified (M).**
  - The text on a device: the font's `％`, line wrapping in the Mind box, and whether the support
    and pause screens print the same lines.
  - Whether `StatueInfoUIController` prints the leader ability through these lines or through its
    `ABI_LEAD_TEXT_*` description.
  - The original rules (our `Combine` and `Solo` choice).
