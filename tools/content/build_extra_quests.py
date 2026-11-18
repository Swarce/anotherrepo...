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
    """stype: None (any), one StatueType, or a list of them."""
    types = None if stype is None else (set(stype) if isinstance(stype, (list, tuple, set)) else {stype})
    pool = [s["statueId"] for s in statues if types is None or s.get("statueType") in types] or [s["statueId"] for s in statues]
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


# Material days (research pack v2.growth.weekday, launch guide): Mon/Thu Wisdom, Tue/Fri Spirit,
# Wed/Sat Courage, Sunday all three.
SUNDAY = [1, 2, 3]


def theme(text, day):
    found = [t for w, t in TYPE_WORDS.items() if w in text]
    if "Burning Spirit" in text:
        return 3
    if "Boundless Wit" in text:
        return 2
    if len(found) == 1:
        return found[0]
    return found or (SUNDAY if day == "SUN" else None)


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
        stype = theme(label_text(f"CHAPTER_NAME_DY_{dy}_{first}"), dy)
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
# Kinds (docs/research/25-events-raids.md): "exchange" (Fight Coin exchange, Build 22), "point"
# (event points and point rewards, Build 23), "high_score" / "high_score_shop" (eventCollectType
# 4 / 5), "quiz" (question and answer stages). The 5600xx events are "High Score Battle" by their
# campaign name, with a scoring rule in each stage name; 560001 has an event-shop goods label, so it
# gets the shop. 50090 has quiz chapters. Currencies are items in the player's files: Fight Coin
# (3017) where the event's own is unknown (ours), Fight Coin 2 (3032) for the high-score shop
# (ours), the Chunky Necklace (3106) that 50090's own quiz texts name.
EVENTS = [("50035", "exchange", 3017), ("50045", "point", 0), ("50032", "exchange", 3017),
          ("50077", "exchange", 3017), ("560001", "high_score_shop", 3032), ("560005", "high_score", 0),
          ("50090", "quiz", 3106), ("50033", "exchange", 3017), ("50061", "exchange", 3017)]
minds = json.load(open("server/content/minds.json"))["minds"]


def label_id(ev):
    """`50035` -> `500035`: a 5-digit event id names 6-digit banners and labels."""
    return f"{ev[:2]}0{ev[2:]}" if len(ev) == 5 else ev


def bonus(ev):
    """Event bonus units (ours: no source names those of these events): three 4-star+ statues and
    two 4-star Minds, a stable pick by the event id."""
    st = sorted(s["statueId"] for s in statues if (s.get("rarity") or 4) >= 4)
    md = sorted(m["mindId"] for m in minds if m["rarity"] >= 4)
    out = []
    for pool, n in ((st, 3), (md, 2)):
        got, i = [], 0
        while len(got) < n:
            x = pool[h(ev, "bonus", i) % len(pool)]
            i += 1
            if x not in got:
                got.append(x)
        out.append(sorted(got))
    return out


SCORE_WORDS = [("Max", 3), ("Total", 4), ("Turns", 2)]  # DTEventPointType MaxDamage 3, TotalDamage 4, Turn 2


def scoring(sid):
    """A high-score stage's scoring rule from its name ("Max & Total & Turns", "Max Damage", ...)."""
    text = label_text(sid)
    return [t for w, t in SCORE_WORDS if re.search(rf"\b{w}\b", text)]


script_lines = {}
for p in glob.glob("server/content/l10n/en/*.json"):
    for k, (_, e) in json.load(open(p))["entries"].items():
        m = re.match(r"script:(e_ev_eq_\d+_\w+?_\d+)_script_text_jp:-?\d+:(\d+)$", k)
        if m:
            script_lines.setdefault(m.group(1), []).append((int(m.group(2)), e))
STOP = {"the", "a", "an", "on", "of", "is", "stage", "answer", "slower", "healing"}


def tokens(t):
    return set(re.findall(r"[a-z0-9]+", t.lower())) - STOP


