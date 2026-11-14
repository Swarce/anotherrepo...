# Design: rebuilding `in_memory_master_data` and `versionfile` (M1.5)

Status: **draft for review** (written in Build 01, implementation starts in Build 02).

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
any other file. Nothing derived from game files is committed; the JSON in the repo holds only values
we authored or sourced from public wikis (with citations).

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
