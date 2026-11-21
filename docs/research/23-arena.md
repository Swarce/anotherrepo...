# 23 — Arena (PvP against AI-controlled teams)

Read in the 3.12.0 client: H = read in the disassembly, M = inferred. Research pack notes (the
owner's private pack, dated guides and screenshots) are cited by their id. Everything marked
"ours" is a choice of this server. Server code: `server/services/src/services/arena.rs`.

## 1. Client facts

### 1.1 Entry (H)

- My Page takes the first `/info/event_list` entry with `eventType` 6 (Pvp) or 7 (PvpAuto). With
  none, the Arena button stays as it is (no Arena). With one, it parses `eventEndDate`,
  `eventFinishedDate` and `userBattleInfo.fieverEndTime` as dates: before the end date the
  button is "On", before the finished date "Finish" (results only). `userBattleInfo: {}` leaves
  `fieverEndTime` null, which the date parser is expected to throw on (M).
- A tap stores the entry and loads scene 1600 (`VSTopP2UI`, `PvP2HomeController`). The campaign
  id goes back as `eventCampaignId` in every pvp request. Arena campaign ids have name labels
  (`EQ_510002`..`EQ_510027`).

### 1.2 Two rule sets (H for scene registration, M for "dead")

- The 2017 home (`PvPHomeController`, scene `VSTopUI`, action VS battle) is not registered in
  the scene table, so 3.12.0 cannot reach it.
- The 2019 home (`PvP2HomeController`, scene 1600) is the live one: per-team durability with a
  rest interval, `recover_hp_pvp`, opponent shuffle, Crazy Diamond points, team-power and
  win-streak scoring from `get_pvp_config`, and a tactical (DT) battle.

### 1.3 Routes and shapes (H)

| Route | Request | Response |
|---|---|---|
| `/pvp/get_pvp_config` | `eventCampaignId` | `teamPowerBasePointList [{Rank (string), addPoint}]`, `winningStreakPointRateList [{winningStreakNum, pointRate}]` |
| `/event/point_reward_list` | `eventCampaignId` | `userEventPoint`, `clanEventPoint`, `userPointRewardList`, `clanPointRewardList` (EventPointInfo `{rewardTitle, rewardTitleReplace, rewardText, rewardTextReplace, necessaryEventPoint, itemSetInfo, getFlg}`) |
| `/item/heal_item_list` | `type` (2 = BP) | `itemList` |
| `/pvp/home_acqusition_check` | `eventCampaignId` | `result` (polled) |
| `/pvp/home` | `eventCampaignId, pollingFlg` | `endDate, finishDate, pvpTeamList, pvpUserInfo, todaysPoint, recoveryTimes, selectedTeamId, pollingTimes, damageInfo [{type, enemyName, damage, teamNo}], eventCampaignList, forceDefeatFlg, battleType` (polled) |
| `/pvp/enemy_player_list` | `eventCampaignId, teamNo, reChoiceFlg` (1 = shuffle) | `pvpTeamList` (PvpOpponentTeamInfo), `userInfo` |
| `/pvp/pvp_start` | `eventCampaignId, teamNum, enemyuId, enemyTeamNum` | `userInfo, pvpTeamInfo, pvpEnemyTeamInfo, pvpEnemyInfo, winningStreak, battleModeInfo, userRank, winningStreakAtkRate, winningStreakDefRate, seed, dayNight` |
| `/pvp/pvp_end` | `result` (0 win, 1 loss), `defeatedEnemyList [{statueId, count}], teamNo, battleTime, battleTurn, myTeamInfoList [{hp}], myTeamPower, enemyTeamPower` | `userInfo, pvpTeamInfo, getRankingPoint, enemyuId, winningStreak, feverRate, feverGauge, getFeverRankingPoint, totalWin, totalLose, totalHpBonusPoint, restStatueBonusPoint, eventRewardList [{itemSetInfo, necessaryEventPoint, rewardTitle, rewardTitleReplace}]` |
| `/pvp/recover_hp_pvp` | `typeId` (0 Diamond Coins, 1 item), `itemId, eventCampaignId, teamNum` | `userInfo, itemList` |
| `/pvp/fever_mode` | `eventCampaignId` | `userInfo, feverEndDate, feverGauge, feverAddTime, feverAddFlag, feverRate` |
| `/pvp/team_info_pvp` | — | `pvpTeamList, mindSeqInfo` |
| `/pvp/set_team_all_pvp` | `teamList [{teamNum, leaderStatueInfo, memberLeftStatueInfo, memberRightStatueInfo}]` (EquipStatueSetting `{statueuId, equipMindList [{equipSeq, minduId}], assistStatueList [{equipSeq, statueuId}]}`) | `pvpTeamList` |
| `/pvp/set_team_pvp` | `teamNum, leaderstatueuId, memberLeftStatueuId, memberRightStatueuId` | `pvpTeamInfo` (no caller found) |
| `/pvp/statue_select_pvp`, `/pvp/leader_change_pvp`, `/pvp/equip_mind_pvp`, `/pvp/equip_multiple_minds_pvp`, `/pvp/equip_multiple_assist_pvp`, `/pvp/auto_set_and_equip` | as the `/team/*` routes | `pvpTeamInfo` |

- **PvpTeamInfo** = TeamInfo + `hp` (0..10000, `PVP_TEAM_HP_MAX`), `recoveryTime` (a date the
  client counts down to) and `fatiguCount` (the team's BP cost, shown and charged as such).
- **PvpOpponentTeamInfo** = `pvpTeamInfo` (three statues), `enemyName, enemyRank,
  enemyEventPoint, winPoints, enemyuId`. The banner shows `(winPoints + teamPowerBase[rank of
  the team].addPoint) x (1 + streakRate / K)`, K a float constant (100 is M).
- **PvpUserInfo**: `rankingPoint, feverEndDate, feverGauge` (max 10000), `feverAddTime,
  feverAddFlag, feverRate, rank, winningStreak, nextEventPoint, nextEventItem` (ItemSetInfo),
  `crazyDiamondPoint` (10000 shows the Crazy Diamond button), `crazyDiamondNotifyFlg`.
- **BattleModeInfo**: `battleType` (PvpBattleType Normal 0, Pvp 1, PvpAuto 2; 2 turns on the AI
  arena mode, which changes the damage calculation), `battleTimelimit`, `battleTurnlimit`.
- GameConst: BP full 5, fever gauge max 10000, `PVP_TIME_LIMIT_SEC` 120, `PVP_TEAM_HP_MAX`
  10000, recovery item types AP 1 / BP 2 / all 3. EventCampaignType Pvp 6, PvpAuto 7.
- The home's recovery list always holds a Diamond Coin entry (item 0 labels, the player's coin
  count) next to any items from `heal_item_list`.
- Status codes: 52002 enemy not found, 52003 fever lack, 52004 not started, 52005 team fatigue,
  52006 no leader, 52008 opponent empty (locks that opponent), 52010 recovery material lack,
  52015 team interval (handled by `enemy_player_list`), 52019 already started (handled by
  `home`); 51901 event not open (`pvp_start` shows the end dialog).

### 1.4 Battle (H)

- `pvp_start` success: the client stores the team HPs and the BP cost and starts the tactical
  battle itself: map `map02_under_colos01_PVP` (area `AREA_map02_under_colos01_PVP`), formation
  **`PatternPVP`** (an `area_position_pattern_table` row name), seed and dayNight from the
  response, enemy team `pvpEnemyTeamInfo`, turn limit `battleModeInfo.battleTurnlimit`.
- Empty `AiId`s fall back to the client's arena AI (`ArenaPlayer`).
- The result screen shows `getRankingPoint`, `getFeverRankingPoint`, `totalHpBonusPoint` and
  `restStatueBonusPoint`; `eventRewardList` is null-checked.

### 1.5 Crash traps (H unless marked)

- `userBattleInfo.fieverEndTime` null (M: throws in the date parser).
- `/pvp/home`: `pvpUserInfo` null (null dereference), `eventCampaignList` iterated; a null
  `pvpTeamList` becomes empty but then no team can fight.
- Opponent `teamOrder` must hold three StatueSettings (the banner reads all three).
- `get_pvp_config.teamPowerBasePointList` is indexed by team rank (out of range scores 0).
- `pvp_start.battleModeInfo` null (null dereference); `userInfo` must have a `username`.
- Battle units need `modelDataList` and `stEffRateInfo` (shared with the story battle).

### 1.6 Research pack (dated rules)

- `v2.arena.ai_opponent` (2018 guide): opponents were other players' AI-controlled teams.
- `arena.teams` (2019): a separate roster of up to five teams of three; statues, Minds and assists
  may not be reused across it.
- `arena.durability_2019`: losses empty a durability bar; at zero a team rests thirty minutes.
  (`arena.durability_2017` says one hour: a different revision.)
- `arena.fever`: Fever after four battles, three times the points. `v2.arena.early_fever`: ten
  minutes.
- `arena.bp_cost`, `arena.edit_gate`, `v2.arena.early_bp` / `early_cost`: BP 5, team costs that
  rise after use and reset when BP is full, no edits until the cost is one.
- `arena.scoring`, `arena.survivor_bonus`: opponent power, streak and surviving members add
  points; the formula is unknown.
- `v2.arena.early_failure`: timeouts and force-closes counted as defeats.

## 2. Server (ours unless marked)

- **Campaign:** `/info/event_list` lists one PvpAuto entry, campaign `510027`, open and finished
  dates in 2099; `fieverEndTime` is the running Fever's end, else a date in 2000. `/pvp/home`
  sends the same far dates, so the Arena never closes.
- **Roster:** `Collection.arena_teams`, five tactical teams. Existing saves get them on their
  next login; team 1 starts with the tactical main team's members. The formation routes share
  the `/team/*` logic (`team::Roster::Arena`). Single-team edits behave like the story ones;
  `set_team_all_pvp` (whose reply carries the whole roster) and the auto formation keep a statue
  to one Arena team. Arena Minds count as equipped (`useFlg`, fodder protection).
- **BP and cost:** BP stays 5 of 5 (as AP is never spent offline) and every team costs 1, so the
  edit gate never closes.
- **Opponents:** three AI teams of three statues of different characters, kept until a battle or
  a shuffle (shuffles are free). They scale on the chosen team's average level and rarity and on
  the season points: easier (level -3, rarity -1, 10 base points), even (15) and harder (level +5,
  rarity +1, 25). Level grows by 1 per 100 points, rarity by 1 per 2000 (3..6 stars). No Minds or
  assists. Names are English handles with a number.
- **Battle:** `pvp_start` checks the team (leader, not resting) and the opponent, records the
  pending battle and sends battleType 2, 120 s (client constant) and 20 turns. `pvp_end` settles
  that battle once; without one it pays nothing (a resent request). A missing `result` is a loss.
- **Points (a win):** the client's banner formula with our tables: `(winPoints + addPoint[rank])
  x (100 + streakRate) / 100`, the rank computed like the client's team rank (score = hp/1000 +
  atk/2000 + def/2000 + 0.003 x Mind capacity, our team_rank_table). addPoint E..SSS+: 5, 8, 10,
  12, 15, 18, 21, 24, 27, 30, 35. Streak rates: 1 win 0 %, 2 10 %, 3 20 %, 4 30 %, 5+ 50 %. HP
  bonus: up to 20 points for the share of HP left (`myTeamInfoList` against the team's HP);
  5 points per statue standing. `getRankingPoint` is the battle points without the bonuses.
- **A loss:** no points lost, the streak ends, the team loses 2500 durability (four losses empty
  it). At zero it rests 30 minutes (research pack) and comes back full; `recoveryTime` is the end
  of the rest. `recover_hp_pvp` refills a team (or all teams when `teamNum` names none) for free.
  `heal_item_list` is empty: our item catalog records no recovery items.
- **Fever:** each battle outside Fever adds a quarter of the gauge (four battles, research pack);
  `fever_mode` starts ten minutes (research pack) of triple points (`feverRate` 300, our unit);
  `getFeverRankingPoint` is the extra.
- **Ranks and rewards:** `pvpUserInfo.rank` is a ranking position against a virtual field
  (position 1 at 10000 points, one place per 10 points short). Point rewards each season (100,
  300, 600, 1000, 1500, 2500, 4000, 6000 points: Diamond Coins, Memory Rings, gold, Fight Coins)
  go to the Message Box when reached and are listed in `eventRewardList` and in
  `/event/point_reward_list` (`getFlg`). Seasons are weeks turning on Monday at 04:00 JST; a
  season with battles sends a reward by final position (top 10: 500 Diamond Coins, 100: 300,
  500: 150, else 50) and points, streak, Fever and rewards start again.
- **Not built:** defense (nobody attacks offline: `damageInfo` empty), the Crazy Diamond button
  (point 0), `recoveryTimes` (0; its meaning is unclear), `forceDefeatFlg` (false; an abandoned
  battle is dropped without a penalty), streak attack/defense rates (0), clan points.
- **Formation pattern:** `area_position_pattern_table` gets a `PatternPVP` row with the story
  positions.

To be confirmed on a device: the whole flow. Expected but unverified: the Arena button on My Page,
the home, the opponent banners, the battle and the result screen.
