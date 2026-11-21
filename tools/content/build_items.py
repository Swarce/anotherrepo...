#!/usr/bin/env python3
"""Build server/content/items.json: the items the offline server can hold, give and sell
(docs/research/15-items-and-enhancement.md).

Identifiers only, from the player's files: an item is listed when it has both an icon
(`it_<id:04>` in `ui_item_00`; pass the bundle's container listing) and a name label
(`ITEM_<id:04>_NAME`, our English already imported); profile currencies (0, 10, 20, 40) are left
out because kind() has no range for them. The first categories follow the id ranges read in the
client; the later ones (town, ap, awakening, honor, event_point, event, 4001-4006, raid_energy,
mind_box, awakening_medal) are ours, by id range from the item labels; EXP values come from the wiki where it gives them (bronze 1,000, silver 4,000, rainbow
64,000 per tier), the gold tier (16,000), platinum and all gold costs are ours.

usage: python3 -I tools/content/build_items.py --icons <ui_item_00 listing> --out server/content/items.json
"""
import argparse, glob, json, re

ap = argparse.ArgumentParser()
ap.add_argument("--icons", required=True, help="container listing of ui_item_00 (lines with it_<id>.bytes)")
ap.add_argument("--out", required=True)
a = ap.parse_args()

icons = {int(m.group(1)) for m in re.finditer(r"/it_(\d{4})\.bytes", open(a.icons).read())}
names = set()
for p in glob.glob("server/content/l10n/en/*.json"):
    for k in json.load(open(p))["entries"]:
        m = re.fullmatch(r"text:Item_Text_JP:ITEM_(\d{4})_NAME", k)
        if m:
            names.add(int(m.group(1)))
ids = sorted(icons & names)


def kind(i):
    """(category, ItemType) by id range (client 3.12.0; ItemType is not read by the build code)."""
    if i == 31:
        return "ring", 11
    if 50 <= i <= 289:
        return "ticket", 6
    if 300 <= i <= 306:
        return "summon_medal", 5
    if i in (1021, 1022, 1023, 1038, 1047, 1048):
        return "key", 8
    if 1002 <= i <= 1050 or 1200 <= i <= 1202:
        return "tree", 7
    if 6100 <= i <= 6138:
        return "exp", 0
    if 3000 <= i <= 3184:
        return "exchange", 11
    if 1100 <= i <= 1102:  # BP Recovery Cola XS/S/normal (1/2/5 BP, their detail texts)
        return "bp", 14  # ENERGY_RECOVER_ADDITION
    # The ranges below are ours, read from the item labels (ITEM_<id>_NAME/_DETAIL), not from client
    # code. ItemType is set only where a Jjs.ItemType enum name matches the item text; else 0.
    if i == 41:  # Town Medal (Town donations)
        return "town", 0
    if i in (1001, 7001):  # Sandwich and friends: restore AP
        return "ap", 0
    if i == 1301 or 8500 <= i <= 8502:  # Awakening Medal Shard, awakening idols
        return "awakening", 0
    if 1500 <= i <= 1533:  # certificates (honors)
        return "honor", 0
    if i == 2000:
        return "event_point", 12  # EVENT_POINT
    if i == 2001:
        return "event_point", 0
    if 2002 <= i <= 2013:
        return "event", 11  # EVENT_ITEM
    if 4001 <= i <= 4003 or i == 4006:
        return "exchange", 11  # EVENT_ITEM
    if 4004 <= i <= 4005:  # Giveback / Thanks Fest Points: summon points (GASYA_POINT 5 not applied)
        return "exchange", 0
    if 5001 <= i <= 5002:  # Raid Energy recovery
        return "raid_energy", 0
    if i == 6001:
        return "mind_box", 15  # MIND_BOX_EXTEND
    if 8600 <= i <= 8749:  # Awakening Medals (only those with an icon pass)
        return "awakening_medal", 0
    return None, 0


# EXP materials: per tier (bronze, silver, gold, rainbow). Wiki: 1,000 / 4,000 / 64,000; gold ours.
TIER_EXP = [1000, 4000, 16000, 64000]
TIER_GOLD = [100, 400, 1600, 6400]  # ours
ATTR = [(6100, 4), (6104, 5), (6108, 6), (6120, 11), (6124, 12)]  # first id, BonusCondition


def exp_fields(i):
    for first, cond in ATTR:
        if first <= i < first + 4:
            t = i - first
            return dict(exp=TIER_EXP[t], bonusCondition=cond, bonusRate=120, useGold=TIER_GOLD[t], stack=0)
    if 6112 <= i <= 6115:  # trophies: favour statues
        t = i - 6112
        return dict(exp=TIER_EXP[t], bonusCondition=2, bonusRate=120, useGold=TIER_GOLD[t], stack=0)
    if 6116 <= i <= 6119:  # Mind medals: Minds only
        t = i - 6116
        return dict(exp=TIER_EXP[t], bonusCondition=3, bonusRate=120, useGold=TIER_GOLD[t], stack=0)
    if 6128 <= i <= 6134:  # platinum: x5 on the matching attribute (detail text); base ours
        cond = [4, 5, 6, 11, 12, 2, 3][i - 6128]
        return dict(exp=64000, bonusCondition=cond, bonusRate=500, useGold=6400, stack=0)
    if 6135 <= i <= 6138:  # turtles: +1 boost on a statue of that rarity
        return dict(exp=0, bonusCondition=0, bonusRate=100, useGold=10000, stack=3 + (i - 6135))
    return None


items = []
for prio, i in enumerate(ids):
    cat, typ = kind(i)
    if not cat:
        continue
    rec = {"itemId": i, "category": cat, "type": typ, "priority": prio + 1}
    if cat == "exp":
        f = exp_fields(i)
        if not f:
            continue
        rec.update(f)
    items.append(rec)

doc = {"about": "Items with an icon and a name in the player's files; categories by id range (client for ring, "
                "ticket, summon_medal, key, tree, exp, exchange 3000-3184 and bp; ours from the item labels for "
                "the rest, with ItemType only where an enum name matches the item text); "
                "EXP values per tier from the wiki where known, the rest ours. See docs/research/15-items-and-enhancement.md.",
       "sources": [{"source": "jdr-default", "note": "generated by tools/content/build_items.py; ids from the player's "
                    "ui_item_00 icons and the retail ITEM_<id>_NAME labels; the town, ap, awakening, honor, event_point, "
                    "event, exchange 4001-4006, raid_energy, mind_box and awakening_medal ranges are ours, from the labels"},
                   {"source": "Gamerch ジョジョDR攻略Wiki (Wayback Machine)",
                    "page": "https://jojodr.gamerch.com/", "note": "medal EXP: bronze 1,000, silver 4,000, rainbow 64,000; same attribute x1.2"}],
       "items": items}
json.dump(doc, open(a.out, "w"), indent=0)
from collections import Counter
print(len(items), "items:", dict(Counter(x["category"] for x in items)))
