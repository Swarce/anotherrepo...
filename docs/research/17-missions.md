# 17 — Missions (`trophy/*`, Build 21)

Read in the 3.12.0 client (H read in code, M inferred).

- **"trophy" is the Missions screen (H).** `TrophyUIController` lives in `MissionUI` (`scene_mypage`),
  with tabs Daily, All and Limited. It calls `/trophy/trophy_list {allFlag}`,
  `/trophy/trophy_result_get {achievementId}` and `/trophy/get_all_trophy_result {}`.
- **TrophyInfo.**
  - `status`: 0 not achieved, 1 claimable, 2 received.
  - `day`: 0 normal tab, 1–7 = Mon..Sun daily tab, 8 limited.
  - `achievementText` is a label key; the target is written into the label, with no tags.
  - The progress bar shows min(count, param)/param.
  - `acEnd` `"2036-01-01 00:00:00"` means no end; any other value must parse.
  - The row shows `itemSetInfo`'s first item, statue or Mind.
- **The client does not filter dailies by weekday (H),** so only today's are sent.
- **Crashes (H):** a null `itemSetInfo` or a null list inside it; an `acEnd` that is null, empty or
  unparseable.
- **Claims.** The response's `trophyList` replaces the client's list. `popupFlg` must be false
  (true opens a store-review dialog). The client then says the rewards went to the message box, and since Build 22 they do (docs/research/18).
  Our box is not built yet, so rewards arrive on the account directly: gold, free Diamond Coins,
  or catalog items. Friend points have no wallet offline.
- **Home badge:** `get_menu_notify.unopenedAchievement` is the number of claimable missions.
- **Progress (H): the client never reports it.** The server counts quest clears, rank EXP and
  gold earned, daily-quest clears, a full team, a leader change, Mind equips, enhancements,
  boosts, summons, profile edits and encyclopedia visits.
  - Daily counters reset at 04:00 JST; claimed missions stay claimed (`Progress.missions`).
- **Content** (`server/content/missions.json`): 20 missions.
  - Text: the game's own labels (`Daily_ACHIEVEMENT_Text_*`, `ACHIEVEMENT_Text_*`).
  - Rewards: from the wiki's mission page, the text matched to the labels by source hash.
  - Only missions the server can count are listed. Left out: JoJo/Mind links, follows, towns,
    transfer passes, support, selling.
