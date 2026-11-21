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
ap.add_argument("--calibrate", action="store_true",
                help="every action id plays one unmistakable shot (high, in front, to the right, 45-degree field), for a "
                     "device test of the camera conventions; result, finish and boss shots stay")
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


def P(base, look, rot, length, fov=30.0, boff=(0, 0, 0), loff=(0, 0, 0)):
    """An absolute camera pose (node fields without the id)."""
    n = node("", base, look, rot, length, fov, boff, loff)
    del n["id"]
    return n


# Shots measured on every tactical action of the owner's footage (6 videos, 223 actions, read frame
# by frame with background alignment; private, local/wiki-raw/research-b33/camera-pack/, workflow
# notes in docs/research/31 §3). Each shot is a list of steps (absolute pose, sec, inter):
#   - `inter` is the per-frame share of the way to the pose (SetCameraInfo 0xD6FE60): 0.09 is the
#     original's glide (about 93 % of the way in 0.5 s, the measured push-in), 1.0 a hard cut;
#   - during step i the pose blends to step i+1's over step i's sec, so (A, h), (A, m), (B, ...)
#     holds A for h and then moves to B over m;
#   - later poses are sent as deltas (CreateNodeParam adds every later node to the first).
# rotate.x positive is below the bone; rotate.y 0 in front of the unit, 90 its right side (the
# turn camera's side for allies; enemies are mirrored by the client for 1104xx-1499xx ids);
# offsets in the unit's frame, z towards the target.
NA_A = P(SPINE2, SPINE2, (3, 82, 0), 4.0, 30, (0, 0, 1.8), (0, -0.1, 1.8))
NA_B = P(SPINE2, SPINE2, (3, 82, 0), 4.5, 30, (0, 0, 2.8), (0, -0.1, 2.8))
DEFAULT_REF = P(SPINE2, SPINE2, (-5, 75, 0), 6.5, 30, (0, 0, 1.8), (0, -0.2, 1.8))
RUSH = [P(SPINE2, SPINE2, (15, 90, 0), 1.5, 38, (0, 0, 0), (0, 0, 0.8)),
        P(SPINE2, SPINE2, (20, 60, 0), 1.4, 38, (0, 0, 0), (0, 0, 0.8)),
        P(SPINE2, SPINE2, (10, 120, 10), 1.6, 38, (0, 0, 0), (0, 0, 0.8)),
        P(SPINE2, SPINE2, (25, 90, 0), 1.2, 38, (0, 0, 0), (0, 0, 0.8)),
        P(SPINE2, SPINE2, (5, 75, 0), 3.0, 38, (0, 0, 0), (0, 0, 0.8))]
