#!/usr/bin/env python3
"""Build server/content/quests_extra.json: daily and event chapters for the Quest Select tiles
(docs/research/13-menus.md §Daily/Event/Campaign).

Only identifiers come from the player's files: chapter and stage label keys from the game's text
(by key, our English already imported), banner bundle names from the player's file list. No
source gives the original daily/event stage layouts, so every stage reuses the map and areas of a
real tactical story stage and gets enemies, levels and rewards that are ours (`jdr-default`),
except the Easy-tier daily AP/EXP/gold, which the wiki gives (cited).

From Build 43 it also reads research-b43/stages_extra (`--research` may be given more than once):
Labyrinth room costs and area counts by room label (Bahamut floor screenshots), the cost-by-slot
pattern for rooms the screenshots leave out (floors 5-20), the ShoutWiki floor rewards of floors
1-4, and data-only facts (enemy-level previews, clear conditions, daily, event and Arena numbers)
that no stage id of ours carries. They go to the `labyrinth` and `wiki` keys, each with its page,
capture and access date; `server/services/src/labyrinth.rs` serves the room costs and the floor
rewards (docs/research/22 §2.1). `--export` (the JP label export) checks each room name against the
client's `STAGE_DU01_*` label and matches reward names to `ITEM_<id>_NAME` labels.

usage: python3 -I tools/content/build_extra_quests.py --names <jdr-tool names output> \
         [--gamerch local/wiki-raw/gamerch/stages.json] \
         [--research local/wiki-raw/research-b33/stages_extra/results.jsonl \
          --research local/wiki-raw/research-b43/stages_extra/results.jsonl \
          --export local/l10n/export-b33.tsv] --out server/content/quests_extra.json
"""
import argparse, glob, hashlib, json, re

ap = argparse.ArgumentParser()
ap.add_argument("--names", required=True, help="jdr-tool names output (logical bundle names)")
ap.add_argument("--gamerch", help="local/wiki-raw/gamerch/stages.json (daily quest numbers)")
ap.add_argument("--research", action="append", default=[],
                help="a research pass's stage facts (results.jsonl): useStamina, gainRankExp, gold, "
                "recommendedLevel per stageId, with page and confidence (medium/high used); from Build 43 "
                "also Labyrinth, daily, event and Arena facts (research-b43); repeatable")
ap.add_argument("--export", help="local/l10n/export-b33.tsv (JP labels: room names, item names)")
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
# Research facts per stage (Build 33): medium/high only; the recommended level is the enemy level,
# as for the story stages.
facts = {}
records = []  # every medium/high research record, in file order
for path in a.research:
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if r.get("confidence") not in ("high", "medium"):
            continue
        records.append(r)
        if r.get("stageId") and r.get("field") in (
                "useStamina", "gainRankExp", "gold", "recommendedLevel") and isinstance(r.get("value"), int):
            facts.setdefault(r["stageId"], {})[r["field"]] = r
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
            f = facts.get(sid, {})
            if f:
                apc = f["useStamina"]["value"] if "useStamina" in f else apc
                exp = f["gainRankExp"]["value"] if "gainRankExp" in f else exp
                gold = f["gold"]["value"] if "gold" in f else gold
                lv = f["recommendedLevel"]["value"] if "recommendedLevel" in f else lv
                first_fact = next(iter(f.values()))
                page = first_fact["url"]
                src = {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)" if "gamerch" in page else page.split("/")[2],
                       "page": page, **({"capture": first_fact["capture"]} if first_fact.get("capture") else {}),
                       "accessed": first_fact.get("accessed") or "2026-11-20",
                       "note": "wiki: " + ", ".join(sorted(k.replace("recommendedLevel", "recommended level as enemy level")
                                                         for k in f)) + "; map, areas, enemies and the rest ours"}
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

# ---- Labyrinth and data-only facts (research-b43/stages_extra, Build 43) ----
def site(url):
    host = url.split("/")[2]
    if "forum.gamer.com.tw" in host:
        return "Bahamut forum (bsn 31489)"
    if "shoutwiki.com" in host:
        return "Diamond Records Wiki (ShoutWiki), CC BY-SA 3.0"
    if "atwiki.jp" in host:
        return "ジョジョDR２ @ ウィキ (atwiki, Wayback Machine)"
    if "jojowiki.com" in host:
        return "JoJo Wiki"
    return host


used_pages = {}


def rsource(r, note=None):
    """A record's citation: site, page, the screenshot it was read from, capture, access date."""
    out = {"source": site(r["url"]), "page": r["url"]}
    img = re.search(r"https://truth\.bahamut\.com\.tw/\S+?\.(?:JPG|PNG|jpg|png)", r.get("note") or "")
    if img:
        out["image"] = img.group(0)
    rev = re.search(r"page revision (\d{4}-\d\d-\d\d)", r.get("note") or "")
    if rev:
        out["revision"] = rev.group(1)
    if r.get("capture"):
        out["capture"] = r["capture"]
    out["accessed"] = r["accessed"]
    out["confidence"] = r["confidence"]
    if note:
        out["note"] = note
    used_pages.setdefault(r["url"], r["accessed"])
    return out


