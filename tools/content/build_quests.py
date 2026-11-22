#!/usr/bin/env python3
"""Build server/content/quests.json: story chapters and stages (identifiers from the player's own
files via scan_quests.py) with numbers from the community wikis where available.

Gamerch chapters attach by title: the wiki's chapter title (without its "Chapter N" prefix) is
compared after NFKC normalisation with the game's own CHAPTER_NAME_<id> label, and a chapter is used
only when the wiki lists exactly as many stages as the game has (stage k -> the chapter's k-th stage).
Fields the wiki leaves blank keep marked defaults and are named in the stage's `defaults` list; an
enemy level under half of the previous stage's is treated as a wiki typo and replaced the same way.

Research passes (`--research`, the stages_story results.jsonl of research-b33 and research-b43;
Build 43): only high and medium records are read, and they attach by the game ids the
researchers matched (their READMEs say how). Per stage: AP (`useStamina`), the recommended level
(used as the enemy level, as for Gamerch), the three star missions, and the tactical first-clear
and special rewards. A stage the Gamerch merge already sourced keeps its Gamerch values (a medium
verdict does not change a wiki value). Chapter-level facts (an EXP maximum, an unmapped stage,
a stage-count conflict) go to the chapter's `wiki` list and are not served.

Star missions (`missions`, QuestMissionClearType values the server sends on action stages) come
from the wiki's own condition texts (Gamerch 特別報酬条件, JoJo Wiki, the Bahamut forum), mapped to
the client's types through MISSION_TYPES; stages without one keep the server's default missions.
Reward names stay as the wiki writes them (`nameJp`); `itemId` is set only when the name equals
exactly one of the game's own ITEM_<id>_NAME labels (NFKC). Rewards are data only (not served).

Defaults (stamina, rank EXP, gold, enemy level, enemies from the same JoJo part, a battle-ready map)
grow with the chapter's position among chapters of its type and stay inside the game's limits
(stamina <= 20, the rank-1 AP; level <= 99). Enemies are team statues only (not assist-only ones).
Dates in the output come from the inputs, so a rerun reproduces the file byte for byte.

usage: python3 -I tools/content/build_quests.py --scan local/content/quests_scan.json \
          --statues server/content/statues.json --export local/l10n/export-b33.tsv \
          --gamerch local/wiki-raw/gamerch/stages.json \
          --research local/wiki-raw/research-b33/stages_story/results.jsonl \
          --research local/wiki-raw/research-b43/stages_story/results.jsonl \
          --out server/content/quests.json
"""
import argparse
import json
import re
import unicodedata

ap = argparse.ArgumentParser()
ap.add_argument("--scan", required=True)
ap.add_argument("--statues", required=True)
ap.add_argument("--gamerch")
ap.add_argument("--export", help="the player's l10n export (CHAPTER_NAME, ITEM_<id>_NAME and "
                "QuestMissionText labels), needed with --gamerch and --research")
ap.add_argument("--research", action="append", default=[],
                help="a research stages_story results.jsonl (repeatable; high/medium records only)")
ap.add_argument("--out", required=True)
a = ap.parse_args()
scan = json.load(open(a.scan))
# Enemies are drawn from team statues: assist-only statues (Build 25) never fight.
statues = [s for s in json.load(open(a.statues))["statues"] if not s.get("assistOnly")]


def norm(x):
    return unicodedata.normalize("NFKC", x or "").replace(" ", "").replace("\u3000", "")


# The wikis' star-mission texts -> the client's QuestMissionClearType (label QuestMissionText_<type + 2>).
# Japanese: Gamerch's wording, which is the game's own label or the label without を/する; English:
# JoJo Wiki's and the research notes' wording (parenthetical remarks dropped before the lookup).
MISSION_TYPES = {
    "クエストクリア": 14, "クエストをクリア": 14, "クエストをクリアする": 14, "Complete the quest": 14,
    "チームメンバーが全員生存": 7, "All team members survive": 7,
    "スキルを使用する": 20,
    "SPスキルを使用する": 22, "Use at least one SP": 22,
    "ドドドモードを使用する": 4,
    "ドドドモードでボスを倒す": 25,
    "スキルでボスを倒す": 26, "Defeat the boss with a skill": 26,
    "SPスキルでボスを倒す": 27, "Defeat Boss with an SP skill": 27,
}


