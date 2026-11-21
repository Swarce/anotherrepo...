#!/usr/bin/env python3
"""Inventory the action-battle SP cinematics in the player's own files: per model id (psid), the
`SKILL_<NN>` action-set entries of its action set (actionset_statue_<psid>), and the entries whose
FSM switches the battle into the SP cinematic (`DoChangeBattleSceneMode`, or its `CameraKeep`
variant, with `mode` 2). That action is what starts the cinematic: DoChangeBattleSceneMode.OnEnter
0xCFF29C -> BattleManager.ChangeMode 0xC072B8 -> StartSpecialSkill 0xC07694, which loads the
camera from the psid's animation_statue_spskill bundle (docs/research/30-action-battle.md §5). No
server table takes part. Entry numbers only: no names, clips or other asset data.

build_action_skills.py reads the result to keep cinematic entries out of the skill and SP slots
and to pick the finisher that plays the cinematic.

usage: python3 -I tools/content/scan_action_cinematics.py --data <files dir> --keys local/keys.json \
          --tool target/release/jdr-tool --out server/content/action_cinematics.json
"""
import argparse
import json
import os
import re
import struct
import subprocess
import tempfile

import UnityPy

ap = argparse.ArgumentParser()
for x in ("--data", "--keys", "--tool", "--out"):
    ap.add_argument(x, required=True)
a = ap.parse_args()

files = {}
for line in subprocess.run([a.tool, "names", "--data", a.data, "--keys", a.keys], check=True,
                           capture_output=True, text=True).stdout.splitlines():
    p = line.split("\t")
    files[p[0]] = p[1]

SCENE_MODE = ("DoChangeBattleSceneMode", "DoChangeBattleSceneModeCameraKeep")


def mode_param(ad, a0, a1):
    """The int `mode` parameter of one PlayMaker action (actionData params a0..a1), else None."""
    for k in range(a0, a1):
        if ad["paramName"][k] != "mode":
            continue
        pos, size = ad["paramDataPos"][k], ad["paramByteDataSize"][k]
        if ad["paramDataType"][k] in (0, 7) and size == 4:
            return struct.unpack("<i", bytes(x & 0xFF for x in ad["byteData"][pos:pos + size]))[0]
    return None


cinematic, skill_sets = {}, {}
with tempfile.TemporaryDirectory() as tmp:
    for name in sorted(n for n in files if re.fullmatch(r"actionset_statue_\d{4}", n)):
        psid = name.rsplit("_", 1)[1]
        dec = os.path.join(tmp, "b")
        r = subprocess.run([a.tool, "decrypt", "--keys", a.keys, "--in", os.path.join(a.data, files[name]),
                            "--out", dec], capture_output=True)
        env = UnityPy.load(dec if r.returncode == 0 else os.path.join(a.data, files[name]))
        sets, cin = set(), set()
        for o in env.objects:
            if o.type.name != "MonoBehaviour":
                continue
            try:
                t = o.read_typetree()
            except Exception:
                continue
            fsm = t.get("fsm")
            m = re.search(r"_SKILL_(\d\d)$", t.get("m_Name", ""))
            if not fsm or not m:
                continue
            nn = int(m.group(1))
            sets.add(nn)
            for st in fsm.get("states", []):
                ad = st["actionData"]
                starts = ad["actionStartIndex"]
                for i, an in enumerate(ad["actionNames"]):
                    if an.split(".")[-1] not in SCENE_MODE:
                        continue
                    a1 = starts[i + 1] if i + 1 < len(starts) else len(ad["paramName"])
                    if mode_param(ad, starts[i], a1) == 2:
                        cin.add(nn)
        if os.path.exists(dec):
            os.remove(dec)
        skill_sets[psid] = sorted(sets)
        if cin:
            cinematic[psid] = sorted(cin)

json.dump({"about": "Action-battle SP cinematics (tools/content/scan_action_cinematics.py; "
                    "docs/research/30-action-battle.md §5): per model id (psid), the action-set entries that "
                    "start the SP cinematic (`cinematic`), and every SKILL_<NN> entry of its action set "
                    "(`skillSets`). A skill id SKILL_<p>_<NN> plays entry NN%10 (0 -> 10; 61..70 kept). "
                    "Entry numbers only, read from the player's files.",
           "sources": [{"source": "jdr-research", "doc": "docs/research/30-action-battle.md §5",
                        "note": "player files, actionset_statue_<psid> FSM, DoChangeBattleSceneMode mode 2"}],
           "cinematic": cinematic, "skillSets": skill_sets}, open(a.out, "w"), indent=1)
open(a.out, "a").write("\n")
print(len(skill_sets), "action sets;", len(cinematic), "with a cinematic entry")
