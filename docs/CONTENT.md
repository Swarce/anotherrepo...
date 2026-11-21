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
     when the entry keeps rarity in order within the id's variant family; then the JoJoWiki dump
     (`--jojowiki`) cross-checks them (see "Wiki cross-checks" below);
   - stages: Gamerch chapter title against `CHAPTER_NAME_<id>`, and only when the wiki lists as many
     stages as the game has (chapters 2, 6, 8 and 17 differ and keep defaults);
   - minds: name against `MIND_NAME_<id>`, then a loose key without punctuation for the 17 Minds
     checked in Build 41 (docs/research/16 §9);
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
   (docs/research/19-assists.md §5). From Build 41 it reads two ShoutWiki dumps (`statues.json`,
   `oldmode_statues.json`) and two research files (`research-b33/statues` and `/action`); the
   command is in the script's docstring. Leader abilities are rebuilt first:
   `build_leader_abilities.py … --table` (docs/research/12 §4). The leader builder replaces its
   `DT_LEAD_*` rows in place, while the Mind-link and assist builders move theirs to the end of the
   table, so run `build_mind_links.py` (step 7) before this step to keep the committed row order.
7. Skill Minds and time stop (Build 25): `tools/content/build_mind_skills.py` →
   `server/content/mind_skills.json` (which Mind grants which skill, read from the game's own
   `MIND_TEXT_<id>` and `SKILL_NAME_<p>_<NN>` labels, and which skills stop time). Mind links:
   `tools/content/build_mind_links.py` → `server/content/mind_links.json` (Gamerch's link stat per
   Mind) and the `DT_MLINK_*` rows of `dtbattle_ability_data_table`
   (docs/research/26-links-and-skills.md).
8. Action-battle skills (Build 33, matching redone in Build 41):
   `tools/content/scan_action_cinematics.py` → `server/content/action_cinematics.json` (per model
   id, the SP-cinematic entries and the `SKILL_<NN>` entries of its action set, read from the
   player's files; entry numbers only). Then `tools/content/build_action_skills.py` (research
   `research-b33/action` and `research-b34/action`) → `server/content/action_skills.json`, and
   `build_skill_table.py` → `skill_data_table`. The commands are in the scripts' docstrings.
   Wiki names match the game's `SKILL_NAME_<p>_<NN>` labels strictly or, failing that, by a strict
   fuzzy rule. Statues without wiki names use their tactical skills, never in the skill or SP slot
   when that skill plays the cinematic. From rarity 5 their finisher is the single label at the
   cinematic entry (docs/research/30-action-battle.md §4–5).

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
| Statue rarity, type, Lv99 | Gamerch (leader- or assist-ability name), ShoutWiki (CC BY-SA 3.0, by elimination), the Build 33 research pass (JoJoWiki CC BY-SA 4.0 and ShoutWiki pages matched by ability names, `tools/content/research_statues.py`) and, from Build 41, the JoJoWiki dump for 22 defaults whose character, rarity, type and assist status the wiki lists once | 228 of 292 with stats, 231 with a type (Build 41); the 64 others are mostly ★3/★4 villains and alternate-attribute versions no wiki lists |
| Statue Lv1, max tree | the same pages where they show them | Lv1 from a wiki page for 101 statues, Lv99 / 10 rounded half up for the other 127; max tree missing for 3 (2000_28, 2100_03, 4000_18: the wikis disagree) |
| Stats per level | linear Lv1→Lv99 | all |
| Tactical basic attack (`dtAttack`) | the statue's own action sets in the player's files (`SKILL_<psid3>_99` = ATTACK, else its lowest SKILL_NN) | all 266 (64 distinct ids) |
| My Page background per statue | 21 backgrounds in the player's files, chosen by JoJo part (stable per statue id) | default choice |
| Story chapters/stages/areas | player's files | 82 chapters (29 action, 53 tactical), 370 stages |
| Stage AP, rank EXP, gold, recommended level | Gamerch (Wayback snapshot, chapters 1–23 matched by title) | 86 of 370 stages (gold blank on the wiki for 39 of them, so default); enemies and rewards default (the wiki shows them as images); the recommended level is used as the enemy level |
| Player rank: EXP per rank, AP max | Gamerch user-rank page | ranks 1–99 |
| Minds: rarity, cost, flat stats | Gamerch (name; from Build 41 also a loose key without punctuation for 17 Minds) | 434 owned; 396 have no cost on the wiki and take the cost every Mind of their rarity has there (★1 3, ★2 5, ★3 8, ★4 12; Build 33) |
| Mind Lv10 flat stats (`lv10`) | Gamerch MAX values; for 9 Minds the wiki leaves blank, the Diamond Records Preservation (PC) data (original-game screenshots at Lv10, an archived translation; cited per row) | 156 Minds (Build 41); the client does not read Mind hp/atk/def (docs/research/16 §9) |
| Assist-ability conditions | ShoutWiki requirement icons (statue pages and 3 Old Mode templates), the research passes (ShoutWiki and Gamerch statue pages, incl. the live Gamerch pages) | 175 statues (Build 41; 116 in Build 33); the others always active; 6 held (docs/research/19 §5.3) |
| Tactical leader abilities (`DT_LEAD_*`) | Gamerch leader numbers (Wayback dump; from Build 41 also live pages via the research pass) | 92 statues (Build 41: +1020_03, 1080_02, 1100_13, 3001_03) |
| Daily quest numbers | Gamerch day pages via the Build 33 pass | Normal tier of every weekday (AP 8, level 20) and Tuesday's EXP/gold (15 stages); the other tiers ours |
| Tactical tuning | atwiki ジョジョDR２ (tactical era, Wayback) and the Bahamut forum | Reversal 4 turns, speed +200%, cool turns halved; chain ×1.1/×1.35/×1.5/×2.0; type chart ×1.5/×0.7; same-type assist ×2 (Build 33) |
| Item ids, categories | player's files: an `it_<id>` icon in `ui_item_00` and an `ITEM_<id>_NAME` label (`tools/content/build_items.py --icons <ui_item_00 listing>`); categories by the client's id ranges or, from Build 41, by ranges read from the item labels (docs/research/15) | 414 items |
| Events, gacha | Gamerch (fetched: 48 gacha, 22 events) | not served yet |

