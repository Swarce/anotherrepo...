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
  `clientmes:<Label>`.
- `{name}` placeholders must be kept exactly; the importer requires the Japanese column and refuses
  a placeholder mismatch, a wrong or malformed hash, or Japanese characters in the English column.

## Translating

```bash
jdr-tool l10n export --data <files dir> --keys local/keys.json --apk <game.apk> --out local/l10n/work.tsv
```

```bash
jdr-tool l10n import --in local/l10n/work.tsv --repo .
```

The TSV (`key, hash, english, japanese`) contains game text: keep it under `local/` (git-ignored).
Fill the `english` column for the rows you translate, then import. Export again at any time: rows
already translated come back filled in; stale ones (the source changed) come back empty with the
old English in a `previous_english` column for reference. Export from an original copy of the game
files (your backup), not from a phone folder that already holds English files.

The in-game terms-of-use text is replaced by a short notice that this is an unofficial offline
build (the original terms no longer apply); it is not a translation of the publisher's terms.

## Status (Build 07)

| Group | Strings | English |
|---|---|---|
| ClientMes | 236 | all |
| UI prefab labels | 1,210 | all |
| Menu text tables (My Page, options, title, team, training, filters, shop, gacha, arena, special actions, …) | 1,087 | all |
| Other text tables (names, skills, items, missions, events, …) | ~48,500 | not yet |
