# Status

Newest build first. Nothing here is "fixed" or "working on device" until a device log shows it.

## Build 24 — Arena fix, ability effect lines, assist abilities, the loading atlas (branch `wip/b24`)

### What the Build 23 device test and owner showed
- Everything worked except the Arena, which loaded forever (device log: a NullReferenceException
  in `PvPHomeData.SetRewardInformation`).
- Build the rest of what was pending.

### What changed
- **Fixed: the Arena home hang.** The reward panel builds each reward's thumbnail from
  `ItemInfo.icon`, which our reward items did not send. Every reward and goods item set
  (missions, Labyrinth, Arena, events, shops) now sends a complete ItemInfo.
- **Ability effect lines** (docs/research/16 §8): Mind detail, leader ability, formation, pause
  and support screens show effect lines such as "ATK+5.0％". Our `affect_text_rule_table` fills
  the table the game ships empty, and every ability affect names an existing game label.
- **Assists** (docs/research/19 §5): assist abilities (16 shared rows; 35 statues from ShoutWiki,
  the rest a default; ours) and the assist character's skill (button 6, the statue's own normal
  skill; ours). The 3.12 tactical battle never applies assist abilities, so the server copies a
  seat's active ones onto the seat statue's own abilities (ours).
- **The redrawn loading-screen atlas** is applied in the APK: the patcher asked for a Sprite type
  the file does not have. It now asks only for the types each file has and proves every object
  round-trips before editing.
- **Arena, finished** (docs/research/23):
  - `/pvp/home` sent `pollingTimes 0`: the client re-polls every frame with the buttons locked.
    It now sends 30 s (ours); likely a second cause of the hang.
  - Rankings: the Arena board ranks the player among 999 AI rivals per season, and event 50045
    has its own board (the three `event/get_event_ranking_*` routes).
  - Defence: after 15 minutes away, up to 3 simulated attacks on the Arena teams, shown as the
    client's defence messages.
  - Crazy Diamond: a gauge filled by durability lost; when full, the Full Revive button revives
    every resting team.
  - A battle that was started and never ended counts as a loss, with the client's
    "connection lost" notice. Win streaks raise ATK/DEF (+2% per win, up to +10%).
  - Every Arena and ranking reply is checked in tests against the client's own class fields: no
    string, list or object it reads is missing or null.
  - The values are ours.
- Build number 24, server marker `JDR_EMBEDDED_SERVER_b024`.

### Device checklist (expected; to be confirmed on a device)
- [ ] The Arena home loads; opponents, a battle and its result
- [ ] Mind detail shows its effect line ("ATK+5.0％" etc.); leader abilities show theirs
- [ ] A team with a leader assist: the Assist button in tactical battle and its skill; battles
      without assists unchanged
- [ ] The loading screen shows the redrawn English atlas
- [ ] Arena rankings (top 100 and around me); event 50045 ranking button
- [ ] After 15+ minutes away: defence messages on the Arena home; Full Revive when the gauge is
      full

## Build 23 — full retranslation with the story, Labyrinth, Arena and the remaining systems (branch `wip/b23`)

### What the owner asked and sent
- 46 more redrawn images (all merged; the art pack now has 510).
- Build everything not built yet.
- A full retranslation that reads like an official Bandai Namco localization, text only; for the
  art, a list of the images whose wording should change, with suggested text.

### What changed
- **Retranslation** (docs/LOCALIZATION.md, "The Build 23 retranslation").
  - Every string retranslated from the Japanese: 30,719 unique strings, 65,428 entries.
  - A style guide and glossary based on Bandai Namco's English JoJo and mobile games. Original
    Stand and character names and "Menacing", by the owner's choice.
  - **The story is in English for the first time**: 14,259 dialogue lines from 1,609 scene
    bundles (new `script:` keys).
  - UI labels checked against their room; 55 shortened.
- **Art.** All 510 redrawn images reviewed against the new style: 422 kept, 80 sent back as a
  correction kit with suggested text (private), 8 ruby letters blanked.
- **Labyrinth** (docs/research/22): the tile opens; 20 floors of rooms that reuse tactical stages,
  Mentality (300, refilled daily), 3 Souls, Tarot select and flip, rewards in the Message Box on an
  escape or a full clear. Contents and rewards ours.
- **Arena** (docs/research/23): an always-open season against AI teams built from the roster (the
  2019 rule set): a separate Arena roster, opponents and shuffle, durability and rest, Fever,
  streaks, point and season rewards in the Message Box. Values ours.
- **Mind levelling** (docs/research/16 §7): Minds level up with Mind medals or other Minds, with the
  client's EXP and gold rules. It crashed before (empty reply).
- **Auto team** (the auto formation popup): the best three statues by total, ATK, DEF or HP, with
  Minds.
- **Friend Points**: 10 per quest clear and from missions; the Friend Point Mind Summon.
- **Growth tree costs**: each square costs a tree material of the statue's colour and gold; fork
  squares need an Expansion Key (amounts ours).
- **Event point rewards**: event 50045 earns points; its ten rewards arrive in the Message Box as
  the points reach them.
