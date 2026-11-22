#!/usr/bin/env python3
"""Action-battle leader abilities (docs/research/30-action-battle.md §7) ->
server/content/action_abilities.json, plus the AF_* rows they need in affect_table and
affect_text_rule_table. Ids, enum names and numbers only: no wiki text goes into the output (a
statue's "left" lists the hold categories of the lines not served, never their text).

usage:
  python3 -I tools/content/build_action_abilities.py --wiki local/wiki-raw/gamerch/statues.json \
      --labels local/l10n/export-b33.tsv --server-statues server/content/statues.json \
      --research local/wiki-raw/research-b33/action/results.jsonl \
      --research local/wiki-raw/research-b43/assists/results.jsonl \
      --affect-table server/content/masterdata/affect_table.json \
      --rule-table server/content/masterdata/affect_text_rule_table.json \
      [--schema server/content/masterdata/schema.json] [--out server/content/action_abilities.json] \
      [--report <file.json>]

Run after build_leader_abilities.py and before build_assist_abilities.py (docs/CONTENT.md).

Sources and matching (as build_leader_abilities.py):
  --wiki       the Gamerch Wayback dump: each record's statue_id, when ABI_LEAD_NAME_<sid> exists in
               --labels and the statue is in --server-statues. A dump record takes precedence.
  --research   leader_ability records of the research pass taken from a live Gamerch statue page
               (confidence high or medium): the page leader name equals one ABI_LEAD_NAME JP label
               after NFKC and the URL title's ★rarity and 【type】 pick exactly one roster statue.
               3041_03 and 4000_23, held in Build 41 (RESEARCH_HELD), were released in Build 43
               (research-b43/assists re-read their live pages). Where two records match one statue,
               the higher confidence wins, else the first one read. A record with 'lv1' and
               'max' lists instead of 'effects' (research-b43) reads as 'line(MAX n)'.
Each line goes through tools/content/action_vocab.py parse(): the condition prefix, the MAX forms
(Lv10 = the MAX when plausible, else Lv1; none = Lv1), the stat body and the hold rules (gold/EXP and
BARRIER have no client consumer; PER_STYPE is not traced; DoDoDo-mode DoDoDo charge has no label;
two conditions, extra text, a non-integer flat value, missing labels and bases without a consumer
are held). Two lines with the same (affectId, p1, p2) are summed. A statue with no served line is
not listed (the server sends {}).

Ids: abilityId LEAD_<sid> and the AF_[COND_]STAT_OP affect ids are ours; abilityName is the game's
ABI_LEAD_NAME_<sid> label. The server sends abilityLevel 1 (collection.rs ACTION_LEADER_LEVEL).

The affect_table / affect_text_rule_table edit (action_vocab.append_affects, idempotent, shared with
build_assist_abilities.py) appends one row per AF_* id used here plus the ACTION_AFFECT targets
(the action twins of the tactical-only DTAF_* ids that inline assist rows carry), keeping every
existing row and its order. A rerun gives byte-identical files.
"""
import argparse, collections, importlib.util, json, os

_spec = importlib.util.spec_from_file_location(
    'action_vocab', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'action_vocab.py'))
V = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--wiki', required=True)
    ap.add_argument('--labels', required=True)
    ap.add_argument('--server-statues', required=True)
    ap.add_argument('--research', action='append', default=[])
    ap.add_argument('--affect-table', required=True)
    ap.add_argument('--rule-table', required=True)
    ap.add_argument('--schema', default='server/content/masterdata/schema.json')
    ap.add_argument('--out', default='server/content/action_abilities.json')
    ap.add_argument('--report')
    a = ap.parse_args()

    V.load_schema(a.schema)
    labels = V.load_labels(a.labels)
    V.check_labels(labels)
    alabels = V.affect_labels(labels)
    lead_jp = V.leader_names(labels)
    lead_keys = {k.rsplit('ABI_LEAD_NAME_', 1)[1] for k in labels if ':ABI_LEAD_NAME_' in k}
    srv = V.server_statues(a.server_statues)
    wiki = json.load(open(a.wiki, encoding='utf-8'))

    entries = []
    for s in wiki['statues']:
        src = s.get('source') or {}
        entries.append({'sid': s.get('statue_id'), 'lines': s.get('leader_ability_values') or [],
                        'source': {'kind': 'wayback', 'source': 'Gamerch ジョジョDR攻略Wiki (Wayback Machine)',
                                   'page': src.get('page_url', ''), 'capture': src.get('capture'),
                                   'accessed': src.get('accessed', '')}})
    held_research, found = set(), []
    for path in a.research:
        for line in open(path, encoding='utf-8'):
            r = json.loads(line)
            sid = V.match_leader_record(r, lead_jp, srv)
            if sid == 'held':
                held_research.add(r.get('statueId'))
                continue
            if sid:
                found.append((sid, r))
    # Two records for one statue (research-b33 and research-b43 of the same page): the higher
    # confidence first, else the order read; the first one listed is used.
    found = [x for _i, x in sorted(enumerate(found), key=lambda t: (-V.RANK[t[1][1]['confidence']], t[0]))]
    for sid, r in found:
        entries.append({'sid': sid, 'lines': V.leader_lines(r['value']),
                        'source': {'kind': 'research', 'source': 'Gamerch ジョジョDR攻略Wiki',
                                   'page': r['url'], 'accessed': r.get('accessed', '')}})

    cats, maxes, stats = collections.Counter(), collections.Counter(), collections.Counter()
    leaders, seen = [], set()
    for e in entries:
        sid = e['sid']
        if not sid or not e['lines'] or sid in seen:
            continue
        if sid not in lead_keys:
            stats['statue_no_label'] += 1
            continue
        if sid not in srv:
            stats['statue_not_in_roster'] += 1
            continue
        seen.add(sid)
        stats['statue_matched'] += 1
        params, left = [], []
        for line in e['lines']:
            cat, p = V.parse(line, alabels)
            cats[cat] += 1
            if p is None:
                left.append(cat)
                continue
            maxes[p['max']] += 1
            params.append(p)
        if not params:
            stats['statue_nothing_served'] += 1
            continue
        stats['statue_listed'] += 1
        stats['statue_listed_' + e['source']['kind']] += 1
        if not left:
            stats['statue_every_line_served'] += 1
        leaders.append({'statueId': sid, 'abilityId': 'LEAD_' + sid, 'abilityName': 'ABI_LEAD_NAME_' + sid,
                        'params': [V.content_param(p) for p in V.merge(params)],
                        'left': sorted(left), 'source': e['source']})
    leaders.sort(key=lambda x: x['statueId'])
    used = []
    for l in leaders:
        for p in l['params']:
            if p['affectId'] not in used:
                used.append(p['affectId'])
    report = {'stats': dict(sorted(stats.items())), 'lines': dict(sorted(cats.items())),
              'max': dict(sorted(maxes.items())), 'affect_ids': len(used),
              'research_held': sorted(held_research)}

    research_pages = {'LEAD_' + l['statueId']: l['source']['page'] for l in leaders
                      if l['source']['kind'] == 'research'}
    accessed = max([l['source']['accessed'] for l in leaders if l['source']['kind'] == 'research'] or [''])
    doc = {
        'about': 'Action-battle leader abilities (StatueInfo.leaderAbilityInfo, docs/research/30-action-battle.md '
                 '§7): one entry per statue whose wiki leader ability has at least one line the action battle '
                 'reads. The server sends {abilityId, abilityLevel, abilityName, abilityParamList, isInit true} '
                 'with the params inline (paramListLv1_0..7 / paramListLv10_0..7 from lv1 / lv10). Units: p0 per '
                 'mille (N% x 10, xX as (X-1) x 1000, flat values as they are, SP uses as the count); p1 the tribe '
                 'or StatueType of a target; p2 seconds x 10, an HP threshold per mille or the holder tribe. '
                 '"left" lists the hold categories of the lines not served. Built by '
                 'tools/content/build_action_abilities.py.',
        'sources': [
            {'source': 'Gamerch ジョジョDR攻略Wiki (Wayback Machine snapshot)', 'page': V.GAMERCH,
             'accessed': wiki.get('meta', {}).get('accessed', ''),
             'note': 'leader numbers only; per-record archive URLs in local/wiki-raw/gamerch/statues.json'},
            {'source': 'Gamerch ジョジョDR攻略Wiki (live statue pages, research pass)', 'page': V.GAMERCH,
             'pages': research_pages, 'accessed': accessed,
             'note': 'research-b33/action and research-b43/assists leader_ability records: leader numbers only, '
                     'for statues the Wayback dump lacks'},
            {'source': 'jdr-research', 'doc': 'docs/research/30-action-battle.md §7',
             'note': 'units, conditions and the leader path read in the 3.12.0 client'},
            {'source': 'jdr-default',
             'note': 'ability ids LEAD_<sid> and affect ids AF_* are ours; Lv10 = Lv1 where the wiki gives no '
                     'usable MAX; abilityLevel 1'},
        ],
        'leaders': leaders,
        'report': report,
    }
    with open(a.out, 'w', encoding='utf-8') as f:
        f.write(V.dumps(doc))
    targets = [t for _d, t in V.ACTION_AFFECT]
    for t in targets:
        assert V.hold_of(t, alabels) is None, t
    added, radded = V.append_affects(used + targets, a.affect_table, a.rule_table)
    report['affect_rows_added'], report['rule_rows_added'] = added, radded
    print(json.dumps(report, indent=1))
    if a.report:
        with open(a.report, 'w', encoding='utf-8') as f:
            f.write(V.dumps(report))


if __name__ == '__main__':
    main()
