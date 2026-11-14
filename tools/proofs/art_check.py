#!/usr/bin/env python3
"""Check rebuilt bundles that carry redrawn art, with UnityPy (an independent Unity reader).

For every sprite that has an image in the art pack, the pixels at the sprite's rectangle in the
rebuilt (RGBA32) texture must equal the PNG exactly; every other pixel must equal the original
texture as UnityPy decodes it (so our ETC decoder and the paste touch nothing else).

usage: python3 tools/proofs/art_check.py <state dir (with l10n/gen and l10n/orig)> <replacements dir>
       python3 tools/proofs/art_check.py --apk <patched.apk> <original.apk> <replacements dir>
requires: pip install UnityPy (with texture2ddecoder), Pillow
"""
import glob
import os
import re
import sys

import UnityPy
from PIL import Image, ImageChops

def safe(s):
    return re.sub(r"[^A-Za-z0-9_.\-\[\]() ]", "_", s)[:120] or "_"


if sys.argv[1] == "--apk":
    # In-APK screens: every apk_<file> image must equal its sprite in the patched APK, and the
    # textures must be RGBA32.
    import zipfile
    patched, original, reps = zipfile.ZipFile(sys.argv[2]), zipfile.ZipFile(sys.argv[3]), sys.argv[4]
    n = 0
    for b in sorted(d for d in os.listdir(reps) if d.startswith("apk_")):
        env = UnityPy.load(patched.read("assets/bin/Data/" + b[4:]))
        sprites, seen = {}, set()
        for o in env.objects:
            if o.type.name == "Sprite":
                sp = o.read()
                base = safe(sp.m_Name)
                sprites[f"{base}__{o.path_id}.png"] = sp
                if base not in seen:
                    sprites[f"{base}.png"] = sp
                seen.add(base)
        for f in sorted(os.listdir(os.path.join(reps, b, "sprites"))):
            s = sprites[f]
            t = s.m_RD.texture.read()
            assert int(t.m_TextureFormat) == 4, (b, f, t.m_TextureFormat)
            img = t.image.convert("RGBA")
            r, h = s.m_RD.textureRect, img.height
            box = (int(round(r.x)), int(round(h - r.y - r.height)), int(round(r.x + r.width)), int(round(h - r.y)))
            png = Image.open(os.path.join(reps, b, "sprites", f)).convert("RGBA")
            assert ImageChops.difference(img.crop(box), png).getbbox() is None, (b, f)
            n += 1
    assert n > 0, "no in-APK images in the pack"
    print(f"OK: {n} redrawn sprites in the APK's own screens match their PNGs exactly (RGBA32)")
    sys.exit(0)

state, reps = sys.argv[1], sys.argv[2]


def safe(s):
    return re.sub(r"[^A-Za-z0-9_.\-\[\]() ]", "_", s)[:120] or "_"


def bundle_name(env):
    for o in env.objects:
        if o.type.name == "AssetBundle":
            return o.read().m_Name
    return None


checked = textures = compared = 0
for gen in sorted(glob.glob(os.path.join(state, "l10n", "gen", "*.bytes"))):
    env = UnityPy.load(gen)
    name = bundle_name(env)
    sdir = os.path.join(reps, name or "", "sprites")
    if not name or not os.path.isdir(sdir):
        continue
    orig_path = os.path.join(state, "l10n", "orig", os.path.basename(gen))
    orig_env = UnityPy.load(orig_path) if open(orig_path, "rb").read(4) != b"JJEC" else None
    seen, by_tex = {}, {}
    # The exporter's rules: only sprites cut from a written texture (non-empty, not the font
    # texture) with a non-empty box were exported and take part in the naming. The textures are
    # judged on the ORIGINAL bundle (sizes and names do not change).
    src_env = orig_env or env
    exported = {}
    for o in src_env.objects:
        if o.type.name == "Texture2D":
            t0 = o.read()
            if t0.m_Width and t0.m_Height and t0.m_Name != "Font Texture":
                exported[o.path_id] = t0.m_Height
    for o in env.objects:
        if o.type.name != "Sprite":
            continue
        s = o.read()
        th = exported.get(s.m_RD.texture.path_id)
        if th is None:
            continue
        r0 = s.m_RD.textureRect
        if round(r0.x + r0.width) <= round(r0.x) or round(th - r0.y) <= round(th - r0.y - r0.height):
            continue
        base = safe(s.m_Name)
        first = base not in seen
        seen[base] = 1
        cands = ([f"{base}.png"] if first else []) + [f"{base}__{o.path_id}.png"]
        f = next((c for c in cands if os.path.exists(os.path.join(sdir, c))), None)
        if f:
            by_tex.setdefault(s.m_RD.texture.path_id, []).append((s.m_RD.textureRect, f))
    for tex_id, sprites in by_tex.items():
        t = env.assets[0].files[tex_id].read() if False else next(
            o for o in env.objects if o.path_id == tex_id).read()
        assert int(t.m_TextureFormat) == 4, (name, t.m_Name, t.m_TextureFormat)
        img = t.image.convert("RGBA")
        h = img.height
        mask = Image.new("L", img.size, 0)
        for r, f in sprites:
            box = (int(round(r.x)), int(round(h - r.y - r.height)), int(round(r.x + r.width)), int(round(h - r.y)))
            png = Image.open(os.path.join(sdir, f)).convert("RGBA")
            got = img.crop(box)
            assert got.size == png.size, (name, f, got.size, png.size)
            assert ImageChops.difference(got, png).getbbox() is None, (name, f, "pixels differ")
            mask.paste(255, box)
            checked += 1
        if orig_env is not None:
            ot = next(o for o in orig_env.objects if o.path_id == tex_id).read().image.convert("RGBA")
            diff = ImageChops.difference(ot, img)
            diff.paste((0, 0, 0, 0), mask=mask)
            assert diff.getbbox() is None, (name, t.m_Name, "pixels outside the sprites changed", diff.getbbox())
            compared += 1
        textures += 1
assert checked > 0, "no replaced sprites found"
print(f"OK: {checked} redrawn sprites match their PNGs exactly in {textures} RGBA32 textures; "
      f"in {compared} of them (originals not encrypted) everything else equals the original")
