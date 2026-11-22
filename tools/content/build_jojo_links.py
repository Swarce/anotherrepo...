#!/usr/bin/env python3
"""Action-battle JoJo Links (TeamInfo.jojoLinkList, docs/research/26-links-and-skills.md §1 and
docs/research/30-action-battle.md §7) -> server/content/jojo_links.json, plus the AF_* rows they
need in affect_table and affect_text_rule_table. Ids, family ids, enum names and numbers only: no
wiki text goes into the output ("held" gives our own reason, never the wiki line).

usage:
  python3 -I tools/content/build_jojo_links.py \
      --research local/wiki-raw/research-b33/action/results.jsonl \
      --research-b43 local/wiki-raw/research-b43/links_traits_minds/results.jsonl \
      --saved-table local/wiki-raw/research-2026-11-15-v2/catalog/jojo_links.json \
      --saved-sources local/wiki-raw/research-2026-11-15-v2/catalog/sources.json \
      --labels local/l10n/export-b33.tsv --server-statues server/content/statues.json \
      --affect-table server/content/masterdata/affect_table.json \
      --rule-table server/content/masterdata/affect_text_rule_table.json \
      [--schema server/content/masterdata/schema.json] [--out server/content/jojo_links.json] \
      [--report <file.json>]

Run last, after build_statue_traits.py (docs/CONTENT.md): the trait links count the statues'
tribeTypeList (server/content/statue_traits.json), which the server reads itself.

What the client does with a link (ApplyJojoLinkAbilityAffect 0xF17A30): every TeamInfo.jojoLinkList
entry whose abilityInfo is valid applies to all three members (affect source 4, key charaLinkId).
type, dataList, name and matchType are never read and the client checks no members, so the server
decides which links are active (collection.rs jojo_link_list) from this file.

Sources:
  --research     the research pass's links.jojo 'link' records (the Gamerch ジョジョリンク page,
                 its comments and the Bahamut forum thread; url, accessed and confidence per record).
                 A record's ABI_JOLINK_NAME_<nnn> JP label must equal the record name after NFKC.
  --research-b43 (Build 43) the links.jojo supplements of research-b43/links_traits_minds (high/medium):
                 'link_effect' records replace the effect of the link's record (in-game team screenshots
                 attached to Gamerch comments, cited as 'effectSource' with page, capture and access
                 date); when the screenshot's active team holds every member the record names, a
                 low-confidence record becomes medium (the screenshot shows the link active with those
                 members). 'link_members' and 'link_source' records are corroboration only, cited in
                 'also'; their verdicts (053 member rule open, 054 probably two female statues) are
                 applied by hand below.
  --saved-table  the saved Gamerch link table (Wayback 2017-07-06, sources.json id 'links'), only for
                 a link with no research record whose label equals the row name after NFKC: today
                 J001 = link 34 and J032 = link 12 (confidence "provisional").

Members: a member name maps to the psid families (statueId[:4]) of the statues in --server-statues
whose STATUE_NAME_<sid> JP label (NFKC, the part before '～', no spaces) equals it; a '【N部】'
suffix also requires statues.json part N. Members marked '?' are dropped. FAMILY_ALIAS adds the
variant families whose label is a short name (2011 Caesar, 2111 Stroheim, 3041 Polnareff); 4192
(Kira as Kosaku) is not added (it is unclear which character it counts as). 特徴【T】 members make a
trait link (T through the player's ATTR_TEXT_<n> JP labels).

Rules: 'both', 'all 3', 'all 3 only' -> min = number of groups; 'any N' -> min N; 'N statues with
the trait' -> trait {tribe, min N}. Any other rule string must be in RULE_OVERRIDES (hand table,
each with a reason), else the link is held. A rule is active when at least `min` distinct groups
each have a seat statue whose family is in the group; rules are OR'd.

Effects: split on '/' or '／'; a part with no condition prefix takes the previous part's prefix
(the '瀕死時、ATK+5.0% / DEF+5.0%' links: both lines are near-death), forward only. Each part goes
through tools/content/action_vocab.py parse() (units: per mille, xX as (X-1) x 1000, SP uses as the
count; Lv10 = Lv1). A 'CONFLICT:' effect is replaced by RESOLVED (hand table, with a reason) or held.
A link is held as a whole when any part is held (gold/EXP have no client consumer), when it has no
members or no effect, when its members are unknown, when its record is low confidence (unless
RESOLVED), or when HAND_HELD names it. Ids JOLINK_<nnn> and the AF_* affect ids are ours;
abilityName is the game's ABI_JOLINK_NAME_<nnn> label.

The affect_table / affect_text_rule_table edit is action_vocab.append_affects (idempotent, shared
with the other action builders). A rerun gives byte-identical files.
"""
import argparse, collections, importlib.util, json, os, re

