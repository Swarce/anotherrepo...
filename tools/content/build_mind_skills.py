#!/usr/bin/env python3
"""Build server/content/mind_skills.json: skill Minds and time-stop skills (docs/research/26).

Skill Minds (mindType 2). The game's own Mind description label MIND_TEXT_<mindId> says which
skill a Mind grants and whether it is an SP (special) skill or a normal one; the skill's name is
the label SKILL_NAME_<p>_<NN> with the same text. So, for every Mind of server/content/minds.json:
  - its description names one skill by quotation; the skill keys whose name label has exactly that
    text are the candidates (all comparisons are local; only keys are written);
  - a candidate is usable when some roster statue of prefix <p> has the action set SKILL_<NN> on its
    model (statues_scan.json), so the unit can play it; those statues are the Mind's
    limitCharaList (the client compares statueIds);
  - the candidate kept is the one that reuses a statue skill row of the same kind, else the one with
    the most statues, else the lowest key;
  - the skill row: the statue's own row when one exists with the same kind (normal/special); else a
    new row. Its id is SKILL_<key>, or MSKILL_<key> when a statue row of the other kind already uses
    SKILL_<key> (the id is free-form for the client; the action set comes from viewSkillId).
    Classification copies the statue row when the skill is one; otherwise our English reading of the
    skill text: every such text carries the damage-rank tag, so they are hits (element and
    single/all from the wording); a skill without the tag is skipped and the Mind stays an ability
    Mind.
Time stop. A skill stops time when its text says the user acts in stopped time (English: "stopped
time", "only the user can move"). Those get the client's own category DTSC_WORLD_<n> as their sub
category (ProcessType 4 WORLD, turn n): LIGHT (1 turn) for normal skills, MIDDLE (2 turns) for
special ones (the turn counts are the labels'; which skill gets which is ours).

Build 44. The quoted name is read up to the closing bracket before the line break, so a skill name
that is itself in brackets (4036's 「「世界(ザ・ワールド)！」」) resolves. --research-b44 (each
research-b44 family file; only topic links_traits_minds, field skill_mind, high/medium records are
read): in-game Mind detail screens (the ameblo blog, read as data). For a Mind we serve, the record is
cited in the Mind's "source"; its Lv1 damage rank replaces ours on a new skill row (a statue's own
row is not changed, and rank 0 is not sent: our rows are hits and the client reads rank 0 as the
damage-rank table's last entry; a difference is reported), and a '時止め N ターン' line gives the time-stop
category whose label names N turns (DTSC_WORLD_*). Records of Minds not in minds.json are counted
and not used.

usage: python3 -I tools/content/build_mind_skills.py --export local/l10n/work23b.tsv \
         --scan local/content/statues_scan.json --statues server/content/statues.json \
         --minds server/content/minds.json \
         [--research-b44 local/wiki-raw/research-b44/<family>/results.jsonl ...] \
         --out server/content/mind_skills.json
"""
import argparse
import collections
import json
import os
import re

ap = argparse.ArgumentParser()
for x in ("--export", "--scan", "--statues", "--minds", "--out"):
    ap.add_argument(x, required=True)
ap.add_argument("--research-b44", action="append", default=[])
a = ap.parse_args()

jp, en = {}, {}
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    if len(c) >= 4 and c[0].startswith("text:"):
        key = c[0].split(":")[-1]
        jp[key], en[key] = c[3], c[2]

roster = json.load(open(a.statues))["statues"]
roster_ids = {s["statueId"] for s in roster}
statue_skill = {s["id"]: s for d in roster for s in d.get("dtSkills", [])}
actions = {s["statueId"]: set(s.get("dtActions") or []) for s in json.load(open(a.scan))["statues"]}
mind_ids = [m["mindId"] for m in json.load(open(a.minds))["minds"]]

by_name = collections.defaultdict(list)
for k, v in jp.items():
    if re.fullmatch(r"SKILL_NAME_\d{3}_\d{2}", k):
        by_name[v].append(k[len("SKILL_NAME_"):])
dtsc = {k for k in jp if k.startswith("DTSC_")}

