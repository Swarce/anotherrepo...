# 23 — Arena (PvP against AI-controlled teams)

Read in the 3.12.0 client: H = read in the disassembly, M = inferred. Research pack notes (the
owner's private pack, dated guides and screenshots) are cited by their id. Everything marked
"ours" is a choice of this server. Server code: `server/services/src/services/arena.rs` and
`ranking.rs` (Build 24: rankings, defence, Crazy Diamond, abandoned battles).

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
| `/pvp/recover_hp_pvp` | `typeId` = hpRecoveryMethodType (0 Crazy Diamond, 1 Diamond Coins, 2 item; Build 24 correction), `itemId, eventCampaignId, teamNum` (the team's `teamNum`) | `userInfo, itemList` |
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

### 1.7 Home polling and its notices (H, Build 24)

- `pollingTimes` is the number of seconds between polls (`pollingInterval`, compared with the
  frame time). Every poll locks the UI's buttons until its three requests
  (`home_acqusition_check`, `/user/user_info`, `/pvp/home`) return, and at 0 the home starts a
  poll on every frame. Build 23 sent 0, which keeps the home locked (M: part of the device hang
  Build 24 fixed).
- `/pvp/home` reply, besides the shapes above: `endDate` and `finishDate` (parsed with DateTime.TryParse; a null `endDate` reads as a default date),
  `selectedTeamId`, `battleType` (2 shows the auto-battle marker), `eventCampaignList` (entries
  whose `eventCampaignId` equals the campaign fill the campaign panel; every id is compared, so
  none may be null). The request's `pollingFlg` is the client's "defence notice still pending"
  flag, not a polling mode.
- Notices, in this order (`NotifyExec`, texts `PVPHOME_000xx`):
  1. `forceDefeatFlg` true: "Connection was lost during battle. The previous battle counts as a
     loss." (00031).
  2. `recoveryTimes` = 4 after a battle, with at least two usable teams: "After 4 battles, the BP
     cost of all teams except the one used dropped by 1." (00029). `recoveryTimes` also lights a
     four-segment gauge (`SetBPRecoveryStatus`): it is the battle count of the BP cost recovery
     cycle.
  3. `pvpUserInfo.crazyDiamondNotifyFlg`: "Full Revive is now available." (00028).
  4. Defence (`damageInfo`, only on the poll that opens the home): one message per entry per
     team and the team banner's durability animation.

### 1.8 Defence (H)

- `damageInfo` is a list of DefenseInfo structs `{type, enemyName, damage, teamNo}`.
  `registDamageInfo` groups them by `teamNo - 1` (an index into `pvpTeamList`, so 1..5) and
  computes each team's total damage itself, as the drop from the durability it last showed to
  the new `hp`. The message per entry: `type` other than 0 "You lost the battle." (00050);
  type 0 with `damage` 0 "You were attacked by {enemyName} and drove them off." (00049); with
  damage "You were attacked by {enemyName} and were defeated." (00044).
- So the client takes simulated attacks as they are: lower the team's `hp` and list who
  attacked; an empty list is the clean state.

### 1.9 Crazy Diamond (H)

- `pvpUserInfo.crazyDiamondPoint` equal to `PVP_CRAZYDIAMOND_MAX` (10000) shows the Crazy Diamond
  button (`diamondButton`); below, it is hidden. The button asks "Use Full Revive? All Retired
  teams will be revived." (00027) and sends `/pvp/recover_hp_pvp` with `typeId` 0; "No teams are
  Retired." (00045) when none rests. How the gauge fills is server-side (unknown).

### 1.10 BP cost cycle (H)

- `fatiguCount` is the team's BP cost; the formation refuses to edit a team whose cost is above
  a value it holds (`IsPossibleFormationTeam`; M: the base cost of 1), and a resting team (`hp` 0 with `recoveryTime` ahead,
  `IsFormationInterval`). Costs rise with use and fall every four battles (the 00029 text);
  the rise itself is server-side.

### 1.11 Win-streak rates (H)

- `pvp_start`'s `winningStreakAtkRate` / `winningStreakDefRate` go to the tactical battle's
  correction parameters; in a PvP battle every unit of the player's team (TeamType Player) gets a
  tenth of each added to its ATK and DEF rates (`CharacterAffectGroup.Setup`,
  `CalcPvpWinningStreakParam`). M: the PvP check is `BizarreQuestSceneManager.CurrentBattleMode`
  1.

### 1.12 Rankings (H)

- The Arena home's ranking button stores the campaign id, the event type (PvpAuto 3 when the
  home's `battleType` is 2, else Pvp 2) and `pvpUserInfo.rankingPoint`, then adds the
  `RankingUI` scene. An event home asks for `/event/get_event_ranking_list` when its
  `eventInfo.eventRankingFlg` is 1, keeps the ranking whose campaign is the event's, and shows
  its ranking button (event type Normal 1).
- `get_event_ranking_list {}`: `eventList` [{eventCampaignId, order, rankingList [{type
  (Personal 1, Clan 2), eventType, groupNo, rankingKey, rankingName, order}]}]. The screen takes
  the entry whose `eventCampaignId` equals the campaign (String.Equals on every entry's id: none
  may be null) and its ranking with the lowest `order`; without one the screen stays empty.
- `get_event_ranking_detail {eventCampaignId, rankingKey, rankingRange}`: range 0 = around me,
  1 = top (the screen opens on top when the player has no points). `eventCampaignId` is the cached
  `bizarre_list` eventInfo's id whenever one is cached, even on the Arena's board, so only
  `rankingKey` is reliable. Reply: `type` 2 shows `clanRankList`, else `personalRankList`
  (counted: never null); `personalRank` is "my rank" (0 = not ranked; the around filter then
  says "You are not participating"); `userEventPoint` with the `Clan_Ranking_pt` unit;
  `rankingName` the title (a text label); `endDate` the time left (DateTime.TryParse). A status
  51901 leaves the screen empty.
- PersonalRankingInfo banner: `statueId` thumbnail, `username`, `userRank`, `winCount`,
  `loseCount`, `lastLoginAt` (a date), `rank`, `userEventPoint`; `userId` String.Equals the
  player's `/user/user_info_mini` id (marks and centres "me"; none may be null).
- `get_event_ranking_reward {eventCampaignId, rankingKey}`: `rankingRewardList` [{rewardItemSet,
  rankMin, rankMax, unit}]; "my reward" is the first row whose `rankMax` reaches my rank; the
  reward list shows "rankMin-rankMax"; every ItemSetInfo list and ItemInfo name/unit is read.

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
- **Ranks and rewards:** `pvpUserInfo.rank` is the player's place in the season's ranking
  field (§3). Point rewards each season (100,
  300, 600, 1000, 1500, 2500, 4000, 6000 points: Diamond Coins, Memory Rings, gold, Fight Coins)
  go to the Message Box when reached and are listed in `eventRewardList` and in
  `/event/point_reward_list` (`getFlg`). Seasons are weeks turning on Monday at 04:00 JST; a
  season with battles sends a reward by its final place in the field (top 10: 500 Diamond
  Coins, 100: 300, 500: 150, else 50) and points, streak, Fever and rewards start again.
- **Polling:** `pollingTimes` 30 seconds (Build 24; it was 0).
- **Defence (Build 24):** AI rivals attack while the player is away. Only an absence of 15
  minutes or more since the last `/pvp/home` (measured from `last_seen`, written at most every 5
  minutes while the home polls), only inside the current season. Time is cut in 2-hour slots;
  each slot has one possible attack (50 %), at a moment within the slot, and an absence brings at
  most the 3 latest. The target is one of the teams with a leader that is not resting then; the
  attacker is a rival ranked near the player (§3). The defence holds 60 % of the time (damage
  0); otherwise the team loses 1250 durability (half a lost battle) and can retire and rest like
  after a battle. Seeds are the player id and the slot, so an absence always brings the same
  attacks. The home that simulates them sends them once as `damageInfo` (type 0). Points never
  change on defence.
- **Crazy Diamond (Build 24):** the gauge fills with every durability point the teams lose
  (battles and defences), up to 10000: one team's worth. Full, it shows the button and
  `crazyDiamondNotifyFlg` is sent once. `recover_hp_pvp` typeId 0 needs a full gauge (52010
  otherwise), revives every resting team and empties the gauge (kept when none rests). The
  gauge survives the season roll.
- **Abandoned battles (Build 24):** a `pvp_start` without its `pvp_end` counts as a loss when
  the home is polled 10 seconds or more after it started (the client stops polling while it
  starts a battle): streak 0, a loss, 2500 durability, a battle for Fever; `forceDefeatFlg` is
  true once. A late `pvp_end` pays nothing.
- **recoveryTimes:** 0. Costs never rise offline (BP is always full and every team costs 1, see
  "BP and cost"), so the four-battle recovery cycle (§1.10) has nothing to count, and the 00029
  notice would be untrue.
- **Win-streak rates (Build 24):** 20 per win of the running streak, at most 100, for both: +2 %
  ATK and DEF per win for the player's units, at most +10 % (§1.11).
- **Opponents' names:** rivals of the ranking field (one below the player, one near, one above),
  with their points.
- **Not built:** clan rankings and clan points (no Towns offline), the rising BP costs,
  `eventCampaignList` (empty), ranking rewards for events (events never close offline).
- **Formation pattern:** `area_position_pattern_table` gets a `PatternPVP` row with the story
  positions.

## 3. Rankings (server, ours unless marked; `server/services/src/services/ranking.rs`)

- **Field:** 999 AI rivals per board, so a thousand places with the player. Rival i (0 = best)
  ends near `top x (1 - i/999)^2`, give or take 10 %; top 10000 for an Arena season, 5000 for an
  event. Stronger rivals lead rarer statues (6 stars at the top, 3 at the bottom), have a higher
  rank (220 down to about 20), an English handle with a number, and (Arena) `points/25` wins and
  a third as many losses. Seeds are the campaign and the season (Arena) or the event id, so a
  board keeps its rivals all season.
- **Arena board:** rivals earn their points over the week (5 % at the season start, all at its
  end). The player's place is behind every rival with more points (`pvpUserInfo.rank`, the board
  and the season reward use the same field; the season reward uses the final points).
  `endDate` is the next season start.
- **Event boards:** each point event (50045) has one (`eventRankingFlg` 1 in `bizarre_list`);
  the field does not move, the player's score is the event points.
- **Routes:** the list holds the Arena (`arena-510027`, PvpAuto) and each point event
  (`event-<id>`, Normal), personal only, named by the campaign's `EQ_<label>_Name` text. The
  detail goes by `rankingKey` (then by `eventCampaignId`); top = places 1..100, around = 10
  places either side; the player's entry carries their id, name, rank, the tactical main
  leader's statue and (Arena) their wins and losses. An unknown key is an empty board. Rewards:
  the Arena's season tiers (1-10, 11-100, 101-500, 501-1000); none for events.

## 4. Null audit (Build 24)

Every Arena and ranking reply is walked in tests (`services/shapes.rs`) against the client's
classes (string, list and object fields of the 3.12.0 classes, inherited ones included; the
fixture `services/testdata/client_shapes.json` comes from dump.cs with the static fields left
out): none may be missing or null, in every state the routes produce. Allowed, with the reason
in the walker: the client's own countdown caches (`recoveryCalcInfo`, UserInfo's `*LocalInfo`);
an empty seat's `statueInfo` (`{statueId: ""}`; the banners reset that seat without reading it);
`{}` for "none" in the shared statue and Mind builders (AbilityInfo, SkillInfo, TokkouInfo,
StEffRateInfo, whose fields are private in the client and filled from the master tables). The
raid part of `/info/event_list` is not the Arena's and is not walked.

| Reply | Walk | Fixed in Build 24 |
|---|---|---|
| `/info/event_list` (Arena entry, `userBattleInfo`) | complete | — |
| `get_pvp_config`, `point_reward_list`, `heal_item_list`, `home_acqusition_check` | complete | ItemInfo strings (b24 base) |
| `/pvp/home` | complete (with teams resting, defence, a full gauge) | `pollingTimes` 30 (was 0) |
| `enemy_player_list`, `pvp_start`, `pvp_end` (win, loss, point rewards) | complete | — |
| `recover_hp_pvp`, `fever_mode` | complete | typeId meaning |
| formation routes (`team_info_pvp`, `set_team_all_pvp`, `leader_change_pvp`, `auto_set_and_equip`, `equip_multiple_minds_pvp`) | complete | — |
| `/user/user_info_mini` | complete | — |
| ranking list, detail (Arena, event, unknown key), reward | complete | new routes |

To be confirmed on a device: the whole flow. Expected but unverified: the Arena button on My Page,
the home (no longer locked by polling), the opponent banners, the battle and the result screen,
the defence messages on entering the home, the Crazy Diamond button and Full Revive, the
connection-lost notice, the ranking screens (Arena and event 50045) and the reward list.
