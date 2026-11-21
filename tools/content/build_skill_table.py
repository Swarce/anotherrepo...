#!/usr/bin/env python3
"""Build server/content/masterdata/skill_data_table.json: the classic (action-mode) skill rows the
client shows on classic skill tabs and in the dictionary (docs/research/12 §3). One row per
`SKILL_NAME_<X>` label key in the player's l10n export (DUMMY keys skipped): the name and text
fields are label keys (the client looks them up); type, button, cool-down and rates are ours.
Only label keys and numbers are written.

usage: python3 -I tools/content/build_skill_table.py --export local/l10n/work.tsv \
          --out server/content/masterdata/skill_data_table.json
"""
import argparse
import json
import re

ap = argparse.ArgumentParser()
ap.add_argument("--export", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--action", default="server/content/action_skills.json",
                help="action_skills.json (build_action_skills.py): which ids are skills, SP skills and finishers")
a = ap.parse_args()
keys = set()
for line in open(a.export, encoding="utf-8"):
    k = line.split("\t", 1)[0].split(":")[-1]
    keys.add(k)
# Roles from the action-battle skill lists (Build 33): a skill (type 2), an SP skill (type 3, uses)
# or a finisher (type 3); the wiki's cool-downs and uses where given, else ours (skill 5 s, SP 3 s
# with 3 uses, finisher 10 s with 2 uses: the most common values on the Gamerch pages).
role = {}
try:
    recs = list(json.load(open(a.action))["statues"].values())
    # wiki-sourced statues first: their roles win over the tactical fallbacks of other variants
    recs.sort(key=lambda r: r.get("source", {}).get("source") == "jdr-default")
    for rec in recs:
        for slot, kind, cool, uses in (("skill", 2, 5, 0), ("sp", 3, 3, 3), ("finisher", 3, 10, 2)):
            x = rec.get(slot)
            if x and x["id"] not in role:
                role[x["id"]] = (kind, x.get("cooldown", cool), x.get("uses", uses), 1 if kind == 2 else 4)
except FileNotFoundError:
    pass
rows = []
for k in sorted(keys):
    m = re.fullmatch(r"SKILL_NAME_(\d{3}_\d{2}(?:_\d{2})?)", k)
    if not m:
        continue
    x = m.group(1)
    nn = int(x.split("_")[1])
    # 99 = the basic attack (combo); odd = normal skill; even = special (our pairing, see §3).
    kind = 1 if nn == 99 else 2 if nn % 2 else 3
    cool, uses = {1: (0, 0), 2: (5, 0), 3: (10, 2)}[kind]
    button = 1
    if f"SKILL_{x}" in role:
        kind, cool, uses, button = role[f"SKILL_{x}"]
    rows.append({
        "skillId": f"SKILL_{x}", "skillName": k,
        "skillProfileText": f"SKILL_TEXT_{x}" if f"SKILL_TEXT_{x}" in keys else "",
        "skillType": kind, "bottunType": button, "coolDownTime": cool, "limitCount": uses,
        "skillRateLv01": 100, "skillRateLv10": 150, "pvpSkillRateLv01": 100, "pvpSkillRateLv10": 150,
        "skillParamList": [],
    })
doc = {
    "asset": "skill_data_table",
    "about": "Classic (action-mode) skill rows: one per SKILL_NAME_<X> label key in the player's files. The tactical battle does not read this table; classic skill tabs, the dictionary and the old action battle do. Name and text are label keys; numbers are ours.",
    "sources": [
        {"source": "jdr-research", "doc": "docs/research/12-master-data-fill.md §3", "note": "ids and label keys from the player's text tables (SKILL_NAME_/SKILL_TEXT_ keys)"},
        {"source": "jdr-research", "doc": "docs/research/30-action-battle.md", "note": "skill, SP skill and finisher roles and their cool-downs/uses from server/content/action_skills.json (Gamerch statue pages where matched; other finishers are the label at the statue's retail SP-cinematic entry, action_cinematics.json)"},
        {"source": "jdr-default", "note": "ids in no statue's action skills: type by number (99 attack, odd normal, even special); rates; cool-downs and uses where the wiki gives none"},
    ],
    "data": {"paramList": rows},
}
json.dump(doc, open(a.out, "w"), indent=1)
open(a.out, "a").write("\n")
print(len(rows), "skill rows")
