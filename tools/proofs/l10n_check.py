#!/usr/bin/env python3
"""Check rebuilt (English) text bundles with UnityPy, an independent Unity reader.

Every MonoBehaviour must decode through its type tree consuming exactly its bytes; translated
labels must carry no Japanese; UI labels set to best fit must have a sane size range (min <= max; the game itself uses min 0).

usage: python3 tools/proofs/l10n_check.py <dir with rebuilt .bytes files>
requires: pip install UnityPy
"""
import glob
import os
import re
import sys

import UnityPy

jp = re.compile("[぀-ヿ㐀-䶿一-鿿]")
files = sorted(glob.glob(os.path.join(sys.argv[1], "*.bytes")))
assert files, "no rebuilt files"
objs = texts = english = empty = japanese = best_fit = 0
for p in files:
    env = UnityPy.load(p)
    for o in env.objects:
        if o.type.name != "MonoBehaviour":
            continue
        t = o.read_typetree(check_read=True)
        objs += 1
        if "m_Text" in t:
            texts += 1
            s = t["m_Text"]
            if not s.strip():
                empty += 1  # filled in by code at runtime
            elif jp.search(s):
                japanese += 1
            else:
                english += 1
            fd = t.get("m_FontData") or {}
            if fd.get("m_BestFit"):
                best_fit += 1
                assert 0 <= fd["m_MinSize"] <= fd["m_MaxSize"], (p, fd)
assert objs > 0 and texts > 0, "nothing decoded"
assert english > 0, "no UI text without Japanese: the English did not land"
# Every exported UI label is translated, so a Japanese label means the pipeline missed one.
assert japanese == 0, f"{japanese} UI texts still carry Japanese"
print(f"OK: {len(files)} rebuilt bundles parse with UnityPy; {objs} MonoBehaviours, "
      f"{texts} UI texts ({english} English, {empty} empty (set at runtime), 0 Japanese; {best_fit} best fit)")
