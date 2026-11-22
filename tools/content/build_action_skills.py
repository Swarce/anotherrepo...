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
     When several labels share a skill's or SP's name, the one the statue's action set plays off
     its cinematic entry, else the one extended label (61..70) the set has, else the first.
     A statue with any matched slot is wiki-sourced; the match method of each slot is in its note.
  1b. --research-kit files (research-b43, Build 43: kits read for statues that had none). Gamerch
     names as above; ShoutWiki's English fan names are not matched, but its cool-downs and uses
     are kept (on the matched id, else on the jdr-default pick); finisher_label resolves a held
     finisher to one candidate label at the cinematic entry; a slot the wikis leave out keeps the
     jdr-default pick, marked "from": "jdr-default"; action_mode_usable false adds "actionMode".
  1c. Build 44: the research-b44 family files are given as --research-kit too. Their action.skills
     records (the official press release's skill and SP names) read as above; their action_skills
     records give action_mode_usable (an in-game profile screenshot ending with the Tactical-Battle-only
     tag); an action_mode_usable_source_says record that disagrees is kept as "sourceSays" in the
     statue's actionMode, not applied. REFILE moves research-b43 records filed under the wrong id.
  2. Otherwise the statue's tactical skills (statues.json dtSkills: the normal one as the skill,
     the special one as the SP skill), skipping a skill whose entry is the statue's cinematic entry
     or has no action-set entry; from rarity 5, the finisher is the single label at the cinematic
     entry (none when several labels sit there).
  The cinematic entries and action-set entries come from server/content/action_cinematics.json
  (tools/content/scan_action_cinematics.py, the player's files).

usage: python3 -I tools/content/build_action_skills.py \
         --research local/wiki-raw/research-b33/action/results.jsonl \
         --research local/wiki-raw/research-b34/action/results.jsonl \
         --research-kit local/wiki-raw/research-b43/action_skills/results.jsonl \
         [--research-kit local/wiki-raw/research-b44/<family>/results.jsonl ...] \
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
ap.add_argument("--research-kit", action="append",
                help="research results.jsonl read for statues that had no wiki kit (research-b43): as --research, "
                     "but a slot its records do not fill keeps the jdr-default pick, and its finisher verdicts "
                     "(finisher_label) and Action Battle flags (action_mode_usable) are read")
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

# Records filed under the wrong id (Build 44): research-b43/action_skills filed the Gamerch ★5 華麗なる跳躍
# Straizo page under 1090_03; research-b43/statues showed it is 1090_13's (the page's statue ability
# is 1090_13's), and the official press release (research-b44/official_and_press) gives the ★5 Solitary
# Straizo the same skill and SP names. (statueId, NFKC page-title fragment) -> statueId.
REFILE = {("1090_03", "【孤高】ストレイツォ★5(華麗なる跳躍)"): "1090_13"}


def refile(r):
    title = unicodedata.normalize("NFKC", r.get("url") or "")
    return next((to for (frm, frag), to in REFILE.items() if r["statueId"] == frm and frag in title), r["statueId"])


wiki = {}  # statueId -> field -> [record, ...] in file order (a later record wins)
kit_only = {}  # statueId -> True while every slot record comes from a --research-kit file
# assist_skill records are not read: the action battle gets no assist skill (assistSkillInfo {}).
KIT_FIELDS = ("skill", "sp_skill_1", "sp_skill_2_finisher", "finisher_label")
# research-b44 files mix topics: of topic action_skills only the Action Battle flags are read (its
# tactical cool-downs go to build_dt_cool_turns.py).
B44_FIELDS = ("action_mode_usable", "action_mode_usable_source_says")
for path, kit in [(x, False) for x in a.research] + [(x, True) for x in a.research_kit or []]:
    rnd = (re.search(r"research-(b\d+)", path) or [None, ""])[1]
    for line in open(path, encoding="utf-8"):
        r = json.loads(line)
        if r.get("statueId") and r.get("confidence") in ("high", "medium") and (
                r.get("topic") == "action.skills" or (r.get("topic") == "action_skills" and r.get("field") in B44_FIELDS)):
            r["_round"] = rnd
            sid = refile(r)
            wiki.setdefault(sid, {}).setdefault(r["field"], []).append(r)
            if r["field"] in KIT_FIELDS:
                kit_only[sid] = kit_only.get(sid, True) and kit


def host(r):
    return (re.match(r"https?://([^/]+)", r.get("url") or "") or [None, ""])[1]


def site(r):
    h = host(r)
    return ("Diamond Records Wiki (ShoutWiki)" if "shoutwiki" in h else
            "Bandai Namco press release (4Gamer.net)" if "4gamer" in h else
            "player blog (Ameba), in-game screenshots" if "ameblo" in h else "Gamerch ジョジョDR攻略Wiki (live)")


def numbers(r):
    return {k: r["value"][k] for k in ("cooldown", "uses") if isinstance((r or {}).get("value", {}).get(k), int)}


def decade(nns):
    """The NN decade (01..10, 11..20, ...) the statue's matched skills share, else 01..10."""
    ds = {(int(n) - 1) // 10 for n in nns if int(n) <= 60}
    d = ds.pop() if len(ds) == 1 else 0
    return {f"{x:02d}" for x in range(d * 10 + 1, d * 10 + 11)}


def match(p, name, slot, ce, matched, sets=None):
    lab = labels.get(p, {})
    if name:
        strict = [nn for nn, t in lab.items() if norm(t) == norm(name)]
        if len(strict) > 1 and slot == "finisher":
            at = [nn for nn in strict if entry(nn) in ce]
            if len(at) == 1:
                return at[0], "strict match, the label at the retail cinematic entry"
        if len(strict) > 1 and slot != "finisher" and sets:
            # Several labels share the name: the one the statue's action set can play, off its
            # cinematic entry (3322_08's SP 肉の芽 is 332_02 and 332_63; the 3322 set has no entry 2).
            ok = [nn for nn in strict if entry(nn) in sets and entry(nn) not in ce]
            if len(ok) == 1 and ok[0] != strict[0]:
                return ok[0], "strict match, the one label its action set plays off the cinematic entry"
            # Still several: the extended label (61..70) the set has. research-b43: the event ★6
            # statues' SP is their _61/_63 label (3020_28 C・F・H・バリエーション = 302_06 and 302_61).
            ext = [nn for nn in ok if 61 <= int(nn) <= 70]
            if len(ok) > 1 and len(ext) == 1 and ext[0] != strict[0]:
                return ext[0], "strict match, the extended label its action set plays"
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
stats = {"wiki": 0, "tactical": 0, "unmatched_names": 0, "fuzzy": 0, "cinematic_policy": 0, "kit": 0,
         "finisher_verdicts": 0, "numbers_on_default": 0, "numbers_unused": 0, "not_usable": 0, "source_says": 0}
SLOTS = (("skill", "skill"), ("sp_skill_1", "sp"), ("sp_skill_2_finisher", "finisher"))
unused = []


def default_kit(s, p, ce, sets):
    """The jdr-default pick: the statue's tactical skills (normal as skill, special as SP) and, from
    rarity 5, the finisher at its cinematic entry. Returns the slots and whether the cinematic
    policy decided anything."""
    kit, policy = {"skill": None, "sp": None, "finisher": None}, False
    for d in s.get("dtSkills") or []:
        slot = "sp" if d.get("special") else "skill"
        nn = d["id"].rsplit("_", 1)[-1]
        if kit[slot] is not None or nn not in have.get(p, set()):
            continue
        # The skill and SP slots never play the SP cinematic, and a skill needs an action-set entry.
        if entry(nn) in ce or (sets is not None and entry(nn) not in sets):
            policy = True
            continue
        kit[slot] = {"id": d["id"]}
    if (s.get("rarity") or 0) >= 5 and ce:
        lab = labels.get(p, {})
        at = [nn for nn in lab if entry(nn) in ce and (sets is None or entry(nn) in sets)]
        if len(at) == 1:  # several labels at the entry (3041 304_06/16, 4011 401_06/16/26...): none
            kit["finisher"] = {"id": f"SKILL_{p}_{at[0]}"}
            policy = True
    return kit, policy


def verdict(p, w, ce):
    """research-b43's finisher_label: a held finisher resolved to one of its candidate labels (the
    wiki's finisher name equals that label's text). Kept only when it is a candidate of the
    statue's prefix at its cinematic entry."""
    fl = (w.get("finisher_label") or [None])[-1]
    if not fl:
        return None, None
    lab = fl["value"].get("label") or ""
    nn = lab.rsplit("_", 1)[-1]
    ok = (lab.startswith(f"SKILL_{p}_") and lab in (fl["value"].get("candidates") or [])
          and nn in labels.get(p, {}) and entry(nn) in ce)
    return (nn, fl) if ok else (None, None)


for s in statues:
    sid = s["statueId"]
    p = sid[:3]
    psids = {m[:4] for m in s.get("modelDataList") or []}
    ce = set().union(*(cinematic.get(x, set()) for x in psids))
    sets = set().union(*(skill_sets[x] for x in psids if x in skill_sets)) if psids & set(skill_sets) else None
    rec = {"attack": f"SKILL_{p}_99", "skill": None, "sp": None, "finisher": None}
    w = wiki.get(sid, {})
    kit = kit_only.get(sid, False)
    dkit, dpolicy = default_kit(s, p, ce, sets) if kit else ({}, False)
    how, used, matched = {}, [], []
    for field, slot in SLOTS:
        rs = w.get(field) or []
        # A name is matched from the wiki whose names are the game's (Gamerch, Japanese); ShoutWiki's
        # English fan names are not matched (they would need our own translations as the bridge).
        jp = [r for r in rs if r["value"].get("lang") != "en"]
        r = jp[-1] if jp else None
        nn = method = None
        if r:
            nn, method = match(p, r["value"].get("name"), slot, ce, matched, sets)
            if not nn:
                stats["unmatched_names"] += 1
        if slot == "finisher" and kit:
            vnn, fl = verdict(p, w, ce)
            if vnn and not nn:
                nn, r = vnn, fl
                method = f"the research pass's verdict for a held finisher ({site(fl)}: the finisher name = this label's text)"
                stats["finisher_verdicts"] += 1
            elif vnn and method:
                method += ", as the research pass's verdict" if vnn == nn else f" (the research verdict says {vnn})"
        # Cool-downs and uses: the matched record's, else another wiki's for the same slot.
        nums, nrec = (numbers(r), r) if nn and numbers(r) else ({}, None)
        if not nums:
            other = [x for x in rs if numbers(x) and (not (nn and r) or host(x) != host(r))]
            if other:
                nums, nrec = numbers(other[-1]), other[-1]
        if nn:
            stats["fuzzy"] += method.startswith("fuzzy")
            matched.append(nn)
            rec[slot] = {"id": f"SKILL_{p}_{nn}", **nums}
            if nrec is not None and nrec is not r:
                method += f", cool-down/uses from {site(nrec)}"
            how[slot] = method
            used.append((slot, r))
            if nrec is not None and nrec is not r:
                used.append((slot, nrec))
        elif kit and dkit.get(slot):
            rec[slot] = {**dkit[slot], **nums, "from": "jdr-default"}
            if nums:
                how[slot] = f"jdr-default pick, cool-down/uses from {site(nrec)} (its name is not matched)"
                used.append((slot, nrec))
                stats["numbers_on_default"] += 1
            else:
                how[slot] = "jdr-default pick"
        elif nums:
            stats["numbers_unused"] += 1
            unused.append(f"{sid} {slot}")
    if kit and used:
        # A jdr-default pick that a wiki slot already holds is dropped.
        ids = [rec[x]["id"] for x in ("skill", "sp", "finisher") if rec[x] and not rec[x].get("from")]
        for x in ("skill", "sp", "finisher"):
            if rec[x] and rec[x].get("from") and rec[x]["id"] in ids:
                rec[x], how[x] = None, f"none (the jdr-default pick {rec[x]['id']} is a wiki slot's)"
    if used and not kit:
        stats["wiki"] += 1
        notes = "; ".join(f"{k}: {v}" for k, v in how.items())
        dates = sorted({r.get("accessed") or "" for _, r in used})
        later = "".join(f"; the {slot} block read again {r['accessed']}" for slot, r in used
                        if len(dates) > 1 and r.get("accessed") == dates[-1])
        src = {"source": "Gamerch ジョジョDR攻略Wiki (live)", "page": used[0][1]["url"], "accessed": dates[-1],
               "note": f"skill names matched to the game's SKILL_NAME labels ({notes}); cool-downs and uses as "
                       f"printed (action era){later}"}
    elif used:
        stats["wiki"] += 1
        stats["kit"] += 1
        notes = "; ".join(f"{k}: {how[k]}" for k in ("skill", "sp", "finisher") if k in how)
        pages = []
        for _, r in used:
            if r["url"] not in [x["page"] for x in pages]:
                pages.append({"source": site(r), "page": r["url"], "accessed": r["accessed"],
                              **({"capture": r["capture"]} if r.get("capture") else {}),
                              **({"license": "CC BY-SA 3.0"} if "shoutwiki" in host(r) else {})})
        b44 = any(r.get("_round") == "b44" for _, r in used)
        src = {**pages[0],
               "note": (f"research-b43/b44 kit (Gamerch and official press-release names" if b44 else
                        f"research-b43 kit (Gamerch names") +
                       f" matched to the game's SKILL_NAME labels; ShoutWiki "
                       f"cool-downs and uses as printed, action era; a slot the wikis leave out keeps the jdr-default "
                       f"pick): {notes}"
                       + "".join(sorted({f"; the Gamerch page's records refiled from {r['statueId']} (REFILE)"
                                         for _, r in used if refile(r) != r["statueId"]}))}
        if pages[1:]:
            src["also"] = pages[1:]
    else:
        kit, policy = default_kit(s, p, ce, sets)
        rec.update(kit)
        if policy:
            stats["cinematic_policy"] += 1
            src = {"source": "jdr-default", "note": "jdr-default + retail cinematic set (player files): no wiki skill "
                   "names; the statue's tactical skills (normal as skill, special as SP) that have an action-set "
                   "entry and do not play its SP cinematic, and from rarity 5 the finisher at its cinematic entry (action_cinematics.json)"}
        else:
            src = {"source": "jdr-default", "note": "no wiki skill names: the statue's tactical skills (normal as skill, special as SP)"}
        stats["tactical"] += 1
    rec["source"] = src
    um = (w.get("action_mode_usable") or [None])[-1]
    if um is not None and um["value"] is False:
        stats["not_usable"] += 1
        rec["actionMode"] = {"usable": False, "source": site(um), "page": um["url"], "accessed": um["accessed"],
                             **({"license": "CC BY-SA 3.0"} if "shoutwiki" in host(um) else {}),
                             "note": ("the in-game profile (a player's screenshot) ends with the Tactical-Battle-only "
                                      "tag (research-b44)" if um["_round"] == "b44" else
                                      "the page's Action Battle (Old Mode) tab says the statue cannot be used there "
                                      "(research-b43)") + "; the kit above is kept and needs no source"}
        ss = (w.get("action_mode_usable_source_says") or [None])[-1]
        if ss is not None:
            stats["source_says"] += 1
            rec["actionMode"]["sourceSays"] = {
                "source": site(ss), "page": ss["url"], "accessed": ss["accessed"], "confidence": ss["confidence"],
                "note": "research-b44: the in-game profile has no Tactical-Battle-only tag (5170_14, 5220_14 and "
                        "4031_18 have it), against the page above; noted, not applied"}
    out[sid] = rec
json.dump({"about": "Action-battle skills per statue (tools/content/build_action_skills.py; docs/research/30-action-battle.md): "
                    "the attack (SKILL_<p>_99), skill, SP skill and finisher ids for skillList, with the wiki's "
                    "cool-downs and uses where it gives them. Finishers without a wiki source are the label at the "
                    "statue's retail SP-cinematic entry (action_cinematics.json).",
           "statues": out}, open(a.out, "w"), ensure_ascii=False, indent=1)
open(a.out, "a").write("\n")
print(len(out), "statues", stats)
if unused:
    print("wiki cool-downs/uses with no slot id to carry them:", ", ".join(unused))
