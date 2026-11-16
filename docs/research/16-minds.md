# 16 — Mind equipment (Build 21) and abilities (Build 22)

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

- **Not yet:** Mind levelling (`/mind/strengthening*`, in the Build scene).

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
  row's `affectTextId` with `MindInfo.level`. Our affect rows have `affectTextId ""` (no label
  says just "ATK +x%"), so that line is blank for now.
- Abilities apply in slots 1–4 (7–8 too, which stay locked offline).
