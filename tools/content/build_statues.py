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
  - tactical skills (`dtSkills`, docs/research/12 §3): no source says which SKILL_NN belongs to which
    variant, so variant k of a character (statueIds of one 3-digit prefix, sorted) takes
    SKILL_{2k-1} (normal skill) and SKILL_{2k} (special), else the k-th available pair, among the
    NN that both have an action set on the model and a DTbattle_SKILL_TEXT label. The skill text is
    read locally only to classify it (damage or heal, one target or all, element); buff and debuff
    skills wait for buff data, and skills with no damage, heal or buff wording are sent as
    single-target hits. Only label keys, ids and our numbers are written;
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
ap.add_argument("--clips", help="local/content/figure_clips.tsv (scan_figure_clips.py): pose clips per model")
ap.add_argument("--heights", help="local/content/model_heights.tsv (scan_model_heights.py): model heights")
ap.add_argument("--extra", action="append", default=[],
                help="research collections with exact statueId matches ({records:[{statueId, rarity, type, stats:{lv1,lv99,tree_max}, source, confidence}]}); "
                     "used for statues without a Gamerch match, medium/high confidence and plausible stats only")
a = ap.parse_args()

scan = json.load(open(a.scan))["statues"]
labels = {}
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    if len(c) >= 4:
        labels[c[0]] = c[3].replace("\\n", "\n")
lead = {k.rsplit("_", 2)[-2] + "_" + k.rsplit("_", 1)[-1] for k in labels if k.startswith("text:Ability_Leader_Name_JP:ABI_LEAD_NAME_")}
# Obtainable: playable, with a leader-ability label and a thumbnail (else a blank card in the lists).
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


DTSC = {k.split(":")[-1] for k in labels if ":DTSC_" in k}
TEXT = {k.split(":")[-1]: v for k, v in labels.items() if ":DTbattle_SKILL_TEXT_" in k or ":SKILL_NAME_" in k}
ATTR = {"PHYSICAL": "PHYSICAL", "STAND_PHYSICAL": "STAND_PHY", "HAMON": "HAMON", "UV": "UV", "VMP": "VMP",
        "ELECT": "ELECTOCAL", "FLAME": "FIRE", "ICE": "PHYSICAL", "BOMB": "PHYSICAL"}


def classify(t):
    """(process, category label, TargetType, ProcessType, hit attribute) from a skill's text."""
    elem = ("STAND_PHYSICAL" if "スタンド" in t else "HAMON" if "波紋" in t else "UV" if "紫外線" in t
            else "VMP" if "吸血" in t else "FLAME" if ("炎" in t or "火" in t) else "ELECT" if "電" in t
            else "ICE" if ("氷" in t or "冷" in t) else "BOMB" if "爆" in t else "PHYSICAL")
    if "[DR{damagerank}]" in t:
        allt = any(w in t for w in ("全体", "敵チーム", "広範囲", "周囲"))
        cat = "DTSC_" + elem + ("_ALL" if allt else "")
        if cat not in DTSC:
            cat = "DTSC_PHYSICAL" + ("_ALL" if allt else "")
        return "DAMAGE", cat, 2 if allt else 1, 1, ATTR[elem]
    if "回復" in t:
        base = "DTSC_" + ("HAMON" if "波紋" in t else "STAND" if "スタンド" in t else "PHYSICAL") + "_HEAL"
        cat = base + ("_ALL" if ("チーム" in t or "全体" in t) else "_MINE" if ("自身" in t and "味方" not in t) else "")
        if cat not in DTSC:
            cat = base if base in DTSC else "DTSC_PHYSICAL_HEAL"
        return "HEAL", cat, 4 if cat.endswith("_ALL") else 5 if cat.endswith("_MINE") else 3, 3, "NONE"
    buff = any(w in t for w in ("アップ", "上昇", "ダウン", "封印", "状態変化"))
    if buff:
        return None  # buffs and debuffs need buff data (dtBuff rows), not sent yet
    # Anything else (no damage-rank tag, no heal or buff wording): a single-target hit with the
    # skill's own animation, so the statue keeps its skill button (Build 13 device log: Jonathan
    # and Jotaro had no special because their specials fell here).
    return "DAMAGE", "DTSC_" + elem if "DTSC_" + elem in DTSC else "DTSC_PHYSICAL", 1, 1, ATTR[elem]


# Button icons by kind: single-target damage ONE_ATTACK 7 / SP_ONE_ATTACK 38, all-target DT_AREA_ATTACK
# 83 / 90, heal DT_HEEL 75 / 82 (SkillButtonType; the icon choice is ours).
BUTTON = {("DAMAGE", 1): (7, 38), ("DAMAGE", 2): (83, 90), ("HEAL", 3): (75, 82), ("HEAL", 4): (75, 82),
          ("HEAL", 5): (75, 82)}
by_prefix = collections.defaultdict(list)
for s in obtainable:
    by_prefix[s["statueId"][:3]].append(s["statueId"])