_spec = importlib.util.spec_from_file_location(
    'action_vocab', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'action_vocab.py'))
V = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V)

# Variant families whose STATUE_NAME label is a short name: they count as this member name.
FAMILY_ALIAS = {
    '2011': 'シーザー・アントニオ・ツェペリ',  # Caesar ~A Youth of Fierce Resolve~
    '2111': 'ルドル・フォン・シュトロハイム',  # Stroheim ~Back from Hell~
    '3041': 'ジャン・ピエール・ポルナレフ',  # Polnareff ~Possessed by Anubis~
}

# Rule strings the builder cannot parse: [(member names, min)], each with the reason.
RULE_OVERRIDES = {
    19: ([(['ルドル・フォン・シュトロハイム', '吉良吉影'], 2), (['ルドル・フォン・シュトロハイム', '噴上裕也'], 2)],
         'Stroheim + Kira or Stroheim + Yuya (record rule and the saved row J006)'),
    30: ([(['花京院典明', 'ジャン・ピエール・ポルナレフ'], 2), (['花京院典明', 'ジョセフ・ジョースター【3部】'], 2)],
         'stricter form where sources differ: Gamerch says any 2, Bahamut says Kakyoin + one of the others'),
    32: ([(['ペット・ショップ', 'ンドゥール', 'マライア'], 3)],
         'all 3 members (the trait remark in the record is a guess)'),
    42: ([(['ジョセフ・ジョースター【2部】', '広瀬康一', '虹村億泰'], 2), (['サンタナ', 'ジョセフ・ジョースター【2部】'], 2)],
         'any 2 of Joseph (Part 2) / Koichi / Okuyasu, or Santana + Joseph (Part 2), as the record says'),
    53: ([(['DIO', 'リサリサ', 'マライア'], 3)],
         'all 3 members: the only team any source shows with the link active (research-b43 screenshot; the '
         'researchers leave the member rule open, so the stricter form)'),
    57: ([(['カーズ', 'ワムウ'], 2)],
         'Kars + Wamuu (the third member is marked uncertain in the record)'),
}
# 'CONFLICT:' effects replaced by one side, with the reason.
RESOLVED = {
    46: ('クールタイム短縮:速度+2.0%',
         'the Gamerch main table and Bahamut agree on cool time +2.0%; the comment variant is ignored'),
}
# Links held by hand (reason only).
HAND_HELD = {
    35: 'two Gamerch rows under one name (different members and effects)',
    60: 'crit-taken 0.07 (probably 0.70) and the Straizo membership is uncertain',
    54: 'effect known (research-b43); the members are probably any two female statues (medium, from two '
        'screenshots), and our statues carry no gender trait (ATTR_TEXT_16-18 are not sent)',
}
GOLD_EXP = {'gold_rate', 'gold_flat', 'exp_rate'}


def family_names(labels, srv):
    """{(NFKC base name, part): {family}} from the STATUE_NAME_<sid> JP labels of the roster."""
    by = collections.defaultdict(set)
    for sid, s in srv.items():
        jp = labels.get('text:StatueName_JP:STATUE_NAME_' + sid)
        assert jp, sid
        base = V.norm(re.split('～|~', V.nfkc(jp))[0])
        by[(base, s.get('part'))].add(sid[:4])
    for fam, name in FAMILY_ALIAS.items():
        parts = {s.get('part') for sid, s in srv.items() if sid[:4] == fam}
        assert parts, fam
        for p in parts:
            by[(V.norm(name), p)].add(fam)
    return by


def resolve(member, by):
    m = re.match(r'(.*)【([0-9])部】$', V.nfkc(member))
    base = V.norm(m.group(1) if m else member)
    part = int(m.group(2)) if m else None
    return sorted({f for (b, p), fs in by.items() if b == base and (part is None or p == part) for f in fs})


