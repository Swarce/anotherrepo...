# 05 — Live-ops: events, rankings, raid, PvP (stub)

What the client expects for time-limited content. Offline, events run as a fresh, deterministic
rotation computed from the server clock, with event texts rewritten and no real-world dates.
Details and the rotation design come in M5.

Confidence: H for endpoint names, request fields and the boot-time `event_list` call; M for the
meaning of empty answers; the event content itself is not researched yet.

## 1. Event list at MyPage (H/M)

- `/info/event_list {}` is called on every MyPage load (B-boot §0, step 15).
- Response: `{ eventList: List<EventCampaignInfo>, userBattleInfo{fieverEndTime},
  raidInfo{raidBossInfo, raidStageInfo, totalRescueCount} }`.
- An empty list with empty objects is accepted (M); Build 01 sends that.
  `UpdatePVPInfo` reads the cached response.
- Banners: `/info/banner_list`; notices: `/info/check_forced_notify`, `/info/notify_list`.
- `emergencyInfo` in every response can show a ticker notice (02 §5); we keep it off.

## 2. Event endpoints (H names)

| Area | Endpoints | Key request fields |
|---|---|---|
| Event points / ranking | `/event/point_reward_list`, `/event/get_event_ranking_list`, `/event/get_event_ranking_detail`, `/event/get_event_ranking_reward`, `/event/clan_member_event_point_list` | `eventCampaignId`, `rankingKey`, `rankingRange` |
| Event quests | `/bizarre/bizarre_list {category}` (also from `EventHomeUIController`), `/bizarre/bizarre_start` (has campaign coefficients), `/bizarre/bizarre_end` (`eventRankingSendInfo`) | |
| Raid | `/raid/raid_list`, `/raid/raid_start`, `/raid/raid_end`, `/raid/raid_offer_rescue`, `/raid/get_raid_reward`, `/raid/raid_history_list` | `eventCampaignId`, `raidStageUid`, `offerType` |
| PvP | `/pvp/home`, `/pvp/get_pvp_config`, `/pvp/enemy_player_list`, `/pvp/pvp_start`, `/pvp/pvp_end`, `/pvp/fever_mode`, team set-up calls | `eventCampaignId`, `teamNum`, `enemyuId` |
| Event shop | `/shop/event_shop_list`, `/shop/event_shop_buy` | `eventId` |
| Dungeon (tarot) | `/dungeon/*` | `dungeonId`, `floorNum` |

Full list with response classes: [../PROTOCOL.md](../PROTOCOL.md).

## 3. Plan (decided, see ROADMAP)

- Rotation: deterministic from the server clock; survives time jumps and missed days.
- Texts: rewritten in our own words, no real dates; no copied event text.
- Rankings: local only (the player against generated rivals). PvP against generated rival teams.
  Built in Build 24 for the Arena and the point events: research/23-arena.md §1.12 (client) and
  §3 (server).
  Raid rescue by NPC helpers. Clan events: hidden with the clan room.
- Content source for schedules and rewards: public wikis, cited here in M5.

## Open points

- Which `EventCampaignInfo` fields gate the event home screen and its quests (M5).
- Ranking reward tiers and raid boss tables (content, M5).
