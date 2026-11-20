#!/usr/bin/env python3
"""Build the battle's script cameras: server/content/masterdata/Node_el_camera_node_common.json and
Set_el_camera_set_common.json (docs/research/31-battle-camera.md).

The original camera tables died with the server. In the original game every action cut to its
own camera shots and only the input phase used the fixed default view (footage, docs/research/31
§1); the statues' tactical action sets request a script camera id at each of those moments
(DoChangeCamera, local/content/camera_requests.tsv from scan_camera_requests.py). We served one
fixed side view for all of them, so the battle looked locked.

What the client does with a set (H, docs/research/31 §2):
  - the action's camera gets one target, the acting unit (tgt0; its Stand for a pair action), and
    is mirrored for enemies (rotate.y and the x offsets negated, CreateNodeParam 0xD70D44);
  - the camera stands at  bone(base) + R * Euler(rotate) * (0, 0, length)  and looks at
    bone(lookat) + R * lookatOffset, R being the unit's own rotation (flagWorldSet off), with the
    bones followed every frame (flagBindBase/flagBindLookat): rotate.y 0 is in front of the unit,
    180 behind it; a negative rotate.x is above it;
  - the steps of a set's dataList play in turn: during step i the camera moves from step i's pose
    to step i+1's over step i's `sec`; the last pose is held until the next camera request
    (holdFinalPose);
  - step i's pose is NOT its node alone: CreateNodeParam 0xD70D44 starts from the first step's node
    and adds the rotate, length, perspective and both offsets of every later node up to i (only the
    bones come from node i). Later nodes are deltas; Build 33 sent absolute ones, which doubled
    distance, angles and field of view on the second step (cameras far away, under the floor or
    looking away, the owner's device test).

Every requested id gets one of the shots the footage shows, all from the side the turn camera
watches (side-on two-shot, three-quarters behind, closer side, slightly low; a high wide view for
big moves; straight behind for ids the client does not mirror), picked by the role of the states
that request it (wind-up, hit, loop, wrap up) and varied by id, each with a slow push-in. The win, boss-intro and finish shots follow the
footage too. Shot values are ours.

usage: python3 -I tools/content/build_camera_sets.py --requests local/content/camera_requests.tsv \
         --masterdata server/content/masterdata
"""
import argparse
import collections
import csv
import json
import os

ap = argparse.ArgumentParser()
ap.add_argument("--requests", required=True)
ap.add_argument("--masterdata", required=True)
a = ap.parse_args()

# CharacterBoneList.Bone (docs/research/12 §5)
WAIST, SPINE2, NECK, HEAD = 1, 4, 5, 6


def node(nid, base, look, rot, length, fov=30.0, boff=(0, 0, 0), loff=(0, 0, 0)):
    v = lambda t: {"x": float(t[0]), "y": float(t[1]), "z": float(t[2])}
    return {"id": nid, "baseBoneID": base, "lookatBoneID": look, "rotate": v(rot), "perspective": float(fov),
            "length": float(length), "baseOffset": v(boff), "lookatOffset": v(loff)}


def delta(nid, a, b):
    """The node that takes a set from pose a to pose b (b's bones; CreateNodeParam adds it to a)."""
    d = lambda k: {c: round(b[k][c] - a[k][c], 4) for c in "xyz"}
    return {"id": nid, "baseBoneID": b["baseBoneID"], "lookatBoneID": b["lookatBoneID"], "rotate": d("rotate"),
            "perspective": round(b["perspective"] - a["perspective"], 4), "length": round(b["length"] - a["length"], 4),
            "baseOffset": d("baseOffset"), "lookatOffset": d("lookatOffset")}


