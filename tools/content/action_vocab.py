"""Shared action-battle ability vocabulary (docs/research/30-action-battle.md §7).

Loaded by build_action_abilities.py, build_assist_abilities.py, build_statue_traits.py and
build_jojo_links.py by explicit path (the builders run with `python3 -I`):

    spec = importlib.util.spec_from_file_location(
        'action_vocab', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'action_vocab.py'))

What it gives (ids, enum names and numbers only; no wiki text leaves this module):
  parse(line, labels)      one wiki effect line (JP) -> ('ok', param) | (hold category, None)
  parse_en(stat, value, percent, labels)
                           one ShoutWiki assist line ({stat, value, percent}) -> the same
  affect_row(affect_id)    the affect_table row of an AF_* id (prvs and baseAffect from the
                           vocabulary: condition -> prvs, stat -> base)
  append_affects(...)      the one append helper for affect_table and affect_text_rule_table
  ACTION_AFFECT            tactical-only DTAF_* ids -> the action AF_* id with the same meaning

A param is {affectId, prvs, baseAffect, affectTextId, giveLimit -1, lv1 [8], lv10 [8], max} with
max 'max' | 'no_max' | 'max_rejected'. Units (read in the 3.12.0 client, docs/research/30 §7):
  "N%"  -> N x 10 (per mille; damage-taken and "-" lines negative)
  "xX"  -> round((X - 1) x 1000) (a "X倍" taken line below 1 comes out negative)
  flat  -> the *_CON base with p0 = N; SP uses -> BASE_SPSTC_ADD with p0 = the count
  p1    -> the tribe (ATRBT target) or StatueType (STYPE target)
  p2    -> seconds x 10 (DEFEAT, ASSIST_USE), HP per mille (OVER_HP, UNDER_HP), tribe (ATRBT holder)
Every slot is lerped from Lv1 to Lv10 by the ability level (GetAffectParamValue 0x205F328). Lv10 is
the wiki MAX when it is plausible (same side of neutral, at least as far out, at most 3x as far),
else Lv1.
"""
import collections, json, re, unicodedata

NUM = r'([0-9]+(?:\.[0-9]+)?)'

# Jjs tribe ids (ATTR_TEXT_<n>): the wiki words, checked against the player's JP labels by
# check_labels(). 大アルアナ is a wiki typo of 大アルカナ.
TRIBE_JP = {'吸血鬼': 1, 'ジョースター家': 2, '波紋使い': 3, '柱の男': 4, 'スタンド使い': 5,
            '大アルカナ': 6, 'エジプト9栄神': 7, '究極生命体': 8}
TRIBE_TYPO = {'大アルアナ': 6}
TRIBE_WORD = {1: 'VAMPIRE', 2: 'JOESTAR', 3: 'HAMON', 4: 'HASHIRA', 5: 'STAND', 6: 'M_ARCANA',
              7: 'EGYPT', 8: 'ULTIMATE'}
# Jjs.StatueType
STYPE_JP = {'勇気': 1, '知略': 2, '闘志': 3, '団結': 4, '孤高': 5}
STYPE_WORD = {1: 'BRAVE', 2: 'INTELLI', 3: 'TENACITY', 4: 'UNITY', 5: 'MAVERICK'}

# Condition word -> Jjs.AffectCondition (UpdateAffectCondition jump table 0x236a808).
COND_PRVS = {'': 'PRVS_NONE', 'ORAORA': 'PRVS_ORAORA', 'FNT': 'PRVS_DYING', 'ALONE': 'PRVS_ALONE',
             'HALF': 'PRVS_HALF_HP', 'FULL': 'PRVS_FULL_HP', 'DEFEAT': 'PRVS_DEFEAT',
             'AST_USE': 'PRVS_ASSIST_USE', 'OVER_HP': 'PRVS_OVER_HP', 'UNDER_HP': 'PRVS_UNDER_HP'}
for _t, _w in TRIBE_WORD.items():
    COND_PRVS['ONLY_' + _w] = 'PRVS_ATRBT'