def dt_skills(s):
    sid, g = s["statueId"], s["statueId"][:3]
    k = sorted(by_prefix[g]).index(sid)
    acts = s.get("dtActions") or []
    avail = sorted(int(x[6:]) for x in acts
                   if re.fullmatch(r"SKILL_\d\d", x) and f"DTbattle_SKILL_TEXT_{g}_{int(x[6:]):02d}" in TEXT)
    pair = [n for n in (2 * k + 1, 2 * k + 2) if n in avail]
    if len(pair) < 2:
        pairs = [avail[j:j + 2] for j in range(0, len(avail), 2)]
        pair = pairs[min(k, len(pairs) - 1)] if pairs else []
    out = []
    for i, n in enumerate(pair):
        key = f"{g}_{n:02d}"
        c = classify(TEXT.get(f"DTbattle_SKILL_TEXT_{key}", ""))
        if not c:
            continue
        process, cat, target, ptype, attr = c
        special = i == 1
        out.append({
            "id": f"SKILL_{key}", "special": special,
            "name": f"SKILL_NAME_{key}" if f"SKILL_NAME_{key}" in TEXT else "",
            "text": f"DTbattle_SKILL_TEXT_{key}",
            "button": BUTTON[(process, target)][1 if special else 0],
            "damageRank": 150 if special else 125,
            "category": cat, "targetType": target, "processType": ptype, "attr": attr,
        })
    return out


# Team view (docs/research/12 §7): the bust-up pose clip is chosen by rarity (3 figure_R, 4
# figure_SR, 5 figure_SSR, 6 figure_UR; a suffix from the statueId's last two digits). With no clip
# the model stays in its bind pose (T-pose), so a statue with no wiki rarity takes a rarity whose
# pose exists for its first model; the bust shot frames 1.55-2.05 m, so models are scaled to fit.
CLIPS = {}
if a.clips:
    for line in open(a.clips):
        p = line.rstrip("\n").split("\t")
        CLIPS[p[1]] = set(p[2].split()) if len(p) > 2 else set()
HEIGHTS = {}
if a.heights:
    for line in open(a.heights):
        p = line.rstrip("\n").split("\t")
        HEIGHTS[p[1]] = float(p[3])
POSE = {3: "figure_R", 4: "figure_SR", 5: "figure_SSR", 6: "figure_UR"}


def pose_exists(sid, model, r):
    c = int(sid[-2:])
    tens, ones = (c % 100) // 10, c % 10
    suffix = ("_" + (str(tens) if tens else "") + ("A" if ones >= 5 else "")) if (tens or ones >= 5) else ""
    return POSE[r] + suffix in CLIPS.get(model, set())


def pose_rarity(s):
    """A rarity whose team-view pose exists (closest to the one the id's numbering suggests)."""
    models = s.get("modelDataList") or []
    if not models or models[0] not in CLIPS:
        return None
    ok = [r for r in (3, 4, 5, 6) if pose_exists(s["statueId"], models[0], r)]
    if not ok:
        return None
    hint = 2 + int(s["statueId"][-1]) if s["statueId"][-1] in "1234" else 5
    return min(ok, key=lambda r: (abs(r - hint), -r))


def team_scale(s):
    models = s.get("modelDataList") or []
    h = HEIGHTS.get(models[0]) if models else None
    if not h or h <= 0:
        return 100
    return round(100 * min(max(h, 1.55), 2.05) / h)


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
# Research collections: exact statueId matches only.
extra = {}
for path in a.extra:
    for r in json.load(open(path)).get("records", []):
        sid, st = r.get("statueId"), r.get("stats") or {}
        if (sid in jp_name and sid not in gamerch and r.get("confidence") in ("high", "medium")
                and full(st.get("lv1")) and full(st.get("lv99")) and plausible(st["lv1"], st["lv99"])):
            url = r["source"]["url"]
            site = ("Diamond Records Wiki (ShoutWiki)" if "shoutwiki" in url else
                    "Gamerch ジョジョDR攻略Wiki (Wayback Machine)" if "gamerch" in url else None)
            if site:
                r["source"]["site"] = site
                if "shoutwiki" in url:
                    r["source"]["license"] = "CC BY-SA 3.0"
                extra.setdefault(sid, r)
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
        "dtSkills": dt_skills(s),
        "teamScale": team_scale(s),
        # No st_<statueId> thumbnail in the ui_statue_* atlases: the lists would show a blank card
        # (Build 17 device test). Story stages still use these as enemies, so they stay, enemy-only.
        **({} if s.get("thumbnail") is True else {"enemyOnly": True}),
        # The win-screen quote: the game's own Result_Chara_Comment_<statueId> label, when present.
        "resultComment": (f"Result_Chara_Comment_{sid}"
                          if f"text:Result_Chara_Comment_JP:Result_Chara_Comment_{sid}" in labels else None),
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
    # 3) Research collections with an exact statueId (medium/high confidence, plausible stats), for
    #    statues still without numbers.
    x = extra.get(sid)
    if x and rec["lv99"] is None:
        st = x["stats"]
        rec.update({
            "rarity": x.get("rarity"),
            "statueType": GAMERCH_TYPE.get(x.get("type")),
            "lv1": st["lv1"], "lv99": st["lv99"],
            "maxTree": st["tree_max"] if full(st.get("tree_max")) else None,
            "source": {"source": x["source"]["site"], "page": x["source"]["url"],
                       **({"license": x["source"]["license"]} if x["source"].get("license") else {}),
                       **({"capture": x["source"]["archive_url"]} if x["source"].get("archive_url") else {}),
                       "accessed": x["source"].get("accessed", TODAY)},
        })
        matched += 1
        by_source["extra"] += 1
    if rec["rarity"] is None:
        r = pose_rarity(s)
        if r:
            rec["rarity"] = r
            rec["rarityFrom"] = "the team-view pose clips present in the player's files"
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
print(f"{len(out)} statues ({sum(1 for r in out if r.get('enemyOnly'))} enemy-only, no thumbnail); {matched} with wiki stats ({dict(by_source)}), {len(out) - matched} on defaults; "
      f"rejected as implausible or out of rarity order: {sorted(set(rejected))}")
