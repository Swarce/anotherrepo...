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
    # In-APK screens: every apk_<file> image must equal its sprite (or, for a whole texture such
    # as the loading-screen atlas, the texture) in the patched APK, and the textures must be RGBA32.
    import zipfile
    patched, original, reps = zipfile.ZipFile(sys.argv[2]), zipfile.ZipFile(sys.argv[3]), sys.argv[4]
    n = skipped = whole = retyped = 0
    # The fields a rewritten texture changes (in-file RGBA32, one mip).
    TEXTURE_EDITED = {"m_TextureFormat", "m_CompleteImageSize", "m_MipCount", "m_MipMap", "image data", "m_StreamData"}
    for b in sorted(d for d in os.listdir(reps) if d.startswith("apk_")):
        entry = "assets/bin/Data/" + b[4:]
        if patched.read(entry) == original.read(entry):
            # The patcher leaves a file it cannot rewrite safely unchanged (and reports it).
            print(f"note: {entry} unchanged (left out by the patcher)")
            skipped += 1
            continue
        env = UnityPy.load(patched.read(entry))
        sprites, seen = {}, set()
        for o in env.objects:
            if o.type.name == "Sprite":
                sp = o.read()
                base = safe(sp.m_Name)
                sprites[f"{base}__{o.path_id}.png"] = sp
                if base not in seen:
                    sprites[f"{base}.png"] = sp
                seen.add(base)
        # Whole textures (`<bundle>/<texture>.png`): the texture equals the PNG.
        textures, tseen = {}, set()
        for o in env.objects:
            if o.type.name == "Texture2D":
                t = o.read()
                base = safe(t.m_Name)
                textures.setdefault(f"{base}.png" if base not in tseen else f"{base}__{o.path_id}.png", (o.path_id, t))
                textures.setdefault(f"{base}__{o.path_id}.png", (o.path_id, t))
                tseen.add(base)
        touched = set()
        for f in sorted(x for x in os.listdir(os.path.join(reps, b)) if x.endswith(".png")):
            pid, t = textures[f]
            touched.add(pid)
            whole += 1
            assert int(t.m_TextureFormat) == 4, (b, f, t.m_TextureFormat)
            png = Image.open(os.path.join(reps, b, f)).convert("RGBA")
            assert ImageChops.difference(t.image.convert("RGBA"), png).getbbox() is None, (b, f)
            n += 1
        sdir = os.path.join(reps, b, "sprites")
        for f in sorted(os.listdir(sdir) if os.path.isdir(sdir) else []):
            s = sprites[f]
            touched.add(s.m_RD.texture.path_id)
            t = s.m_RD.texture.read()
            assert int(t.m_TextureFormat) == 4, (b, f, t.m_TextureFormat)
            img = t.image.convert("RGBA")
            r, h = s.m_RD.textureRect, img.height
            box = (int(round(r.x)), int(round(h - r.y - r.height)), int(round(r.x + r.width)), int(round(h - r.y)))
            png = Image.open(os.path.join(reps, b, "sprites", f)).convert("RGBA")
            assert ImageChops.difference(img.crop(box), png).getbbox() is None, (b, f)
            n += 1
        # Nothing else changed: the same objects, every object but the touched textures
        # byte-identical, and the touched textures' other fields as before. The files carry no
        # type trees, so UnityPy reads them through its own class database for this Unity
        # version, and each read must consume its object exactly (an independent check of the
        # layout the patcher wrote with).
        env0 = UnityPy.load(original.read(entry))
        o0 = {o.path_id: o for o in env0.objects}
        o1 = {o.path_id: o for o in env.objects}
        assert sorted(o0) == sorted(o1), (entry, "object table changed")
        for pid, a in o0.items():
            c = o1[pid]
            assert a.type.name == c.type.name, (entry, pid)
            if pid not in touched:
                assert a.get_raw_data() == c.get_raw_data(), (entry, pid, "changed")
                continue
            assert a.type.name == "Texture2D", (entry, pid)
            d0, d1 = a.parse_as_dict(), c.parse_as_dict()
            assert list(d0) == list(d1), (entry, pid)
            for k in d0:
                if k not in TEXTURE_EDITED:
                    assert d0[k] == d1[k], (entry, pid, k)
            assert d1["m_StreamData"]["size"] == 0 and len(d1["image data"]) == d1["m_Width"] * d1["m_Height"] * 4
            retyped += 1
    assert n > 0 or skipped > 0, "no in-APK images in the pack"
    print(f"OK: {n} redrawn images in the APK's own screens ({whole} whole textures) match their PNGs exactly (RGBA32);"
          f" {retyped} textures rewritten, every other object byte-identical; {skipped} files left unchanged")
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
    bdir = os.path.join(reps, name or "")
    whole_pngs = [x for x in os.listdir(bdir) if x.endswith(".png")] if name and os.path.isdir(bdir) else []
    if not name or not (os.path.isdir(sdir) or whole_pngs):
        continue
    orig_path = os.path.join(state, "l10n", "orig", os.path.basename(gen))
    orig_env = UnityPy.load(orig_path) if open(orig_path, "rb").read(4) != b"JJEC" else None
    seen, by_tex = {}, {}
    # The exporter's rules: only sprites cut from a written texture (non-empty, not the font
    # texture) with a non-empty box were exported and take part in the naming. The textures are
    # judged on the ORIGINAL bundle (sizes and names do not change).
    src_env = orig_env or env
    exported, whole, tseen = {}, {}, set()
    for o in src_env.objects:
        if o.type.name == "Texture2D":
            t0 = o.read()
            if t0.m_Width and t0.m_Height and t0.m_Name != "Font Texture":
                exported[o.path_id] = t0.m_Height
                base = safe(t0.m_Name)
                cands = ([f"{base}.png"] if base not in tseen else []) + [f"{base}__{o.path_id}.png"]
                tseen.add(base)
                f = next((c for c in cands if c in whole_pngs), None)
                if f:
                    whole[o.path_id] = f
                    by_tex.setdefault(o.path_id, [])
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
        base_img = None
        if tex_id in whole:
            # A whole-texture image: everything outside the sprites pasted over it equals it.
            base_img = Image.open(os.path.join(bdir, whole[tex_id])).convert("RGBA")
            assert base_img.size == img.size, (name, whole[tex_id], base_img.size, img.size)
            checked += 1
        for r, f in sprites:
            box = (int(round(r.x)), int(round(h - r.y - r.height)), int(round(r.x + r.width)), int(round(h - r.y)))
            png = Image.open(os.path.join(sdir, f)).convert("RGBA")
            got = img.crop(box)
            assert got.size == png.size, (name, f, got.size, png.size)
            assert ImageChops.difference(got, png).getbbox() is None, (name, f, "pixels differ")
            mask.paste(255, box)
            checked += 1
        if base_img is not None:
            diff = ImageChops.difference(base_img, img)
            diff.paste((0, 0, 0, 0), mask=mask)
            assert diff.getbbox() is None, (name, whole[tex_id], "texture differs from its PNG")
        elif orig_env is not None:
            ot = next(o for o in orig_env.objects if o.path_id == tex_id).read().image.convert("RGBA")
            diff = ImageChops.difference(ot, img)
            diff.paste((0, 0, 0, 0), mask=mask)
            assert diff.getbbox() is None, (name, t.m_Name, "pixels outside the sprites changed", diff.getbbox())
            compared += 1
        textures += 1
assert checked > 0, "no replaced sprites found"
print(f"OK: {checked} redrawn sprites match their PNGs exactly in {textures} RGBA32 textures; "
      f"in {compared} of them (originals not encrypted) everything else equals the original")