- Not built: the Mind/ability effect line (the client's effect text rules ship empty), assist
  abilities and skills, Arena defence against other players, rankings.
- Build number 23, server marker `JDR_EMBEDDED_SERVER_b023`.

### Device checklist (expected; to be confirmed on a device)
- [ ] Menus, items, skills and missions read in the new English; story scenes show English dialogue
- [ ] No label spills out of its box (Town, Summon, Options and Team screens especially)
- [ ] Labyrinth: the tile opens; pick a room, Tarot, battle, next floor; escape pays into the
      Message Box
- [ ] Arena: the button on My Page; opponents; a battle and its points; durability and rest
- [ ] Mind levelling with a Mind medal and with a Mind; no crash on the result screen
- [ ] Auto team fills three statues and Minds
- [ ] Friend Point Mind Summon after a few quest clears
- [ ] Growth tree squares show and take their costs; a fork square needs a key
- [ ] Event 50045 shows points; rewards arrive in the Message Box

## Build 22 — Message Box, Mind abilities, assists, event exchange, box and Count Up summons (branch `wip/b22`)

### What the owner asked
- Fix the "SUCcess" lettering and look for leftover Japanese art (summons, events and others).
- Continue with the Build 22 features: Mind abilities, assists, Message Box, event exchange, box
  and Count Up summons.

### What changed
- **Art** (outside the repo; the player's art pack).
  - "SUCCESS" letters rescaled to one height in all three result sets.
  - A private redraw kit for the 71 images that still show Japanese, with sizes, meanings and
    instructions.
  - The duplicate-summon caption was not found in any text table, code string or texture.
- **Message Box** (docs/research/18).
  - Waiting and received tabs, pages, receive one or all.
  - Mission rewards go there, as the Missions screen says, plus a one-time welcome gift
    (100,000 gold and 500 Memory Rings).
  - The Home badge counts waiting gifts.
- **Mind abilities** (docs/research/16 §6).
  - Equipped Minds now raise HP, ATK or DEF in tactical battle: +2/3/5/8% by rarity (ours; no
    numbers survive).
  - The Mind detail screen's effect line stays blank (the game has no plain "ATK +x%" label).
- **Assists** (docs/research/19).
  - Up to 3 assist statues per team seat: equip, move, clear.
  - They add a share of their own stats in tactical battle (the client's formula, our
    constants).
  - Assist abilities and the assist character's skill are not built yet.
- **Event exchange** (docs/research/20).
  - Event Home's exchange button opens a shop of materials priced in Fight Coins.
  - Each event-quest clear drops 20 Fight Coins.
  - The currency and goods are ours.
- **Box and Count Up summons** (docs/research/21).
  - Statue Box Summon: 50 statues per box (1 five-star), with Reset and Box Status.
  - Count Up rounds 1–5. A round ends after 20/30/50/50/50 pulls, and its Golden Summon Ticket
    arrives in the Message Box.
  - The ticket button draws one statue of four stars or better.
  - Round 5's 10x ends in a five-star.
- Build number 22, server marker `JDR_EMBEDDED_SERVER_b022`.

### Device checklist (expected; to be confirmed on a device)
- [ ] "SUCCESS" letters are even on the level-up result
- [ ] Message Box: the welcome gifts show; receive one and all; the received tab; the Home badge
- [ ] A mission claim puts its reward in the Message Box
- [ ] An equipped Mind raises the statue's stats in a tactical battle
- [ ] Assists: equip, move and clear on the team screen; the team still loads into battle
- [ ] Event Home shows the exchange button and the Fight Coin count; an event clear adds 20; a buy
      works
- [ ] Box Summon: pulls lower the count; Box Status opens; Reset refills
- [ ] Count Up: after 20 pulls round 2 shows; the ticket is in the Message Box; the ticket button
      draws

## Build 21 — Missions, Mind equipment, items and enhancement (branch `wip/b21`)

### What the owner asked
- Start building the systems the audit found empty or stubbed.

### What changed
- **Items and enhancement** (docs/research/15).
  - 327 real items: each has an icon and a name in the player's files.
  - A one-time starting stock (materials, turtles, tree materials, keys, 3,000 Memory Rings) and
    1,000,000 gold, for testing.
  - Statue enhancement with materials uses the client's EXP (×1.2 matching attribute, ×5
    platinum) and gold arithmetic; turtles add a boost.
  - Fixed: every material cost 1 gold (gold ratio 0); it now costs as listed.
  - The Ring Exchange sells materials for Memory Rings.
- **Mind equipment** (docs/research/16).
  - Minds are equipped on team seats: slots 1–4, capacity 40.
  - Moving a Mind to another seat of the team takes it off the first; "remove all" works.
  - Locked slots, unknown Minds and capacity overruns are refused with the game's own messages.
  - Fixed: tapping the heart (lock) crashed the client; it now stores the lock.
  - Equipped Minds do nothing in battle yet: 3.12 uses only Mind ability rows, which come later.
- **Missions** (docs/research/17).
  - Daily missions (clear 1/5/10 quests, earn 100/200/300 EXP) and 14 regular missions, with the
    game's own text and the wiki's rewards.
  - The server counts progress; daily missions reset at 04:00 JST.
  - Claim one or all; the Home badge shows claimable missions. Rewards arrive on the account
    directly (the message box is not built yet).
- Build number 21, server marker `JDR_EMBEDDED_SERVER_b021`.

### Device checklist (expected; to be confirmed on a device)
- [ ] Items list shows materials and rings; enhancing a statue with medals raises its level and
      costs gold
- [ ] A turtle of the statue's rarity adds a boost (+1)
- [ ] Ring Exchange lists goods; buying takes rings and adds the item
- [ ] Equip, move and remove Minds on the team screen; the heart lock works without a crash
- [ ] Missions: today's dailies; a quest clear makes "Clear a Quest" claimable; claim and claim
      all; the Home badge

## Build 20 — Reversal, daily and event quests, research audit (branch `wip/b20`)

### What the owner asked and sent
- Daily quests, event quests and campaigns enabled, to test more content.
- A research pack (118 documented behaviours) and 13 more redrawn tip pictures.

### What changed
- **Audit** (docs/research/14). Every documented behaviour is mapped to our code, with
  priorities.
- **Reversal on** (docs/research/11 §5).
  - The gauge fills from skills (10/15/20%), hits taken (10%) and statues lost (25%).
  - Reversal gives 3 actions at 10× speed with no cool-downs; a boss kill in the last area turns
    the remaining actions into overkill.
  - Field meanings are from the client; the values are ours.
- **Quest Select** (docs/research/13): today's Daily tile, four Event tiles and their Campaign
  Info entries.
  - 72 daily and event chapters and 222 stages, with the game's own names and banners.
  - Stages use story maps; enemies and rewards are ours.
- **Summon:** the tenth draw of a 10× summon is the guaranteed slot (★4 98.5%, ★5 1.5%; launch
  guide).
- **Art pack:** 13 more tip pictures (32 of 66 now redrawn). These say "Fighting Spirit" where the
  rest of the game says "Spirit".
- Build number 20, server marker `JDR_EMBEDDED_SERVER_b020`.

### Device checklist (expected; to be confirmed on a device)
- [ ] Reversal gauge fills; tapping it at 100% gives extra actions; a boss kill in Reversal shows
      overkill
- [ ] Daily Quests tile shows today's banner and its two quest lines; stages play
- [ ] Event tiles open Event Home with their chapters; stages play and return to Event Home
- [ ] Campaign Info shows the banners; a tap opens a page
- [ ] New tip pictures on the loading screen

## Build 19 — English that fits the UI (branch `wip/b19`)

### What the Build 18 device test and owner showed
- The team view works.
- Some English overflows its UI, and some UI shows no text at all.

### What changed
- **Measuring (new `tools/l10n/ui_fit.py`).** Every one of the 4,863 `UI.Text` labels in the
  player's UI bundles is laid out the way Unity's text generator does it: the box comes from the
  prefab's RectTransforms, and the text is measured with the label's own font from the player's
  files (FOT-NewRodin Pro, FOT-Matisse ProN, IPAex Gothic), with wrap, line height and best fit.
  The original Japanese fits in 98.2% of the measurable labels, which checks the measurement.
  - It includes 402 text-table rows that labels show through `BaseTextBox.defaultTextLabel`.
  - Labels that size themselves (a ContentSizeFitter or a layout group) are compared with the
    room the Japanese and the button art leave.
- **Shorter English** where it overflowed (docs/LOCALIZATION.md, "Fitting the UI"):
  - 89 UI labels, for example "Ability Description" → "Ability Info",
    "Commercial Transactions Notice" → "Legal Notice", "Summon Details" → "Details",
    "Withdraw Application" → "Cancel" and "Achievements" → "Records";
  - about 3,000 text rows by consistent rules: "X Stone Shard" → "X Shard", "Calendar Slab" →
    "Slab", Easy/Normal/Hard for 初級/中級/上級 (only where the Japanese has them), "Tarot: …",
    "Memorial Mind", "Elite Raid", (S)/(M)/(L)/(XL) in buffs and skill texts, shorter skill
    categories ("• Fire attack on one foe"), and a few menu, shop, Town and login-bonus labels.
- **Best fit for runtime labels.** Until now only labels whose baked text we replaced could shrink.
  The new `server/content/l10n/ui_layout.json` (label ids only) gives best fit to 1,015 labels
  that the game fills at runtime. It also gives wrap and best fit to 23 labels that spilled
  sideways. All of them are in bundles the server already rebuilds; the generator version is 3,
  so cached English bundles are rebuilt once.
- Remaining: 11 self-sizing labels within about 6% of their room, and 4 labels that shrink to
  54-71%.
- **Second device test (screenshots).**
  - Empty star rows in the stage window and on the results screen, and a blank tip board on the
    loading screen: the server sent `starList: []` and `questTips: {}`.
  - Tactical stages now carry three missions: clear the quest, all members survive, and clear
    within N turns. They use the game's own mission labels, the client checks them itself, and
    the stars stay lit per stage once cleared.
  - Each stage also has a loading tip: one of the game's 66 tips with a title, text and picture
    (docs/research/11 §4).
- **Win shot** (the owner asked to redo the win poses).
  - The result camera aimed at a fixed 2.2 m, so winners sat low in the frame and small ones left
    it.
  - It now follows each winner's chest and neck bones from 3.2 m (docs/research/12 §5).
- **Art pack:** "First Clear Rewards" runs into the "Special Rewards" tab.
  - It is the redrawn sprite `ui_bizarre/sprites/Bizarre_005_lbl_000_2.png` (174×40). The
    original lettering also spans the full width.
  - A redraw with about 12–15 px of free space on the right (or "First Clear") fixes it.
- Build number 19, server marker `JDR_EMBEDDED_SERVER_b019`.

### Device checklist (expected; to be confirmed on a device)
- [ ] Menus, buttons and popups: no text over the button art or outside its box
- [ ] Labels that were blank now show text (please name any that are still blank)
- [ ] Long names (skills, Minds, places) shrink to fit instead of overflowing
- [ ] Stage window and results: three mission rows with text; stars light for cleared missions
- [ ] Loading screen: tip title, text and picture
- [ ] Win screen: the winner centred, small characters in frame

## Build 18 — team view fixed, research data merged (branch `wip/b18`)

### What the Build 17 device test and owner showed
- Everything works, but team-screen statues were sometimes out of frame or T-posing.
- The owner gathered a research collection with an outside assistant (statues, systems).

### What changed
- **Team view** (docs/research/12 §7).
  - The bust-up pose clip is chosen by rarity. Statues with no wiki rarity (we defaulted to 4,
    which has no clip for 16 of them) now take a rarity whose pose clip exists in the player's
    files (120 statues).
  - Each statue is framed from its actual team pose: the pose is evaluated and skinned from the
    player's files, the head is centred over its slot (`teamPos.x`), the head bone sits at
    1.45-1.70 m (scale 50-150%), and very short figures are lifted (`teamPos.y`). This replaces the
    first pass, which used bind-pose heights; the device still showed statues off centre.
- **Research collection** (docs/CONTENT.md):
  - stats for 5 more statues (145 of 266 now have wiki stats), cited per ShoutWiki page;
  - 11 existing matches confirmed with identical numbers;
  - evaded hits deal 1/4 damage (wiki).
- Build number 18, server marker `JDR_EMBEDDED_SERVER_b018`.

### Device checklist (expected; to be confirmed on a device)
- [ ] Team setup: no T-poses; small models (Iggy, Pet Shop, Enya, Shigechi) and Tarkus framed
- [ ] Star counts look right for the statues whose rarity now comes from the files

## Build 17 — every visible text in English, Stands no longer overlap their user (branch `wip/b17`)

### What the Build 16 device test and owner showed
- Attackers stop in front of their target, but Stands sometimes overlapped their own user.
- Draws and their cutscenes work, and the win quote shows.
- Some text was still Japanese: loading screen text, minor descriptions and buttons. It is real
  text, not art.

### What changed
- **Stands** (docs/research/12 §6). A Stand appears on its user and rushes about 2 m forward,
  stopping 1.05 m from the target. Stand users now stop 2–2.5 m away; units without a Stand stop
  1.2–1.5 m away.
- **The last text** (survey: 0 Japanese left in the exported UI; the 883 "Japanese" labels the
  check reported were empty runtime-filled fields):
  - The 356 chapter-description and dungeon-UI rows are translated. All 51,081 text-table rows
    are now English.
  - New patch `english-apk-text`:
    - It rewrites the 72 Japanese labels in the 11 scenes built into the APK (loading overlay,
      download screen, title, save select, errors). It reads and writes them field by field and
      refuses anything else; other objects stay byte-identical.
    - It also rewrites 22 visible C# string literals (Follow buttons, rank and days-left text,
      "until" dates, "Saving...", yen, purchase errors). Literals longer than the original are
      relocated in global-metadata.dat.
    - Every site is checked against its source hash, and a post-check confirms 72 of 72 labels
      and 22 of 22 literals.
  - The `minigame_01` hand names are exported and translated (33 bundles are rebuilt).
  - `l10n_check.py` counts empty labels separately and fails on any Japanese UI text.
