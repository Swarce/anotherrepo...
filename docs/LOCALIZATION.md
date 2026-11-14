# Localization (English)

The game never had an English release. We translate its text **in place**: the language setting
stays "jp", and the Japanese strings are replaced with English in the files the game already loads.
UI images keep their Japanese lettering for now (`tools/ui-assets/export_ui.py` exports them for
redrawing; re-import comes in a later build).

## Where the text lives

| Source | Size (3.12.0) | How we replace it |
|---|---|---|
| `text.jp` bundle: 111 `{Label, Text}` tables (`TextData`) | ~49,600 strings | the server rebuilds the bundle |
| UI prefabs (`ui_*`, `scene_*`): `m_Text`/`defaultText` of baked labels | 1,210 strings in 31 bundles | the server rebuilds the bundles |
| `Text/ClientMes_JP` in the APK (boot, download, error dialogs) | 236 strings | the patcher (`english-clientmes`) |
| Story scripts (`*_script_text_jp`, 1,624 bundles) | ~298k characters | later build: export/translate tools for the player's own translator |

## How the English reaches the game

1. On the phone the server rebuilds each target bundle from the **original** (kept under
   `<data>/l10n/orig/`), writes the English file to `<data>/l10n/gen/`, and lists it in the generated
   manifest with its md5 and size (plain UnityFS, `isEncrypt=false`).
2. The client sees a changed md5, downloads the file from `/cdn` once (its normal update path), and
   records it in `vcs.bytes`. Later boots reuse the cached English files (rebuilt only when the
   original, the translations or the generator change).
3. Removing a translation serves the original again, so the client reverts on its next boot.
4. UI labels we replace get Unity "best fit" (shrink to fit their box, down to half size), since
   English usually runs longer than Japanese.
5. The importer never overwrites the game's `vcs.bytes` or our English files with a staging copy.

## Translation files (`server/content/l10n/en/*.json`)

```json
{ "sources": [{"source": "jdr-default", "note": "translated by the project"}],
  "entries": { "<key>": ["<hash>", "<English>"] } }
```

- `<hash>` = first 8 hex digits of SHA-256 of the Japanese source. A translation applies only while
  the source still matches; otherwise it is reported as stale. **The Japanese text is never stored.**
- Keys: `text:<Table>:<Label>` (repeated labels: `#2`, `#3`, …), `ui:<bundle>:<file>:<path id>:<field>`,
  `clientmes:<Label>`.
- `{name}` placeholders must be kept exactly; the importer refuses a mismatch, a wrong hash, or
  Japanese characters in the English column.

## Translating

```bash
jdr-tool l10n export --data <files dir> --keys local/keys.json --apk <game.apk> --out local/l10n/work.tsv
```

```bash
jdr-tool l10n import --in local/l10n/work.tsv --repo .
```

The TSV (`key, hash, english, japanese`) contains game text: keep it under `local/` (git-ignored).
Fill the `english` column for the rows you translate, then import. Export again at any time: rows
already translated come back filled in, stale ones empty.

## Status (Build 07)

| Group | Strings | English |
|---|---|---|
| ClientMes | 236 | all |
| UI prefab labels | 1,210 | all |
| Menu text tables (My Page, options, title, team, training, filters, shop, gacha, arena, …) | ~1,100 | all |
| Other text tables (names, skills, items, missions, events, …) | ~48,500 | not yet |
