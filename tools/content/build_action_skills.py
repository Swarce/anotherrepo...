#!/usr/bin/env python3
"""Build server/content/action_skills.json: each statue's action-battle skills, the `skillList` the
action battle reads by index (CharacterStatus.SetStatueInfo 0xF17DC4 with
GameConst.GetSkillTypeFromSkillArray 0xDB5254: 0 attack, 1 skill 1, 2 skill 2, 3 SP 1, 4 SP 2;
docs/research/30-action-battle.md). A skill id SKILL_<p>_<NN> plays action-set entry
VariationSkillIdToSkillId(NN) (GameConst 0xDB627C): an extended id 61..70 is kept, any other is
NN%10 with 0 -> 10 (so SKILL_401_16 plays entry 6). FsmEventDoSkill.GetDoSkillEventName 0xF8370C
turns the entry into the DoSkill event the statue's action-set hub answers with its SKILL_<entry>
action set. The attack slot keeps 99, the combo attack (SetSkillInfo 0xF18838 skips the mapping
for slot 0).

Sources, in order (ids and numbers only):
  1. The Gamerch statue pages (live wiki, action era; the research passes' per-statue records): the
     skill's, SP skill's and finisher's names, matched to the game's own SKILL_NAME_<p>_<NN> labels of
     the statue's 3-digit prefix, with their cool-downs and uses. Matching, per slot:
       - strict: equal after norm() (NFKC; spaces, brackets, dots, dashes, tildes, ! and ? removed,
         one trailing ッ dropped). When several labels share the name, a finisher takes the one at
         the statue's retail cinematic entry, a skill or SP the first label;
       - fuzzy: otherwise the best difflib ratio, if at least 0.80 with the runner-up under 0.60
         (wiki typos and kana/kanji variants);
       - a finisher whose page prints no name takes the single label of the statue's NN decade at
         the cinematic entry.
     A statue with any matched slot is wiki-sourced; the match method of each slot is in its note.
  2. Otherwise the statue's tactical skills (statues.json dtSkills: the normal one as the skill,
     the special one as the SP skill), skipping a skill whose entry is the statue's cinematic entry
     or has no action-set entry; from rarity 5, the finisher is the single label at the cinematic
     entry (none when several labels sit there).
  The cinematic entries and action-set entries come from server/content/action_cinematics.json
  (tools/content/scan_action_cinematics.py, the player's files).

usage: python3 -I tools/content/build_action_skills.py \
         --research local/wiki-raw/research-b33/action/results.jsonl \
         --research local/wiki-raw/research-b34/action/results.jsonl \
         --export local/l10n/export-b33.tsv --out server/content/action_skills.json
"""
import argparse
import difflib
import json
import re
import unicodedata

ap = argparse.ArgumentParser()
ap.add_argument("--research", required=True, action="append",
                help="research results.jsonl; repeat it, a later file's record wins for the same statue and field")
ap.add_argument("--export", required=True)
ap.add_argument("--statues", default="server/content/statues.json")
ap.add_argument("--cinematics", default="server/content/action_cinematics.json")
ap.add_argument("--out", required=True)
a = ap.parse_args()


def norm(s):
    s = unicodedata.normalize("NFKC", s or "")
    # Not ［＋］: SKILL_304_14 and 304_18 differ only by it.
    s = re.sub(r"[\s・·…\.。、!！?？「」『』〜~～ー\-―─—–‐‑－]", "", s)
    return re.sub(r"ッ$", "", s)


def entry(nn):
    """GameConst.VariationSkillIdToSkillId 0xDB627C: the action-set entry a skill NN plays."""
    n = int(nn)
    return n if n < 1 or 61 <= n <= 70 else (n % 10 or 10)


labels = {}  # prefix -> {NN: label text}, in export order; 95..99 (attack and system) left out
first = {}  # prefix -> {Build 33 normalised text: first NN}: the tactical fallback's label check
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    m = re.fullmatch(r"text:SkillName_JP:SKILL_NAME_(\d{3})_(\d{2})", c[0]) if len(c) >= 4 else None
    if m:
        first.setdefault(m.group(1), {}).setdefault(re.sub(r"[\s・…\.。、!！?？「」『』〜~ー\-]", "",
                                                           unicodedata.normalize("NFKC", c[3])), m.group(2))
        if m.group(2) not in ("95", "96", "97", "98", "99"):
            labels.setdefault(m.group(1), {}).setdefault(m.group(2), c[3])
# The tactical fallback takes a dtSkill only when its NN is the first label of its text, as in
# Build 33 (a later NN sharing the text is skipped), so the fallback picks stay as they were.
have = {k: set(v.values()) for k, v in first.items()}

cin_doc = json.load(open(a.cinematics))
cinematic = {k: set(v) for k, v in cin_doc["cinematic"].items()}
# An action set with no SKILL_<NN> entry read (5140: no FSM at all) counts as unknown.
skill_sets = {k: set(v) for k, v in cin_doc["skillSets"].items() if v}

wiki = {}
for path in a.research:
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if r.get("topic") == "action.skills" and r.get("statueId") and r.get("confidence") in ("high", "medium"):
            wiki.setdefault(r["statueId"], {})[r["field"]] = r


