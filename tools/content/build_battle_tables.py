#!/usr/bin/env python3
"""Proposed content files (repo content format) for the battle-only tables.
usage: python3 -I tools/content/build_battle_tables.py local/content/battle_assets.json local/content/statue_heights.txt server/content/masterdata
Inputs are identifier inventories of the player's own files (model names, bullet ids, BLETList
owner models); no values are copied from them. All numbers not derived from those are our defaults."""
import json, sys, os, re

def jhash(s):  # System.String.GetHashCode in this il2cpp build (0x13D8EA8) == Java hashCode, int32
    h = 0
    for ch in s.encode('utf-16-le').decode('utf-16-le'):
        h = (h * 31 + ord(ch)) & 0xFFFFFFFF
    return h - (1 << 32) if h & 0x80000000 else h

inv = json.load(open(sys.argv[1]))
heights = {}
for l in open(sys.argv[2]):
    p = l.split()
    if len(p) == 3: heights[p[2]] = float(p[1])
out = sys.argv[3]
RES = {"source": "jdr-research", "doc": "docs/research/12-master-data-fill.md §2",
       "note": "reader behaviour and addresses: docs/research/12-master-data-fill.md §2"}
def write(asset, about, sources, data):
    json.dump({"asset": asset, "about": about, "sources": sources, "data": data},
              open(os.path.join(out, asset + ".json"), "w"), indent=1)

# ---- model_table
models = sorted({m for x in inv.values() for m in x["models"]})
rows, seen = [], {}
for m in models:
    h = jhash(m)
    assert h not in seen, (m, seen.get(h))
    seen[h] = m
    stand = m[5] == "1"
    small = heights.get(m, 9) < 1.0
    rows.append({"modelIdHash": h, "commonAnimeType": 2, "characterHitColSize": 0.5, "attackHitColSize": 0.5,
                 "moveSpeed": 5.0, "turnSpeed": 720.0, "damageReactionTableType": 0, "hpbarShowOffsetY": 2.2,
                 "modelAttributeType": 2 if stand else 1, "autoPosMoveSpeedRate": 1.0, "enigmaScreen": False,
                 "gender": 0, "dtBattleHeight": 1 if small else 0})
write("model_table",
      "One row per model in the player's statue_<psid> bundles; modelIdHash = String.GetHashCode(model name) (Java-style 31*h+c). The tactical battle reads only damageReactionTableType (0 = dtbattle_reaction_table_normal), modelAttributeType (Stand turns shadows off) and dtBattleHeight (Small for models under 1 m). Other fields are action-battle only.",
      [RES, {"source": "jdr-research", "doc": "docs/research/12-master-data-fill.md §2", "note": "model names from the player's statue bundles; heights measured from the model bounds (F-statue-positions)"},
       {"source": "jdr-default", "note": "action-battle-only sizes/speeds and the 1 m Small threshold are ours"}],
      {"paramList": rows})
nsmall = sum(r["dtBattleHeight"] for r in rows)

# ---- bullet_parameter
brow, cnt_owned, cnt_all = [], 0, 0
for ps, x in sorted(inv.items()):
    owners = {}
    for anim, b in x["blet"]:
        for m in x["models"]:
            if anim.startswith(m + "_"): owners.setdefault(b, set()).add(m)
    for b in sorted(set(x["dt_bullets"])):
        own = owners.get(b) or set(x["models"])
        if b in owners: cnt_owned += 1
        else: cnt_all += 1
        for m in sorted(own):
            brow.append({"ownerModelIdHash": jhash(m), "bulletId": b, "poolSize": 8, "onPoolEmptyBehaviour": 2,
                         "lifeTime": 5.0, "dieAtCharaHit": True, "dieAtMapHit": False, "dieAtBulletErasableAttackHit": False,
                         "mapCollisionMode": False, "stopMoveOnDying": True, "mapHitScale": 1.0,
                         "frameEventIdIn": 0, "frameEventIdLoop": 0, "frameEventIdOut": 0, "waitInFrameEventEnd": False,
                         "dieAtOwnerDamageReaction": False, "mapCollisionModeBoxShape": False, "checkMagnetForcePull": False})
