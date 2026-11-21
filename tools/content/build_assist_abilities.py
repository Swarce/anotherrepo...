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
ABI_AST_<statueId>_N belong to assist-only statues outside the roster (except 2100_03, whose
effect is unknown), so they are not used.

Conditions: none (always active). The client's condition rows ([{statueType, count}]) are the
server's assist_ability_condition_table, generated from the content file.

Idempotent: replaces any DT_AST_* rows and keeps every other row as it is.

usage: python3 -I tools/content/build_assist_abilities.py \
         --shoutwiki local/wiki-raw/shoutwiki/statues.json --labels local/l10n/work23b.tsv \
         [--statues server/content/statues.json] \
         [--table server/content/masterdata/dtbattle_ability_data_table.json] \
         [--out server/content/assist_abilities.json]
"""
import argparse, collections, json

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
doc["data"]["paramList"] = rows
note = ("Assist abilities DT_AST_<KIND><tier> (tools/content/build_assist_abilities.py): names are the "
        "game's ABI_AST_<KIND><tier>_N label keys; values per tier follow the ShoutWiki assist numbers "
        "where it has them (HP 5/6/10%, ATK 4/6/8/20%, DEF 6/8%, damage dealt 3/5/7/18%), the rest are ours; "
        "Lv10 = Lv1")
if not any(s.get("note") == note for s in doc["sources"]):
    doc["sources"] = [s for s in doc["sources"] if "DT_AST" not in s.get("note", "")]
    doc["sources"].append({"source": "jdr-default", "note": note})
if "DT_AST" not in doc["about"]:
    doc["about"] += " Rows DT_AST_<kind><tier> are the assist abilities (docs/research/19-assists.md)."
json.dump(doc, open(a.table, "w"), ensure_ascii=False, indent=1)
open(a.table, "a").write("\n")

# 2) Statue -> abilities.
statues = json.load(open(a.statues))["statues"]
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
    out.append({"statueId": sid, "abilities": [{"abilityId": row_id(k, t), "conditions": []} for k, t in kinds],
                "source": src})

json.dump({
    "about": "Assist abilities per statue (docs/research/19-assists.md §5): ids of the shared "
             "dtbattle_ability_data_table rows DT_AST_<kind><tier>, with the client's activation "
             "conditions ([{statueType, count}] over the seat's assists; empty = always). Built by "
             "tools/content/build_assist_abilities.py.",
    "sources": [
        {"source": "Diamond Records Wiki (ShoutWiki)", "page": "https://jojodiamondrecords.shoutwiki.com/",
         "license": "CC BY-SA 3.0", "accessed": "2026-11-15"},
        {"source": "jdr-research", "doc": "docs/research/19-assists.md"},
        {"source": "jdr-default", "note": "statues without wiki assist numbers: the leaning stat at tier rarity - 2; conditions empty"},
    ],
    "statues": out,
}, open(a.out, "w"), ensure_ascii=False, indent=1)
open(a.out, "a").write("\n")
print(len(rows), "ability rows;", len(out), "statues", dict(by_src))
