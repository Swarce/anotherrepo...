#!/usr/bin/env python3
"""Parse statue facts from a local cache of the ShoutWiki Diamond Records wiki (CC BY-SA 3.0).

Input: pages.json as fetched from its MediaWiki API ({title: {revid, timestamp, content}}).
Output: one record per statue page with numbers only (stats, cooldowns, ability values) and the
source page + revision. Descriptions/prose are not extracted.

Assist ability (Build 41): the section is an h3 (=== Assist Ability ===) or, on pages with
{{Article|...|...}} tabs, an h2 (== Assist Ability ==) in the second tab; it ends at the next heading,
the tab's closing }} or the categories. `assist_condition` lists the statue types its requirement
shows, one per required assist (e.g. ["Tactical", "Tactical"]), or null when the section has none.
The requirement is the line after "Requirement:", "Activation Condition:" or "Activation Type
Requirements:" (the icons may be on the next line), or a bare line of type icons; icons are
{{Type}} templates or [[File:Type.png]] images.

usage: python3 -I tools/content/shoutwiki_statues.py <pages.json> <out.json>
  e.g. local/wiki-raw/shoutwiki/pages.json -> statues.json (statue pages), and
       local/wiki-raw/shoutwiki/oldmode_pages.json -> oldmode_statues.json (the "Old Mode" templates
       of statues whose current page lacks the action-era values; read by build_assist_abilities.py)
"""
import json
import re
import sys
import urllib.parse

pages = json.load(open(sys.argv[1]))
BASE = "https://jojodiamondrecords.shoutwiki.com/wiki/"


def num(s):
    s = (s or "").replace(",", "").strip()
    return int(s) if re.fullmatch(r"\d+", s) else None


def infobox(c):
    m = re.search(r"\{\{Infobox statue(.*?)\n\}\}", c, re.S)
    if not m:
        return None
    out = {}
    for line in m.group(1).splitlines():
        mm = re.match(r"\s*\|\s*(\w+)\s*=\s*(.*)$", line)
        if mm:
            out[mm.group(1)] = mm.group(2).strip()
    return out


def section(c, title):
    m = re.search(r"===\s*" + re.escape(title) + r"\s*===\s*\n(.*?)(?=\n==|\Z)", c, re.S)
    return m.group(1) if m else ""


TYPES = "Courage|Tactical|Fighting[ _]Spirit|Unity|Solitary"
ICON = re.compile(r"\{\{\s*(" + TYPES + r")\s*\}\}|\[\[\s*File:\s*(" + TYPES + r")\.png[^\]]*\]\]", re.I)
REQ = re.compile(r"^(?:Activation\s+(?:Type\s+)?(?:Conditions?|Requirements?)|Requirements?)\s*:\s*(.*)$", re.I)


def assist_section(c):
    m = re.search(r"^==(=?)\s*Assist Ability\s*\1==\s*\n(.*?)(?=^=|^\}\}\s*$|^\[\[Category|\Z)", c, re.S | re.M)
    return m.group(2) if m else ""


def _type_name(s):
    s = s.replace("_", " ")
    return next(t for t in ("Courage", "Tactical", "Fighting Spirit", "Unity", "Solitary") if t.lower() == s.lower())


def _icons(line):
    return [_type_name(m.group(1) or m.group(2)) for m in ICON.finditer(line)]


def _bare(line):
    """A line of type icons only (separators allowed)."""
    return bool(ICON.search(line)) and not re.sub(r"[\s,+&/]|and", "", ICON.sub("", line))


def assist_condition(sec):
    lines = [l.strip() for l in sec.splitlines()]
    for i, line in enumerate(lines):
        m = REQ.match(line)
        if not m:
            continue
        rest = m.group(1).strip()
        if rest:
            got = _icons(rest) or [_type_name(w) for w in re.findall(TYPES, rest, re.I)]
            return got or None
        nxt = next((l for l in lines[i + 1:] if l), "")
        return _icons(nxt) if _bare(nxt) else None
    for line in lines:
        if _bare(line):
            return _icons(line)
    return None


def effects(text):
    """Numeric effect lines like 'ATK +7.5%' (no prose)."""
    out = []
    for line in text.splitlines():
        line = re.sub(r"</?br\s*/?>", "", line).strip()
        m = re.match(r"^(.{0,60}?)\s*([+-]\s*\d+(?:\.\d+)?)\s*(%?)\s*(\[.*\])?$", line)
        if m and not line.startswith("''"):
            out.append({"stat": m.group(1).strip(), "value": float(m.group(2).replace(" ", "")), "percent": bool(m.group(3))})
    return out


def cooldown(text):
    m = re.search(r"[Cc]ooldown[^0-9]{0,20}(\d+(?:\.\d+)?)", text)
    u = re.search(r"[Uu]ses?[^0-9]{0,20}(\d+)", text)
    return {"cooldown": float(m.group(1)) if m else None, "uses": int(u.group(1)) if u else None}


records = []
for title, p in pages.items():
    ib = infobox(p["content"])
    if not ib:
        continue
    c = p["content"]
    rec = {
        "name": ib.get("name"),
        "rarity": num((ib.get("rarity") or "").replace("★", "")),
        "type": ib.get("type"),
        "jp_name": ib.get("jpn"),
        "lv1": {k: num(ib.get("lv1" + k)) for k in ("hp", "atk", "def")},
        "lv99": {k: num(ib.get("lv99" + k)) for k in ("hp", "atk", "def")},
        "max": {k: num(ib.get("max" + k)) for k in ("hp", "atk", "def")},
        "awakenable": (ib.get("awakenable") or "").lower() == "yes",
        "availability": ib.get("availability") or None,
        "release_date": ib.get("releasedate") or None,
        "leader_ability_max": effects(section(c, "MAX Lv. Leader Ability")),
        "ability": effects(section(c, "Ability")),
        "assist_ability": effects(assist_section(c)),
        "assist_condition": assist_condition(assist_section(c)),
        "skill": cooldown(section(c, "Skill")),
        "special_skill": cooldown(section(c, "Special Skill")),
        "source": {
            "site": "Diamond Records Wiki (ShoutWiki)",
            "license": "CC BY-SA 3.0",
            "page": BASE + urllib.parse.quote(title.replace(" ", "_")),
            "revid": p["revid"],
            "revision_time": p["timestamp"],
        },
    }
    records.append(rec)
records.sort(key=lambda r: (r["rarity"] or 0, r["name"] or ""))
json.dump(records, open(sys.argv[2], "w"), indent=1, ensure_ascii=False)
full = sum(1 for r in records if all(r["lv1"].values()) and all(r["lv99"].values()))
print(f"{len(records)} statue records; {full} with Lv1+Lv99 stats; "
      f"{sum(1 for r in records if all(r['max'].values()))} with max stats")
