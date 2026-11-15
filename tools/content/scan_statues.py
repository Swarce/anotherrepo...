#!/usr/bin/env python3
"""Inventory the statues (playable characters) present in the player's own game files.

For every statueId named in the text tables (STATUE_NAME_<psid>_<variant>), records which of its
bundles exist (model, animations, action sets, ...) and the model names inside `statue_<psid>`
(the `modelDataList` the server sends), and the tactical-battle action names each model has in
`dt_actionset_statue_<psid>` (ATTACK, SKILL_01, ...; objects named `<model>_<NNN>_<ACTION>`), from
which the server picks the statue's basic attack. Identifiers only; nothing from the files is
copied.

usage: python3 -I tools/content/scan_statues.py --data <files dir> --keys local/keys.json \
          --tool target/release/jdr-tool --export local/l10n/work.tsv --out local/content/statues_scan.json
"""
import argparse
import json
import os
import re
import subprocess
import tempfile

import UnityPy

ap = argparse.ArgumentParser()
for a in ("--data", "--keys", "--tool", "--export", "--out"):
    ap.add_argument(a, required=True)
a = ap.parse_args()

names = {}
out = subprocess.run([a.tool, "names", "--data", a.data, "--keys", a.keys], check=True, capture_output=True, text=True).stdout
for line in out.splitlines():
    n, f, _ = line.split("\t")
    names[n] = f

ids = sorted({m.group(1) for line in open(a.export, encoding="utf-8")
              for m in [re.match(r"text:StatueName_JP:STATUE_NAME_(\d{4}_\d{2})\t", line)] if m})
FAMILIES = ["statue", "animation_statue_common", "animation_statue_battle", "animation_statue_talk",
            "animation_statue_spskill", "actionset_statue", "dt_actionset_statue", "frameevents_statue_ste",
            "frameevents_statue_battle", "frameevents_statue_talk", "effect_statue_common", "effect_statue_ste",
            "effect_statue_battle", "effect_statue_talk", "statue_accessory"]
models = {}
dt_actions = {}


def load(bundle):
    """UnityPy environment of one of the player's bundles (decrypted to a temp file if needed)."""
    f = names.get(bundle)
    if not f:
        return None
    p = os.path.join(a.data, f)
    if open(p, "rb").read(4) == b"JJEC":
        t = tempfile.mkdtemp()
        dec = os.path.join(t, "d")
        subprocess.run([a.tool, "decrypt", "--keys", a.keys, "--in", p, "--out", dec], check=True, capture_output=True)
        env = UnityPy.load(open(dec, "rb").read())
        os.remove(dec)
        os.rmdir(t)
        return env
    return UnityPy.load(p)


def actions(psid):
    """{model: sorted action names} from dt_actionset_statue_<psid>."""
    if psid not in dt_actions:
        found = {}
        env = load(f"dt_actionset_statue_{psid}")
        for o in (env.objects if env else []):
            if o.type.name != "MonoBehaviour":
                continue
            try:
                n = o.peek_name()
            except Exception:
                n = o.read().m_Name
            m = re.match(r"(.+?)_(\d{3})_([A-Z][A-Z0-9_]*)$", n or "")
            if m:
                found.setdefault(m.group(1), set()).add(m.group(3))
        dt_actions[psid] = {k: sorted(v) for k, v in found.items()}
    return dt_actions[psid]


def model_names(psid):
    if psid in models:
        return models[psid]
    f = names.get(f"statue_{psid}")
    found = []
    if f:
        with tempfile.TemporaryDirectory() as t:
            p = os.path.join(a.data, f)
            if open(p, "rb").read(4) == b"JJEC":
                dec = os.path.join(t, "d")
                subprocess.run([a.tool, "decrypt", "--keys", a.keys, "--in", p, "--out", dec], check=True, capture_output=True)
                p = dec
            env = UnityPy.load(p)
            for k in env.container.keys():
                m = re.search(r"/(\d{4}_\d_[^/]+)\.fbx$", k)
                if m:
                    found.append(m.group(1))
    models[psid] = sorted(found)
    return models[psid]


rows = []
for sid in ids:
    psid = sid[:4]
    bundles = [f"{fam}_{psid}" for fam in FAMILIES if f"{fam}_{psid}" in names]
    ms = model_names(psid)
    human = [m for m in ms if m[5] == "0"]
    stand = [m for m in ms if m[5] == "1"]
    rows.append({
        "statueId": sid,
        "psid": int(psid),
        "part": int(psid[0]),
        "modelDataList": human[:1] + stand[:1],
        "dtActions": actions(psid).get(human[0], []) if human else [],
        "bundles": bundles,
        "thumbnail": f"st_{sid}",
        "playable": bool(human) and f"animation_statue_common_{psid}" in names
                    and (f"actionset_statue_{psid}" in names or f"dt_actionset_statue_{psid}" in names),
    })
os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
json.dump({"statues": rows}, open(a.out, "w"), indent=1)
print(f"{len(rows)} statueIds, {sum(r['playable'] for r in rows)} playable (model + common animation + action set)")
