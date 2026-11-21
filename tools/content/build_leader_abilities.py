#!/usr/bin/env python3
"""Generate leader-ability content for the tactical battle (dtbattle) from the wiki leader-ability
numbers. Ids, enum names and numbers only: no wiki text goes into the output.

usage:
  python3 -I tools/content/build_leader_abilities.py --wiki local/wiki-raw/gamerch/statues.json \
      --labels local/l10n/work2.tsv \
      --affect-table server/content/masterdata/affect_table.json \
      --out-content <dir> --out-schemas <dir> [--server-statues server/content/statues.json] \
      [--report <file.json>]

Writes:
  <out-content>/affect_table.json                 the 3 existing rows + one row per affect id used
  <out-content>/dtbattle_ability_data_table.json  one row DT_LEAD_<statueId> per statue
  <out-content>/leader_abilities.json             {statueId: abilityId} for the server
  <out-schemas>/dtbattle_ability_data_table.json  schema fragment (asset entry only)

Client facts this relies on (libil2cpp RVAs, JoJo DR 3.12.0; see the K report):
  value(level) = RoundToInt(Lerp(paramLv1_i, paramLv10_i, (clamp(level,1,10)-1)/9))
      CharacterAffectParam.GetGeneralValue 0xEDA880 / GetAffectParamValue 0xEDAA44,
      GameConst +0x410/+0x414/+0x418 = 8/1/10 (cctor 0xDB40F0/0xDB42EC/0xDB42F8)
  stat rates: value is per-mille of the stat (DTAffectFixAndRateConfigInfo.CalcConfigValue
      0xCCA5C4: fixed + RoundToInt(permillage * 0.001 * base))
  damage rates: rate = 1 + sum(permillage)/1000, positive = more damage, for both the attacker's
      deal terms and the receiver's receive terms (CalcSkillHpDamage 0xEFDF34..0xEFE1AC)
  crit damage: clamp(1.0 + deal/1000 + receive/1000, lower, upper) (CalcCriticalDamageRate 0xEFF180)
  STYPE rates: param 1 = target/attacker Jjs.StatueType (CalcAffectParam 0xED4458/0xED44C4)
  giveLimit becomes usableCount; 0 means never active (UpdateAffectCondition 0xFB8F0C), so -1.
"""
import argparse, collections, json, os, re, unicodedata

# --- wiki phrase vocabulary (unicode escapes keep this file ASCII) --------------------------------
J_DMG_DEAL = '\u4e0e\u3048\u308b\u30c0\u30e1\u30fc\u30b8'
J_DMG_RECV = '\u53d7\u3051\u308b\u30c0\u30e1\u30fc\u30b8'
J_CRT_DEAL = '\u30af\u30ea\u30c6\u30a3\u30ab\u30eb\u3092\u4e0e\u3048\u305f(?:\u6642|\u3068\u304d)\u306e\u30c0\u30e1\u30fc\u30b8'
J_CRT_RECV = '\u30af\u30ea\u30c6\u30a3\u30ab\u30eb\u3092\u53d7\u3051\u305f\u6642\u306e\u30c0\u30e1\u30fc\u30b8'
J_TIMES = '\u500d'
J_TYPE_DEAL = '(?:\u30bf\u30a4\u30d7)?\u306b\u4e0e\u3048\u308b\u30c0\u30e1\u30fc\u30b8'
J_TYPE_RECV = '(?:\u30bf\u30a4\u30d7)?\u304b\u3089\u53d7\u3051\u308b\u30c0\u30e1\u30fc\u30b8'
STYPE = {  # wiki type name -> (id word, Jjs.StatueType value)
    '\u52c7\u6c17': ('BRAVE', 1), '\u77e5\u7565': ('INTELLI', 2), '\u95d8\u5fd7': ('TENACITY', 3),
    '\u56e3\u7d50': ('UNITY', 4), '\u5b64\u9ad8': ('MAVERICK', 5)}
