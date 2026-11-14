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
a = ap.parse_args()
keys = set()
for line in open(a.export, encoding="utf-8"):
    k = line.split("\t", 1)[0].split(":")[-1]
    keys.add(k)
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
    rows.append({
        "skillId": f"SKILL_{x}", "skillName": k,
        "skillProfileText": f"SKILL_TEXT_{x}" if f"SKILL_TEXT_{x}" in keys else "",
        "skillType": kind, "bottunType": 1, "coolDownTime": cool, "limitCount": uses,
        "skillRateLv01": 100, "skillRateLv10": 150, "pvpSkillRateLv01": 100, "pvpSkillRateLv10": 150,
        "skillParamList": [],
    })
doc = {
    "asset": "skill_data_table",
    "about": "Classic (action-mode) skill rows: one per SKILL_NAME_<X> label key in the player's files. The tactical battle does not read this table; classic skill tabs, the dictionary and the old action battle do. Name and text are label keys; numbers are ours.",
    "sources": [
        {"source": "jdr-research", "doc": "docs/research/12-master-data-fill.md §3", "note": "ids and label keys from the player's text tables (SKILL_NAME_/SKILL_TEXT_ keys)"},
        {"source": "jdr-default", "note": "skill type by number (99 attack, odd normal, even special), cool-downs, uses and rates"},
    ],
    "data": {"paramList": rows},
}
json.dump(doc, open(a.out, "w"), indent=1)
open(a.out, "a").write("\n")
print(len(rows), "skill rows")