# Stat op -> (BaseAffectType, unit). unit: permil (+N%), -permil (-N%), flat, -flat, mag (xX),
# count.
OPS = {
    'ATK_RATEADD': ('BASE_ATK_RATECON', 'permil'), 'DEF_RATEADD': ('BASE_DEF_RATECON', 'permil'),
    'HP_RATEADD': ('BASE_HP_RATECON', 'permil'), 'DEF_RATESUB': ('BASE_DEF_RATECON', '-permil'),
    'ATK_ADD': ('BASE_ATK_CON', 'flat'),
    'DMG_RATEADD': ('BASE_DMG_RATECON', 'permil'), 'DMG_ADD': ('BASE_DMG_CON', 'flat'),
    'VCT_DAM_RATESUB': ('BASE_VCT_DAM_RATECON', '-permil'), 'VCT_DAM_SUB': ('BASE_VCT_DAM_CON', '-flat'),
    'CRTRATE_ADD': ('BASE_CRTRATE_CON', 'mag'), 'CRT_DMG_RATEADD': ('BASE_CRT_DMG_RATECON', 'mag'),
    'VCT_CRT_DMG_RATESUB': ('BASE_VCT_CRT_DMG_RATECON', 'mag'),
    'ORACHAGE_FAST': ('BASE_ORACHAGE_CON', 'permil'), 'CT_FAST': ('BASE_CT_CON', 'permil'),
    'MOVE_FAST': ('BASE_MOVE_CON', 'permil'), 'CURE_RATEADD': ('BASE_CURE_RATECON', 'permil'),
    'SP_SKILL_RATEADD': ('BASE_SP_SKILL_RATECON', 'permil'),
    'NORMAL_SKILL_RATEADD': ('BASE_NORMAL_SKILL_RATECON', 'permil'),
    'SPSTC_ADD': ('BASE_SPSTC_ADD', 'count'),
    'STEP_RANGE': ('BASE_STEP_RANGE', 'permil'), 'STEP_TIME': ('BASE_STEP_TIME', 'permil'),
    'COMBO_GAUGE_RATE': ('BASE_COMBO_GAUGE_RATE', 'permil'),
    'HP_BOOST_ATK': ('BASE_HP_BOOST_ATK', 'permil'), 'DEF_BOOST_ATK': ('BASE_DEF_BOOST_ATK', 'permil'),
    'ATK_BOOST_HP': ('BASE_ATK_BOOST_HP', 'permil'), 'ATK_BOOST_DEF': ('BASE_ATK_BOOST_DEF', 'permil'),
    'HP_BOOST_DEF': ('BASE_HP_BOOST_DEF', 'permil'), 'DEF_BOOST_HP': ('BASE_DEF_BOOST_HP', 'permil'),
}
for _t, _w in TRIBE_WORD.items():
    OPS['ATRBT_%s_DMG_RATEADD' % _w] = ('BASE_ATRBT_DMG_RATECON', 'permil')
    OPS['ATRBT_%s_VCT_DAM_RATESUB' % _w] = ('BASE_ATRBT_VCT_DAM_RATECON', '-permil')
for _t, _w in STYPE_WORD.items():
    OPS['%s_STYPE_DMG_RATEADD' % _w] = ('BASE_STYPE_DMG_RATECON', 'permil')
    OPS['%s_STYPE_VCT_DAM_RATESUB' % _w] = ('BASE_STYPE_VCT_DAM_RATECON', '-permil')

# BaseAffectType values the action battle reads (UpdateActiveAffectParameter 0x2066F84 cases,
# BASE_SPSTC_ADD 27 by CalcAdditionalSpSkillUsableCount, 88 by its own reader). Anything else takes
# the default case and is held ('no_consumer').
CONSUMERS = set(range(1, 21)) | {23, 24, 25, 26, 27} | set(range(45, 58)) | {60, 61} \
    | set(range(74, 80)) | {81} | set(range(88, 92))
# Bases whose value counts only at battle start (max HP is computed once in SetStatueInfo), so
# they may only use a condition known at that point.
HP_BASES = {'BASE_HP_CON', 'BASE_HP_RATECON', 'BASE_ATK_BOOST_HP', 'BASE_DEF_BOOST_HP'}
HP_PRVS = {'PRVS_NONE', 'PRVS_ATRBT'}
# Text rules: slot 1 is one line per value for the target-type bases (p1 = tribe / StatueType);
# slot 2 (seconds, HP threshold, holder tribe) is one line per value for these conditions.
SOLO_P1_BASES = ['BASE_ATRBT_DMG_RATECON', 'BASE_ATRBT_VCT_DAM_RATECON', 'BASE_STYPE_DMG_RATECON',
                 'BASE_STYPE_VCT_DAM_RATECON']
SOLO_P2_PRVS = ['PRVS_DEFEAT', 'PRVS_ASSIST_USE', 'PRVS_ATRBT', 'PRVS_OVER_HP', 'PRVS_UNDER_HP']
# Labels without a JP script (docs/research/16-minds.md §8): in text.jp, not in the l10n export.
NO_JP_LABELS = {'AFFECT_HP_RATEADD_TEXT', 'AFFECT_ATK_RATEADD_TEXT', 'AFFECT_DEF_RATEADD_TEXT'}

