# 16 — Mind equipment (Build 21)

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
  - Ours are empty `{}` for now (skipped safely), so equipped Minds show but do nothing in battle
    yet.
  - Next: `DT_MIND_<id>` rows in `dtbattle_ability_data_table`, using the existing rate-add
    affects; the labels `ABI_MIND_NAME_<id>` exist.
- **Not yet:** Mind levelling (`/mind/strengthening*`, in the Build scene).
