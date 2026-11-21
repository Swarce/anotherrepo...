# 13 — What the MyPage menus need from the server (Build 15)

Our own analysis of the 3.12.0 client. Addresses are RVAs in `libil2cpp.so`; H/M/L as in the
other notes. No game text is reproduced.

## 1. Summon (gasha)
- **Why it hung (H).** `GashaUIController.StartLoad` 0xDDEC80 splits `gashaList` into statue banners
  (types 1/3/5) and mind banners (types 2/4/6). With no statue banner the screen opens the Mind tab.
  `SetBoxGasha` 0xE63F8C then reads entry 0 of that tab's list (`boxFlag`, `boxResetEnable`,
  `boxRemain`) with no length check.
- **What we send.** One statue banner (300000) and one mind banner (400000).
  - `gashaWindow` must be a banner bundle that exists in the player's files: `gw_<id>_<suffix>`,
    461 of them. The client downloads `gashaWindow + ".bytes"`.
  - `gashaAppealInfo` must be a list.
  - `payType` 4 (paid in diamonds) with price 0: draws are free offline.
- **Draw** (`/gasha/gasha_play`).
  - The response is `{userInfo, getResultList, originalStatueInfo, Lv1StatueMasterInfo}`; both
    trailing lists are required.
  - Each result is `{dataType, dataInfo, datauId, level, newFlg, effectInfo{statue…, mind…}}`.
  - Rarity follows the Gamerch rates: statues ★5 1.5%, ★4 12.5%, ★3 86%; minds ★4 7%, ★3 93%.
  - Everything is already owned offline, so a draw shows the owned copy.
- **Free draw (Build 25).** `freeDrawFlag` > 0 adds a free play button (button type 10,
  `setupPlayButton` iterator 0xDE21B8); it is lit while `freeDrawRemainingNum` > 0 (0xDE2B34) and
  sends `countType` 1, `playType` 3 (`GashaButtonSetEvent` lambda 0xE665EC) (H). The Friend Point
  summon (290000) sends flag 1, limit 1 and today's remaining count; a free pull costs nothing
  and a second one gets 51302 (E_GASHA_REMAIN_FREE_EMPTY). One a day, at 04:00 JST, is ours
  (research pack `economy.daily_reset`). `discountDraw*` (the one-coin offer, checked first at
  0xDE2160) stays 0.

## 2. Shop
- **Why it hung (H).** `Shop1UIController.Update` 0xD82C5C reads `shopList[0].goodsList.Count` with
  no length check.
- **What we send.** One shop with an empty goods list. `itemSetInfo` objects carry all four lists.
  `shop_buy` and `event_shop_list` answer without changing anything.

## 3. Growth tree
- **The client has no tree data (H).** `ui_tree` holds only tiles and icons. `/tree/skill_list` must
  send the whole graph: `{treeInfo{treeId, squareList[SquareInfo]}, itemList}`.
- **What each square needs (H).**
  - A non-null `squareEffectList` object; it is a single object, not a list.
  - Non-null `openMaterialList` and `lockList`.
  - When closed, a `beforeSquareId` that names an existing square.
  - The root square is open, with effect 0.
- **Effect ids (H, Build 42).** Read in `GrowthTreeBoardUIController`: the tile icon
  (`ConvEffectIdToTextureIndex` 0xDECF3C, jump table 0x223E7C0, enum `EffectTextureId`), the tile
  label (`GetTreeEffectName` 0xDECCB8) and the window text (`GetTreeEffectExplainName` 0xDECDC0;
  both accept 1-14 except 6 and 8, mask 0x3F5F, labels via the relocated tables 0x2B5B6A0 /
  0x2B5B710), and the status preview (`UpdateUI` 0xDED0EC, jump table 0x223E80C).

  | Id | Icon | Tile label / text | Preview adds to |
  |---|---|---|---|
  | 1, 2, 3 | `Tip_Atk/Def/Hp_000-002` by `iconLevel` | `GRID_LABEL_ATTACK/_DEFENSE/_HP`, `Tree_Message_05/06/07` | ATK, DEF, HP |
  | 4 | `Tip_Skill1_000` | `GRID_LABEL_NORMAL_SKILL_1`, `_08` | skill 1 |
  | 5 | `Tip_SPSkill1_000` | `GRID_LABEL_SP_SKILL_NEW`, `_09` | SP skill 1 |
  | 6, 8 | `Tip_Skill2_001`, `Tip_Skill2_000` | none (masked out) | 8: skill 2 |
  | 7 | `Tip_SPSkill2_001` | `GRID_LABEL_SP_NEW` (finisher unlocked), `_10` | nothing |
  | 9 | `Tip_SPSkill2_000` | `GRID_LABEL__SP_SKILL_2`, `_11` | SP skill 2 (finisher) |
  | 10 | `Tip_Cp_000/001` by `iconLevel` | `GRID_LABEL_MIND_CAPAXITY`, `_12` | Mind capacity (+param) |
  | 11 | `Tip_LAbility_000` | `GRID_LABEL_LEADER_ABILITY`, `_13` | leader ability level |
  | 12 | `Tip_Item_000` | `GRID_LABEL_STORN` (one Diamond Coin), `_14` | nothing |
  | 13 | `Tip_AssistSkill_000` | `GRID_LABEL_ASSIST_SKILL`, `_18` | assist skill level |
  | 14 | `Tip_Evolution_000` | `GRID_LABEL_EVOLUTION`, `_19` | nothing |

  This confirms our old list (12 diamonds, 13 assist, 14 evolution). The PC playtest's ids
  differ (12 evolution, 13 item, assist on 4); the client wins.
