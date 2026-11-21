#!/usr/bin/env python3
"""Pose statue models with their team-view clip and measure (or render) them from the team camera.

Reads the player's own decrypted bundles (statue_<psid>, animation_statue_common_<psid>); writes
numbers only (and, with --png, private preview images for local checking). build_statues.py uses
the numbers to frame each statue in the team view (docs/research/12 §7).

How a pose is evaluated (Unity generic AnimationClip, read with UnityPy):
  - m_ClipBindingConstant.genericBindings lists the animated properties; each Transform binding
    (typeID 4) takes 3 curves (attribute 1 position, 3 scale, 4 euler) or 4 (attribute 2 rotation);
    any other binding takes 1. Bindings name their transform by CRC32 of its path below the model
    root.
  - The curve values at a time come from the streamed clip (Hermite keys), then the dense clip
    (sampled frames), then the constant clip, in that order of curve index.
  - The team pose clips are a few frames long; the first frame is used.
Skinning: v' = sum_i w_i * (World(bone_i) @ BindPose_i) @ v.

usage: python3 -I tools/content/pose_models.py --dec <dir of decrypted bundles> \
         --statues server/content/statues.json --out local/content/team_pose.tsv [--png DIR] [--only ID,...]
"""
import argparse, collections, json, math, os, re, struct, sys, zlib
import numpy as np
import UnityPy
from UnityPy.helpers.MeshHelper import MeshHandler

ap = argparse.ArgumentParser()
ap.add_argument("--dec", required=True)
ap.add_argument("--statues", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--png")
ap.add_argument("--only")
ap.add_argument("--clip", help="pose every model with this common clip suffix (e.g. evt_btl_lp, the win appeal) "
                               "instead of the team-view figure clip")
a = ap.parse_args()

POSE = {3: "figure_R", 4: "figure_SR", 5: "figure_SSR", 6: "figure_UR"}


def clip_name(sid, rarity):
    c = int(sid[-2:])
    tens, ones = (c % 100) // 10, c % 10
    suf = ("_" + (str(tens) if tens else "") + ("A" if ones >= 5 else "")) if (tens or ones >= 5) else ""
    return POSE[rarity] + suf


def qmat(x, y, z, w):
    return np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
                     [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
                     [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]])


def euler_mat(ex, ey, ez):
    """Unity Euler (degrees): applied Z, then X, then Y (R = Ry @ Rx @ Rz)."""
    ex, ey, ez = (math.radians(v) for v in (ex, ey, ez))
    cx, sx, cy, sy, cz, sz = math.cos(ex), math.sin(ex), math.cos(ey), math.sin(ey), math.cos(ez), math.sin(ez)
    Rx = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]])
    Ry = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
    Rz = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
    return Ry @ Rx @ Rz


_bundles = {}


def bundle(name):
    if name not in _bundles:
        p = os.path.join(a.dec, name)
        _bundles[name] = UnityPy.load(p) if os.path.exists(p) else None
    return _bundles[name]


def clip_values(t, at=None):
    """Curve values of a clip at its start, in curve-index order."""
    c = t["m_MuscleClip"]["m_Clip"]["data"]
    vals = []
    sc = c["m_StreamedClip"]
    n = sc["curveCount"]
    if n:
        raw = struct.pack("<%dI" % len(sc["data"]), *sc["data"])
        cur = [0.0] * n
        t0 = t["m_MuscleClip"]["m_StartTime"]
        off = 0
        while off + 8 <= len(raw):
            time, cnt = struct.unpack_from("<fI", raw, off)
            off += 8
            if time > t0 + 1e-6:
                break
            for _ in range(cnt):
                idx, k0, k1, k2, k3 = struct.unpack_from("<I4f", raw, off)
                off += 20
                dt = t0 - time if time > -1e30 else 0.0
                cur[idx] = ((k0 * dt + k1) * dt + k2) * dt + k3
        vals += cur
    d = c["m_DenseClip"]
    if d["m_CurveCount"]:
        vals += list(d["m_SampleArray"][: d["m_CurveCount"]])
    vals += list(c["m_ConstantClip"]["data"])
    return vals