def mission_key(text):
    t = re.sub(r"[(（][^)）]*[)）]", "", unicodedata.normalize("NFKC", text))
    return t if re.search(r"[ぁ-ヿ]", t) else re.sub(r"[^a-z]", "", t.lower())


MISSION_TYPES = {mission_key(k): v for k, v in MISSION_TYPES.items()}


def mission_types(conds, where):
    out = []
    for c in conds:
        k = mission_key(c)
        if k not in MISSION_TYPES:
            raise SystemExit(f"{where}: mission text {c!r} has no QuestMissionClearType in MISSION_TYPES")
        out.append(MISSION_TYPES[k])
    return out


def site(url):
    if "atwiki.jp/jojodrr" in url:
        return "ジョジョDR２ @ ウィキ (atwiki, Wayback Machine)"
    if "gamerch.com" in url:
        return "Gamerch ジョジョDR攻略Wiki" + (" (Wayback Machine)" if "web.archive.org" in url else " (live page)")
    if "forum.gamer.com.tw" in url:
        return "Bahamut forum (bsn 31489)"
    if "jojowiki.com" in url:
        return "JoJo Wiki"
    raise SystemExit(f"unknown research source {url}")


wiki = {}  # game stageId -> numbers from the wiki
missions = {}  # game stageId -> (QuestMissionClearType list, source)
gamerch_dates = set()  # access dates of the wiki data used (the output's dates come from them)
unmatched = []
labels = {}  # NFKC label -> ids, per label kind
if a.export:
    for line in open(a.export, encoding="utf-8"):
        c = line.rstrip("\n").split("\t")
        if len(c) < 4:
            continue
        m = re.fullmatch(r"text:[^:]+:ITEM_(\d+)_NAME", c[0])
        if m:
            labels.setdefault(("item", norm(c[3])), set()).add(int(m.group(1)))
        m = re.fullmatch(r"text:QuestMission_JP:QuestMissionText_(\d+)", c[0])
        if m:
            labels.setdefault(("mission", c[3]), set()).add(int(m.group(1)) - 2)
    # Where a wiki wording is the game's own label, the label's type must be the table's.
    for k, v in MISSION_TYPES.items():
        got = labels.get(("mission", k))
        if got is not None and got != {v}:
            raise SystemExit(f"MISSION_TYPES[{k!r}] = {v}, the game's label says {sorted(got)}")
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
            gsrc = {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": w["source"]["page_url"],
                    "capture": w["source"].get("capture"), "accessed": w["source"].get("accessed")}
            if w["source"].get("accessed"):
                gamerch_dates.add(w["source"]["accessed"])
            if w.get("special_conditions"):
                missions[stage_id] = (mission_types(w["special_conditions"], stage_id), dict(gsrc))
            if w.get("ap") is None:
                continue
            rec = {"useStamina": w["ap"], "source": gsrc}
            for field, key in (("gainRankExp", "exp"), ("gold", "gold"), ("enemyLevel", "recommended_level")):
                if w.get(key) is not None:
                    rec[field] = w[key]
            wiki[stage_id] = rec

# Research passes (stages_story). Records name our game ids (`subject`, or b33's `stageId`).
CONF = {"high": 2, "medium": 1}
research = {}  # (subject, field) -> best record (higher confidence, then the first)
for path in a.research:
    if not a.export:
        ap.error("--research needs --export")
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if r.get("confidence") not in CONF:
            continue
        subj = r.get("subject") or r.get("stageId")
        old = research.get((subj, r["field"]))
        if old is None or CONF[r["confidence"]] > CONF[old["confidence"]]:
            research[(subj, r["field"])] = r


