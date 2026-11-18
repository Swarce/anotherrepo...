# Localization (English)

The game never had an English release. We translate its text **in place**: the language setting
stays "jp", and the Japanese strings are replaced with English in the files the game already loads.
UI images with Japanese lettering can be replaced with redrawn art (see "Redrawn UI art" below).

## Where the text lives

| Source | Size (3.12.0) | How we replace it |
|---|---|---|
| `text.jp` bundle: 111 `{Label, Text}` tables (`TextData`) | ~49,600 strings | the server rebuilds the bundle |
| UI prefabs (`ui_*`, `scene_*`, `minigame_*`): `m_Text`/`defaultText` of baked labels | 1,212 strings in 32 bundles | the server rebuilds the bundles |
| `Text/ClientMes_JP` in the APK (boot, download, error dialogs) | 236 strings | the patcher (`english-clientmes`) |
| `UI.Text` labels of the scenes built into the APK (`assets/bin/Data/level*`: loading overlay, first download, save select, contact, cache, error screens) | 72 strings in 11 scenes | the patcher (`english-apk-text`) |
| C# string literals in `global-metadata.dat` that the player sees (Follow buttons, arena rank and days left, end dates, "approx.", "Saving...", yen, purchase errors) | 22 of the 139 Japanese literals | the patcher (`english-apk-text`) |
| Story scripts (`*_script_text_jp`, 1,609 bundles: one MonoBehaviour `{m_Name, text: [line, …]}` each) | 14,259 lines (7,568 unique, ~200k characters) | the server rebuilds the bundles (keys `script:<bundle>:<path id>:<line>`, `en/story.json`; Build 23) |

## How the English reaches the game

1. On the phone the server rebuilds each target bundle from the **original** (kept under
   `<data>/l10n/orig/`), writes the English file to `<data>/l10n/gen/`, and lists it in the generated
   manifest with its md5 and size (plain UnityFS, `isEncrypt=false`).
2. The client sees a changed md5, downloads the file from `/cdn` once (its normal update path), and
   records it in `vcs.bytes`. Later boots reuse the cached English files (rebuilt only when the
   original, the translations or the generator change).
3. Removing a translation serves the original again, so the client reverts on its next boot (also
   when a bundle loses its last translation).
   The server remembers every English file it ever produced, so a copy of one of them on disk (an
   older version the client has not replaced yet, or a backup restored later) is never mistaken
   for an original, and a file that does not parse or holds no Japanese never replaces the kept
   original.
4. UI labels we replace get Unity "best fit" (shrink to fit their box, down to half size), since
   English usually runs longer than Japanese.
5. The importer leaves an English file alone while it is byte-identical to one the server produced,
   and the game's `vcs.bytes` alone while the staging copy is the one already imported; anything
   else (a broken file, a newer backup in staging) is restored as before.
6. While the client downloads, `/cdn` serves the English files from the last finished build; a
   rebuild happens only when the client asks for the manifest again.

## Translation files (`server/content/l10n/en/*.json`)

```json
{ "sources": [{"source": "jdr-default", "note": "translated by the project"}],
  "entries": { "<key>": ["<hash>", "<English>"] } }
```

- `<hash>` = first 8 hex digits of SHA-256 of the Japanese source. A translation applies only while
  the source still matches; otherwise it is reported as stale. **The Japanese text is never stored.**
- Keys: `text:<Table>:<Label>` (repeated labels: `#2`, `#3`, …), `ui:<bundle>:<file>:<path id>:<field>`,
  `clientmes:<Label>`, `apk:<scene file>:<path id>:m_Text` (e.g. `apk:level2:158:m_Text`) and
  `literal:<index>` (both in `apk_text.json`).
- `{name}` placeholders must be kept exactly; the importer requires the Japanese column and refuses
  a placeholder mismatch, a wrong or malformed hash, or Japanese characters in the English column.
- Only strings with Japanese script are offered and replaced. Rows without it stay as the game has
  them, e.g. the stat labels `AFFECT_ATK_RATEADD_TEXT` = `ATK+{decimal_Param0}％` (the full-width
  `％` is not Japanese script); the ability effect lines use them (research doc 16 §8).
- An empty `english` cell means "not translated". A cell of only spaces is imported as is: a
  deliberate blank, for a Japanese suffix English does not need (`literal:6813`, the rank suffix
  after the no-rank text).

