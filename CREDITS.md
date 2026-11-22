# Credits

## Game-content sources

Values the original server held, and which no player copy contains (stats, skill numbers, level
curves, gacha pools, shop prices, stage data), are rebuilt in `server/content/` from public
community sources. Every file there names its sources; this page lists them all.

What we take: **facts and numbers only** (a stat value, a cooldown, a price). We do not copy
descriptions, guides, tables' layout, images or any other prose from these sites; text shown in the
game comes from the game's own files or is written by us.

| Source | Used for | Files |
|---|---|---|
| [Gamerch ジョジョDR攻略Wiki](https://jojodr.gamerch.com/) (archived pages via the Wayback Machine) | statue rarity, type and stats (129 statues); story stage AP, rank EXP, gold and recommended level (86 stages); mind rarity, cost and flat stats (417 minds); the player rank table | `server/content/statues.json`, `quests.json`, `minds.json`, `ranks.json` (per-record page) |
| [Diamond Records Wiki (ShoutWiki)](https://jojodiamondrecords.shoutwiki.com/), [CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/) | statue rarity, type, Lv1/Lv99/max stats (11 statues); assist-ability stat and value (35 statues) | `server/content/statues.json`, `assist_abilities.json` (per-record page + revision) |
| [JoJo's Bizarre Encyclopedia (JoJoWiki)](https://jojowiki.com/Diamond_Records), [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) | statue rarity, type, Lv99 and max-tree stats (Build 33) | `server/content/statues.json` (per-record page + revision) |
| Diamond Records Preservation (PC playtest) by C0rinah, used with the author's permission (2026-11-22) | the Town object unlock order, the Mind capacity base of 16, the choice of the three action-stage missions, and Lv10 stats for 9 Minds (from original-game screenshots and an archived translation); kept until a wiki source replaces them | `server/services/src/town.rs`, `collection.rs`, `quests.rs`, `server/content/minds.json` |
| Our own research (`jdr-research`) and defaults (`jdr-default`) | tutorial flags, identifiers from the player's files, values no source gives | `server/content/masterdata/`, `statues.json`, `quests.json` |

From the CC BY-SA source only facts (numbers) were extracted; no text, layout or images. Each such
record names its page and revision, the licence and the date it was read.


Thank you to the players and wiki editors who documented the game while it was running.

## Tools and references

- [vgmstream](https://github.com/vgmstream/vgmstream): the public HCA key list (the sound key is
  supplied by the player at patch time, not stored here).
- Unity 2018 "16GB address" allocator crash on Android 11+: the
  [Unity forum thread](https://discussions.unity.com/t/android-11-crash-when-using-system-allocator/832246)
  and the write-up in
  [project-liminal-gate](https://github.com/anzensan/project-liminal-gate/blob/main/docs/advanced-configuration.md),
  which pointed to building a `DynamicHeapAllocator` instead (our patch and its proof are our own).
