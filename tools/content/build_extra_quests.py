#!/usr/bin/env python3
"""Build server/content/quests_extra.json: daily and event chapters for the Quest Select tiles
(docs/research/13-menus.md §Daily/Event/Campaign).

Only identifiers come from the player's files: chapter and stage label keys from the game's text
(by key, our English already imported), banner bundle names from the player's file list. No
source gives the original daily/event stage layouts, so every stage reuses the map and areas of a
real tactical story stage and gets enemies, levels and rewards that are ours (`jdr-default`),
except the Easy-tier daily AP/EXP/gold, which the wiki gives (cited).

usage: python3 -I tools/content/build_extra_quests.py --names <jdr-tool names output> \
         [--gamerch local/wiki-raw/gamerch/stages.json] --out server/content/quests_extra.json
"""
import argparse, glob, hashlib, json, re

ap = argparse.ArgumentParser()
ap.add_argument("--names", required=True, help="jdr-tool names output (logical bundle names)")
ap.add_argument("--gamerch", help="local/wiki-raw/gamerch/stages.json (daily quest numbers)")
ap.add_argument("--out", required=True)
a = ap.parse_args()

labels = {}
for p in glob.glob("server/content/l10n/en/*.json"):
    for k in json.load(open(p))["entries"]:
        if k.startswith("text:"):
            labels[k.split(":", 2)[2]] = k
bundles = {l.split("\t")[0] for l in open(a.names)}
story = json.load(open("server/content/quests.json"))["chapters"]
statues = json.load(open("server/content/statues.json"))["statues"]
dt_stages = [s for c in story if c["stageType"] == 1 for s in c["stages"] if s.get("mapId")]
assert dt_stages


def h(*parts):
    return int(hashlib.sha256("/".join(map(str, parts)).encode()).hexdigest()[:12], 16)


def enemies(seed, stype=None, n=3):
    pool = [s["statueId"] for s in statues if stype is None or s.get("statueType") == stype] or [s["statueId"] for s in statues]
    pool.sort()
    return [pool[h(seed, i) % len(pool)] for i in range(n)]


def stage(stage_id, seed, level, ap, exp, gold, stype=None, source=None):
    base = dt_stages[h(seed, "map") % len(dt_stages)]
    rec = {"stageId": stage_id, "areas": base["areas"], "mapId": base["mapId"], "useStamina": ap,
           "gainRankExp": exp, "gold": gold, "enemyLevel": level, "enemies": enemies(seed, stype),
           "source": source or {"source": "jdr-default",
                                "note": f"map and areas of {base['stageId']}; enemies, level and rewards ours"}}
    return rec


TYPE_WORDS = {"Courage": 1, "Wisdom": 2, "Spirit": 3, "Unity": 4, "Solitude": 5}
en = {}
for p in glob.glob("server/content/l10n/en/*.json"):
    for k, (_, e) in json.load(open(p))["entries"].items():
        en[k] = e


def label_text(lab):
    k = labels.get(lab)
    return en.get(k, "") if k else ""


def theme(text):
    found = [t for w, t in TYPE_WORDS.items() if w in text]
    if "Burning Spirit" in text:
        return 3
    if "Boundless Wit" in text:
        return 2
    return found[0] if len(found) == 1 else None


# ---- daily: two quest lines per weekday, four difficulties each ----
DAYS = [("SUN", "SUN", "sun"), ("MON", "MON", "mon"), ("TUE", "TUE", "tue"), ("WED", "WED", "wed"),
        ("THU", "THU", "thu"), ("FRI", "FRI", "fri"), ("STA", "SAT", "sta")]  # (DY code, DY2 code, banner)
TIERS = [(10, 6, 40, 500), (25, 8, 70, 800), (40, 10, 100, 1200), (60, 12, 150, 1600)]  # level, AP, EXP, gold
wiki = {}
if a.gamerch:
    for x in json.load(open(a.gamerch))["daily"]:
        if x.get("index_on_page") == 1 and x.get("ap"):
            wiki = {"ap": x["ap"], "exp": x["exp"], "gold": x["gold"], "page": x["source"]["page_url"]}
            break
