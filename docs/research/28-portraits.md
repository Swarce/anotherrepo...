# 28 — Generated portraits (Build 27)

Read in the 3.12.0 client and the player's own files: H = read in the disassembly or measured in
the files, M = inferred. Code: `tools/content/render_portraits.py` (+ `render_portraits.js`,
`posing.py`), `server/services/src/art.rs` (`add_thumbnails`, `make_thumbnail`). No image is in
the repository: the tool reads the player's files and writes a private art pack.

## 1. The thumbnails in the client

- **Name (H).** `ResourceConst.GetStatueThumnailAssetName` 0xE15CE8: a ResourceConst prefix,
  the statueId and `.bytes`. About 60 callers (lists, banners, team panels, battle UI) load it
  through `CommonUIManager.LoadResource` 0xCE9448.
- **Lookup (H, docs/research/09).** `ResourceManager` registers every `m_Container` key of a
  loaded bundle and finds assets by name. A thumbnail is the TextAsset
  `.../statuethumnail_convert/android/ui_statue_0N/st_<id>.bytes`.
- **Format (H, files).** Each TextAsset holds a JJEC-encrypted bundle (asset key) with one
  serialized file and its `.resS`. Inside: Texture2D `st_<id>` (128x128, ETC2_RGBA8), Sprite
  `st_<id>` and an AssetBundle whose container lists `.../st_<id>.png` twice (Sprite and Texture).
- **Sprites (H, files).** 420 of the 643 sprites are tight meshes (8 vertices) cut to their own
  outline, smallest rectangle 120.8 x 119.9. 67 are full 128x128 quads (4 vertices).
- **Counts (H, files).** 643 thumbnails (478 in `ui_statue_00`, 165 in `ui_statue_01`). 55 of
  the 292 roster statues have none: 53 regular and 2 assist-only.
- **Drawn in (H, files).** The frame, the rarity stars, the attribute badge and the assist-only
  "A" mark are part of each image, not overlays:
  - frame by rarity: 3 silver-blue, 4 gold, 5 jewelled vines, 6 pink lightning;
  - badge and background colour by attribute: Brave blue star, Intelli green gem, Tenacity red
    heart, Unity gold knot, Maverick purple;
  - six star slots: gold up to the rarity, then dark empty slots up to the top rarity the statue
    can reach. Statues already at their top show none.

## 2. Measured from the thumbnails (H, files)

- **Rarity from the frame.** Matching each thumbnail's border to the mean border of each wiki
  rarity sorts all 237 roster thumbnails into four clean groups. For nine statues the frame
  disagrees with our wiki rarity of 5: 4011_15/16/18, 4130_25/26/28 and 4170_15/16/18 have the
  3-, 4- and 6-star frames.
- **Empty slots.** Of the regular thumbnails, these have none: 12 of 34 (3-star), 14 of 42
  (4-star), 20 of 78 (5-star), 63 of 63 (6-star).
- **Background.** A diamond lattice, cells 18.8 x 20.5 px, offset (6.75, 14.75). Three thumbnails
  of two attributes agree to 0.2 px (`--fit-lattice`). The lines are about 30% darker than the
  colour field, with a faint light edge.
- **Stars.** Centres at x = 17.1 + 12.65 k (k = 0..5), y = 108.7, on a black backing.
- **Framing.** Renders of 32 statues that have a thumbnail were matched to it (crop scale and
  offset). The 18 clean matches give a crop about 0.30 m wide, centred 2.4 cm above the HEAD bone
  (median; interquartile scale 2.71-3.29 of a 512 px render at 1.2 m, 20 degrees).

## 3. What the tool does (ours)

- **Pose.** The statue's first model in its figure (menu) clip (`posing.clip_name`: figure_R/SR/
  SSR/UR by rarity, with the variant suffix), first frame. The fallbacks are the rarity's base
  clip, then any figure clip. If the pose hides the character (3330_02, Vanilla Ice inside
  Cream), the second model's head is framed.
- **Render.** three.js in headless Chromium: unlit textures (the toon shading is in the textures),
  an inverted-hull outline of 1.5 mm, a 20-degree camera 0.95 m in front of the HEAD bone, aimed
  2.4 cm above it, 512 px, scaled to 128 px.
- **Layers.** Each layer is the per-pixel mode of the thumbnails that share it: the mean of the
  ones in the winning colour bin, which averages out their texture compression.
  - frame, bottom bar and gold stars: the regular thumbnails of that rarity with no empty slots.
    The server sends `rareMax` = `rare`, so the statue has no upgrade to show;
  - face opening: what those thumbnails do not share, connected to the middle;
  - badge corner: kept as frame only where an even mix of attributes agrees;
  - badge: the attribute's thumbnails, inside a circle fitted to its firm core;
  - "A" mark: the assist-only thumbnails;
  - background: per attribute, a colour field fitted to pixels three or more thumbnails agree
    on, with the lattice drawn over it.
- **Rarity and attribute.** As the server sends them: the wiki values, else rarity 4 and
  attribute 1 + psid mod 5.

## 4. Into the game (ours)

- The tool writes `portraits.zip` (`replacements/ui_statue_01/thumbs/st_<id>.png`), an art pack:
  `jdr-patch patch ... --art <ui art.zip> --art portraits.zip`.
- On the phone, the server rebuilds `ui_statue_01` with the other art (`art::apply`). For each
  image (`add_thumbnails`):
  - clone the bundle's thumbnail whose sprite covers the most (a full quad, so no other frame's
    outline clips the new one);
  - rename its objects, store the pixels as RGBA32, drop the `.resS`, rename the serialized
    file `CAB-<md5 of the name>`;
  - encrypt the result with the asset key and add it as a TextAsset with a preload entry and a
    container key (kept sorted).
- Thumbnails the bundle already has are left alone and reported.
- **Checks.** `tests/real_portraits.rs` runs on the player's `ui_statue_01` with synthetic images:
  original objects stay byte-identical, the container stays sorted, and each new entry decrypts to
  its image (top row first). `tools/proofs/portrait_check.py` reads the same result with UnityPy:
  every key resolves, and the new sprites draw 128x128.

## 5. Open (M)

- That the lists show the new entries is inferred from the lookup by container key; it needs a
  device.
- Statues with no wiki attribute show the server's placeholder attribute, in the portrait as in
  the game. The real thumbnails would give the right one for the 145 statues that have one (not
  done).
