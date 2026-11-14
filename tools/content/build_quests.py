#!/usr/bin/env python3
"""Build server/content/quests.json: story chapters and stages (identifiers from the player's own
files via scan_quests.py) with numbers from the community wikis where available.

Until the Gamerch stage tables are merged, stages carry marked defaults (stamina, rank EXP, gold,
enemies from the same JoJo part, a battle-ready map). Each stage cites its source.

usage: python3 -I tools/content/build_quests.py --scan local/content/quests_scan.json \
          --statues server/content/statues.json [--gamerch local/wiki-raw/gamerch/stages.json] \
          --out server/content/quests.json
"""
import argparse
import datetime
import json

ap = argparse.ArgumentParser()
ap.add_argument("--scan", required=True)
ap.add_argument("--statues", required=True)
ap.add_argument("--gamerch")
ap.add_argument("--out", required=True)
a = ap.parse_args()
scan = json.load(open(a.scan))
statues = json.load(open(a.statues))["statues"]
wiki = json.load(open(a.gamerch)) if a.gamerch else {}
battle_maps = [m["mapId"] for m in scan["maps"] if m["battle"] and not m["mapId"].startswith("map00_clan")]
dt_maps = [m["mapId"] for m in scan["maps"] if m["dt"] and not m["mapId"].startswith("map00_clan")]
by_part = {}
for s in statues:
    by_part.setdefault(s["part"], []).append(s["statueId"])


def part_of_chapter(stage_type, num):
    # Story chapters follow the JoJo parts in order; without a source, spread them evenly (default).
    return 1 + min(4, num * 5 // (31 if stage_type == 0 else 481))


chapters = []
for c in scan["battle"] + scan["dt"]:
    if not c["stages"]:
        continue
    st = c["stageType"]
    part = part_of_chapter(st, c["num"])
    pool = by_part.get(part) or by_part.get(1)
    maps = battle_maps if st == 0 else dt_maps
    stages = []
    for i, s in enumerate(c["stages"]):
        w = wiki.get(s["stageId"])
        n = c["num"] * 10 + i
        stage = {
            "stageId": s["stageId"],
            "areas": s.get("areas") or [],
            "mapId": maps[n % len(maps)],
            "useStamina": 5 + c["num"] // 2,
            "gainRankExp": 10 + c["num"] * 5,
            "gold": 200 + c["num"] * 100,
            "enemyLevel": 1 + c["num"] * 3,
            "enemies": [pool[(n * 3 + k) % len(pool)] for k in range(3)],
            "source": {"source": "jdr-default", "note": "no wiki data merged for this stage yet"},
        }
        if w:
            stage.update({k: w[k] for k in ("useStamina", "gainRankExp", "gold", "enemyLevel") if k in w})
            stage["source"] = w["source"]
        stages.append(stage)
    chapters.append({"chapterId": c["chapterId"], "num": c["num"], "stageType": st,
                     "chapterMap": c["chapterMap"], "stages": stages})

doc = {
    "about": "Story chapters and stages: identifiers from the player's own files (scan_quests.py); numbers from community wikis where merged, else marked defaults. See docs/CONTENT.md.",
    "sources": [{"source": "jdr-default", "note": "identifiers from the player's files; defaults where no wiki data",
                 "accessed": datetime.date.today().isoformat()}],
    "chapters": chapters,
}
json.dump(doc, open(a.out, "w"), indent=1)
print(f"{len(chapters)} chapters, {sum(len(c['stages']) for c in chapters)} stages, "
      f"{sum(1 for c in chapters for s in c['stages'] if s['source']['source'] != 'jdr-default')} with wiki numbers")
