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
4. Builders merge them (`tools/content/build_statues.py`, `build_quests.py`, `build_extra_quests.py` (daily and event chapters; from Build 43 also the Labyrinth room costs and floor rewards of research-b43/stages_extra, command in its docstring, docs/research/22 §2.1), `build_minds.py`) →
   `server/content/statues.json`, `quests.json`, `minds.json`. A wiki value attaches to a game id
   only on an unambiguous match against the game's own Japanese labels (from the player's l10n
   export, compared after NFKC normalisation, so full-width and half-width forms agree); ids a wiki
   page may carry are not trusted. Everything else stays a marked default:
   - statues: Gamerch by leader-ability name (`ABI_LEAD_NAME_<id>`; assist-ability name
     `ABI_AST_<id>_N` for assist-only statues); ShoutWiki by elimination within a character, only
     when the entry keeps rarity in order within the id's variant family; then the JoJoWiki dump
     (`--jojowiki`) cross-checks them (see "Wiki cross-checks" below);
   - stages: Gamerch chapter title against `CHAPTER_NAME_<id>`, and only when the wiki lists as many
     stages as the game has (chapters 2, 6, 8 and 17 differ and keep defaults). From Build 43
     `build_quests.py --research` also reads the stages_story records of research-b33 and
     research-b43 (high/medium only; command in its docstring): AP and recommended level for
     CHAPTER_ST_23..28, star missions (`missions`), the tactical first-clear and special rewards
     (`rewards`, data only) and chapter-level facts (`wiki`, not served); each value names its
     page, capture and access date in the stage's `source` or `also` (docs/research/10 §3.9);
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
   `oldmode_statues.json`) and two research files (`research-b33/statues` and `/action`), from
   Build 43 also `research-b43/assists`; the command is in the script's docstring. Leader abilities
   are rebuilt first: `build_leader_abilities.py … --table` (docs/research/12 §4; from Build 43 it
   and `build_action_abilities.py` also take `--research research-b43/assists`). The leader builder replaces its
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
   `research-b33/action` and `research-b34/action`, from Build 43 also `--research-kit
   research-b43/action_skills`) → `server/content/action_skills.json`, and
   `build_skill_table.py` → `skill_data_table`. The commands are in the scripts' docstrings.
   Wiki names match the game's `SKILL_NAME_<p>_<NN>` labels strictly or, failing that, by a strict
   fuzzy rule. Statues without wiki names use their tactical skills, never in the skill or SP slot
   when that skill plays the cinematic. From rarity 5 their finisher is the single label at the
   cinematic entry (docs/research/30-action-battle.md §4–5). Build 43 kits (§4a there): Gamerch
   names for 34 more statues, ShoutWiki cool-downs/uses (its English names are not matched), 10
   held finishers resolved, and `"actionMode": {"usable": false}` (with its page) for the 10
   statues the wiki says cannot play the action battle; a kit slot the wikis leave out keeps the
   jdr-default pick, marked `"from": "jdr-default"`.

9. Action-battle leader and assist abilities, statue traits (Build 41, docs/research/30 §7): after
   the leader builder, `tools/content/build_action_abilities.py` → `server/content/action_abilities.json`
   (StatueInfo.leaderAbilityInfo, inline) and the `AF_*` rows of `affect_table` /
   `affect_text_rule_table`; then `build_assist_abilities.py` (step 6) adds the assist `"action"`
   params; then `build_statue_traits.py` → `server/content/statue_traits.json` (tribeTypeList);
   then `build_jojo_links.py` → `server/content/jojo_links.json` (TeamInfo.jojoLinkList, computed
   per team by the server; docs/research/26 §1). From Build 43 the traits builder also takes
   `--verdicts research-b43/links_traits_minds` (traits for statues no other rule covers, cited per
   row in `rowSources`) and the links builder `--research-b43` of the same file (link effects read
   from Gamerch comment screenshots, cited as `effectSource`). All share the line parser
   `tools/content/action_vocab.py`, keep existing table rows and are idempotent; the commands are
   in the scripts' docstrings.

## Notices (Build 42)

`server/content/news.json` holds the Notices screen's posts (our own words; no game or other
builds' text), rendered by `server/services/src/web.rs` into the three tabs
(docs/research/27-support-and-news.md §2):

- `posts[]`: `{id, build?, date, title, category, blocks[]}`, in display order (newest first).
  `category` is `update` (Update tab), `fault` (Fault tab) or `other` (Other tab). Update posts
  are `build-NNN` with their `build` and release `date` (YYYY-MM-DD); a post whose `build` is
  above the server's build is not shown yet.