# Our English reading for skills that are no statue's skill (they all carry the damage-rank tag).
ELEMENTS = [("Stand", "STAND_PHYSICAL", "STAND_PHY"), ("Hamon", "HAMON", "HAMON"), ("UV", "UV", "UV"),
            ("vampir", "VMP", "VMP"), ("flame", "FLAME", "FIRE"), ("fire", "FLAME", "FIRE"),
            ("electr", "ELECT", "ELECTOCAL"), ("ice", "ICE", "PHYSICAL"), ("bomb", "BOMB", "PHYSICAL"),
            ("explo", "BOMB", "PHYSICAL")]
ALL_WORDS = ("enemy team", "all enemies", "wide area", "around")


def english_hit(text):
    if "[DR{damagerank}]" not in text:
        return None
    body = text.split("\\n")[0]
    elem, attr = "PHYSICAL", "PHYSICAL"
    for word, e, at in ELEMENTS:
        if re.search(r"\b" + word, body, re.I):
            elem, attr = e, at
            break
    allt = any(w in body.lower() for w in ALL_WORDS)
    cat = "DTSC_" + elem + ("_ALL" if allt else "")
    if cat not in dtsc:
        cat = "DTSC_PHYSICAL" + ("_ALL" if allt else "")
    return cat, 2 if allt else 1, attr


BUTTON = {1: (7, 38), 2: (83, 90)}  # single / all-target hits (as build_statues.py)
# "(SP)skill<open quote>name<close quote>" in the Mind description (written escaped), up to the close
# quote before the escaped line break or the particle を: the name may itself be in brackets.
QUOTE = re.compile("(SP)?\u30b9\u30ad\u30eb\u300c(.+?)\u300d(?=\\\\n|\u3092)")


def record(key, special):
    sid = f"SKILL_{key}"
    row = statue_skill.get(sid)
    if row and row["special"] == special:
        return dict(row), True
    if row:
        rec = dict(row)
        rec["id"] = f"MSKILL_{key}"
        if rec["processType"] == 1:
            rec["button"] = BUTTON[rec["targetType"]][1 if special else 0]
    else:
        hit = english_hit(en.get(f"DTbattle_SKILL_TEXT_{key}", ""))
        if not hit:
            return None, False
        cat, target, attr = hit
        rec = {"id": sid, "name": f"SKILL_NAME_{key}" if f"SKILL_NAME_{key}" in jp else "",
               "text": f"DTbattle_SKILL_TEXT_{key}", "button": BUTTON[target][1 if special else 0],
               "category": cat, "targetType": target, "processType": 1, "attr": attr}
    rec["special"] = special
    rec["damageRank"] = 150 if special else 125
    return rec, False


minds = []
skipped = collections.Counter()
for mid in mind_ids:
    m = QUOTE.search(jp.get(f"MIND_TEXT_{mid}", ""))
    if not m:
        continue
    special = bool(m.group(1))
    cands = []
    for key in by_name.get(m.group(2), []):
        p, nn = key.split("_")
        users = sorted(sid for sid in roster_ids if sid.startswith(p) and f"SKILL_{nn}" in actions.get(sid, ()))
        if not users:
            continue
        rec, reused = record(key, special)
        if rec:
            cands.append((not reused, -len(users), key, rec, users))
    if not cands:
        skipped["no usable skill"] += 1
        continue
    _, _, key, rec, users = min(cands, key=lambda c: c[:3])
    minds.append({"mindId": mid, "skill": rec, "statues": users})

time_stop = {}
for rec in list(statue_skill.values()) + [m["skill"] for m in minds]:
    key = rec["text"][len("DTbattle_SKILL_TEXT_"):]
    t = en.get(f"DTbattle_SKILL_TEXT_{key}", "").lower()
    if "stopped time" in t or "only the user can move" in t:
        time_stop[rec["id"]] = "DTSC_WORLD_MIDDLE" if rec["special"] else "DTSC_WORLD_LIGHT"

# Build 44: in-game Mind detail screens (research-b44, see the module docstring).
B44_SOURCES = (("https://ameblo.jp/", "鎖国的ブログ (ameblo; in-game Mind detail screenshots, read as data)"),)
WORLD_BY_TURNS = {int(m.group(1)): k for k in sorted(dtsc) if k.startswith("DTSC_WORLD_")
                  for m in [re.search("\\[([0-9]+)\u30bf\u30fc\u30f3\\]", jp[k])] if m}