CFL = P(NECK, NECK, (20, 30, 0), 1.3, 35, (0, 0, 0), (0, -0.1, 0))
LFW = P(SPINE2, SPINE2, (18, 35, 0), 2.0, 45, (0, 0, 0), (0, 0.2, 0))
BAL = P(SPINE2, SPINE2, (5, 160, 0), 2.5, 32, (0, 0, 0), (0, 0, 3.0))
BAL2 = P(SPINE2, SPINE2, (5, 160, 0), 2.33, 32, (0, 0, 0), (0, 0, 3.0))
REV = P(SPINE2, SPINE2, (0, 15, 0), 6.0, 30, (0, 0, 0), (0, -0.1, 0.5))
KNEE = P(SPINE2, SPINE2, (6, 88, 0), 2.6, 30, (0, 0, 1.0), (0, -0.1, 1.0))
SHOTS = {
    # normal attacks (111000): glide in towards the target along the turn view's line, hold, then a
    # 1 m truck with a slight pull-out (measured: no cut in 54 of 59 ally attacks)
    "NA_PUSH": [(NA_A, 0.6, 0.09), (NA_A, 1.5, 0.09), (NA_B, 2.0, 0.09)],
    # a later request inside the same attack: stay (no visible change)
    "NA_HOLD": [(NA_B, 2.0, 0.05)],
    # Stand attacks (131000): the same, closer, trucking with the Stand
    "ST3_PUSH": [(P(SPINE2, SPINE2, (7, 82, 0), 3.2, 30, (0, 0, 2.0), (0, -0.1, 2.0)), 0.5, 0.09),
                 (P(SPINE2, SPINE2, (7, 82, 0), 3.2, 30, (0, 0, 2.0), (0, -0.1, 2.0)), 1.2, 0.09),
                 (P(SPINE2, SPINE2, (7, 82, 0), 3.6, 30, (0, 0, 3.2), (0, -0.1, 3.2)), 1.5, 0.09)],
    # 131101: a lower, waist-up follow
    "FOLLOW": [(P(SPINE2, SPINE2, (10, 85, 0), 2.6, 30, (0, 0, 2.4), (0, -0.1, 2.4)), 0.3, 0.09),
               (P(SPINE2, SPINE2, (10, 85, 0), 2.6, 30, (0, 0, 2.4), (0, -0.1, 2.4)), 1.2, 0.09),
               (P(SPINE2, SPINE2, (10, 85, 0), 2.6, 30, (0, 0, 3.4), (0, -0.1, 3.4)), 1.0, 0.09)],
    # 101008 wrap-up: glide out towards the turn view
    "PULL_OUT": [(P(SPINE2, SPINE2, (-2, 78, 0), 5.6, 30, (0, 0, 1.8), (0, -0.2, 1.8)), 0.5, 0.1)],
    # a request that should change nothing visible: the turn view, glided very slowly
    "DEFAULT_HOLD": [(DEFAULT_REF, 1.0, 0.05)],
    # skills and SPs (cuts). The original holds an angle 0.5-1 s inside a skill (measured mean 3.5
    # cuts per blue skill, 5.7 per SP with its cinematic), so a cut shot holds its first angle and
    # then cuts once more (a 0.05 s blend reads as a cut).
    "CLOSE_FRONT_LOW": [(CFL, 0.6, 1.0), (CFL, 0.05, 1.0), (LFW, 0.8, 1.0)],
    "LOW_FRONT_WIDE": [(LFW, 0.6, 1.0), (LFW, 0.05, 1.0), (REV, 1.0, 1.0)],
    "BEHIND_ACTOR_LOW": [(BAL, 0.5, 1.0), (BAL2, 0.5, 1.0), (BAL2, 0.05, 1.0), (REV, 1.0, 1.0)],
    "REVERSE": [(REV, 0.8, 1.0), (REV, 0.05, 1.0), (KNEE, 1.0, 1.0)],
    "SIDE_CLOSE": [(P(SPINE2, SPINE2, (6, 88, 0), 2.6, 30, (0, 0, 1.0), (0, -0.1, 1.0)), 3.2, 1.0)],
    "NEAR_DEFAULT_WIDE": [(P(SPINE2, SPINE2, (-6, 88, 0), 7.0, 30, (0, 0, 2.2), (0, -0.2, 2.2)), 1.0, 1.0),
                          (P(SPINE2, SPINE2, (-6, 88, 0), 7.35, 30, (0, 0, 2.2), (0, -0.2, 2.2)), 1.0, 1.0)],
    "HIGH_3Q": [(P(SPINE2, SPINE2, (-40, 30, 0), 5.5, 35, (0, 0, 0), (0, 0, 2.5)), 1.5, 1.0)],
    "TOPDOWN": [(P(WAIST, WAIST, (-65, 90, 0), 8.0, 35, (0, 0, 2.5), (0, 0, 2.5)), 1.0, 1.0),
                (P(WAIST, WAIST, (-60, 90, 0), 10.0, 35, (0, 0, 2.5), (0, 0, 2.5)), 1.5, 1.0)],
    "HIGH_BEHIND_PARTY": [(P(SPINE2, SPINE2, (-45, 150, 0), 8.0, 35, (0, 0, 0), (0, 0, 3.0)), 2.5, 1.0),
                          (P(SPINE2, SPINE2, (-45, 150, 0), 10.0, 35, (0, 0, 0), (0, 0, 3.0)), 1.0, 1.0)],
    "HIGH_BEHIND_DRIFT": [(P(SPINE2, SPINE2, (-40, 160, 0), 6.0, 35, (0, 0, 0), (0, 0, 3.0)), 1.5, 1.0),
                          (P(SPINE2, SPINE2, (-40, 160, 0), 6.8, 35, (0, 0, 0), (0, 0, 3.0)), 1.0, 1.0)],
    "STAND_PUSH": [(P(SPINE2, SPINE2, (0, 85, 0), 4.0, 30, (0, 0, 0.8), (0, -0.1, 0.8)), 1.0, 0.09)],
    "STAND_PUSH_CLOSE": [(P(SPINE2, SPINE2, (1, 90, 0), 3.5, 30, (0, 0, 0.8), (0, -0.1, 0.8)), 1.0, 0.09)],
    "SIDE_GLIDE": [(P(SPINE2, SPINE2, (4, 88, 0), 3.6, 30, (0, 0, 1.5), (0, -0.1, 1.5)), 1.0, 0.09)],
    "TWO_SHOT_FRONT_LOW": [(P(SPINE2, SPINE2, (17, 30, 0), 2.2, 34, (0, 0, 0), (0, 0, 0.3)), 0.7, 1.0)],
    "STAND_LOOM": [(P(NECK, NECK, (20, 35, 0), 2.3, 35, (0, 0, 0), (0, -0.2, 0)), 1.0, 1.0)],
    "CHEST_UP": [(P(NECK, NECK, (10, 20, 0), 1.8, 35, (0, 0, 0), (0, -0.1, 0)), 0.6, 1.0)],
    "LOW_FRONT_STAND": [(P(SPINE2, SPINE2, (15, 40, 0), 1.4, 45, (0, 0, 0), (0, 0.2, 0)), 0.5, 1.0)],
    "GANG_PARTYSIDE": [(P(SPINE2, SPINE2, (6, 45, 0), 4.3, 30, (0, 0, 0), (0, -0.1, 1.6)), 1.2, 1.0)],
    # rush: a cut every 0.5 s through five close angles (hold, then a 0.05 s blend that reads as a cut)
    "RUSH_CUTS": [x for pz in RUSH for x in ((pz, 0.45, 1.0), (pz, 0.05, 1.0))],
    # straight behind the attacker: for the few action ids the client does not mirror (102xxx),
    # which must look right for either team
    "behind": [(P(SPINE2, SPINE2, (-6, 180, 0), 4.0, 36, (0, 0, 0), (0, -0.2, 1.6)), 1.2, 1.0),
               (P(SPINE2, SPINE2, (-6, 180, 0), 3.6, 36, (0, 0, 0), (0, -0.2, 1.6)), 0.5, 1.0)],
}

