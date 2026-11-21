#!/usr/bin/env python3
"""Height of each statue model (bind-pose world bounds of its skinned meshes) from the player's own
statue_<psid> bundles, numbers only. build_statues.py uses it to size models for the team view's
bust shot (docs/research/12 §7).

usage: python3 -I tools/content/scan_model_heights.py <dir of decrypted bundles named by logical name> > local/content/model_heights.tsv
"""
import sys, UnityPy, glob, os, re
import numpy as np
def qm(q):
    x,y,z,w=q['x'],q['y'],q['z'],q['w']
    return np.array([[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w)],[2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w)],[2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]])
for p in sorted(glob.glob(sys.argv[1]+'/statue_*')):
    n=os.path.basename(p)
    if not re.fullmatch(r'statue_\d+',n): continue
    env=UnityPy.load(p)
    tr={}; go={}
    for o in env.objects:
        if o.type.name=='Transform':
            t=o.read_typetree(); tr[o.path_id]=t
        elif o.type.name=='GameObject':
            go[o.path_id]=o.read_typetree()['m_Name']
    def world(pid):
        M=np.eye(4); chain=[]
        while pid and pid in tr:
            chain.append(pid); pid=tr[pid]['m_Father']['m_PathID']
        root=chain[-1] if chain else None
        for c in reversed(chain):
            t=tr[c]; L=np.eye(4)
            L[:3,:3]=qm(t['m_LocalRotation'])*np.array([t['m_LocalScale'][k] for k in 'xyz'])
            L[:3,3]=[t['m_LocalPosition'][k] for k in 'xyz']
            M=M@L
        return M,root
    res={}
    for o in env.objects:
        if o.type.name!='SkinnedMeshRenderer': continue
        s=o.read_typetree()
        rb=s['m_RootBone']['m_PathID']
        if not rb:
            gid=s['m_GameObject']['m_PathID']
            rb=[k for k,t in tr.items() if t['m_GameObject']['m_PathID']==gid][0]
        M,root=world(rb)
        c=np.array([s['m_AABB']['m_Center'][k] for k in 'xyz']); e=np.array([s['m_AABB']['m_Extent'][k] for k in 'xyz'])
        ys=[]; xs=[]; zs=[]
        for sx in (-1,1):
            for sy in (-1,1):
                for sz in (-1,1):
                    w=M@np.append(c+e*np.array([sx,sy,sz]),1); ys.append(w[1]); xs.append(w[0])
        rn=go.get(tr[root]['m_GameObject']['m_PathID'],'?')
        r=res.setdefault(rn,[9,-9,9,-9]); r[0]=min(r[0],min(ys)); r[1]=max(r[1],max(ys)); r[2]=min(r[2],min(xs)); r[3]=max(r[3],max(xs))
    for rn,(lo,hi,xl,xh) in sorted(res.items()):
        print(f"{n[7:]}\t{rn}\t{lo:.2f}\t{hi:.2f}\t{xl:.2f}\t{xh:.2f}")