by_mind = {m["mindId"]: m for m in minds}
b44_used, b44_other, b44_report = 0, 0, []
for path in a.research_b44:
    fam = os.path.basename(os.path.dirname(path))
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if r.get("topic") != "links_traits_minds" or r.get("field") != "skill_mind" \
                or r.get("confidence") not in ("high", "medium"):
            continue
        v = r["value"]
        m = by_mind.get(v.get("mindId"))
        if m is None:
            b44_other += 1
            continue
        assert "source" not in m, v["mindId"]
        rec, applied, noted = m["skill"], [], []
        assert v.get("skillLabel") in (None, "SKILL_" + rec["text"][len("DTbattle_SKILL_TEXT_"):]), v
        dr = v.get("damageRank_lv1")
        if dr is not None and dr != rec["damageRank"]:
            if dr == 0:
                # our row is a hit (processType 1), and the damage-rank table reads rank 0 as its last
                # entry (masterdata/dtbattle_damage_rank_table.json), so "no damage" cannot be sent this way
                noted.append("damage rank 0 (kept ours)")
                b44_report.append(f"{v['mindId']}: damage rank 0 against our {rec['damageRank']} (kept: rank 0 "
                                  "would read the last damage-rank entry)")
            elif rec["id"] in statue_skill:
                noted.append(f"damage rank {dr} (kept the statue row's)")
                b44_report.append(f"{v['mindId']}: damage rank {dr} against the statue row's {rec['damageRank']} (kept)")
            else:
                rec["damageRank"] = dr
                applied.append("damage rank (Lv1)")
        for e in v.get("effect") or []:
            tm = re.fullmatch("\u6642\u6b62\u3081\\s*([0-9]+)\\s*\u30bf\u30fc\u30f3", e.strip())
            if not tm:
                continue
            cat = WORLD_BY_TURNS.get(int(tm.group(1)))
            if rec["id"] not in time_stop or cat is None:
                noted.append(f"time stop {tm.group(1)} turns (not a time-stop skill here)")
                b44_report.append(f"{v['mindId']}: time stop {tm.group(1)} turns, not a time-stop skill here (kept)")
            elif time_stop[rec["id"]] != cat:
                time_stop[rec["id"]] = cat
                applied.append(f"time stop {tm.group(1)} turns")
        src = next(n for host, n in B44_SOURCES if r["url"].startswith(host))
        m["source"] = {"source": src, "page": r["url"], **({"capture": r["capture"]} if r.get("capture") else {}),
                       "accessed": r["accessed"], "confidence": r["confidence"],
                       "note": f"research-b44/{fam} skill_mind: "
                               + ", ".join(applied + noted or ["agrees with ours"])}
        b44_used += 1

doc = {
    "about": "Skill Minds (mindType 2) and time-stop skills, from the game's own labels (MIND_TEXT_<id> names the skill; "
             "SKILL_NAME_<p>_<NN> carries it) and action sets. See tools/content/build_mind_skills.py and docs/research/26.",
    "sources": [
        {"source": "jdr-research", "doc": "docs/research/26-links-and-skills.md",
         "note": "label keys (MIND_TEXT, SKILL_NAME, DTbattle_SKILL_TEXT, DTSC_WORLD_*) and statue action sets from the player's files"},
        {"source": "jdr-default", "note": "damage ranks, buttons and the classification of skills that are no statue's skill are ours; "
                                          "time-stop turns: 1 for normal skills, 2 for special ones (ours)"
                                          + ("; except where a Mind's source (research-b44) gives them" if b44_used else "")},
    ],
    "minds": minds,
    "timeStop": dict(sorted(time_stop.items())),
}
json.dump(doc, open(a.out, "w"), indent=1, ensure_ascii=False)
print(f"{len(minds)} skill Minds ({sum(1 for m in minds if m['skill']['special'])} SP), "
      f"{sum(1 for m in minds if m['skill']['id'] not in statue_skill)} new skill rows, {len(time_stop)} time-stop skills; "
      f"skipped {dict(skipped)}; research-b44: {b44_used} Minds read, {b44_other} not in minds.json")
for x in b44_report:
    print("research-b44 not applied:", x)
