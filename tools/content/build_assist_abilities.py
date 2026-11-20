#!/usr/bin/env python3
"""Assist abilities (docs/research/19-assists.md §5): the shared ability rows
`DT_AST_<KIND><tier>` in server/content/masterdata/dtbattle_ability_data_table.json and
server/content/assist_abilities.json, which gives every statue of the roster its abilities.

Sources, in order (ids, enum names and numbers only; no wiki text goes into the output):
  1. ShoutWiki's per-statue assist-ability numbers (action-battle era), when the entry matches one
     roster statue exactly: the statue's page (statues.json source), else the character's JP name
     label plus the statue's wiki rarity and type, with exactly one entry and one statue on each
     side. Only the stats the tactical battle has an affect for are kept (HP, ATK, DEF, damage
     dealt); each wiki value takes the tier whose value is nearest.
  2. Otherwise a default (jdr-default): one ability on the stat the statue's stats lean to (HP
     counted at two thirds, as for the Minds; ATK when unknown), tier = rarity - 2 (3* tier 1 ..
     6* tier 4).

Label keys: the game names its generic assist abilities ABI_AST_<KIND><1-4>_N
(Ability_Assist_Name_JP); each row takes the key of its kind and tier. The per-statue keys
ABI_AST_<statueId>_N belong to the assist-only statues (`assistOnly` in statues.json; 2100_03,
a roster statue with one, keeps the shared rows since its effect is unknown): each assist-only
statue gets its own row DT_AST_<statueId> under its own label, with the kind and tier it would
otherwise take (wiki, else the default).

Research facts (--research, Build 33): an assist_ability record of medium or high confidence for a
statue gives
  - its activation condition: the attributes the wiki lists (one per required assist, e.g. two
    Fighting Spirit) as [{statueType, count}];
  - its effects where the tactical battle has an affect for them (HP, ATK, DEF, damage dealt, as
    percentages; nearest tier). Action-era effects (move speed, DoDoDo, cool-down, damage to a
    race, one stat from another) have none: the statue then keeps its ShoutWiki or default kind,
    under the wiki's condition.

Conditions otherwise: none (always active). The client's condition rows ([{statueType, count}])
are the server's assist_ability_condition_table, generated from the content file.

Idempotent: replaces any DT_AST_* rows and keeps every other row as it is.

usage: python3 -I tools/content/build_assist_abilities.py \
         --shoutwiki local/wiki-raw/shoutwiki/statues.json --labels local/l10n/work23b.tsv \
         [--statues server/content/statues.json] \
         [--table server/content/masterdata/dtbattle_ability_data_table.json] \
         [--out server/content/assist_abilities.json]
"""
import argparse, collections, json, re

# kind -> (affect id, per-mille value of tiers 1..4). Tier values follow the ShoutWiki numbers
# (HP 5/6/10%, ATK 4/6/8/20%, DEF 6/8%, damage dealt 3/5/7/18%); the others are ours.
KINDS = {
    "HP_RATEADD": ("DTAF_HP_RATEADD", [50, 60, 100, 200]),
    "ATK_RATEADD": ("DTAF_ATK_RATEADD", [40, 60, 80, 200]),
    "DEF_RATEADD": ("DTAF_DEF_RATEADD", [60, 80, 100, 200]),
    "DMG_RATEADD": ("DTAF_DMG_RATEADD", [30, 50, 70, 180]),
}
# ShoutWiki stat names (exact, case-insensitive) -> kind. Anything else (move speed, cool-down,
# DoDoDo charge, KO timer, attribute targets...) has no tactical affect yet and is skipped.
WIKI_STAT = {"hp": "HP_RATEADD", "atk": "ATK_RATEADD", "def": "DEF_RATEADD",
             "damage dealt": "DMG_RATEADD", "damage given": "DMG_RATEADD"}
STATUE_TYPE = {"Courage": 1, "Tactical": 2, "Fighting Spirit": 3, "Unity": 4, "Solitary": 5}

ap = argparse.ArgumentParser()
ap.add_argument("--shoutwiki", required=True)
ap.add_argument("--labels", required=True, help="l10n export (key, hash, english, japanese)")
ap.add_argument("--statues", default="server/content/statues.json")
ap.add_argument("--table", default="server/content/masterdata/dtbattle_ability_data_table.json")
ap.add_argument("--out", default="server/content/assist_abilities.json")
ap.add_argument("--research", help="a research pass's statue facts (results.jsonl): assist_ability records "
                "with statueId, effects, condition attributes, page and confidence")
a = ap.parse_args()

