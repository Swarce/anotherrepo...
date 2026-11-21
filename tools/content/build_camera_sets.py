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
         --masterdata server/content/masterdata [--statues server/content/statues.json]
"""
import argparse
import collections
import csv
import json
import os
import re

ap = argparse.ArgumentParser()
ap.add_argument("--requests", required=True)
ap.add_argument("--masterdata", required=True)
ap.add_argument("--statues", default="server/content/statues.json")
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
# their shots land on the same side.
# The original's shots, read in every frame of the owner's footage (eight reviews, private, under
# local/wiki-raw/research-b33/camera-pack/review/), depend on the kind of action:
#   - normal attacks and blue skills stay close to the default view: pushed in by 15-25 % or a
#     side-on two-shot, sometimes one low cut behind the attacker (ZCUC-PwLXxA 460-463, 898-904;
#     x0kbxom31cs: no cut at all for many of them);
#   - red (Stand) SP skills and time stops cut between angles on the battlefield: over the shoulder
#     behind the attacker, a low front three-quarter, the reverse from beyond the target looking
#     back, a 90-degree side shot, a high wide view 45-60 degrees down (ZCUC 930-952, 1009-1019,
#     1072-1080; x0kb 166-233, 515-536; X9xSMXBpsQw 3:00-3:13).
# All of them watch from the turn camera's side of the line between the teams.
# Measured on the original frames (X9xSMXBpsQw 113-133, ZCUC-PwLXxA 460-920, X9xS 628) and
# checked by rendering posed models (attacker and target 1.8 m apart): the main shot stands on the
# turn camera's side 45-60 degrees off the attacker's facing, low (about 1.1 m) and level, about
# 5 m from the attacker, so the attacker is at the left third head to feet and the target nearer
# the camera at the right; the second is over the attacker's shoulder about 3.8 m back. The
# Build 33 shots (perpendicular or behind, 2.6-4.6 m, 1.6-2.0 m up, 30 degrees) were too close,
# too high and too square to the action (the owner's Build 34 recording, 0:37-0:42).
SHOTS = {
    # attacker at the left third, the target nearer at the right (the most common original shot)
    "two": (node("jdr_two_a", SPINE2, SPINE2, (2, 45, 0), 5.0, 36, (0, 0, 0), (0, -0.1, 1.1)),
            node("jdr_two_b", SPINE2, SPINE2, (2, 44, 0), 4.6, 36, (0, 0, 0), (0, -0.1, 1.1))),
    # the same, wider and squarer (both fighters full body)
    "two_wide": (node("jdr_twow_a", SPINE2, SPINE2, (0, 60, 0), 6.2, 36, (0, 0, 0), (0, -0.1, 1.0)),
                 node("jdr_twow_b", SPINE2, SPINE2, (0, 59, 0), 5.7, 36, (0, 0, 0), (0, -0.1, 1.0))),
    # over the attacker's right shoulder, its back at mid-left, the target beyond
    "ots": (node("jdr_ots_a", SPINE2, SPINE2, (-2, 150, 0), 3.8, 36, (0.3, 0, 0), (0, -0.2, 2.0)),
            node("jdr_ots_b", SPINE2, SPINE2, (-2, 152, 0), 3.5, 36, (0.3, 0, 0), (0, -0.2, 2.0))),
    # SP: low front three-quarter close-up of the attacker
    "front_low": (node("jdr_flow_a", SPINE2, NECK, (12, 40, 0), 2.4, 36),
                  node("jdr_flow_b", SPINE2, NECK, (12, 44, 0), 2.15, 36)),
    # SP: the reverse, from beyond the target looking back at the attacker
    "reverse": (node("jdr_rev_a", SPINE2, SPINE2, (3, 20, 0), 4.6, 36, (0, 0, 0), (0, 0, 0.6)),
                node("jdr_rev_b", SPINE2, SPINE2, (3, 22, 0), 4.2, 36, (0, 0, 0), (0, 0, 0.6))),
    # SP and big area moves: high and wide, about 50 degrees down
    "wide": (node("jdr_wide_a", WAIST, WAIST, (-50, 115, 0), 7.2, 36, (0, 0, 0), (0, -0.3, 1.5)),
             node("jdr_wide_b", WAIST, WAIST, (-50, 113, 0), 6.6, 36, (0, 0, 0), (0, -0.3, 1.5))),
    # straight behind the attacker: for the few action ids the client does not mirror (102xxx),
    # which must look right for either team
    "behind": (node("jdr_behind_a", SPINE2, SPINE2, (-6, 180, 0), 4.0, 36, (0, 0, 0), (0, -0.2, 1.6)),
               node("jdr_behind_b", SPINE2, SPINE2, (-6, 180, 0), 3.6, 36, (0, 0, 0), (0, -0.2, 1.6))),
}
EXTRA_NODES = [
    # the client's own test node and the old side view (kept for ids nothing else claims)
    node("ski_cam_00", 0, 0, (-20, 0, 0), 13.0, 25.0, (0, 0, 0), (0, 0.6, 0)),
    node("jdr_side", 0, 0, (-8, 180, 0), 9.0, 30.0, (0, 1.0, 0), (0, 1.0, 0)),
    # win pose (footage: ZCUC-PwLXxA 5:17-5:22 and 9:06, X9xSMXBpsQw 6:16-6:20): a tight head-and-
    # chest shot from well below (about 25-30 degrees up at the face), the winner right of centre
    # and turned towards screen left where WIN! sits; it starts closer and more turned and settles
    # over about 1 s, then holds
    # The winner stands on the result pivot facing away from the frame the battle shots use: the
    # owner's recording of Build 34 (0:48-0:52) showed this shot from behind. So the result nodes
    # are those numbers turned 180 degrees about the winner (rotate.y + 180, offsets' x and z
    # negated).
    node("jdr_result", SPINE2, NECK, (30, 214, 0), 1.0, 34, (0, 0, 0), (-0.2, 0.12, 0)),
    node("jdr_result_b", SPINE2, NECK, (26, 207, 0), 1.15, 34, (0, 0, 0), (-0.26, 0.1, 0)),
    # boss intro (id 201, not mirrored, always an enemy facing -X: a negative rotate.y is the
    # watched side): low on the legs, then a tilted low full-body shot (crane up)
    node("jdr_boss_a", WAIST, WAIST, (22, -24, 0), 2.0, 34, (0, 0, 0), (0, -0.4, 0)),
    node("jdr_boss_b", WAIST, HEAD, (14, -30, 10), 3.2, 34),
    # stage finish (not mirrored, the downed enemy): on the ground beside it, watched side
    node("jdr_finish", WAIST, WAIST, (-6, -62, 0), 2.3, 36),
]

# shots by the kind of action requesting the id (attack, skill, sp) and the role of its states
KIND_SHOTS = {
    "attack": {"in": ["two", "two_wide"], "hit": ["two", "ots"], "loop": ["two"],
               "out": ["two_wide"], "any": ["two", "ots"]},
    "skill": {"in": ["two", "ots"], "hit": ["two", "ots", "two_wide"], "loop": ["two", "ots"],
              "out": ["two_wide", "two"], "any": ["two", "ots", "two_wide"]},
    "sp": {"in": ["ots", "front_low", "two"], "hit": ["reverse", "two", "wide", "front_low"],
           "loop": ["two", "reverse", "ots"], "out": ["wide", "front_low"],
           "any": ["ots", "reverse", "front_low", "two"]},
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


# Which skills are red (SP): the statues' tactical skills (statues.json dtSkills `special`).
SPECIAL = {}
for st in json.load(open(a.statues, encoding="utf-8"))["statues"]:
    for k in st.get("dtSkills") or []:
        SPECIAL[k["id"]] = SPECIAL.get(k["id"], False) or bool(k["special"])


def kind(psid, template):
    """attack / skill / sp from the action set's template name (e.g. 3000_1_stapla_016_SKILL_06)."""
    m = re.search(r"_\d{3}_([A-Z].*)$", template)
    suffix = m.group(1) if m else ""
    ms = re.fullmatch(r"SKILL_(\d\d)", suffix)
    if ms:
        return "sp" if SPECIAL.get("SKILL_%s_%s" % (psid[:3], ms.group(1))) else "skill"
    return "attack" if "ATTACK" in suffix else None


roles = collections.defaultdict(collections.Counter)
kinds = collections.defaultdict(collections.Counter)
for r in csv.DictReader(open(a.requests, encoding="utf-8"), delimiter="\t"):
    if r["cameraType"] != "1" or not r["scriptCameraId"]:
        continue
    cid = norm_id(int(r["scriptCameraId"]))
    roles[cid][role(r["state"])] += 1
    k = kind(r["psid"], r.get("template", ""))
    if k:
        kinds[cid][k] += 1

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
        data = [step("jdr_result", 1.0), step("jdr_result_to_b", 4.0)]
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
        else:
            top = rc.most_common(1)[0][0]
            kc = kinds[cid]
            # an id any SP requests gets the SP grammar when SPs are at least a third of its users
            k = "sp" if kc["sp"] * 3 >= sum(kc.values()) > 0 else (kc.most_common(1)[0][0] if kc else "skill")
            if 140000 <= cid < 150000 and k == "sp" and top in ("hit", "out"):
                shot = "wide"  # the 14xxxx family: big moves (e.g. the road roller)
            else:
                choices = KIND_SHOTS[k][top]
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
