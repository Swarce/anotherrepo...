#!/usr/bin/env python3
"""Inventory of the figure (pose) animation clips per model in the player's own
animation_statue_common_<psid> bundles (identifiers only). build_statues.py uses it to pick a rarity
whose team-view pose exists (docs/research/12 §7).

usage: python3 -I tools/content/scan_figure_clips.py <dir of decrypted bundles named by logical name> local/content/figure_clips.tsv
"""
import sys, UnityPy, glob, re, collections
out=open(sys.argv[2],'w')
for p in sorted(glob.glob(sys.argv[1]+'/animation_statue_common_*')):
    psid=p.rsplit('_',1)[1]
    env=UnityPy.load(p)
    clips=collections.defaultdict(list)
    for o in env.objects:
        if o.type.name=='AnimationClip':
            n=o.peek_name()
            m=re.match(r'(\d{4}_\d_[a-z0-9]+)_(.*)',n)
            if m: clips[m.group(1)].append(m.group(2))
            else: clips['?'].append(n)
    for model,cl in sorted(clips.items()):
        fig=sorted(c for c in cl if c.startswith('figure'))
        out.write('%s\t%s\t%s\t%s\n'%(psid,model,' '.join(fig),' '.join(sorted(c for c in cl if not c.startswith('figure')))))
