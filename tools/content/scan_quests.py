#!/usr/bin/env python3
"""Inventory the story quests present in the player's own game files: chapters and stages (from the
text labels) with the area scene bundles and chapter-map bundles that exist, and the maps.
Identifiers only.

usage: python3 -I tools/content/scan_quests.py --data <files dir> --keys local/keys.json \
          --tool target/release/jdr-tool --export local/l10n/work.tsv --out local/content/quests_scan.json
"""
import argparse
import collections
import json
import os
import re
import subprocess

ap = argparse.ArgumentParser()
for a in ("--data", "--keys", "--tool", "--export", "--out"):
    ap.add_argument(a, required=True)
a = ap.parse_args()

names = set()
out = subprocess.run([a.tool, "names", "--data", a.data, "--keys", a.keys], check=True, capture_output=True, text=True).stdout
for line in out.splitlines():
    names.add(line.split("\t")[0])
labels = set()
for line in open(a.export, encoding="utf-8"):
    labels.add(line.split("\t")[0].split(":")[-1])

areas = collections.defaultdict(list)
for n in sorted(names):
    m = re.fullmatch(r"area_st_(\d\d)_(\d\d)_(\d\d)", n)
    if m:
        areas[(m.group(1), m.group(2))].append(n)

battle = []
for cc in range(0, 100):
    c = f"{cc:02d}"
    if f"CHAPTER_NAME_ST_{c}" not in labels:
        continue
    stages = []
    for ss in range(1, 30):
        s = f"{ss:02d}"
        sid = f"STAGE_ST_{c}_{s}"
        if sid in labels and areas.get((c, s)):
            stages.append({"stageId": sid, "areas": areas[(c, s)]})
    cmap = f"bizarre_chapter_map_{cc:03d}"
    battle.append({"chapterId": f"CHAPTER_ST_{c}", "num": cc, "stageType": 0,
                   "chapterMap": cc if cmap in names else None, "stages": stages})

dt = []
for cc in range(0, 1000):
    tag = f"{cc:02d}" if cc < 100 else str(cc)
    if f"CHAPTER_NAME_DT_ST_{tag}" not in labels:
        continue
    stages = [{"stageId": f"DT_STAGE_ST_{tag}_{ss:02d}"} for ss in range(1, 30)
              if f"DT_STAGE_ST_{tag}_{ss:02d}" in labels]
    cmap = f"bizarre_chapter_map_{1000 + cc}"
    dt.append({"chapterId": f"CHAPTER_DT_ST_{tag}", "num": cc, "stageType": 1,
               "chapterMap": 1000 + cc if cmap in names else None, "stages": stages})

maps = collections.defaultdict(set)
for n in names:
    m = re.fullmatch(r"(map\d\d_[a-z]+\d*)_(base|collider|collider_dt|hiru|yoru)", n)
    if m:
        maps[m.group(1)].add(m.group(2))
maplist = [{"mapId": k, "battle": {"base", "collider", "hiru"} <= v, "dt": {"base", "collider_dt", "hiru"} <= v}
           for k, v in sorted(maps.items())]
os.makedirs(os.path.dirname(a.out) or ".", exist_ok=True)
json.dump({"battle": battle, "dt": dt, "maps": maplist}, open(a.out, "w"), indent=1)
print(f"battle chapters {len(battle)} ({sum(len(c['stages']) for c in battle)} stages with areas), "
      f"DT chapters {len(dt)} ({sum(len(c['stages']) for c in dt)} stages), maps {len(maplist)} "
      f"({sum(m['battle'] for m in maplist)} battle-ready, {sum(m['dt'] for m in maplist)} DT-ready)")
