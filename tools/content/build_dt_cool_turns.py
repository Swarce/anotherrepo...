#!/usr/bin/env python3
"""Build server/content/dt_cool_turns.json: documented tactical cool turns (at the highest skill
level, the value players listed) and uses per skill id, from the research pass's tactical statue
records (the tactical-era atwiki and the Bahamut forum, 2019). A skill is matched by its name
against the game's own SKILL_NAME_<p>_<NN> labels of the statue's 3-digit prefix, after NFKC
normalisation; names the labels do not have (translated ones) are skipped. Ids and numbers only.

Build 44 (--research-b44, each research-b44 family file; records of topic action_skills, high or
medium): a dt_skill_uses_cooldown record names its skill label itself (SKILL_<p>_<NN>, checked against
SkillName_JP) with the tactical cool-down and uses at the highest skill level (a 2019 Reddit
retrospective by a long-time translator, medium); a 'skill:<name>' record (the atwiki's reader
comments) is matched by name to the labels of its statue's prefix (a trailing ッ dropped; failing
that, build_action_skills.py's fuzzy rule: ratio >= 0.80, runner-up under 0.60). A skill
the research-b33 pass already gives keeps that value (a b44 record that agrees is listed under
"also"; one that disagrees is reported); a second b44 record for the same skill is listed under
"also" when it agrees. USES_HELD keeps a skill's uses out where the sources disagree on them.

usage: python3 -I tools/content/build_dt_cool_turns.py --research local/wiki-raw/research-b33/tactical/results.jsonl \
         [--research-b44 local/wiki-raw/research-b44/<family>/results.jsonl ...] \
         --export local/l10n/export-b33.tsv --out server/content/dt_cool_turns.json
"""
import argparse
import difflib
import json
import re
import unicodedata

ap = argparse.ArgumentParser()
ap.add_argument("--research", required=True)
ap.add_argument("--research-b44", action="append", default=[])
ap.add_argument("--export", required=True)
ap.add_argument("--out", required=True)
a = ap.parse_args()


def norm(s):
    s = unicodedata.normalize("NFKC", s or "")
    return re.sub(r"[\s・…\.。、!！?？「」『』〜~ー\-\[\]()（）]", "", s)


# Uses held where the sources disagree (Build 44): the cool-down they share is kept, the uses are not
# sent, so the server's default applies.
USES_HELD = {
    "SKILL_521_05": "uses held: 2 in the Reddit retrospective, 3 on the ShoutWiki tactical template (the "
                    "2026-11-15 pack); both give cool-down 3",
}

names, label_ids = {}, set()
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    m = re.fullmatch(r"text:SkillName_JP:SKILL_NAME_(\d{3})_(\d{2})", c[0]) if len(c) >= 4 else None
    if m:
        names.setdefault(m.group(1), {})[norm(c[3])] = f"SKILL_{m.group(1)}_{m.group(2)}"
        label_ids.add(f"SKILL_{m.group(1)}_{m.group(2)}")

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


def where(r):
    return {"page": r["url"], **({"capture": r["capture"]} if r.get("capture") else {}), "accessed": r.get("accessed")}


b33 = set(out)
b44_new, b44_also, b44_disagree, b44_unmatched = 0, 0, [], 0
for path in a.research_b44:
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if r.get("topic") != "action_skills" or r.get("confidence") not in ("high", "medium") \
                or not r.get("statueId") or not isinstance(r.get("value"), dict):
            continue
        v, f = r["value"], r.get("field") or ""
        if f == "dt_skill_uses_cooldown":
            hit, ct, uses = v.get("skill"), v.get("coolTurn"), v.get("uses")
            if hit not in label_ids or not hit.startswith(f"SKILL_{r['statueId'][:3]}_"):
                hit = None
        elif f.startswith("skill:"):
            nm = re.sub("ッ$", "", norm(f[len("skill:"):]))
            cands = {re.sub("ッ$", "", k): i for k, i in names.get(r["statueId"][:3], {}).items()}
            hit = cands.get(nm)
            if not hit:
                # build_action_skills.py's fuzzy rule: ratio >= 0.80 with the runner-up under 0.60.
                sc = sorted(((difflib.SequenceMatcher(None, nm, k).ratio(), i) for k, i in cands.items()), reverse=True)
                if sc and sc[0][0] >= 0.8 and (len(sc) < 2 or sc[1][0] < 0.6):
                    hit = sc[0][1]
            ct, uses = v.get("CT"), v.get("uses")
        else:
            continue
        if not hit or not isinstance(ct, int):
            b44_unmatched += 1
            continue
        uses = uses if isinstance(uses, int) else None
        cur = out.get(hit)
        if cur is not None:
            if cur["coolTurn"] == ct and cur.get("uses", uses if hit in USES_HELD else None) == uses:
                if all(x["page"] != r["url"] for x in [cur] + cur.get("also", [])):
                    cur.setdefault("also", []).append({**where(r), "confidence": r["confidence"]})
                    b44_also += 1
            else:
                b44_disagree.append(f"{hit} ({'research-b33' if hit in b33 else 'research-b44'} kept)")
            continue
        out[hit] = {"coolTurn": ct, **({"uses": uses} if uses is not None and hit not in USES_HELD else {}),
                    **where(r), "confidence": r["confidence"],
                    **({"note": USES_HELD[hit]} if hit in USES_HELD else {})}
        b44_new += 1
json.dump({"about": "Documented tactical cool turns at the highest skill level, and uses, per skill id "
                    "(tools/content/build_dt_cool_turns.py): the tactical-era atwiki ジョジョDR２ and the Bahamut "
                    "forum (2019), matched by skill name to the game's labels; from Build 44 also a 2019 Reddit "
                    "retrospective (r/JoJoDiamondRecords, a translator's tactical kits at max level, medium) that "
                    "names each skill's label, and atwiki reader comments read from the Wayback captures of its RSS "
                    "feeds (capture). The server sends them as dtMaxCoolTurn (level 10); see docs/research/26 §4.",
           "skills": dict(sorted(out.items()))}, open(a.out, "w"), ensure_ascii=False, indent=1)
open(a.out, "a").write("\n")
print(len(out), "skills;", unmatched, "names not in the game's labels; research-b44:", b44_new, "new,", b44_also,
      "agreeing,", b44_unmatched, "unmatched")
if b44_disagree:
    print("research-b44 values that disagree (not used):", ", ".join(b44_disagree))
