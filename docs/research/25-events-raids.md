# 25 — Raids, high scores, quizzes, event bonuses and campaigns (Build 25)

What client 3.12.0 does with the remaining event kinds, and what the offline server sends. H = read
in the disassembly (`libil2cpp.so` 3.12.0 arm64, addresses are RVAs) or in the player's files;
M = inferred. No game text is reproduced; labels are named by key. Earlier event notes:
[05](05-liveops-events.md), [13](13-menus.md) (Quest Select), [20](20-event-exchange.md)
(exchange, point events), [23](23-arena.md) §3 (rankings).

## 1. Raids (Elite Raid boss battles)

### Client (H)

- **How a boss appears.** After a tactical battle, `BattleSceneController.OnResult` (iterator
  0xEC5120) calls `RaidBattleSceneController.HasOccurredRaidBattle` 0xCDE680: true when
  `bizarre_end.raidStageInfo` is non-null and `RaidStageInfo.IsValid` 0xE19C78
  (`raidStageUid != 0`). Then `ToRaidBattleResult(DiscoveryResult, raidStageInfo, raidBossInfo)`
  stores the stage (its `category` becomes the selected category) and the boss, and
  `AppearRaidBattleCut` loads the scenes **`RaidStartsCutINUI`** and **`RaidBattleResultUI`**.
  Otherwise the battle ends as usual (`EndBattle`).
- **The fight starts only from that screen.** `RaidBattleResultUIController.CheckRaidPoint` checks
  the raid points (`userInfo.raidPoint`, items 5001/5002 restore them) and calls
  `OnStartRequestRaidBattle` → `/raid/raid_start {stageId, raidStageUid, teamNo, sallyStatus
  (1 normal, 2 full), payType}`; `RaidStartResponse.enableRaidStart` 0 Enable, 1 Defeated,
  2 Timeout. The battle ends with `/raid/raid_end {result, defeatedEnemyList, dtDropList,
  totalDamage}` → `clearResult` (0 Withdraw, 1 MyDefeat, 2 OtherDefeat, 3 Timeout),
  `raidDropList`, `raidEventResult {userEventPointAll, getUserEventPoint, damage, eventRewardList,
  subjugateEventPoint, subjugateRewardList}` and the boss again (DamageResult mode). The rescue
  buttons send `/raid/raid_offer_rescue {offerType}` (0 all, 1 follow/followers).
- **No caller:** `/raid/raid_list`, `/raid/raid_history_list` and `/raid/get_raid_reward` are not
  requested anywhere (no field or local of their request classes). Event Home's raid panel
  (`UpdateUiRaidBoss` 0xD14420) has no caller, and `/info/event_list.raidInfo` is not read
  (`MypageUIController.UpdatePVPInfo` is the response's only reader).
- **The scenes are missing.** The 18 scene bundles of the player's files and the 23 built-in
  scenes list no raid scene (`RaidBattleResultUI` and `RaidStartsCutINUI` are only string literals
  in the code). `GameSceneManager.LoadScene` loads by name with `SceneManager.LoadSceneAsync`, so a
  raid boss would leave the client waiting on a scene that cannot load (M: hang or error).

**So raids cannot be played with these files.** The labels exist (`RaidText_JP`,
`STAGE_RAID_0001..0004_NAME`) but not the screens.

### Server

- Every `raidStageInfo` it sends (`bizarre_end`, `bizarre_list.raidInfo`, `event_list.raidInfo`)
  is complete with `raidStageUid 0`, and `raidBossInfo` is a complete "no boss" object
  (`raidBossStatus -1`).
- The six routes answer an inactive raid (services/raid.rs): no bosses, the start refused as timed
  out, the end a withdrawal, the rescue failed, an empty history, nothing to collect.
- The 2017 "Strong Enemy Invasion" events have no chapter labels in 3.12 (`EQ_5900xx` names only),
  so there is nothing to rebuild as quests either.

## 2. High-score events

### Client (H)

- **Event Home.** `eventCollectType` 4 (HiScore) shows no item or point panel and no exchange or
  reward button; 5 (HiScoreShop) shows the `eventItemList` icons and the exchange (with
  `eventShopFlg 1`). `eventRankingFlg 1` adds the ranking (type Normal 1, as for point events).
- **The stage's rule.** `StageInfo.eventPointSettingList [{type, min, max}]` (DTEventPointType:
  1 Point, 2 Turn, 3 MaxDamage, 4 TotalDamage) is read only by
  `BizarreQuestSceneManager.MakeBattleCondition` 0xE0C930, which adds the matching condition line:
  `DTbattleText_MES_002` (max damage), `_036` (total), `_037` (turns), `_038` (total and turns),
  `_039` (max and total), `_003` (max and turns), `_040` (all three). `min` and `max` are not read
  there.