def load_model(model):
    """Transforms, renderers and the root of `model` in statue_<psid>."""
    env = bundle("statue_" + model[:4])
    if env is None:
        return None
    objs = {o.path_id: o for o in env.objects}
    go_name, tr = {}, {}
    for o in env.objects:
        if o.type.name == "GameObject":
            go_name[o.path_id] = o.read_typetree()["m_Name"]
    for o in env.objects:
        if o.type.name == "Transform":
            t = o.read_typetree()
            tr[o.path_id] = {"go": t["m_GameObject"]["m_PathID"], "father": t["m_Father"]["m_PathID"],
                             "pos": np.array([t["m_LocalPosition"][k] for k in "xyz"], float),
                             "rot": qmat(*(t["m_LocalRotation"][k] for k in "xyzw")),
                             "scl": np.array([t["m_LocalScale"][k] for k in "xyz"], float)}
    roots = [p for p, t in tr.items() if go_name.get(t["go"]) == model]
    if not roots:
        roots = [p for p, t in tr.items() if not t["father"]]
    root = roots[0]
    # paths below the root
    def path(p):
        parts = []
        while p and p != root and p in tr:
            parts.append(go_name.get(tr[p]["go"], ""))
            p = tr[p]["father"]
        return "/".join(reversed(parts)) if p == root else None
    by_hash = {}
    for p in tr:
        s = path(p)
        if s is not None:
            by_hash[zlib.crc32(s.encode())] = p
    go_tr = {t["go"]: p for p, t in tr.items()}

    def under_root(gid):
        p = go_tr.get(gid)
        while p and p in tr:
            if p == root:
                return True
            p = tr[p]["father"]
        return False

    smrs = []
    for o in env.objects:
        if o.type.name != "SkinnedMeshRenderer":
            continue
        s = o.read_typetree()
        if not under_root(s["m_GameObject"]["m_PathID"]):
            continue
        if not s.get("m_Enabled", 1):
            continue
        mo = objs.get(s["m_Mesh"]["m_PathID"])
        if mo is None:
            continue
        m = mo.read()
        h = MeshHandler(m)
        h.process()
        if not h.m_Vertices:
            continue
        idx = []
        for sm in m.m_SubMeshes:
            first = sm.firstByte // (2 if h.m_Use16BitIndices else 4)
            idx += [sm.baseVertex + i for i in h.m_IndexBuffer[first:first + sm.indexCount]]
        smrs.append({"verts": np.array(h.m_Vertices, float).reshape(-1, 3),
                     "bi": np.array(h.m_BoneIndices, int).reshape(-1, 4) if h.m_BoneIndices else None,
                     "bw": np.array(h.m_BoneWeights, float).reshape(-1, 4) if h.m_BoneWeights else None,
                     "bind": [np.array([[getattr(bp, f"e{r}{c}") for c in range(4)] for r in range(4)])
                              for bp in m.m_BindPose],
                     "bones": [b["m_PathID"] for b in s["m_Bones"]],
                     "tris": np.array(idx, int).reshape(-1, 3),
                     "go": s["m_GameObject"]["m_PathID"]})
    named = {go_name.get(t["go"]): p for p, t in tr.items() if under_root(t["go"])}
    return {"tr": tr, "root": root, "by_hash": by_hash, "smrs": smrs, "named": named}


def apply_clip(mdl, clipname):
    env = bundle("animation_statue_common_" + clipname[:4])
    if env is None:
        return False
    for o in env.objects:
        if o.type.name == "AnimationClip" and o.peek_name() == clipname:
            t = o.read_typetree()
            break
    else:
        return False
    vals = clip_values(t)
    i = 0
    for b in t["m_ClipBindingConstant"]["genericBindings"]:
        n = (4 if b["attribute"] == 2 else 3) if b["typeID"] == 4 else 1
        v = vals[i:i + n]
        i += n
        p = mdl["by_hash"].get(b["path"])
        if b["typeID"] != 4 or p is None or len(v) < n:
            continue
        T = mdl["tr"][p]
        if b["attribute"] == 1:
            T["pos"] = np.array(v, float)
        elif b["attribute"] == 2:
            q = np.array(v, float)
            q /= np.linalg.norm(q) or 1
            T["rot"] = qmat(*q)
        elif b["attribute"] == 3:
            T["scl"] = np.array(v, float)
        elif b["attribute"] == 4:
            T["rot"] = euler_mat(*v)
    return True


def world(mdl):
    tr, W = mdl["tr"], {}
    def w(p):
        if p in W:
            return W[p]
        t = tr[p]
        L = np.eye(4)
        L[:3, :3] = t["rot"] * t["scl"]
        L[:3, 3] = t["pos"]
        # the model root sits at the anchor: ignore what is above it
        W[p] = L if p == mdl["root"] or not t["father"] or t["father"] not in tr else w(t["father"]) @ L
        return W[p]
    for p in tr:
        w(p)
    return W


def skinned(mdl):
    W = world(mdl)
    out = []
    for s in mdl["smrs"]:
        v = np.c_[s["verts"], np.ones(len(s["verts"]))]
        if s["bi"] is None or not s["bones"]:
            M = W.get(s["go"], np.eye(4))
            pv = (v @ M.T)[:, :3]
        else:
            mats = np.array([W.get(b, np.eye(4)) @ bp for b, bp in zip(s["bones"], s["bind"])])
            # Props the pose hides (weapons, effects) sit on bones scaled to ~1% and parked below
            # the floor: drop vertices whose bones are that small.
            bscale = np.abs(np.linalg.det(np.array([W.get(b, np.eye(4))[:3, :3] for b in s["bones"]]))) ** (1 / 3)
            pv = np.zeros((len(v), 3))
            vs = np.zeros(len(v))
            for k in range(4):
                bi = np.clip(s["bi"][:, k], 0, len(mats) - 1)
                pv += s["bw"][:, k:k + 1] * np.einsum("nij,nj->ni", mats[bi], v)[:, :3]
                vs += s["bw"][:, k] * bscale[bi]
            keep = vs > 0.1
            remap = np.cumsum(keep) - 1
            tris = s["tris"][keep[s["tris"]].all(1)]
            pv, s_tris = pv[keep], remap[tris]
            out.append((pv, s_tris))
            continue
        out.append((pv, s["tris"]))
    return out


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
