#!/usr/bin/env python3
"""Export the menu/UI images from the player's own game files, for redrawing.

Writes, per bundle, every texture as PNG plus every sprite cut out of its atlas, and a manifest
that records where each image came from (bundle, object, format, size, sprite rectangles), so a
later import step can put redrawn images back in the same place.

Nothing is downloaded and nothing leaves the player's machine. Run it on your own copy:

    python3 -I tools/ui-assets/export_ui.py --data <files dir> --keys local/keys.json \
        --tool target/release/jdr-tool [--apk <game.apk>] --out ui-export

Bundles exported: ui_*, scene_* (menu canvases), mind_frame_*, inapp, plus the in-APK
`assets/bin/Data` files (title, loading, touch effects) when --apk is given.
requires: pip install UnityPy (with texture2ddecoder for ETC/ASTC)
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import zipfile

import UnityPy

UI_PATTERNS = [r"^ui_", r"^scene_", r"^mind_frame", r"^inapp$"]

# Unity TextureFormat values seen in this game (the importer re-encodes to the same format).
FORMATS = {1: "Alpha8", 3: "RGB24", 4: "RGBA32", 34: "ETC_RGB4", 47: "ETC2_RGBA8"}


def names(tool, data, keys):
    out = subprocess.run([tool, "names", "--data", data, "--keys", keys], check=True,
                         capture_output=True, text=True).stdout
    for line in out.splitlines():
        n, f, _ = line.split("\t")
        yield n, f


def load_bundle(tool, keys, path, tmp):
    raw = open(path, "rb").read(16)
    if raw.startswith(b"JJEC"):
        dec = os.path.join(tmp, "dec.unityfs")
        subprocess.run([tool, "decrypt", "--keys", keys, "--in", path, "--out", dec], check=True,
                       capture_output=True)
        return UnityPy.load(open(dec, "rb").read())
    return UnityPy.load(open(path, "rb").read())


def safe(s):
    return re.sub(r"[^A-Za-z0-9_.\-\[\]() ]", "_", s)[:120] or "_"


def export_env(env, label, src, outdir, manifest):
    texdir = os.path.join(outdir, safe(label))
    textures = {}
    for o in env.objects:
        if o.type.name != "Texture2D":
            continue
        t = o.read()
        if t.m_Width == 0 or t.m_Height == 0 or t.m_Name == "Font Texture":
            continue
        os.makedirs(texdir, exist_ok=True)
        fn = f"{safe(t.m_Name)}.png"
        if os.path.exists(os.path.join(texdir, fn)):
            fn = f"{safe(t.m_Name)}__{o.path_id}.png"
        try:
            img = t.image
        except Exception as e:  # unsupported format: record it, keep going
            manifest["skipped"].append({"bundle": label, "texture": t.m_Name, "reason": str(e)})
            continue
        img.save(os.path.join(texdir, fn))
        code = int(getattr(t.m_TextureFormat, "value", t.m_TextureFormat))
        fmt = FORMATS.get(code, f"format {code}")
        textures[o.path_id] = img
        manifest["textures"].append({
            "bundle": label, "source": src, "path_id": o.path_id, "name": t.m_Name,
            "file": f"{safe(label)}/{fn}", "width": t.m_Width, "height": t.m_Height,
            "format": fmt, "mips": getattr(t, "m_MipCount", 1),
            "png_sha256": hashlib.sha256(img.tobytes()).hexdigest(),
        })
    for o in env.objects:
        if o.type.name != "Sprite":
            continue
        s = o.read()
        rd = s.m_RD
        tex_id = rd.texture.path_id
        if tex_id not in textures:
            continue
        r = rd.textureRect
        img = textures[tex_id]
        h = img.height
        box = (int(round(r.x)), int(round(h - r.y - r.height)), int(round(r.x + r.width)), int(round(h - r.y)))
        if box[2] <= box[0] or box[3] <= box[1]:
            continue
        sdir = os.path.join(texdir, "sprites")
        os.makedirs(sdir, exist_ok=True)
        fn = f"{safe(s.m_Name)}.png"
        if os.path.exists(os.path.join(sdir, fn)):
            fn = f"{safe(s.m_Name)}__{o.path_id}.png"
        settings = getattr(rd, "settingsRaw", 0)
        rotated = bool((settings >> 2) & 0xF) if isinstance(settings, int) else False
        img.crop(box).save(os.path.join(sdir, fn))
        manifest["sprites"].append({
            "bundle": label, "path_id": o.path_id, "name": s.m_Name,
            "file": f"{safe(label)}/sprites/{fn}", "texture_path_id": tex_id,
            "rect": [r.x, r.y, r.width, r.height], "rotated_in_atlas": rotated,
        })


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--keys", required=True)
    ap.add_argument("--tool", required=True, help="path to jdr-tool")
    ap.add_argument("--apk")
    ap.add_argument("--out", required=True)
    ap.add_argument("--only", help="regex on bundle names (default: UI bundles)")
    a = ap.parse_args()
    pats = [re.compile(a.only)] if a.only else [re.compile(p) for p in UI_PATTERNS]
    os.makedirs(a.out, exist_ok=True)
    manifest = {"about": "UI images exported from the player's own copy; see README.txt",
                "textures": [], "sprites": [], "skipped": []}
    picked = sorted((n, f) for n, f in names(a.tool, a.data, a.keys) if any(p.search(n) for p in pats))
    with tempfile.TemporaryDirectory() as tmp:
        for i, (n, f) in enumerate(picked, 1):
            try:
                env = load_bundle(a.tool, a.keys, os.path.join(a.data, f), tmp)
                export_env(env, n, f"files/{f}", a.out, manifest)
            except Exception as e:
                manifest["skipped"].append({"bundle": n, "reason": str(e)})
            print(f"[{i}/{len(picked)}] {n}", file=sys.stderr)
        if a.apk:
            with zipfile.ZipFile(a.apk) as z:
                for zn in sorted(z.namelist()):
                    if not zn.startswith("assets/bin/Data/") or "/" in zn[len("assets/bin/Data/"):]:
                        continue
                    if not re.fullmatch(r"[0-9a-f]{32}", zn.rsplit("/", 1)[1]):
                        continue
                    try:
                        env = UnityPy.load(z.read(zn))
                        export_env(env, "apk_" + zn.rsplit("/", 1)[1], zn, a.out, manifest)
                    except Exception as e:
                        manifest["skipped"].append({"bundle": zn, "reason": str(e)})
    json.dump(manifest, open(os.path.join(a.out, "manifest.json"), "w"), indent=1, ensure_ascii=False)
    open(os.path.join(a.out, "README.txt"), "w").write(README)
    print(f"exported {len(manifest['textures'])} textures, {len(manifest['sprites'])} sprites "
          f"from {len(picked)} bundles; {len(manifest['skipped'])} skipped", file=sys.stderr)


README = """UI images exported from your own copy of the game.

Layout
  <bundle>/<texture>.png          whole textures; most are sprite atlases (many images packed in one)
  <bundle>/sprites/<sprite>.png   each sprite cut out of its atlas, for reference and redrawing
  manifest.json                   where everything came from (bundle, object id, format, size, rects)

Redrawing
  - Keep file names and folders unchanged, and keep each image's pixel size.
  - Edit either the atlas PNG, or the single sprite PNGs (the importer pastes sprites back into
    their atlas at the recorded rectangle). Do not edit both for the same atlas.
  - Keep transparency (PNG with alpha).
  - Only return the files you changed, in the same folders.
  - Sprites marked "rotated_in_atlas": true in manifest.json are stored rotated in the atlas.
  - Textures in format ETC_RGB4 have no alpha channel in the game (it is either opaque, or the
    transparency lives in a separate "_Alpha" texture next to it); keep them opaque.

These files come from the game; do not share them publicly.
"""

if __name__ == "__main__":
    main()
