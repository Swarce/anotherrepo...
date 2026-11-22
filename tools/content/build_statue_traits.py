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
      --verdicts local/wiki-raw/research-b43/links_traits_minds/results.jsonl \
      --research-b44 local/wiki-raw/research-b44/blogs_archives/results.jsonl \
      --research-b44 local/wiki-raw/research-b44/communities/results.jsonl \
      --research-b44 local/wiki-raw/research-b44/jp_strategy_sites/results.jsonl \
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
  5. Research verdicts (--verdicts, Build 43: research-b43/links_traits_minds, high/medium only), for
     statues the rules above leave without a set: a 'traits' record gives the set of the statue the
     researchers named (siblings' sets, a statue page or ShoutWiki categories; [] = the sources show
     no tribe, listed in "noTribe" and still sent as []); a 'traits_lower_bound' record gives its set
     as a lower bound (the statue may carry more). Then a statue still without a set takes the
     agreeing set of its family's own and full-verdict sets (4090_72 Tonio: the researchers derive
     [Stand User] for the whole family, whichever of 4090_71/72 the one Gamerch page is). Each such
     row names its page, capture and access date in "rowSources".
  6. Build 44 research (--research-b44: 'traits'/'tribes' records of topic links_traits_minds or statues,
     high/medium, with a statueId we serve; the highest confidence wins, an equal-confidence disagreement
     holds the statue): in-game status tabs (ameblo), the atwiki 特徴 row and a Reddit trait list. A set
     is applied to a statue without one, over a research-b43 lower bound it contains, or over our set
     when it strictly contains it (the wiki categories miss the team traits, which 3.9.0 added); a
     smaller or different set is reported, not applied, and an equal one is noted under "also". Ids
     outside 1-11/15 (Trish's 性別(女)) are reported, not sent. Then, in each family with such a set,
     the statues filled by the family rule, without a set or with a lower bound under it take the
     agreeing set of the family's sourced statues.
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
# Build 44 source families by page host (research-b44 READMEs)
B44_SOURCES = (
    ('https://ameblo.jp/', '鎖国的ブログ (ameblo; in-game status-tab screenshots, read as data)'),
    ('https://www65.atwiki.jp/jojodrr/', 'ジョジョDR２ @ ウィキ (atwiki; page source in an archived RSS feed)'),
    ('https://www.reddit.com/r/JoJoDiamondRecords/', 'r/JoJoDiamondRecords (Reddit; a translator\'s retrospective guide)'),
)


B44_PAGES = (
    ('https://ameblo.jp/yamasan-drm/', B44_SOURCES[0][1]),
    ('https://www65.atwiki.jp/jojodrr/', B44_SOURCES[1][1]),
    ('https://www.reddit.com/r/JoJoDiamondRecords/comments/dijnrg/mostly_recent_character_retrospectiveguide/',
     B44_SOURCES[2][1]),
)


def b44_row(r, also, dropped):
    """A research-b44 record as a rowSources entry (page, capture, access date, confidence)."""
    url = r['url']
    name = next(n for host, n in B44_SOURCES if url.startswith(host))
    row = {'source': name, 'page': url}
    if r.get('capture'):
        row['capture'] = r['capture']
    row.update({'accessed': r['accessed'], 'confidence': r['confidence'],
                'note': 'research-b44/%s %s' % (r['_family'], r['field'])
                        + ('; trait ids %s are shown but not sent (only 1-11 and 15 are)' % dropped if dropped else '')})
    if also:
        row['also'] = [{'source': next(n for host, n in B44_SOURCES if x['url'].startswith(host)), 'page': x['url'],
                        **({'capture': x['capture']} if x.get('capture') else {}),
                        'accessed': x['accessed'], 'confidence': x['confidence']} for x in also]
    return row


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
    ap.add_argument('--verdicts', action='append', default=[])
    ap.add_argument('--research-b44', action='append', default=[])
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

    # 5) research verdicts, then a family fill from the own and full-verdict sets
    row_sources, verdict, no_tribe, verdict_skipped = {}, {}, [], []
    for path in a.verdicts:
        for line in open(path, encoding='utf-8'):
            r = json.loads(line)
            if r.get('topic') != 'traits' or r.get('field') not in ('traits', 'traits_lower_bound') \
                    or r.get('confidence') not in ('high', 'medium'):
                continue
            sid = r['statueId']
            assert sid in srv, sid
            words = [V.nfkc(w).strip() for w in r['value']]
            assert all(w in jp_trait for w in words), (sid, words)
            got = sorted({jp_trait[w] for w in words})
            if sid in tribes or sid in row_sources:
                verdict_skipped.append(sid)
                continue
            lower = r['field'] == 'traits_lower_bound'
            url = r.get('url') or ''
            shout = url.startswith(SHOUTWIKI)
            row = {'source': 'Diamond Records Wiki (ShoutWiki)' if shout else 'Gamerch ジョジョDR攻略Wiki',
                   'page': url}
            if shout:
                row['license'] = 'CC BY-SA 3.0'
            if r.get('capture'):
                row['capture'] = r['capture']
            row.update({'accessed': r.get('accessed') or '', 'confidence': r['confidence'],
                        'note': 'research-b43/links_traits_minds ' + r['field'] + (
                            ': lower bound, the statue may carry more' if lower else
                            ': the sources show no tribe' if not got else ': the set the researchers give')})
            row_sources[sid] = row
            stats['verdict_' + ('lower_bound' if lower else 'empty' if not got else 'set')] += 1
            if not got:
                no_tribe.append(sid)
                continue
            tribes[sid], frm[sid] = got, 'research'
            if not lower:
                verdict[sid] = got
    for f, sids in sorted(fam.items()):
        sets = {tuple(t) for s in sids for t in ([own[s]] if s in own else []) + ([verdict[s]] if s in verdict else [])}
        if len(sets) != 1 or not any(s in verdict for s in sids):
            continue
        fs = list(next(iter(sets)))
        for sid in sids:
            if sid not in tribes and sid not in no_tribe:
                tribes[sid], frm[sid] = fs, 'family'
                row_sources[sid] = {'source': 'jdr-default', 'note': 'family fill from the research-b43 sets of ' +
                                    ', '.join(s for s in sids if s in verdict)}
                stats['verdict_family_fill'] += 1
    lower_bound = {sid for sid, r in row_sources.items() if 'lower bound' in r.get('note', '')}

    # 6) Build 44 research (--research-b44): in-game status tabs, the atwiki and Reddit trait lists
    b44_applied, b44_confirmed, b44_held, b44_dropped, b44_unread = {}, [], [], {}, []
    jp_any = {V.nfkc(labels[k]): int(k.rsplit('_', 1)[1]) for k in labels
              if re.fullmatch(r'text:AttributeText_JP:ATTR_TEXT_[0-9]+', k)}
    b44 = collections.defaultdict(list)
    for path in a.research_b44:
        for line in open(path, encoding='utf-8'):
            r = json.loads(line)
            r['_family'] = os.path.basename(os.path.dirname(path))
            if r.get('topic') not in ('links_traits_minds', 'statues') or r.get('field') not in ('traits', 'tribes') \
                    or r.get('confidence') not in ('high', 'medium'):
                continue
            sid = r.get('statueId')
            if sid not in srv:
                b44_unread.append(sid or r.get('subject'))
                continue
            v = r['value']
            words = v if isinstance(v, list) else v.get('labels', [])
            ids = sorted({jp_any[V.nfkc(w).strip()] for w in words})
            if isinstance(v, dict):
                given = v.get('tribeTypeIds', v.get('tribe_ids'))
                assert given is None or sorted(given) == ids, (sid, given, ids)
            b44[sid].append((r, ids))
    for sid, recs in sorted(b44.items()):
        recs.sort(key=lambda x: {'high': 0, 'medium': 1}[x[0]['confidence']])
        r, ids = recs[0]
        got = [n for n in ids if n in ALLOWED]
        if any(n not in ALLOWED for n in ids):
            b44_dropped[sid] = [n for n in ids if n not in ALLOWED]
        same = [x for x, i in recs[1:] if [n for n in i if n in ALLOWED] == got]
        if any(x['confidence'] == r['confidence'] for x, i in recs[1:] if x not in same):
            b44_held.append((sid, 'the research-b44 records disagree'))
            continue
        row = b44_row(r, same, b44_dropped.get(sid))
        cur = tribes.get(sid)
        if sid in no_tribe:
            if got:
                b44_held.append((sid, 'research-b44 %s against no tribe' % got))
            else:
                row_sources[sid].setdefault('also', []).append(row)
                b44_confirmed.append(sid)
            continue
        if cur is not None and set(got) == set(cur) and sid not in lower_bound:
            b44_confirmed.append(sid)
            if sid in row_sources:
                row_sources[sid].setdefault('also', []).append(row)
            continue
        if not got or (cur is not None and not set(got) > set(cur)
                       and not (sid in lower_bound and set(got) >= set(cur))):
            b44_held.append((sid, 'research-b44 %s against ours %s (%s)' % (got, cur, frm.get(sid))))
            continue
        if cur is not None:
            row['note'] += ('; replaces the research-b43 lower bound %s' % cur if sid in lower_bound
                            else '; extends %s (from %s), whose source lacks %s' % (
                                cur, frm[sid], ', '.join(str(n) for n in got if n not in cur)))
        tribes[sid], frm[sid], row_sources[sid] = got, 'research', row
        lower_bound.discard(sid)
        b44_applied[sid] = got
        stats['b44_set'] += 1
    # Family fill from the Build 44 sets: in a family with a Build 44 set, the statues filled by the family
    # rule, still without a set or with a lower bound under it take the agreeing set of its sourced statues.
    for f, sids in sorted(fam.items()):
        if not any(s in b44_applied for s in sids):
            continue
        sets = {tuple(tribes[s]) for s in sids if s in tribes and frm[s] != 'family' and s not in lower_bound}
        if len(sets) != 1:
            continue
        fs = list(next(iter(sets)))
        for sid in sids:
            was = tribes.get(sid)
            if sid in no_tribe or was == fs or (was is not None and frm[sid] != 'family' and sid not in lower_bound) \
                    or (sid in lower_bound and not set(fs) >= set(was)):
                continue
            tribes[sid], frm[sid] = fs, 'family'
            prev = row_sources.get(sid)
            row_sources[sid] = {'source': 'jdr-default', 'note': 'family fill from the research-b44 sets of ' +
                                ', '.join(s for s in sids if s in b44_applied) +
                                ('' if was is None else '; was %s' % was)}
            if sid in lower_bound and prev and prev.get('source') != 'jdr-default':
                # the lower bound the fill extends keeps its citation
                row_sources[sid]['also'] = [prev]
            lower_bound.discard(sid)
            stats['b44_family_fill'] += 1
    b44_rows = [r for r in row_sources.values() for r in [r] + r.get('also', [])
                if r.get('note', '').startswith('research-b44')]
    b44_accessed = max((r['accessed'] for r in b44_rows), default='')
    for sid, t in tribes.items():
        assert t and set(t) <= ALLOWED, sid
    no_tribe.sort()
    none = sorted(s for s in srv if s not in tribes and s not in no_tribe)

    doc = {
        'about': 'Statue traits: the {tribeTypeId: n} list of StatueInfo.tribeTypeList per statueId '
                 '(docs/research/30-action-battle.md §7). n is the Jjs tribe id of the ATTR_TEXT_<n> label; only '
                 'ids 1-11 and 15 are used. "from" says where a statue\'s set comes from: gamerch (its live statue '
                 'page), shoutwiki (the page\'s categories), family (the agreeing set of its statueId[:4] family) '
                 'or sibling (a hand-named family of the same character); from Build 43 also research (a research '
                 'verdict, cited per row in "rowSources"; Build 44 also extends a set by the traits a status tab or trait '
                 'list shows). Statues in "noTribe" have sources that show no tribe and '
                 'statues in "none" have no source; both send []. Built by tools/content/build_statue_traits.py.',
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
            {'source': 'research-b43 (links_traits_minds): Gamerch ジョジョDR攻略Wiki and ShoutWiki', 'page': V.GAMERCH,
             'accessed': max(r['accessed'] for r in row_sources.values() if 'accessed' in r) if row_sources else '',
             'note': 'traits for statues the rules above leave without a set (siblings\' Gamerch sets, statue pages, '
                     'ShoutWiki categories; high/medium only), each cited in rowSources with page, capture and '
                     'access date'},
            *([{'source': name, 'page': page, 'accessed': b44_accessed,
                'note': 'research-b44 (Build 44): sets for statues without one, and sets that extend ours by a trait '
                        'the wiki categories lack (high/medium; a smaller or different set is reported, not applied), '
                        'then a family fill from them; each cited in rowSources (a confirming record under "also")'}
               for page, name in B44_PAGES] if b44_applied else []),
        ],
        'tribes': dict(sorted(tribes.items())),
        'from': dict(sorted(frm.items())),
        'rowSources': dict(sorted(row_sources.items())),
        'noTribe': no_tribe,
        'none': none,
    }
    with open(a.out, 'w', encoding='utf-8') as f:
        f.write(V.dumps(doc))
    c = collections.Counter(frm.values())
    print(json.dumps({'statues': len(srv), 'own_sets': len(own), 'with_set': len(tribes), 'none': len(none),
                      'none_families': sorted({s[:4] for s in none}), 'from': dict(sorted(c.items())),
                      'gamerch': dict(sorted(stats.items())), 'gamerch_conflicts': sorted(gamerch_conflict),
                      'shoutwiki_conflicts': sorted(shout_conflict), 'verdicts_skipped': verdict_skipped,
                      'no_tribe': no_tribe, 'b44_applied': b44_applied, 'b44_confirmed': b44_confirmed,
                      'b44_held': b44_held, 'b44_not_sent': b44_dropped, 'b44_unread': b44_unread,
                      'disagreements (statue, gamerch, shoutwiki)': disagree}, ensure_ascii=False))


if __name__ == '__main__':
    main()
