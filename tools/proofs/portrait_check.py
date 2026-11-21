#!/usr/bin/env python3
"""Independent check (UnityPy) of a thumbnail bundle with added statue thumbnails, as written by
`cargo test -p jdr-services --test real_portraits` (JDR_PORTRAIT_OUT): every container key points
at a TextAsset of that name, and each added one decrypts (jdr-tool decrypt) to a bundle whose
Sprite draws the whole 128x128 test image, its top-left marker red and opaque.

usage: python3 -I tools/proofs/portrait_check.py <bundle> <keys.json> <jdr-tool>
"""
import os, subprocess, sys, tempfile
import UnityPy

bundle, keys, tool = sys.argv[1:4]
env = UnityPy.load(bundle)
ab = next(o for o in env.objects if o.type.name == "AssetBundle").read_typetree()
by_pid = {o.path_id: o for o in env.objects}
for k, v in ab["m_Container"]:
    o = by_pid[v["asset"]["m_PathID"]]
    assert o.type.name == "TextAsset" and k.endswith("/" + o.peek_name() + ".bytes"), k
added = [k for k, _ in ab["m_Container"] if "/st_9990_" in k]
assert len(added) == 2, added
with tempfile.TemporaryDirectory() as d:
    for k, v in ab["m_Container"]:
        if "/st_9990_" not in k:
            continue
        t = by_pid[v["asset"]["m_PathID"]].read()
        raw = t.m_Script.encode("utf-8", "surrogateescape") if isinstance(t.m_Script, str) else bytes(t.m_Script)
        enc, dec = os.path.join(d, t.m_Name), os.path.join(d, t.m_Name + ".dec")
        open(enc, "wb").write(raw)
        subprocess.run([tool, "decrypt", "--keys", keys, "--in", enc, "--out", dec], check=True, capture_output=True)
        inner = UnityPy.load(dec)
        sp = next(o for o in inner.objects if o.type.name == "Sprite").read()
        im = sp.image
        assert sp.m_Name == t.m_Name and im.size == (128, 128), (sp.m_Name, im.size)
        assert im.getpixel((0, 0))[:2] == (255, 0) and im.getpixel((0, 0))[3] == 255, im.getpixel((0, 0))
        print(f"{t.m_Name}: sprite 128x128, image as given")
print(f"OK: {len(ab['m_Container'])} container keys resolve; 2 added thumbnails read back through UnityPy")
