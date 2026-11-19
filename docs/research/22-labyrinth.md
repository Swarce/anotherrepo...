# 22 — Labyrinth of the Mind (dungeon)

Read in the 3.12.0 client: H = read in the disassembly, M = inferred. "Pack" = the owner's
research pack (`v2.labyrinth.*`, private). Code: `server/services/src/labyrinth.rs` (content,
state, rules) and `server/services/src/services/dungeon.rs` (routes, battle hand-over).

## 1. Client facts

### 1.1 Entry (H)

- The Quest Select tile is locked while `get_bizarre_select.dungeonLockFlg == 1`. Tapping it
  resets the dungeon manager and opens scene 1004 (BizarreDungeonTopUI).
- The top screen always asks `/user/user_info` first: Souls and Mentality come from
  `userInfo.soul` / `userInfo.mentality`.
- Then, by screen state:
  - Dungeon select: `/dungeon/dungeon_list`. If any dungeon has `floorNum > floorMax` it is
    cleared, and the client asks `/dungeon/earn_reward` and shows the Clear panel.
  - Room select after a battle: `dungeon_list`, then `stage_list`.
  - Escape and Clear: `earn_reward`.
- There is no "start run" route. Challenge is client-only:
  - another dungeon in progress → DUNGEON_DIALOG_002 (blocked);
  - the same one → straight to the rooms;
  - none → confirm DUNGEON_DIALOG_000.
  So a run starts at `stage_list` (M).
- The dungeon in progress is the **first** DungeonInfo with `floorNum >= 0`; -1 = not started.

### 1.2 Routes (H)

| Route | Request | Response |
|---|---|---|
| `/dungeon/dungeon_list` | `{}` | `dungeonList` List<DungeonInfo> |
| `/dungeon/stage_list` | `{dungeonId}` | `stageList` List<DungeonStageInfo>, `floorNum`, `teamInfoList` List<TeamInfo> |
| `/dungeon/tarot_select` | `{stageId, dungeonId, floorNum, teamNo}` | `selectTarotList`, `usedTarotList` (List<TarotInfo>) |
| `/dungeon/tarot_flip` | `{}` | `selectTarotList` |
| `/dungeon/earn_reward` | `{}` | `rewardItemList` List<ItemSetInfo> |
| `/dungeon/reward_check` | `{dungeonId}` | `rewardItemList` List<DungeonRewardInfo> |
| `/bizarre/bizarre_start` | adds `tarotInfo {tarotId, tarotPosition}`, `dungeonStartFlg` 1, `dungeonId`, `floorNum`, `stageType` 1 | the usual response |
| `/bizarre/bizarre_end` | adds `dungeonStartFlg`, `dungeonId`, `floorNum` | `dungeonClearFlag`, `dungeonGameoverFlag` |

`tarot_select` is sent only when a dungeon, a room, a floor and a team are all selected;
otherwise nothing is sent and nothing happens.

### 1.3 Shapes (H)

- **DungeonInfo**: `dungeonId`, `name` (label), `summary` (label), `floorMax`, `floorNum`
  (-1 none), `floorClearNum` (shown), `difficulty` (E0 … SSS+10), `imgNo`
  (`bizarre_dungeon_{imgNo:D3}.png`, 000–004 exist), `stageType` (not read).
- **DungeonStageInfo**: `stageInfo` (a full StageInfo), `mapGimmickFlg`.
  - The room banner reads the stage name, `areaMax`, `useMentality`, `specialRewardFlg` and
    `clearFlg`.
  - The info pane reads `enemyList`, `steadyItemSet`, `rewardItemSet` (while clearFlg ≠ 1),
    `reward3StarsItemSet`, `bossDropItemSet` and `bizarreEffectList`.
- **TarotInfo**: `tarotId`, `name` (label key, `TAROT_NAME_000`), `description` (label,
  `TAROT_DESCRIPTION_000` / `_000_REV`), `abilityInfo` (an **AbilityInfo**), `position`
  (0 upright, 1 reversed), `imgNo` (card sprite 0–21).
- **DungeonRewardInfo**: `rewardItemSet` (ItemSetInfo), `floorNum`, `reward_title`,
  `reward_text`. A row shows as achieved while the dungeon's `floorNum` is past it.
- **GameConst**: `DUNGEON_SOUL_MAX` 3, `DUNGEON_MENTARITY_MAX` 300.
  - `DUNGEON_CARD_REV_USED_MENTARITY` is 30, but it only feeds a text placeholder.
  - The shipped texts (DUNGEON_SCENE_CARD_001, DUNGEON_DIALOG_005) say **20**.