# condition prefix -> (id word, Jjs.AffectCondition name). All four are evaluated by the client with
# no parameters (UpdateAffectCondition 0xFB8F8C.. : IsMoribundHp = HP <= 30%, all other members
# dead, LeftHpRate <= 0.5, LeftHpRate >= 1.0).
COND = [  # regex (the wiki sometimes drops the comma), id word, condition
    ('\u7015\u6b7b\u6642\u3001?', 'DYING_', 'PRVS_DYING'),
    ('HP\u534a\u6e1b\u6642\u3001?', 'HALF_HP_', 'PRVS_HALF_HP'),
    ('HPMAX\u6642\u3001?', 'FULL_HP_', 'PRVS_FULL_HP'),
    ('\u30c1\u30fc\u30e0\u5185\u306e\u30e1\u30f3\u30d0\u30fc\u304c\u4e00\u4eba\u306b\u306a\u3063\u305f\u6642\u3001?', 'ALONE_', 'PRVS_ALONE'),
    ('\u30c1\u30fc\u30e0\u30e1\u30f3\u30d0\u30fc\u304c\u6b8b\u308a\u4e00\u4eba\u306e\u6642\u3001?', 'ALONE_', 'PRVS_ALONE'),
]
# Left out on purpose (category -> pattern); first match wins, checked before the body table.
EXCLUDE = [
    ('reversal_mode', '\u30c9\u30c9\u30c9'),               # DoDoDo mode/charge: reversal is off
    ('crit_rate_multiplier', '\u30af\u30ea\u30c6\u30a3\u30ab\u30eb\u767a\u751f\u78ba\u7387'),
    ('heal_capped', '\u56de\u5fa9\u91cf'),                  # limitedHealRateMax 1.0 caps it
    ('attribute_filter', '\u3010'),                         # needs tribeTypeList (sent empty)
    ('action_only', '\u79fb\u52d5\u901f\u5ea6|\u30b9\u30c6\u30c3\u30d7|\u30af\u30fc\u30eb\u30bf\u30a4\u30e0|'
                    '\u79d2\u9593|\u53d6\u5f97\u30b4\u30fc\u30eb\u30c9|\u30b9\u30ad\u30eb\u306e\u6027\u80fd|'
                    'SP\u30b9\u30ad\u30eb|\u6575\u3092\u5012\u3059\u5ea6'),
]
NUM = r'([0-9]+(?:\.[0-9]+)?)'
# body -> (id stem, BaseAffectType name, kind). kind: pct (+N%), neg (-N%), mul (N times)
BODY = [
    (r'ATK ?\+' + NUM + '%', 'ATK_RATEADD', 'BASE_ATK_RATECON', 'pct'),
    (r'HP ?\+' + NUM + '%', 'HP_RATEADD', 'BASE_HP_RATECON', 'pct'),
    (r'DEF ?\+' + NUM + '%', 'DEF_RATEADD', 'BASE_DEF_RATECON', 'pct'),
    (J_DMG_DEAL + r':?\+' + NUM + '%', 'DMG_RATEADD', 'BASE_DT_DMG_RATECON', 'pct'),
    (J_DMG_RECV + r'-' + NUM + '%', 'VCT_DAM_RATESUB', 'BASE_DT_VCT_DAM_RATECON', 'neg'),
    (J_CRT_DEAL + NUM + J_TIMES, 'CRT_DMG_RATEADD', 'BASE_CRT_DMG_RATECON', 'mul'),
    (J_CRT_RECV + NUM + J_TIMES, 'VCT_CRT_DMG_RATESUB', 'BASE_VCT_CRT_DMG_RATECON', 'mul'),
]
for jp, (word, val) in STYPE.items():
    BODY.append((jp + J_TYPE_DEAL + r':?\+' + NUM + '%', word + '_STYPE_DMG_RATEADD',
                 'BASE_DT_STYPE_DMG_RATECON', 'pct', val))
    BODY.append((jp + J_TYPE_RECV + r'-' + NUM + '%', word + '_STYPE_VCT_DAM_RATESUB',
                 'BASE_DT_STYPE_VCT_DAM_RATECON', 'neg', val))
MAXRE = re.compile(r'\((?:MAX ?)?([+-]?)' + NUM + r'(?:%|' + J_TIMES + r')?\)')
MAXJUNK = re.compile(r'\(MAX[^)]*\)')  # e.g. '(MAX+?%)': no usable Lv10 value


def norm(s):
    return unicodedata.normalize('NFKC', s).replace('\u2212', '-').strip()


def plausible_max(v1, v10, neutral):
    """MAX must lie on the same side of neutral, at least as far out, and at most 3x as far."""
    d1, d10 = v1 - neutral, v10 - neutral
    if d1 == 0 or d1 * d10 <= 0:
        return False
    return abs(d1) <= abs(d10) <= 3 * abs(d1)


def permille(kind, v):
    if kind == 'pct':
        return round(v * 10)
    if kind == 'neg':
        return -round(v * 10)
    return round((v - 1.0) * 1000)  # mul