- `blocks[]`: `{kind, text}` with `kind` `heading`, `paragraph`, `note` (small grey) or `banner`
  (`text` names one of our own images served next to the page, `[a-z0-9_-]`; none yet). Other
  kinds are skipped. `text` may use `<color=#RRGGBB>`, `<b>`, `<i>` and `\n`; anything else is
  shown as text.
- Each build adds one post at the top, in player-facing words (from STATUS.md "What changed").
  A `[[...]]` placeholder must not reach a shown post (test `notice_pages_have_no_external_urls`).
- The style is `server/content/notice.css` (ours).

## Town object unlocks (Build 42)

`server/services/src/town.rs::OBJECT_UNLOCKS` gives each of the 20 Town objects (ids and models
from the player's files) the Town Rank that unlocks it: one per rank from 2, the 21st capped at
Rank 20. The order is design from the Diamond Records PC playtest, used with its author's permission (its trophy list,
matched by model); no PC text or file is used, and its coin costs are not: objects stay free
because the 3.12.0 client has no object cost. Placement still needs Rank 8 (client rule;
docs/research/24-town.md §2).

## Main shop (Build 42)

`server/services/src/services/item.rs::catalog()` holds the goods of the six shops of the 3.12.0
client (ids, `SHOP_000NN_NAME` labels and `ShopTypeIcon_000NN` sprites from the player's files;
docs/research/15 §Shops). Goods names are the game's own labels with our translations
(`GOODS_NNNN_*` where one fits, else `ITEM_`, `STATUE_NAME_` and `MIND_NAME_`/`MIND_TEXT_`);
no text from other builds. Which goods each shop sells and every price are ours:

| Shop | Pays with | Goods (ours) |
|---|---|---|
| 1 Buy Diamond Coins | gold (item 20) | 1/6/12/30/60/90 coins (`GOODS_0001-0006`), 10,000 gold a coin; the client empties this shop offline (no store), so it is listed last |
| 2 Expand Mind Box | Diamond Coins | 5 slots, 10 coins (the box is at its offline maximum: no change) |
| 3 Restore AP | Diamond Coins | Sandwich 10 |
| 4 Diamond Coin Exchange | gold, Diamond Coins | the same coin packs for gold; keys and Rainbow Trophy 5, turtles 3/10/30/60, every statue 30/60/150/300 by rarity (owned: +1 boost) |
| 5 Memory Ring Exchange | Memory Rings | the materials as before; every ability Mind 10/30/100/300 by rarity (a new Lv1 copy) |
| 6 Buy BP Recovery Cola | Diamond Coins | the 5-BP cola x1/x5/x12 for 5/20/40 |

Diamond Coins are a real wallet in the shop (the coin packs, Town payouts, presents); nothing
else charges them. No PC price is used.

## Growth tree square kinds (Build 42)

`server/services/src/collection.rs::tree_squares` builds every statue's board (docs/research/13
§3). The kinds, their tile icons and their labels are the 3.12.0 client's (`TREE_*` constants,
`GRID_LABEL_*` / `Tree_Message_NN` with our translations); placement and amounts are ours. No
PC board, value or cost is used.

| Effect | Squares (ours) | Each |
|---|---|---|
| 1/2/3 ATK/DEF/HP | the three lines and two forks, as before | the stat's share of the tree max |
| 4/5 skill, SP skill | `A6`, `Af3` / `D6`, `Df3` for the skills the statue has | +2 levels |
| 9 finisher level | `P1`, `P2` after `Df3`, rarity 5+ statues with a finisher | +2 levels |
| 10 Mind capacity | `C1`-`C8` past the HP line, combat statues | +4 |
| 11 leader ability level | `L1`-`L3`, statues with a leader ability | +3 levels |
| 12 Diamond Coin | `G1`, `G2` at the ends | 1 coin, once, when opened |
| 13 assist skill level | `K1`-`K3`, assist-only statues | +3 levels |
| 14 awakening | `E`, last | nothing (cosmetic) |

Mind capacity is max(40, 16 + the opened CP squares): 40 until the seventh square, 48 with all
eight (the base of 16 is design from the Diamond Records PC playtest, used with its author's permission; the +4 squares and
the floor of 40 are ours). Levels are 1 plus the squares, capped at 10. Costs follow the depth
rule of docs/research/13 §3.

## Action-stage star missions (Build 42)

