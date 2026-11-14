#!/usr/bin/env python3
"""Build server/content/statues.json: the obtainable statues with identifiers from the player's
own files (scan_statues.py) and numbers from the community wikis.

Matching (strict; anything ambiguous falls back to marked defaults):
  - a statueId is obtainable when it has a model + animations + action set AND a leader-ability label;
  - Gamerch statue pages attach by their leader-ability name (or, for assist-only statues, the
    assist-ability name), compared after NFKC normalisation with the game's own
    ABI_LEAD_NAME_<id> / ABI_AST_<id>_N labels; a name that maps to more than one statueId is skipped;
  - ShoutWiki entries attach by elimination: after the exact matches, when exactly one entry and
    exactly one statueId of that character remain, and the entry's rarity keeps rarity
    non-decreasing within the id's variant family (same tens digit of the suffix);
  - the tactical basic attack (`dtAttack`): `SKILL_<first 3 digits>_99` when the statue's first model
    has an ATTACK action set (the client maps skill number 99 to ATTACK), else its lowest SKILL_NN
    action; none when the model has no tactical actions;
  - a record is used only when every stat has 5 <= Lv99 / Lv1 <= 20 (the wikis show Lv99 ~ 10 x Lv1);
    anything else is a wiki typo and the statue keeps defaults.
Only numbers are written, each record citing its source page.

usage: python3 -I tools/content/build_statues.py --scan local/content/statues_scan.json \
         --export local/l10n/work.tsv --shoutwiki local/wiki-raw/shoutwiki/statues.json \
         --out server/content/statues.json
"""
import argparse
import collections
import datetime
import json
import re
import unicodedata

ap = argparse.ArgumentParser()
for x in ("--scan", "--export", "--shoutwiki", "--out"):
    ap.add_argument(x, required=True)
ap.add_argument("--gamerch", help="local/wiki-raw/gamerch/statues.json (matched by leader-ability name)")
a = ap.parse_args()

scan = json.load(open(a.scan))["statues"]
labels = {}
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    if len(c) >= 4:
        labels[c[0]] = c[3].replace("\\n", "\n")
lead = {k.rsplit("_", 2)[-2] + "_" + k.rsplit("_", 1)[-1] for k in labels if k.startswith("text:Ability_Leader_Name_JP:ABI_LEAD_NAME_")}
obtainable = [s for s in scan if s["playable"] and s["statueId"] in lead]
jp_name = {s["statueId"]: labels.get(f"text:StatueName_JP:STATUE_NAME_{s['statueId']}", "") for s in obtainable}

wiki = json.load(open(a.shoutwiki))
by_name = collections.defaultdict(list)
for w in wiki:
    if w.get("jp_name"):
        by_name[w["jp_name"].replace(" ", "").replace("・", "")].append(w)
ids_by_name = collections.defaultdict(list)
for sid, n in jp_name.items():
    ids_by_name[n.replace(" ", "").replace("・", "")].append(sid)

STATUE_TYPE = {"Courage": 1, "Tactical": 2, "Fighting Spirit": 3, "Unity": 4, "Solitary": 5}
GAMERCH_TYPE = {"勇気": 1, "知略": 2, "闘志": 3, "団結": 4, "孤高": 5}
TODAY = datetime.date.today().isoformat()


def norm(x):
    return unicodedata.normalize("NFKC", x or "").replace(" ", "").replace("\u3000", "")


def family(sid):
    return (sid[:4], sid[5:6])


def plausible(lv1, lv99):
    return all(lv1.get(k) and 5 <= lv99[k] / lv1[k] <= 20 for k in ("hp", "atk", "def"))


rejected = []


def dt_attack(s):
    acts = s.get("dtActions") or []
    if "ATTACK" in acts:
        return f"SKILL_{s['statueId'][:3]}_99"
    nums = sorted(int(x[6:]) for x in acts if re.fullmatch(r"SKILL_\d\d", x))
    return f"SKILL_{s['statueId'][:3]}_{nums[0]:02d}" if nums else None



def full(x):
    return isinstance(x, dict) and all(isinstance(x.get(k), int) for k in ("hp", "atk", "def"))


# 1) Gamerch: exact match by leader-ability (or assist-ability) name against the game's labels.
by_ability = collections.defaultdict(set)
for k, v in labels.items():
    m = re.fullmatch(r"text:Ability_Leader_Name_JP:ABI_LEAD_NAME_(\d{4}_\d{2})", k) or \
        re.fullmatch(r"text:Ability_Assist_Name_JP:ABI_AST_(\d{4}_\d{2})_N", k)
    if m and m.group(1) in jp_name:
        by_ability[norm(v)].add(m.group(1))