def parse(effect):
    """-> ('ok', param dict) | (category, None)"""
    e = norm(effect)
    m = MAXRE.search(e)
    body = MAXJUNK.sub('', MAXRE.sub('', e)).strip()
    for cat, rx in EXCLUDE:
        if re.search(rx, body):
            return cat, None
    cond_word, prvs = '', 'PRVS_NONE'
    for pre, word, name in COND:
        cm = re.match(pre, body)
        if cm:
            body, cond_word, prvs = body[cm.end():], word, name
            break
    for row in BODY:
        rx, stem, base, kind = row[:4]
        stype = row[4] if len(row) > 4 else 0
        mm = re.fullmatch(rx, body)
        if not mm:
            continue
        v1 = float(mm.group(1))
        neutral = 1.0 if kind == 'mul' else 0.0
        v10, max_note = v1, 'no_max'
        if m:
            cand = float(m.group(2))
            if plausible_max(v1, cand, neutral):
                v10, max_note = cand, 'max'
            else:
                max_note = 'max_rejected'
        p1, p10 = permille(kind, v1), permille(kind, v10)
        return 'ok', {'affectId': 'DTAF_' + cond_word + stem, 'baseAffect': base, 'prvs': prvs,
                      'lv1': p1, 'lv10': p10, 'stype': stype, 'max': max_note}
    return 'unmatched', None


