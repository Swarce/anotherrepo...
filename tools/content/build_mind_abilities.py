#!/usr/bin/env python3
"""Add the Mind abilities to server/content/masterdata/dtbattle_ability_data_table.json
(docs/research/16-minds.md §6).

In 3.12.0 a Mind changes a statue only through its ability row: the client never reads the
Mind's HP/ATK/DEF. No Mind ability numbers survive (only 124 of the 417 Minds even have a name
label), so the rows are ours: 12 shared rows `DT_MIND_<HP|ATK|DEF>_<rarity>`, a stat rate that
grows with rarity, from Lv1 to twice that at Lv10. The server gives each Mind the row of the
stat its wiki stats lean to (collection::mind_ability).

Idempotent: replaces any DT_MIND_* rows and keeps every other row as it is.

usage: python3 -I tools/content/build_mind_abilities.py [--table <dtbattle_ability_data_table.json>]
"""
import argparse, json

# Lv1 per-mille by rarity (ours); Lv10 is twice that.
RATE = {1: 20, 2: 30, 3: 50, 4: 80}
STATS = {"HP": "DTAF_HP_RATEADD", "ATK": "DTAF_ATK_RATEADD", "DEF": "DTAF_DEF_RATEADD"}

ap = argparse.ArgumentParser()
ap.add_argument("--table", default="server/content/masterdata/dtbattle_ability_data_table.json")
a = ap.parse_args()

doc = json.load(open(a.table))
rows = [r for r in doc["data"]["paramList"] if not r["abilityId"].startswith("DT_MIND_")]
for stat, affect in STATS.items():
    for rarity, lv1 in RATE.items():
        # Slot 0 is the per-mille amount; key order as in the leader rows.
        p = {"affectId": affect, "giveLimit": -1,
             **{f"paramLv1_{i}": lv1 if i == 0 else 0 for i in range(8)},
             **{f"paramLv10_{i}": 2 * lv1 if i == 0 else 0 for i in range(8)}}
        rows.append({"abilityId": f"DT_MIND_{stat}_{rarity}", "abilityName": "", "abilityParamList": [p]})
doc["data"]["paramList"] = rows

note = ("Mind abilities DT_MIND_<HP|ATK|DEF>_<rarity> are ours (tools/content/build_mind_abilities.py): "
        "Lv1 2/3/5/8% by rarity 1-4, doubled at Lv10")
if not any(s.get("note") == note for s in doc["sources"]):
    doc["sources"] = [s for s in doc["sources"] if "DT_MIND" not in s.get("note", "")]
    doc["sources"].append({"source": "jdr-default", "note": note})
if "DT_MIND" not in doc["about"]:
    doc["about"] += (" Rows DT_MIND_<stat>_<rarity> are the Mind abilities (ours; 3.12 Minds act only "
                     "through ability rows).")
json.dump(doc, open(a.table, "w"), ensure_ascii=False, indent=1)
open(a.table, "a").write("\n")
print(len(rows), "rows,", sum(r["abilityId"].startswith("DT_MIND_") for r in rows), "Mind rows")