- Build number 17, server marker `JDR_EMBEDDED_SERVER_b017`.

### Device checklist (expected; to be confirmed on a device)
- [ ] Stands rush out in front of their user
- [ ] The loading overlay, download screen, title and save select are in English; the long
      download paragraphs fit their boxes (best fit is off)
- [ ] Follow buttons, PvP rank text and banner dates are in English

## Build 16 — the full non-story translation, attack spacing, draw buttons, win quotes, a real tree (branch `wip/b16`)

### What the Build 15 device log and owner showed
- Battles are seen from the side, but attackers walked into their targets.
- Summon, Shop, the growth tree and the dictionary open, but Summon had no draw button.
- The growth tree works but should be a real tree.
- The win screen showed placeholder quotes.
- Owner's request: finish the text translation, minus the story.

### What changed
- **Translation.** 17,151 more rows are in English: effects and buffs, achievements and missions,
  campaign/inbox/login-bonus messages, skill texts and categories, item/mind/character
  descriptions, win quotes, and Town/glossary/tips.
  - The six batches follow local/l10n/GUIDE.md and the glossary, and were checked for
    placeholders, line breaks and leftover Japanese.
  - 50,725 of 51,081 text rows are now translated. The 356 left are story: chapter, event-chapter,
    daily and dungeon scene text.
- **Attack spacing.** Skill `dtRangeMin`/`dtRangeMax` are centimetres
  (`CharacterProxy.SetSkillTargetPos` 0xEE9304); 0/0 walked attackers into the target's centre.
  - Melee now stops 1–1.5 m away, bullet users 4–8 m, and heals move only when the ally is far.
  - Move ranges and the bystanders' escape distance are also centimetres, and are fixed.
- **Summon buttons.** Pay type Cash (4) draws no play buttons (jump table 0x223E70C); the banners
  now use Free (2) at price 0.
- **Win quotes.** `battleResultComment` is the game's own `Result_Chara_Comment_<statueId>`
  label (264 statues), now translated.
- **Growth tree.** Three branches from the start square (ATK, HP, DEF), with forks.
- Build number 16, server marker `JDR_EMBEDDED_SERVER_b016`.

### Device checklist (expected; to be confirmed on a device)
- [ ] Attackers stop in front of their target
- [ ] Summon shows single and 10-draw buttons, and a draw plays
- [ ] The win screen shows the character's quote in English
- [ ] Skill, effect, item and achievement texts read in English; the growth tree branches

## Build 15 — battle from the side, menus open (branch `wip/b15`)

### What the Build 14 device log and owner showed
- The Growthbeat crash is gone, the results screen works, and specials work.
- Battles were seen from behind, and the win shot was wrong.
- Summon, Shop and the growth tree loaded forever ("probably a lot of menus").

### What changed
- **Battle camera** (docs/research/12 §5).
  - The turn camera is world-fixed on −Z, so the teams now face each other along X: players on the
    left, enemies on the right, spread in depth.
  - Camera sets now cover all 266 ids the statues' animations request. A missing id had frozen the
    camera. Shots use a side view, and the win uses a close three-quarter shot.
- **Menus** (docs/research/13):
  - Summon: a statue and a mind banner, free draws by the published rates.
  - Shop: opens with one shop and no goods.
  - Growth tree: a generated tree per statue; squares are free, saved, and add stats.
  - Dictionary: every statue and mind.
  - Enhance, evolution, sell, inbox and trophy claims, and protect/sort: stubs, so the screens open.
- **Profile:** `user/update_comment` saves the profile message.
- Build number 15, server marker `JDR_EMBEDDED_SERVER_b015`.

### Known gaps
- The shop sells nothing yet.
- Enhance, evolution and sell change nothing (no items or materials yet).
- The tree layout and the camera shots are ours.

### Device checklist (expected; to be confirmed on a device)
- [ ] Battle seen from the side; the win shot shows the winner
- [ ] Summon opens, and a single and a 10-draw play their animations
- [ ] Shop, growth tree (open a square: stats rise) and dictionary open
- [ ] The profile message is kept

## Build 14 — results screen, specials for every team, leader abilities, no Growthbeat (branch `wip/b14`)

### What the Build 13 device log showed
- Battles finish, all animations play, and normal skills work.
- The results screen hung: a NullReferenceException in `BattleResultHighScoreUIController.Setup`.
- Special skills could not be used.
- Owner's request: stub the Growthbeat crash at startup.

### What changed
- **Results screen:** `bizarre_end` sends `oldEventRankingResultInfo {point 0, eventDataList []}`.
  The high-score panel reads that list without a null check (0xC0D4B4).
- **Specials:** no client gate blocks them for our data. The default team's statues simply had no
  special, because their second skill's text fell outside the damage/heal classification. Skills
  with no damage, heal or buff wording are now sent as single-target hits, so 237 of 266 statues
  have a special (229 with two extra skills). A special has 3 uses per battle; empty specials
  refill only in reversal mode, which is off.
- **Leader abilities** (docs/research/12 §4): 88 statues get `DT_LEAD_<statueId>`, with the
  wiki's numbers in client-verified per-mille units and 21 new affect rows. The leader's ability
  applies to the whole team.
