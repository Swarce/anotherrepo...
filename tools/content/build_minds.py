#!/usr/bin/env python3
"""Build server/content/minds.json: minds (cards) from the Gamerch wiki matched to game ids by name
(the wiki's Japanese name against the game's own MIND_NAME_<id> label, after NFKC normalisation; a
name shared by several ids is skipped), with rarity, cost and flat HP/ATK/DEF from their effects.
Only minds whose image bundle (mim_<id>) exists in the player's files.

usage: python3 -I tools/content/build_minds.py --gamerch local/wiki-raw/gamerch/minds.json \
          --export local/l10n/work.tsv --data <files dir> --keys local/keys.json \
          --tool target/release/jdr-tool --out server/content/minds.json
"""
import argparse
import datetime
import json
import re
import subprocess
import unicodedata

ap = argparse.ArgumentParser()
for x in ("--gamerch", "--export", "--data", "--keys", "--tool", "--out"):
    ap.add_argument(x, required=True)
a = ap.parse_args()
names = {l.split("\t")[0] for l in subprocess.run([a.tool, "names", "--data", a.data, "--keys", a.keys],
                                                  check=True, capture_output=True, text=True).stdout.splitlines()}


def norm(x):
    return unicodedata.normalize("NFKC", x or "").replace(" ", "").replace("\u3000", "")


ids_by_name = {}
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    m = re.fullmatch(r"text:MindName_JP:MIND_NAME_(\d{4})", c[0]) if len(c) >= 4 else None
    if m:
        ids_by_name.setdefault(norm(c[3]), set()).add(m.group(1))
doc = json.load(open(a.gamerch))
recs = doc if isinstance(doc, list) else doc["minds"]


def flat(effects, stat):
    for e in effects or []:
        if e.get("stat") == stat and not e.get("basis"):
            m = re.fullmatch(r"\+?(\d+)", str(e.get("value") or "").replace(",", ""))
            if m:
                return int(m.group(1))
    return 0


# Cost by rarity: the wiki lists a cost for some Minds, and every Mind of a rarity with a known cost
# has the same one (1: 3, 2: 5, 3: 8, 4: 12 in the 2026-11-14 capture). Minds without one get it.
by_rarity = {}
for r in recs:
    rr = re.sub(r"\D", "", str(r.get("rarity") or ""))
    if isinstance(r.get("cost"), int) and rr:
        by_rarity.setdefault(int(rr), set()).add(r["cost"])
COST = {k: next(iter(v)) for k, v in by_rarity.items() if len(v) == 1}

out, seen = [], set()
for r in recs:
    ids = ids_by_name.get(norm(r.get("name_jp")), set())
    mid = next(iter(ids)) if len(ids) == 1 else None
    if not mid or mid in seen or f"mim_{mid}" not in names:
        continue
    seen.add(mid)
    rarity = int(str(r.get("rarity") or "☆3").strip("☆") or 3)
    out.append({
        "mindId": mid,
        "rarity": rarity,
        "cost": r["cost"] if isinstance(r.get("cost"), int) else COST.get(rarity, rarity * 3),
        "hp": flat(r.get("effects"), "HP"),
        "atk": flat(r.get("effects"), "ATK"),
        "def": flat(r.get("effects"), "DEF"),
        "source": {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": r["source"]["page_url"],
                   "accessed": r["source"].get("accessed"),
                   **({} if isinstance(r.get("cost"), int) else
                      {"note": "cost of every Mind of this rarity on the wiki" if rarity in COST
                       else "cost = 3 x rarity (jdr-default)"})},
    })
out.sort(key=lambda m: m["mindId"])
json.dump({"about": "Minds matched to game ids on the Gamerch wiki: rarity, cost, flat stats (numbers only). See docs/CONTENT.md.",
           "sources": [{"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine snapshot)", "page": "https://jojodr.gamerch.com/",
                        "accessed": datetime.date.today().isoformat()}],
           "minds": out}, open(a.out, "w"), indent=1, ensure_ascii=False)
print(f"{len(out)} minds")
