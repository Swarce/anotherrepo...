#!/usr/bin/env python3
"""Private preview of the team view: posed statues (pose_models.py --png DIR writes DIR/meshes.pkl)
placed on the formation anchors, seen from the formation camera (docs/research/12 §7). Flat-shaded
silhouettes, for checking framing locally; the images stay in local/ or the scratchpad.

usage: python3 -I tools/content/render_team_view.py DIR/meshes.pkl server/content/statues.json OUT.png \
         LEADER[,LEFT,RIGHT] [...more teams]  [--sheet]   (--sheet: every statue alone in the leader slot)
"""
import json, math, pickle, sys
import numpy as np
from PIL import Image, ImageDraw

# Formation scene (FormationSceneController anchors) and camera (CameraSettings): research F (H).
ANCHORS = {"leader": ((0.0, 0.0, -3.5), 180.0), "left": ((-0.7, -0.076, -2.3), 174.0), "right": ((0.7, -0.076, -2.3), 186.0)}
CAM_POS = np.array([0.0, 1.97, -9.75])
CAM_PITCH = 3.5
FOV = 10.0
W, H = 1600, 720  # 20:9; the 16:9 area is marked


def ry(deg):
    r = math.radians(deg)
    return np.array([[math.cos(r), 0, math.sin(r)], [0, 1, 0], [-math.sin(r), 0, math.cos(r)]])


def rx(deg):
    r = math.radians(deg)
    return np.array([[1, 0, 0], [0, math.cos(r), -math.sin(r)], [0, math.sin(r), math.cos(r)]])


CAM_R = rx(CAM_PITCH)
F = 1 / math.tan(math.radians(FOV / 2))


def project(p):
    v = (p - CAM_POS) @ CAM_R  # world -> camera (inverse rotation)
    z = v[:, 2]
    x = v[:, 0] / z * F * (H / 2) + W / 2
    y = H / 2 - v[:, 1] / z * F * (H / 2)
    return np.c_[x, y], z


def place(meshes, slot, scale, offset=(0, 0, 0)):
    """offset: teamPos in cm (world space, added to the anchor position)."""
    pos, yaw = ANCHORS[slot]
    R = ry(yaw)
    out = []
    for pv, tris, mi in meshes:
        out.append(((pv * scale) @ R.T + np.array(pos) + np.array(offset) * 0.01, tris, mi))
    return out


COL = [(205, 170, 120), (120, 160, 220), (160, 210, 140)]


def draw(img, placed):
    d = ImageDraw.Draw(img)
    polys = []
    light = np.array([0.3, 0.5, -0.8])
    light /= np.linalg.norm(light)
    for pv, tris, mi in placed:
        if len(tris) == 0:
            continue
        xy, z = project(pv)
        a, b, c = pv[tris[:, 0]], pv[tris[:, 1]], pv[tris[:, 2]]
        n = np.cross(b - a, c - a)
        n /= np.linalg.norm(n, axis=1, keepdims=True) + 1e-12
        sh = 0.35 + 0.65 * np.abs(n @ light)
        depth = z[tris].mean(1)
        for t, dep, s in zip(tris, depth, sh):
            polys.append((dep, [tuple(xy[i]) for i in t], tuple(int(ch * s) for ch in COL[mi % 3])))
    polys.sort(key=lambda q: -q[0])
    for _, pts, col in polys:
        d.polygon(pts, fill=col)


def frame(img, label=""):
    d = ImageDraw.Draw(img)
    x0 = (W - H * 16 / 9) / 2
    d.rectangle([x0, 0, W - x0, H - 1], outline=(255, 0, 0), width=2)
    d.line([W / 2, 0, W / 2, H], fill=(255, 0, 0))
    if label:
        d.text((x0 + 8, 8), label, fill=(255, 255, 255))


if __name__ == "__main__":
    meshes = {k: v["meshes"] for k, v in pickle.load(open(sys.argv[1], "rb")).items()}
    defs = {d["statueId"]: d for d in json.load(open(sys.argv[2]))["statues"]}
    out = sys.argv[3]
    rest = sys.argv[4:]
    scale = lambda sid: defs[sid].get("teamScale", 100) / 100
    pos = lambda sid: defs[sid].get("teamPos", [0, 0, 0])
    if "--sheet" in rest:
        only = [x for x in rest if x != "--sheet"]
        ids = [s for s in (only[0].split(",") if only else defs) if s in meshes]
        cols = 6
        th = (W // 4, H // 4)
        sheet = Image.new("RGB", (cols * th[0], math.ceil(len(ids) / cols) * th[1]), (30, 30, 30))
        for i, sid in enumerate(ids):
            img = Image.new("RGB", (W, H), (60, 60, 70))
            draw(img, place(meshes[sid], "leader", scale(sid), pos(sid)))
            frame(img)
            img = img.resize(th)
            ImageDraw.Draw(img).text((6, 4), "%s s=%d pos=%s" % (sid, defs[sid].get("teamScale", 100), defs[sid].get("teamPos")), fill=(255, 255, 0))
            sheet.paste(img, ((i % cols) * th[0], (i // cols) * th[1]))
        sheet.save(out)
    else:
        teams = [t.split(",") for t in rest]
        sheet = Image.new("RGB", (W, H * len(teams)), (0, 0, 0))
        for k, team in enumerate(teams):
            img = Image.new("RGB", (W, H), (60, 60, 70))
            placed = []
            for slot, sid in zip(("left", "right", "leader"), (team[1:] + [None, None])[:2] + [team[0]]):
                if sid:
                    placed += place(meshes[sid], slot, scale(sid), pos(sid))
            draw(img, placed)
            frame(img, " ".join(team))
            sheet.paste(img, (0, k * H))
        sheet.save(out)