- **Errors**: 52101 another dungeon started; 52102 not enough Mentality → DUNGEON_DIALOG_004
  (tarot_select and tarot_flip); 10103 → ERROR_TEXT_00600001.

### 1.4 Run flow (H)

1. `stage_list` lists the current floor's rooms. The team defaults to the `mainFlg 1` entry.
2. Sortie → DUNGEON_DIALOG_001 → `tarot_select` → card scene (1006). The scene shows up to 3
   offered cards and up to 22 used slots.
3. Reverse (optional) → DUNGEON_DIALOG_005 → `tarot_flip` → `user_info`.
4. Picking a card starts the battle with that card: `bizarre_start` with `dungeonStartFlg` 1.
   The battle type is the response's `chapterInfo.stageType`; a null chapterInfo throws.
5. After a win, the client reads the two `bizarre_end` flags:
   - clear 1 → Clear panel (`earn_reward`);
   - game over 1 → back to dungeon select;
   - neither → back to the rooms (`dungeon_list` + `stage_list`, next floor).
6. On a defeat or retire, the client takes a Soul from the `bizarre_start` userInfo for its own
   display. At 0 it shows DUNGEON_DIALOG_006, then follows the same flags.
7. Escape: DUNGEON_DIALOG_003 → `earn_reward` → the retire panel lists `rewardItemList`.
8. Resumption: see §1.8 (built in Build 25).

### 1.5 Tarot ability path (H for the code path, M for the effect)

- The tactical battle takes the selected card's `abilityInfo` (OnSceneLoadCallback 0xEBCBE0).
  `SetupCharacterInfo` → `CharacterStatus.SetStatueInfo` 0xEFC570 → `ApplyAbility(tarot,
  source 12)` applies it to every team member.
- It is an `AbilityInfo`, not a `DtAbilityInfo`. Its `IsValid` first calls
  `SetupFromAbilityData` 0x205E7D8, which **returns at once when `isInit` is true**. Otherwise
  it replaces the parameters from the action-battle `AbilityDataTable`, which we do not ship.
- So we send `isInit: true` with inline parameters (`affectId`, `giveLimit`,
  `paramListLv1_0..7`, `paramListLv10_0..7`). The affects must exist in `affect_table`.
- Not yet seen on a device.

### 1.6 Crash traps

- `bizarre_start.chapterInfo` null → exception (H).
- The dungeon must stay in `dungeon_list`. After a battle the client re-selects it by id, then
  reads `.floorMax` (M).
- `selectTarotList` null or empty → error text, then back to My Page (H).
- `teamInfoList` empty → `tarot_select` is silently never sent (H).
- `imgNo` outside 0–21 → no sprite (M). More than 22 used cards are ignored (H).
- ItemSetInfo lists must be arrays (M).

### 1.7 Labels in the client

`DUNGEON_NAME_000..005`, `DUNGEON_MES_*`, `DUNGEON_SUMMARY_*`,
`DUNGEON_STAGE_NAME_000..200`, and room names `STAGE_DU01_<floor>_<room>`.
- Room labels exist for floors 1–20: 3–6 rooms a floor, one on floors 10 and 20.
- Dungeon DUNGEON_NAME_002 is the mansion those rooms belong to.

Card labels:
- `TAROT_NAME_000..021`; `TAROT_DESCRIPTION_nnn` and `_REV`.
  - Strength has no upright text and Tower no reversed text.
- Ability names `DT_ABI_TAROT_UP/RV_NAME_nnn`.

### 1.8 Resumption (H, Build 25)