# Tactical-only affects (BASE_DT_*) that the action battle does not read, mapped to the action
# affect with the same meaning and numbers (per mille of damage; p1 = StatueType). The server's
# inline_ability() uses the same table (collection.rs ACTION_AFFECT).
ACTION_AFFECT = [('DTAF_DMG_RATEADD', 'AF_DMG_RATEADD'),
                 ('DTAF_VCT_DAM_RATESUB', 'AF_VCT_DAM_RATESUB'),
                 ('DTAF_FULL_HP_DMG_RATEADD', 'AF_FULL_DMG_RATEADD')] + \
    [('DTAF_%s_STYPE_DMG_RATEADD' % w, 'AF_%s_STYPE_DMG_RATEADD' % w) for w in STYPE_WORD.values()] + \
    [('DTAF_BRAVE_STYPE_VCT_DAM_RATESUB', 'AF_BRAVE_STYPE_VCT_DAM_RATESUB'),
     ('DTAF_MAVERICK_STYPE_VCT_DAM_RATESUB', 'AF_MAVERICK_STYPE_VCT_DAM_RATESUB')]


def nfkc(s):
    return unicodedata.normalize('NFKC', s or '')


def norm(s):
    """NFKC, U+2212 -> '-', no whitespace at all, no trailing '。', no leading 特徴."""
    t = re.sub(r'\s+', '', nfkc(s).replace('−', '-'))
    t = re.sub(r'。$', '', t)
    return re.sub(r'^特徴', '', t)


# --- MAX forms (removed from the body before classification; "MAX" in any case) ---------------
MAXRE = re.compile(r'\(MAX:?([+-]?)' + NUM + r'(%|倍|秒間|秒)?\)', re.I)
MAXSEC = re.compile(r'\(MAX:?' + NUM + r'秒間?、[^)0-9]*([+-]?)' + NUM + r'(%|倍)?\)$', re.I)
BARE = re.compile(r'(?<=[0-9%倍])\(([+-]?)' + NUM + r'(%|倍)\)$')

# --- condition prefixes: (regex, word, p2 kind) -------------------------------------------------
CONDS = [
    (r'^ドドドモード(?:時|で)、?', 'ORAORA', None),
    (r'^瀕死時、?', 'FNT', None),
    (r'^(?:チームメンバーが残り一人の?(?:時|とき)|チーム内のメンバーが一人(?:だけ)?にな(?:っ|った)(?:時|とき))、?',
     'ALONE', None),
    (r'^HP半[減滅]時、?', 'HALF', None),
    (r'^HPMAX時、?', 'FULL', None),
    (r'^敵を倒す(?:度|たび)、?' + NUM + r'秒間?(?:\(MAX:?' + NUM + r'秒間?\))?、?', 'DEFEAT', 'sec'),
    (r'^アシストスキル使用後' + NUM + r'秒間、?', 'AST_USE', 'sec'),
    (r'^HP' + NUM + r'%以上の時、?', 'OVER_HP', 'hp'),
    (r'^HP' + NUM + r'%以下の時、?', 'UNDER_HP', 'hp'),
    (r'^チームの同スタチュータイプの数×', 'PER_STYPE', None),
]
HELD_COND = {'PER_STYPE': 'per_stype'}

