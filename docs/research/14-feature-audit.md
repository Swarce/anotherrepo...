# 14 — Feature audit against the owner's research pack (Build 20)

The owner's research pack (2026-11-15, gathered with an outside research assistant; private,
under `local/wiki-raw/`) lists 118 documented behaviours of the public game. Each has sources, an
era/mode and an uncertainty. This file maps every one to our code. The research ids are research
identifiers, not game ids, and are never sent to the client.

Target: the 3.12.0 client (the last version), with Tactical Battle primary and compatible Action
Battle quests retained.

**Status:**
- **server**: implemented in our server.
- **client**: client behaviour that needs nothing from us beyond data we send (✓ seen on a device).
- **partial**: some of it is implemented.
- **missing**: needs server work.
- **offline**: a deliberate project choice (no payments, everything owned, open calendar).
- **unverified**: not checked on a device yet.

## Priorities

1. **Tactical Reversal is switched off** (`tactical.reversal`, `tactical.overkill`; documented).
   `dtbattle_damage_param` sends `_reversalEnable 0`, a Build 11 choice made only to get past
   BattleInit. It is a core tactical mechanic, and two of the game's missions use it.
   → **done in Build 20** (`docs/research/11` §5): Reversal on, gauge from skills, hits taken and
   deaths; 3 actions with overkill.
2. **Missions screen is empty** (`home.missions`, `quest.mission_predicates`).
   `trophy/trophy_list` returns nothing. Daily, cumulative and limited missions need definitions
   and rewards that no source gives. The text tables hold the mission wording.
   → next: a small offline set built from the game's own labels, rewards ours and marked.
3. **Action-mode roster compatibility** (`party.mode_rosters`).
   Only Action-compatible statues may enter Action quests. We let every statue into both teams.
   → needs the client's compatibility flag (statue info) and a check of the Action battle on a device.
4. **Action-stage mission rows** (`quest.story`, `quest.mission_predicates`).
   Tactical stages send three missions (Build 19). Action stages send none: their
   QuestMissionClearType list and labels differ and are not traced yet.
5. **Daily boundary at 04:00 JST** (`economy.daily_reset`, `labyrinth.resource`).
   Nothing daily exists yet. When missions or a login bonus arrive, use 04:00 JST (`server/core/src/clock.rs`).

Everything else is a client behaviour, already server-side, out of scope offline, or a later
system (Arena, Labyrinth, Town, events) that the client needs a whole protocol for.

## Boot and account

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| boot.terms | server | Tutorial flags kept per account (`user/tutorial`, `get_tutorial_step`); the battle tutorial pays once per started battle (`Progress.pending`) | Interrupt at each step on a device and check for repeats (unverified) |
| boot.tutorial_guarantee | offline | Every statue is owned; the tutorial summon draws from the owned roster | — (provisional source) |
| boot.naming | server | `user/update_username` | — |
| boot.title | client ✓ | Title from the APK; art pack redraws it | — |
| boot.transfer_issue / transfer_expiry / transfer_input | offline | One local account; the button shows its screen with our English | No transfer partner offline; saves can be backed up as files |
| boot.save_choice | client | Save select is one of the APK scenes we translate | unverified |
| boot.cross_os | offline | No paid currency | — |

## Home and menus

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| home.leader | server ✓ | Main tactical team leader on My Page (`statue_setting_table`, Build 12) | Alternate voice lines are client side |
| home.resources | server ✓ | `user_info`: rank, AP (always full), gold, coins | — |
| home.present_box | server | `present/present_list`, `present/get`, `get_all` (empty box) | — |
| home.missions | missing | `trophy/trophy_list` empty | Priority 2 |
| home.notices | partial | `info/*` routes return empty lists | Offline: no notices (choice) |
| home.collection | server | `book/statue_list`, `mind_list`, `item_list`, `yogo_list` | Collection achievements depend on missions |
| home.profile | server | `user/update_comment` saves the greeting | — |
| home.options | client ✓ | Options grid works; labels in English (Build 19 shortened) | Purchase history / transfer have no backend (offline) |
| home.settings | client | Local settings | unverified |
| home.bulk_download | client | The client's own download path; we serve `/cdn` | — |

## Party and quests

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| party.presets | partial | 5 teams per battle type (`collection.rs`, team numbers 1–5 since Build 11) | The launch narration says 10; check which count the 3.12.0 team screen pages through |
| party.three | server ✓ | Leader, left, right slots per team | — |
| party.variants | server ✓ | Each rarity/type variant is its own statue (`statues.json`, 266) | — |
| party.jojo_links | missing | `Ability_JOJOlink_Name` labels exist; no link rows sent | Provisional source; needs stronger evidence |
| party.mode_rosters | partial | Separate Action and Tactical teams | Priority 3 |
| party.assists | partial | Assist ability labels exist; statues carry no assist Minds yet | Tied to Mind equipment |
| party.assist_slots | missing | — | Needs the Mind equip routes (not called in our logs yet) |
| party.friend_tactical | offline | `support/support_user_s` returns no friends | No other players offline |
| quest.story | server ✓ | All chapters open; stage window, missions (Build 19), clears recorded | Action-stage missions: priority 4 |
| quest.daily | partial | Daily chapters are listed with the story (Gamerch data) | Day-of-week rotation not applied (offline: always open) |
| quest.support | offline | No friend support | — |
| quest.mission_predicates | partial | Tactical DT_QM 1/2/10 (Build 19) | Action predicates: priority 4 |
| quest.support_finisher | offline | — | — |
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
- `action.resume`: `bizarre/bizarre_resumption` returns nothing to resume (missing; low).
- `action.solo_exp`: statue EXP is split as the clear pays it (ours).