jp = {}
if a.export:
    for line in open(a.export, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        if len(f) >= 4 and f[0].startswith("text:"):
            jp[f[0].split(":", 2)[2]] = f[3]


def item_by_jp(name):
    """The item whose game label (`ITEM_<id>_NAME`) is exactly this Japanese name, if one is."""
    ids = sorted({int(m.group(1)) for k, v in jp.items() if v == name
                  for m in [re.fullmatch(r"ITEM_(\d+)_NAME", k)] if m})
    return ids[0] if len(ids) == 1 else None


ROOM = re.compile(r"(STAGE_DU01_(\d\d)_(\d\d)) (.+)$")
# Why a data-only fact is not served (docs/research/22 §2.1, 23 §1.6, 25 §6, 13).
HELD = {
    "stages_extra.labyrinth": "data only: action-era (2018) preview or note; our rooms reuse story stages",
    "stages_extra.daily": "data only: confirms the weekday tiles; no stage numbers",
    "stages_extra.events": "data only: no stage id of ours carries these numbers",
    "stages_extra.arena": "data only: 2017 action-era Arena; our point scale, ladder and BP rate stay ours",
}
lab_rooms, lab_wiki, wiki_facts = {}, [], {}
pattern = floor_rewards = None
for r in records:
    if not r.get("topic", "").startswith("stages_extra.") or r.get("stageId"):
        continue
    m = ROOM.match(r["subject"])
    if m and r["field"] in ("useMentality", "areaMax") and isinstance(r["value"], int):
        label, floor, room, name = m.group(1), int(m.group(2)), int(m.group(3)), m.group(4)
        assert a.export, "--export is needed to check the room names"
        assert jp.get(label) == name, (label, name, jp.get(label))
        row = lab_rooms.setdefault(label, {"label": label, "floor": floor, "room": room})
        row[r["field"]] = r["value"]
        row.setdefault("source", rsource(r))
        continue
    if r["field"] == "useMentality_pattern":
        pattern = r
    elif r["field"] == "floorReward":
        floor_rewards = r
    fact = {"subject": r["subject"], "field": r["field"], "value": r["value"],
            **rsource(r, r.get("note") or None)}
    if r["field"] not in ("useMentality_pattern", "floorReward"):
        fact["held"] = HELD[r["topic"]]
    if r["topic"] == "stages_extra.labyrinth":
        lab_wiki.append(fact)
    else:
        wiki_facts.setdefault(r["topic"].split(".", 1)[1], []).append(fact)

labyrinth = {}
if lab_rooms:
    rooms = sorted(lab_rooms.values(), key=lambda x: (x["floor"], x["room"]))
    # The cost-by-slot pattern (medium, derived from the screenshots: from floor 5 on every room 01
    # seen costs 35, every room 02 27, every room 03 25) fills rooms 01-03 the screenshots leave
    # out, from floor 5 on; floors 1-4 differ and keep ours.
    if pattern:
        pv = pattern["value"]
        for floor in range(5, 21):
            for room in (1, 2, 3):
                label = f"STAGE_DU01_{floor:02}_{room:02}"
                if label in labels and label not in lab_rooms and isinstance(pv.get(f"rr{room:02}"), int):
                    rooms.append({"label": label, "floor": floor, "room": room,
                                  "useMentality": pv[f"rr{room:02}"],
                                  "source": rsource(pattern, f"cost-by-slot pattern (room {room:02} from floor 5 "
                                                             "on); not on a screenshot")})
        rooms.sort(key=lambda x: (x["floor"], x["room"]))
    rewards = []
    for x in (floor_rewards or {}).get("value", []):
        iid = item_by_jp(x["item_jp"])
        if iid is None:
            continue  # no game label with that name: not served
        row = {"floor": x["floor"], "itemId": iid, "nameJp": x["item_jp"], "count": x["qty"] or 10,
               "source": rsource(floor_rewards)}
        if not x["qty"]:
            row["countFrom"] = "jdr-default"
            row["source"]["note"] = "quantity cut off on the screenshot: count ours"
        rewards.append(row)
    labyrinth = {"about": "Labyrinth room costs (useMentality) and area counts (areaMax, data only: our rooms have "
                          "one area) by the client's room label, and floor rewards; read by "
                          "server/services/src/labyrinth.rs. See docs/research/22-labyrinth.md §2.1.",
                 "rooms": rooms, "floorRewards": rewards, "wiki": lab_wiki}

ids = [s["stageId"] for c in story + chapters for s in c["stages"]]
assert len(ids) == len(set(ids)), "stage ids are unique"
doc = {"about": "Daily and event chapters for the Quest Select tiles. Labels and banners from the player's files; "
                "every stage reuses a tactical story stage's map; enemies, levels and rewards are ours unless cited. "
                "See docs/research/13-menus.md.",
       "sources": [{"source": "jdr-default", "note": "generated by tools/content/build_extra_quests.py"}]
       + [{"source": site(u), "page": u, "accessed": d, "note": "research pass (stages_extra)"}
          for u, d in sorted(used_pages.items())],
       "daily": daily, "events": events, "campaigns": campaigns, "chapters": chapters}
if labyrinth:
    doc["labyrinth"] = labyrinth
if wiki_facts:
    doc["wiki"] = wiki_facts
json.dump(doc, open(a.out, "w"), indent=1, ensure_ascii=False)
print(len(chapters), "chapters;", sum(len(c["stages"]) for c in chapters), "stages;",
      len(daily), "days;", len(events), "events;", len(campaigns), "campaigns;",
      len(labyrinth.get("rooms", [])), "labyrinth rooms;", len(labyrinth.get("floorRewards", [])), "floor rewards;",
      sum(len(v) for v in wiki_facts.values()) + len(labyrinth.get("wiki", [])), "data-only facts")
