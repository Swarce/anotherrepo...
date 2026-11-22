#!/usr/bin/env python3
"""Generate leader-ability content for the tactical battle (dtbattle) from the wiki leader-ability
numbers. Ids, enum names and numbers only: no wiki text goes into the output.

usage:
  python3 -I tools/content/build_leader_abilities.py --wiki local/wiki-raw/gamerch/statues.json \
      --labels local/l10n/work2.tsv \
      --affect-table server/content/masterdata/affect_table.json \
      --out-content <dir> --out-schemas <dir> [--server-statues server/content/statues.json] \
      [--research local/wiki-raw/research-b33/action/results.jsonl] [--report <file.json>]

  Rerun on the committed table (Build 41; replaces only the DT_LEAD_* rows, keeps the Mind, assist and
  Mind-link rows and the file's about/sources, appends any new affect row to --affect-table):
  python3 -I tools/content/build_leader_abilities.py --wiki local/wiki-raw/gamerch/statues.json \
      --labels local/l10n/export-b33.tsv \
      --affect-table server/content/masterdata/affect_table.json \
      --server-statues server/content/statues.json \
      --research local/wiki-raw/research-b33/action/results.jsonl \
      --research local/wiki-raw/research-b43/assists/results.jsonl \
      --table server/content/masterdata/dtbattle_ability_data_table.json

Writes:
  <out-content>/affect_table.json                 the 3 existing rows + one row per affect id used
  <out-content>/dtbattle_ability_data_table.json  one row DT_LEAD_<statueId> per statue
  <out-content>/leader_abilities.json             {statueId: abilityId} for the server
  <out-schemas>/dtbattle_ability_data_table.json  schema fragment (asset entry only)

--research (Build 41): leader_ability records of a research pass (results.jsonl) taken from a live
Gamerch statue page (url https://jojodr.gamerch.com/<title>), for statues the --wiki dump does not
give. The record's statueId is not trusted: the page's leader name must equal one ABI_LEAD_NAME
label (after NFKC), and when several statues share that label, the page title's rarity (★N) and
type (【勇気】...) must pick exactly one roster statue whose rarity and type are both known
(--server-statues). So 3001_13, which shares 3001_03's label but has no known rarity or type, never
gets 3001_03's row. The same effect rules apply. Build 43: a record may give 'lv1' and 'max' lists
instead of 'effects' (read as 'line(MAX n)'); where two records match one statue the higher
confidence wins; 3041_03 and 4000_23 are no longer held. A high-confidence ShoutWiki record whose
level is 'MAX (Reversal/DR2)' (research-b43/assists) gives a row to the one roster statue whose
statues.json source is that page, for statues the Gamerch sources lack: its English lines go through
EN_LINES, and Lv1 = Lv10 = the MAX value (no Lv1 is published).

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
import argparse, collections, json, os, re, unicodedata, urllib.parse

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

# --research matches held for a later check (Build 41): these statues gained the rarity and type that
# make them match only after the plan that verified the research rows was made, so their pages have
# not been checked against the live wiki yet.
# Build 43: 3041_03 and 4000_23 released (research-b43/assists re-read both live pages: leader name,
# rarity, type and Lv99 as the roster).
RESEARCH_HELD = {}
RANK = {'high': 3, 'medium': 2}

# --research ShoutWiki MAX values (Build 43): a statue page's 'MAX Lv. Leader Ability' of the
# Reversal (tactical) era, in English, read into the Gamerch line form that parse() reads. Lines that
# match none of these (reversal gauge, an HP-above-half condition, trait filters) are not used.
SHOUTWIKI = 'https://jojodiamondrecords.shoutwiki.com/wiki/'
EN_TYPE = {'Courage': '\u52c7\u6c17', 'Tactical': '\u77e5\u7565', 'Fighting Spirit': '\u95d8\u5fd7',
           'Unity': '\u56e3\u7d50', 'Solitary': '\u5b64\u9ad8'}
EN_LINES = [
    (r'(HP|ATK|DEF) ?\+ ?([0-9.]+)%', lambda m: '%s+%s%%' % (m.group(1), m.group(2))),
    (r'Damage (?:dealt )?to (' + '|'.join(EN_TYPE) + r')(?: Type)? ?\+ ?([0-9.]+)%',
     lambda m: EN_TYPE[m.group(1)] + J_TYPE_DEAL.replace('(?:\u30bf\u30a4\u30d7)?', '\u30bf\u30a4\u30d7')
     + '+%s%%' % m.group(2)),
    (r'When HP is halved, (HP|ATK|DEF) ?\+ ?([0-9.]+)%',
     lambda m: 'HP\u534a\u6e1b\u6642\u3001%s+%s%%' % (m.group(1), m.group(2))),
]


def en_line(e):
    for rx, fn in EN_LINES:
        m = re.fullmatch(rx, unicodedata.normalize('NFKC', e).strip(), re.I)
        if m:
            return fn(m)
    return None


def record_lines(v):
    """A leader_ability record's lines: 'effects', else (research-b43) 'lv1' with the matching 'max'
    line's number as '(MAX n)', the form the Gamerch pages print."""
    if v.get('effects'):
        return v['effects']
    lv1, mx = v.get('lv1') or [], v.get('max') or []
    out = []
    for i, line in enumerate(lv1):
        m = re.search(r'([+-]?[0-9]+(?:\.[0-9]+)?)(%|\u500d)?$', norm(mx[i])) if i < len(mx) else None
        out.append(line + ('(MAX%s%s)' % (m.group(1), m.group(2) or '') if m else ''))
    return out

