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
6. Assist abilities (Build 24): `tools/content/build_assist_abilities.py` →
   `server/content/assist_abilities.json` and the `DT_AST_*` rows of
   `dtbattle_ability_data_table`; the server generates `assist_ability_condition_table` from it
   (docs/research/19-assists.md §5).
7. Skill Minds and time stop (Build 25): `tools/content/build_mind_skills.py` →
   `server/content/mind_skills.json` (which Mind grants which skill, read from the game's own
   `MIND_TEXT_<id>` and `SKILL_NAME_<p>_<NN>` labels, and which skills stop time). Mind links:
   `tools/content/build_mind_links.py` → `server/content/mind_links.json` (Gamerch's link stat per
   Mind) and the `DT_MLINK_*` rows of `dtbattle_ability_data_table`
   (docs/research/26-links-and-skills.md).

## Offline rules (preservation sandbox)

- The player owns every obtainable statue (266: model + animations + action set + leader ability in
  the player's files) at level 1; teams: 5 per battle type, the tactical main team (Jonathan,
  Jotaro, Josuke when present) leads My Page.
- Build 25: plus the 26 assist-only statues (`assistOnly` in statues.json: a model, animations,
  an action set and their own `ABI_AST_<id>_N` label, no leader ability). They can be assists,
  never team members (docs/research/19-assists.md §6).
- All story chapters are open; a cleared stage is recorded; a clear grants the stage's gold and rank
  EXP, and the same amount as statue EXP to the three statues that fought (a `jdr-default` until a
  source gives statue EXP per stage). A clear pays once per started battle.
- Defaults grow with a chapter's position among chapters of its type and stay in the game's limits:
  stamina ≤ 20 (the rank-1 AP maximum), enemy level ≤ 99.

## Current coverage (Build 10)

| Data | Source | State |
|---|---|---|
| Statue ids, models, bundles | player's files | 266 obtainable |
| Statue rarity, type, Lv99 | Gamerch (leader- or assist-ability name), ShoutWiki (CC BY-SA 3.0, by elimination) and the Build 33 research pass (JoJoWiki CC BY-SA 4.0 and ShoutWiki pages matched by ability names, `tools/content/research_statues.py`) | 206 of 292 (129 Gamerch, 11 ShoutWiki, 65 research collections, 1 assist-only); the 86 others are mostly ★3/★4 villains and alternate-attribute versions no wiki lists |
| Statue Lv1, max tree | the same pages where they show them | Lv1 from the wiki for 35 statues, Lv99 / 10 for the other 105 (in those 35, Lv99 / Lv1 is 9.5–10.0); max tree missing for 47 |
| Stats per level | linear Lv1→Lv99 | all |
| Tactical basic attack (`dtAttack`) | the statue's own action sets in the player's files (`SKILL_<psid3>_99` = ATTACK, else its lowest SKILL_NN) | all 266 (64 distinct ids) |
| My Page background per statue | 21 backgrounds in the player's files, chosen by JoJo part (stable per statue id) | default choice |
| Story chapters/stages/areas | player's files | 82 chapters (29 action, 53 tactical), 370 stages |
| Stage AP, rank EXP, gold, recommended level | Gamerch (Wayback snapshot, chapters 1–23 matched by title) | 86 of 370 stages (gold blank on the wiki for 39 of them, so default); enemies and rewards default (the wiki shows them as images); the recommended level is used as the enemy level |
| Player rank: EXP per rank, AP max | Gamerch user-rank page | ranks 1–99 |
| Minds: rarity, cost, flat stats | Gamerch (name) | 417 owned; 381 have no cost on the wiki and take the cost every Mind of their rarity has there (★1 3, ★2 5, ★3 8, ★4 12; Build 33) |
| Assist-ability conditions | the Build 33 research pass (ShoutWiki/JoJoWiki statue pages) | 116 statues; the others always active (`build_assist_abilities.py --research`) |
| Daily quest numbers | Gamerch day pages via the Build 33 pass | Normal tier of every weekday (AP 8, level 20) and Tuesday's EXP/gold (15 stages); the other tiers ours |
| Tactical tuning | atwiki ジョジョDR２ (tactical era, Wayback) and the Bahamut forum | Reversal 4 turns, speed +200%, cool turns halved; chain ×1.1/×1.35/×1.5/×2.0; type chart ×1.5/×0.7; same-type assist ×2 (Build 33) |
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
