#!/usr/bin/env python3
"""Pose statue models with their team-view clip and measure (or render) them from the team camera.

Reads the player's own decrypted bundles (statue_<psid>, animation_statue_common_<psid>); writes
numbers only (and, with --png, private preview images for local checking). build_statues.py uses
the numbers to frame each statue in the team view (docs/research/12 §7).

How a pose is evaluated: see posing.py.

usage: python3 -I tools/content/pose_models.py --dec <dir of decrypted bundles> \
         --statues server/content/statues.json --out local/content/team_pose.tsv [--png DIR] [--only ID,...]
"""
import argparse, json, os, sys
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import posing  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("--dec", required=True)
ap.add_argument("--statues", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--png")
ap.add_argument("--only")
ap.add_argument("--clip", help="pose every model with this common clip suffix (e.g. evt_btl_lp, the win appeal) "
                               "instead of the team-view figure clip")
a = ap.parse_args()
B = posing.Bundles(a.dec)
clip_name, world, skinned = posing.clip_name, posing.world, posing.skinned


def load_model(model):
    return posing.load_model(B, model)


def apply_clip(mdl, clipname):
    return posing.apply_clip(B, mdl, clipname)


statues = json.load(open(a.statues))["statues"]
only = set(a.only.split(",")) if a.only else None
rows = []
cache = {}
for d in statues:
    sid = d["statueId"]
    if only and sid not in only:
        continue
    r = d.get("rarity") or 4
    meshes, posed, bones = [], [], {}
    for model in d["modelDataList"]:
        mdl = load_model(model)
        if mdl is None:
            continue
        ok = apply_clip(mdl, f"{model}_{a.clip or clip_name(sid, r)}")
        posed.append(ok)
        if len(posed) == 1:
            W = world(mdl)
            bones = {n: W[mdl["named"][n]][:3, 3] for n in ("HEAD", "NECK", "SPINE2", "WAIST", "RESERVE")
                     if n in mdl["named"]}
        meshes += [(pv, tr, len(posed) - 1) for pv, tr in skinned(mdl)]
    if not meshes:
        continue
    allv = np.vstack([m[0] for m in meshes])
    lo, hi = allv.min(0), allv.max(0)
    if not any(m[2] == 0 and len(m[0]) for m in meshes):
        print(sid, "first model has no mesh", file=sys.stderr)
        continue
    first = np.vstack([m[0] for m in meshes if m[2] == 0])
    flo, fhi = first.min(0), first.max(0)
    rows.append((sid, int(posed[0]), lo, hi, flo, fhi, bones))
    cache[sid] = {"meshes": meshes, "bones": bones}
    print(sid, "posed" if posed[0] else "UNPOSED", "".join("+" if p else "-" for p in posed), "group x[%.2f,%.2f] y[%.2f,%.2f] z[%.2f,%.2f]" % (lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]),
          "first y[%.2f,%.2f]" % (flo[1], fhi[1]), file=sys.stderr)

with open(a.out, "w") as f:
    f.write("statueId\tposed\tgroup_xmin\tgroup_xmax\tgroup_ymin\tgroup_ymax\tgroup_zmin\tgroup_zmax\tfirst_xmin\tfirst_xmax\tfirst_ymin\tfirst_ymax"
            "\thead_x\thead_y\twaist_x\twaist_y\tupper_xmin\tupper_xmax\n")
    for sid, ok, lo, hi, flo, fhi, bones in rows:
        hb, wb = bones.get("HEAD"), bones.get("WAIST")
        first = np.vstack([m[0] for m in cache[sid]["meshes"] if m[2] == 0])
        cut = wb[1] if wb is not None else 0.5 * fhi[1]
        up = first[first[:, 1] >= cut]
        up = up if len(up) else first
        extra = [hb[0] if hb is not None else None, hb[1] if hb is not None else None,
                 wb[0] if wb is not None else None, wb[1] if wb is not None else None, up[:, 0].min(), up[:, 0].max()]
        f.write("\t".join([sid, str(ok)] + ["%.3f" % x for x in (lo[0], hi[0], lo[1], hi[1], lo[2], hi[2], flo[0], fhi[0], flo[1], fhi[1])]
                           + ["" if x is None else "%.3f" % x for x in extra]) + "\n")

if a.png:
    import pickle
    os.makedirs(a.png, exist_ok=True)
    pickle.dump(cache, open(os.path.join(a.png, "meshes.pkl"), "wb"))
