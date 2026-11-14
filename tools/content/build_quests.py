#!/usr/bin/env python3
"""Build server/content/quests.json: story chapters and stages (identifiers from the player's own
files via scan_quests.py) with numbers from the community wikis where available.

Gamerch chapters attach by title: the wiki's chapter title (without its "Chapter N" prefix) is
compared after NFKC normalisation with the game's own CHAPTER_NAME_<id> label, and a chapter is used
only when the wiki lists exactly as many stages as the game has (stage k -> the chapter's k-th stage).
Fields the wiki leaves blank keep marked defaults and are named in the stage's `defaults` list; an
enemy level under half of the previous stage's is treated as a wiki typo and replaced the same way.

Defaults (stamina, rank EXP, gold, enemy level, enemies from the same JoJo part, a battle-ready map)
grow with the chapter's position among chapters of its type and stay inside the game's limits
(stamina <= 20, the rank-1 AP; level <= 99).

usage: python3 -I tools/content/build_quests.py --scan local/content/quests_scan.json \
          --statues server/content/statues.json --export local/l10n/work.tsv \
          [--gamerch local/wiki-raw/gamerch/stages.json] --out server/content/quests.json
"""
import argparse
import datetime
import json
import re
import unicodedata

ap = argparse.ArgumentParser()
ap.add_argument("--scan", required=True)
ap.add_argument("--statues", required=True)
ap.add_argument("--gamerch")
ap.add_argument("--export", help="the player's l10n export (CHAPTER_NAME labels), needed with --gamerch")
ap.add_argument("--out", required=True)
a = ap.parse_args()
scan = json.load(open(a.scan))
statues = json.load(open(a.statues))["statues"]


def norm(x):
    return unicodedata.normalize("NFKC", x or "").replace(" ", "").replace("\u3000", "")


wiki = {}  # game stageId -> numbers from the wiki
unmatched = []
if a.gamerch:
    if not a.export:
        ap.error("--gamerch needs --export")
    title_to_chapter = {}
    for line in open(a.export, encoding="utf-8"):
        c = line.rstrip("\n").split("\t")
        m = re.fullmatch(r"text:ChapterName_JP:CHAPTER_NAME_(ST_\d+)", c[0]) if len(c) >= 4 else None
        if m:
            title_to_chapter.setdefault(norm(c[3]), set()).add("CHAPTER_" + m.group(1))
    game_stages = {c["chapterId"]: [s["stageId"] for s in c["stages"]] for c in scan["battle"]}
    by_title = {}
    for w in json.load(open(a.gamerch)).get("stages", []):
        by_title.setdefault(w["chapter_title_jp"], []).append(w)
    for title, ws in by_title.items():
        ids = title_to_chapter.get(norm(re.sub(r"^\s*第\s*[0-9０-９]+\s*章", "", title)), set())
        ws.sort(key=lambda w: w["stage_no"])
        if len(ids) != 1 or len(game_stages.get(next(iter(ids)), [])) != len(ws):
            unmatched.append(title)
            continue
        for w, stage_id in zip(ws, game_stages[next(iter(ids))]):
            if w.get("ap") is None:
                continue
            rec = {"useStamina": w["ap"], "source": {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)",
                   "page": w["source"]["page_url"], "capture": w["source"].get("capture"),
                   "accessed": w["source"].get("accessed")}}
            for field, key in (("gainRankExp", "exp"), ("gold", "gold"), ("enemyLevel", "recommended_level")):
                if w.get(key) is not None:
                    rec[field] = w[key]
            wiki[stage_id] = rec
battle_maps = [m["mapId"] for m in scan["maps"] if m["battle"] and not m["mapId"].startswith("map00_clan")]
dt_maps = [m["mapId"] for m in scan["maps"] if m["dt"] and not m["mapId"].startswith("map00_clan")]
by_part = {}
for s in statues:
    by_part.setdefault(s["part"], []).append(s["statueId"])


def part_of_chapter(pos, count):
    # Story chapters follow the JoJo parts in order; without a source, spread them evenly (default).
    return 1 + min(4, pos * 5 // max(1, count))


FIELDS = ("useStamina", "gainRankExp", "gold", "enemyLevel")
chapters = []
ordered = []  # (position among chapters of its type, how many of that type, chapter)
for kind in (scan["battle"], scan["dt"]):
    kind = [c for c in kind if c["stages"]]
    ordered += [(pos, len(kind), c) for pos, c in enumerate(kind)]
for pos, count, c in ordered:
    st = c["stageType"]
    part = part_of_chapter(pos, count)
    pool = by_part.get(part) or by_part.get(1)
    maps = battle_maps if st == 0 else dt_maps
    stages = []
    prev_level = None
    for i, s in enumerate(c["stages"]):
        w = wiki.get(s["stageId"])
        n = pos * 10 + i
        stage = {
            "stageId": s["stageId"],
            "areas": s.get("areas") or [],
            "mapId": maps[n % len(maps)],
            "useStamina": min(20, 5 + pos // 3),
            "gainRankExp": 10 + pos * 20,
            "gold": 250 + pos * 150,
            "enemyLevel": min(99, 1 + pos * 2 + i),
            "enemies": [pool[(n * 3 + k) % len(pool)] for k in range(3)],
            "source": {"source": "jdr-default", "note": "no wiki data merged for this stage yet"},
        }
        if w:
            if "enemyLevel" in w and prev_level and w["enemyLevel"] * 2 < prev_level:
                w = {k: v for k, v in w.items() if k != "enemyLevel"}
                w.setdefault("note", "wiki recommended level looks like a typo (under half the previous stage's); previous stage's used")
                stage["enemyLevel"] = prev_level
            stage.update({k: w[k] for k in FIELDS if k in w})
            stage["source"] = dict(w["source"], **({"note": w["note"]} if "note" in w else {}))
            missing = [k for k in FIELDS if k not in w]
            if missing:
                stage["defaults"] = missing
            stage["enemyLevel"] = min(99, stage["enemyLevel"])
        prev_level = stage["enemyLevel"]
        stages.append(stage)
    chapters.append({"chapterId": c["chapterId"], "num": c["num"], "stageType": st,
                     "chapterMap": c["chapterMap"], "stages": stages})

doc = {
    "about": "Story chapters and stages: identifiers from the player's own files (scan_quests.py); numbers from community wikis where merged, else marked defaults. See docs/CONTENT.md.",
    "sources": [{"source": "jdr-default", "note": "identifiers from the player's files; defaults where no wiki data",
                 "accessed": datetime.date.today().isoformat()}]
               + ([{"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": "https://jojodr.gamerch.com/",
                    "accessed": datetime.date.today().isoformat()}] if a.gamerch else []),
    "chapters": chapters,
}
json.dump(doc, open(a.out, "w"), indent=1)
print(f"Gamerch chapters not matched (title or stage count): {unmatched}")
print(f"{len(chapters)} chapters, {sum(len(c['stages']) for c in chapters)} stages, "
      f"{sum(1 for c in chapters for s in c['stages'] if s['source']['source'] != 'jdr-default')} with wiki numbers")