- **The stage banner.** `BizarreStageBannerController.SetDtBattleIcon` shows a high-score box with
  `StageInfo.highScore`, and hides it when the value is **-1**. Until Build 24 every stage sent 0,
  which showed the box with 0 on every tactical stage (M: what the box looks like).
- **What the client sends.** Every tactical `bizarre_end` carries `eventRankingSendInfo {turn,
  maxDamage, totalDamage}` (BattleSceneController.RequestBattleEnd 0xEC58A0: total turns, largest
  damage to the boss, total damage dealt).
- **The result panel.** `BattleResultUIController.SetUI` builds this battle's
  `EventRankingResultInfo` itself: `point = nowEventRankingPoint` (from the reply) and rows Turn,
  MaxDamage and TotalDamage from the battle log. `BattleResultHighScoreUIController.Setup`
  0xC0D4B4 shows the panel only when the reply's `oldEventRankingResultInfo.eventDataList` has
  entries: the old score (or "-" on a first clear), this score, one row per old entry beside this
  battle's value of the same mode (EventRankingMode 2/3/4; a Point row finds no partner and is
  skipped), and the "new record" animation when old < now.
- **Currency.** `eventResult.eventType 8` (UserHighScoreShop) lists `getEventItemList` like type 3.

### Content (H, from the player's files)

- `EQ_560001..560005_Name` are "High Score Battle" campaigns; banners exist for 560001 and 560005.
  Each has three chapters (training, high-score quest, a strategy course).
- The stage names carry the rule: "Max Damage", "Total Damage", "Total & Turns", "Max & Turns",
  "Max & Total & Turns", "Turns" (560005), "Max Damage" (560001). The tool turns them into
  `scoring` per stage. Other stages of these events are ordinary.
- 560001 has an event-shop goods label (`event_shop_detail_560001_01_text`), so it gets the shop;
  560005 is the plain high-score kind. The text labels `Battle_Highscore_*` include "Total Score"
  (M: a total of stage bests ranked).

### Server (ours where marked)

- Score (ours): turns score 100,000 for a one-turn clear, 5,000 less per further turn, at least
  10,000; max damage scores itself up to 200,000; total damage up to 1,000,000; the stage's rule
  sums its parts. `eventPointSettingList` carries these ranges.
- Each high-score stage keeps its best battle; the ranking point (`userEventPoint`) is the sum of
  the stage bests; the field of AI rivals is scaled to 150,000 per high-score stage (ours).
- The reply's `nowEventRankingPoint` is this score, `oldEventRankingResultInfo` the stage's best
  with one row per scored value (zeros before a first clear). A loss scores nothing and shows no
  panel.
- High-score shop (560001): Fight Coin 2 (item 3032, ours), one per 2,000 points, 5 to 100 a
  high-score clear, 20 for the other stages; the exchange takes that coin (the Ring Exchange goods).

## 3. Quiz events

### Content (H)

- Events 50084, 50090, 50094 and 50097 have "Quiz Quest" chapters. A quiz chapter is a "Question
  Stage" and three "Answer Stage <n>: <choice>" stages.
- 50090's own scripts (`e_ev_eq_500090_*`, our English) state the rules: clear the question stage
  for an answer pass, clear the right answer stage for a collection item (the Chunky Necklace,
  3106), certificates by the number of necklaces after the event. They also state each right
  answer ("The answer is (n)"). The tool finds it for all ten quizzes of 50090 (by the choice the
  answer line names, else the script after the question that lists the choices).
- The items include an answer pass per quiz (3107–3116 for 50090) and the collection items of the
  four quiz events.