gamerch = {}
if a.gamerch:
    for g in json.load(open(a.gamerch))["statues"]:
        if not full(g.get("lv99")):
            continue
        ids = by_ability.get(norm(g.get("leader_ability_jp")), set()) or \
            by_ability.get(norm(g.get("assist_ability_jp")), set())
        if len(ids) != 1:
            continue
        sid = next(iter(ids))
        lv1 = g["lv1"] if full(g.get("lv1")) else {k: round(v / 10) for k, v in g["lv99"].items()}
        if not plausible(lv1, g["lv99"]):
            rejected.append(sid)
            continue
        gamerch[sid] = g
out = []
matched = 0
by_source = collections.Counter()
claimed = collections.defaultdict(set)  # character key -> {(rarity, type)} taken by exact matches
for sid, g in gamerch.items():
    if sid in jp_name:
        claimed[jp_name[sid].replace(" ", "").replace("・", "")].add((g.get("rarity"), GAMERCH_TYPE.get(g.get("type"))))
remaining_ids = collections.defaultdict(list)
for sid, n in jp_name.items():
    if sid not in gamerch:
        remaining_ids[n.replace(" ", "").replace("・", "")].append(sid)
for s in obtainable:
    sid = s["statueId"]
    rec = {
        "statueId": sid,
        "part": s["part"],
        "modelDataList": s["modelDataList"],
        "dtAttack": dt_attack(s),
        "rarity": None,
        "statueType": None,
        "lv1": None,
        "lv99": None,
        "maxTree": None,
        "source": {"source": "jdr-default", "note": "no unambiguous wiki match yet; server uses defaults"},
    }
    key = jp_name[sid].replace(" ", "").replace("・", "")
    g = gamerch.get(sid)
    if g:
        lv99 = g["lv99"]
        lv1 = g["lv1"] if full(g.get("lv1")) else {k: round(v / 10) for k, v in lv99.items()}
        rec.update({
            "rarity": g.get("rarity"),
            "statueType": GAMERCH_TYPE.get(g.get("type")),
            "lv1": lv1,
            "lv99": lv99,
            "maxTree": g["max_tree"] if full(g.get("max_tree")) else None,
            "source": {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": g["source"]["page_url"],
                       "capture": g["source"].get("capture"), "accessed": g["source"].get("accessed", TODAY),
                       **({} if full(g.get("lv1")) else {"note": "Lv1 = Lv99 / 10 (linear, as the wikis show)"})},
        })
        matched += 1
        by_source["gamerch"] += 1
        out.append(rec)
        continue
    # 2) ShoutWiki: entries of this character whose (rarity, type) no exact match claimed, when
    #    exactly one such entry and exactly one unmatched statueId remain.
    ws = [w for w in by_name.get(key, []) if (w["rarity"], STATUE_TYPE.get(w["type"])) not in claimed[key]]
    if len(remaining_ids[key]) == 1 and len(ws) == 1:
        w = ws[0]
        siblings = [(o, gamerch[o].get("rarity")) for o in gamerch if family(o) == family(sid)]
        ordered = all((o < sid) == (r <= w["rarity"]) or r == w["rarity"] for o, r in siblings if r)
        if not (all(w["lv1"].values()) and all(w["lv99"].values())):
            pass
        elif not ordered or not plausible(w["lv1"], w["lv99"]):
            rejected.append(sid)
        else:
            rec.update({
                "rarity": w["rarity"],
                "statueType": STATUE_TYPE.get(w["type"]),
                "lv1": w["lv1"],
                "lv99": w["lv99"],
                "maxTree": w["max"] if all(w["max"].values()) else None,
                "source": {"source": w["source"]["site"], "license": w["source"]["license"],
                           "page": w["source"]["page"], "revid": w["source"]["revid"],
                           "accessed": datetime.date.today().isoformat()},
            })
            matched += 1
            by_source["shoutwiki"] += 1
    out.append(rec)

doc = {
    "about": "Obtainable statues: identifiers from the player's own files (scan_statues.py), stats from community wikis where the match is unambiguous (each record cites its page). See docs/CONTENT.md.",
    "sources": [
        {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine snapshot)", "page": "https://jojodr.gamerch.com/",
         "accessed": datetime.date.today().isoformat()},
        {"source": "Diamond Records Wiki (ShoutWiki)", "page": "https://jojodiamondrecords.shoutwiki.com/", "license": "CC BY-SA 3.0",
         "accessed": datetime.date.today().isoformat()},
        {"source": "jdr-default", "note": "identifiers and model names from the player's files; defaults where no wiki match"},
    ],
    "statues": out,
}
json.dump(doc, open(a.out, "w"), indent=1, ensure_ascii=False)
print(f"{len(out)} obtainable statues; {matched} with wiki stats ({dict(by_source)}), {len(out) - matched} on defaults; "
      f"rejected as implausible or out of rarity order: {sorted(set(rejected))}")
