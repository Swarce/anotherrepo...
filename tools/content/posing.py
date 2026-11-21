"""Pose statue models with an animation clip (shared by pose_models.py and render_portraits.py).

Reads the player's own decrypted bundles (statue_<psid>, animation_statue_common_<psid>).

How a pose is evaluated (Unity generic AnimationClip, read with UnityPy):
  - m_ClipBindingConstant.genericBindings lists the animated properties; each Transform binding
    (typeID 4) takes 3 curves (attribute 1 position, 3 scale, 4 euler) or 4 (attribute 2 rotation);
    any other binding takes 1. Bindings name their transform by CRC32 of its path below the model
    root.
  - The curve values at a time come from the streamed clip (Hermite keys), then the dense clip
    (sampled frames), then the constant clip, in that order of curve index.
  - The figure (menu) pose clips are a few frames long; the first frame is used.
Skinning: v' = sum_i w_i * (World(bone_i) @ BindPose_i) @ v.
"""
import math, os, struct, zlib
import numpy as np
import UnityPy
from UnityPy.helpers.MeshHelper import MeshHandler

POSE = {3: "figure_R", 4: "figure_SR", 5: "figure_SSR", 6: "figure_UR"}


def clip_name(sid, rarity):
    """The figure (team view / menu) pose clip of a statue: figure_<rarity>[_<tens>][A]."""
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


class Bundles:
    def __init__(self, dec):
        self.dec, self._b = dec, {}

    def get(self, name):
        if name not in self._b:
            p = os.path.join(self.dec, name)
            self._b[name] = UnityPy.load(p) if os.path.exists(p) else None
        return self._b[name]


def clip_values(t):
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


def load_model(bundles, model, textures=False):
    """Transforms, renderers and the root of `model` in statue_<psid>. With `textures`, each
    renderer also keeps its UVs, its triangles per material and each material's main texture."""
    env = bundles.get("statue_" + model[:4])
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
        subs = []
        for sm in m.m_SubMeshes:
            first = sm.firstByte // (2 if h.m_Use16BitIndices else 4)
            subs.append(np.array([sm.baseVertex + i for i in h.m_IndexBuffer[first:first + sm.indexCount]],
                                 int).reshape(-1, 3))
        r = {"verts": np.array(h.m_Vertices, float).reshape(-1, 3),
             "bi": np.array(h.m_BoneIndices, int).reshape(-1, 4) if h.m_BoneIndices else None,
             "bw": np.array(h.m_BoneWeights, float).reshape(-1, 4) if h.m_BoneWeights else None,
             "bind": [np.array([[getattr(bp, f"e{r}{c}") for c in range(4)] for r in range(4)])
                      for bp in m.m_BindPose],
             "bones": [b["m_PathID"] for b in s["m_Bones"]],
             "tris": np.vstack(subs) if subs else np.zeros((0, 3), int),
             "go": s["m_GameObject"]["m_PathID"],
             "name": go_name.get(s["m_GameObject"]["m_PathID"], "")}
        if textures:
            r["uv"] = np.array(h.m_UV0, float).reshape(-1, 2) if h.m_UV0 else np.zeros((len(r["verts"]), 2))
            r["subs"] = subs
            r["tex"] = []
            for i in range(len(subs)):
                mats = s["m_Materials"]
                mp = mats[min(i, len(mats) - 1)]["m_PathID"] if mats else 0
                r["tex"].append(main_texture(objs, mp))
        smrs.append(r)
    named = {go_name.get(t["go"]): p for p, t in tr.items() if under_root(t["go"])}
    return {"tr": tr, "root": root, "by_hash": by_hash, "smrs": smrs, "named": named}


def main_texture(objs, mat_pid):
    """(texture name, PIL image) of a material's first texture (statue materials name it _TEX)."""
    mo = objs.get(mat_pid)
    if mo is None:
        return None
    mt = mo.read_typetree()
    envs = mt["m_SavedProperties"]["m_TexEnvs"]
    for want in ("_TEX", "_MainTex", None):
        for k, v in envs:
            p = v["m_Texture"]["m_PathID"]
            if p and p in objs and (want is None or k == want):
                t = objs[p].read()
                return (t.m_Name, t.image)
    return None


def apply_clip(bundles, mdl, clipname):
    env = bundles.get("animation_statue_common_" + clipname[:4])
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


def skin(mdl, s, W):
    """Posed vertices of one renderer and the mask of those kept. Props the pose hides
    (weapons, effects) sit on bones scaled to ~1% and parked below the floor: their vertices are
    dropped."""
    v = np.c_[s["verts"], np.ones(len(s["verts"]))]
    if s["bi"] is None or not s["bones"]:
        M = W.get(s["go"], np.eye(4))
        return (v @ M.T)[:, :3], np.ones(len(v), bool)
    mats = np.array([W.get(b, np.eye(4)) @ bp for b, bp in zip(s["bones"], s["bind"])])
    bscale = np.abs(np.linalg.det(np.array([W.get(b, np.eye(4))[:3, :3] for b in s["bones"]]))) ** (1 / 3)
    pv = np.zeros((len(v), 3))
    vs = np.zeros(len(v))
    for k in range(4):
        bi = np.clip(s["bi"][:, k], 0, len(mats) - 1)
        pv += s["bw"][:, k:k + 1] * np.einsum("nij,nj->ni", mats[bi], v)[:, :3]
        vs += s["bw"][:, k] * bscale[bi]
    return pv, vs > 0.1


def skinned(mdl):
    """[(posed vertices, triangles)] per renderer, hidden props dropped."""
    W = world(mdl)
    out = []
    for s in mdl["smrs"]:
        pv, keep = skin(mdl, s, W)
        remap = np.cumsum(keep) - 1
        tris = s["tris"][keep[s["tris"]].all(1)]
        out.append((pv[keep], remap[tris]))
    return out


def skinned_parts(mdl):
    """Like skinned(), per material: [{verts, uv, tris, tex}] (needs load_model(textures=True))."""
    W = world(mdl)
    out = []
    for s in mdl["smrs"]:
        pv, keep = skin(mdl, s, W)
        remap = np.cumsum(keep) - 1
        for tris, tex in zip(s["subs"], s["tex"]):
            t = tris[keep[tris].all(1)]
            if len(t):
                out.append({"verts": pv, "uv": s["uv"], "tris": t, "tex": tex, "name": s["name"]})
        del remap
    return out


def bone_positions(mdl, names=("HEAD", "NECK", "SPINE2", "WAIST", "RESERVE")):
    W = world(mdl)
    return {n: W[mdl["named"][n]][:3, 3] for n in names if n in mdl["named"]}


def bone_matrix(mdl, name):
    W = world(mdl)
    p = mdl["named"].get(name)
    return W[p] if p else None
