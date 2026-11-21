# 14 — Feature audit against the owner's research pack (Build 20, refreshed in Build 25)

The owner's research pack (2026-11-15, gathered with an outside research assistant; private,
under `local/wiki-raw/`) lists 118 documented behaviours of the public game, and its revision 2
adds 47 behaviour notes. Each has sources, an era/mode and an uncertainty. This file maps every
one to our code. The research ids are research identifiers, not game ids, and are never sent to
the client.

Target: the 3.12.0 client (the last version), with Tactical Battle primary and compatible Action
Battle quests retained.

**Status:**
- **server**: implemented in our server.
- **client**: client behaviour that needs nothing from us beyond data we send (✓ seen on a device).
- **partial**: some of it is implemented.
- **missing**: needs server work.
- **offline**: a deliberate project choice (no payments, everything owned, open calendar).
- **unverified**: not checked on a device yet. Everything built in Builds 21–25 is unverified
  until a device log shows it (docs/STATUS.md device checklists); "✓" marks what a device showed.

## Priorities (Build 25)

The Build 20 priorities are done except the Action-mode ones:
1. ~~Tactical Reversal~~: on since Build 20 (docs/research/11 §5).
2. ~~Missions screen~~: Build 21 (docs/research/17).
3. **Action-mode roster compatibility** (`party.mode_rosters`): still open. Every statue may enter
   both teams; the Action battle has not been played on a device.
4. **Action-stage mission rows** (`quest.mission_predicates`): still open; Action stages send no
   star missions.
5. ~~Daily boundary at 04:00 JST~~: missions, the daily tile, Mentality, Arena days and the free
   Friend Point draw all turn at 04:00 JST (`missions::game_day`).