- **The window (H).** Current values come from the StatueInfo: `mindCapacity` (a fixed text
  instead for `assistAttr` 1), `leaderAbilityInfo.abilityLevel` (`dtLeaderAbilityInfo` when
  `useStageType` is 2; we send 0) and the `skillList` levels; the selected path's params are
  shown as "+N" / the next level.
- **No original layout survives, so ours is:**
  - a start square, then 14 squares in a line cycling ATK, DEF, HP;
  - together they add the wiki's tree-max minus Lv99 stats (20% of Lv99 when unknown; from
    Build 41 the tree max is known for 225 of the 228 statues with wiki stats, docs/CONTENT.md
    "Wiki cross-checks");
  - squares open in order (`/tree/acquisition`: a square needs the one before it);
  - opened squares are saved per statue and added to its stats.
  - Build 25: up to four skill-level squares (effects 4 and 5) after the ATK and DEF lines and
    their forks; each adds 2 levels to the normal or special skill (docs/research/26 §4).
  - Build 42 (ours, docs/CONTENT.md "Growth tree square kinds"): finisher squares (9) after the
    SP fork; past the HP line eight CP squares (10, +4) in two arms ending in a Diamond Coin
    square (12, the coin is granted when it opens), three leader squares (11, +3 levels) in the
    middle and the awakening square (14, cosmetic) last. Assist-only statues get three assist
    squares (13, +3 levels) instead of CP and leader. Not built: 6/8 (no second skill in 3.12)
    and 7 (a finisher gate would lock what rarity 5 gives today).
  - Effects: `mindCapacity` = max(40, 16 + CP squares) (docs/research/16); the leader level
    goes into both `leaderAbilityInfo` and `dtLeaderAbilityInfo`; the finisher level into the
    action `skillList[4]`; the assist level into `dtAssistSkillInfo`.
  - Open (device): the board now reaches column 13; whether the tree screen's scroll area shows
    it all, and the new tiles' icons and texts.
- **Costs (Build 23).**
  - **Client (H):** it checks costs only against the tree screen's own `itemList`; gold is an
    ordinary material, item 20. A locked square (`unlockFlg 0`) needs `openMaterialList` and
    `lockList` both; there is no separate unlock route. A far tap sends the whole path
    (`multiOpen 1`), trimmed to what the player can pay.
  - **Ours:** each square costs a tree material of the statue's colour (the item details give
    Courage blue, Wisdom green, Spirit red; Unity and Solitude use white and black) at a tier set
    by depth (Shard, Stone, Brooch, Gem, Tablet, Slab), 2 + depth of them, and 1,000 x depth x
    rarity gold. The first fork square of a branch is locked behind that colour's Expansion Key.
  - The server pays square by square along the path and stops at the first it cannot pay
    (10102 gold, 10108 items).

## 4. Other screens

