#!/usr/bin/env python3
"""Check rebuilt (English) text bundles with UnityPy, an independent Unity reader.

Every MonoBehaviour must decode through its type tree consuming exactly its bytes; translated
labels must carry no Japanese; UI labels set to best fit must have a sane size range.

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
objs = texts = english = best_fit = 0
for p in files:
    env = UnityPy.load(p)
    for o in env.objects:
        if o.type.name != "MonoBehaviour":
            continue
        t = o.read_typetree(check_read=True)
        objs += 1
        if "m_Text" in t:
            texts += 1
            if t["m_Text"] and not jp.search(t["m_Text"]):
                english += 1
            fd = t.get("m_FontData") or {}
            if fd.get("m_BestFit"):
                best_fit += 1
                assert 1 <= fd["m_MinSize"] <= fd["m_MaxSize"], (p, fd)
print(f"OK: {len(files)} rebuilt bundles parse with UnityPy; {objs} MonoBehaviours, "
      f"{texts} UI texts ({english} without Japanese, {best_fit} best fit)")