# --- stat bodies: (regex, op or hold category) ---------------------------------------------------
STATS = [
    (r'^ATK\+?' + NUM + '%', 'ATK_RATEADD'), (r'^DEF\+' + NUM + '%', 'DEF_RATEADD'),
    (r'^HP\+?' + NUM + '%', 'HP_RATEADD'), (r'^-?DEF-' + NUM + '%', 'DEF_RATESUB'),
    (r'^-DEF' + NUM + '%', 'DEF_RATESUB'),
    (r'^DEF' + NUM + '%', 'DEF_RATEADD'),
    (r'^ATK\+' + NUM + '$', 'ATK_ADD'),
    (r'^与えるダメージ:?\+?' + NUM + '%', 'DMG_RATEADD'),
    (r'^与えるダメージ:?\+?' + NUM + '追加', 'DMG_ADD'),
    (r'^受けるダメージ-' + NUM + '%', 'VCT_DAM_RATESUB'),
    (r'^受けるダメージを-' + NUM, 'VCT_DAM_SUB'),
    (r'^クリティカル発生確率', 'CRTRATE_ADD'),
    (r'^クリティカルを与え(?:た|る)(?:時|とき)のダメージ', 'CRT_DMG_RATEADD'),
    (r'^クリティカルを受けた時のダメージ', 'VCT_CRT_DMG_RATESUB'),
    (r'^ドドドチャージ:?効率', 'ORACHAGE_FAST'),
    (r'^クールタイム短縮', 'CT_FAST'),
    (r'^移動速度', 'MOVE_FAST'),
    (r'^回復量', 'CURE_RATEADD'),
    (r'^SPスキルの?性能', 'SP_SKILL_RATEADD'),
    (r'^スキルの性能', 'NORMAL_SKILL_RATEADD'),
    (r'^SPスキルの使用回数を[0-9]+回増加', 'SPSTC_ADD'),
    (r'^ステップによる跳躍距離', 'STEP_RANGE'),
    (r'^ステップ速度', 'STEP_TIME'),
    (r'^連続撃破数(?:が|に)つながりやすくなる', 'COMBO_GAUGE_RATE'),
    (r'^HPの' + NUM + '%分、ATKを上昇', 'HP_BOOST_ATK'),
    (r'^DEFの' + NUM + '%?分、ATKを上昇', 'DEF_BOOST_ATK'),
    (r'^ATKの' + NUM + '%分、HPを上昇', 'ATK_BOOST_HP'),
    (r'^ATKの' + NUM + '%分、DEFを上昇', 'ATK_BOOST_DEF'),
    (r'^HPの' + NUM + '%分、DEFを上昇', 'HP_BOOST_DEF'),
    (r'^DEFの' + NUM + '%分、HPを上昇', 'DEF_BOOST_HP'),
    # held: no client consumer (default case) or not traced
    (r'^(?:バトル時、)?取得ゴールド\+' + NUM + '%', 'gold_rate'),
    (r'^(?:バトル時、)?取得ゴールド\+' + NUM, 'gold_flat'),
    (r'^(?:バトル時、)?(?:スタチュー)?取得経験値\+' + NUM + '%', 'exp_rate'),
    (r'^(?:[0-9]+|一)回だけ、ダメージ(?:を|が)受けなくなる', 'barrier'),
    (r'^吹き飛んだ敵による衝突ダメージ', 'blow_dmg'),
    (r'^爆発(?:で|による)?(?:与える)?ダメージ', 'bomber_dmg'),
]


def _tribe(word):
    return TRIBE_JP.get(word) or TRIBE_TYPO.get(word)


def classify(body):
    """body (normalised, MAX forms removed) -> (cond word, p2 or None, op, p1 or None, rest) or a
    hold category string. rest is the stat body (the condition and any holder removed)."""
    cond, cond_m = '', None
    for rx, word, _kind in CONDS:
        m = re.match(rx, body)
        if m:
            cond, cond_m, body = word, m, body[m.end():]
            break
    body = re.sub(r'^特徴', '', body)
    p2 = None
    m = re.match(r'^【([^】]+)】(?:の|が)(?:スタチューに)?', body)
    if m and _tribe(m.group(1)) and not re.match(r'^【[^】]+】(?:に|から)', body):
        if cond:
            return 'two_conditions'
        t = _tribe(m.group(1))
        cond, p2, body = 'ONLY_' + TRIBE_WORD[t], t, re.sub(r'^特徴', '', body[m.end():])
    if cond in HELD_COND:
        return HELD_COND[cond]
    m = re.match(r'^【([^】]+)】(?:のスタチュー)?(に|から)(?:与える|受ける)ダメージ', body)
    if m and _tribe(m.group(1)):
        t = _tribe(m.group(1))
        op = ('ATRBT_%s_DMG_RATEADD' if m.group(2) == 'に' else 'ATRBT_%s_VCT_DAM_RATESUB') % TRIBE_WORD[t]
        return cond, p2, op, t, cond_m, body
    m = re.match(r'^(' + '|'.join(STYPE_JP) + r')(?:タイプ)?(に|から)(?:与える|受ける)ダメージ', body)
    if m:
        st = STYPE_JP[m.group(1)]
        op = ('%s_STYPE_DMG_RATEADD' if m.group(2) == 'に' else '%s_STYPE_VCT_DAM_RATESUB') % STYPE_WORD[st]
        return cond, p2, op, st, cond_m, body
    for rx, op in STATS:
        if re.match(rx, body):
            if op not in OPS:
                return op
            return cond, p2, op, None, cond_m, body
    return 'unclassified'


def plausible_max(v1, v10, neutral):
    """MAX must lie on the same side of neutral, at least as far out, and at most 3x as far."""
    d1, d10 = v1 - neutral, v10 - neutral
    return d1 != 0 and d1 * d10 > 0 and abs(d1) <= abs(d10) <= 3 * abs(d1)


def convert(unit, v):
    return {'permil': round(v * 10), '-permil': -round(abs(v) * 10), 'flat': round(v),
            '-flat': -round(abs(v)), 'mag': round((v - 1) * 1000), 'count': round(v)}[unit]