# Shots: (start node, end node), both absolute here; the end node is sent as a delta.
# The unit faces local +Z, its target 1-3 m ahead. rotate.y 90 puts the camera on the unit's right:
# for an ally (facing +X) that is world -Z, the side the turn camera watches from; the client mirrors
# ids 100400-100999 and 110000-149999 for enemies (GameConst DTBattleCameraIdMirroring*, H), so
# their shots land on the same side. In the original footage (owner's frame package: X9xSMXBpsQw
# 0:56-1:07 and 5:12-5:18, ZCUC-PwLXxA 3:49-3:52) every action shot stays on that side: side-on to
# three-quarters behind the attacker, at chest height or a little lower, framing attacker and target
# from the knees or waist up, about 3-5 m away; big area moves land in a high wide view.
SHOTS = {
    # side-on two-shot of attacker and target, chest height
    "side": (node("jdr_side_a", SPINE2, SPINE2, (-4, 100, 0), 4.6, 30, (0, 0, 0), (0, 0, 1.0)),
             node("jdr_side_b", SPINE2, SPINE2, (-4, 98, 0), 4.2, 30, (0, 0, 0), (0, 0, 1.0))),
    # three-quarters behind the attacker, looking past it at the target
    "back": (node("jdr_back_a", SPINE2, SPINE2, (-7, 130, 0), 4.2, 30, (0, 0, 0), (0, -0.15, 1.8)),
             node("jdr_back_b", SPINE2, SPINE2, (-7, 127, 0), 3.8, 30, (0, 0, 0), (0, -0.15, 1.8))),
    # closer side shot on the attacker
    "close": (node("jdr_close_a", SPINE2, NECK, (0, 90, 0), 3.2, 30, (0, 0, 0), (0, -0.1, 0.6)),
              node("jdr_close_b", SPINE2, NECK, (0, 88, 0), 2.9, 30, (0, 0, 0), (0, -0.1, 0.6))),
    # a little low, looking up slightly at the attacker and target
    "low": (node("jdr_low_a", WAIST, NECK, (6, 95, 0), 3.8, 30, (0, 0, 0), (0, 0, 0.9)),
            node("jdr_low_b", WAIST, NECK, (6, 93, 0), 3.5, 30, (0, 0, 0), (0, 0, 0.9))),
    # high and wide, for the big area hits
    "wide": (node("jdr_wide_a", WAIST, WAIST, (-22, 105, 0), 8.0, 32, (0, 0, 0), (0, -0.3, 1.5)),
             node("jdr_wide_b", WAIST, WAIST, (-22, 103, 0), 7.4, 32, (0, 0, 0), (0, -0.3, 1.5))),
    # straight behind the attacker: for the few action ids the client does not mirror (102xxx),
    # which must look right for either team
    "behind": (node("jdr_behind_a", SPINE2, SPINE2, (-10, 180, 0), 4.0, 30, (0, 0, 0), (0, -0.2, 1.6)),
               node("jdr_behind_b", SPINE2, SPINE2, (-10, 180, 0), 3.6, 30, (0, 0, 0), (0, -0.2, 1.6))),
}
EXTRA_NODES = [
    # the client's own test node and the old side view (kept for ids nothing else claims)
    node("ski_cam_00", 0, 0, (-20, 0, 0), 13.0, 25.0, (0, 0, 0), (0, 0.6, 0)),
    node("jdr_side", 0, 0, (-8, 180, 0), 9.0, 30.0, (0, 1.0, 0), (0, 1.0, 0)),
    # win pose: low, chest up, a short push-in (footage: 0.5 s ease-in, then held)
    node("jdr_result", SPINE2, NECK, (8, 22, 0), 2.6, 30),
    node("jdr_result_b", SPINE2, NECK, (8, 24, 0), 2.05, 30),
    # boss intro (id 201, not mirrored, always an enemy facing -X: a negative rotate.y is the
    # watched side): low on the legs, then a tilted low full-body shot (crane up)
    node("jdr_boss_a", WAIST, WAIST, (22, -24, 0), 2.0, 34, (0, 0, 0), (0, -0.4, 0)),
    node("jdr_boss_b", WAIST, HEAD, (14, -30, 10), 3.2, 34),
    # stage finish (not mirrored, the downed enemy): on the ground beside it, watched side
    node("jdr_finish", WAIST, WAIST, (-6, -62, 0), 2.3, 36),
]

ROLE_SHOTS = {
    "in": ["side", "back", "close"],
    "hit": ["side", "low", "close"],
    "loop": ["side", "back"],
    "out": ["side", "close"],
    "any": ["side", "back", "low", "close"],
}


def mirrored(i):
    return 100400 <= i <= 100999 or 110000 <= i <= 149999


def role(state):
    s = state
    if "ヒット" in s or "段目" in s or "hit" in s.lower():
        return "hit"
    if "ループ" in s or "loop" in s.lower():
        return "loop"
    if "アウト" in s or "終" in s or "out" in s.lower():
        return "out"
    if "イン" in s or "開始" in s or "in" in s.lower() or "溜め" in s:
        return "in"
    return "any"


def norm_id(i):
    # ids 1000-2999 are requested with 100000 added (docs/research/12 §5)
    return i + 100000 if 1000 <= i < 3000 else i


roles = collections.defaultdict(collections.Counter)
for r in csv.DictReader(open(a.requests, encoding="utf-8"), delimiter="\t"):
    if r["cameraType"] != "1" or not r["scriptCameraId"]:
        continue
    roles[norm_id(int(r["scriptCameraId"]))][role(r["state"])] += 1

set_path = os.path.join(a.masterdata, "Set_el_camera_set_common.json")
node_path = os.path.join(a.masterdata, "Node_el_camera_node_common.json")
old_sets = json.load(open(set_path))
old_ids = [s["cameraID"] for s in old_sets["data"]["listData"]]