# ids whose shot was observed in the footage (the measurement's id mapping)
OVERRIDE = {
    111000: "NA_PUSH", 131000: "ST3_PUSH", 131101: "FOLLOW", 101008: "PULL_OUT", 101007: "SIDE_CLOSE",
    130000: "CLOSE_FRONT_LOW", 130001: "HIGH_3Q", 130002: "NEAR_DEFAULT_WIDE",
    130003: "SIDE_GLIDE", 130004: "STAND_PUSH", 130005: "REVERSE", 130007: "LOW_FRONT_STAND",
    130009: "STAND_PUSH_CLOSE", 130010: "TOPDOWN", 130603: "BEHIND_ACTOR_LOW", 130606: "HIGH_BEHIND_DRIFT",
    130616: "HIGH_BEHIND_PARTY", 130624: "TWO_SHOT_FRONT_LOW", 130639: "CHEST_UP", 140002: "BEHIND_ACTOR_LOW",
    120900: "NEAR_DEFAULT_WIDE", 143800: "NEAR_DEFAULT_WIDE", 113800: "NEAR_DEFAULT_WIDE",
}
for _rush in (130801, 130811, 130900, 130901):
    OVERRIDE[_rush] = "RUSH_CUTS"

# Unobserved ids: weighted by the measured frequencies (blue skills: 10 % no change, 15 % glide,
# 75 % cut at the wind-up; 5/15/80 at the hit; rush loops; a cut near the turn view at the
# wrap-up; red SPs: a glide or close cuts at the wind-up, 2-4 field cuts, a cut near the turn view).
KIND_SHOTS = {
    "attack": {"in": ["NA_PUSH"], "hit": ["NA_HOLD"], "loop": ["NA_HOLD"], "out": ["PULL_OUT"],
               "any": ["NA_PUSH"]},
    "skill": {"in": ["CLOSE_FRONT_LOW"] * 4 + ["BEHIND_ACTOR_LOW"] * 3 + ["SIDE_GLIDE"] * 2 + ["DEFAULT_HOLD"],
              "hit": ["REVERSE"] * 7 + ["LOW_FRONT_WIDE"] * 5 + ["SIDE_CLOSE"] * 4 + ["SIDE_GLIDE"] * 3 + ["DEFAULT_HOLD"],
              "loop": ["RUSH_CUTS"] * 2 + ["REVERSE", "SIDE_CLOSE"],
              "out": ["NEAR_DEFAULT_WIDE"],
              "any": ["CLOSE_FRONT_LOW", "REVERSE", "LOW_FRONT_WIDE", "SIDE_GLIDE"]},
    "sp": {"in": ["STAND_PUSH"] * 2 + ["CLOSE_FRONT_LOW"] * 2 + ["LOW_FRONT_WIDE"],
           "hit": ["BEHIND_ACTOR_LOW", "HIGH_3Q", "TOPDOWN", "HIGH_BEHIND_PARTY", "REVERSE"],
           "loop": ["HIGH_BEHIND_DRIFT", "REVERSE", "BEHIND_ACTOR_LOW"],
           "out": ["NEAR_DEFAULT_WIDE"],
           "any": ["BEHIND_ACTOR_LOW", "REVERSE", "CLOSE_FRONT_LOW", "HIGH_3Q"]},
}