def split_id(affect_id):
    """AF_[COND_]OP -> (cond word, op)."""
    assert affect_id.startswith('AF_'), affect_id
    rest = affect_id[3:]
    for word in sorted(COND_PRVS, key=len, reverse=True):
        if word and rest.startswith(word + '_') and rest[len(word) + 1:] in OPS:
            return word, rest[len(word) + 1:]
    assert rest in OPS, affect_id
    return '', rest


def affect_row(affect_id):
    """AF_[COND_]OP -> {affectId, affectTextId, prvs, baseAffect, ...} (the affect_table row)."""
    cond, op = split_id(affect_id)
    rest = affect_id[3:]
    base = OPS[op][0]
    return {'affectId': affect_id, 'affectTextId': 'AFFECT_' + rest + '_TEXT', 'prvs': COND_PRVS[cond],
            'baseAffect': base, 'affectLimitNormal': [], 'affectLimitExtra': [], 'textSortIndex': 0}


def hold_of(affect_id, labels=None):
    """The hold category of an AF_* id (None = it may be served)."""
    return _hold_checks(*split_id(affect_id), labels)


def _hold_checks(cond, op, labels):
    """Hold rules on a classified (cond, op): returns a category or None."""
    prvs, base = COND_PRVS[cond], OPS[op][0]
    if cond == 'ORAORA' and op == 'ORACHAGE_FAST':
        return 'no_label'
    if base in HP_BASES and prvs not in HP_PRVS:
        return 'hp_conditional'
    text = 'AFFECT_' + (cond + '_' if cond else '') + op + '_TEXT'
    if labels is not None and text not in labels and text not in NO_JP_LABELS:
        return 'no_label'
    if base not in BASE_VALUE or BASE_VALUE[base] not in CONSUMERS:
        return 'no_consumer'
    return None


def _param(cond, op, p1, p2, v1, v10, sec, maxnote):
    unit = OPS[op][1]
    lv1, lv10 = [0] * 8, [0] * 8
    lv1[0], lv10[0] = convert(unit, v1), convert(unit, v10)
    if p1 is not None:
        lv1[1] = lv10[1] = p1
    if p2 is not None:
        lv1[2] = lv10[2] = p2
    if sec is not None:
        lv1[2], lv10[2] = round(sec[0] * 10), round(sec[1] * 10)
    aid = 'AF_' + (cond + '_' if cond else '') + op
    row = affect_row(aid)
    return {'affectId': aid, 'prvs': row['prvs'], 'baseAffect': row['baseAffect'],
            'affectTextId': row['affectTextId'], 'giveLimit': -1, 'lv1': lv1, 'lv10': lv10, 'max': maxnote}


def parse(line, labels=None):
    """One JP effect line -> ('ok', param) | (category, None). labels: the affectText_JP label
    names of the player's text (a missing label holds the line), or None to skip that check."""
    t = norm(line)
    mx, sec2 = [], None
    ms = MAXSEC.search(t)
    if ms:
        sec2, mx, t = float(ms.group(1)), [('sec', ms)], t[:ms.start()]
    else:
        mb = BARE.search(t)
        if mb:
            mx, t = [('bare', mb)], t[:mb.start()]
    if not mx:
        mx = [('max', x) for x in MAXRE.finditer(t) if '秒' not in (x.group(3) or '')]
    body = MAXRE.sub('', t)
    got = classify(body)
    if isinstance(got, str):
        return got, None
    cond, p2, op, p1, cond_m, rest = got
    sec = None
    if cond in ('DEFEAT', 'AST_USE'):
        # read on the line before the MAX forms are removed: '敵を倒す度、12秒間(MAX18秒間)、…'
        rx = [c[0] for c in CONDS if c[1] == cond][0]
        sm = re.match(rx, t) or cond_m
        s1 = float(sm.group(1))
        s10 = float(sm.group(2)) if cond == 'DEFEAT' and sm.group(2) else s1
        sec = (s1, sec2 if sec2 is not None else s10)
    if cond_m is not None and cond in ('OVER_HP', 'UNDER_HP'):
        p2 = round(float(cond_m.group(1)) * 10)
    nums = re.findall(NUM, re.sub(r'^【[^】]+】', '', rest))
    if len(nums) != 1:
        return 'extra_text', None
    unit = OPS[op][1]
    if unit == 'flat' and float(nums[0]) != int(float(nums[0])):
        return 'nonint_flat', None
    held = _hold_checks(cond, op, labels)
    if held:
        return held, None
    v1 = float(nums[0])
    v10, maxnote = v1, 'no_max'
    if mx:
        kind, g = mx[-1]
        c = float(g.group(3) if kind == 'sec' else g.group(2))
        neutral = 1.0 if unit == 'mag' else 0.0
        if plausible_max(v1, c, neutral):
            v10, maxnote = c, 'max'
        else:
            maxnote = 'max_rejected'
    return 'ok', _param(cond, op, p1, p2, v1, v10, sec, maxnote)