def decade(nns):
    """The NN decade (01..10, 11..20, ...) the statue's matched skills share, else 01..10."""
    ds = {(int(n) - 1) // 10 for n in nns if int(n) <= 60}
    d = ds.pop() if len(ds) == 1 else 0
    return {f"{x:02d}" for x in range(d * 10 + 1, d * 10 + 11)}


def match(p, name, slot, ce, matched):
    lab = labels.get(p, {})
    if name:
        strict = [nn for nn, t in lab.items() if norm(t) == norm(name)]
        if len(strict) > 1 and slot == "finisher":
            at = [nn for nn in strict if entry(nn) in ce]
            if len(at) == 1:
                return at[0], "strict match, the label at the retail cinematic entry"
        if strict:
            return strict[0], "strict match"
        sc = sorted(((difflib.SequenceMatcher(None, norm(name), norm(t)).ratio(), nn) for nn, t in lab.items()),
                    reverse=True)
        if sc and sc[0][0] >= 0.8 and (len(sc) < 2 or sc[1][0] < 0.6):
            return sc[0][1], f"fuzzy match {sc[0][0]:.2f} (runner-up {sc[1][0] if len(sc) > 1 else 0:.2f})"
        return None, None
    if slot == "finisher":
        dec = decade(matched)
        at = [nn for nn in lab if entry(nn) in ce and nn in dec]
        if len(at) == 1:
            return at[0], "Gamerch SP2 (name blank) + retail cinematic set"
    return None, None


statues = json.load(open(a.statues))["statues"]
out = {}
stats = {"wiki": 0, "tactical": 0, "unmatched_names": 0, "fuzzy": 0, "cinematic_policy": 0}
SLOTS = (("skill", "skill"), ("sp_skill_1", "sp"), ("sp_skill_2_finisher", "finisher"))
for s in statues:
    sid = s["statueId"]
    p = sid[:3]
    psids = {m[:4] for m in s.get("modelDataList") or []}
    ce = set().union(*(cinematic.get(x, set()) for x in psids))
    sets = set().union(*(skill_sets[x] for x in psids if x in skill_sets)) if psids & set(skill_sets) else None
    rec = {"attack": f"SKILL_{p}_99", "skill": None, "sp": None, "finisher": None}
    w = wiki.get(sid, {})
    how, used, matched = {}, [], []
    for field, slot in SLOTS:
        r = w.get(field)
        if not r:
            continue
        nn, method = match(p, r["value"].get("name"), slot, ce, matched)
        if not nn:
            stats["unmatched_names"] += 1
            continue
        stats["fuzzy"] += method.startswith("fuzzy")
        matched.append(nn)
        rec[slot] = {"id": f"SKILL_{p}_{nn}",
                     **{k: r["value"][k] for k in ("cooldown", "uses") if isinstance(r["value"].get(k), int)}}
        how[slot] = method
        used.append((slot, r))
    if used:
        stats["wiki"] += 1
        notes = "; ".join(f"{k}: {v}" for k, v in how.items())
        dates = sorted({r.get("accessed") or "" for _, r in used})
        later = "".join(f"; the {slot} block read again {r['accessed']}" for slot, r in used
                        if len(dates) > 1 and r.get("accessed") == dates[-1])
        src = {"source": "Gamerch ジョジョDR攻略Wiki (live)", "page": used[0][1]["url"], "accessed": dates[-1],
               "note": f"skill names matched to the game's SKILL_NAME labels ({notes}); cool-downs and uses as "
                       f"printed (action era){later}"}
    else:
        policy = False
        for d in s.get("dtSkills") or []:
            slot = "sp" if d.get("special") else "skill"
            nn = d["id"].rsplit("_", 1)[-1]
            if rec[slot] is not None or nn not in have.get(p, set()):
                continue
            # The skill and SP slots never play the SP cinematic, and a skill needs an action-set entry.
            if entry(nn) in ce or (sets is not None and entry(nn) not in sets):
                policy = True
                continue
            rec[slot] = {"id": d["id"]}
        if (s.get("rarity") or 0) >= 5 and ce:
            lab = labels.get(p, {})
            at = [nn for nn in lab if entry(nn) in ce and (sets is None or entry(nn) in sets)]
            if len(at) == 1:  # several labels at the entry (3041 304_06/16, 4011 401_06/16/26...): none
                rec["finisher"] = {"id": f"SKILL_{p}_{at[0]}"}
                policy = True
        if policy:
            stats["cinematic_policy"] += 1
            src = {"source": "jdr-default", "note": "jdr-default + retail cinematic set (player files): no wiki skill "
                   "names; the statue's tactical skills (normal as skill, special as SP) that have an action-set "
                   "entry and do not play its SP cinematic, and from rarity 5 the finisher at its cinematic entry (action_cinematics.json)"}
        else:
            src = {"source": "jdr-default", "note": "no wiki skill names: the statue's tactical skills (normal as skill, special as SP)"}
        stats["tactical"] += 1
    rec["source"] = src
    out[sid] = rec
json.dump({"about": "Action-battle skills per statue (tools/content/build_action_skills.py; docs/research/30-action-battle.md): "
                    "the attack (SKILL_<p>_99), skill, SP skill and finisher ids for skillList, with the wiki's "
                    "cool-downs and uses where it gives them. Finishers without a wiki source are the label at the "
                    "statue's retail SP-cinematic entry (action_cinematics.json).",
           "statues": out}, open(a.out, "w"), ensure_ascii=False, indent=1)
open(a.out, "a").write("\n")
print(len(out), "statues", stats)