- My Page asks `/bizarre/bizarre_resumption` on every load (`MypageUIController` StartLoad
  0xC39368). Status 10000 opens a Yes/Cancel dialog: Client_Mes_0009 ("Suspended game data was
  found…") when `dungeonId` is empty, Client_Mes_0010 (the Mind Labyrinth one, "If you quit
  during Tarot Card selection, you will return to Tarot Card selection") when it is set
  (0xC3A1C0). Any other status (we send 50301, E_BIZALLE_NOT_START) shows nothing.
- Yes, by `returnDestination`:
  - 0 TarotSelect: `BizarreDungeonManager.SetResumptionCard(selectTarotList, usedTarotList,
    dungeonId, stageInfo, floorNum, teamInfo)` 0xD4936C, then the card scene (1006). Picking a
    card sends the usual `bizarre_start`.
  - 1 Battle: `BizarreQuestSceneManager` takes userInfo, chapterInfo, stageInfo, teamInfo,
    areaList, continueCount, both Tarot lists, dungeonId, floorNum and `seed`; with a dungeonId
    it sets `dungeonStartFlg` 1 and the dungeon manager's return flag, then
    `StartBizarreQuest`. The battle's card is `selectTarotList[0]` (`GetSelectTarotInfo`
    0xE03604, read by the battle's `OnSceneLoadCallback`).
- Cancel: `/bizarre/destruction {stageType}` (`DestroyBizarreResumption` 0xC37A48, the route's
  only caller).
- `BizarreResumptionResponse` = the start reply's fields plus `returnDestination`,
  `selectTarotList`, `usedTarotList`, `isRaidDelete` (no `tarotInfo`).

## 2. Research pack facts

- 20 floors. Room costs seen: 22–35 Mentality, 1–5 areas.
- Mentality restores daily at 04:00 JST up to 300. The earlier guide says +100 a day.
- Floor rewards are earned as floors are cleared but paid only on a full clear or an escape.
- A defeat or retire costs a Soul. Losing all Souls loses the pending rewards.
- Escape pays the eligible rewards and restores the Souls to three.
- 22 Tarot identities, three offered per room, one picked.
  - A picked card is unavailable until the deck resets: after all 22, an escape, or losing all
    Souls.
  - Flipping reverses all three and costs 20.
- The pack's upright values are 2018 action-era values; they differ from the 3.12.0 texts.

## 3. Server

**State** (`Progress.dungeon`, `#[serde(default)]`; older saves start full):
- Mentality spent today and its game day.
- The run: dungeon, floor, Souls lost, the room paid for, the team, the offer, the chosen card,
  the used cards and the floors earned.
- The best floor and the full-clear count.

**Dungeons (Build 25, ours):** three grades of one mansion, named by the game's
`CHAPTER_NAME_D_01..03` ("Beginner/Intermediate/Advanced: The Hidden and Sealed Mansion"):

| id | Name | Summary | Difficulty | Enemy levels | Floor rewards |
|---|---|---|---|---|---|
| DU01 | CHAPTER_NAME_D_01 | DUNGEON_MES_001 | C (2) | the story stage's | ×1 |
| DU02 | CHAPTER_NAME_D_02 | DUNGEON_MES_002 | A (4) | +15 (≤ 99) | ×2 |
| DU03 | CHAPTER_NAME_D_03 | DUNGEON_MES_003 | SS (7) | +30 (≤ 99) | ×3 |

- The same rooms, costs and picture in all three. Room ids are `STAGE_DUnn_ff_rr`; the banner
  shows the Beginner room labels (`STAGE_DU01_ff_rr`, the only room labels the client has).
- One run at a time (52101 for another dungeon, as the client itself warns). The best floor is
  kept per dungeon (`best_floors`; saves before Build 25 keep theirs as the Beginner one's).
- The other `DUNGEON_NAME_000..005` names (castle, colosseum, town, two towers) stay unused: no
  rooms, floor plans or labels belong to them.

**Routes:**
- `get_bizarre_select`: `dungeonLockFlg` 0.
- `user_info`: `soul` = 3 − Souls lost in the run (3 without one); `mentality` = current.
- `dungeon_list`: the three dungeons.
  - Fields: name, summary and difficulty as above, `floorMax` 20, `imgNo` 2, `floorClearNum` =
    best floor cleared there.
  - `floorNum` is -1 with no run, the floor otherwise, and 21 once floor 20 is cleared.
- `stage_list {dungeonId}`: starts the run on floor 1 if none (an empty id means the run's
  dungeon, else the Beginner one). Returns the floor's rooms and every tactical team.
- `tarot_select`: the room must be on the current floor (else 10103).
  - Charges `useMentality` (52102 when short), then deals up to 3 cards not used since the
    deck reset (all 22 used → the deck resets first). Cards are dealt upright.
  - A resent request for the room already paid returns the same offer free.
- `tarot_flip`: needs an offer (10103). Charges 20 (52102) and reverses all three.
- `bizarre_start` for a room id: the room must be the one paid for (10103).
  - Takes the card off the offer; it is used until the deck resets.
  - Sends the room's StageInfo, the stage's areas and the stage's chapterInfo (stageType 1,
    never null), the full TarotInfo, `dungeonId` and `floorNum`.
- `bizarre_end` for a room id: once per started battle.
  - Win: the floor's reward joins the pending list and the run moves down a floor;
    `dungeonClearFlag` 1 after floor 20. The stage's rank EXP goes to the player and the
    team; no gold.
  - Loss or retire (any result ≠ 0): one Soul. At 0 the run and its pending rewards are gone;
    `dungeonGameoverFlag` 1.
  - A resent end changes nothing.
- `earn_reward` (Escape and Clear): ends the run.
  - Every pending floor reward goes to the Message Box; `rewardItemList` lists them (one
    ItemSetInfo each).
  - Souls and the deck start fresh. A second call pays nothing.
- `reward_check {dungeonId}`: one row per floor, with that dungeon's rewards.
- `bizarre_resumption` (Build 25, §1.8):
  - a room battle started and not ended → Battle (1), with the card it started with as
    `selectTarotList[0]`, the run's used cards, the room's stage, chapter and areas and the
    team that started it;
  - a room paid for whose cards are still on offer → TarotSelect (0), with the offer;
  - a story, daily or event battle started and not ended → Battle with `dungeonId` "" (checked
    first; services/bizarre.rs);
  - otherwise 50301.
- `/bizarre/destruction` (No): a story battle is dropped (nothing paid); a room battle counts
  as retired (a Soul, ours: the pack says a retire costs one); a Tarot pick is no longer offered
  (`declined`), while the room and its cards stay paid for.

**Content (ours):**
- Rooms reuse the 240 tactical story stages, sorted by enemy level. Each floor takes its
  twentieth of the list.
- Three rooms a floor and one on floors 10 and 20 (the hardest stage of the slice).
- A room keeps the stage's enemies, map and single area (`areaMax` 1). Its id and name label
  are `STAGE_DU01_ff_rr`.
- Costs: 22 + (floor − 1) × 10 / 19, plus 3 per room down the list, capped at 35. The single
  rooms of floors 10 and 20 cost 30 and 35.
- Floor rewards, one item each:
  - floors 5/10/15/20: 100/200/300/400 Memory Rings;
  - other odd floors: 2,000 + 1,000 × floor gold;
  - other even floors: trophies (6112 ×5, 6113 ×5, 6114 ×3, 6115 ×2 by depth).
- A room's `rewardItemSet` previews its floor's reward; `steadyItemSet` is empty.

**Tarot effects**: the client's 3.12.0 card texts, where an affect in our `affect_table` does the
same thing; otherwise `abilityInfo: {}` (the card still shows, without effect).

| Card | Upright | Reversed |
|---|---|---|
| 001 Magician | vs Wisdom +100% (INTELLI_STYPE_DMG) | — |
| 006 Lovers | vs Unity +100% (UNITY_STYPE_DMG) | — |
| 008 Justice | vs Spirit +100% (TENACITY_STYPE_DMG) | — |
| 009 Hermit | vs Courage +100% (BRAVE_STYPE_DMG) | taken from Courage +100% (BRAVE_STYPE_VCT_DAM) |
| 011 Strength | ATK +50% (pack value; no client text) | ATK −50% |
| 012 Hanged Man | taken from Solitude +100% (MAVERICK_STYPE_VCT_DAM) | vs Solitude +100% (MAVERICK_STYPE_DMG) |
| 013 Death | damage taken +50% | damage taken −50% |
| 016 Tower | DEF −50% | DEF +50% (ours; no client text) |
| 020 Judgement | damage dealt +50% | damage dealt −50% |
| 021 World | crit damage +30% (CRT_DMG) | crit damage taken +30% (VCT_CRT_DMG) |

Evasion, speed, accuracy, crit rate, the attribute targets (Vampires, Pillar Men, Major
Arcana, Stand users, Hamon users, Joestar family), the HP-threshold cards, and the reversed
Wisdom/Unity/Spirit cards (no matching affect) send `{}`.

**Mentality**: cap 300 (client). It refills **fully** at the 04:00 JST day roll
(`missions::game_day`). This is ours: the pack's earlier guide says +100 a day.

## 4. What is ours

- One dungeon and its labels.
- Rooms built from story stages, room counts and costs.
- Rewards, and their delivery through the Message Box.
- The full daily Mentality refill.
- The Tarot-to-affect mapping, Strength upright and Tower reversed.
- Free resent `tarot_select`; unlimited flips at 20 each.
- The three grades, their level steps and reward multipliers; the room labels shared by all three.
- A declined room battle costs a Soul; a declined Tarot pick keeps the room paid for.
- Rank EXP for labyrinth wins; `floorClearNum` as the best floor.
- The deck also resets on a full clear.

## 5. Left out / unverified

- Not yet run on a device. In particular, the tarot ability path (`isInit` inline parameters)
  and the room banner's use of `STAGE_DU01_*` labels are M.
- Multi-area rooms: our stages have one area. Map gimmicks are off (`mapGimmickFlg` 0).
- The game's other dungeon names (DUNGEON_NAME_000/001/003–005) are unused (no rooms or labels
  for them).
- Resumption has not run on a device: the dialog, the card scene from Resume, and a resumed
  battle's Tarot (taken from `selectTarotList[0]`, H for the read, M for the whole flow).
- Continues inside a labyrinth battle stay free.
- `/bizarre/destruction` is sent only by the resume dialog's Cancel (H, Build 25); a retire inside
  the battle goes through `bizarre_end`.
