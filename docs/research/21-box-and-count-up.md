# 21 — Box and Count Up summons (Build 22)

Read in the 3.12.0 client: H = read in the disassembly, M = inferred.

## 1. Box summons (H)

- A banner is a box when `boxFlag == 1`.
- `boxResetEnable`: 1 shows an enabled Reset button, 2 a disabled one; any other value hides it.
- `boxRemain` gates the play buttons: at least 1 for a single pull, at least 10 for the 10x.
  `boxResetRemain`/`boxResetLimit` are never read.
- `/gasha/get_box_gasha {gashaId}` → `{html}`: the "Box Status" button shows this HTML in a web
  view (never null).
- `/gasha/reset_box_gasha {gashaId}` → `{remain}`. On success the client asks for gasha_list
  again, so the list must show the refilled box. 51310–51312 or 10103 show "Cannot be reset".
- Box pulls use the normal `/gasha/gasha_play`.

## 2. Count Up summons (H)

- Count Up is the "step" system: `stepFlg == 1`. `stepCount` (> 0) is shown as the round
  number; `stepBonusMulti` shows as "+N" on the 10x button; `appealTextId` (never null in step
  mode) is a label shown as a pop (`GASHA_STEP_01` "1 ★5 Guaranteed!").
- The game's series: gashaIds 350010–350018, one per round 1–5 (labels "Statue Count-Up Summon:
  Round N", banner bundles in the player's files).
- **Round hand-over:** the client remembers the played gashaId and its `stepGroup`, and later
  re-selects a banner with that gashaId or with `stepFlg 1` and the same group. So the server
  swaps in the next round's gashaId under the same group and the screen stays on the series.
- There is no Count Up route and no result field for the ticket (`GashaResult.itemInfo` is never
  read), so the ticket goes to the Message Box (also what the research pack says).
- A ticket shows as a ticket button when the banner's `ticketId` matches an item the player holds
  in gasha_list's `itemList` (never null); that pull is `playType 4`.

## 3. Server (Build 22)

- **Box:** "Statue Box Summon" (600370). A box holds 50 statues: 1 five-star, 9 four-star and
  40 three-star (ours). Pulls take from what is left; Reset refills it at any time; Box Status
  lists what is left.
- **Count Up:** the current round's banner, under one group.
  - A round ends after 20/30/50/50/50 pulls (research pack tiers). Its reward is one Golden
    Summon Ticket (item 282, ours) in the Message Box, and the next round shows; after round 5
    the series starts again.
  - Round 5's 10x pull ends in a five-star, with the game's "1 ★5 Guaranteed!" pop (ours).
  - The ticket button draws one statue of four stars or better (ours). Ticket pulls do not count
    for the round.
- Everything is still free; draws pick from owned statues as before.
