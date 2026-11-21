#!/usr/bin/env python3
"""Build server/content/action_skills.json: each statue's action-battle skills, the `skillList` the
action battle reads by index (CharacterStatus.SetStatueInfo 0xF17DC4 with
GameConst.GetSkillTypeFromSkillArray 0xDB5254: 0 attack, 1 skill 1, 2 skill 2, 3 SP 1, 4 SP 2;
docs/research/30-action-battle.md). A skill id SKILL_<p>_<NN> fires DoSkill<NN> on the statue's
action-set hub, which plays its SKILL_<NN> action set; 99 is the combo attack.

Sources, in order (ids and numbers only):
  1. The Gamerch statue pages (live wiki, action era; the research pass's per-statue records): the
     skill's, SP skill's and finisher's names, matched to the game's own SKILL_NAME_<p>_<NN> labels of
     the statue's 3-digit prefix after NFKC normalisation, with their cool-downs and uses.
  2. Otherwise the statue's tactical skills (statues.json dtSkills: the normal one as the skill,
     the special one as the SP skill), with our numbers.

usage: python3 -I tools/content/build_action_skills.py --research local/wiki-raw/research-b33/action/results.jsonl \
         --export local/l10n/export-b33.tsv --out server/content/action_skills.json
"""
import argparse
import json
import re
import unicodedata

ap = argparse.ArgumentParser()
ap.add_argument("--research", required=True)
ap.add_argument("--export", required=True)
ap.add_argument("--statues", default="server/content/statues.json")
ap.add_argument("--out", required=True)
a = ap.parse_args()


def norm(s):
    s = unicodedata.normalize("NFKC", s or "")
    return re.sub(r"[\s・…\.。、!！?？「」『』〜~ー\-]", "", s)


names = {}  # prefix -> {norm name: NN}
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    m = re.fullmatch(r"text:SkillName_JP:SKILL_NAME_(\d{3})_(\d{2})", c[0]) if len(c) >= 4 else None
    if m:
        names.setdefault(m.group(1), {}).setdefault(norm(c[3]), m.group(2))

wiki = {}
for line in open(a.research, encoding="utf-8"):
    r = json.loads(line)
    if r.get("topic") == "action.skills" and r.get("statueId") and r.get("confidence") in ("high", "medium"):
        wiki.setdefault(r["statueId"], {})[r["field"]] = r

statues = json.load(open(a.statues))["statues"]
out, stats = {}, {"wiki": 0, "tactical": 0, "unmatched_names": 0}
for s in statues:
    sid = s["statueId"]
    p = sid[:3]
    rec = {"attack": f"SKILL_{p}_99", "skill": None, "sp": None, "finisher": None}
    w = wiki.get(sid, {})
    src = None
    for field, slot in (("skill", "skill"), ("sp_skill_1", "sp"), ("sp_skill_2_finisher", "finisher")):
        r = w.get(field)
        if not r:
            continue
        nn = names.get(p, {}).get(norm(r["value"].get("name")))
        if not nn:
            stats["unmatched_names"] += 1
            continue
        rec[slot] = {"id": f"SKILL_{p}_{nn}",
                     **{k: r["value"][k] for k in ("cooldown", "uses") if isinstance(r["value"].get(k), int)}}
        src = {"source": "Gamerch ジョジョDR攻略Wiki (live)", "page": r["url"], "accessed": r.get("accessed"),
               "note": "skill names matched to the game's SKILL_NAME labels; cool-downs and uses as printed (action era)"}
    if rec["skill"] or rec["sp"]:
        stats["wiki"] += 1
    else:
        have = set(names.get(p, {}).values())  # NN with a SKILL_NAME label = a skill_data_table row
        for d in s.get("dtSkills") or []:
            slot = "sp" if d.get("special") else "skill"
            if rec[slot] is None and d["id"].rsplit("_", 1)[-1] in have:
                rec[slot] = {"id": d["id"]}
        src = {"source": "jdr-default", "note": "no wiki skill names: the statue's tactical skills (normal as skill, special as SP)"}
        stats["tactical"] += 1
    rec["source"] = src
    out[sid] = rec
json.dump({"about": "Action-battle skills per statue (tools/content/build_action_skills.py; docs/research/30-action-battle.md): "
                    "the attack (SKILL_<p>_99), skill, SP skill and finisher ids for skillList, with the wiki's "
                    "cool-downs and uses where it gives them.",
           "statues": out}, open(a.out, "w"), ensure_ascii=False, indent=1)
open(a.out, "a").write("\n")
print(len(out), "statues", stats)
