#!/usr/bin/env python3
"""Build server/content/dt_cool_turns.json: documented tactical cool turns (at the highest skill
level, the value players listed) and uses per skill id, from the research pass's tactical statue
records (the tactical-era atwiki and the Bahamut forum, 2019). A skill is matched by its name
against the game's own SKILL_NAME_<p>_<NN> labels of the statue's 3-digit prefix, after NFKC
normalisation; names the labels do not have (translated ones) are skipped. Ids and numbers only.

usage: python3 -I tools/content/build_dt_cool_turns.py --research local/wiki-raw/research-b33/tactical/results.jsonl \
         --export local/l10n/export-b33.tsv --out server/content/dt_cool_turns.json
"""
import argparse
import json
import re
import unicodedata

ap = argparse.ArgumentParser()
ap.add_argument("--research", required=True)
ap.add_argument("--export", required=True)
ap.add_argument("--out", required=True)
a = ap.parse_args()


def norm(s):
    s = unicodedata.normalize("NFKC", s or "")
    return re.sub(r"[\s・…\.。、!！?？「」『』〜~ー\-\[\]()（）]", "", s)


names = {}
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    m = re.fullmatch(r"text:SkillName_JP:SKILL_NAME_(\d{3})_(\d{2})", c[0]) if len(c) >= 4 else None
    if m:
        names.setdefault(m.group(1), {})[norm(c[3])] = f"SKILL_{m.group(1)}_{m.group(2)}"

out, unmatched = {}, 0
for line in open(a.research, encoding="utf-8"):
    r = json.loads(line)
    if r.get("topic") != "skills" or r.get("field") != "skills" or not r.get("statueId"):
        continue
    prefixes = {sid[:3] for sid in r["statueId"].split("|")}
    for seg in str(r["value"]).split(";"):
        ct = re.search(r"\bCT\s*(\d+)", seg)
        if not ct:
            continue
        uses = re.search(r"\buses\s*(\d+)", seg)
        name = re.split(r"\s(?:DR|CT|uses)\s*\d", seg)[0]
        name = re.sub(r"^\s*(?:normal|SP|blue|red|deadly|blue Mind|red Mind)\s+", "", name.strip())
        hit = None
        for p in prefixes:
            hit = names.get(p, {}).get(norm(name))
            if hit:
                break
        if not hit:
            unmatched += 1
            continue
        out[hit] = {"coolTurn": int(ct.group(1)), **({"uses": int(uses.group(1))} if uses else {}),
                    "page": r["url"], "accessed": r.get("accessed")}
json.dump({"about": "Documented tactical cool turns at the highest skill level, and uses, per skill id "
                    "(tools/content/build_dt_cool_turns.py): the tactical-era atwiki ジョジョDR２ and the Bahamut "
                    "forum (2019), matched by skill name to the game's labels. The server sends them as "
                    "dtMaxCoolTurn (level 10); see docs/research/26 §4.",
           "skills": dict(sorted(out.items()))}, open(a.out, "w"), ensure_ascii=False, indent=1)
open(a.out, "a").write("\n")
print(len(out), "skills;", unmatched, "names not in the game's labels")