Open next: the Action battle (which JoJo Links need), buff/debuff/summon skill families (no
source for their amounts), raids (no raid screens in the player's files).

## Boot and account

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| boot.terms | server ✓ | Tutorial flags kept per account (`user/tutorial`, `get_tutorial_step`); the terms page loads from our web module (`web.rs`) | Interrupting the battle tutorial at each step (unverified) |
| boot.tutorial_guarantee | offline | Every statue is owned; the tutorial summon draws from the owned roster | — (provisional source) |
| boot.naming | server ✓ | `user/update_username` | — |
| boot.title | client ✓ | Title from the APK; art pack redraws it | — |
| boot.transfer_issue / transfer_expiry / transfer_input | offline | One local account; the button shows its screen with our English | No transfer partner offline; saves can be backed up as files |
| boot.save_choice | client | Save select is one of the APK scenes we translate | unverified |
| boot.cross_os | offline | No paid currency | — |

## Home and menus

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| home.leader | server ✓ | Main tactical team leader on My Page (`statue_setting_table`, Build 12) | Alternate voice lines are client side |
| home.resources | server ✓ | `user_info`: rank, AP (always full), gold, Diamond Coins (free), BP (Arena, Build 25), Friend Points | — |
| home.present_box | server | Message Box: waiting and received tabs, receive one or all; mission, Labyrinth, Arena, event and Count Up rewards and the welcome gift go there (Build 22, docs/research/18) | unverified |
| home.missions | server | Daily (reset 04:00 JST) and regular missions with the game's labels and the wiki's rewards; claim one or all; Home badge (Build 21, docs/research/17) | Limited-time tab empty (no events close offline); link, follow and Town missions left out |
| home.notices | server | Notices: three tabs load our local pages, the Updates tab "what's in this build" and new until opened; Home badge (Build 25, docs/research/24 §2) | Banners stay empty (no banner art says "what's new"); no forced notice |
| home.collection | server | `book/statue_list`, `mind_list`, `item_list`, `yogo_list`; opening it counts for its mission (Build 21) | — |
| home.profile | server | `user/update_comment` saves the greeting; counts for its mission | — |
| home.options | client ✓ | Options grid works; labels in English (Build 19 shortened) | Purchase history / transfer have no backend (offline) |
| home.settings | client | Local settings | unverified |
| home.bulk_download | client | The client's own download path; we serve `/cdn` | — |

## Party and quests

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| party.presets | partial | 5 teams per battle type and 5 Arena teams (`collection.rs`) | The launch narration says 10; the 3.12.0 page count is unchecked |
| party.three | server ✓ | Leader, left, right slots per team | — |
| party.variants | server ✓ | Each rarity/type variant is its own statue: 266 members and 26 assist-only statues (`statues.json`, Build 25) | — |
| party.jojo_links | client | Action mode only (H): the tactical battle passes `jojoLinkList` but never applies it, and the tactical formation hides the JoJo link window; we send `[]` (docs/research/26 §1) | Built only if the Action battle is served; 39 of the 42 archived rows match an `ABI_JOLINK_NAME_<nnn>` label |
| party.mode_rosters | partial | Separate Action and Tactical teams | Priority 3 |
| party.assists | server | Up to 3 assists per seat (Build 22); assist abilities folded onto the seat statue and the assist character's skill on button 6 (Build 24); assist-only statues (Build 25) (docs/research/19 §4–6) | unverified |
| party.assist_slots | server | 3 assists per seat; each adds its share of stats; one assist skill per team (the leader seat's first assist, client) (docs/research/19 §3, §5.2) | "Only 4-star assists have an active skill" not applied: every assist has one (ours) |
| party.friend_tactical | server | Five AI supporters a day; the tactical battle applies a FollowEach supporter's leader ability to the team (client); +10 Friend Points per clear with one (Build 25, docs/research/24 §1) | unverified; no other players offline |
| quest.story | server ✓ | All chapters open; stage window, missions (Build 19), clears recorded; resumption of a battle left unfinished (Build 25, docs/research/22 §1.8) | Action-stage missions: priority 4 |
| quest.daily | server | Today's tile by weekday (category 6 + weekday, 04:00 JST), each weekday's two quest lines with Easy–Expert rows, AP and rewards; clears and stars per stage (Build 20; rotation re-checked and tested in Build 25, docs/research/13 Quest Select) | No per-day play limits (no source gives any) |
| quest.support | partial | AI supporters for quests (Build 25, docs/research/24 §1) | The Action battle's callable support is not served |
| quest.mission_predicates | partial | Tactical DT_QM 1/2/10 (Build 19) | Action predicates: priority 4 |
| quest.support_finisher | offline | — | Action battle only; provisional source |
| quest.rank_ap | server | Rank from EXP (`rank_for_exp`); AP is always full | Refill on rank-up is moot (offline choice) |
| quest.rank_example | server | Stage rank EXP from the wiki per stage | — |

## Action Battle (client side, not checked offline yet)

`action.controls`, `action.resources`, `action.type_triangle`, `action.dododo_charge`,
`action.dododo_types`, `action.dododo_during_sp`, `action.flinch_response`,
`action.stand_hurtbox`, `action.area_carry`, `action.obstacles`, `action.body_collision`,
`action.unity_solitary` and `action.typeless` are behaviours of the client's Action battle.

We send its tables (reaction tables, `battle_damage_param`, `battle_combo_param`,
`oraoramode_setting`, `bullet_parameter`, `skill_data_table`, `statue_affinity_table`).

- **Unverified:** no Action quest has been played on a device yet.
- `action.resume` — **server (Build 25)**: `bizarre/bizarre_resumption` offers a battle that was
  started and never ended (story, daily, event or a Labyrinth room) or a Labyrinth Tarot pick;
  No drops it (docs/research/22 §1.8). The pack's checkpoint detail (areas, saved HP) is the
  client's.
- `action.solo_exp`: statue EXP is split as the clear pays it (ours).

## Tactical Battle

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| tactical.active_reserve | client ✓ | Formation rows (`area_position_pattern_table`) | — |
| tactical.turn_queue | server ✓ | skillWait 100/50/0 drives the queue (docs/research/11 §3) | Values ours |
| tactical.ct | partial | Cool turns by skill level (client: min + ceil((max − min) / 10 × level)): 2/3 up to level 4, one less from 5, two less at 10; tree skill squares and Mind levels raise the level (Build 25, docs/research/26 §4) | Values ours; no source for per-skill CT |
| tactical.targeting | client ✓ | — | — |
| tactical.pause_auto | client | Auto and pause work; pause lists the stage missions (Build 19) and the supporter (Build 25) | unverified |
| tactical.evasion_crit | server | `_missDamageRate 0.25`, `_criticalDamageRate 1.5` (wiki, Build 18) | — |
| tactical.reversal | server | On since Build 20: gauge from `dododoRate` (10/15/20%), hits taken (10%) and deaths (25%); 3 actions at 10× speed, no cool-downs (values ours) | unverified on a device |
| tactical.overkill | client | Remaining Reversal actions become overkill hits after a boss kill in the last area (`_reversalMaxTurn` 3) | unverified |
| tactical.skill_families | partial | Damage, heal and single/all classification (`build_statues.py`); time stop (Build 25) | Buff, debuff and bind families need `dtBuff` rows; the client's own `DTSC_*` labels give each family's target and turns (docs/research/26 §5) |
| tactical.time_stop | server | 7 skills whose text says the user acts in stopped time get the client's `DTSC_WORLD_LIGHT` / `_MIDDLE` sub category (ProcessType 4, 1 or 2 turns; Build 25, docs/research/26 §5) | Turn counts ours; unverified on a device |

## Progression

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| progress.levels | server | Level and EXP per statue; growth tree progress; duplicate boosts from turtles (`OwnedStatue.boost`, shown as `convergence`); equipped Minds per seat (Builds 21–23, docs/research/15, 16) | — |
| progress.tree | partial | Server-built branching tree (`tree_squares`) with material, gold and key costs (Build 23) | Original node graphs and costs unknown (ours); SP, DoDoDo and Mind-capacity squares not built |
| progress.mind_capacity | server | Slots 1–4 open, capacity 40, cost 3 × rarity; over-capacity and locked slots refused with the game's codes (Build 21, docs/research/16) | Tree squares that raise capacity: not built |
| progress.mind_links | server | Mind links (Build 25): ATK/DEF/HP links from Gamerch, a `mindLinkList` per seat, abilities ours and folded into the statue's abilities (docs/research/26 §2) | Link amounts ours; move-speed and crit links left out |
| progress.mind_lock | server | The heart lock is stored (`/mind/protect`, it crashed before); locked and equipped Minds cannot be levelling fodder (Builds 21, 23, docs/research/16 §7) | unverified |
| progress.mind_levels | server | Minds level 1–10 by rarity curve with Mind medals or other Minds, the client's EXP and gold rules (Build 23, docs/research/16 §7); Mind abilities scale with level (§6) | unverified |
| progress.material_bonuses | server | Enhancement with materials: the client's arithmetic, ×1.2 for a matching attribute (medals), trophies for any statue, Mind medals for Minds, ×5 platinum (Build 21, docs/research/15) | Tier gold costs ours |
| progress.turtles | server | Bronze/silver/gold/rainbow turtles add one boost to a 3/4/5/6-star statue (Build 21, docs/research/15) | unverified |
| progress.material_migration | server | Enhancement materials live in the item inventory (`item/exp_item_list`), not as Minds (Build 21) | — (3.12.0 is the post-2019 layout) |
| progress.awakening / awakening_boost / awakening_requirements | offline | Every rarity variant is already owned; `statue/evolution_info` answers | Awakening transaction moot offline |
| progress.skill_slots | server | Normal and special skills per statue (Build 13); skill Minds in frames 5 and 6 add a second normal or special skill (Build 25: 87 Minds, docs/research/26 §3); tree skill squares raise skill levels | Rows for Mind skills that are no statue's skill are ours |

## Economy

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| economy.wallets | server | Gold, rank EXP, Memory Rings, Friend Points, Fight Coins, event points and Arena BP are real; Diamond Coins and AP never run out (offline choice) | — |
| economy.summon_prices | offline | Banners free (`payType 2`, price 0) | Choice: no payments |
| economy.guaranteed_slot | server | Normal draws 86/12.5/1.5 (3/4/5-star); the tenth draw of a 10x summon is the guaranteed slot, 98.5/1.5 (4/5-star; launch guide), since Build 20 | — |
| economy.friend_gacha | server | Friend Point Mind Summon (Build 23, docs/research/21 §4) and its daily free draw (Build 25, docs/research/13 §1) | Price 200/2,000 and rates ours (the pack says 100/1,000) |
| economy.inventory_expand / refill_revive | offline | Continue is free; no inventory cap | — |
| economy.box / count_up | server | Statue Box Summon (50 per box, Reset, Box Status) and the Count Up rounds with tickets in the Message Box (Build 22, docs/research/21) | Contents and ticket ours |
| economy.exchange | server | Ring Exchange (Build 21, BP Colas since Build 25); event exchange for Fight Coins (Build 22, docs/research/20) | Awakening fragments and certificates: no awakening offline |
| economy.cola | server | BP Recovery Colas (1101 2 BP, 1102 5 BP; their texts) restore Arena BP via `/user/recover_battle_point`; 5 of each to start, 50 Memory Rings at the Ring Exchange (Build 25, docs/research/23 §1.10, §2) | Cola 1100 (1 BP) has no icon in the player's files |
| economy.daily_reset | server | 04:00 JST for daily missions, the daily tile, Mentality, Arena days and the free Friend Point draw (`missions::game_day`) | The one-coin summon offer: paid, offline choice |

## Social, Arena, Labyrinth, events

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| social.friends | offline | Follow, follower and block lists empty; supporters are AI (docs/research/27) | No other players |
| social.town / town_points / town_growth / town_payout | server | A solo Town with AI members (Build 25, docs/research/24-town.md): create and lead a Town or join one of 12 AI Towns; AI applicants, Sub-Masters, expel; the Town home over the client's realtime socket on the HTTP port (members walk, chat kept); Town Medals (item 41) donated for points and levels from `clan_table`, objects unlocked by level, the donation gift and level rewards in the Message Box | AI Towns, curve and rewards ours; unverified: spawn points (near the origin), the Town Medal icon, object models matched by name |
| arena.* (12 rows) | server | The 2019 rule set (docs/research/23): a separate five-team roster; AI opponents and a free shuffle; durability with a 30-minute rest (`arena.durability_2019`); Fever after four battles at triple points; streak, team-power and survivor scoring; BP costs that rise after each battle, drop for the other teams every fourth battle and reset with a full bar, BP regained over time (`arena.bp_cost`, Build 25); rankings, defence, Crazy Diamond, abandoned battles as losses (Build 24) | Values ours; the client's edit gate is not enforced by the server; the 2017 rules (one-hour rest, revive-all threshold, cost-decay counters) stay out |
| labyrinth.* (5 rows) | server | Beginner, Intermediate and Advanced dungeons (Build 25), 20 floors of rooms built from story stages, Mentality 300 refilled at 04:00 JST, 3 Souls, Tarot select and flip, rewards on an escape or full clear, resumption of a battle or a Tarot pick (docs/research/22) | Rooms have one area; the refill is full (pack: +100); unverified |
| event.quiz / high_score / bonuses | server | High-score events 560001 (with shop) and 560005: per-stage rules, bests, the result panel, a ranking; quiz event 50090 with the answers from its scripts and certificates; bonus statues and Minds per event (Build 25, docs/research/25-events-raids.md) | Score formula, quiz rewards and bonus units ours; Mind tokkou abilities not built |
| event.town_ranking / reruns / end_service | offline | Events always open (open calendar); the Town home's event panel stays hidden and member event points are 0 | No Town events offline |
| event.raid | client | The raid screens are not in the player's files, so a raid boss never appears; the six `/raid/*` routes answer an inactive raid (Build 25) | Unreachable with these files |

## Presentation

- **Client:** `obscure.title_voice`, `obscure.loading`, `obscure.enya_trip`, `obscure.night`
  (a supporter's statue is now sent with the battle, so a DIO supporter may trigger it; client,
  unverified), `obscure.stage_variants`.
- **Offline:** `obscure.cached_daily` (a daily category asked for after the day turned is still
  served).
- **Kept out:** `obscure.unused`. The roster is the 266 statues with models and leader abilities
  in the player's files, plus the 26 assist-only statues with a model and their own assist label
  (Build 25); reported unused units stay out, as the pack advises.

## Numeric pack

`prior_numeric_data/` is the earlier collection, already merged in Build 18 (5 more statues, 11
confirmed). It is unchanged, so there is nothing new to import.

## Revision 2 of the research pack

The owner sent a revised pack (filed privately as `local/wiki-raw/research-2026-11-15-v2/`). It
adds 47 behaviour notes, 33 player flows, upright values for the 22 Tarot cards, 7 event
examples and 8 event Minds. The numeric data is unchanged.

| Note | Status | Our implementation | Gap / next |
|---|---|---|---|
| v2.growth.weekday | server | Daily enemies follow the material days: Mon/Thu Wisdom, Tue/Fri Spirit, Wed/Sat Courage, Sun all three (`build_extra_quests.py`, Build 20) | — |
| v2.growth.party_exp | server | A clear gives statue EXP to the three statues that fought | Assists and supporters gain none (ours) |
| v2.growth.launch_slots | server | Four Mind slots open (Build 21, docs/research/16) | Dated evidence; slots 5–8 stay closed |
| v2.growth.cp_level | server | Capacity is a fixed 40; levels never raise it (Build 21) | CP tree squares not built |
| v2.growth.cp_reject | server | A Mind over capacity is refused (51705, Build 21) | — |
| v2.growth.tree_resources | partial | Server-built tree with stat squares and material costs (Build 23) | SP, DoDoDo stocks and Mind-capacity squares |
| v2.growth.material_rarity | server | Bronze/silver/gold/rainbow tiers, ×1.2 for a matching medal (Build 21, docs/research/15) | Gold tier EXP ours |
| v2.growth.crossfeed | server | Any material feeds any statue; trophies favour statues, Mind medals Minds (Build 21) | — |
| v2.growth.assist_display / assist_conditions / assist_scaling | server | Assists per seat with the client's stat formula (constants ours), abilities with the client's condition check (conditions empty, ours), assist skill (Builds 22, 24, docs/research/19) | Original conditions unknown |
| v2.action.armor_weak / armor_strong / crit_legacy / unity_phases / unity_stack / zeppeli_sacrifice | client | Action battle behaviour; the client's tables decide | Action battle is unverified offline |
| v2.arena.* (8 rows) | server | AI-controlled opponents built from our roster (2018 guide); the 2019 rule set; Fever 10 minutes (`early_fever`); missing or abandoned results count as losses (`early_failure`); BP 5 with rising costs and BP regained (`early_bp`, `early_cost`, Build 25) (docs/research/23) | No in-battle character switching (the 2019 AI arena has none) |
| v2.labyrinth.* (10 rows) | server | 20 floors; floor rewards wait in the run and go to the Message Box on a full clear or an escape; a defeat or retire costs a Soul, the last one forfeits the run; escape restores the Souls; 22 Tarot identities, three offered, each used once until the deck resets; flipping all three costs 20; room costs 22–35; three grades (Build 25) (docs/research/22) | Card effects only where an existing affect fits; rooms have one area |
| v2.countup.old_ticket / ticket_pool / ticket_delivery | server | Tiers 20/30/50/50/50; the ticket goes to the Message Box (Build 22, docs/research/21) | Which ticket each round gave is unknown (ours) |
| v2.events.* (9 rows) | partial | Nine events playable (Build 20, 25): the Fight Coin exchange (Build 22); event 50045's points and point rewards (Build 23) and ranking against AI rivals (Build 24, docs/research/23-arena.md §3); high-score events 560001/560005 with per-stage rules, bests, the result panel, a ranking and a high-score shop; quiz event 50090 with certificates; event bonus units; campaign banners on Quest Select and the Arena home (Build 25, docs/research/25-events-raids.md) | No ranking rewards (events never close offline); raids unreachable (no raid scene in the player's files); one-event examples, not general rules; never sum anniversary tiers |

**Player flows** (`catalog/player_flows.json`, 33). Their audit checks match the rows above. The
screens the Build 20 audit found empty (Missions, Mind equipment, assists, enhancement materials,
event exchange, box and Count Up summons) are built (Builds 21–24); still empty are the Town,
JoJo and Mind links, Skill Minds and the Action battle's own rules.
