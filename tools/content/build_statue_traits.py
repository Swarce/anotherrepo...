#!/usr/bin/env python3
"""Statue traits (StatueInfo.tribeTypeList, docs/research/30-action-battle.md §7) ->
server/content/statue_traits.json. Trait ids only (Jjs tribe ids, the ATTR_TEXT_<n> labels); no wiki
text goes into the output.

usage:
  python3 -I tools/content/build_statue_traits.py \
      --research local/wiki-raw/research-b33/action/results.jsonl \
      --research-statues local/wiki-raw/research-b33/statues/results.jsonl \
      --wiki local/wiki-raw/gamerch/statues.json --labels local/l10n/export-b33.tsv \
      --shoutwiki-pages local/wiki-raw/shoutwiki/pages.json \
      [--server-statues server/content/statues.json] [--assist server/content/assist_abilities.json] \
      [--out server/content/statue_traits.json]

The client ORs 1<<n of each {tribeTypeId: n} into the statue's attribute flag (AffectStatus.
SetStatueInfo 0x2062124): it feeds the PRVS_ATRBT conditions and the ATRBT target damage of the
action abilities, the trait list of the statue detail screens and the tactical Vampire death effect.

Rules, in order:
  1. Gamerch 'traits' records of the research pass (live statue pages): accepted only when the
     leader_ability record with the same url is accepted by the leader matching of
     build_action_abilities.py (leader name label + the title's rarity and type), or when the Wayback
     dump has that statue page (its statue_page_url). The statue comes from that match, never from
     the record's statueId. JP names map through the player's ATTR_TEXT_<n> JP labels after NFKC (plus
     the wiki typo 大アルアナ); 'アシスト専用' (the assist-only mark) and anything else are ignored, and a
     record that maps to no trait is ignored (it is not an empty set).
  2. Else the [[Category:...]] tags of the ShoutWiki page that statues.json or assist_abilities.json
     cite for the statue, or that a research-b33/statues record (confidence high or medium) cites,
     through the alias table below (our English ATTR labels differ from the category names). A page
     with no trait category gives nothing.
  3. Family fill: a statue with no own set takes its family's set (statueId[:4]) only when every own
     set in the family agrees.
  4. Hand siblings (SIBLINGS): a family with no own set takes the agreeing set of the named families.
Gamerch wins where both give a set; the differences are printed. Only ids 1-11 and 15 are sent (never
12-18 or 21-24).
"""
import argparse, collections, importlib.util, json, os, re, unicodedata, urllib.parse

_spec = importlib.util.spec_from_file_location(
    'action_vocab', os.path.join(os.path.dirname(os.path.abspath(__file__)), 'action_vocab.py'))
V = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(V)

ALLOWED = set(range(1, 12)) | {15}
SHOUTWIKI_CATEGORY = {
    'Vampire': 1, 'Vampires': 1, 'Joestar Bloodline': 2, 'Joestar Family': 2, 'Hamon User': 3,
    'Pillar Man': 4, 'Pillar Men': 4, 'Stand User': 5, 'Major Arcana': 6,
    'Egypt 9 Glory Gods': 7, 'Egyptian 9 Glory Gods': 7, 'Nine Egyptian Gods': 7,
    'Ultimate Life Form': 8, 'Ultimate Lifeform': 8, 'Bucciarati Gang': 9, 'Assassination Squad': 10,
    'Requiem': 15,
}
# family -> families whose (agreeing) set it takes when it has none of its own
SIBLINGS = {
    '3323': (['3321', '3322'], 'DIO: the same character as 3321/3322'),
    '4031': (['4030'], 'Rohan: the same character as 4030'),
}
SHOUTWIKI = 'https://jojodiamondrecords.shoutwiki.com/wiki/'


def key_url(u):
    return unicodedata.normalize('NFKC', urllib.parse.unquote(u or '')).strip()