| Screen | Routes | What we send |
|---|---|---|
| Dictionary | `/book/statue_list`, `/book/mind_list`, `/book/item_list`, `/book/yogo_list` | Every statue (Lv1 and Lv99 info) and every mind, all met |
| Enhance | `/item/exp_item_list` (on entry), `/statue/strengthening(_item)` | No EXP items yet; the statue is unchanged |
| Evolution | `/statue/evolution_info` | Every view shows the statue as it is |
| Mind sell | `/sell/sell` | Nothing sold |
| Inbox, trophies | `/present/get(_all)`, `/trophy/trophy_result_get` | Empty |
| Protect, sort order, "new" marks | `/mind|statue/protect`, `save_sort`, `new_flg_release` | Accepted |

All three priority diagnoses and every field layout are H. The secondary screens were checked for
which route they call and which lists must be non-null (M).

## Quest Select: Daily, Event and Campaign (Build 20)

The owner asked for these tiles so more content can be tested. H unless marked, from
`BizarreCategorySelectUIController` (OnLoad 0xD3E398).

- **Tiles.** Picked by `bannerType` only: 1 story, 2 daily, 3 event. A tapped tile sends
  `bizarre_list{category}`.
  - `questHomeId`, `day`, `Priority` and the dates are not read.
  - Tutorial flags gate the tiles: daily needs OPEN_00, event needs OPEN_01. The embedded server
    skips the tutorial, so both are set.
  - `dungeonLockFlg: 1` keeps the labyrinth locked; we now send 0 (docs/research/22-labyrinth.md).
  - A null `bannerFileName`, or a null `eventMasterId` while campaigns are shown, crashes the
    screen; an empty or missing banner just leaves the tile blank.
- **Daily.** The client uses the first bannerType-2 entry for both its category and its art.
  - We send today's weekday (category 6 + weekday, the day turning at 04:00 JST, research pack
    `economy.daily_reset`) with art `bizarre_type_daily_<sun|mon|tue|wed|thu|fri|sta>`.
  - An empty chapter list shows an "expired" dialog. The story ChapterInfo with the daily
    category works.
- **Event.** Categories 13–1000 open Event Home.
  - `eventInfo.eventCampaignId` must be set, or the screen backs out with "expired".
  - Ranking, collection points, shop, BGM and the event summon stay off (0 or "").
  - Event chapters need category ≥ 13 to return to Event Home after a battle.
  - The tile art is `bannerFileName + "_large"`; the player's files hold 180 such banners
    (`eq_<6-digit id>_<hash>_start_large`).
- **Campaign Info.** Items with `bizarreHomeFlag` true, ordered by `signboardPriority`, with art
  `banner + "_small"`. A tap opens `s3_url + htmlLink` in the web view (our web module answers
  any path).
- **Rotation and reset (Build 25 check).** The tile, its category and the art follow the weekday
  of the game day (`quests::daily_on`, `missions::game_day`: the day turns at 04:00 JST; tested
  for all seven days and the 03:59/04:00 edge). Every weekday has its own chapters. A daily
  category asked for after the day turned is still served (the research pack's
  `obscure.cached_daily` describes the same leniency; ours). Daily missions and the Labyrinth's
  Mentality turn on the same boundary.
  - The research pack's `economy.daily_reset` names a daily free friend draw: the Friend Point
    Mind Summon now has one free single draw a day (§1, Build 25).
  - Not built: the daily one-coin summon offer (`discountDraw*`; paid currency, no payments
    offline) and per-day play limits on daily stages (no source gives any).
- **Content** (`server/content/quests_extra.json`, `tools/content/build_extra_quests.py`).
  - Daily: per weekday, the game's two quest lines (`CHAPTER_NAME_DY_<day>_01`/`_04`), each with
    its Easy–Expert stages (`STAGE_DY2_<day>_001..004`, `_101..104`). Enemies follow the
    documented material days (Mon/Thu Wisdom, Tue/Fri Spirit, Wed/Sat Courage, Sunday all three;
    research pack v2.growth.weekday).
  - Events: 50035 Cinderella's Wizard, 50045 The Girl Standing in the Alley, 50032 Challenge
    Letters, 50077 2nd Anniversary. That is 72 chapters and 222 stages in all, each with its own
    chapter and stage label from the game's text and its banner (id `50032` → banner
    `eq_500032_…`; earlier event ids map ambiguously, so they are left out). Build 25 adds five
    more events (high score, quiz, exchange): 139 chapters and 393 stages (docs/research/25 §6).
  - Every stage reuses a tactical story stage's map. Enemies, levels and rewards are ours,
    except the Easy-tier daily AP/EXP/gold (wiki). Clears and stars are kept per stage, apart
    from the story.