def quiz_of(ev, chapter_id, sids):
    """A quiz chapter: a "Question Stage" and three "Answer Stage <n>: <choice>" stages. The right
    choice comes from the event's own scripts: an answer script names it ("The answer is (n)! ..."),
    or else the script right after the question script that lists these choices."""
    q = [s for s in sids if label_text(s) == "Question Stage"]
    ans = sorted((int(m.group(1)), s, m.group(2)) for s in sids
                 for m in [re.match(r"Answer Stage (\d): (.+)$", label_text(s))] if m)
    if len(q) != 1 or [a[0] for a in ans] != [1, 2, 3]:
        return None
    scripts = {k: [e for _, e in sorted(v)] for k, v in script_lines.items()
               if k.startswith(f"e_ev_eq_{label_id(ev)}_")}
    choices = [tokens(c) for _, _, c in ans]
    uniq = [c - set().union(*(o for o in choices if o is not c)) for c in choices]
    found = set()
    for v in scripts.values():
        for line in v:
            m = re.match(r"The answer is \((\d)\)", line)
            if m and tokens(line) & uniq[int(m.group(1)) - 1]:
                found.add(int(m.group(1)))
    if len(found) != 1:
        best, n = 0, None
        for k, v in sorted(scripts.items()):
            opts = next((line for line in v if line.startswith("(1)")), None)
            if not opts:
                continue
            score = len(tokens(opts) & set().union(*choices))
            nxt = re.sub(r"_(\d+)$", lambda m: f"_{int(m.group(1)) + 1:02d}", k)
            m = re.match(r"The answer is \((\d)\)", (scripts.get(nxt) or [""])[0])
            if m and score > best:
                best, n = score, int(m.group(1))
        found = {n} if n else set()
    if len(found) != 1:
        return None
    right = found.pop()
    return {"chapterId": chapter_id, "question": q[0], "answers": [s for _, s, _ in ans],
            "correct": ans[right - 1][1]}


events = []
for n, (ev, kind, currency) in enumerate(EVENTS):
    ban = next((b[:-6] for b in sorted(bundles) if re.fullmatch(rf"eq_{label_id(ev)}_\w+_start_large", b)), None)
    assert ban, ev
    category = 13 + n
    chs = sorted(l for l in labels if re.fullmatch(rf"CHAPTER_NAME_EV_{ev}_\d\d", l))
    flat = sorted((l for l in labels if re.fullmatch(rf"STAGE_EV_{ev}_\d+", l)), key=lambda l: int(l.rsplit("_", 1)[1]))
    per = max(1, len(flat) // max(1, len(chs)))
    score, quiz = {}, []
    for j, cname in enumerate(chs):
        nn = cname.rsplit("_", 1)[1]
        own = sorted((l for l in labels if re.fullmatch(rf"STAGE_EV_{ev}_{nn}_\d+", l)), key=lambda l: int(l.rsplit("_", 1)[1]))
        sids = own or flat[j * per:(j + 1) * per]
        if not sids:
            continue  # no stage label in the game's text: leave the chapter out
        lv = min(80, 10 + 5 * j)
        cid = f"CHAPTER_EV_{ev}_{nn}"
        chapters.append({"chapterId": cid, "num": j + 1, "stageType": 1, "chapterMap": 0,
                         "category": category,
                         "stages": [stage(s, s, lv, 10, 30 + 5 * j, 600 + 100 * j) for s in sids[:6]]})
        if kind.startswith("high_score"):
            score.update({s: r for s in sids[:6] for r in [scoring(s)] if r})
        if kind == "quiz":
            q = quiz_of(ev, cid, sids[:6])
            if q:
                quiz.append(q)
    st, md = bonus(ev)
    rec = {"eventId": ev, "category": category, "banner": ban, "name": f"CHAPTER_NAME_EV_{ev}_01",
           "kind": kind, "currency": currency, "bonusStatues": st, "bonusMinds": md}
    if kind.startswith("high_score"):
        assert score, ev
        rec["scoring"] = score
    if kind == "quiz":
        assert quiz, ev
        rec["quiz"] = quiz
    events.append(rec)

# ---- campaigns (Campaign Info, the Arena home): a banner `ec_<id>_<hash>_start_small` with a
# campaign name label `EC_<id>_Name` ----
campaigns = []
for b in sorted(bundles):
    m = re.fullmatch(r"(ec_(\d{6})_\w+_start)_small", b)
    if m and f"EC_{m.group(2)}_Name" in labels:
        campaigns.append({"campaignId": m.group(2), "banner": m.group(1), "name": f"EC_{m.group(2)}_Name"})
assert campaigns

ids = [s["stageId"] for c in story + chapters for s in c["stages"]]
assert len(ids) == len(set(ids)), "stage ids are unique"
doc = {"about": "Daily and event chapters for the Quest Select tiles. Labels and banners from the player's files; "
                "every stage reuses a tactical story stage's map; enemies, levels and rewards are ours unless cited. "
                "See docs/research/13-menus.md.",
       "sources": [{"source": "jdr-default", "note": "generated by tools/content/build_extra_quests.py"}],
       "daily": daily, "events": events, "campaigns": campaigns, "chapters": chapters}
json.dump(doc, open(a.out, "w"), indent=1, ensure_ascii=False)
print(len(chapters), "chapters;", sum(len(c["stages"]) for c in chapters), "stages;",
      len(daily), "days;", len(events), "events;", len(campaigns), "campaigns")