## Wiki cross-checks (Build 41)

`build_statues.py --jojowiki local/wiki-raw/jojowiki/statues.json` (the dump of
`parse_jojowiki_statues.py`, with its `meta.json`) runs after the matches above. JoJoWiki gives Lv99
and the tree max but no Lv1; ShoutWiki and Gamerch are the second and third sources. No value comes
from anywhere else.
- **Lv1 rounding.** Where no page shows Lv1 it is Lv99 / 10 rounded half up. Of ShoutWiki's 604
  Lv1/Lv99 stat pairs, all 46 with a .5 round up (543 are exact, 15 are typos). Python's
  half-to-even `round()` had 12 statues one point low on one stat.
- **A Gamerch Lv99 that is the tree max.** For 2010_28, 3322_08, 4043_08 and 4193_08 the Gamerch
  page leaves Lv1 and the max row blank and puts the tree values in the Lv99 row. Those values equal
  all three numbers of the JoJoWiki (and ShoutWiki) tree max for that statue. Lv99 now comes from
  that entry, the Gamerch numbers become `maxTree`, and Lv1 is ShoutWiki's (4193_08) or the half-up
  rule. Before this, these four statues had about 1.4 to 2 times their real stats.
- **maxTree where ours was null** (43 statues). A JoJoWiki, ShoutWiki or Gamerch entry with the same
  rarity, type and Lv99 gives the tree max. Where the pages disagree we take the value two sources
  agree on (1090_13, 1100_13), or none if there's no majority (2100_03). JoJoWiki's main stat grid
  counts as a source where it differs from the character subpage (3000_13). Where JoJoWiki and
  ShoutWiki agree on a Lv99 one point from Gamerch's, that Lv99 is used (4030_02, ATK 3672).
- **Typed defaults** (`TYPED_DEFAULTS`). For 25 statues the game's own `STATUE_NAME` label and
  character id name the character and variant (a JoJoWiki page), and the wiki lists exactly one
  statue of that character, variant, rarity, type and assist status. They get that type, and 22 of
  them also get its Lv99 and tree max (Lv1 by the half-up rule, which equals ShoutWiki's Lv1
  wherever a page shows one). Conflicts are resolved for the value that fits the other page's Lv1,
  and the row's note records them. 2150_72, 4280_72, 5290_62 and 5330_72 have JoJoWiki as their only
  source. The 3 others keep default stats: for 4220_71 and 4220_72 both wikis give the same Lv99,
  and for 5000_62 the subpage copies another statue.
- **Held, not changed:**
  - 2000_28 and 4000_18: Gamerch is exactly 0.93 × JoJoWiki and ShoutWiki, which looks like a
    version change.
  - the Lv1 values for 2010_01, 3020_02, 4160_02 and 1020_02, where Gamerch differs from ShoutWiki.
  - 3041_04 and the 5000_x3/x4 Giorno ids, whose identity is unsettled.

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
