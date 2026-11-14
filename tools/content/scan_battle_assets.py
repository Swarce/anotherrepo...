#!/usr/bin/env python3
"""Inventory of the battle assets in the player's own files (identifiers only), input to build_battle_tables.py:
bullets: dt_actionset_statue_<psid> FSM names bullet_<id>, frameevents_statue_battle_<psid>
BLETList (animName -> owner model, bulletId), and model names in statue_<psid> (.fbx).

usage: python3 -I tools/content/scan_battle_assets.py <dir of decrypted bundles named by logical name> local/content/battle_assets.json
"""
import sys,os,re,json,UnityPy
d=sys.argv[1]; out={}
def objs(f):
    p=os.path.join(d,f)
    if not os.path.exists(p): return None
    return UnityPy.load(p)
psids=sorted({m.group(1) for f in os.listdir(d) for m in [re.match(r'(?:dt_actionset_statue|frameevents_statue_battle|statue)_(\d{4})$',f)] if m})
rows={}
for ps in psids:
    r={'models':[], 'dt_bullets':[], 'old_bullets':[], 'blet':[], 'dt_actions':{}}
    env=objs('statue_'+ps)
    if env:
        for k in env.container.keys():
            m=re.search(r'/(\d{4}_\d_[^/]+)\.fbx$',k)
            if m: r['models'].append(m.group(1))
    r['models']=sorted(set(r['models']))
    for fam,key in (('dt_actionset_statue_','dt_bullets'),('actionset_statue_','old_bullets')):
        env=objs(fam+ps)
        if not env: continue
        for o in env.objects:
            if o.type.name!='MonoBehaviour': continue
            try: n=o.peek_name()
            except Exception: n=None
            if not n: continue
            m=re.match(r'bullet_(\d+)$',n)
            if m: r[key].append(int(m.group(1)))
            m=re.match(r'(.+?)_(\d{3})_([A-Z][A-Z0-9_]*)$',n)
            if m and key=='dt_bullets': r['dt_actions'].setdefault(m.group(1),[]).append(m.group(3))
    env=objs('frameevents_statue_battle_'+ps)
    if env:
        for o in env.objects:
            if o.type.name!='MonoBehaviour': continue
            t=o.read_typetree()
            if 'BLETList' not in t: continue
            for b in t['BLETList']:
                r['blet'].append([b['animName'],b['bulletId']])
    rows[ps]=r
json.dump(rows,open(sys.argv[2],'w'),indent=0)
nb=sum(len(r['dt_bullets']) for r in rows.values())
print(len(rows),'psids',nb,'dt bullets',sum(len(r['blet']) for r in rows.values()),'blet')