- **`no-growthbeat` patch** (owner's request): `GrowthbeatBehaviour.Awake`, `SendTag` and
  `SendEvent` return at once (fingerprinted, with a post-check, `--disable no-growthbeat`).
  The SDK is never started, so its thread no longer crashes offline and nothing is tracked.
- Build number 14, server marker `JDR_EMBEDDED_SERVER_b014`.

### Device checklist (expected; to be confirmed on a device)
- [ ] No Growthbeat FATAL EXCEPTION in the log at startup
- [ ] The results screen continues after Confirm
- [ ] The special-skill button works for Jonathan, Jotaro and Josuke
- [ ] Leader ability effects (e.g. HP up) show in battle stats

## Build 13 — the master data filled (branch `wip/b13`)

### What the owner asked
"Implement everything missing in the master data table minus the dead one" (`signboard_table`).

### What changed (docs/research/12)
- **Startup tables (11):**
  - glossary (167 terms);
  - characteristics;
  - full 6×6 type-affinity matrices (the game's own chart, our percentages);
  - ora-ora mode (the client's constants);
  - affects (the 3 the client names);
  - Towns (levels and objects);
  - effect reduction, status abnormality and characteristic skills (neutral).
- **Battle tables (11):**
  - tactical and action reaction tables: hit and death animations. Without the death rows a killed
    unit was never retired, so battles could stall;
  - `model_table` (210 models);
  - `bullet_parameter` (394 bullets; 7 statues' attacks fired bullets that died at launch);
  - action-battle damage, combo and PvP status parameters.
- **Core tables:**
  - script cameras for every camera id the client hard-codes. Without them the tactical stage
    finish and result probably wait forever;
  - team rank (the client's formula, our thresholds);
  - mind levels;
  - the classic skill table (1,109 rows by label key).
- **Tactical skills:** 382 normal and special skills (188 statues get 2 extra buttons, 73 get 1),
  with the game's own names, texts, categories and animations. The damage rank table now has 256
  entries, so a skill's rank sets its power (attack ×1.0, normal ×1.25, special ×1.5).
- **Left out, with reasons in doc 12:**
  - `statue_special_scene_data_table`, `assist_ability_condition_table`,
    `map_gimmick_pattern_table`, `affect_text_rule_table`, `action_branch_data_table`: empty, since
    no source exists and a wrong row can crash or spawn the wrong actor;
  - `skill_create_character_table`, `additional_statue_load_table`: absent; summon and preload
    only, no source;
  - leader abilities: pending until their value units are verified.
- Build number 13, server marker `JDR_EMBEDDED_SERVER_b013`.

### Known gaps / risks (expected)
- The statue-to-skill pairing is inferred (a variant may show a sibling's move).
- Special skills might need a gauge or cut-in camera we have not traced. If they stall, they can be
  sent as normal skills.
- The affinity percentages, all tuning numbers and the camera shots are our own choices.
- The new tables have not run on a device; the next log decides.

### Device checklist (expected; to be confirmed on a device)
- [ ] Boot and My Page as before (the startup tables load)
- [ ] Battle: hit and death animations; a killed unit leaves; the stage finish and result screens
      appear
- [ ] Skill buttons show names and texts; a normal skill and a special skill can be used
- [ ] Glossary shows terms; the team rank letter appears on the formation screen

## Build 12 — real attacks, working turns and damage, team member changes (branch `wip/b12`)

### What the Build 11 device log showed
- The My Page leader was in frame, and leader changes stuck.
- Changing a team member made the team vanish (`team/statue_select` had no route).
- The tutorial battle loaded with units placed. But no animations played, every hit did 1 damage,
  turns never ended, and the stage had no description or rewards.

### What changed
- **Team:** `team/statue_select` puts the chosen statue in the seat. A team member swaps seats,
  a side seat can be emptied but the leader seat never can, and both battle types follow.
- **My Page:** the leader is 12% closer, at (72, -113, 322) cm, as the owner asked.
- **Battle** (docs/research/11 §3):
  - the damage spread is now sent in the client's 1/10000 unit (we had sent percent, so every
    hit did 1 damage);
  - attacks wait 100, a unit change 50 and cancel 0, so turns rotate;
  - story enemies use the attack-only AI `zako01_cmn`;
  - every statue attacks with an action set of its own model: `SKILL_<psid3>_99` (99 means
    ATTACK) for 263 statues, `SKILL_204_01` for the 3 whose model has no ATTACK action, picked from
    the player's files, with one skill row per attack id (64);
  - the stage window lists the stage's gold, and tactical chapters show the game's own chapter text.
- Build number 12, server marker `JDR_EMBEDDED_SERVER_b012`.

### Known gaps / risks (expected)
- Damage numbers and turn waits are our own choices.
- Statues have only their basic attack: no special skills or abilities yet.
- Statues start at level 1, so later stages may be too hard.
- Tactical stages have no description label of their own in the game's text.

### Device checklist (expected; to be confirmed on a device)
- [ ] My Page leader closer; changing a team member sticks
- [ ] Battle: attack animations play, damage looks normal, enemies take turns, the battle ends
      and the result screen appears
- [ ] The stage window shows the gold reward and the chapter text

## Build 11 — team view, My Page leader, first tactical battle tables (branch `wip/b11`)

### What the Build 10 device log showed
- My Page had its background, but the leader model was off frame. The character list worked.
- Team Setup showed no 3D statues, and Unity logged step-offset warnings from the pedestal code.
- Pressing Battle on a story stage hung. `bizarre_start` answered, then `dtbattle.ReversalManager.Init`
  threw a NullReferenceException (`dtbattle_damage_param` missing), and the battle update threw on
  every frame.
- The profile card, Set as Leader, followers, blocks and trophies called routes we did not answer.

### What changed
- **Positions** (docs/research/11 §1): the client reads positions in centimetres and
  `teamScaleScalar` as a percentage. Our 1.0 made team statues 1% size, which most likely hid
  them (not yet confirmed on a device), so it is now 100. The My
  Page leader stands 3.66 m in front of the `CameraIdSet0` camera, facing it. Rows also set
  `mindFrameID` 1 and the PvP AI ids to 1.
- **Tactical battle tables** (docs/research/11 §2), new master-data content:
  - `dtbattle_damage_param`: one row of chosen tuning values, reversal off;
  - `dtbattle_damage_rank_table`;
  - `dtbattle_skill_data_table`: the client's fixed skills, our basic attack `JDR_ATK_00` and
    three categories;
  - an `area_position_pattern_table` row named in every tactical area's `formation`.
  
  Statues in teams (and enemies) carry the basic attack; statue lists and detail views do not,
  since their skill views were not traced. Tactical enemies are not bosses for now.
  The SecurityStringValue encoding is implemented. The real-data proof
  (`tools/proofs/masterdata_check.py`, run by `scripts/build.sh`) decodes it back with its own
  implementation of the client's reading; this is the same reading, re-implemented, not a second
  source.
- **Routes:** `team/leader_change`, `team/set_team` (both battle types follow; a team number outside
  1–5 is refused; edits do not rotate save backups), `user/user_info_mini`
  (the card shows the main leader), `follow/follower_list`, `follow/block_list`,
  `trophy/trophy_list`, `info/notify_list`, `gasha/gasha_list` (empty).
- Build number 11, server marker `JDR_EMBEDDED_SERVER_b011`.

### Known gaps / risks (expected)
- The battle may get further and stop at the next missing piece. Our basic attack has no
  animation id, the AI uses the client's `playerauto` default, and all damage numbers are our own
  choices. The device log will show the next failure.
- My Page and team positions come from scene measurements, not the original values, so some very
  tall or very small models may sit oddly.

### Device checklist (expected; to be confirmed on a device)
- [ ] My Page: the leader is in frame
- [ ] Team Setup shows the three statues; Set as Leader sticks after leaving and returning
- [ ] Profile card opens
- [ ] Story → a stage → Battle: how far it gets (field shown? units placed? Attack button? a hit?)

## Build 10 — characters, teams, story quests from the wikis (branch `wip/b10`)

### What the Build 09 device log showed
Redrawn English art on screen; the My Page background was black (no team: the background and the
leader model come from the tactical main team's leader and its `statue_setting_table` row).

### What changed
- **Collection:** the player owns every obtainable statue (266: model, animations, action set and
  leader ability present in the player's files) at level 1, and 5 teams per battle type; the
  tactical main team (Jonathan, Jotaro, Josuke) is sent with login and team_info, which should give
  My Page its background (`bg_00NN` by JoJo part) and the leader model. Routes: `statue/statue_list`,
  `statue/statue_info`, `team/team_info`.
- **Statue numbers from the wikis:** rarity, type and Lv99 stats for 140 of 266 statues (129 from
  Gamerch matched by leader- or assist-ability name against the game's labels, 11 from ShoutWiki by
  elimination); Lv1 from the wiki for 35 of them, Lv99 / 10 for the rest; stats per level are linear
  Lv1→Lv99. Two wiki records were rejected (a typo and a rarity-order conflict). The rest, mostly
  released after the 2018 snapshot, use marked defaults.
- **Minds:** the player owns the 417 minds matched to game ids by name on Gamerch (rarity, flat
  stats; cost is a default for most); routes `mind/mind_list`, `mind/mind_info`.
- **Generated master tables:** `statue_setting_table` (266 rows), `statue_level_table` (266 × 99
  levels), `rank_table` (ranks 1–99 from Gamerch).
- **Story quests:** story button, 82 chapters (29 action, 53 tactical) and 370 stages from the
  player's files; 86 stages carry Gamerch numbers (AP, rank EXP, gold where shown, recommended level),
  matched by chapter title; the rest have defaults kept inside the game's limits (stamina ≤ 20,
  level ≤ 99). Battles are described with the player's area scenes and a wave of enemy statues;
  a clear is recorded and grants gold, rank EXP and statue EXP to the team that fought, once per
  started battle; rank-ups and AP max follow the Gamerch rank table.
- **Saves:** every change goes through one locked read-change-write; bookkeeping (login time, play
  counts) no longer rotates the 5 backups. Routes: `bizarre_list`,
  `stage_list`, `bizarre_start`, `bizarre_end`, `bizarre_continue`, `destruction`,
  `check_timed_chapter`, `support/support_user_s`, `clan/recommend`.
- **Wiki fetch:** ShoutWiki (351 pages), JoJoWiki (86 pages), Gamerch via the Wayback Machine (178
  archived data pages of 1,004; most were never archived). Raw pages stay local; only numbers with
  their source pages are committed (`docs/CONTENT.md`, `CREDITS.md`).
- Build number 10, server marker `JDR_EMBEDDED_SERVER_b010`.

### Known gaps / risks (expected)
- Battles have never run offline and probably fail somewhere: statues carry no skills, abilities or
  level-up table and the battle master tables are empty; enemy spawn naming (`WAVE_A`/`01`) is a
  guess; tactical battles send no formation or AI id (the position-pattern and AI tables are not
  generated), so tactical stages are the least likely to load. The result code for a win (0) is
  medium confidence. The stage list should show; whether start/end work needs the device log.
- Enemies use the wiki's recommended level, and the player's statues start at level 1 and only grow
  from clears, so later action chapters may be too hard.
- Enemy line-ups and rewards are defaults (the wiki shows them as images); stages after chapter 23
  and the tactical chapters have default numbers.
- Skills and abilities are not filled yet (statue details show none).

### Device checklist — expected; to be confirmed on a device
- [ ] My Page shows a background and the leader model (Jonathan)
- [ ] Team/Enhance and the statue list show the roster with names, rarity, levels, stats
- [ ] Quests → Story shows chapters and stages; starting a stage loads (or note where it stops)

## Build 09 — redrawn UI art (branch `wip/b09`)

### What came in
The owner's redrawn art pack: 464 sprites (all in English lettering) from 41 UI bundles and 4
title/boot screen files in the APK, at the exported sizes; terms Summon, Enhance,
Courage/Wisdom/Spirit/Unity/Solitude.

### What changed
- **Art import.** `jdr-patch … --art <pack.zip> --data <files>`: title/boot screens repainted in
  the APK (13 sprites, 4 files); menu art (451 sprites) stored in the APK (`assets/jdr/art.bin`,
  ~19 MB) and applied by the server on the phone while it rebuilds the English bundles (same
  originals, cache and single download). Atlases with replaced sprites are stored as RGBA32.
- `jdr_unity::texture`: ETC1/ETC2_RGBA8/RGB24/RGBA32/ARGB32/Alpha8 decode, PNG decode, paste.
- The bundle rebuild runs on up to 4 threads (desktop: ~1.1–1.5 s for all 60 bundles with art).
- Text terms aligned with the art (Gacha → Summon, Power Up → Enhance, Bravery/Intellect/
  Tenacity/Maverick → Courage/Wisdom/Spirit/Solitude).
- Server log: `[art] art pack loaded: N image(s)` and `[art] … sprite(s) replaced in this rebuild`.
- **Two reviews** (correctness; behaviour/risk). Fixed: sprite boxes now computed exactly as the
  exporter cut them (ties-to-even edges; three sprites would otherwise have been off by a pixel),
  the exporter's naming rule for repeated names, art generator version in the cache key, sprites
  with a separate alpha texture or rotation refused instead of drawn wrong, a bad image skips only
  itself (the bundle keeps its English), the patcher checks the whole pack against the player's
  bundles before building, title-screen layouts borrowed only from types with the same hash,
  macOS zip debris ignored, 2 rebuild workers when art is present, text fixes after the term
  switch, CONTRIBUTING: no game data in releases/issues/PRs.
- Build number 9, server marker `JDR_EMBEDDED_SERVER_b009`.

### Verified off-device
- Real data: all 451 menu sprites matched and replaced, 0 unmatched; UnityPy confirms every
  replaced sprite equals its PNG exactly and, in the textures whose originals are unencrypted, every
  other pixel equals the original (so the ETC decoder and the paste change nothing else).
- Real patch with the pack: 13 sprites in the 4 in-APK screen files equal their PNGs
  (`tools/proofs/art_check.py --apk`, run by build.sh).
- Removing the pack reverts: bundles with text go back to the text-only files, art-only bundles to
  their originals (real-data test).

### Known gaps / risks (expected)
- Animated lettering made of several sprites (e.g. SUCCESS!!, RETIRED, RATE UP!) may need spacing
  or timing adjustments (owner's notes); nine tutorial screens are not redrawn yet.
- RGBA32 atlases use 4x the memory of ETC2 (8x of ETC1) ones while those screens are open; check
  with `adb shell dumpsys meminfo com.bandainamcoent.jojodr` on the busiest menus.
- Storage on the phone (measured on the owner's files): originals kept 111 MiB, served English/art
  files 182 MiB (the game's own copies of those are replaced by the downloaded ones), plus the
  19 MB pack in the APK.

### Device result (Galaxy S22, Android 16) — from the owner's log and screenshots
- [x] `[art] art pack loaded: 451 image(s)`; `[l10n] 60 English file(s) ready in 1322 ms, 60 already
      on the phone` (cached after the first boot's download).
- [x] My Page buttons, header and side menu show the redrawn English art (Inbox, Notices, Missions,
      Friends, Home, Team/Enhance, Quests, My Town, Summon, Shop, Menu); Options screen in English.
- [ ] My Page 3D background is black: expected without a team (no leader statue, no
      `statue_setting_table` row) — next build.
- Town "Pick Up" dialog body still Japanese (description text tables, next batches); one route
  unknown: `clan/recommend` (now handled).

## Build 08 — English names (branch `wip/b08`)

### What the Build 07 device result showed
The download prompt appeared on the first boot; menu text labels show the English. Labels drawn
into images stay Japanese (the owner is redrawing them).

### What changed
- **Second English batch (names): 31,039 rows** from 7,690 unique strings: statue/character, event
  character and place names; skill, ability, buff and tarot names; Mind and item names, rewards and
  event shop items; chapter, stage, quest, tutorial and title names. With Build 07's batch, 33,336
  of the 50,845 text strings are English (0 stale), plus all 236 ClientMes messages.
- Names follow one name list (709 entries: characters, Stands, places, titles), so every batch
  spells them the same way.
- `jdr-tool l10n import --expand-with`: one translation per unique Japanese string, applied to
  every row with that source. New translation file `text_content.json`.
- Server log: `[l10n] … N already on the phone` (how many English files the client holds).
- Build number 8, server marker `JDR_EMBEDDED_SERVER_b008`.
- **Reviews:** code (nothing serious; `--expand-with` now checks the repo's files, not the compiled-in
  copy) and translation quality ("good": glossary followed, no placeholder or line-break errors).
  Fixed from the quality review: item names unified across tables, Hamon (not Ripple) throughout,
  Mind/skill titles reused in shop and reward tables synced to one English, the team screen's type
  labels (Bravery/Intellect/Tenacity/Maverick), a stray space in tarot effects, two mistranslations.

### Verified off-device
- Real data with the shipped translations: 32 English files, 33,336 strings replaced, 0 stale.

### Device checklist — expected; to be confirmed on a device
- [ ] Download prompt once (text.jp changed); names, skills, items, chapters show English
- [ ] `[l10n] 32 English file(s) ready …`, then on the next boot `32 already on the phone`

## Build 07 — English menus, UI art export (branch `wip/b07`)

### What the Build 06 device log showed
MyPage reached; five menu routes fell back to the unknown-route answer (present box, follow list,
items, shop, quest select).

### What changed
- **English in place** (docs/LOCALIZATION.md). The server rebuilds `text.jp` and the UI prefab
  bundles with English from `server/content/l10n/en/`, keeps the originals, and lists the English
  files in the manifest with their md5/size. The client downloads them once through its own update
  path (expect a download prompt on the first boot, about 100 MB). Replaced UI labels get "best fit"
  so longer English shrinks into its box. Log line: `[l10n] N English file(s) ready …`.
- **English boot/download/error messages**: new patch `english-clientmes` rewrites the in-APK
  ClientMes table (fingerprinted, `--disable english-clientmes`).
- **First English batch** (2,533 strings): all ClientMes messages (236), all UI prefab labels
  (1,210), and the menu text tables (1,087: My Page, options, title, team, training, evolution,
  filters/sorting, battle UI, quest types/results, arena, shop, gacha, special actions, …).
  Statue/skill/item names and descriptions, missions and events (~48,500 strings) follow in later
  batches; story scripts later still. The in-game terms text is replaced by a short "unofficial
  offline build" notice rather than translated.
- **Translator tools:** `jdr-tool l10n export` (every string of the player's copy into a local TSV)
  and `jdr-tool l10n import` (refuses lost placeholders, wrong hashes, Japanese in the English).
- **jdr-unity:** whole SerializedFile parse/write (layout and data offset preserved), generic
  type-tree decode/encode, Unity's common string table, LZ4 bundle writer.
- **Importer:** never overwrites the game's own `vcs.bytes` or our English files with a staging copy
  (otherwise a staging folder left in place would undo them at every boot).
- **Menu routes:** `get_bizarre_select`, `present_list`, `follow_list`, `item_list`, `shop_list` answer
  with a well-formed empty state.
- **UI art export** (`tools/ui-assets/export_ui.py`): every menu texture/atlas and cut-out sprite from
  the player's own copy, with a manifest for re-import (re-import comes later). Sent to the owner.
- Build number 7, server marker `JDR_EMBEDDED_SERVER_b007`.
- **Two reviews** (correctness; behaviour/risk). Both found the same serious bug, now fixed: after a
  translation update, the English file still on the phone could have been saved as the
  "original", losing the Japanese copy. Also fixed: cached English files kept their encryption
  flag; a bundle that loses its last translation is reverted; a failed state save offers no English
  that boot (instead of risking originals); a half-written or English file never replaces a kept
  original; `/cdn` serves from the last build instead of rebuilding mid-download; importer keep
  rules only while files are really ours; ClientMes post-check now counts the English rows;
  translator import requires the Japanese column and a well-formed hash; corrupt array counts are
  refused; the UI export manifest no longer pairs plain and obfuscated names; term fixes.

### Verified off-device
- Real data: all 287 serialized files / 137,728 objects of the UI bundles and text.jp decode through
  their type trees and encode back byte-identically; the files re-serialize identically.
- Real data: rebuilding every target with a placeholder for **every** string (50,845) replaces all of
  them; the rebuilt bundles hold no Japanese, parse with UnityPy (exact byte consumption), and a
  second run reuses the cache with identical md5s. Desktop: ~3 s first build, ~0.5 s cached.
- With the real translations: 32 English files, 2,297 strings replaced, 0 stale (+ 236 ClientMes in
  the APK); the real ClientMes rewrites byte-identically when nothing changes.

### Known gaps / risks (expected)
- First boot after installing builds the English files on the phone (desktop: ~2–3 s; the phone is
  slower) while the client waits for the manifest.
- Extra storage: originals + English copies (~220 MB).
- Best fit could make some labels small; layouts are untested on device.
- Text in UI images stays Japanese until the redrawn art is imported.

### Device result (Galaxy S22, Android 16) — from the owner's log (a later boot of Build 07)
- [x] `[l10n] 32 English file(s) ready in 1145 ms (0 rebuilt now …)`: the English files built on an
      earlier boot were reused from the cache (phone: ~1.1 s cached).
- [x] Boot to My Page as before; `unknown_routes=0`: the five menu routes now have handlers.
- [x] Import with the staging folder still present: `13480 already fine, 0 copied` (nothing undone).
- [x] First boot after installing showed the download prompt (owner); the English files were
      downloaded then, and later boots reuse them.
- [x] Text labels in the menus show the English (owner: "the ones that are text were translated
      fine"). Labels drawn into icons/images stay Japanese until the redrawn art is imported.

## Build 06 — master tables generated on the phone (branch `wip/b06`)

### What the Build 05 device logs showed (Galaxy S22, Android 16)
- Boot reached `getfiledata` for `in_memory_master_data` and stopped on `[cdn] … 404`: the bundle
  the dead server used to send with the game's master tables. No player copy has it (it was only
  ever held in memory).
- During an import, the client looped title → login until `vcs.bytes` arrived.

### What changed
- **`in_memory_master_data` is generated** by the server (embedded and standalone), next to the
  manifest, and listed in it with the md5 and size of exactly what `/cdn` serves (plain UnityFS,
  `isEncrypt=false`). Log line: `[assets] versionfile built: … in_memory_master_data: N tables (…)`.
- **Schema** (`server/content/masterdata/schema.json`): the 17 core assets and the serialized fields
  of the 57 types they reach, from our analysis of the client (docs/research/09-master-data.md);
  only what our content needs (code identifiers, no values or text). The bundle holds about 45
  assets, not 18: 27 more battle tables are read from the same bundle; their layouts are mapped in
  the research and join the schema with their content. Their boot readers null-check (checked in
  the code for the two read earliest).
- **Content:** `tutorial_flag_table` with the 34 flag labels the client queries (ids 0..33, no
  tutorial script downloads), so, with `get_tutorial_step` answering all ones, every tutorial and hint
  counts as done and the daily/event quest categories unlock. The other 16 core tables are written
  empty (each loader accepts an empty list); battle tables are left out until they have content.
- `jdr_unity::managed` (type trees + binary layout in the engine's own node shapes) and
  `jdr_unity::objects` (AssetBundle/MonoScript, shared with the manifest; manifest bytes unchanged).
- Import: leftovers of an interrupted import are removed; `/client/getkey` waits (≤ 12 s) while an
  import runs, so the game holds on the title instead of looping (the flag clears even if the import
  panics).
- Content files must carry well-formed sources (`jdr-research` + doc, `jdr-default` + note, outside
  sources + page + access date); CONTRIBUTING and CREDITS updated.
- `jdr-server --tutorial play` now warns: there are no tutorial scripts in the tables yet.
- If the master tables ever fail to build, the manifest is still built and served (without them),
  and the error is logged.
- Two reviews (correctness; behaviour/risk). The correctness review compared our type trees with real
  engine trees from the player's files (MonoBehaviour header, nested-class lists, strings, bools,
  Vector3, vectors: exact match) and found nothing serious. Fixed: preload order now object → script
  as the engine writes it; informational 0x8000 no longer set for lists of fixed-size structs;
  `List<bool>` refused until its packing is needed; duplicate content files refused; the import
  flag is set before the first request can arrive; the UnityPy check now compares every field
  (defaults included) with the content.
- Build number 6, server marker `JDR_EMBEDDED_SERVER_b006`.

### Verified off-device
- Real data, through the server context as on the phone: the manifest lists the master-data
  bundle with the md5 and size of exactly the bytes `/cdn` serves.
- Unit tests: the tutorial-table type tree equals the research template node for node; binary layout
  (string/bool alignment, lists); every class in the schema resolves and serializes; content rules
  (sources required, typos refused, enum names); deterministic bundle.
- Real data (`JDR_DATA`/`JDR_KEYS`): bundle built with the player's own MonoScript/AssetBundle type
  entries; `tools/proofs/masterdata_check.py` (UnityPy) reads all 17 tables at the client's container
  paths, each consuming exactly its bytes, and every tutorial row equals the content file.

### Known gaps / risks (expected)
- Whether the engine reads our objects through the shipped type trees (script hash differs from the
  build's) is expected from Unity's behaviour and from the manifest working, but is **first tested on
  the device** in this build.
- No team yet, so MyPage shows no leader model. Glossary empty. Battles, gacha scenes and special-move
  cameras need real rows (M4 content) and the battle tables.
- `rank_table` exists but is empty: player-rank lookups return a zero row (battle-result exp bar).

### Device result (Galaxy S22, Android 16) — confirmed by device log
- [x] `versionfile built: 13480 entries, 1845 KiB; in_memory_master_data: 17 tables
      (tutorial_flag_table 34 rows), 32 KiB; 1061 ms` (in the background, before the title).
- [x] `[cdn] GET /cdn/0a59….bytes 200 out=33596B (generated in_memory_master_data)`: the client
      accepted the generated tables (our MonoScripts and type trees) and left TitleLoad.
- [x] **MyPage reached** (owner: "We got to the menu"): `product_list`, `banner_list`, `user_info`,
      `team_info`, `check_forced_notify`, `event_list`, `bizarre_resumption`, `get_loginbonus`,
      `get_menu_notify`, then menu navigation (present box, follow list, items, shop).
- [x] No tutorial ran (the 34-flag table + all-ones `get_tutorial_step` work as intended).
- Unknown routes hit (answered with an empty OK): `bizarre/get_bizarre_select`,
  `present/present_list`, `follow/follow_list`, `item/item_list`, `shop/shop_list`. Next build adds
  proper handlers.
- Import re-run was a no-op (13,480 already fine, 10 s); the staging folder can be deleted.

## Build 05 — keep the game files, import them, review fixes (branch `wip/b05`)

### What the Build 04 device logs showed (Galaxy S22, Android 16)
- The account persisted across the update (login with the saved uuid, `existStatus 0`).
- `vcs.bytes` was missing: the game had **deleted its whole download folder**. Cause, from the
  client code: `ResourceDownloadUIController.StartLoad` runs a one-time "cache clear of non-encrypted
  files" (`DoCacheClearProcess` → `IOManager.DeleteAllInDirectory(persistentDataPath)`) whenever the
  PlayerPrefs flag behind `SaveDataManager.LoadNonObfuscatedClearFlag` is unset, which it is after a
  reinstall. Our server never writes there.
- After the owner pushed the files back with adb: `vcs.bytes: Permission denied`; the files belong to
  the shell user, the game falls back to the title and logs in again in a loop.

### What changed
- **`keep-files` patch** (libil2cpp @0xfd1534, fingerprinted): `LoadNonObfuscatedClearFlag` returns
  true, so the wipe never runs. `tools/proofs/keep_files_check.py` confirms on the player's binary
  that the cache clear is its only caller.
- **File importer** in the embedded host: copies `/data/local/tmp/jdr-import/files` into the game's
  `files/` as the app (replacing unreadable shell-owned copies, `vcs.bytes` last, temp file +
  rename, source untouched), then the manifest is built. Expected to work (apps may read
  world-readable files there); to be confirmed on a device.
- Boot diagnostics: `[assets] game files: N file(s), …` (vcs.bytes present? recorded files missing?).
- Manifest build outside the lock, failures cached for 30 s, poison-safe, template scan capped.
- **Review fixes (Builds 03–04):** `unity-heap` now uses 16 MiB pools (shorter pool/large lists in
  busy heaps; the proof allows exactly that one field to differ and also checks the calling
  convention, the callers and the storage size); bundle reader caps block/node counts and sizes;
  type-tree flags match Unity's (0x8000 on variable-size nodes); UnityPy check script for the
  manifest; `build.sh` lists skipped checks and requires the proofs when `JDR_APK` is set.
- Docs: restore step via the staging folder, missing-files and permission troubleshooting,
  `--disable unity-heap`, log redaction; CREDITS links.
- Build number 5, server marker `JDR_EMBEDDED_SERVER_b005`.

### Device result (Galaxy S22, Android 16) — confirmed by device logs
- [x] Import from `/data/local/tmp/jdr-import/files` works (apps may read it): first run copied
      11,000 files / 1.2 GB in ~20 s before the game was closed; the next run resumed and finished in
      13 s (2,380 copied, 2,378 unreadable shell-owned copies replaced, 0 failed).
- [x] `[assets] game files: 13480 file(s), all 13479 recorded files present`;
      `versionfile built: 13479 entries, 1845 KiB, 3544 ms` (background, before the title).
- [x] The client accepted the generated manifest: `[cdn] … 200 … (generated versionfile)`, **no other
      downloads** (every local file judged up to date), no errors.
- [x] Then `get_system_config`, `get_tutorial_step`, and `getfiledata` for `in_memory_master_data`
      → 404: the expected M1.5 stop.
- [ ] After a reinstall or "Clear storage", the files are no longer wiped (not tested yet).
- Note: during an import the client loops title → login until `vcs.bytes` arrives (it is copied
  last). Next build: hold `/client/getkey` until the import finishes; clean leftover temp files.

## Build 04 — generated asset manifest (branch `wip/b04`)

### What changed
- **`server/unity` (new, shared):** UnityFS read (LZ4/LZ4HC) and write (uncompressed),
  SerializedFile v17 type-table parse and write, type trees for our MonoBehaviour classes.
- **`versionfile` generated by the server** from the game's own `files/` folder (read-only): one
  entry per file recorded in `vcs.bytes` (path = decoded name + `.bytes`, obfuscated name, the md5 the
  game recorded, real size, encrypted flag from the JJEC header). Engine type definitions
  (MonoScript, AssetBundle) are lifted at runtime from one of the player's bundles and must match
  their fingerprints. Cached until `vcs.bytes` changes; the embedded host builds it in the
  background at boot. Served when `getfiledata` asks for the manifest's obfuscated name.
- Settings split: `files_dir` (CDN import folder) vs `game_files_dir` (manifest source).
- Content policy: wiki-sourced values committed with sources, credited in `CREDITS.md`.
- Build number 4, server marker `JDR_EMBEDDED_SERVER_b004`.

### Verified off-device
- Real data: 13,479 entries (4,368 encrypted), 1.89 MB bundle, built in ~0.1 s on a PC.
- UnityPy (an independent Unity reader) parses the bundle (`tools/proofs/versionfile_check.py`, run
  by `build.sh` with `JDR_DATA` + `JDR_KEYS`): container
  `assets/resources/version/android/versionfile.asset`, MonoScript
  `ScSystem.ScIO.AssetBundleVersionInfo` / `Assembly-CSharp.dll`, every entry decodes through our
  type tree.
- End to end on the standalone server: `getfiledata` returns the URL for the manifest's obfuscated
  name, and that URL serves the generated bundle (cached on repeat).

### Known gaps / risks (expected)
- `in_memory_master_data` is not built yet: the game is expected to stop at that request (M1.5,
  Build 05).
- Our `AssetBundleVersionInfo` type carries its own script hash, so the engine must read it through
  the type tree (the standard path for bundles with type trees). To be confirmed on a device.

### Device checklist — expected; to be confirmed on a device
- [ ] `[assets] versionfile built: … entries` in the `JDR` log soon after boot
- [ ] `[cdn] GET /cdn/<manifest>.bytes 200 … (generated versionfile)`
- [ ] no re-download of game files (no other `/cdn/` requests for existing files)
- [ ] next requests: get_system_config / get_tutorial_step and/or the master-data bundle (expected stop)

## Build 03 — Unity allocator crash fix (branch `wip/b03`)

### What the Build 02 device log showed (Galaxy S22, Android 16)
- **The embedded server works on the device:** `jdr embedded build 2` (+5 ms), `keys loaded`
  (+19 ms), `listening on 127.0.0.1:18080`; then `POST /common/get_version 200` twice and
  `POST /api/user/login 200` (new user, `existStatus 4`).
- 26 ms after the login reply the game crashed in `libunity.so` (SIGSEGV, fault addr 0x106645) inside
  `JsonUtility.FromJson`, right after Unity's own error **"Using memoryadresses from more that 16GB
  of memory"**. Growthbeat (analytics) failed offline and caught its own exception (not the cause).

### Cause
Unity 2018.4.1's `UnityDefaultAllocator` (ALLOC_DEFAULT, ALLOC_GAMEOBJECT, ALLOC_GFX) records every
malloc'd block in a page table with 5 slots of 4 GiB address windows. Android 11+'s Scudo allocator
spreads memory over more windows; the slot search returns -1 and the next access faults (crash
frames 0x9ffdb4 / 0x9ff968 are exactly that code). This is an engine bug on new Android, not a
server-data problem; Unity fixed it in a later 2018.4 release. The `-systemallocator` switch does
not help: in this release build its result is ignored.

### What changed
- **`unity-heap` patch** (evidence: the boot crash is gone on the device; long sessions, memory use
  and frame pacing with the new allocators are not yet observed): replaces the 96-byte body of the `UnityDefaultAllocator` constructor
  (libunity @0x9ff824, fingerprinted by library and window SHA-256) with code that builds a
  `DynamicHeapAllocator`, field for field like the engine's own inline `ALLOC_FALLBACK` heap. The
  proof script checks on the real binary that only the three allocators above call that
  constructor, that the page-table code is reachable only from its own class, and that each object
  has 0x140 bytes of storage (the constructor writes below 0x118). The new heaps lock (like the
  fallback heap) and use 1 MiB pools.
- **Equivalence proof on the real binary** (`tools/proofs/unity_heap_equiv.py`; `build.sh` runs it
  and requires it whenever `JDR_APK` is set):
  arm64 emulation of the patched constructor vs the engine's inline construction on a poisoned
  object → 129 bytes written, **0 bytes differ**, same mutex-init call.
- Build number 3, server marker `JDR_EMBEDDED_SERVER_b003`.

### Device result (Galaxy S22, Android 16) — confirmed by device log
- [x] no "Using memoryadresses from more that 16GB" line, no SIGSEGV after login (fix confirmed)
- [x] terms page served (`GET /web/information/terms_a.html 200`), Accept → `get_nonce` →
      `[account] created user_id=100000001` → second login with the new uuid
- [x] `/api/client/getkey 200`, then `getfiledata` for **one** file: `versionfile` (decoded name) →
      `GET /cdn/<obfuscated>.bytes 404` → the expected stall (M1.5 builds `versionfile`)
- [ ] `get_system_config` / `get_tutorial_step` not seen yet (they come after the manifest)
- Notes: Growthbeat throws offline and catches it itself (harmless; debloat in M6). The process
  ended with SIG 9 (app closed), not a crash.

## Build 02 — embedded server (branch `wip/b02`)

### What changed
- **`libjdr.so`** (`hosts/embedded`, aarch64, NDK r28c API 21, 16 KiB pages): the same server as
  `jdr-server`, inside the game process. A constructor only spawns a thread (the game never waits);
  that thread finds the app's own APK from where `libjdr.so` is mapped, reads the player's keys from
  the stored asset `assets/jdr/keys.json`, and serves on 127.0.0.1:18080 (retries forever if the port
  is taken, FATAL line once a minute). State in `/data/user/<n>/<pkg>/jdr/` (adoptable-storage
  fallback); logs and save backups copied to `/sdcard/Android/data/<pkg>/jdr/{logs,saves}/`
  (changed files only, logs pruned); every log line mirrored to logcat, tag `JDR`.
- **`embed-server` patch:** adds `lib/arm64-v8a/libjdr.so` (validated: aarch64, 16 KiB, build
  marker `JDR_EMBEDDED_SERVER_b002` shown in the report), embeds the keys asset, and renames
  `libmain.so`'s `liblog.so` dependency to `libjdr.so` (same length, in place; libmain imports no
  liblog symbols, and libjdr depends on liblog itself). Fingerprinted on the libmain SHA-256; the ELF
  editor proves the string is unshared (dynamic tags, verneed/verdef, all symbols via DT_HASH,
  tail-merged strings) and returns errors on malformed input.
- `--port` is refused with the in-game server; `--disable embed-server` builds the PC dev-loop APK.
- Server limits: 2 MiB bodies, 32 connections, 20 s read/write timeouts.
- `build.sh` builds `libjdr.so` when `ANDROID_NDK` is set and only embeds a library built in the
  same run.
- Docs: PLAYER-SETUP rewritten for the in-game server (inline steps, success checklist, recovery
  without uninstall, save/key warnings), SECURITY in-game section, DEV-LOOP `adb reverse --remove-all`.
- Two independent reviews (correctness inside the game process; behaviour/risk): all findings fixed.

### Tests
| Suite | Result |
|---|---|
| core | 16 passed |
| crypto (+ real data with `JDR_DATA`) | 14 + 1 passed |
| embedded (zip reader, own-APK detection with foreign APKs mapped, boots from an APK asset and answers get_version, log pruning, marker) | 7 passed |
| patcher (incl. ELF NEEDED swap and its refusals) | 15 passed |
| services | 9 passed |
| end-to-end boot sequence | 5 passed |
| real patch with the embedded server (`JDR_APK`, `JDR_KEYS`, `ANDROID_NDK`) | all post-checks ok |

Verified off-device: `libjdr.so` has one constructor in `.init_array` (relocated to `jdr_init`),
BIND_NOW, 16 KiB LOAD alignment; patched `libmain.so` NEEDED = libjdr, libstdc++, libm, libdl, libc;
the keys asset reads back from the signed APK (after the APK Signing Block) identically to `zipfile`.

### Known gaps (expected)
- Same stall point as Build 01: `versionfile` and `in_memory_master_data` (M1.5).
- The "game keeps working if our module fails" latch covers Rust panics only; if `libjdr.so` cannot
  load, the game cannot start (recovery documented, no uninstall needed).

### Device checklist (Galaxy S22, Android 16) — expected; to be confirmed on a device
Follow docs/PLAYER-SETUP.md. Send `local/logcat.txt` and `adb pull …/jdr/logs`.
- [ ] patched APK installs; the game launches (does not close immediately)
- [ ] logcat `JDR`: `jdr embedded build 2`, `keys loaded`, `listening on 127.0.0.1:18080`
- [ ] `[api] POST /common/get_version 200` at the title screen
- [ ] terms page → `[account] created` → second login
- [ ] getkey / get_system_config / get_tutorial_step / getfiledata, then the expected stall
- [ ] `/sdcard/Android/data/com.bandainamcoent.jojodr/jdr/logs/` and `…/saves/` appear
- [ ] restart the game: same account (`existStatus 0`), no FATAL bind line

## Build 01 — dev loop groundwork (branch `wip/b01`)

### What changed
- **Research (M0):** docs/research/01–08, docs/PROTOCOL.md (171 request classes → paths),
  docs/protocol/{envelope,boot,assets}.md. Decision on §2.2: Rust server, arm64 only, redirect by
  rewriting one metadata string literal + allowing cleartext HTTP; the server then hands out its own
  API/web bases.
- **Key recovery** (`jdr-tool keys`): asset key + IV recovered from the player's own files (frequency
  attack on obfuscated names, IV from the fixed UnityFS header), envelope key read from the player's
  own APK (fingerprinted), sound key supplied by the player. Checks the copy is complete against the
  game's own `vcs.bytes` record. Keys go to a git-ignored `local/keys.json`.
- **Patcher v0** (`jdr-patch`): `server-url` (metadata literal #4137 → `http://127.0.0.1:18080`),
  `cleartext` (`android:usesCleartextTraffic="true"` via a binary-manifest editor), `drop-armv7`.
  Build fingerprinted on metadata, libil2cpp and manifest SHA-256. zipalign (16 KiB pages for .so),
  apksigner v1+v2+v3, signing-key pin, input/output guards, patch report with post-checks.
- **Server v0** (`jdr-server`, standalone): std-only HTTP/1.1 server on 127.0.0.1, envelope codec,
  local accounts (schema-versioned, backups, unreadable save ≠ new user), the new-player boot chain
  (get_version, login, get_nonce, create_user, getkey, get_system_config, get_tutorial_step,
  tutorial, getfiledata, product_list, user_info, update_username, team_info, check_forced_notify,
  event_list, banner_list, get_loginbonus, get_menu_notify, bizarre_resumption), terms page, CDN from
  the player's files, per-request log lines, `[status]` line, secret-field redaction, loopback-Host
  check. Unknown routes answer OK and are logged as `UNKNOWN ROUTE`.
- Two independent reviews (correctness; behaviour/risk): all findings fixed except the content
  question below.

### Tests
| Suite | Result |
|---|---|
| core (http, envelope, clock, storage) | 16 passed |
| crypto (aes, names, containers, key recovery) | 14 passed |
| crypto real-data (`JDR_DATA`): key recovery, all 4,368 encrypted bundles decrypt, vcs md5/size valid | 1 passed |
| patcher (metadata, AXML, zip, signing parsers, patches) | 12 passed |
| services (accounts, CDN, redaction, host check, URLs) | 9 passed |
| end-to-end over a socket (boot sequence in client order) | 5 passed |
| real patch of the 3.12.0 APK (`JDR_APK`) | all post-checks ok; `aapt2` parses the manifest |
| `cargo check --target aarch64-linux-android` (shared code) | ok |

### Known gaps (expected)
- `versionfile` and `in_memory_master_data` do not exist in any player copy (they were only ever
  held in memory). Until M1.5 rebuilds them, **TitleLoad is expected to stall** after getkey /
  get_system_config / get_tutorial_step / getfiledata.
- Tutorial is skipped by default (`--tutorial play` exists, but the tutorial needs master data).
- Sound key unverified (public list entry); a wrong key means silent downloaded audio.
- No team/statues yet: MyPage would show no leader model.

### Device checklist (Galaxy S22, Android 16) — expected; to be confirmed on a device
Follow docs/DEV-LOOP.md. Please send the newest `local/server/logs/server-*.log` and
`local/logcat.txt`.
- [ ] `jdr-tool keys` prints "copy is complete"
- [ ] patched APK installs (arm64), icon launches
- [ ] title screen; server log shows `POST /common/get_version 200`
- [ ] tap start → terms page appears with an Accept button (`GET /web/information/terms_a.html 200`)
- [ ] account created (`[account] created …`), second `/api/user/login` with existStatus 0
- [ ] `/api/client/getkey`, `/api/common/get_system_config`, `/api/user/get_tutorial_step` answered
- [ ] which files `getfiledata` asks for (log line `[assets] getfiledata …`) and where it stalls
- [ ] any `UNKNOWN ROUTE` lines
- [ ] logcat: exceptions after the last server request (to confirm the stall cause)

### Open
- **Content policy question (owner):** commit wiki-sourced values (stats, skill numbers) for the
  master-data rebuild as cited JSON, or keep them out of the repo.
- M1.5 design: docs/design/master-data.md (draft).
