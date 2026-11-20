#!/usr/bin/env python3
"""Inventory the spawn points of the action-battle area scenes in the player's own files: each
area scene (area_<id>) holds a `root` with pivot groups `WAVE_<X>` (their children are the spawn
points, in order), `TRIGGER_<X>` zones, `PlayerStart`, `PlayerResult` and camera targets. The
action battle finds a wave's pivot with GameObject.Find(pivot) and spawns the enemy at child
`offset` (SpawnData.Init 0xCB1E9C, docs/research/30-action-battle.md). Object names and counts
only.

usage: python3 -I tools/content/scan_action_areas.py --data <files dir> --keys local/keys.json \
          --tool target/release/jdr-tool --quests server/content/quests.json \
          --out server/content/action_areas.json
"""
import argparse
import json
import os
import subprocess
import tempfile

import UnityPy

ap = argparse.ArgumentParser()
for x in ("--data", "--keys", "--tool", "--quests", "--out"):
    ap.add_argument(x, required=True)
a = ap.parse_args()

files = {}
for line in subprocess.run([a.tool, "names", "--data", a.data, "--keys", a.keys], check=True,
                           capture_output=True, text=True).stdout.splitlines():
    p = line.split("\t")
    files[p[0]] = p[1]
quests = json.load(open(a.quests))
areas = sorted({x for c in quests["chapters"] if c["stageType"] == 0 for s in c["stages"] for x in s["areas"]})

out, missing = {}, []
with tempfile.TemporaryDirectory() as tmp:
    for area in areas:
        f = files.get(area)
        if not f:
            missing.append(area)
            continue
        dec = os.path.join(tmp, area)
        r = subprocess.run([a.tool, "decrypt", "--keys", a.keys, "--in", os.path.join(a.data, f), "--out", dec],
                           capture_output=True)
        src = dec if r.returncode == 0 else os.path.join(a.data, f)
        env = UnityPy.load(src)
        names, tr = {}, {}
        for o in env.objects:
            if o.type.name == "GameObject":
                names[o.path_id] = o.read_typetree()["m_Name"]
            elif o.type.name == "Transform":
                tr[o.path_id] = o.read_typetree()
        groups = {}
        for t in tr.values():
            n = names.get(t["m_GameObject"]["m_PathID"], "")
            father = tr.get(t["m_Father"]["m_PathID"])
            if father and names.get(father["m_GameObject"]["m_PathID"]) == "root" and \
                    (n.startswith("WAVE_") or n.startswith("TRIGGER_")):
                groups[n] = len(t["m_Children"])
        out[area] = {"waves": {k: v for k, v in sorted(groups.items()) if k.startswith("WAVE_")},
                     "triggers": sorted(k for k in groups if k.startswith("TRIGGER_"))}
# Models with an action-mode action set (actionset_statue_<psid>); the others (late statues) only
# have the tactical one.
action_sets = sorted(n.rsplit("_", 1)[1] for n in files if n.startswith("actionset_statue_"))
json.dump({"about": "Spawn points of the action-battle area scenes (tools/content/scan_action_areas.py): per area "
                    "scene, its WAVE_<X> pivot groups with their number of spawn points, and its TRIGGER_<X> "
                    "zones; and the model ids (psid) that have an action-mode action set. Object names, ids and "
                    "counts from the player's files; docs/research/30-action-battle.md.",
           "actionSets": action_sets, "areas": out}, open(a.out, "w"), indent=1)
open(a.out, "a").write("\n")
print(len(out), "areas;", len(missing), "missing:", missing[:5])