## Translating

```bash
jdr-tool l10n export --data <files dir> --keys local/keys.json --apk <game.apk> --out local/l10n/work.tsv
```

```bash
jdr-tool l10n import --in local/l10n/work.tsv --repo .
```

With `--apk`, the export also lists the ClientMes rows, the APK's scene labels and the allow-listed
C# literals (the literals only from an unpatched APK of a verified build).

The TSV (`key, hash, english, japanese`) contains game text: keep it under `local/` (git-ignored).
Fill the `english` column for the rows you translate, then import. Export again at any time: rows
already translated come back filled in; stale ones (the source changed) come back empty with the
old English in a `previous_english` column for reference. Export from an original copy of the game
files (your backup), not from a phone folder that already holds English files.

The in-game terms-of-use text is replaced by a short notice that this is an unofficial offline
build (the original terms no longer apply); it is not a translation of the publisher's terms.

## The Build 23 retranslation

Every string was retranslated in one pass, to read like an official localization.

- **Style and terms.** A private style guide and glossary (1,193 terms, with sources) drawn from
  Bandai Namco's own English JoJo games (All-Star Battle R, Eyes of Heaven) for UI wording, system
  terms and battle cries, and from their English mobile games for the gacha UI register. By the
  owner's decision, Stands and characters keep their **original names** (Crazy Diamond, Sticky
  Fingers, King Crimson, Vanilla Ice...), not the Western renames, and ドドド/ゴゴゴ is
  "Menacing".
- **Batches.** `tools/l10n/make_batches.py` splits an export into batches of unique source strings,
  grouped by table, screen or story scene (a scene is never split). Each batch was translated from
  the Japanese and checked with `tools/l10n/check_batch.py` (the importer's rules) before import.
- **Consistency.** `tools/l10n/term_check.py` reports a term rendered more than one way across
  batches and fixes the listed variants (`--fix`).
- **Import.** `jdr-tool l10n import --in <all batches> --expand-with <export> --replace` gives every
  row with the same source the same English and overwrites the old translations.
- **Fit.** `tools/l10n/ui_fit.py` was run on the result; the 55 labels that grew past their room
  were shortened.

## Text built into the APK (`english-apk-text`, Build 17)

- **Scenes.** The level files carry no type trees. `patcher/src/apktext.rs` decodes a Unity 2018.4
  `UI.Text` field by field (its `m_OnCullStateChanged` type name must be `MaskableGraphic`'s, and
  the object must be consumed exactly); `m_Text` is the last field, so a new text changes nothing
  before it. Each of the 11 scene files is fingerprinted (SHA-256, `known.rs`), must round-trip
  unchanged before it is edited, and after the edit is decoded again: the edited labels hold the
  English with the bytes before `m_Text` unchanged, every other object and table is byte-identical.
  The real-data test also decodes every label through the engine's own `UI.Text` type tree from a
  downloaded `scene_*` bundle. Best fit is not switched on for these labels (the files otherwise
  stay byte-identical), so the English is kept about as long as the Japanese.
- **Literals.** `known.rs` lists the player-visible literals by index (`ui_literals`, with what
  each one is). A literal is replaced only while its text still hashes as the translation expects.
  English that fits in the original bytes is written in place (`meta::replace_literal`); longer
  English (4 literals) is appended at the end of `global-metadata.dat` and its entry repointed
  (`meta::set_literal`): the runtime maps the whole file and reads `data offset + index` with no
  bounds check, and nothing else in the file moves. Date formats are .NET `DateTime` formats:
  English words in them are quoted (`'Until' yyyy/MM/dd HH:mm`).
- **Post-check.** The output APK is re-read: as many scene labels and literals must hold our
  English as the patch replaced on the input.
- **Left out.** About 115 debug-only literals (debug menus, logs, asserts, movie tests); the two
  Japanese `System.DateTime` patterns (parse-only, never shown); the product name in
  `globalgamemanagers` (not shown in game).

## Fitting the UI (Build 19)

English runs longer than Japanese, and the UI was laid out for Japanese.

- **Check.** Run this from your own copy of the bundles (decrypted with `jdr-tool decrypt`):

  ```bash
  python3 -I tools/l10n/ui_fit.py --dec <decrypted bundles> --out local/l10n/fit.tsv --hints server/content/l10n/ui_layout.json
  ```

  It lays out every `UI.Text` label as Unity does: the box comes from the prefab's RectTransforms,
  the label's own font is measured, and wrap, line height and best fit are applied. It writes a
  private report (keys, numbers, our English) with a `problem` column. The original Japanese fits
  in 98.2% of the labels it can measure (`--calibrate` prints this), which checks the
  measurement. Labels that size themselves (an active ContentSizeFitter or a layout group) are
  compared with the room the Japanese and the button art leave.
- **Layout hints** (`server/content/l10n/ui_layout.json`, label ids `bundle:file:pathId` only).
  The server applies them while it rebuilds a bundle:
  - `bestFit`: labels that wrap and truncate get best fit, down to half size. Many are filled at
    runtime from `text.jp`, so their baked text was never replaced.
  - `wrap`: labels that spilled sideways with our English get wrap and truncate first, so best
    fit has a box to fit.

  Labels with the bitmap digit fonts (`Font_*Num*`) are never listed: they hold only digits and a
  few symbols, so a letter put in one would not show at all.
- **Writing for the UI.**
  - Keep button and title text within the room the Japanese had. The report's `grows` and
    `no wrap` problems name the label and its room in pixels.
  - Prefer the short forms used across the game: Easy/Normal/Hard, (S)/(M)/(L)/(XL),
    "X Shard", "X Slab", "Tarot: …", "• Fire attack on one foe".
  - Character, Stand, skill and Mind names stay whole: best fit shrinks them instead.

## Redrawn UI art (Build 09)

1. `tools/ui-assets/export_ui.py` exports every menu texture and each sprite cut out of its atlas
   from the player's own copy (`<bundle>/sprites/<sprite>.png`, exact rectangle size).
2. The player redraws the sprites they want and zips them as `replacements/<bundle>/sprites/…`.
3. `jdr-patch patch … --art <pack.zip> --data <files>`:
   - images for the APK's own screens (`apk_<file>`: title, boot, and the loading-screen sprite
     atlas `apk_00000000000000000000000000000000/SpriteAtlasTexture-LoadAtlas-2048x2048-fmt47.png`,
     a whole texture in a file that holds only that Texture2D) are pasted into those textures in
     the APK. Those files carry no type trees: each Texture2D/Sprite layout is taken from a
     downloaded bundle with the same type hash, and every object of those types must decode
     through it exactly and encode back to the same bytes before anything is edited. A file
     without such a layout (or whose pixels sit in a `.resS` stream missing from the APK) is left
     unchanged and reported;
   - images for downloaded bundles are stored in the APK (`assets/jdr/art.bin`).
