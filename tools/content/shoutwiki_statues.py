#!/usr/bin/env python3
"""Parse statue facts from a local cache of the ShoutWiki Diamond Records wiki (CC BY-SA 3.0).

Input: pages.json as fetched from its MediaWiki API ({title: {revid, timestamp, content}}).
Output: one record per statue page with numbers only (stats, cooldowns, ability values) and the
source page + revision. Descriptions/prose are not extracted.

usage: python3 -I tools/content/shoutwiki_statues.py <pages.json> <out.json>
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


def effects(text):
    """Numeric effect lines like 'ATK +7.5%' (no prose)."""
    out = []
    for line in text.splitlines():
        line = line.strip()
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
        "assist_ability": effects(section(c, "Assist Ability")),
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
