#!/usr/bin/env python3
"""Check rebuilt (English) text bundles with UnityPy, an independent Unity reader.

Every MonoBehaviour must decode through its type tree consuming exactly its bytes; translated
labels must carry no Japanese; UI labels set to best fit must have a sane size range (min <= max; the game itself uses min 0);
the statue name labels have their fixed box (no vertical fitter, two lines tall, best fit from the design size), and the
Town list values a one-line box.

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
name_boxes = line_boxes = 0
lines = line_jp = 0
for p in files:
    env = UnityPy.load(p)
    # statue name labels: fixed box (no vertical fitter), two lines tall, text at the top
    for o in env.objects:
        if o.type.name != "GameObject":
            continue
        g = o.read_typetree()
        town = g["m_Name"] in ("Txt_ActiveTime", "Txt_ClanPolicy", "Txt_ApprovalPolicy")
        if not (g["m_Name"].startswith("Txt_StatueName") or town):
            continue
        sf = o.assets_file
        text = fit = rt = None
        for c in g["m_Component"]:
            co = sf.objects.get(c["component"]["m_PathID"])
            if co is None:
                continue
            ct = co.read_typetree()
            if co.type.name == "RectTransform":
                rt = ct
            elif "m_FontData" in ct:
                text = ct
            elif "m_VerticalFit" in ct:
                fit = ct
        if text is None or fit is None or rt is None:
            continue
        fd = text["m_FontData"]
        if town:
            assert fit["m_VerticalFit"] == 0 and fd["m_BestFit"], (p, g["m_Name"], "Town value still sized by its fitter")
            assert abs(rt["m_SizeDelta"]["y"] - fd["m_FontSize"] * 7 / 6) < 0.01, (p, g["m_Name"], rt["m_SizeDelta"])
            line_boxes += 1
            continue
        if fit["m_VerticalFit"] == 0 and fd["m_BestFit"] and fd["m_Alignment"] < 3:
            assert abs(rt["m_SizeDelta"]["y"] - 1.5 * fd["m_FontSize"]) < 0.01, (p, g["m_Name"], rt["m_SizeDelta"])
            assert fd["m_MaxSize"] == fd["m_FontSize"] and fd["m_MinSize"] <= fd["m_MaxSize"], (p, fd)
            name_boxes += 1
        elif fit["m_VerticalFit"] == 2 and fd["m_BestFit"] and abs(fd["m_LineSpacing"] - 1.0) < 1e-6:
            raise AssertionError(f"{p}: {g['m_Name']} is still sized by its fitter")
    for o in env.objects:
        if o.type.name != "MonoBehaviour":
            continue
        t = o.read_typetree(check_read=True)
        objs += 1
        if str(t.get("m_Name", "")).endswith("_script_text_JP") and isinstance(t.get("text"), list):
            for line in t["text"]:
                lines += 1
                line_jp += bool(jp.search(line))
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
assert name_boxes >= 16, f"{name_boxes} statue name boxes"
assert line_boxes >= 9, f"{line_boxes} Town value boxes"
assert english > 0, "no UI text without Japanese: the English did not land"
# Every exported UI label is translated, so a Japanese label means the pipeline missed one.
assert japanese == 0, f"{japanese} UI texts still carry Japanese"
assert line_jp == 0, f"{line_jp} story lines still carry Japanese"
print(f"OK: {len(files)} rebuilt bundles parse with UnityPy; {objs} MonoBehaviours, "
      f"{texts} UI texts ({english} English, {empty} empty (set at runtime), 0 Japanese; {best_fit} best fit); "
      f"{lines} story lines, 0 Japanese; {name_boxes} statue name boxes, {line_boxes} Town value boxes")
