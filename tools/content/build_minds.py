#!/usr/bin/env python3
"""Build server/content/minds.json: minds (cards) matched to game ids on the Gamerch wiki
(exact Japanese name, matched locally when fetching), with rarity, cost and flat HP/ATK/DEF from
their effects. Only minds whose image bundle (mim_<id>) exists in the player's files.

usage: python3 -I tools/content/build_minds.py --gamerch local/wiki-raw/gamerch/minds.json \
          --data <files dir> --keys local/keys.json --tool target/release/jdr-tool --out server/content/minds.json
"""
import argparse
import datetime
import json
import re
import subprocess

ap = argparse.ArgumentParser()
for x in ("--gamerch", "--data", "--keys", "--tool", "--out"):
    ap.add_argument(x, required=True)
a = ap.parse_args()
names = {l.split("\t")[0] for l in subprocess.run([a.tool, "names", "--data", a.data, "--keys", a.keys],
                                                  check=True, capture_output=True, text=True).stdout.splitlines()}
doc = json.load(open(a.gamerch))
recs = doc if isinstance(doc, list) else doc["minds"]


def flat(effects, stat):
    for e in effects or []:
        if e.get("stat") == stat and not e.get("basis"):
            m = re.fullmatch(r"\+?(\d+)", str(e.get("value") or "").replace(",", ""))
            if m:
                return int(m.group(1))
    return 0


out, seen = [], set()
for r in recs:
    mid = r.get("mind_id")
    if not mid or mid in seen or f"mim_{mid}" not in names:
        continue
    seen.add(mid)
    rarity = int(str(r.get("rarity") or "☆3").strip("☆") or 3)
    out.append({
        "mindId": mid,
        "rarity": rarity,
        "cost": r["cost"] if isinstance(r.get("cost"), int) else rarity * 3,
        "hp": flat(r.get("effects"), "HP"),
        "atk": flat(r.get("effects"), "ATK"),
        "def": flat(r.get("effects"), "DEF"),
        "source": {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": r["source"]["page_url"],
                   "accessed": r["source"].get("accessed"),
                   **({} if isinstance(r.get("cost"), int) else {"note": "cost = 3 x rarity (jdr-default)"})},
    })
out.sort(key=lambda m: m["mindId"])
json.dump({"about": "Minds matched to game ids on the Gamerch wiki: rarity, cost, flat stats (numbers only). See docs/CONTENT.md.",
           "sources": [{"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine snapshot)", "page": "https://jojodr.gamerch.com/",
                        "accessed": datetime.date.today().isoformat()}],
           "minds": out}, open(a.out, "w"), indent=1, ensure_ascii=False)
print(f"{len(out)} minds")