def pick(choices, cid):
    return choices[(cid * 2654435761 % 2 ** 32) % len(choices)]


EXTRA_NODES = [
    # the client's own test node and the old side view (kept for ids nothing else claims)
    node("ski_cam_00", 0, 0, (-20, 0, 0), 13.0, 25.0, (0, 0, 0), (0, 0.6, 0)),
    node("jdr_side", 0, 0, (-8, 180, 0), 9.0, 30.0, (0, 1.0, 0), (0, 1.0, 0)),
    # win pose (the owner's screenshot of the original, and ZCUC-PwLXxA 5:17-5:22, X9xSMXBpsQw
    # 6:16-6:20): static while the appeal animation plays; the winner faces the camera, right of
    # centre (about 70 % across), framed from the top of the head to the upper thighs, the camera a
    # little below chest height. Checked by rendering the statues' own win-appeal clip.
    node("jdr_result", SPINE2, NECK, (5, 16, 0), 2.4, 34, (0, 0, 0), (0.5, -0.35, 0)),
    # boss intro (id 201, not mirrored, always an enemy facing -X: a negative rotate.y is the
    # watched side): low on the legs, then a tilted low full-body shot (crane up)
    node("jdr_boss_a", WAIST, WAIST, (22, -24, 0), 2.0, 34, (0, 0, 0), (0, -0.4, 0)),
    node("jdr_boss_b", WAIST, HEAD, (14, -30, 10), 3.2, 34),
    # stage finish (not mirrored, the downed enemy): on the ground beside it, watched side
    node("jdr_finish", WAIST, WAIST, (-6, -62, 0), 2.3, 36),
]

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


def step(nid, sec, inter=1.0):
    # `inter` is the share of the way the camera moves towards this step's pose on every frame
    # (ScriptCamera.SetCameraInfo 0xD6FE60: position and field of view are
    # Lerp(current, pose, inter); only the rotation is set outright, looking at the target).
    # 0 never moves the camera: Builds 33-36 sent 0 and the camera stayed at the turn view's
    # position, only turning towards the attacker. 1 cuts to the pose and follows it exactly; the
    # client's own sample sets use 0.3.
    return {"cameraNode": nid, "sec": sec, "inter": inter, "targetIDBase": 0, "targetIDLookat": 0}


def steps(shot):
    return [step("jdr_%s_%d" % (shot, k), sec, inter) for k, (_, sec, inter) in enumerate(SHOTS[shot])]


def cam_set(cid, data, begin=0.0, end=0.3):
    return {"cameraID": cid, "beginInter": begin, "endInter": end, "flagMaintain": False,
            "flagBindBase": True, "flagBindLookat": True, "flagNowConnect": False, "flagWorldSet": False,
            "flagForcusAhead": False, "flagNotUpdateBonePosition": False, "flagNotInter": False, "dataList": data}