4. On the phone the server pastes them into the decoded atlases while it rebuilds the English
   bundles (same originals, cache and one-time download), so menus and text change together.
   Changed atlases are stored as uncompressed RGBA32 (no lossy re-compression): more memory per
   atlas (a 2048×2048 atlas takes 16 MB instead of 4 MB).
5. The art pack is the player's own and never goes into the repo.

Terms in the text follow the art: Summon, Enhance, Courage/Wisdom/Spirit/Unity/Solitude, Town.

## Status (Build 08)

| Group | Strings (rows) | English |
|---|---|---|
| ClientMes | 236 | all |
| UI prefab labels | 1,212 | all (the 2 `minigame_01` labels in Build 17) |
| APK scene labels and C# UI literals | 72 + 22 | all (Build 17) |
| Menu text tables | 1,087 | all (Build 07) |
| Names: statues/characters, event characters, places | 2,072 | all |
| Skill, ability, buff, tarot names | 2,331 | all |
| Mind and item names, rewards, event shop | 12,440 | all |
| Chapter, stage, quest, tutorial, glossary/achievement/tip titles | 13,062 | all |
| Rows in other tables whose text is one of those names | 1,134 | all |
| Descriptions: skill effects, missions, profiles, gifts, campaigns, login bonus, comments, … | 17,509 | next |
| Story scripts | ~298k characters | later (tools for the player's own translator) |

Character, Stand and place names follow one name list kept with the local working files (official
English: anime/VIZ; Stands by their original names), so every batch spells them the same way.
`jdr-tool l10n import --expand-with <export.tsv>` gives every row with the same Japanese source the
same English, so each unique string is translated once.
