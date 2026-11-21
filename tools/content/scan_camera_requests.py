#!/usr/bin/env python3
"""Inventory the camera requests in the statues' tactical action sets (dt_actionset_statue_<psid>):
every DoChangeCamera / DoChangeCameraDelay action of their PlayMaker FSMs, with the FSM's name, the
template asset (e.g. 3000_1_stapla_016_SKILL_06: the action set, which names the skill), the state, the camera type (0 normal, 1 script), the script camera
id, the delay and the flags. The script camera ids are rows of Set_el_camera_set_common
(docs/research/31-battle-camera.md). Ids, names of action sets and states, numbers only.

usage: python3 -I tools/content/scan_camera_requests.py --data <files dir> --keys local/keys.json \
          --tool target/release/jdr-tool --out local/content/camera_requests.tsv
"""
import argparse
import os
import struct
import subprocess
import tempfile

import UnityPy

ap = argparse.ArgumentParser()
for x in ("--data", "--keys", "--tool", "--out"):
    ap.add_argument(x, required=True)
ap.add_argument("--prefix", default="dt_actionset_statue_")
a = ap.parse_args()

files = {}
for line in subprocess.run([a.tool, "names", "--data", a.data, "--keys", a.keys], check=True,
                           capture_output=True, text=True).stdout.splitlines():
    p = line.split("\t")
    files[p[0]] = p[1]


def decode(ad, k):
    t, pos, size = ad["paramDataType"][k], ad["paramDataPos"][k], ad["paramByteDataSize"][k]
    raw = bytes(x & 0xFF for x in ad["byteData"][pos:pos + size])
    if t in (0, 7) and size == 4:
        return struct.unpack("<i", raw)[0]
    if t == 2 and size == 4:
        return round(struct.unpack("<f", raw)[0], 3)
    if t == 1 and size == 1:
        return int(raw[0])
    return None


rows = []
with tempfile.TemporaryDirectory() as tmp:
    for name in sorted(n for n in files if n.startswith(a.prefix)):
        dec = os.path.join(tmp, name)
        r = subprocess.run([a.tool, "decrypt", "--keys", a.keys, "--in", os.path.join(a.data, files[name]), "--out", dec],
                           capture_output=True)
        env = UnityPy.load(dec if r.returncode == 0 else os.path.join(a.data, files[name]))
        for o in env.objects:
            if o.type.name != "MonoBehaviour":
                continue
            try:
                t = o.read_typetree()
            except Exception:
                continue
            f = t.get("fsm")
            if not f:
                continue
            for st in f.get("states", []):
                ad = st["actionData"]
                starts = ad["actionStartIndex"]
                for i, an in enumerate(ad["actionNames"]):
                    short = an.split(".")[-1]
                    if not short.startswith("DoChangeCamera"):
                        continue
                    a0 = starts[i]
                    a1 = starts[i + 1] if i + 1 < len(starts) else len(ad["paramName"])
                    p = {ad["paramName"][k]: decode(ad, k) for k in range(a0, a1)}
                    rows.append([name.replace(a.prefix, ""), f.get("name", ""), t.get("m_Name", ""), st.get("name", ""), short,
                                 p.get("cameraType"), p.get("scriptCameraId"), p.get("delaySec"),
                                 p.get("targetMainCharacter"), p.get("setBattleCameraDefaultPose")])
with open(a.out, "w", encoding="utf-8") as out:
    out.write("psid\tactionset\ttemplate\tstate\taction\tcameraType\tscriptCameraId\tdelaySec\ttargetMain\tdefaultPose\n")
    for r in rows:
        out.write("\t".join("" if x is None else str(x) for x in r) + "\n")
print(len(rows), "camera requests ->", a.out)