def sec_params(lv1, lv10, stype):
    p = {'affectId': None, 'giveLimit': -1}
    for i in range(8):
        p['paramLv1_%d' % i] = 0
    for i in range(8):
        p['paramLv10_%d' % i] = 0
    p['paramLv1_0'], p['paramLv10_0'] = lv1, lv10
    if stype:
        p['paramLv1_1'] = p['paramLv10_1'] = stype
    return p


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--wiki', required=True)
    ap.add_argument('--labels', required=True)
    ap.add_argument('--affect-table', required=True)
    ap.add_argument('--out-content', required=True)
    ap.add_argument('--out-schemas', required=True)
    ap.add_argument('--server-statues')
    ap.add_argument('--report')
    a = ap.parse_args()

    wiki = json.load(open(a.wiki, encoding='utf-8'))
    keys = set()
    for line in open(a.labels, encoding='utf-8'):
        k = line.split('\t', 1)[0]
        if ':ABI_LEAD_NAME_' in k:
            keys.add(k.rsplit(':', 1)[1])
    base_affect = json.load(open(a.affect_table, encoding='utf-8'))

    stats = collections.Counter()
    per_cat = collections.Counter()
    affects = {}          # affectId -> (baseAffect, prvs)
    rows, mapping, seen = [], {}, set()
    for s in wiki['statues']:
        sid, vals = s.get('statue_id'), s.get('leader_ability_values') or []
        if not sid or not vals:
            stats['statue_no_id_or_values'] += 1
            continue
        if sid in seen:
            stats['statue_duplicate'] += 1
            continue
        seen.add(sid)
        name = 'ABI_LEAD_NAME_' + sid
        if name not in keys:
            stats['statue_no_label_key'] += 1
            continue
        params = collections.OrderedDict()
        for e in vals:
            cat, p = parse(e)
            per_cat[cat] += 1
            if p is None:
                continue
            stats['effect_' + p['max']] += 1
            prev = affects.setdefault(p['affectId'], (p['baseAffect'], p['prvs']))
            assert prev == (p['baseAffect'], p['prvs']), p['affectId']
            key = (p['affectId'], p['stype'])
            if key in params:  # same effect twice in one ability: the client sums them anyway
                params[key]['lv1'] += p['lv1']
                params[key]['lv10'] += p['lv10']
                stats['effect_merged'] += 1
            else:
                params[key] = p
        if not params:
            stats['statue_nothing_verified'] += 1
            continue
        plist = []
        for p in params.values():
            q = sec_params(p['lv1'], p['lv10'], p['stype'])
            q['affectId'] = p['affectId']
            plist.append(q)
        aid = 'DT_LEAD_' + sid
        rows.append({'abilityId': aid, 'abilityName': name, 'abilityParamList': plist})
        mapping[sid] = aid
        stats['statue_with_row'] += 1
    rows.sort(key=lambda r: r['abilityId'])

    wiki_src = {'source': 'Gamerch JoJo DR wiki (Wayback Machine snapshot)',
                'page': wiki.get('meta', {}).get('site', 'https://jojodr.gamerch.com') + '/',
                'accessed': wiki.get('meta', {}).get('accessed', ''),
                'note': 'leader-ability numbers only (Lv1 value and the MAX value as Lv10); per-record archive URLs are in local/wiki-raw/gamerch/statues.json'}
    research = {'source': 'jdr-research', 'doc': 'research K: leader abilities (dtbattle affect units and the server-to-affect path)',
                'note': 'units, slots and conditions read in the client code; see scripts/K_abilities.py header'}

    # affect_table: keep the existing rows, add ours
    old = base_affect['data']['paramList']
    old_ids = {r['affectId'] for r in old}
    new_rows = [{'affectId': i, 'affectTextId': '', 'prvs': affects[i][1], 'baseAffect': affects[i][0],
                 'affectLimitNormal': [], 'affectLimitExtra': [], 'textSortIndex': 0}
                for i in sorted(affects) if i not in old_ids]
    affect_doc = {
        'asset': 'affect_table',
        'about': 'The three affect ids the client names in code, plus one row per affect id used by the leader abilities in dtbattle_ability_data_table. Each id names one BaseAffectType and one AffectCondition; the amount comes from the ability row (per-mille, negative for the *_SUB ids). Empty limit lists mean no cap. affectTextId is empty: the player\'s labels have no text key for these plain stat ids.',
        'sources': base_affect.get('sources', []) + [research,
                    {'source': 'jdr-default', 'note': 'affect id names are ours, in the DTAF_[COND_]STAT_OP style of the client\'s DTAF_* label keys; textSortIndex 0'}],
        'data': {'paramList': old + new_rows}}
    table_doc = {
        'asset': 'dtbattle_ability_data_table',
        'about': 'Leader abilities for the tactical battle: one row DT_LEAD_<statueId> per statue whose wiki leader ability has at least one effect with verified units. abilityName is the ABI_LEAD_NAME_<statueId> label key. paramLv1_0/paramLv10_0 are per-mille at ability level 1 and 10 (the client lerps between them); paramLv*_1 is the Jjs.StatueType for the *_STYPE_* affects. giveLimit -1 = unlimited (0 would never apply).',
        'sources': [wiki_src, research,
                    {'source': 'jdr-default', 'note': 'ability ids DT_LEAD_<statueId> are ours (the server sends the same ids); where the wiki MAX value is missing or implausible Lv10 = Lv1'}],
        'data': {'paramList': rows}}
    map_doc = {
        'asset': 'leader_abilities',
        'about': 'Server-side: the dtLeaderAbilityInfo.abilityId to send for each statueId (send {abilityId, abilityLevel 1..10, abilityName "", abilityParamList [], isInit false}). Statues not listed send {} as today.',
        'sources': [wiki_src, research],
        'data': mapping}
    schema_frag = {
        '_note': 'Same class as ability_data_table (Jjs.AbilityDataTable); its types are already in schema.json, so only the asset entry is new. affect_table needs no new types: every BaseAffectType and AffectCondition name used is already in the committed enum maps.',
        'assets': [{'name': 'dtbattle_ability_data_table', 'class': 'Jjs.AbilityDataTable',
                    'core': False, 'confidence': 'H'}],
        'types': {}}

    os.makedirs(a.out_content, exist_ok=True)
    os.makedirs(a.out_schemas, exist_ok=True)
    def dump(obj, path):
        with open(path, 'w', encoding='ascii') as f:
            json.dump(obj, f, indent=1, ensure_ascii=True)
            f.write('\n')
    dump(affect_doc, os.path.join(a.out_content, 'affect_table.json'))
    dump(table_doc, os.path.join(a.out_content, 'dtbattle_ability_data_table.json'))
    dump(map_doc, os.path.join(a.out_content, 'leader_abilities.json'))
    dump(schema_frag, os.path.join(a.out_schemas, 'dtbattle_ability_data_table.json'))

    by_affect = collections.Counter(q['affectId'] for r in rows for q in r['abilityParamList'])
    report = {'stats': dict(stats), 'effects_by_category': dict(per_cat),
              'params_by_affect': dict(sorted(by_affect.items())),
              'affect_rows_added': len(new_rows)}
    if a.server_statues:
        srv = json.load(open(a.server_statues, encoding='utf-8'))
        lst = srv.get('data', srv)
        lst = lst.get('statues', lst) if isinstance(lst, dict) else lst
        ids = {x.get('statueId') or x.get('id') for x in lst if isinstance(x, dict)}
        ids.discard(None)
        report['server_statues'] = len(ids)
        report['server_statues_with_leader_ability'] = len(ids & set(mapping))
    print(json.dumps(report, indent=1))
    if a.report:
        dump(report, a.report)


if __name__ == '__main__':
    main()