def title_of(url):
    url = (url or '').split(' (')[0]
    if not url.startswith(SHOUTWIKI):
        return None
    return urllib.parse.unquote(url[len(SHOUTWIKI):]).replace('_', ' ')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--research', required=True)
    ap.add_argument('--research-statues', required=True)
    ap.add_argument('--wiki', required=True)
    ap.add_argument('--labels', required=True)
    ap.add_argument('--shoutwiki-pages', required=True)
    ap.add_argument('--server-statues', default='server/content/statues.json')
    ap.add_argument('--assist', default='server/content/assist_abilities.json')
    ap.add_argument('--out', default='server/content/statue_traits.json')
    a = ap.parse_args()

    labels = V.load_labels(a.labels)
    V.check_labels(labels)
    jp_trait = {}
    for n in ALLOWED:
        jp = labels.get('text:AttributeText_JP:ATTR_TEXT_%d' % n)
        assert jp, n
        jp_trait[V.nfkc(jp)] = n
    jp_trait.update(V.TRIBE_TYPO)
    for name, n in SHOUTWIKI_CATEGORY.items():
        assert n in ALLOWED and 'text:AttributeText_JP:ATTR_TEXT_%d' % n in labels, name
    srv = V.server_statues(a.server_statues)
    lead_jp = V.leader_names(labels)

    # 1) Gamerch traits records -> statue (leader match by url, else the Wayback dump's statue page).
    recs = [json.loads(l) for l in open(a.research, encoding='utf-8')]
    by_url = {}
    for r in recs:
        sid = V.match_leader_record(r, lead_jp, srv)
        if sid and sid != 'held':
            by_url.setdefault(key_url(r['url']), sid)
    wiki = json.load(open(a.wiki, encoding='utf-8'))
    for s in wiki['statues']:
        u = (s.get('source') or {}).get('statue_page_url')
        if u and s.get('statue_id') in srv:
            by_url.setdefault(key_url(u), s['statue_id'])
    gamerch, gamerch_conflict, stats = {}, set(), collections.Counter()
    accessed_g = ''
    for r in recs:
        if r.get('field') != 'traits':
            continue
        stats['gamerch_records'] += 1
        sid = by_url.get(key_url(r.get('url')))
        if not sid:
            stats['gamerch_unmatched'] += 1
            continue
        vals = r['value'] if isinstance(r['value'], list) else [r['value']]
        words = [V.nfkc(w).strip() for v in vals for w in re.split('[、,]', v or '')]
        got = sorted({jp_trait[w] for w in words if w in jp_trait})
        if not got:
            stats['gamerch_no_trait'] += 1
            continue
        if sid in gamerch and gamerch[sid] != got:
            gamerch_conflict.add(sid)
        gamerch.setdefault(sid, got)
        accessed_g = max(accessed_g, r.get('accessed') or '')
    for sid in gamerch_conflict:
        del gamerch[sid]

    # 2) ShoutWiki categories of the cited pages.
    pages = json.load(open(a.shoutwiki_pages, encoding='utf-8'))
    cited = collections.defaultdict(set)
    for s in json.load(open(a.server_statues, encoding='utf-8'))['statues']:
        t = title_of((s.get('source') or {}).get('page'))
        if t:
            cited[s['statueId']].add(t)
    for s in json.load(open(a.assist, encoding='utf-8'))['statues']:
        t = title_of((s.get('source') or {}).get('page'))
        if t:
            cited[s['statueId']].add(t)
    for line in open(a.research_statues, encoding='utf-8'):
        r = json.loads(line)
        t = title_of(r.get('url'))
        if t and r.get('confidence') in ('high', 'medium') and r.get('statueId') in srv:
            cited[r['statueId']].add(t)
    shout, shout_conflict, revids = {}, set(), {}
    for sid, titles in cited.items():
        sets = set()
        for t in titles:
            p = pages.get(t)
            if not p:
                continue
            cats = [c.strip() for c in re.findall(r'\[\[Category:([^\]|]+)', p.get('content', ''))]
            got = tuple(sorted({SHOUTWIKI_CATEGORY[c] for c in cats if c in SHOUTWIKI_CATEGORY}))
            if got:
                sets.add(got)
                revids[t] = int(p['revid'])
        if len(sets) > 1:
            shout_conflict.add(sid)
        elif sets:
            shout[sid] = list(next(iter(sets)))

    # 3) own sets, family fill, siblings
    tribes, frm = {}, {}
    disagree = []
    for sid in srv:
        if sid in gamerch:
            tribes[sid], frm[sid] = gamerch[sid], 'gamerch'
            if sid in shout and shout[sid] != gamerch[sid]:
                disagree.append((sid, gamerch[sid], shout[sid]))
        elif sid in shout:
            tribes[sid], frm[sid] = shout[sid], 'shoutwiki'
    own = dict(tribes)
    fam = collections.defaultdict(list)
    for sid in srv:
        fam[sid[:4]].append(sid)

    def family_set(f):
        sets = {tuple(own[s]) for s in fam.get(f, []) if s in own}
        return list(next(iter(sets))) if len(sets) == 1 else None

    for f, sids in fam.items():
        fs = family_set(f)
        for sid in sids:
            if sid not in tribes and fs is not None:
                tribes[sid], frm[sid] = fs, 'family'
    for f, (sibs, _why) in SIBLINGS.items():
        if any(s in own for s in fam.get(f, [])):
            continue
        sets = {tuple(own[s]) for g in sibs for s in fam.get(g, []) if s in own}
        if len(sets) == 1:
            for sid in fam.get(f, []):
                if sid not in tribes:
                    tribes[sid], frm[sid] = list(next(iter(sets))), 'sibling'
    for sid, t in tribes.items():
        assert t and set(t) <= ALLOWED, sid
    none = sorted(s for s in srv if s not in tribes)

    doc = {
        'about': 'Statue traits: the {tribeTypeId: n} list of StatueInfo.tribeTypeList per statueId '
                 '(docs/research/30-action-battle.md §7). n is the Jjs tribe id of the ATTR_TEXT_<n> label; only '
                 'ids 1-11 and 15 are used. "from" says where a statue\'s set comes from: gamerch (its live statue '
                 'page), shoutwiki (the page\'s categories), family (the agreeing set of its statueId[:4] family) '
                 'or sibling (a hand-named family of the same character). Statues in "none" have no source and send '
                 '[]. Built by tools/content/build_statue_traits.py.',
        'sources': [
            {'source': 'Gamerch ジョジョDR攻略Wiki (live statue pages, research pass)', 'page': V.GAMERCH,
             'accessed': accessed_g,
             'note': 'research-b33/action \'traits\' records; the statue is the one the leader matching (or the '
                     'Wayback dump) gives the page, never the record\'s own statueId'},
            {'source': 'Diamond Records Wiki (ShoutWiki)', 'page': 'https://jojodiamondrecords.shoutwiki.com/',
             'license': 'CC BY-SA 3.0', 'accessed': '2026-11-14',
             'note': '[[Category:...]] tags of the page cited for the statue, through an alias table',
             'revids': dict(sorted(revids.items()))},
            {'source': 'jdr-default',
             'note': 'family fill: a statue with no own set takes its family\'s set when every own set of the family '
                     'agrees; hand siblings: ' + '; '.join('%s <- %s (%s)' % (f, '/'.join(s), w)
                                                           for f, (s, w) in sorted(SIBLINGS.items()))},
        ],
        'tribes': dict(sorted(tribes.items())),
        'from': dict(sorted(frm.items())),
        'none': none,
    }
    with open(a.out, 'w', encoding='utf-8') as f:
        f.write(V.dumps(doc))
    c = collections.Counter(frm.values())
    print(json.dumps({'statues': len(srv), 'own_sets': len(own), 'with_set': len(tribes), 'none': len(none),
                      'none_families': sorted({s[:4] for s in none}), 'from': dict(sorted(c.items())),
                      'gamerch': dict(sorted(stats.items())), 'gamerch_conflicts': sorted(gamerch_conflict),
                      'shoutwiki_conflicts': sorted(shout_conflict),
                      'disagreements (statue, gamerch, shoutwiki)': disagree}, ensure_ascii=False))


if __name__ == '__main__':
    main()