write("bullet_parameter",
      "One row per (owner model, bulletId). bulletIds are the bullet_<id> FSMs in the player's dt_actionset_statue_<psid>; the owner model comes from the BLETList (bullet launch) events in frameevents_statue_battle_<psid> when listed there, otherwise every model of that statue. ownerModelIdHash = String.GetHashCode(model name). The tactical battle builds one pool per row (CreateBulletPool) and kills a launched bullet at once when its row is missing. Frame-event ids 0 = GameConst.InvalidFrameEventId (no bullet effect).",
      [RES, {"source": "jdr-default", "note": "pool size, life time and flags are ours"}], {"param": brow})

# ---- reaction tables (index = Jjs.AttackReactionType)
MAP = ["", "DAMAGE_S", "DAMAGE_B", "DAMAGE_HVY", "DAMAGE_BLOW", "DAMAGE_BIND", "DAMAGE_PARALYSIS", "DOWN",
       "DOWN", "DOWN", "DAMAGE_B", "DAMAGE_INVISIBLE", "DAMAGE_VISIBLE", "DIE", "DIE_DOWN"]
def table(no_blow, dt):
    ps = []
    for i, a in enumerate(MAP):
        if no_blow and a in ("DAMAGE_BLOW", "DOWN"): a = "DAMAGE_B"
        vanish = dt and 1 <= i <= 12
        it = {"actionSetName": a, "actionSetEntryId": 0, "standVanish": vanish}
        ps.append({"item": [it, dict(it), dict(it)]})
    return {"param": ps}
for dt in (False, True):
    for v, nb in (("normal", False), ("mini", False), ("mini_no_blow", True)):
        name = ("dtbattle_" if dt else "") + "reaction_table_" + v
        write(name,
              "Row i = Jjs.AttackReactionType i (None, DamageSmall, DamageBig, DamageHeavy, DamageBlow, DamageBind, DamageParalysis, Down, DamageBindDown, DamageParalysisDown, DamagePull, DamageInvidible, DamageVidible, DIE, DIE_DOWN). Action names are the common action sets in the player's (dt_)actionset_common (common_NNN_<NAME>), entry 0 as the sets' own DoChangeActionSet calls use. The tactical battle reads item[0]; the action battle reads item[0..2] by receiver status, so every row has 3 items. Without the DIE rows a killed tactical unit never runs DoRetireCharacter.",
              [RES, {"source": "jdr-default", "note": "the type->action mapping and standVanish choice are ours"}],
              table(nb, dt))

# ---- old-battle / pvp scalars
write("battle_damage_param", "Action-battle damage constants; row 0 normal, 1 PvP, 2 PvP AI arena (AttackDamageCalculator.CalculateDamage). Not read by the tactical battle.",
      [RES, {"source": "jdr-default", "note": "all values ours"}],
      {"param": [{"OffensePowerCoeff": 100, "DefensePowerCoeff": 100, "MinimumDamageGuarantee": 0.05,
                  "CriticalDamagePercent": 150, "RandomDamagePercent": 5, "DefensePowerIgnoreCounterCoeff": 0}] * 3})
write("battle_combo_param", "PvP (action battle) combo damage multipliers by combo step; the client clamps the index to Count-1, so lists must not be empty. Not read by the tactical battle.",
      [RES, {"source": "jdr-default", "note": "neutral 1.0 multipliers"}],
      {"magnification": [1.0], "magnificationAIAutoBattle": [1.0]})
write("battle_pvp_statue_status_parameter", "PvP (action battle) HP/ATK/DEF multipliers. Not read by the tactical battle.",
      [RES, {"source": "jdr-default", "note": "neutral 1.0"}],
      {"hpRate": 1.0, "offenseRate": 1.0, "defenseRate": 1.0, "hpAIAutoBattleRate": 1.0,
       "offenseAIAutoBattleRate": 1.0, "defenseAIAutoBattleRate": 1.0})
print(f"models {len(rows)} (small {nsmall}, stand {sum(r['modelAttributeType']==2 for r in rows)}); "
      f"bullet rows {len(brow)} (owner from BLET {cnt_owned}, all-models {cnt_all})")
