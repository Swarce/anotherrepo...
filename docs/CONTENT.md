# Server content (Build 10)

What the dead server held and no player copy contains, rebuilt as committed JSON under
`server/content/`. Identifiers come from the player's own files (scanned locally); numbers come from
community wikis (each record cites its page) or are marked `jdr-default`.

## Pipeline

1. `tools/content/scan_statues.py`, `tools/content/scan_quests.py` read the player's own files and
   text labels (local only) → `local/content/*_scan.json`: statue ids, model names, bundles present;
   chapters, stages, area scenes, chapter maps, maps.
2. Wiki pages are fetched into `local/wiki-raw/` (git-ignored; never committed):
   ShoutWiki (MediaWiki API), JoJoWiki (MediaWiki API via a fetch service), Gamerch (Wayback
   Machine snapshot via a fetch service). No images.
3. Parsers extract numbers only (`tools/content/shoutwiki_statues.py`, …).
4. Builders merge them (`tools/content/build_statues.py`, `build_quests.py`) →
   `server/content/statues.json`, `server/content/quests.json`. A wiki value attaches to a game id
   only on an unambiguous match; everything else stays a marked default.
5. The server serves them: statue list/teams (`collection.rs`), story quests (`quests.rs`), and the
   generated master tables `statue_setting_table` (My Page background per statue) and
   `statue_level_table` (stats per level).

## Offline rules (preservation sandbox)

- The player owns every obtainable statue (266: model + animations + action set + leader ability in
  the player's files) at level 1; teams: 5 per battle type, the tactical main team (Jonathan,
  Jotaro, Josuke when present) leads My Page.
- All story chapters are open; a cleared stage is recorded; clears grant the stage's gold and rank
  EXP.

## Current coverage (Build 10)

| Data | Source | State |
|---|---|---|
| Statue ids, models, bundles | player's files | 266 obtainable |
| Statue rarity, type, Lv1/Lv99, max tree | Gamerch (exact match by leader-ability name) and ShoutWiki (CC BY-SA 3.0, by elimination) | 142 of 266 (129 Gamerch, 13 ShoutWiki); the rest (mostly released after the 2018 snapshot) default |
| Stats per level | linear Lv1→Lv99 (195 of 200 wiki statues have Lv99 = 10 × Lv1) | all |
| My Page background per statue | 21 backgrounds in the player's files, chosen by JoJo part | default choice |
| Story chapters/stages/areas | player's files | 82 chapters, 370 stages |
| Stage AP, rank EXP, gold, recommended level | Gamerch (Wayback snapshot, chapters 1–23) | 100 of 370 stages; enemies and rewards default (the wiki shows them as images) |
| Player rank: EXP per rank, AP max | Gamerch user-rank page | ranks 1–99 |
| Minds, items, events, gacha | wikis (pending) | not yet |
