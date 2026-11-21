#!/usr/bin/env python3
"""Build server/content/minds.json: minds (cards) from the Gamerch wiki matched to game ids by name
(the wiki's Japanese name against the game's own MIND_NAME_<id> label, after NFKC normalisation; a
name shared by several ids is skipped), with rarity, cost and flat HP/ATK/DEF from their effects.
Only minds whose image bundle (mim_<id>) exists in the player's files.

Build 41:
- A second, loose name key (NFKC without punctuation, ー, ―, …, ・ and ~) is tried only for a wiki
  record whose exact name matches nothing, and only when that key names one game id and one wiki
  record, and only for the ids in LOOSE_CHECKED (17 more Minds: the wiki writes 「」 for 『』,
  ーー for ――, ... for …). Other candidates are printed and held.
- Flat stats are read from each effect's raw text, so 'ATK50(MAX+100)', 'HP+ 480(MAX+720)' and the
  full-width ＋ parse. Lv1 is the first effect of that stat with no basis and no condition (a
  condition such as ドドドモード時 or 【波紋使い】の makes the bonus conditional, so it is skipped).
  Negative (【デメリット】) values are left out (held for the owner).
- `lv10` is that effect's MAX value: from value_max, or from the effect's text where the capture
  left value_max empty ('HP+800(+1000)', 'HP+600{MAX+800)', 'DEF+250(MAX DEF+400)'). A wiki Lv1 of
  '?' with a numeric MAX keeps the MAX and takes the default Lv1 below.
- `PC_OBSERVED`: Lv10 values the wiki leaves blank ('MAX+?' or no MAX) for 9 Minds, transcribed in
  the Diamond Records Preservation (PC) data from original-game screenshots at Lv10 or from an
  archived translation; cited per row. Only the stats our wiki capture lacks; a wiki Lv1 is kept.
- A Lv1 nothing gives is round(Lv10 x r), r the median Lv1/Lv10 of the capture's ★3/★4 stat pairs
  (jdr-default, marked in `lv1Source`).

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


def loose(x):
    return "".join(ch for ch in norm(x) if not unicodedata.category(ch).startswith("P")
                   and ch not in "ー―…・~～" and not ch.isspace())


ids_by_name, ids_by_loose = {}, {}
for line in open(a.export, encoding="utf-8"):
    c = line.rstrip("\n").split("\t")
    m = re.fullmatch(r"text:MindName_JP:MIND_NAME_(\d{4})", c[0]) if len(c) >= 4 else None
    if m:
        ids_by_name.setdefault(norm(c[3]), set()).add(m.group(1))
        ids_by_loose.setdefault(loose(c[3]), set()).add(m.group(1))
doc = json.load(open(a.gamerch))
recs = doc if isinstance(doc, list) else doc["minds"]

# One flat stat effect: 'HP+680(MAX+1020)', 'ATK50(MAX+100)', 'HP+ 480(MAX+720)', 'HP+800(+1000)',
# 'HP+600{MAX+800)', 'DEF+250(MAX DEF+400)', 'DEF+?(MAX+650)', 'HP+600' (after NFKC).
FLAT = re.compile(r"(HP|ATK|DEF)\s*([+-]?)\s*(\d+|\?)\s*"
                  r"(?:[({]\s*(?:MAX)?\s*(?:HP|ATK|DEF)?\s*([+-]?)\s*(\d+|\?)\s*\))?\s*(【デメリット】)?[。.]?")


def stats(effects):
    """{stat: (lv1 or None, lv10 or None)} for HP/ATK/DEF, first unconditional flat effect each."""
    got = {}
    for e in effects or []:
        if e.get("basis") or e.get("condition"):
            continue
        m = FLAT.fullmatch(unicodedata.normalize("NFKC", e.get("raw") or "").replace(",", "").strip())
        if not m or m.group(1).lower() in got:
            continue
        stat, sign, v1, sign10, v10, demerit = m.groups()
        if sign == "-" or sign10 == "-" or demerit:
            continue  # demerits: held
        got[stat.lower()] = (int(v1) if v1.isdigit() else None,
                             int(v10) if v10 and v10.isdigit() else None)
    return got


# Lv10 endpoints the wiki leaves blank, from the Diamond Records Preservation (PC) data (only these
# stats; see the module docstring). Screenshot rows give the sha256 of the original-game image.
SHOT = "Diamond Records Preservation (PC) data, original-game screenshot at Lv10 (sha256 {})"
TRANSL = "Diamond Records Preservation (PC) data, archived translation of the Mind's page"
PC_OBSERVED = {
    "0002": ({"hp": 1020, "atk": 550}, TRANSL),
    "0019": ({"hp": 1350, "def": 850}, SHOT.format("bfb1e672216fd3c7ad2a5fd602d57ce4b326e8c6c8fa759e4e4fd488fc8c2360")),
    "7189": ({"hp": 800, "def": 1100}, SHOT.format("ca91b5afad51682df5c070d26ce434c5c015084197fe198fe502c564d584f889")),
    "7238": ({"hp": 1400, "def": 800}, SHOT.format("25e8eb857f3adcd3eb7e98f8c777256970706e9b36437c4877d753bf0e8c24fc")),
    "7260": ({"hp": 1050, "atk": 570}, SHOT.format("21abce71f5798c27adb65a0fb00fe9bb6450767173b1c7352167241a54794974")),
    "7350": ({"hp": 1500, "atk": 720}, TRANSL),
    "7392": ({"atk": 720}, SHOT.format("794bc16e3c5a4e51a4285460d207f979046161262b4a4ac1c6a667fd1235e7b8")),
    "7439": ({"hp": 2350}, SHOT.format("2fbb2b479b60d9d4d73a57582e4ed9de0fbb1685a3891ae56653197dd775fb9a")
             + "; the wiki's HP+2078 (no MAX) is kept as Lv1, though it may not be one"),
    "7490": ({"hp": 1500, "atk": 5000, "def": 1000},
             SHOT.format("cd2490a00373bd61b82a9fde7ad5e81ab40b4553733499e6b58bfe94c988eec3")),
}
# A Lv1 the same archived translation gives where the wiki has no stats at all.
PC_LV1 = {"7350": ({"hp": 900, "atk": 520}, TRANSL)}


# Cost by rarity: the wiki lists a cost for some Minds, and every Mind of a rarity with a known cost
# has the same one (1: 3, 2: 5, 3: 8, 4: 12 in the 2026-11-14 capture). Minds without one get it.
by_rarity = {}
for r in recs:
    rr = re.sub(r"\D", "", str(r.get("rarity") or ""))
    if isinstance(r.get("cost"), int) and rr:
        by_rarity.setdefault(int(rr), set()).add(r["cost"])
COST = {k: next(iter(v)) for k, v in by_rarity.items() if len(v) == 1}


def rarity_of(r):
    return int(str(r.get("rarity") or "☆3").strip("☆") or 3)


# The default Lv1/Lv10 ratio: the median over the capture's ★3/★4 flat stat pairs.
pairs = sorted(v1 / v10 for r in recs if rarity_of(r) in (3, 4)
               for v1, v10 in stats(r.get("effects")).values() if v1 and v10 and v10 >= v1)
RATIO = round(pairs[len(pairs) // 2], 2) if pairs else 0.71

# The loose matches checked by hand in the Build 41 pass (name, rarity and stats against the game's
# label and bundle). Other loose candidates are printed and held until someone checks them.
LOOSE_CHECKED = {"1005", "1041", "2008", "4006", "4047", "4093", "4118", "4127", "7038", "7089", "7106",
                 "7191", "7201", "7302", "7303", "7318", "7428"}

# Strict names first; the loose key only for records the strict key leaves unmatched.
matched, strict_ids = [], set()
for r in recs:
    ids = ids_by_name.get(norm(r.get("name_jp")), set())
    matched.append(next(iter(ids)) if len(ids) == 1 else None)
    strict_ids |= ids
loose_recs = {}
for r, mid in zip(recs, matched):
    if mid is None and r.get("name_jp"):
        loose_recs.setdefault(loose(r["name_jp"]), []).append(r)
held = []
for i, (r, mid) in enumerate(zip(recs, matched)):
    if mid is None and r.get("name_jp"):
        k = loose(r["name_jp"])
        ids = ids_by_loose.get(k, set())
        if len(ids) == 1 and len(loose_recs[k]) == 1 and not ids & strict_ids:
            if ids & LOOSE_CHECKED:
                matched[i] = next(iter(ids))
            else:
                held.append(next(iter(ids)))

out, seen = [], set()
for r, mid in zip(recs, matched):
    if not mid or mid in seen or f"mim_{mid}" not in names:
        continue
    seen.add(mid)
    rarity = rarity_of(r)
    st = stats(r.get("effects"))
    lv1 = {k: (st.get(k) or (0, None))[0] for k in ("hp", "atk", "def")}
    lv10, lv10_src, lv1_src = None, None, None
    if any(v10 is not None for _, v10 in st.values()):
        lv10 = {k: (st[k][1] if st.get(k) and st[k][1] is not None else lv1[k] or 0)
                for k in ("hp", "atk", "def")}
        # Where the capture's value_max is empty, the MAX came from the effect's own text.
        notes = [f"'{e['raw']}'" for e in r.get("effects") or []
                 if not e.get("value_max") and not e.get("basis") and not e.get("condition")
                 and (m := FLAT.fullmatch(unicodedata.normalize("NFKC", e.get("raw") or "").strip()))
                 and m.group(5)]
        lv10_src = "Gamerch: the MAX value of the effect" + (
            f" (read from the effect text {', '.join(notes)})" if notes else "")
    if mid in PC_OBSERVED:
        vals, why = PC_OBSERVED[mid]
        lv10 = {k: vals.get(k, (lv10 or {}).get(k, lv1[k] or 0)) for k in ("hp", "atk", "def")}
        lv10_src = f"{', '.join(k.upper() for k in vals)}: {why}" + (f"; others: {lv10_src}" if lv10_src else "")
    if mid in PC_LV1:
        vals, why = PC_LV1[mid]
        lv1.update(vals)
        lv1_src = why
    if lv10:
        guess = [k for k in ("hp", "atk", "def") if lv10[k] and not lv1[k]]
        for k in guess:
            lv1[k] = int(lv10[k] * RATIO + 0.5)
        if guess:
            lv1_src = (f"{', '.join(k.upper() for k in guess)}: round(Lv10 x {RATIO}) (jdr-default; the median"
                       " Lv1/Lv10 of the wiki's ★3/★4 Minds), no Lv1 in our sources")
    lv1 = {k: v or 0 for k, v in lv1.items()}
    out.append({
        "mindId": mid,
        "rarity": rarity,
        "cost": r["cost"] if isinstance(r.get("cost"), int) else COST.get(rarity, rarity * 3),
        **lv1,
        **({"lv10": lv10, "lv10Source": lv10_src} if lv10 else {}),
        **({"lv1Source": lv1_src} if lv1_src else {}),
        "source": {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)", "page": r["source"]["page_url"],
                   "accessed": r["source"].get("accessed"),
                   **({} if isinstance(r.get("cost"), int) else
                      {"note": "cost of every Mind of this rarity on the wiki" if rarity in COST
                       else "cost = 3 x rarity (jdr-default)"})},
    })
out.sort(key=lambda m: m["mindId"])
json.dump({"about": "Minds matched to game ids on the Gamerch wiki: rarity, cost, flat stats at Lv1 and, where known, Lv10"
                    " (numbers only). See docs/CONTENT.md.",
           "sources": [{"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine snapshot)", "page": "https://jojodr.gamerch.com/",
                        "accessed": datetime.date.today().isoformat()},
                       {"source": "jdr-research", "doc": "docs/research/16-minds.md §9",
                        "note": "Diamond Records Preservation (PC) data: Lv10 stats the wiki leaves blank for 9 Minds"
                                " (original-game screenshots at Lv10, an archived translation), cited per row in"
                                " lv10Source / lv1Source; numbers only, nothing copied"}],
           "minds": out}, open(a.out, "w"), indent=1, ensure_ascii=False)
print(f"{len(out)} minds, {sum('lv10' in m for m in out)} with Lv10; Lv1 default ratio {RATIO}")
if held:
    print("loose name matches held (not checked yet):", " ".join(sorted(held)))