RESULT = set(range(301, 315)) | set(range(101001, 101029))
BOSS_INTRO = {201, 202, 204, 100201}  # 201 the client's; 202/204/100201 requested by the boss's entrance appeal
FINISH = {100300, 100301, 100310, 100311, 350, 351}


def step(nid, sec):
    return {"cameraNode": nid, "sec": sec, "inter": 0.0, "targetIDBase": 0, "targetIDLookat": 0}


def steps(shot, push=1.2):
    # a slow push-in from the start pose to the end pose over `push` seconds, then held
    s0, _ = SHOTS[shot]
    return [step(s0["id"], push), step(s0["id"] + "_to_b", 0.5)]


def cam_set(cid, data, begin=0.0, end=0.3):
    return {"cameraID": cid, "beginInter": begin, "endInter": end, "flagMaintain": False,
            "flagBindBase": True, "flagBindLookat": True, "flagNowConnect": False, "flagWorldSet": False,
            "flagForcusAhead": False, "flagNotUpdateBonePosition": False, "flagNotInter": False, "dataList": data}


sets, used = [], collections.Counter()
for cid in sorted(set(old_ids) | set(roles)):
    if cid in RESULT:
        data = [step("jdr_result", 0.5), step("jdr_result_to_b", 4.0)]
        used["result"] += 1
        sets.append(cam_set(cid, data))
    elif cid in BOSS_INTRO:
        data = [step("jdr_boss_a", 1.4), step("jdr_boss_a_to_b", 0.8)]
        used["boss"] += 1
        sets.append(cam_set(cid, data))
    elif cid in FINISH:
        data = [step("jdr_finish", 1.5)]
        used["finish"] += 1
        sets.append(cam_set(cid, data))
    elif cid in roles:
        rc = roles[cid]
        if not mirrored(cid):
            shot = "behind"  # the same for either team
        elif 140000 <= cid < 150000:
            shot = "wide"  # the 14xxxx family: big moves (e.g. the road roller)
        else:
            top = rc.most_common(1)[0][0]
            choices = ROLE_SHOTS[top]
            shot = choices[cid % len(choices)]
        used[shot] += 1
        sets.append(cam_set(cid, steps(shot)))
    else:
        # hard-coded ids no action set requests (PvP intro 41/42 and the like): the old side view
        sets.append(cam_set(cid, [{"cameraNode": "jdr_side", "sec": 1.0, "inter": 0.3, "targetIDBase": 0,
                                   "targetIDLookat": 0}], 0.3))
        sets[-1]["flagWorldSet"] = True
        sets[-1]["flagBindBase"] = sets[-1]["flagBindLookat"] = False
        used["side"] += 1

by_id = {n["id"]: n for n in EXTRA_NODES}
nodes = [n for a0, b0 in SHOTS.values() for n in (a0, delta(a0["id"] + "_to_b", a0, b0))]
nodes += [n for n in EXTRA_NODES if n["id"] not in ("jdr_result_b", "jdr_boss_b")]
nodes += [delta("jdr_result_to_b", by_id["jdr_result"], by_id["jdr_result_b"]),
          delta("jdr_boss_a_to_b", by_id["jdr_boss_a"], by_id["jdr_boss_b"])]
old_nodes = json.load(open(node_path))
old_nodes["about"] = ("Camera nodes (tools/content/build_camera_sets.py, docs/research/31-battle-camera.md): the shots of the "
                      "original battle footage, framed on the acting unit's bones (CharacterBoneList.Bone WAIST 1, SPINE2 4, "
                      "NECK 5, HEAD 6), all on the side the turn camera watches: side-on two-shot, three-quarters behind, closer "
                      "side, slightly low, high wide, straight behind (each a start node and a <id>_to_b node: the client adds every "
                      "later step's node to the first, so later nodes are deltas); the win, boss-intro "
                      "and finish shots; ski_cam_00 (the client's test node) and jdr_side (the old world-fixed side view). "
                      "Values are ours.")
old_nodes["data"]["listData"] = nodes
old_sets["about"] = ("Script camera sets for every camera id the client hard-codes and every id the statues' tactical action "
                     "sets request (tools/content/build_camera_sets.py, docs/research/31-battle-camera.md): each requested id "
                     "plays one of the footage's shots on the acting unit, chosen by the role of the states that request it, "
                     "with a slow push-in; win shots (301-314, 101001-101028) a low chest-up push-in; the boss intro (201) a "
                     "crane up to a tilted full-body shot; stage finishes (100300-100311, 350/351) a ground-level shot. "
                     "flagMaintain stays false so each shot ends on its own.")
old_sets["data"]["listData"] = sets
for path, doc in ((node_path, old_nodes), (set_path, old_sets)):
    json.dump(doc, open(path, "w"), ensure_ascii=False, indent=1)
    open(path, "a").write("\n")
print(len(sets), "sets,", len(nodes), "nodes;", dict(used))