# --- ShoutWiki assist lines ({stat, value, percent}, lowercased stat) ----------------------------
_T_EN = r'(?:\[vampires?\]|\[\[:category:vampire\|[^\]]*\]\]|vampires?)'
# (also 'pillarmen' without the space, as one ShoutWiki page writes it)
_P_EN = r'(?:\[pillar m[ae]n\]|\[\[:category:pillar man\|[^\]]*\]\]|pillar ?m[ae]n)'
EN_BASE = [
    (r'hp', '', 'HP_RATEADD', None, None), (r'atk', '', 'ATK_RATEADD', None, None),
    (r'def', '', 'DEF_RATEADD', None, None),
    (r'damage dealt|damage given', '', 'DMG_RATEADD', None, None),
    (r'dododo charge efficiency|dododdo charge efficiency|dododo charge efficency', '', 'ORACHAGE_FAST', None, None),
    (r'movement speed', '', 'MOVE_FAST', None, None),
    (r'cooldown (?:time )?reduction(?: speed)?', '', 'CT_FAST', None, None),
    (r'recovery amount', '', 'CURE_RATEADD', None, None),
    (r'damage (?:dealt )?to ' + _T_EN, '', 'ATRBT_VAMPIRE_DMG_RATEADD', 1, None),
    (r'damage (?:dealt )?to ' + _P_EN, '', 'ATRBT_HASHIRA_DMG_RATEADD', 4, None),
    (r'damage (?:dealt )?to stand users', '', 'ATRBT_STAND_DMG_RATEADD', 5, None),
    (r"\[vampire\] atk|vampires' atk", 'ONLY_VAMPIRE', 'ATK_RATEADD', None, 1),
    (r"vampires' def", 'ONLY_VAMPIRE', 'DEF_RATEADD', None, 1),
    (r"vampires' recovery|\[vampire\] recover efficiency", 'ONLY_VAMPIRE', 'CURE_RATEADD', None, 1),
    (r'\[hamon users\] def', 'ONLY_HAMON', 'DEF_RATEADD', None, 3),
    (r"joestar family's dealt damage", 'ONLY_JOESTAR', 'DMG_RATEADD', None, 2),
    (r"joestar family's received damage", 'ONLY_JOESTAR', 'VCT_DAM_RATESUB', None, 2),
    (r'in dododo mode, atk', 'ORAORA', 'ATK_RATEADD', None, None),
    (r'in dododo mode, movement speed', 'ORAORA', 'MOVE_FAST', None, None),
    (r'in dododo mode, cooldown reduction speed', 'ORAORA', 'CT_FAST', None, None),
    (r'when hp is max, atk', 'FULL', 'ATK_RATEADD', None, None),
]
EN_HELD = [(r'(?:continuous|increased) ko timer', 'ko_timer'),
           (r'collision damage by blown enemies', 'blow_dmg'),
           (r'damage from explosions', 'bomber_dmg')]
EN_AST = [re.compile(r'^for ' + NUM + r' seconds after using an assist skill, (.+)$'),
          re.compile(r'^after using assist ability ?, for ' + NUM + r' seconds, (.+)$')]


def parse_en(stat, value, percent, labels=None):
    """One ShoutWiki assist line -> ('ok', param) | (category, None). ShoutWiki gives one value
    (no MAX): Lv10 = Lv1. Its sign is kept (the received-damage value is already negative)."""
    s = re.sub(r'\s+', ' ', nfkc(stat).strip().lower())
    sec = None
    for rx in EN_AST:
        m = rx.match(s)
        if m:
            sec, s = float(m.group(1)), m.group(2).strip()
            break
    for rx, cat in EN_HELD:
        if re.fullmatch(rx, s):
            return cat, None
    for rx, cond, op, p1, p2 in EN_BASE:
        if not re.fullmatch(rx, s):
            continue
        if sec is not None:
            if cond:
                return 'two_conditions', None
            cond = 'AST_USE'
        if value is None or (OPS[op][1] in ('permil', '-permil') and not percent):
            return 'unclassified', None
        held = _hold_checks(cond, op, labels)
        if held:
            return held, None
        v = float(value)
        lv1 = [0] * 8
        lv1[0] = round(v * 10)  # ShoutWiki's own sign
        if p1 is not None:
            lv1[1] = p1
        if p2 is not None:
            lv1[2] = p2
        if sec is not None:
            lv1[2] = round(sec * 10)
        aid = 'AF_' + (cond + '_' if cond else '') + op
        row = affect_row(aid)
        return 'ok', {'affectId': aid, 'prvs': row['prvs'], 'baseAffect': row['baseAffect'],
                      'affectTextId': row['affectTextId'], 'giveLimit': -1, 'lv1': lv1,
                      'lv10': list(lv1), 'max': 'no_max'}
    return 'unclassified', None