sets, used = [], collections.Counter()
for cid in sorted(set(old_ids) | set(roles)):
    if cid in (101007, 101008):
        # mostly requested by skills and an attack wrap-up (15 and 5 requests against 4 and 2 from win
        # appeals): their measured action shots
        shot = OVERRIDE[cid]
        used[shot] += 1
        sets.append(cam_set(cid, steps(shot)))
    elif cid in RESULT:
        data = [step("jdr_result", 5.0)]
        used["result"] += 1
        sets.append(cam_set(cid, data))
        # still while the appeal animation plays: the bones' positions are taken on the first frame
        # (playStartPosBase/Lookat, ScrptCameraUpdate 0xD6FAC8) instead of followed every frame
        sets[-1]["flagBindBase"] = sets[-1]["flagBindLookat"] = False
    elif cid in BOSS_INTRO:
        data = [step("jdr_boss_a", 1.4), step("jdr_boss_a_to_b", 0.8)]
        used["boss"] += 1
        sets.append(cam_set(cid, data))
    elif cid in FINISH:
        data = [step("jdr_finish", 1.5)]
        used["finish"] += 1
        sets.append(cam_set(cid, data))
    elif cid in roles and a.calibrate:
        # one shot for every action: 4.5 m from the unit's chest, 50 degrees above, 40 degrees to
        # the right of its front (rotate.y 40), looking at its chest, 45-degree field; held 2.5 s
        used["calibrate"] += 1
        sets.append(cam_set(cid, [step("jdr_calib", 2.5)]))
    elif cid in roles:
        rc = roles[cid]
        if cid in OVERRIDE:
            shot = OVERRIDE[cid]
        elif not mirrored(cid):
            shot = "behind"  # the same for either team
        else:
            top = rc.most_common(1)[0][0]
            kc = kinds[cid]
            # an id any SP requests gets the SP grammar when SPs are at least a third of its users
            k = "sp" if kc["sp"] * 3 >= sum(kc.values()) > 0 else (kc.most_common(1)[0][0] if kc else "skill")
            if 140000 <= cid < 150000 and k == "sp" and top in ("hit", "out"):
                shot = pick(["TOPDOWN", "HIGH_3Q", "HIGH_BEHIND_PARTY"], cid)  # big area moves
            else:
                shot = pick(KIND_SHOTS[k][top], cid)
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
nodes = []
for name, st in SHOTS.items():
    if not used[name]:
        continue
    prev = None
    for k, (pose, _, _) in enumerate(st):
        cur = dict(pose, id="jdr_%s_%d" % (name, k))
        nodes.append(cur if prev is None else delta(cur["id"], prev, cur))
        prev = cur
nodes += [n for n in EXTRA_NODES if n["id"] not in ("jdr_boss_b",)]
if a.calibrate:
    nodes.append(node("jdr_calib", SPINE2, SPINE2, (-50, 40, 0), 4.5, 45))
nodes += [delta("jdr_boss_a_to_b", by_id["jdr_boss_a"], by_id["jdr_boss_b"])]
old_nodes = json.load(open(node_path))
old_nodes["about"] = ("Camera nodes (tools/content/build_camera_sets.py, docs/research/31-battle-camera.md): the shots of the "
                      "original battle footage, framed on the acting unit's bones (CharacterBoneList.Bone WAIST 1, SPINE2 4, "
                      "NECK 5, HEAD 6), measured on every tactical action of the original footage: glides for normal attacks, cuts "
                      "for skills and SPs (jdr_<shot>_0 is a pose, jdr_<shot>_<k> the delta to its next step: the client adds every "
                      "later step's node to the first); the win, boss-intro "
                      "and finish shots; ski_cam_00 (the client's test node) and jdr_side (the old world-fixed side view). "
                      "Values are ours.")
old_nodes["data"]["listData"] = nodes
old_sets["about"] = ("Script camera sets for every camera id the client hard-codes and every id the statues' tactical action "
                     "sets request (tools/content/build_camera_sets.py, docs/research/31-battle-camera.md): each requested id "
                     "plays the shot measured for it in the original footage, or one chosen by the kind of action and the role of "
                     "the states that request it with the measured frequencies; normal attacks glide (inter 0.09), skills and SPs "
                     "cut (inter 1.0); win shots (301-314, 101001-101028) a still facing shot; the boss intro (201) a "
                     "crane up to a tilted full-body shot; stage finishes (100300-100311, 350/351) a ground-level shot. "
                     "flagMaintain stays false so each shot ends on its own.")
old_sets["data"]["listData"] = sets
for path, doc in ((node_path, old_nodes), (set_path, old_sets)):
    json.dump(doc, open(path, "w"), ensure_ascii=False, indent=1)
    open(path, "a").write("\n")
print(len(sets), "sets,", len(nodes), "nodes;", dict(used))