used_pages = {}  # research pages the output cites -> access date


def rsource(r, fields=None, note=None):
    used_pages.setdefault(r["url"].split("#")[0], r["accessed"])
    out = {"source": site(r["url"]), "page": r["url"]}
    if r.get("capture"):
        out["capture"] = r["capture"]
    out["accessed"] = r["accessed"]
    out["confidence"] = r["confidence"]
    if fields:
        out["fields"] = fields
    if note:
        out["note"] = note
    return out


def reward(v):
    ids = labels.get(("item", norm(v["name_jp"])), set())
    return {"nameJp": v["name_jp"], "qty": v.get("qty"), "itemId": next(iter(ids)) if len(ids) == 1 else None}


game_ids = {c["chapterId"]: [s["stageId"] for s in c["stages"]] for k in ("battle", "dt") for c in scan[k]}
all_stages = {s for ss in game_ids.values() for s in ss}
stage_extra = {}  # stageId -> extra keys (rewards) and their sources (`also`)
mission_conflicts = {}  # boss stageId -> the research conflict record on its third mission
CONFLICT_NOTE = ("conflict on the boss-stage third mission: Bahamut players (2019) say a regular (blue) skill, "
                 "JoJo Wiki and Gamerch an SP skill; each stage keeps its own source's wording")
chapter_wiki = {}  # chapterId -> chapter-level facts (not served)


def chapter_range(subject):
    m = re.fullmatch(r"(CHAPTER_(?:DT_)?ST_)(\d+)\.\.CHAPTER_(?:DT_)?ST_(\d+)(?: final stages)?", subject)
    if not m:
        return [subject] if subject in game_ids else []
    lo, hi = int(m.group(2)), int(m.group(3))
    return [c for c in game_ids if c.startswith(m.group(1)) and lo <= int(c[len(m.group(1)):]) <= hi]