EN_LINE = re.compile(r'^(.*?)\s*([+-]?)\s*([0-9]+(?:\.[0-9]+)?)\s*(%?)$')


def parse_en_line(line, labels=None):
    """A research record's English effect line ('Damage Dealt to [Vampire] +6.0%', from a ShoutWiki
    page) -> parse_en of its stat, value and percent sign."""
    m = EN_LINE.match(nfkc(line).strip())
    if not m:
        return 'unclassified', None
    return parse_en(m.group(1), float(m.group(2) + m.group(3)), m.group(4) == '%', labels)


def merge(params):
    """Two lines with the same (affectId, p1, p2) in one ability: p0 is summed (Lv1 and Lv10)."""
    out = collections.OrderedDict()
    for p in params:
        key = (p['affectId'], p['lv1'][1], p['lv1'][2])
        if key in out:
            out[key]['lv1'][0] += p['lv1'][0]
            out[key]['lv10'][0] += p['lv10'][0]
        else:
            out[key] = {**p, 'lv1': list(p['lv1']), 'lv10': list(p['lv10'])}
    return list(out.values())


def content_param(p):
    """The param as the content files keep it."""
    return {'affectId': p['affectId'], 'giveLimit': -1, 'lv1': p['lv1'], 'lv10': p['lv10']}


# --- inputs --------------------------------------------------------------------------------------
BASE_VALUE = {}


def load_schema(path):
    """BaseAffectType and AffectCondition values from schema.json; every vocabulary name must be
    one of them."""
    types = json.load(open(path, encoding='utf-8'))['types']
    BASE_VALUE.clear()
    BASE_VALUE.update(types['Jjs.BaseAffectType']['values'])
    prvs = types['Jjs.AffectCondition']['values']
    for b, _u in OPS.values():
        assert b in BASE_VALUE, b
    for p in COND_PRVS.values():
        assert p in prvs, p
    return BASE_VALUE


def load_labels(path):
    """l10n export (key, hash, english, japanese) -> {key: japanese}."""
    out = {}
    for line in open(path, encoding='utf-8'):
        c = line.rstrip('\n').split('\t')
        if len(c) >= 4:
            out[c[0]] = c[3]
    return out


def affect_labels(labels):
    """The affectText_JP label names."""
    p = 'text:affectText_JP:'
    return {k[len(p):] for k in labels if k.startswith(p)}


def check_labels(labels):
    """The tribe words equal the player's ATTR_TEXT_<n> JP labels after NFKC."""
    for word, n in TRIBE_JP.items():
        jp = labels.get('text:AttributeText_JP:ATTR_TEXT_%d' % n)
        assert jp is not None and nfkc(jp) == word, (n, word)


# --- the one append helper -----------------------------------------------------------------------
AFFECT_SOURCE = {'source': 'jdr-default',
                 'note': 'AF_* action-battle affects (ours ids; units docs/research/30-action-battle.md §7): '
                         'AF_[COND_]STAT_OP, prvs from the condition and baseAffect from the stat of the '
                         'shared vocabulary tools/content/action_vocab.py'}
RULE_SOURCE = {'source': 'jdr-default',
               'note': 'rows for the AF_* action affects (tools/content/action_vocab.py): slot 0 Combine, '
                       'slot 1 Solo for the ATRBT/STYPE target bases, slot 2 Solo for the DEFEAT, ASSIST_USE, '
                       'ATRBT, OVER_HP and UNDER_HP conditions (seconds, HP threshold, holder tribe)'}


def _dump(doc, path, ascii_only):
    with open(path, 'w', encoding='utf-8') as f:
        json.dump(doc, f, indent=1, ensure_ascii=ascii_only)
        f.write('\n')