def parse_effect(effect, alabels):
    """'A / B' -> (params, None) or (None, hold category)."""
    params, prefix = [], ''
    for part in re.split(r'[/／]', effect):
        t = V.norm(part)
        if not t:
            return None, 'unclassified'
        m = None
        for rx, _w, _k in V.CONDS:
            m = re.match(rx, t)
            if m:
                break
        if m:
            prefix = m.group(0)
        elif prefix:
            t = prefix + t
        cat, p = V.parse(t, alabels)
        if p is None:
            return None, cat
        assert p['lv1'] == p['lv10'], (effect, p)
        params.append(p)
    return V.merge(params), None


def saved_row(row):
    """A saved-table row -> (members, rule, effect) or None. Format: '【effect】 「A」+「B」…' (all of
    them) or '【effect】 特徴【T】のスタチューをN人' (trait)."""
    t = V.nfkc(row.get('effect_and_requirement_ja_source') or '')
    m = re.match(r'^\s*【([^】]+)】\s*(.*)$', t)
    if not m:
        return None
    effect, rest = m.group(1), m.group(2).strip()
    tm = re.fullmatch(r'特徴【([^】]+)】のスタチューを([0-9]+)人', re.sub(r'\s+', '', rest))
    if tm:
        return ['特徴【%s】' % tm.group(1)], '%s statues with the trait' % tm.group(2), effect
    names = [n.strip() for n in re.findall(r'「([^」]+)」', rest)]
    if names and re.fullmatch(r'[+\s]*', re.sub(r'「[^」]+」', '', rest)):
        return names, 'all %d' % len(names), effect
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--research', required=True)
    ap.add_argument('--research-b43', action='append', default=[])
    ap.add_argument('--saved-table', required=True)
    ap.add_argument('--saved-sources', required=True)
    ap.add_argument('--labels', required=True)
    ap.add_argument('--server-statues', required=True)
    ap.add_argument('--affect-table', required=True)
    ap.add_argument('--rule-table', required=True)
    ap.add_argument('--schema', default='server/content/masterdata/schema.json')
    ap.add_argument('--out', default='server/content/jojo_links.json')
    ap.add_argument('--report')
    a = ap.parse_args()

    V.load_schema(a.schema)
    labels = V.load_labels(a.labels)
    V.check_labels(labels)
    alabels = V.affect_labels(labels)
    srv = V.server_statues(a.server_statues)
    by = family_names(labels, srv)
    names = {}
    for k, jp in labels.items():
        m = re.search(r':ABI_JOLINK_NAME_([0-9]{3})$', k)
        if m:
            names[int(m.group(1))] = jp
    assert names, 'no ABI_JOLINK_NAME labels'

    records = {}
    for line in open(a.research, encoding='utf-8'):
        r = json.loads(line)
        if r.get('topic') != 'links.jojo' or r.get('field') != 'link':
            continue
        v = r['value']
        n = int(v['label'].rsplit('_', 1)[1])
        assert V.norm(v['name']) == V.norm(names[n]), n
        assert n not in records, n
        url = r.get('url') or ''
        src = 'Gamerch ジョジョDR攻略Wiki (ジョジョリンク page)' if 'gamerch.com' in url else \
            'Bahamut forum (巴哈姆特)' if 'gamer.com.tw' in url else 'jdr client labels'
        records[n] = {'members': v.get('members'), 'rule': v.get('member_rule'), 'effect': v.get('effect'),
                      'confidence': r.get('confidence'),
                      'source': {'source': src, 'page': url, 'accessed': r.get('accessed', ''),
                                 'confidence': r.get('confidence')}}
    for path in a.research_b43:
        for line in open(path, encoding='utf-8'):
            r = json.loads(line)
            if r.get('topic') != 'links.jojo' or r.get('confidence') not in ('high', 'medium'):
                continue
            v = r['value']
            n = int(v['label'].rsplit('_', 1)[1])
            assert n in records and V.norm(v.get('name', names[n])) == V.norm(names[n]), n
            rec = records[n]
            cite = {'source': 'Gamerch ジョジョDR攻略Wiki (ジョジョリンク page, comment screenshot)',
                    'page': r.get('url') or '', 'capture': r.get('capture'), 'accessed': r.get('accessed', ''),
                    'confidence': r['confidence']}
            if r['field'] == 'link_effect':
                eff = v['effect']
                rec['effect'] = ' / '.join(eff) if isinstance(eff, list) else eff
                rec['source']['effectSource'] = cite
                team = {V.norm(x) for x in v.get('observed_active_team') or []}
                members = [m for m in rec['members'] or [] if '?' not in m]
                if rec['confidence'] == 'low' and members and all(V.norm(m) in team for m in members):
                    rec['confidence'] = rec['source']['confidence'] = 'medium'
                    rec['source']['note'] = ('research-b43: a team screenshot shows the link active with every '
                                             'member the record names (was low)')
            elif r['field'] in ('link_members', 'link_source'):
                rec['source'].setdefault('also', []).append(dict(cite, field=r['field']))
            else:
                raise SystemExit('unknown links.jojo field %r' % r['field'])
    saved_src = {s['id']: s for s in json.load(open(a.saved_sources, encoding='utf-8'))}
    for row in json.load(open(a.saved_table, encoding='utf-8')):
        hits = [n for n, jp in names.items() if V.norm(jp) == V.norm(row.get('name_ja_source'))]
        if len(hits) != 1 or hits[0] in records:
            continue
        got = saved_row(row)
        s = saved_src[row['source_id']]
        records[hits[0]] = {
            'members': got[0] if got else None, 'rule': got[1] if got else None,
            'effect': got[2] if got else None, 'confidence': row.get('confidence'),
            'source': {'source': 'Gamerch ジョジョDR攻略Wiki (Wayback Machine snapshot, saved link table)',
                       'page': s['url'], 'accessed': s['accessed'], 'confidence': row.get('confidence'),
                       'note': 'local/wiki-raw/research-2026-11-15-v2/catalog/jojo_links.json row %s'
                               % row['catalog_id']}}

    links, held = [], []
    stats = collections.Counter()
    for n in sorted(names):
        r = records.get(n)

        def hold(reason):
            held.append({'charaLinkId': n, 'reason': reason})
            stats['held'] += 1

        if r is None:
            hold('no record')
            continue
        if not r['members'] or r['effect'] is None:
            hold('name only (no members or effect in any source)')
            continue
        if n in HAND_HELD:
            hold(HAND_HELD[n])
            continue
        effect, note = r['effect'], None
        if effect.startswith('CONFLICT'):
            if n not in RESOLVED:
                hold('conflict between sources')
                continue
            effect, note = RESOLVED[n]
        if 'unknown' in effect:
            hold('effect unknown')
            continue
        members = [m for m in r['members'] if '?' not in m]
        trait, groups, gnames = None, [], []
        for m in members:
            tm = re.fullmatch(r'特徴【([^】]+)】', V.nfkc(m))
            if tm:
                trait = V.TRIBE_JP.get(tm.group(1)) or V.TRIBE_TYPO.get(tm.group(1))
                assert trait, m
                continue
            fams = resolve(m, by)
            if not fams:
                groups = None
                break
            groups.append(fams)
            gnames.append(V.nfkc(m))
        if groups is None or (not groups and trait is None):
            hold('members unknown')
            continue
        if r['confidence'] == 'low' and n not in RESOLVED:
            hold('low confidence')
            continue
        rules, trait_rule = [], None
        rule = r['rule'] or ''
        if n in RULE_OVERRIDES:
            spec, note = RULE_OVERRIDES[n]
            for ms, mn in spec:
                idx = [gnames.index(V.nfkc(x)) for x in ms]
                rules.append({'groups': idx, 'min': mn})
        elif trait is not None:
            tm = re.fullmatch(r'([0-9]+) statues with the trait', rule)
            if not tm or groups:
                hold('rule not parsed')
                continue
            trait_rule = {'tribe': trait, 'min': int(tm.group(1))}
        elif rule in ('both', 'all 3', 'all 3 only') or re.fullmatch(r'all [0-9]+', rule):
            if rule == 'both' and len(groups) != 2:
                hold('rule not parsed')
                continue
            rules.append({'groups': list(range(len(groups))), 'min': len(groups)})
        elif re.fullmatch(r'any [0-9]+', rule):
            rules.append({'groups': list(range(len(groups))), 'min': int(rule.split()[1])})
        else:
            hold('rule not parsed')
            continue
        params, cat = parse_effect(effect, alabels)
        if params is None:
            hold("gold/EXP '0.1%' unit ambiguous, and no client consumer" if cat in GOLD_EXP
                 else 'effect line held: ' + cat)
            continue
        if trait_rule is not None:
            groups = []
        src = dict(r['source'])
        if note:
            src['note'] = (src['note'] + '; ' if 'note' in src else '') + note
        stats['served'] += 1
        stats['served_trait' if trait_rule else 'served_character'] += 1
        links.append({'charaLinkId': n, 'abilityName': 'ABI_JOLINK_NAME_%03d' % n,
                      'groups': groups, 'rules': rules, 'trait': trait_rule,
                      'params': [V.content_param(p) for p in params], 'source': src})

    used = []
    for l in links:
        for p in l['params']:
            if p['affectId'] not in used:
                used.append(p['affectId'])
    report = {'stats': dict(sorted(stats.items())),
              'held_reasons': dict(sorted(collections.Counter(h['reason'] for h in held).items())),
              'affect_ids': len(used)}
    doc = {
        'about': 'Action-battle JoJo Links (TeamInfo.jojoLinkList, docs/research/26-links-and-skills.md §1, '
                 'docs/research/30-action-battle.md §7). The server checks each team: a link is active when one of '
                 'its rules holds (at least "min" distinct groups each have a seat statue whose family, statueId[:4], '
                 'is in the group; rules are OR\'d) or, for a trait link, when at least "min" seat statues carry the '
                 'tribe in their tribeTypeList. Assists do not count. An active link is sent as {charaLinkId, '
                 'abilityId JOLINK_<nnn>, abilityInfo {abilityName, abilityParamList, isInit true}} and applies to '
                 'all three members. Units as action_abilities.json (Lv10 = Lv1). "held" lists the links not served '
                 'with our reason. Built by tools/content/build_jojo_links.py.',
        'sources': [
            {'source': 'Gamerch ジョジョDR攻略Wiki (ジョジョリンク page and its comments, research pass)',
             'page': 'https://jojodr.gamerch.com/ジョジョリンク', 'accessed': '2026-11-20',
             'note': 'research-b33/action links.jojo link records: members, rule and effect per link'},
            {'source': 'Bahamut forum (巴哈姆特)', 'page': 'https://forum.gamer.com.tw/G2.php?bsn=31489&parent=4&sn=18',
             'accessed': '2026-11-20',
             'note': 'via research-b33: the record source for links 32, 33, 40, 45, 57 and 61; corroboration for '
                     '30 and 46'},
            {'source': 'Gamerch ジョジョDR攻略Wiki (ジョジョリンク comment screenshots, research-b43)',
             'page': 'https://jojodr.gamerch.com/ジョジョリンク', 'accessed': '2026-11-22',
             'note': 'research-b43/links_traits_minds: in-game team screenshots attached to comments (read as data, '
                     'not stored) give the effects of links 53, 54, 64 and 76; cited per link in effectSource'},
            {'source': 'Gamerch ジョジョDR攻略Wiki (Wayback Machine snapshot, saved link table)',
             'page': saved_src['links']['url'], 'accessed': saved_src['links']['accessed'],
             'note': 'local/wiki-raw/research-2026-11-15-v2/catalog/jojo_links.json: J001 = link 34, J032 = link 12 '
                     '(confidence provisional)'},
            {'source': 'jdr-research', 'doc': 'docs/research/30-action-battle.md §7',
             'note': 'link affects: source 4 to all three members, key charaLinkId; units as the leader abilities'},
            {'source': 'jdr-default',
             'note': 'ids JOLINK_<nnn> and AF_* are ours; the member -> family mapping (STATUE_NAME labels, '
                     'FAMILY_ALIAS), RULE_OVERRIDES, RESOLVED and HAND_HELD in tools/content/build_jojo_links.py, '
                     'and the stricter rule where sources conflict'},
        ],
        'links': links,
        'held': held,
        'report': report,
    }
    with open(a.out, 'w', encoding='utf-8') as f:
        f.write(V.dumps(doc))
    added, radded = V.append_affects(used, a.affect_table, a.rule_table)
    report['affect_rows_added'], report['rule_rows_added'] = added, radded
    print(json.dumps(report, indent=1, ensure_ascii=False))
    print('served:', ' '.join(str(l['charaLinkId']) for l in links))
    print('held:', ' '.join(str(h['charaLinkId']) for h in held))
    if a.report:
        with open(a.report, 'w', encoding='utf-8') as f:
            f.write(V.dumps(report))


if __name__ == '__main__':
    main()