## Tactical Battle

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| tactical.active_reserve | client ✓ | Formation rows (`area_position_pattern_table`) | — |
| tactical.turn_queue | server ✓ | skillWait 100/50/0 drives the queue (docs/research/11 §3) | Values ours |
| tactical.ct | partial | Skill cool turns 2 (normal) / 3 (special), ours | No source for per-skill CT |
| tactical.targeting | client ✓ | — | — |
| tactical.pause_auto | client | Auto and pause work; pause should now list the stage missions (Build 19) | unverified |
| tactical.evasion_crit | server | `_missDamageRate 0.25`, `_criticalDamageRate 1.5` (wiki, Build 18) | — |
| tactical.reversal | server | On since Build 20: gauge from `dododoRate` (10/15/20%), hits taken (10%) and deaths (25%); 3 actions at 10× speed, no cool-downs (values ours) | unverified on a device |
| tactical.overkill | client | Remaining Reversal actions become overkill hits after a boss kill in the last area (`_reversalMaxTurn` 3) | unverified |
| tactical.skill_families | partial | Damage, heal and single/all classification (`build_statues.py`) | Buff, debuff, bind and summon families wait for buff data |
| tactical.time_stop | missing | Time-stop buffs have labels only | Later |

## Progression

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| progress.levels | partial | Level and EXP per statue; growth tree progress saved (`OwnedStatue.tree`) | Duplicate boosts and Minds are not modelled |
| progress.tree | partial | Server-built branching tree (`tree_squares`) | Original node graphs unknown (gap noted by the pack too) |
| progress.mind_capacity / mind_links / mind_lock / mind_levels | missing | Mind list and Mind summon exist; no equip, links or locks | Needs the Mind equip/lock routes |
| progress.material_bonuses / turtles / material_migration | missing | `statue/strengthening` accepted; no item inventory | Later (item system) |
| progress.awakening / awakening_boost / awakening_requirements | offline | Every rarity variant is already owned; `statue/evolution_info` answers | Awakening transaction moot offline |
| progress.skill_slots | partial | Normal and special skills per statue (Build 13) | Skill Minds wait for Mind equipment |

## Economy

| Feature | Status | Our implementation | Gap / next |
|---|---|---|---|
| economy.wallets | partial | Gold and rank EXP are real; coins 0; AP always full | Offline choice |
| economy.summon_prices | offline | Banners free (`payType 2`, price 0) | Choice: no payments |
| economy.guaranteed_slot | server | Normal draws 86/12.5/1.5 (3/4/5-star); the tenth draw of a 10x summon is the guaranteed slot, 98.5/1.5 (4/5-star; launch guide), since Build 20 | — |
| economy.friend_gacha | missing | — | Friend points have no use offline |
| economy.inventory_expand / refill_revive | offline | Continue is free; no inventory cap | — |
| economy.box / count_up | missing | — | Later, needs banner data |
| economy.exchange | partial | One shop with no goods; event shops empty | Later |
| economy.cola | offline | BP unused | — |
| economy.daily_reset | missing | No daily logic | Priority 5 |

## Social, Arena, Labyrinth, events

Out of scope offline, or later systems:

- **No backend yet (offline):** `social.friends`, `social.town`, `social.town_points`,
  `social.town_growth`, `social.town_payout`. Follow lists, blocks and Town search return empty.
- **Missing:** `arena.*` (12 rows, two rule sets: 2017 and 2019). The client's Arena needs its
  own routes, none of which appear in our logs. Keep the rule sets apart if it is ever built.
- **Missing:** `labyrinth.*` (5 rows). The labyrinth chapters exist in `quests.json`. Mentality,
  souls, tarot and route choice need the dungeon routes, and `bizarre_end`'s `dungeonClearFlag`
  is 0.
- **Offline (open calendar):** `event.quiz`, `event.high_score`, `event.town_ranking`,
  `event.bonuses`, `event.reruns`, `event.end_service`. Event chapters are open; scoring and
  rewards are not implemented.

## Presentation

- **Client:** `obscure.title_voice`, `obscure.loading`, `obscure.enya_trip`, `obscure.night`,
  `obscure.stage_variants`.
- **Offline:** `obscure.cached_daily`.
- **Kept out:** `obscure.unused`. The roster is the 266 statues with models and leader abilities
  in the player's files; reported unused units stay out, as the pack advises.

## Numeric pack

`prior_numeric_data/` is the earlier collection, already merged in Build 18 (5 more statues, 11
confirmed). It is unchanged, so there is nothing new to import.