def append_affects(affect_ids, affect_table, rule_table):
    """Append one affect_table row per AF_* id not there yet (in first-use order, after every
    existing row), and the text rules every AF_* row of the table needs (affect_text_rule_table).
    Existing rows and their order are kept, held ids are never passed. Idempotent. Returns the
    number of rows added to each table."""
    doc = json.load(open(affect_table, encoding='utf-8'))
    rows = doc['data']['paramList']
    have = {r['affectId'] for r in rows}
    added = 0
    for aid in affect_ids:
        if aid in have:
            continue
        rows.append(affect_row(aid))
        have.add(aid)
        added += 1
    if not any(s.get('note') == AFFECT_SOURCE['note'] for s in doc['sources']):
        doc['sources'].append(AFFECT_SOURCE)
    if 'AF_*' not in doc['about']:
        doc['about'] += (' Rows AF_* are the action-battle affects of the leader and assist abilities and '
                         'the JoJo Links (server/content/action_abilities.json, assist_abilities.json "action", '
                         'jojo_links.json; docs/research/30-action-battle.md §7).')
    _dump(doc, affect_table, False)

    rd = json.load(open(rule_table, encoding='utf-8'))
    by_base = rd['data']['baseAffectRuleList']
    by_prvs = rd['data']['pvrsRuleList']
    bases = {r['baseAffectType'] for r in by_base}
    prvss = {r['pvrs'] for r in by_prvs}
    radded = 0
    for r in rows:
        if not r['affectId'].startswith('AF_'):
            continue
        b, p = r['baseAffect'], r['prvs']
        if b not in bases:
            slots = ['Invalid'] * 8
            slots[0] = 'Combine'
            if b in SOLO_P1_BASES:
                slots[1] = 'Solo'
            by_base.append({'pvrs': 'PRVS_NONE', 'baseAffectType': b, 'paramRuleTypeList': slots})
            bases.add(b)
            radded += 1
        if p in SOLO_P2_PRVS and p not in prvss:
            slots = ['Invalid'] * 8
            slots[2] = 'Solo'
            by_prvs.append({'pvrs': p, 'baseAffectType': 'BASE_NONE', 'paramRuleTypeList': slots})
            prvss.add(p)
            radded += 1
    if not any(s.get('note') == RULE_SOURCE['note'] for s in rd['sources']):
        rd['sources'].append(RULE_SOURCE)
    if 'AF_*' not in rd['about']:
        rd['about'] = rd['about'].replace(
            ' No condition rows: no slot of our affects depends on the condition.', '')
        rd['about'] += (' The AF_* action affects add their bases (slot 1 Solo for the ATRBT/STYPE '
                        'targets) and condition rows for the conditions whose slot 2 holds seconds, an HP '
                        'threshold or the holder tribe (Solo).')
    _dump(rd, rule_table, True)
    return added, radded


def dumps(obj):
    """JSON with indent 1, lists of integers on one line (the lv1/lv10 slots), UTF-8, newline."""
    s = json.dumps(obj, indent=1, ensure_ascii=False)
    s = re.sub(r'\[\n\s*(-?[0-9]+(?:,\n\s*-?[0-9]+)*)\n\s*\]',
               lambda m: '[' + ', '.join(x.strip() for x in m.group(1).split(',')) + ']', s)
    return s + '\n'


# --- leader records (shared by build_action_abilities.py and build_statue_traits.py) -------------
GAMERCH = 'https://jojodr.gamerch.com/'
# Research matches held for a later check (as build_leader_abilities.py RESEARCH_HELD).
RESEARCH_HELD = {'3041_03', '4000_23'}
TITLE = re.compile('【(' + '|'.join(STYPE_JP) + ')】.*★([0-9])')


def leader_names(labels):
    """{NFKC leader name: [statueId]} from the ABI_LEAD_NAME_<statueId> JP labels."""
    out = collections.defaultdict(list)
    for k, jp in labels.items():
        if ':ABI_LEAD_NAME_' in k and jp:
            out[norm(jp)].append(k.rsplit('ABI_LEAD_NAME_', 1)[1])
    return out


def match_leader_record(r, lead_jp, srv_by_id):
    """A research leader_ability record (live Gamerch statue page) -> its statueId, 'held', or None.
    The record's statueId is not trusted: the page leader name must equal one ABI_LEAD_NAME label
    after NFKC, and the URL title's ★rarity and 【type】 must pick exactly one roster statue."""
    v, url = r.get('value'), r.get('url') or ''
    if r.get('field') != 'leader_ability' or r.get('confidence') not in ('high', 'medium') \
            or not isinstance(v, dict) or not v.get('name') or not v.get('effects') \
            or not url.startswith(GAMERCH):
        return None
    import urllib.parse
    tm = TITLE.search(norm(urllib.parse.unquote(url[len(GAMERCH):])))
    if not tm:
        return None
    rarity, st = int(tm.group(2)), STYPE_JP[tm.group(1)]
    c = [i for i in lead_jp.get(norm(v['name']), [])
         if srv_by_id.get(i, {}).get('rarity') == rarity and srv_by_id.get(i, {}).get('statueType') == st]
    if len(c) != 1:
        return None
    return 'held' if c[0] in RESEARCH_HELD else c[0]


def server_statues(path):
    srv = json.load(open(path, encoding='utf-8'))
    return {x['statueId']: x for x in srv['statues']}