# affectTextId of a new affect row: the game's affectText_JP label AFFECT_<cond>_<stat>_TEXT with
# the same meaning (docs/research/16-minds.md section 8). DYING takes the near-death (FNT) labels,
# FULL_HP the FULL ones, HALF_HP the HALF ones (Build 43, the first HALF_HP leader row).
def text_id(affect_id):
    s = affect_id[len('DTAF_'):]
    s = s.replace('DYING_', 'FNT_', 1).replace('FULL_HP_', 'FULL_', 1).replace('HALF_HP_', 'HALF_', 1)
    return 'AFFECT_' + s + '_TEXT'


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
    ap.add_argument('--out-content')
    ap.add_argument('--out-schemas')
    ap.add_argument('--table', help='merge the DT_LEAD_* rows into this table in place (and new affect '
                    'rows into --affect-table) instead of writing --out-content/--out-schemas')
    ap.add_argument('--server-statues')
    ap.add_argument('--research', action='append', default=[],
                    help='research results.jsonl with leader_ability records from live Gamerch pages')
    ap.add_argument('--report')
    a = ap.parse_args()
    if not a.table and not (a.out_content and a.out_schemas):
        ap.error('give --table, or --out-content and --out-schemas')
    if a.research and not a.server_statues:
        ap.error('--research needs --server-statues (rarity and type pick the statue)')

    wiki = json.load(open(a.wiki, encoding='utf-8'))
    keys, lead_jp = set(), collections.defaultdict(list)
    for line in open(a.labels, encoding='utf-8'):
        c = line.rstrip('\n').split('\t')
        k = c[0]
        if ':ABI_LEAD_NAME_' in k:
            keys.add(k.rsplit(':', 1)[1])
            if len(c) >= 4 and c[3]:
                lead_jp[norm(c[3])].append(k.rsplit('ABI_LEAD_NAME_', 1)[1])
    base_affect = json.load(open(a.affect_table, encoding='utf-8'))
    srv_by_id = {}
    if a.server_statues:
        srv = json.load(open(a.server_statues, encoding='utf-8'))
        lst = srv.get('data', srv)
        lst = lst.get('statues', lst) if isinstance(lst, dict) else lst
        srv_by_id = {x.get('statueId'): x for x in lst if isinstance(x, dict)}

    # Research records -> wiki-shaped entries (after the --wiki statues, which take precedence).
    research_rows, held, shoutwiki_rows = [], set(), []
    page_ids = collections.defaultdict(list)
    for i, x in srv_by_id.items():
        pg = (x.get('source') or {}).get('page') or ''
        if i and pg.startswith(SHOUTWIKI):
            page_ids[urllib.parse.unquote(pg)].append(i)
    TITLE = re.compile('\u3010(' + '|'.join(STYPE) + ')\u3011.*\u2605([0-9])')
    for path in a.research:
        for line in open(path, encoding='utf-8'):
            r = json.loads(line)
            v, url = r.get('value'), r.get('url') or ''
            if r.get('field') == 'leader_ability' and r.get('confidence') == 'high' and isinstance(v, dict) \
                    and str(v.get('level', '')).startswith('MAX (Reversal') and url.startswith(SHOUTWIKI):
                # The statue is the one roster statue whose stats cite this page (never the record's
                # statueId); Lv1 is not published, so Lv1 = Lv10 = the MAX value.
                page = url.split(' (')[0]
                ids = page_ids.get(urllib.parse.unquote(page), [])
                rev = re.search(r'\(revid (\d+)', url)
                lines = [en_line(e) for e in v.get('effects') or []]
                if len(ids) == 1 and any(lines):
                    shoutwiki_rows.append({'statue_id': ids[0], 'leader_ability_values': [x for x in lines if x],
                                           '_url': url, '_page': page, '_revid': int(rev.group(1)) if rev else None,
                                           '_accessed': r.get('accessed', ''), '_kind': 'shoutwiki'})
                continue
            if r.get('field') != 'leader_ability' or r.get('confidence') not in ('high', 'medium') \
                    or not isinstance(v, dict) or not v.get('name') or not record_lines(v) \
                    or not url.startswith('https://jojodr.gamerch.com/'):
                continue
            title = norm(urllib.parse.unquote(url[len('https://jojodr.gamerch.com/'):]))
            tm = TITLE.search(title)
            if not tm:
                continue
            rarity, stype = int(tm.group(2)), STYPE[tm.group(1)][1]
            cands = [i for i in lead_jp.get(norm(v['name']), [])
                     if srv_by_id.get(i, {}).get('rarity') == rarity
                     and srv_by_id.get(i, {}).get('statueType') == stype]
            if len(cands) != 1:
                continue
            if cands[0] in RESEARCH_HELD:
                held.add(cands[0])
                continue
            research_rows.append({'statue_id': cands[0], 'leader_ability_values': record_lines(v),
                                  '_url': url, '_accessed': r.get('accessed', ''),
                                  '_rank': RANK[r['confidence']], '_kind': 'gamerch'})
    # Two records for one statue (research-b33 and research-b43 of the same page): the higher
    # confidence first, else the order read; the first one is used.
    research_rows.sort(key=lambda x: -x['_rank'])

    stats = collections.Counter()
    per_cat = collections.Counter()
    affects = {}          # affectId -> (baseAffect, prvs)
    rows, mapping, seen, from_research, from_shoutwiki = [], {}, set(), {}, {}
    for s in wiki['statues'] + research_rows + shoutwiki_rows:
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
        if s.get('_kind') == 'shoutwiki':
            from_shoutwiki[sid] = s
            stats['statue_with_row_from_shoutwiki_max'] += 1
        elif '_url' in s:
            from_research[sid] = s
            stats['statue_with_row_from_research'] += 1
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
    new_rows = [{'affectId': i, 'affectTextId': text_id(i), 'prvs': affects[i][1], 'baseAffect': affects[i][0],
                 'affectLimitNormal': [], 'affectLimitExtra': [], 'textSortIndex': 0}
                for i in sorted(affects) if i not in old_ids]
    affect_doc = {
        'asset': 'affect_table',
        'about': 'The three affect ids the client names in code, plus one row per affect id used by the leader abilities and Mind abilities in dtbattle_ability_data_table. Each id names one BaseAffectType and one AffectCondition; the amount comes from the ability row (per-mille, negative for the *_SUB ids). Empty limit lists mean no cap. affectTextId is the game\'s own affectText_JP label with the same meaning (AFFECT_[COND_]STAT_OP_TEXT; DYING takes the near-death FNT labels, FULL_HP the FULL ones): the detail screens build the effect line from it, with the amounts merged by affect_text_rule_table.',
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

    def dump(obj, path):
        with open(path, 'w', encoding='ascii') as f:
            json.dump(obj, f, indent=1, ensure_ascii=True)
            f.write('\n')
    research_src = None
    if from_research:
        research_src = {
            'source': 'Gamerch \u30b8\u30e7\u30b8\u30e7DR\u653b\u7565Wiki (live statue pages, research pass)',
            'page': 'https://jojodr.gamerch.com/',
            'pages': {'DT_LEAD_' + i: from_research[i]['_url'] for i in sorted(from_research)},
            'accessed': max(x['_accessed'] for x in from_research.values()),
            'note': 'leader-ability numbers only, for statues the Wayback dump lacks (build_leader_abilities.py '
                    '--research): the page leader name equals the ABI_LEAD_NAME label and its title gives the '
                    'rarity and type. A line without (MAX ...) gives Lv10 = Lv1.'
                    + (' DT_LEAD_3001_03 is uncertain (M): its +15% has no MAX on the page, and ShoutWiki oldid '
                       '3832 shows +15% as the MAX Lv. value, so Lv1 may be lower.' if '3001_03' in from_research else '')}
    shoutwiki_src = None
    if from_shoutwiki:
        shoutwiki_src = {
            'source': 'Diamond Records Wiki (ShoutWiki) (MAX leader values, research pass)',
            'page': SHOUTWIKI, 'license': 'CC BY-SA 3.0',
            'pages': {'DT_LEAD_' + i: {'page': from_shoutwiki[i]['_page'],
                                       **({'revid': from_shoutwiki[i]['_revid']} if from_shoutwiki[i]['_revid'] else {}),
                                       'accessed': from_shoutwiki[i]['_accessed']} for i in sorted(from_shoutwiki)},
            'accessed': max(x['_accessed'] for x in from_shoutwiki.values()),
            'note': 'research-b43/assists: the Reversal-era (tactical) MAX Lv. leader value of the statue page '
                    'that the statue\'s stats cite, for statues no Gamerch page gives; the page prints no Lv1, '
                    'so Lv1 = Lv10 = the MAX value (ours). Lines without a tactical affect are not used.'}
    if a.table:
        doc = json.load(open(a.table, encoding='utf-8'))
        others = [r for r in doc['data']['paramList'] if not r['abilityId'].startswith('DT_LEAD_')]
        doc['data']['paramList'] = rows + others
        doc['sources'] = [x for x in doc['sources'] if 'research pass)' not in x.get('source', '')]
        if shoutwiki_src:
            doc['sources'].insert(1, shoutwiki_src)
        if research_src:
            doc['sources'].insert(1, research_src)
        with open(a.table, 'w', encoding='utf-8') as f:
            json.dump(doc, f, ensure_ascii=False, indent=1)
            f.write('\n')
        if new_rows:
            base_affect['data']['paramList'] = old + new_rows
            dump(base_affect, a.affect_table)
    else:
        if shoutwiki_src:
            table_doc['sources'].insert(1, shoutwiki_src)
            map_doc['sources'].insert(1, shoutwiki_src)
        if research_src:
            table_doc['sources'].insert(1, research_src)
            map_doc['sources'].insert(1, research_src)
        os.makedirs(a.out_content, exist_ok=True)
        os.makedirs(a.out_schemas, exist_ok=True)
        dump(affect_doc, os.path.join(a.out_content, 'affect_table.json'))
        dump(table_doc, os.path.join(a.out_content, 'dtbattle_ability_data_table.json'))
        dump(map_doc, os.path.join(a.out_content, 'leader_abilities.json'))
        dump(schema_frag, os.path.join(a.out_schemas, 'dtbattle_ability_data_table.json'))

    by_affect = collections.Counter(q['affectId'] for r in rows for q in r['abilityParamList'])
    report = {'stats': dict(stats), 'effects_by_category': dict(per_cat),
              'params_by_affect': dict(sorted(by_affect.items())),
              'affect_rows_added': len(new_rows)}
    report['research_rows'] = sorted(from_research)
    report['shoutwiki_max_rows'] = sorted(from_shoutwiki)
    report['research_held'] = sorted(held)
    if a.server_statues:
        ids = set(srv_by_id)
        ids.discard(None)
        report['server_statues'] = len(ids)
        report['server_statues_with_leader_ability'] = len(ids & set(mapping))
    print(json.dumps(report, indent=1))
    if a.report:
        dump(report, a.report)


if __name__ == '__main__':
    main()