chapters, daily = [], {}
for i, (dy, dy2, ban) in enumerate(DAYS):
    category = 6 + i
    banner = f"bizarre_type_daily_{ban}"
    assert banner in bundles, banner
    daily[dy] = {"category": category, "banner": banner}
    for line, (first, base) in enumerate([("01", 0), ("04", 100)]):
        cid = f"CHAPTER_DY_{dy}_{first}"
        assert f"CHAPTER_NAME_DY_{dy}_{first}" in labels, cid
        stype = theme(label_text(f"CHAPTER_NAME_DY_{dy}_{first}"))
        stages = []
        for t, (lv, apc, exp, gold) in enumerate(TIERS):
            sid = f"STAGE_DY2_{dy2}_{base + t + 1:03d}"
            if sid not in labels:
                continue
            src = None
            if t == 0 and wiki:
                apc, exp, gold = wiki["ap"], wiki["exp"], wiki["gold"]
                src = {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": wiki["page"],
                       "note": "Easy-tier daily AP/EXP/gold; map, areas, enemies and level ours"}
            stages.append(stage(sid, sid, lv, apc, exp, gold, stype, src))
        if stages:
            chapters.append({"chapterId": cid, "num": line + 1, "stageType": 1, "chapterMap": 0,
                             "category": category, "stages": stages})

# ---- events: a varied handful whose banner is in the player's files ----
EVENTS = ["50035", "50045", "50032", "50077"]
events = []
for n, ev in enumerate(EVENTS):
    ban = next((b[:-6] for b in sorted(bundles) if re.fullmatch(rf"eq_{ev[:2]}0{ev[2:]}_\w+_start_large", b)), None)
    assert ban, ev
    category = 13 + n
    chs = sorted(l for l in labels if re.fullmatch(rf"CHAPTER_NAME_EV_{ev}_\d\d", l))
    flat = sorted((l for l in labels if re.fullmatch(rf"STAGE_EV_{ev}_\d+", l)), key=lambda l: int(l.rsplit("_", 1)[1]))
    per = max(1, len(flat) // max(1, len(chs)))
    for j, cname in enumerate(chs):
        nn = cname.rsplit("_", 1)[1]
        own = sorted((l for l in labels if re.fullmatch(rf"STAGE_EV_{ev}_{nn}_\d+", l)), key=lambda l: int(l.rsplit("_", 1)[1]))
        sids = own or flat[j * per:(j + 1) * per]
        if not sids:
            continue  # no stage label in the game's text: leave the chapter out
        lv = min(80, 10 + 5 * j)
        chapters.append({"chapterId": f"CHAPTER_EV_{ev}_{nn}", "num": j + 1, "stageType": 1, "chapterMap": 0,
                         "category": category,
                         "stages": [stage(s, s, lv, 10, 30 + 5 * j, 600 + 100 * j) for s in sids[:6]]})
    events.append({"eventId": ev, "category": category, "banner": ban, "name": f"CHAPTER_NAME_EV_{ev}_01"})

ids = [s["stageId"] for c in story + chapters for s in c["stages"]]
assert len(ids) == len(set(ids)), "stage ids are unique"
doc = {"about": "Daily and event chapters for the Quest Select tiles. Labels and banners from the player's files; "
                "every stage reuses a tactical story stage's map; enemies, levels and rewards are ours unless cited. "
                "See docs/research/13-menus.md.",
       "sources": [{"source": "jdr-default", "note": "generated by tools/content/build_extra_quests.py"}],
       "daily": daily, "events": events, "chapters": chapters}
json.dump(doc, open(a.out, "w"), indent=1, ensure_ascii=False)
print(len(chapters), "chapters;", sum(len(c["stages"]) for c in chapters), "stages;",
      len(daily), "days;", len(events), "events")
