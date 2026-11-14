# Design: rebuilding `in_memory_master_data` and `versionfile` (M1.5)

Status: **implemented** — `versionfile` in Build 04 (device-confirmed), `in_memory_master_data` in
Build 06 (device test pending). The original plan below (a Python/UnityPy tool on the player's PC)
was replaced by generation inside the server, on the phone; see "As built".

## As built (Build 06)

- **Where:** `Ctx::generated()` builds both bundles in the embedded server (and the standalone host),
  in the background at boot and on demand, cached until `vcs.bytes` changes. Master data first, then
  the manifest, which lists it (`in_memory_master_data.bytes`, md5 + size of the exact bytes `/cdn`
  serves, `isEncrypt=false`: plain UnityFS).
- **Schema:** `server/content/masterdata/schema.json` — every asset in the bundle with its class, and
  the serialized fields of 147 types, from our analysis of the client's type metadata
  (docs/research/09-master-data.md). Code identifiers only; no game text, no values.
- **Rows:** one JSON file per table in the same folder (`asset`, `sources`, `data`). Enums by name or
  number, `SecurityIntValue` as a plain number (written seed 0 / value), unknown fields refused.
  A core table without a file is written empty (every core loader accepts an empty list); battle-only
  tables are left out until they have content (their readers null-check; an all-zero settings object
  would be worse than none).
- **Type trees:** `jdr_unity::managed` writes the node shapes the engine itself uses (research 09 §4),
  with our own script hash so the runtime reads through the shipped tree. One MonoScript per class,
  `Assembly-CSharp.dll`, container keys = lowercase full asset paths, sorted; preload table = script +
  object per asset.
- **Content in Build 06:** `tutorial_flag_table` (34 labels the client queries, ids 0..33, no script
  packs) so tutorials and hints count as done and daily/event quests unlock with the all-ones
  `get_tutorial_step`. Everything else is empty.
- **Checks:** unit tests pin the tutorial-table type tree to the research template and the binary
  layout; `tools/proofs/masterdata_check.py` reads the bundle built with the player's own engine type
  entries through UnityPy (exact byte consumption) and compares every value with the content files.

The sections below are the original Build 01 plan, kept for the table/source overview.

## Problem

Two bundles the client needs at every boot were only ever held in memory and never written to the
phone, so no player copy contains them:

| Bundle | What it is | Client use |
|---|---|---|
| `versionfile` | the server's asset manifest: `{path, ObfuscationPath, md5Sum, size, isEncrypt, isObfuscation}` per file | `AssetVersionManager.LoadServerVersionData`; decides what is up to date |
| `in_memory_master_data` | 18 ScriptableObject tables (below) | `TitleLoad` loads all assets; the game cannot pass TitleLoad without it |

## Approach

A tool in `tools/content-gen` (Python + UnityPy, run on the player's machine) builds both bundles
from (a) the player's own files, (b) our own JSON in `server/content/master/`, and writes them to the
server's data folder. The server lists them in `versionfile` and serves them via `getfiledata` like
any other file. Nothing derived from game files is committed. The JSON in the repo holds values we
authored or sourced from public wikis: **committed, with a `sources` list per file and credit in
`CREDITS.md`** (owner decision; facts and numbers only, no copied prose).

### Bundle mechanics
- Start from a real downloaded bundle as the template (same Unity version and serialization).
- MonoBehaviour type trees: generated from the field definitions in the player's own
  `global-metadata.dat`/`libil2cpp.so` at build time (the dumper logic moves into the tool), not
  copied into the repo.
- `m_Script` points at a MonoScript object we add to the bundle (`Assembly-CSharp.dll`, namespace
  `Jjs`, class name). To verify: that the client resolves a MonoScript placed in a downloaded bundle
  (the real master-data bundle must have done the same).
- Asset paths: `Assets/Resources/Common/InMemoryMasterData/<table>.asset` (client loads them by
  these names; `tutorial_flag_table` is loaded individually).
- Served unencrypted (plain UnityFS is accepted for any bundle).

### `versionfile`
- One entry per file the client may request: from the player's `vcs.bytes` (obfuscated name + md5)
  and file sizes on disk, plus our generated bundles (`in_memory_master_data`, translated `text.jp`,
  `*_script_text_jp`).
- `CheckNewVersion` compares server md5 with the md5 recorded in the local `vcs.bytes` plus file
  existence and size, so unchanged files are never re-downloaded.

## The 18 tables and where values come from

| Table | Rows / shape | Source of values |
|---|---|---|
| statue_setting_table | per statue: view positions, camera type, gauge type, AI ids | statue ids from bundle names; positions: defaults tuned on device; AI ids from `aipack` contents |
| statue_level_table | per statue: level → stats | wiki stat tables (base/max) + interpolation |
| skill_data_table | skill id, type, button, cooldown, limit, rates Lv1/Lv10 (PvE/PvP), params | skill ids/names from the text table labels; numbers from wikis |
| ability_data_table | affect id, limits, params Lv1/Lv10 ×8 | wiki ability descriptions; text-table labels |
| assist_ability_condition_table | assist ability conditions | wikis; defaults |
| action_branch_data_table | branch id → damage-reaction action ids | action ids inside the player's `actionset_*` bundles (naming conventions) |
| mind_level_table, rank_table (player), team_rank_table | level/exp curves | wikis; smooth curves |
| tutorial_flag_table | flag ids, labels, launch types, scripts | tutorial script bundles (`main_tutorial_NN`, `sub_tutorial_NN`) and their Lua calls |
| yogo_table | glossary entries | ids from the text table (`YOGO_*` labels) |
| signboard_table | MyPage signboards | defaults (few rows) |
| statue_special_scene_data_table | special-move scenes per statue | `animation_statue_spskill_*` bundle names |
| map_gimmick_pattern_table, area_position_pattern_table | map gimmicks, spawn positions | `area_*`/`map_*` bundles; defaults |
| affect_text_rule_table | affect → text rules | text-table labels |
| Set/Node_el_camera_*_common | common camera sets | defaults; tune on device |

The exact field lists come from the client's type definitions; the tool validates every row against
them before writing.

## Validation
- Unit: round-trip each generated table through UnityPy and compare with the JSON source.
- Real-binary: load the generated bundle with UnityPy using the type trees from the player's client.
- Device: TitleLoad completes; MyPage shows the leader model; one battle runs (logcat clean of
  NullReference in table lookups).

## Open questions
1. Does the client accept a MonoScript embedded in a downloaded bundle? (Expected yes; verify early
   with a one-table bundle.)
2. Which tables are touched on the path to MyPage vs only in battle: build the minimal set first.
3. Wiki coverage for skill/ability numbers: list gaps per statue in docs/research/04.