`server/services/src/quests.rs::action_missions` gives every action stage three star missions
(docs/research/11 §4). From Build 43 a story stage a wiki lists sends the wiki's three
(`missions` in quests.json): Gamerch 特別報酬条件 for the 86 matched stages and 第24章 (CHAPTER_ST_23),
JoJo Wiki's boss list for 第24章's final stage and its default rule and boss list for 第25章 and
stages 1-4 of 第26-29章, and the Bahamut forum (research-b43) for the final stages of 第26-29章
(wiki numbering; our CHAPTER_ST_(N-1)). `build_quests.py` maps the wiki
wording to the client's types (`MISSION_TYPES`; e.g. 14 clear the quest, 7 all team members
survive, 22 use an SP skill, 26/27 defeat the boss with a skill / an SP skill). The boss-stage
conflict (Bahamut: a regular skill; JoJo Wiki and Gamerch: an SP skill) keeps each stage's own
source and is noted on it. The other action stages (the four story chapters with a stage-count
conflict, daily and event stages) send 7 all team members survive, 8 defeat all enemies and
20 use a Skill: that choice is design from the Diamond Records PC playtest, used with its
author's permission; no PC text is used. The types and the labels are the 3.12.0 client's
(`QuestMission_JP:QuestMissionText_<type + 2>`, shown through our translations); `param` is
empty (the client's check has the thresholds built in). Cleared types are kept per
stage in `Progress.stars` (types 1-46) and the result screen echoes the client's list, as for
tactical stages.

## Action assist bonus (Build 42)

`server/services/src/services/misc.rs` (`ASSIST_*`, sent in `/common/get_system_config`) holds
the four `assistStatus*` ints the 3.12.0 client reads to give each assist a share of its own
HP/ATK/DEF in the action battle (docs/research/19-assists.md §3.1): add 100, count 5, step 20,
limit 45, so 10% unboosted, +2% per 5 boosts and 100% at 225 boosts. Source: Gamerch player
posts 101615739 (2017-11-24), 101635727 (2017-11-27) and 104155773 (2019-08-20). The tactical
assist constants (`dtbattle_damage_param`) are unchanged.

## Action-battle script cameras (Build 42)

The action-mode action sets request 88 script camera ids, 62 of which had no row in
`Set_el_camera_set_common.json` (the client then stays frozen in script mode). Rebuild
(docs/research/31 §10):

```
python3 -I tools/content/scan_camera_requests.py --data <files dir> --keys local/keys.json \
  --tool target/release/jdr-tool --out local/content/action_camera_requests.tsv \
  --prefix actionset_statue_ --prefix actionset_common --prefix actionset_gmc
python3 -I tools/content/build_camera_sets.py --requests local/content/camera_requests.tsv \
  --action-requests local/content/action_camera_requests.tsv --masterdata server/content/masterdata
```

Ids 1003–2303 get one of five shots of ours (`ACT_WINDUP`, `ACT_COMBO`, `ACT_STRIKE`, `ACT_WIDE`,
`ACT_RECOVER`), all straight behind and above the acting unit and glided into; 302–314 the win
shot, 315–320 its front-on variant. Ids and numbers only; nothing from other builds. Test:
`masterdata::action_battle_cameras_cover_the_action_sets`.

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
| Statue rarity, type, Lv99 | Gamerch (leader- or assist-ability name), ShoutWiki (CC BY-SA 3.0, by elimination), the Build 33 research pass (JoJoWiki CC BY-SA 4.0 and ShoutWiki pages matched by ability names, `tools/content/research_statues.py`) and, from Build 41, the JoJoWiki dump for 22 defaults whose character, rarity, type and assist status the wiki lists once; from Build 43 the research-b43 verdicts (3041_04's type, 3180_62 assist-only) | 230 of 292 with stats, 233 with a type (Build 43: +3041_04, 3180_62); the 62 others are mostly ★3/★4 villains and alternate-attribute versions no wiki lists |
| Statue Lv1, max tree | the same pages where they show them | Lv1 from a wiki page for 103 statues, Lv99 / 10 rounded half up for the other 127; max tree missing for 3 (2000_28, 2100_03, 4000_18: the wikis disagree) |
| Stats per level | linear Lv1→Lv99 | all |
| Tactical basic attack (`dtAttack`) | the statue's own action sets in the player's files (`SKILL_<psid3>_99` = ATTACK, else its lowest SKILL_NN) | all 266 (64 distinct ids) |
| My Page background per statue | 21 backgrounds in the player's files, chosen by JoJo part (stable per statue id) | default choice |
| Story chapters/stages/areas | player's files | 82 chapters (29 action, 53 tactical), 370 stages |
| Stage AP, rank EXP, gold, recommended level | Gamerch (Wayback snapshot, chapters 1–23 matched by title); Build 43: the stages_story research (Gamerch 第24章 live page, corroborated by the atwiki EXP table, for CHAPTER_ST_23; the atwiki row 旧モードストーリー 23章以降 for CHAPTER_ST_24..28) | 116 of 370 stages (86 + 30 with AP and recommended level only; gold blank on the wiki for 39 of the 86, so default); enemies default (the wiki shows them as images); the recommended level is used as the enemy level; the chapter-level EXP maximum (5000) of ST_24..28 and one unstated DT_ST_438 stage's numbers are recorded, not applied |
| Story star missions (`missions`) | Gamerch 特別報酬条件, JoJo Wiki, the Bahamut forum (Build 43) | 116 of 130 action story stages; the others send the default three (see "Action-stage star missions") |
| Tactical stage rewards (`rewards`) | atwiki ストーリークエスト (Build 43; 第N章 = CHAPTER_DT_ST_4NN where the stage counts agree) | 170 stages in 38 chapters: first clear Diamonds (quantity not given) and one special reward each (67 matched to an item id by the game's own label); data only, not served. Prologue to chapter 4, 7, 21, 33 and 43 held (stage counts differ) |
| Player rank: EXP per rank, AP max | Gamerch user-rank page | ranks 1–99 |
| Minds: rarity, cost, flat stats | Gamerch (name; from Build 41 also a loose key without punctuation for 17 Minds) | 434 owned; 396 have no cost on the wiki and take the cost every Mind of their rarity has there (★1 3, ★2 5, ★3 8, ★4 12; Build 33) |
| Mind Lv10 flat stats (`lv10`) | Gamerch MAX values; for 9 Minds the wiki leaves blank, the Diamond Records Preservation (PC) data (original-game screenshots at Lv10, an archived translation; cited per row) | 156 Minds (Build 41); the client does not read Mind hp/atk/def (docs/research/16 §9) |
| Assist-ability conditions | ShoutWiki requirement icons (statue pages and 3 Old Mode templates), the research passes (ShoutWiki and Gamerch statue pages, incl. the live Gamerch pages; Build 43 research-b43/assists) | 178 statues (Build 43: +3180_62, 4030_04, 4170_16; 175 in Build 41, 116 in Build 33); the others always active; 3 held (docs/research/19 §5.3) |
| Tactical leader abilities (`DT_LEAD_*`) | Gamerch leader numbers (Wayback dump; from Build 41 also live pages via the research pass); Build 43: ShoutWiki's Reversal-era MAX Lv. values where no Gamerch page exists (Lv1 = Lv10 = MAX) | 98 statues (Build 41: +1020_03, 1080_02, 1100_13, 3001_03; Build 43: +3041_03, 4000_23 released, +4031_18, 5030_04, 5080_04, 5150_04 from ShoutWiki) |
| Action leader abilities (`LEAD_*`, inline) | the same Gamerch leader numbers, all action-era lines (docs/research/30 §7) | 144 statues, 333 lines (133 with every line served; Build 43: +3041_03, 4000_23); gold/EXP, barrier and untraced lines held |
| Action-battle skills (`skillList`, `skill_data_table` roles) | Gamerch statue pages (b33/b34; Build 43 also the older-template pages), ShoutWiki cool-downs/uses and finisher names (Build 43, per-row `source`/`also`) | 160 statues with a wiki slot (Build 43: +47), 132 jdr-default; 18 held finishers (no wiki finisher name); 9 printed cool-downs with no slot to carry them |
| Assist action params (`"action"`) | the record each statue cites (Gamerch JP lines, ShoutWiki values) | 105 statues (Build 43: +3180_62); the others send the tactical row (DTAF_* mapped to AF_*) |
| Statue traits (`tribeTypeList`) | live Gamerch statue pages, ShoutWiki categories, family fill; Build 43 research-b43 verdicts (siblings' Gamerch sets, ShoutWiki categories; medium, cited per row) | 270 of 292 (Build 43: +8 research, +1 family fill); 13 more are sourced as no tribe (`noTribe`); 9 in 6 families (Part 5 tactical-only statues, 4280_72) have no source |
| JoJo Links (`jojoLinkList`) | the research pass's Gamerch link page and comments, the Bahamut thread, the saved Gamerch link table (links 12, 34); hand rule overrides (docs/research/26 §1) | 56 links served (5 by trait; Build 43: +53, 64, 76 from research-b43 team screenshots), 22 held (gold/EXP, conflict, name only, members unknown, low confidence; 54 has its effect but a probably-gender rule we cannot express) |
| Daily quest numbers | Gamerch day pages via the Build 33 pass | Normal tier of every weekday (AP 8, level 20) and Tuesday's EXP/gold (15 stages); the other tiers ours. Build 43: the 2019 atwiki confirms the weekday mapping (data only) |
| Labyrinth room costs, floor rewards (`quests_extra.json` `labyrinth`) | Bahamut floor screenshots (2018-04) and ShoutWiki (CC BY-SA 3.0), research-b43 (Build 43) | 36 rooms seen (high) and 15 more from floor 5 on by the cost-by-slot pattern (medium); floors 1-4 rewards (the floor-4 count ours); floor 1-4 rooms no screenshot shows, floors 5-20 rewards, area counts (data only) and the three grades ours. Event and Arena numbers from the same pass are data only (`wiki`) |
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
  - the 5000_x3/x4 Giorno ids, whose identity is unsettled.
  - 1090_03, 1100_03 and 3001_13: the only public page of each is its sibling's (1090_13, 1100_13,
    3001_03; Build 43), so they keep defaults.
- **Settled in Build 43** (research-b43/statues, see "Research collections"): the Lv1 values for
  2010_01, 3020_02, 4160_02 and 1020_02 (unchanged; each row's `lv1Note` cites the live Gamerch page,
  and 1020_02 is not fully settled) and 3041_04's type.

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

The Build 33 and Build 43 passes keep one fact per record (`results.jsonl`);
`tools/content/research_statues.py` turns them into a collection. From Build 43 it reads both files,
and the b43 verdicts apply (live Gamerch pages read 2026-11-22, cited per row):
- **3041_04** (★6 Anubis Polnareff): type Tactical by the verdict (ShoutWiki and the JoJoWiki template
  against one JoJoWiki gallery caption), so it gets the ShoutWiki ★6 page's stats (`typeSource`).
- **3180_62** (Mannish Boy): the live page marks it assist-only, so it gets its JoJoWiki/ShoutWiki stats.
  An assist-only statue takes research stats only with such a record (`assistOnlySource`). Its
  `DT_AST_3180_62` row now leans HP (from its stats) instead of the ATK placeholder.
- **Lv1 checks**: the four held Lv1 conflicts keep their values (Gamerch's 499, 762 and 441 look like
  typos; for 1020_02 both pages show the Lv1 ATK as DEF). A verdict that disagrees with our value is
  reported, not applied.
- **identity** records leave 1090_03, 1100_03 and 3001_13 out (their b33 records are their siblings'
  data). Records with no statueId (Tonio and Boingo, whose ★4 page could be either id) are not read;
  their ids stay as Build 41 has them (4090_72, 3230_62).
- **Assists and leaders** (`research-b43/assists`, read by the leader, action-leader and assist
  builders; details in docs/research/19 §5.3 and 12 §4): 3041_03 and 4000_23 leave `RESEARCH_HELD`
  (live pages re-read); 3180_62, 4030_04 and 4170_16 leave the assist `HELD` (identity by the
  `ABI_AST_3180_62_N` label; ShoutWiki requirements read in the page source); 3230_62, 4090_72 and
  4220_72 stay held (Gamerch prints no condition, ShoutWiki gives one; 4090_72's id is settled).
  The ★6 Koichi page filed under 4011_15 is refiled to 4011_18 (`REFILE`). Four Tactical-era ★6
  statues get ShoutWiki's Reversal-era MAX leader value. Not used: the medium 5100_14 conflict (+9%
  vs +14%), 4011_17's derived values, 4170_16's and 4030_04's English MAX lines (no tactical affect,
  or a 2022 revision), and the `assist_ability_absent` rows (no source is not no ability).

The statue command (the inputs in `local/`; the collection is written anywhere, here `$T`):

    python3 -I tools/content/research_statues.py \
      --facts local/wiki-raw/research-b33/statues/results.jsonl \
      --facts local/wiki-raw/research-b43/statues/results.jsonl --out $T/extra_statues.json
    python3 -I tools/content/build_statues.py --scan local/content/statues_scan.json \
      --export local/l10n/export-b33.tsv --shoutwiki local/wiki-raw/shoutwiki/statues.json \
      --gamerch local/wiki-raw/gamerch/statues.json --jojowiki local/wiki-raw/jojowiki/statues.json \
      --clips local/content/figure_clips.tsv --heights local/content/model_heights.tsv \
      --pose local/content/team_pose.tsv \
      --extra local/wiki-raw/gpt-2026-11-15/JoJoDR_Public_Data_2026-11-15/statues.json \
      --extra $T/extra_statues.json --accessed 2026-11-15 --out server/content/statues.json

Then rerun `build_assist_abilities.py` (step 6) and `build_extra_quests.py`: their enemy pools and
default assist kinds read statues.json.