for (subj, field), r in sorted(research.items()):
    v = r["value"]
    if subj in all_stages and field in ("useStamina", "recommendedLevel"):
        if subj in wiki and "research" not in wiki[subj]:  # already sourced from Gamerch: kept
            continue
        key = "enemyLevel" if field == "recommendedLevel" else field
        rec = wiki.setdefault(subj, {"research": []})
        rec[key] = v
        rec["research"].append((key, r))
    elif subj in all_stages and field == "missions":
        if subj in missions:
            continue
        missions[subj] = (mission_types(v["conditions"], subj), rsource(r))
    elif subj in all_stages and field in ("firstClearRewards", "specialReward"):
        # Named items only (b33's action-stage rewards are image file names).
        items = [reward(x) for x in (v if isinstance(v, list) else [v]) if x.get("name_jp")]
        if not items:
            continue
        kind = "firstClear" if field == "firstClearRewards" else "special"
        e = stage_extra.setdefault(subj, {"rewards": {}, "also": []})
        e["rewards"][kind] = items
        e["also"].append(rsource(r, ["rewards." + kind]))
    elif field == "missions_conflict":
        for cid in chapter_range(subj):
            mission_conflicts[game_ids[cid][-1]] = r
    else:
        cids = chapter_range(subj)
        if not cids:
            continue
        if field == "chapterRewards_conflict":
            n = v["wiki_stage_count"]
            fact = {"field": "rewards", "held": f"the atwiki lists {n} stage{'s' if n != 1 else ''} for "
                    f"{v['wiki_chapter']}, the game has {v['our_stage_count']}: not mapped to stages"}
        elif field == "stageCountChange":
            fact = {"field": "stageCount", "value": v["atwiki_2019_counts"],
                    "note": "2018-09-28 maintenance reduced the new-mode story's prologue to chapter 4 "
                            "(the game's files list the stages from before the cut)"}
        elif field == "gainRankExp_max":
            fact = {"field": "gainRankExpMax", "value": v,
                    "note": "the top stage EXP of chapters 23 on; per-stage EXP not given (stages keep defaults)"}
        elif field == "stageNumbers":
            fact = {"field": "oneStage", "value": {k: v[k] for k in ("useStamina", "gainRankExp", "difficulty")},
                    "note": "one unstated stage of the chapter (not applied); 難易度 is not a player level here"}
        elif field == "useStamina_recommendedLevel_corroboration":
            fact = {"field": "corroboration", "value": v,
                    "note": "a second source for the chapter's AP and recommended level"}
        else:
            continue
        for cid in cids:
            chapter_wiki.setdefault(cid, []).append(dict(fact, **rsource(r)))

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
        also = []
        if w and "research" in w:
            # Numbers from the research passes: one source per page, the first one is the stage's.
            by_page = {}
            for key, r in w["research"]:
                by_page.setdefault(r["url"], (r, []))[1].append(key)
            srcs = [rsource(r, keys) for r, keys in by_page.values()]
            w = dict({k: v for k, v in w.items() if k != "research"}, source=srcs[0])
            also += srcs[1:]
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
        if s["stageId"] in missions:
            types, msrc = missions[s["stageId"]]
            stage["missions"] = types
            conflict = mission_conflicts.get(s["stageId"])
            if msrc.get("page") != stage["source"].get("page"):
                msrc = dict(msrc, fields=["missions"])
                if conflict and conflict["url"] == msrc["page"]:
                    msrc["note"], conflict = CONFLICT_NOTE, None
                also.append(msrc)
            if conflict:
                also.append(rsource(conflict, ["missions"], CONFLICT_NOTE))
        e = stage_extra.get(s["stageId"])
        if e and e["rewards"]:
            stage["rewards"] = e["rewards"]
        also += e["also"] if e else []
        if also:
            # One entry per source and note, listing every field it gives.
            merged = {}
            for x in also:
                k = json.dumps({f: v for f, v in x.items() if f != "fields"}, sort_keys=True, ensure_ascii=False)
                if k in merged:
                    merged[k]["fields"] = merged[k]["fields"] + x["fields"]
                else:
                    merged[k] = dict(x)
            stage["also"] = list(merged.values())
        prev_level = stage["enemyLevel"]
        stages.append(stage)
    chapters.append({"chapterId": c["chapterId"], "num": c["num"], "stageType": st,
                     "chapterMap": c["chapterMap"], "stages": stages,
                     **({"wiki": chapter_wiki[c["chapterId"]]} if c["chapterId"] in chapter_wiki else {})})

doc = {
    "about": "Story chapters and stages: identifiers from the player's own files (scan_quests.py); numbers from community wikis where merged, else marked defaults. See docs/CONTENT.md.",
    "sources": [dict({"source": "jdr-default", "note": "identifiers from the player's files; defaults where no wiki data"},
                     **({"accessed": min(gamerch_dates)} if gamerch_dates else {}))]
               + ([{"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": "https://jojodr.gamerch.com/",
                    "accessed": min(gamerch_dates)}] if gamerch_dates else [])
               + [{"source": site(u), "page": u, "accessed": d, "note": "research pass (stages_story)"}
                  for u, d in sorted(used_pages.items())],
    "chapters": chapters,
}
json.dump(doc, open(a.out, "w"), indent=1)
print(f"Gamerch chapters not matched (title or stage count): {unmatched}")
print(f"{len(chapters)} chapters, {sum(len(c['stages']) for c in chapters)} stages, "
      f"{sum(1 for c in chapters for s in c['stages'] if s['source']['source'] != 'jdr-default')} with wiki numbers, "
      f"{sum(1 for c in chapters for s in c['stages'] if 'missions' in s)} with wiki missions, "
      f"{sum(1 for c in chapters for s in c['stages'] if 'rewards' in s)} with wiki rewards, "
      f"{sum(1 for c in chapters if 'wiki' in c)} chapters with chapter-level facts")
