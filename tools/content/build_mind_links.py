#!/usr/bin/env python3
"""Build server/content/mind_links.json and the Mind link ability rows (docs/research/26 §2).

Which Minds link: the Gamerch Mind pages list a "Mind link" for a few Minds, a bonus for 2, 3 and 4
equipped Minds of the same link (e.g. "x2 ATK+75 / x3 ATK+125 / x4 ATK+200"). Only the stat of each
Mind's link is used (ATK, DEF or HP); links whose effect has no tactical affect (move speed,
critical rate) are left out. The game's labels name the link abilities: ABI_MIND_LINK_NAME_001 (ATK
Up Link), _002 (DEF), _003 (HP).

Amounts are ours: the wiki's are flat action-battle numbers that differ by the Mind's rarity, while
the tactical text and battle use our rate affects. Rows DT_MLINK_<STAT>_<n> (n = 2..4 linked Minds):
+3 / 5 / 8 % (per mille 30 / 50 / 80), the same at Lv1 and Lv10.

Idempotent: replaces any DT_MLINK_* rows of the ability table and keeps every other row.

usage: python3 -I tools/content/build_mind_links.py --gamerch local/wiki-raw/gamerch/minds.json \
         --minds server/content/minds.json --out server/content/mind_links.json \
         [--table server/content/masterdata/dtbattle_ability_data_table.json]
"""
import argparse
import json
import re

ap = argparse.ArgumentParser()
ap.add_argument("--gamerch", required=True)
ap.add_argument("--minds", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--table", default="server/content/masterdata/dtbattle_ability_data_table.json")
a = ap.parse_args()

LINKS = {"ATK": "001", "DEF": "002", "HP": "003"}
AMOUNT = {2: 30, 3: 50, 4: 80}
ours = {m["mindId"] for m in json.load(open(a.minds))["minds"]}

minds = {}
sources = {}
for g in json.load(open(a.gamerch)):
    mid = g.get("mind_id")
    if not mid or mid not in ours or not g.get("mind_link"):
        continue
    stats = {m.group(1) for line in g["mind_link"] for m in [re.search(r"\b(ATK|DEF|HP)\+\d+", line)] if m}
    if len(stats) != 1:
        continue
    stat = stats.pop()
    minds[mid] = f"MIND_LINK_{LINKS[stat]}"
    sources[mid] = g["source"].get("wayback_url") or g["source"].get("page_url")


def ability_id(stat, n):
    return f"DT_MLINK_{stat}_{n}"


links = [{"mindLinkId": f"MIND_LINK_{num}", "stat": stat, "name": f"ABI_MIND_LINK_NAME_{num}",
          "abilities": {str(n): ability_id(stat, n) for n in AMOUNT}}
         for stat, num in LINKS.items()]
doc = {
    "about": "Mind links: which Minds link (the Gamerch Mind pages) and the link abilities by number of linked Minds. "
             "See tools/content/build_mind_links.py and docs/research/26-links-and-skills.md.",
    "sources": [
        {"source": "Gamerch JoJo DR wiki (Wayback Machine snapshot)", "page": "https://jojodr.gamerch.com/",
         "accessed": "2026-11-15", "note": "the stat of each listed Mind link; per-Mind pages in local/wiki-raw/gamerch/minds.json"},
        {"source": "jdr-default", "note": "link ids MIND_LINK_<nnn> and ability rows DT_MLINK_<stat>_<n> are ours, named by the game's "
                                          "ABI_MIND_LINK_NAME_<nnn> labels; amounts +3/5/8% for 2/3/4 Minds are ours"},
    ],
    "links": links,
    "minds": dict(sorted(minds.items())),
}
json.dump(doc, open(a.out, "w"), indent=1, ensure_ascii=False)

table = json.load(open(a.table))
rows = [r for r in table["data"]["paramList"] if not r["abilityId"].startswith("DT_MLINK_")]
for stat, num in LINKS.items():
    for n, permille in AMOUNT.items():
        p = {"affectId": f"DTAF_{stat}_RATEADD", "giveLimit": -1}
        for lv in ("1", "10"):
            for i in range(8):
                p[f"paramLv{lv}_{i}"] = permille if i == 0 else 0
        rows.append({"abilityId": ability_id(stat, n), "abilityName": f"ABI_MIND_LINK_NAME_{num}", "abilityParamList": [p]})
table["data"]["paramList"] = rows
note = ("Mind link abilities DT_MLINK_<stat>_<n> (tools/content/build_mind_links.py): named by the game's "
        "ABI_MIND_LINK_NAME_<nnn> labels; +3/5/8% for 2/3/4 linked Minds (ours); Lv10 = Lv1")
table["sources"] = [s for s in table["sources"] if "DT_MLINK" not in s.get("note", "")]
table["sources"].append({"source": "jdr-default", "note": note})
if "DT_MLINK" not in table["about"]:
    table["about"] += " Rows DT_MLINK_<stat>_<n> are the Mind link abilities (docs/research/26-links-and-skills.md)."
with open(a.table, "w") as f:
    json.dump(table, f, indent=1, ensure_ascii=False)
    f.write("\n")
print(f"{len(minds)} linking Minds: {sorted(minds.items())}")