labels = {}
for line in open(a.labels, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    if len(c) >= 4:
        labels[c[0]] = c[3]


def row_id(kind, tier):
    return f"DT_AST_{kind}{tier}"


def name_key(kind, tier):
    return f"ABI_AST_{kind}{tier}_N"


for kind in KINDS:
    for t in range(1, 5):
        k = f"text:Ability_Assist_Name_JP:{name_key(kind, t)}"
        assert k in labels, f"missing label {k}"

statues = json.load(open(a.statues))["statues"]
OWN = {s["statueId"] for s in statues if s.get("assistOnly")
       and f"text:Ability_Assist_Name_JP:ABI_AST_{s['statueId']}_N" in labels}

# 1) The rows.
doc = json.load(open(a.table))
rows = [r for r in doc["data"]["paramList"] if not r["abilityId"].startswith("DT_AST_")]
for kind, (affect, values) in KINDS.items():
    for t, v in enumerate(values, 1):
        p = {"affectId": affect, "giveLimit": -1,
             **{f"paramLv1_{i}": v if i == 0 else 0 for i in range(8)},
             **{f"paramLv10_{i}": v if i == 0 else 0 for i in range(8)}}
        rows.append({"abilityId": row_id(kind, t), "abilityName": name_key(kind, t),
                     "abilityParamList": [p]})
own_rows = []  # filled with the statues below, appended before the table is written
note = ("Assist abilities DT_AST_<KIND><tier> (tools/content/build_assist_abilities.py): names are the "
        "game's ABI_AST_<KIND><tier>_N label keys; values per tier follow the ShoutWiki assist numbers "
        "where it has them (HP 5/6/10%, ATK 4/6/8/20%, DEF 6/8%, damage dealt 3/5/7/18%), the rest are ours; "
        "Lv10 = Lv1")
if not any(s.get("note") == note for s in doc["sources"]):
    doc["sources"] = [s for s in doc["sources"] if "DT_AST" not in s.get("note", "")]
    doc["sources"].append({"source": "jdr-default", "note": note})
if "DT_AST" not in doc["about"]:
    doc["about"] += " Rows DT_AST_<kind><tier> are the assist abilities (docs/research/19-assists.md)."


# 2) Statue -> abilities.
wiki = json.load(open(a.shoutwiki))


def norm(s):
    return (s or "").replace(" ", "").replace("\u3000", "").replace("\u30fb", "")


def wiki_kinds(w):
    out = []
    for x in w.get("assist_ability") or []:
        kind = WIKI_STAT.get(x.get("stat", "").strip().lower())
        if kind and x.get("percent") and x.get("value"):
            vals = KINDS[kind][1]
            tier = 1 + min(range(4), key=lambda i: abs(vals[i] - 10 * x["value"]))
            out.append((kind, tier))
    return out


by_page = {w["source"]["page"]: w for w in wiki if w.get("source", {}).get("page")}
by_key = collections.defaultdict(list)
for w in wiki:
    if w.get("jp_name") and w.get("type") in STATUE_TYPE:
        by_key[(norm(w["jp_name"]), w.get("rarity"), STATUE_TYPE[w["type"]])].append(w)
ids_by_key = collections.defaultdict(list)
for s in statues:
    if s.get("source", {}).get("source") != "jdr-default":
        n = norm(labels.get(f"text:StatueName_JP:STATUE_NAME_{s['statueId']}"))
        if n:
            ids_by_key[(n, s.get("rarity"), s.get("statueType"))].append(s["statueId"])


def lean(s):
    st = s.get("lv99")
    if not st:
        return "ATK_RATEADD"
    hp = st["hp"] * 2 // 3
    if st["def"] > st["atk"] and st["def"] > hp:
        return "DEF_RATEADD"
    return "HP_RATEADD" if hp > st["atk"] else "ATK_RATEADD"


# 3) Research facts: per statue, the condition and the effects with a tactical affect.
TYPE_JP = {"勇気": 1, "知略": 2, "闘志": 3, "団結": 4, "孤高": 5}
GENERIC = [(re.compile(r"^\s*HP\s*\+\s*([\d.]+)\s*%\s*$", re.I), "HP_RATEADD"),
           (re.compile(r"^\s*ATK\s*\+\s*([\d.]+)\s*%\s*$", re.I), "ATK_RATEADD"),
           (re.compile(r"^\s*DEF\s*\+\s*([\d.]+)\s*%\s*$", re.I), "DEF_RATEADD"),
           (re.compile(r"^\s*Damage\s+(?:Dealt|Given)\s*\+\s*([\d.]+)\s*%\s*$", re.I), "DMG_RATEADD")]
research = {}
if a.research:
    RANK = {"high": 3, "medium": 2}
    for line in open(a.research, encoding="utf-8"):
        r = json.loads(line)
        if r.get("field") != "assist_ability" or r.get("confidence") not in RANK:
            continue
        old = research.get(r["statueId"])
        if old is None or RANK[r["confidence"]] > RANK[old["confidence"]]:
            research[r["statueId"]] = r
        elif RANK[r["confidence"]] == RANK[old["confidence"]] and r["value"] != old["value"]:
            research[r["statueId"]] = {"conflict": True, "confidence": r["confidence"]}


def research_kinds(r):
    out = []
    for e in r["value"].get("effects") or []:
        for rx, kind in GENERIC:
            m = rx.match(e)
            if m:
                vals = KINDS[kind][1]
                out.append((kind, 1 + min(range(4), key=lambda i: abs(vals[i] - 10 * float(m.group(1))))))
    return out


def research_conditions(r):
    types = [TYPE_JP.get(t) for t in r["value"].get("condition_attributes_jp") or []]
    if None in types:
        return None
    c = collections.Counter(types)
    return [{"statueType": t, "count": n} for t, n in sorted(c.items())]


def research_page(r):
    url = r["url"].split(" (")[0]
    rev = re.search(r"\(revid (\d+)", r["url"])
    return url, (int(rev.group(1)) if rev else None)


out, by_src = [], collections.Counter()
for s in statues:
    sid = s["statueId"]
    w = by_page.get(s.get("source", {}).get("page"))
    if w is None:
        n = norm(labels.get(f"text:StatueName_JP:STATUE_NAME_{sid}"))
        key = (n, s.get("rarity"), s.get("statueType"))
        if s.get("source", {}).get("source") != "jdr-default" and len(by_key.get(key, [])) == 1 \
                and ids_by_key.get(key) == [sid]:
            w = by_key[key][0]
    kinds = wiki_kinds(w) if w else []
    if kinds:
        src = {"source": w["source"]["site"], "page": w["source"]["page"], "revid": w["source"].get("revid"),
               "license": w["source"].get("license"), "accessed": "2026-11-15",
               "note":"assist-ability stat and value only (action-battle era); tier = nearest value"}
        by_src["shoutwiki"] += 1
    else:
        kinds = [(lean(s), max(1, min(4, (s.get("rarity") or 4) - 2)))]
        src = {"source": "jdr-default", "note": "no wiki assist ability: the statue's leaning stat, tier = rarity - 2"}
        by_src["default"] += 1
    conditions = []
    r = research.get(sid)
    if r and not r.get("conflict"):
        rk, rc = research_kinds(r), research_conditions(r)
        page, revid = research_page(r)
        site = ("Diamond Records Wiki (ShoutWiki)" if "shoutwiki" in page else
                "JoJo's Bizarre Encyclopedia (JoJoWiki)" if "jojowiki" in page else
                "Gamerch ジョジョDR攻略Wiki (Wayback Machine)" if "gamerch" in page else None)
        if site and (rk or rc):
            if rk and src["source"] == "jdr-default":
                kinds = rk
            if rc is not None:
                conditions = rc
            src = {"source": site, "page": page, **({"revid": revid} if revid else {}),
                   **({"license": "CC BY-SA 3.0"} if "shoutwiki" in page else
                      {"license": "CC BY-SA 4.0"} if "jojowiki" in page else {}),
                   "accessed": r.get("accessed") or "2026-11-14",
                   "note": ("condition and " if rc else "") +
                           ("stat and value" if rk else "no tactical affect for the effect: ours")
                           + " (action-battle era); tier = nearest value"}
            by_src["research"] += 1
    if sid in OWN:
        # Assist-only: its own label; the first kind/tier gives the values (ours).
        k, t = kinds[0]
        affect, values = KINDS[k]
        p = {"affectId": affect, "giveLimit": -1,
             **{f"paramLv1_{i}": values[t - 1] if i == 0 else 0 for i in range(8)},
             **{f"paramLv10_{i}": values[t - 1] if i == 0 else 0 for i in range(8)}}
        own_rows.append({"abilityId": f"DT_AST_{sid}", "abilityName": f"ABI_AST_{sid}_N",
                         "abilityParamList": [p]})
        ids = [f"DT_AST_{sid}"]
        by_src["assist_only"] += 1
    else:
        ids = [row_id(k, t) for k, t in kinds]
    out.append({"statueId": sid, "abilities": [{"abilityId": i, "conditions": conditions} for i in ids],
                "source": src})

rows += own_rows
doc["data"]["paramList"] = rows
json.dump(doc, open(a.table, "w"), ensure_ascii=False, indent=1)
open(a.table, "a").write("\n")

json.dump({
    "about": "Assist abilities per statue (docs/research/19-assists.md §5, §6): ids of the shared "
             "dtbattle_ability_data_table rows DT_AST_<kind><tier> (an assist-only statue has its own "
             "row DT_AST_<statueId>), with the client's activation "
             "conditions ([{statueType, count}] over the seat's assists; empty = always). Built by "
             "tools/content/build_assist_abilities.py.",
    "sources": [
        {"source": "Diamond Records Wiki (ShoutWiki)", "page": "https://jojodiamondrecords.shoutwiki.com/",
         "license": "CC BY-SA 3.0", "accessed": "2026-11-15"},
        {"source": "jdr-research", "doc": "docs/research/19-assists.md"},
        {"source": "jdr-default", "note": "statues without wiki assist numbers: the leaning stat at tier rarity - 2; conditions empty"},
        {"source": "jdr-default", "note": "assist-only statues: their own row under their own ABI_AST_<statueId>_N label, values of the kind and tier above"},
    ],
    "statues": out,
}, open(a.out, "w"), ensure_ascii=False, indent=1)
open(a.out, "a").write("\n")
print(len(rows), "ability rows;", len(out), "statues", dict(by_src))
