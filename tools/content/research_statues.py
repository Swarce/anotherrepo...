#!/usr/bin/env python3
"""Turn a research pass's statue facts (one JSON record per fact: statueId, field, value, url,
capture, accessed, confidence) into a research collection for build_statues.py --extra.

Per statue and field, the value of the highest confidence wins; when records of that confidence
disagree, the field is left out (so a statue with conflicting Lv99 stats, rarity or attribute is
not used). A statue qualifies with all of Lv99 HP/ATK/DEF, rarity and attribute at medium
confidence or better. Lv1 is the wiki's value when it gives one, else Lv99 / 10 rounded, as for the
Gamerch pages (the wikis' Lv1 is always Lv99 / 10). The cited page is that of the Lv99 HP record
(a MediaWiki revision as its permanent `?oldid=` link).

Later passes (`--facts` again, Build 43: research-b43/statues) add verdict records:
  - `attribute_conflict` ({..., verdict}): the researchers' type for a statue whose sources disagree
    (3041_04: 知略); it replaces the attribute pick and is cited as `typeSource`;
  - `lv1_conflict` ({..., verdict}): the Lv1 stat the researchers keep where the pages differ; it goes
    to `checks` (build_statues.py notes it on the statue and changes no value);
  - `identity` ({page_is}): the only public page of this statueId is another id's, so this id has no
    source of its own and is left out whatever its other records say;
  - `assist_only` (true): the statue is assist-only (`assistOnly` in the collection; build_statues.py
    gives research stats to an assist-only statue only with this);
  - a dict value of `lv1`, `lv99` or `maxTree` counts as one record per stat.
Records without a statueId (the researchers left the id open) are not read.
Build 44 (research-b44/<family>, files that mix topics: only topic `statues` is read) adds:
  - `dt_battle_params` / `tactical_params` ({speed, evasion, accuracy, critical[, resist]}): the
    statue's tactical parameters (`tactical` in the collection, accuracy as the client's `hitrate`;
    the highest confidence wins, agreeing records are cited as `also`, an equal-confidence
    disagreement is dropped); the resistance grades are kept as data;
  - `<stat>_source_says` (a hand-typed table, medium): listed as `sourceSays`, never applied;
  - `identity_type` / `identity_type_derived` (the Giorno 5000_03/04 vs 23/24 split): counted as
    held, not applied; the ids stay as they are.

usage: python3 -I tools/content/research_statues.py --facts local/wiki-raw/research-b33/statues/results.jsonl \
         [--facts local/wiki-raw/research-b43/statues/results.jsonl] \
         [--facts local/wiki-raw/research-b44/<family>/results.jsonl ...] --out <collection.json>
  (Build 33 wrote local/wiki-raw/research-b33/statues/extra_statues.json from the b33 file alone;
  Build 43 reads both files, see docs/CONTENT.md "Research collections")
"""
import argparse
import collections
import json
import re

ap = argparse.ArgumentParser()
ap.add_argument("--facts", required=True, action="append")
ap.add_argument("--out", required=True)
a = ap.parse_args()

RANK = {"high": 3, "medium": 2, "low": 1}
facts = collections.defaultdict(lambda: collections.defaultdict(list))
for path in a.facts:
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if r.get("statueId") is None or r.get("topic", "statues") != "statues":
            continue
        if r["field"] in ("lv1", "lv99", "maxTree") and isinstance(r["value"], dict):
            for k in ("hp", "atk", "def"):
                if isinstance(r["value"].get(k), int):
                    facts[r["statueId"]][f"{r['field']}.{k}"].append({**r, "field": f"{r['field']}.{k}", "value": r["value"][k]})
            continue
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


def cite(r):
    """A record's page as a source: url (a MediaWiki revision as its `?oldid=` link), capture, accessed."""
    url = r["url"].split(" (")[0]
    rev = re.search(r"\(revid (\d+)\)", r["url"])
    if rev:
        url += ("&" if "?" in url else "?") + "oldid=" + rev.group(1)
    return {"url": url, **({"capture": r["capture"]} if r.get("capture") else {}), "accessed": r.get("accessed")}


def verdict(recs):
    """The verdict of a conflict record (medium or high; the one of highest confidence), or None."""
    recs = [r for r in recs if RANK.get(r.get("confidence"), 0) >= 2 and isinstance(r["value"], dict)
            and "verdict" in r["value"]]
    return max(recs, key=lambda r: RANK[r["confidence"]]) if recs else None


