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
| _(no outside sources yet)_ | `tutorial_flag_table` uses only our own research (`jdr-research`, docs/research/09) and our own defaults (`jdr-default`) | `server/content/masterdata/` |

Thank you to the players and wiki editors who documented the game while it was running.

## Tools and references

- [vgmstream](https://github.com/vgmstream/vgmstream): the public HCA key list (the sound key is
  supplied by the player at patch time, not stored here).
- Unity 2018 "16GB address" allocator crash on Android 11+: the
  [Unity forum thread](https://discussions.unity.com/t/android-11-crash-when-using-system-allocator/832246)
  and the write-up in
  [project-liminal-gate](https://github.com/anzensan/project-liminal-gate/blob/main/docs/advanced-configuration.md),
  which pointed to building a `DynamicHeapAllocator` instead (our patch and its proof are our own).
