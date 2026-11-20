#!/usr/bin/env python3
"""Turn a research pass's statue facts (one JSON record per fact: statueId, field, value, url,
capture, accessed, confidence) into a research collection for build_statues.py --extra.

Per statue and field, the value of the highest confidence wins; when records of that confidence
disagree, the field is left out (so a statue with conflicting Lv99 stats, rarity or attribute is
not used). A statue qualifies with all of Lv99 HP/ATK/DEF, rarity and attribute at medium
confidence or better. Lv1 is the wiki's value when it gives one, else Lv99 / 10 rounded, as for the
Gamerch pages (the wikis' Lv1 is always Lv99 / 10). The cited page is that of the Lv99 HP record
(a MediaWiki revision as its permanent `?oldid=` link).

usage: python3 -I tools/content/research_statues.py --facts local/wiki-raw/research-b33/statues/results.jsonl \
         --out local/wiki-raw/research-b33/statues/extra_statues.json
"""
import argparse
import collections
import json
import re

ap = argparse.ArgumentParser()
ap.add_argument("--facts", required=True)
ap.add_argument("--out", required=True)
a = ap.parse_args()

RANK = {"high": 3, "medium": 2, "low": 1}
facts = collections.defaultdict(lambda: collections.defaultdict(list))
for line in open(a.facts, encoding="utf-8"):
    r = json.loads(line)
    facts[r["statueId"]][r["field"]].append(r)


def best(recs):
    """The record of the highest confidence, or None when those disagree."""
    if not recs:
        return None
    top = max(RANK.get(r.get("confidence"), 0) for r in recs)
    tops = [r for r in recs if RANK.get(r.get("confidence"), 0) == top]
    if len({json.dumps(r["value"], sort_keys=True) for r in tops}) != 1:
        return None
    return tops[0]


records, skipped = [], collections.Counter()
for sid, fields in sorted(facts.items()):
    pick = {f: best(rs) for f, rs in fields.items()}
    need = ["lv99.hp", "lv99.atk", "lv99.def", "rarity", "attribute"]
    if any(pick.get(f) is None for f in need):
        skipped["missing or conflicting " + ",".join(f for f in need if pick.get(f) is None)] += 1
        continue
    conf = min(RANK[pick[f]["confidence"]] for f in need)
    if conf < 2:
        skipped["low confidence"] += 1
        continue
    lv99 = {k: int(pick[f"lv99.{k}"]["value"]) for k in ("hp", "atk", "def")}
    lv1 = {k: int(pick[f"lv1.{k}"]["value"]) if pick.get(f"lv1.{k}") else round(lv99[k] / 10)
           for k in ("hp", "atk", "def")}
    tree = {k: int(pick[f"maxTree.{k}"]["value"]) for k in ("hp", "atk", "def") if pick.get(f"maxTree.{k}")}
    src = pick["lv99.hp"]
    url = src["url"].split(" (")[0]
    rev = re.search(r"\(revid (\d+)\)", src["url"])
    if rev:  # a MediaWiki page read at one revision: its permanent link
        url += ("&" if "?" in url else "?") + "oldid=" + rev.group(1)
    source = {"url": url, "accessed": src.get("accessed")}
    if src.get("capture"):
        source["archive_url"] = src["capture"] if str(src["capture"]).startswith("http") else None
        if source["archive_url"] is None:
            del source["archive_url"]
    records.append({
        "statueId": sid,
        "rarity": int(pick["rarity"]["value"]),
        "type": pick["attribute"]["value"],
        "stats": {"lv1": lv1, "lv99": lv99, **({"tree_max": tree} if len(tree) == 3 else {})},
        "source": source,
        "confidence": {3: "high", 2: "medium"}[conf],
    })
json.dump({"collected": max((r["source"].get("accessed") or "" for r in records), default=""),
           "records": records}, open(a.out, "w"), ensure_ascii=False, indent=1)
print(f"{len(records)} statues with stats, rarity and attribute (medium or high); skipped: {dict(skipped)}")