TACTICAL = (("speed", "speed"), ("evasion", "evasion"), ("accuracy", "hitrate"), ("critical", "criticalRate"))


def tactical(sid, recs):
    """The tactical speed/evasion/accuracy/critical of a statue (Build 44): the record of the highest
    confidence, the others that agree as `also`; None when records of that confidence disagree."""
    recs = sorted((r for r in recs if RANK.get(r.get("confidence"), 0) >= 2), key=lambda r: -RANK[r["confidence"]])
    if not recs:
        return None
    key = lambda r: tuple(r["value"].get(k) for k, _ in TACTICAL)
    top = recs[0]
    if any(key(r) != key(top) and r["confidence"] == top["confidence"] for r in recs[1:]) \
            or not all(isinstance(v, int) for v in key(top)):
        held["tactical params disagree"] += 1
        return None
    resist = next((r["value"]["resist"] for r in recs if key(r) == key(top) and "resist" in r["value"]), None)
    return {"statueId": sid, "value": {name: top["value"][k] for k, name in TACTICAL},
            **({"resist": resist} if resist else {}),
            "source": {**cite(top), "confidence": top["confidence"]},
            **({"also": [{**cite(r), "confidence": r["confidence"]} for r in recs[1:] if key(r) == key(top)]}
               if len(recs) > 1 else {})}


records, checks, skipped = [], [], collections.Counter()
tactical_rows, tree_checks, held = [], [], collections.Counter()
for sid, fields in sorted(facts.items()):
    pick = {f: best(rs) for f, rs in fields.items()}
    t = tactical(sid, fields.get("dt_battle_params", []) + fields.get("tactical_params", []))
    if t:
        tactical_rows.append(t)
    for f in ("maxTree", "lv99", "lv1"):
        for r in fields.get(f + "_source_says", []):
            tree_checks.append({"statueId": sid, "field": f, "value": r["value"], "source": cite(r),
                                "confidence": r["confidence"], "note": r.get("note")})
    for f in ("identity_type", "identity_type_derived"):
        held[f + " (the id split stays open; not applied)"] += len(fields.get(f, []))
    for r in fields.get("lv1_conflict", []):
        v = verdict([r])
        if v:
            stat = next(k for k in ("hp", "atk", "def") if any(x.endswith("_" + k) for x in v["value"]))
            pages = {site: v["value"][f"{site}_{stat}"] for site in ("gamerch_live", "shoutwiki")
                     if isinstance(v["value"].get(f"{site}_{stat}"), int)}
            checks.append({"statueId": sid, "field": f"lv1.{stat}", "value": v["value"]["verdict"], "pages": pages,
                           "source": cite(v), "confidence": v["confidence"], "note": v.get("note")})
    if any(RANK.get(r.get("confidence"), 0) >= 2 and r["value"].get("page_is") not in (None, sid)
           for r in fields.get("identity", [])):
        skipped["no source of its own (identity record)"] += 1
        continue
    type_verdict = verdict(fields.get("attribute_conflict", []))
    if type_verdict:
        pick["attribute"] = {**type_verdict, "value": type_verdict["value"]["verdict"]}
    assist = best([r for r in fields.get("assist_only", []) if RANK.get(r.get("confidence"), 0) >= 2])
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
        **({"typeSource": {**cite(type_verdict), "confidence": type_verdict["confidence"],
                           "votes": {k: v for k, v in type_verdict["value"].items() if k != "verdict"},
                           "note": type_verdict.get("note")}} if type_verdict else {}),
        **({"assistOnly": True, "assistOnlySource": cite(assist)} if assist and assist["value"] is True else {}),
    })
json.dump({"collected": max((r["source"].get("accessed") or "" for r in records), default=""),
           "records": records, **({"checks": checks} if checks else {}),
           **({"tactical": tactical_rows} if tactical_rows else {}),
           **({"sourceSays": tree_checks} if tree_checks else {})},
          open(a.out, "w"), ensure_ascii=False, indent=1)
print(f"{len(records)} statues with stats, rarity and attribute (medium or high); {len(checks)} Lv1 checks; "
      f"{len(tactical_rows)} tactical parameter rows; {len(tree_checks)} 'source says' values; "
      f"skipped: {dict(skipped)}; held: {dict(held)}")