### Server (ours where marked)

- 50090 is the quiz event: `eventCollectType 2` (the necklace icon and count), `eventShopFlg 0`
  (the "check rewards" button opens the certificate list).
- The first answer stage cleared after the question stage is the answer: right 30 necklaces, wrong
  5 (ours); later the right stage pays 10, the others nothing; every other event stage pays 10.
  Passes are not modelled (every stage is open offline). An answer before the question does not
  count.
- Certificates (ours): seven tiers at 50 to 360 necklaces collected, paid into the Message Box
  when reached and shown received (`getFlg 1`), like point rewards. The event's real certificates
  are not recorded.

## 4. Event bonuses

### Client (H)

- Event bonuses were server data: a Mind's `tokkouInfo` / `dtTokkouInfo` (with `tokkouStart`,
  `tokkouEnd`) drives the Mind screen's bonus panel and the battle's MindTokko ability; the texts
  exist (`ABI_TOKKO_NAME_*` "... Event Bonus", `AFFECT_EVENT_ITEMDRP_*` drop bonuses,
  `AFFECT_EVENT_ONLY_*` "in events only" boosts), but which units had which bonus is not in the
  player's files.

### Server (ours)

- Each event names three bonus statues (4-star or better) and two bonus Minds (4-star), a stable
  pick by the event id. Each bonus statue in the team adds 25 %, each bonus Mind on it 10 %, at
  most +100 %, to what a clear drops (points, currency, quiz items; never the score).
- Event Home's event page (`eventInfo.htmlLink`, our web module) lists the rules and the bonus
  units by name.
- Not done: tokkou abilities on the Minds themselves (they need affect-table rows for the
  `AFFECT_EVENT_ITEMDRP_*` texts and their text rules; untested in battle).

## 5. Campaigns

- **Quest Select (H, Build 20).** `eventCampaignList` entries with `bizarreHomeFlag` show
  `banner + "_small"` in Campaign Info; a tap opens `htmlLink`. Build 25 adds the four campaign
  banners of the player's files that have a name label (`ec_600000`, `600002`, `600003`,
  `600016`) after the events; their page names the campaign.
- **Arena home (H).** The `/pvp/home` `eventCampaignList` was empty. The home
  (`PvP2HomeController` polling callback 0xDD15E8) keeps the entries whose `eventCampaignId`
  equals its campaign (String.CompareTo on each id: none may be null) and shows them in its
  campaign panel (`SlidePanelGroupController.SetSlidePanel`, `banner + "_small"`). Build 25 sends
  one entry for the Arena campaign with an Arena banner of the player's files
  (`eq_510002_..._start`, ours: 510027 has none) and an Arena page.

## 6. More event chapters

`tools/content/build_extra_quests.py` now builds nine events (139 chapters, 393 stages): the four
earlier ones, the two high-score events, the quiz event and two more exchange events (50033,
50061). Each event records its kind, currency, bonus units, the scoring rules (high score) and
the quiz answers. Maps, enemies, levels and rewards stay ours.

## 7. Null walks

`testdata/client_shapes.json` now covers `BizarreSelectResponse`, `BizarreListResponse`,
`StageListResponse`, `BizarreEndResponse`, the six raid responses and the event shop replies; the
tests walk every reply of every event kind. Exempt: `BizarreEndResponse.supportUserInfo` (`{}`, no
support player: the result screen checks `UserInfoMini.Valid`, a non-empty `id`). Not walked:
`BizarreStartResponse` (its empty "none" objects `supportUserInfo`, `tarotInfo` and
`mapGimmickInfo` are the story battle's since Build 10; `MapGimmickInfo.IsValid` 0xB8376C tests
`gimmickId`, the other two are not checked yet), and `bizarre_list.eventInfo` outside events
(`{}`, not read there).

## Open points

- Raids need the two missing scenes; a client with them could be served from §1.
- On a device: the high-score banner box and result panel, the quiz flow, the Arena campaign
  panel, the campaign banners.
- The event talk scripts (`startTalkId` / `endTalkId`) would show the quiz questions and answers;
  their id format is not researched (stages send "").
- The real scoring formula, certificate tiers, bonus units and high-score currency are unknown
  (ours above).
