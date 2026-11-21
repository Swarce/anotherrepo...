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
4. Builders merge them (`tools/content/build_statues.py`, `build_quests.py`, `build_extra_quests.py` (daily and event chapters), `build_minds.py`) →
   `server/content/statues.json`, `quests.json`, `minds.json`. A wiki value attaches to a game id
   only on an unambiguous match against the game's own Japanese labels (from the player's l10n
   export, compared after NFKC normalisation, so full-width and half-width forms agree); ids a wiki
   page may carry are not trusted. Everything else stays a marked default:
   - statues: Gamerch by leader-ability name (`ABI_LEAD_NAME_<id>`; assist-ability name
     `ABI_AST_<id>_N` for assist-only statues); ShoutWiki by elimination within a character, only
     when the entry keeps rarity in order within the id's variant family;
   - stages: Gamerch chapter title against `CHAPTER_NAME_<id>`, and only when the wiki lists as many
     stages as the game has (chapters 2, 6, 8 and 17 differ and keep defaults);
   - minds: name against `MIND_NAME_<id>`;
   - plausibility: a statue whose wiki stats do not have 5 ≤ Lv99 / Lv1 ≤ 20 keeps defaults
     (2 rejected: one ShoutWiki typo, one out of rarity order); an enemy level under half the
     previous stage's is treated as a typo. A stage lists the fields the wiki left blank in
     `defaults`.
5. The server serves them: statue list/teams (`collection.rs`), story quests (`quests.rs`), and the
   generated master tables `statue_setting_table` (My Page background per statue) and
   `statue_level_table` (stats per level).

## Offline rules (preservation sandbox)

- The player owns every obtainable statue (266: model + animations + action set + leader ability in
  the player's files) at level 1; teams: 5 per battle type, the tactical main team (Jonathan,
  Jotaro, Josuke when present) leads My Page.
- All story chapters are open; a cleared stage is recorded; a clear grants the stage's gold and rank
  EXP, and the same amount as statue EXP to the three statues that fought (a `jdr-default` until a
  source gives statue EXP per stage). A clear pays once per started battle.
- Defaults grow with a chapter's position among chapters of its type and stay in the game's limits:
  stamina ≤ 20 (the rank-1 AP maximum), enemy level ≤ 99.

## Current coverage (Build 10)

| Data | Source | State |
|---|---|---|
| Statue ids, models, bundles | player's files | 266 obtainable |
| Statue rarity, type, Lv99 | Gamerch (leader- or assist-ability name) and ShoutWiki (CC BY-SA 3.0, by elimination) | 140 of 266 (129 Gamerch, 11 ShoutWiki); the rest (mostly released after the 2018 snapshot) default |
| Statue Lv1, max tree | the same pages where they show them | Lv1 from the wiki for 35 statues, Lv99 / 10 for the other 105 (in those 35, Lv99 / Lv1 is 9.5–10.0); max tree missing for 47 |
| Stats per level | linear Lv1→Lv99 | all |
| Tactical basic attack (`dtAttack`) | the statue's own action sets in the player's files (`SKILL_<psid3>_99` = ATTACK, else its lowest SKILL_NN) | all 266 (64 distinct ids) |
| My Page background per statue | 21 backgrounds in the player's files, chosen by JoJo part (stable per statue id) | default choice |
| Story chapters/stages/areas | player's files | 82 chapters (29 action, 53 tactical), 370 stages |
| Stage AP, rank EXP, gold, recommended level | Gamerch (Wayback snapshot, chapters 1–23 matched by title) | 86 of 370 stages (gold blank on the wiki for 39 of them, so default); enemies and rewards default (the wiki shows them as images); the recommended level is used as the enemy level |
| Player rank: EXP per rank, AP max | Gamerch user-rank page | ranks 1–99 |
| Minds: rarity, cost, flat stats | Gamerch (name) | 417 owned; cost is a default (3 × rarity) for 381 |
| Items, events, gacha | Gamerch (fetched: 48 gacha, 22 events) | not served yet |

## Research collections (Build 18)

`tools/content/build_statues.py --extra <file>` reads an external research collection: records with
an exact `statueId`, a confidence and a source page, gathered by the owner with an outside assistant
from the community wikis and kept in `local/wiki-raw/` (git-ignored). A record is used only when:
- the statue has no Gamerch or ShoutWiki match;
- its confidence is medium or high;
- its stats pass the Lv99/Lv1 plausibility check.

The source page is cited per record. The first collection (2026-11-15) gave stats for 5 more
statues, and independently confirmed 11 ShoutWiki matches with identical numbers. It also gave the
tactical battle's evaded-hit damage (1/4) and critical multiplier (×1.5). Its skill damage ranks had
no skill names and were inconsistent, so they were not used.
