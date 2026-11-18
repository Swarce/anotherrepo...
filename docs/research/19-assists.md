# 19 — Assists (Build 22)

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
- **Abilities** need `assist_ability_condition_table` rows keyed by statueId (ours is empty).
  Trap: the client finds the index in `dtAssistAbilityList` and reads that index from
  `assistAbilityList`, so both lists must have the same length and order (ours are both `[]`).
- **Assist character:** in tactical battle the leader seat's seq-1 assist joins as the team's
  assist character, using `dtAssistSkillInfo` as its skill; skipped while that is empty (ours).

## 4. Server (Build 22)

- `Team.assists`: per seat, (seq, uid), the same for both battle types of a team, like Minds.
- Every TeamInfo sends the seat's assists as full statue records plus `seq`. A statue that later
  becomes a main member of the team is not shown as an assist.
- equip_multiple_assist applies the diff in order, moves an assist off the team's other seats, and
  refuses a main member, a seq outside 1–3 and an unknown statue (51702/51709).
- **Bonus constants (ours):** base 10%, plus up to 10/10/15/20% for rarity 3/4/5/6 at 10 boosts,
  ×1.2 when the assist shares the seat statue's type. So a fully boosted 5-star assist adds 30% of
  its own stats.
- Not yet: assist abilities, assist skills (the assist character), auto formation.
