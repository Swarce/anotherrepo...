#!/usr/bin/env python3
"""Does our English fit the UI? Measures every UI.Text of the player's own UI bundles with the game's
own font, the way Unity's TextGenerator lays it out (word wrap, line height, best fit), for the
original Japanese and for our English. Writes a private report (keys, numbers, our English; no game
text) to --out.

The Japanese was laid out to fit by the game's designers, so the share of Japanese labels that
"fit" checks the measurement itself (--calibrate prints it for a range of line-height factors).

usage: python3 -I tools/l10n/ui_fit.py --dec <dir of decrypted bundles> --repo . --out local/l10n/b19/fit.tsv
"""
import argparse, collections, glob, hashlib, json, math, os, re, sys
import UnityPy
from PIL import ImageFont

ap = argparse.ArgumentParser()
ap.add_argument("--dec", required=True)
ap.add_argument("--repo", default=".")
ap.add_argument("--out", required=True)
ap.add_argument("--line", type=float, default=None, help="line height / font size (default: calibrated)")
ap.add_argument("--calibrate", action="store_true")
ap.add_argument("--hints", help="write label layout hints (ids only) for the server, e.g. server/content/l10n/ui_layout.json")
ap.add_argument("--list-boxes", action="store_true", help="print the fixed-box labels")
ap.add_argument("--screen", default="1280x720", help="reference canvas size for stretched roots")
a = ap.parse_args()
SW, SH = (float(x) for x in a.screen.split("x"))

JP = re.compile(r"[぀-ヿ㐀-鿿＀-￯]")

# ---- fonts (filled while the bundles load, below) ---------------------------------------------
BITMAP = {}
_pil = {}
CUR = ["FOT-NewRodinPro-DB"]  # font of the label being measured


def font(sz):
    sz = max(1, int(round(sz)))
    k = (CUR[0], sz)
    if k not in _pil:
        _pil[k] = ImageFont.truetype(FONT_FILES.get(CUR[0]) or FONT_FILES["FOT-NewRodinPro-DB"], sz)
    return _pil[k]


def width(s, sz):
    return font(sz).getlength(s) if s else 0.0


# ---- Unity-like layout --------------------------------------------------------------------------
RICH = re.compile(r"</?(b|i|size|color|material|quad)(=[^>]*)?>")


def wrap(text, sz, maxw):
    """Lines Unity's TextGenerator would make (word wrap; CJK may break anywhere)."""
    lines = []
    for para in RICH.sub("", text).replace("\\n", "\n").split("\n"):
        cur = ""
        # tokens: words with their trailing space, or single CJK characters
        toks = re.findall(r"[぀-ヿ㐀-鿿＀-￯]|[^\s぀-ヿ㐀-鿿＀-￯]+\s*|\s+", para)
        for tk in toks:
            if width(cur + tk.rstrip(), sz) <= maxw or not cur:
                if not cur and width(tk.rstrip(), sz) > maxw:
                    # a single word longer than the line: Unity breaks it by character
                    piece = ""
                    for ch in tk:
                        if width(piece + ch, sz) > maxw and piece:
                            lines.append(piece)
                            piece = ""
                        piece += ch
                    cur = piece
                else:
                    cur += tk
            else:
                lines.append(cur.rstrip())
                cur = tk.lstrip()
        lines.append(cur.rstrip())
    return lines


def fits(text, sz, w, h, hwrap, vtrunc, line_k, spacing):
    if not hwrap:
        lines = RICH.sub("", text).replace("\\n", "\n").split("\n")
        widest = max(width(l, sz) for l in lines)
        ok_w = True  # horizontal overflow: never wraps, may spill (reported separately)
    else:
        lines = wrap(text, sz, w)
        widest = max(width(l, sz) for l in lines)
        ok_w = widest <= w + 0.5
    lh = sz * line_k * spacing
    need_h = lh * (len(lines) - 1) + sz * line_k
    ok_h = (need_h <= h + 0.5) or not vtrunc
    return ok_w and ok_h, len(lines), widest, need_h


def best_size(text, fd, w, h, line_k):
    """(size used, fits) as Unity would choose with this FontData."""
    size = fd["m_FontSize"]
    hwrap = fd["m_HorizontalOverflow"] == 0
    vtrunc = fd["m_VerticalOverflow"] == 0
    sp = fd.get("m_LineSpacing", 1.0) or 1.0
    if not fd["m_BestFit"]:
        ok = fits(text, size, w, h, hwrap, vtrunc, line_k, sp)[0]
        return size, ok
    lo, hi = max(1, fd["m_MinSize"]), max(fd["m_MinSize"], fd["m_MaxSize"])
    for s in range(hi, lo - 1, -1):
        if fits(text, s, w, h, hwrap, vtrunc, line_k, sp)[0]:
            return s, True
    return lo, False


# ---- bundles ---------------------------------------------------------------------------------
def load_l10n():
    m = {}
    for p in glob.glob(os.path.join(a.repo, "server/content/l10n/en/*.json")):
        for k, (h, en) in json.load(open(p))["entries"].items():
            m[k] = (h, en)
    return m


L10N = load_l10n()
TEXT_BY_LABEL = {}
for k in L10N:
    if k.startswith("text:"):
        TEXT_BY_LABEL.setdefault(k.split(":", 2)[2], k)

# Labels the game fills at runtime from text.jp rows (their baked text is empty or a placeholder):
# (path regex, key prefixes of the rows they show). Their English is measured against the box too.
CHAPTERS = ("text:ChapterName_JP:", "text:EventChapterName_JP:")
RUNTIME = [
    (r"(BizarreChapterBanner|BizarreEventChapterBanner)/Txt_ChapterName$", CHAPTERS),
    (r"StartCall/Txt_QuestTitle$", CHAPTERS),
    (r"PauseWindowGroup/.*/Img_TitlePanel/Txt_ChapterName$", CHAPTERS),
    (r"Img_StageTitle/Txt_QuestTitleValue$", CHAPTERS),
    (r"BizarreDungeonBanner/Txt_DungeonName$", ("text:DungeonStageName_JP:",)),
    (r"Img_ClanCondition0/Txt_ClanCondition0Value$", ("text:ClanText_JP:ClanPolicy_",)),
    (r"Img_ClanCondition1/Txt_ClanCondition1Value$", ("text:ClanText_JP:ClanActiveTime_",)),
    (r"Img_ClanCondition2/Txt_ClanCondition2Value$", ("text:ClanText_JP:ClanApproval_",)),
]


def runtime_texts(path):
    out = set()
    for rx, prefixes in RUNTIME:
        if re.search(rx, path):
            out |= {en for k, (h, en) in L10N.items() if k.startswith(prefixes) and en.strip()}
    return sorted(out)
JA_SRC = {}  # Japanese of text rows, from the private export (never written out)
ap2 = os.path.join(a.repo, "local/l10n/work3.tsv")
if os.path.exists(ap2):
    import csv
    csv.field_size_limit(10 ** 9)
    with open(ap2) as f:
        rd = csv.reader(f, delimiter="\t")
        hd = next(rd)
        ki, ji = hd.index("key"), hd.index("japanese")
        for x in rd:
            if x[ki].startswith("text:"):
                JA_SRC[x[ki]] = x[ji].replace("\\n", "\n")


def sha8(s):
    return hashlib.sha256(s.encode()).hexdigest()[:8]


# Every Font object in the loaded bundles, by (serialized file name, path id): labels name their font
# through their file's externals list (a shared-assets file, or the fonts bundle's CAB).
FONT_AT = {}
FONT_FILES = {}
SCRIPT_AT = {}


def sfiles_of(env):
    out = []
    for bf in env.files.values():
        for fname, f in getattr(bf, "files", {}).items():
            if hasattr(f, "objects"):
                out.append((fname, f))
    return out


ENVS = {}
for path in sorted(glob.glob(os.path.join(a.dec, "*"))):
    name = os.path.basename(path)
    if re.match(r"(ui_|scene_|minigame_|fonts$)", name):
        ENVS[name] = UnityPy.load(path)
        for fname, sf in sfiles_of(ENVS[name]):
            for pid, o in sf.objects.items():
                if o.type.name == "MonoScript":
                    SCRIPT_AT[(fname.lower(), pid)] = o.read_typetree()["m_ClassName"]
                if o.type.name == "Font":
                    ft = o.read_typetree()
                    FONT_AT[(fname.lower(), pid)] = ft["m_Name"]
                    if ft["m_FontData"] and ft["m_Name"] not in FONT_FILES:
                        fp = os.path.join(os.path.dirname(a.out), "_font_%s.bin" % ft["m_Name"])
                        open(fp, "wb").write(bytes(x & 0xFF for x in ft["m_FontData"]))
                        FONT_FILES[ft["m_Name"]] = fp
                    elif not ft["m_FontData"] and ft.get("m_CharacterRects"):
                        BITMAP[ft["m_Name"]] = {chr(r["index"]) for r in ft["m_CharacterRects"]}


def resolve(sf, fname, pptr, table):
    fid, pid = pptr["m_FileID"], pptr["m_PathID"]
    if fid == 0:
        return table.get((fname.lower(), pid))
    ext = sf.externals[fid - 1].path.lower().rsplit("/", 1)[-1]
    return table.get((ext, pid))


def resolve_font(sf, fname, pptr):
    fid, pid = pptr["m_FileID"], pptr["m_PathID"]
    if fid == 0:
        return FONT_AT.get((fname.lower(), pid), "?")
    ext = sf.externals[fid - 1].path.lower()
    if "unity default resources" in ext:
        return "builtin:%d" % pid
    return FONT_AT.get((ext.rsplit("/", 1)[-1], pid), "?" + ext.rsplit("/", 1)[-1])


rows = []
for path in sorted(glob.glob(os.path.join(a.dec, "*"))):
    name = os.path.basename(path)
    if not re.match(r"(ui_|scene_|minigame_)", name):
        continue
    env = ENVS[name]
    for fi, (sfname, sf) in enumerate(sfiles_of(env)):
        objs = sf.objects
        tt = {}

        def tree(pid):
            if pid not in tt:
                tt[pid] = objs[pid].read_typetree()
            return tt[pid]

        texts = []
        for pid, o in objs.items():
            if o.type.name != "MonoBehaviour":
                continue
            try:
                t = tree(pid)
            except Exception:
                continue
            if "m_FontData" in t and "m_Text" in t:
                texts.append((pid, t))
        if not texts:
            continue
        # RectTransforms by GameObject, and parent chains
        rt_by_go, go_name, go_comps = {}, {}, {}
        for pid, o in objs.items():
            if o.type.name == "RectTransform":
                t = tree(pid)
                rt_by_go[t["m_GameObject"]["m_PathID"]] = pid
            elif o.type.name == "GameObject":
                g = tree(pid)
                go_name[pid] = g["m_Name"]
                go_comps[pid] = [c["component"]["m_PathID"] for c in g["m_Component"]]

        def scripts(gid):
            """{class name: typetree} of the MonoBehaviours on a GameObject."""
            out = {}
            for cp in go_comps.get(gid, []):
                if cp in objs and objs[cp].type.name == "MonoBehaviour":
                    try:
                        ct = tree(cp)
                    except Exception:
                        continue
                    n = resolve(sf, sfname, ct["m_Script"], SCRIPT_AT)
                    if n:
                        out[n] = ct
            return out

        def driven(gid, rpid):
            """Sized at runtime: a ContentSizeFitter on the label, or a layout group that controls it."""
            sc = scripts(gid)
            csf = sc.get("ContentSizeFitter")
            if csf and (csf.get("m_HorizontalFit") or csf.get("m_VerticalFit")):
                return "fitter-h" if csf.get("m_HorizontalFit") else "fitter-v"
            fa = tree(rpid)["m_Father"]["m_PathID"]
            if fa in objs and objs[fa].type.name == "RectTransform":
                psc = scripts(tree(fa)["m_GameObject"]["m_PathID"])
                for n in ("HorizontalLayoutGroup", "VerticalLayoutGroup", "GridLayoutGroup"):
                    if n in psc:
                        lt = psc[n]
                        if n == "GridLayoutGroup" or lt.get("m_ChildControlWidth") or lt.get("m_ChildControlHeight"):
                            return "layout"
            return ""
        size_cache = {}

        def rect_size(rpid):
            if rpid in size_cache:
                return size_cache[rpid]
            t = tree(rpid)
            fa = t["m_Father"]["m_PathID"]
            amin, amax, sd = t["m_AnchorMin"], t["m_AnchorMax"], t["m_SizeDelta"]
            if fa and fa in objs and objs[fa].type.name == "RectTransform":
                pw, ph = rect_size(fa)
            else:
                pw, ph = SW, SH
            r = (pw * (amax["x"] - amin["x"]) + sd["x"], ph * (amax["y"] - amin["y"]) + sd["y"])
            size_cache[rpid] = r
            return r

        def parent_w(rpid):
            fa = tree(rpid)["m_Father"]["m_PathID"]
            if fa in objs and objs[fa].type.name == "RectTransform":
                return rect_size(fa)[0]
            return 0

        def edges(rpid, pw, ph):
            """(left, right, bottom, top) of a RectTransform in its parent's space."""
            t = tree(rpid)
            amin, amax, sd, pos, piv = t["m_AnchorMin"], t["m_AnchorMax"], t["m_SizeDelta"], t["m_AnchoredPosition"], t["m_Pivot"]
            w, h = pw * (amax["x"] - amin["x"]) + sd["x"], ph * (amax["y"] - amin["y"]) + sd["y"]
            px = pw * (amin["x"] + piv["x"] * (amax["x"] - amin["x"])) + pos["x"]
            py = ph * (amin["y"] + piv["y"] * (amax["y"] - amin["y"])) + pos["y"]
            return px - piv["x"] * w, px + (1 - piv["x"]) * w, py - piv["y"] * h, py + (1 - piv["y"]) * h

        def room_right(rpid):
            """Width a left-anchored label (pivot x 0, not stretched sideways) can take: up to the
            nearest sibling to its right on its row (8 px gap) or its parent's right edge (4 px)."""
            t = tree(rpid)
            fa = t["m_Father"]["m_PathID"]
            if t["m_Pivot"]["x"] != 0 or t["m_AnchorMin"]["x"] != t["m_AnchorMax"]["x"]:
                return None
            if not (fa in objs and objs[fa].type.name == "RectTransform"):
                return None
            pw, ph = rect_size(fa)
            l, r, b, tp = edges(rpid, pw, ph)
            if tp - b < 1:
                b, tp = b - 10, tp + 10
            room = pw - 4 - l
            for c in tree(fa)["m_Children"]:
                cp = c["m_PathID"]
                if cp == rpid or cp not in objs:
                    continue
                cl, cr, cb, ct = edges(cp, pw, ph)
                if cr - cl > 1 and ct - cb > 1 and cl > l + 4 and cb < tp and ct > b:
                    room = min(room, cl - 8 - l)
            return room

        def go_path(rpid):
            parts = []
            while rpid and rpid in objs and objs[rpid].type.name == "RectTransform":
                t = tree(rpid)
                parts.append(go_name.get(t["m_GameObject"]["m_PathID"], "?"))
                rpid = t["m_Father"]["m_PathID"]
            return "/".join(reversed(parts))

        for pid, t in texts:
            fd = t["m_FontData"]
            fname = resolve_font(sf, sfname, fd["m_Font"])
            gid = t["m_GameObject"]["m_PathID"]
            rpid = rt_by_go.get(gid)
            if rpid is None:
                continue
            w, h = rect_size(rpid)
            src = t["m_Text"]
            key = f"ui:{name}:{fi}:{pid}:m_Text"
            en = None
            if key in L10N and L10N[key][0] == sha8(src):
                en = L10N[key][1]
            # BaseTextBox shows a text-table row (defaultTextLabel) with front/back strings around it
            sc = scripts(gid)
            btb = sc.get("BaseTextBox") or {}
            tl = btb.get("defaultTextLabel") or ""
            if tl and tl in TEXT_BY_LABEL:
                tkey = TEXT_BY_LABEL[tl]
                if tkey in JA_SRC and tkey in L10N:
                    fs, bs = btb.get("frontString", ""), btb.get("backString", "")
                    src = fs + JA_SRC[tkey] + bs
                    en = fs + L10N[tkey][1] + bs
                    key = tkey
            path = go_path(rpid)
            rows.append(dict(id=f"{name}:{fi}:{pid}", key=key, bundle=name, path=path, font=fname, fd=fd, w=w, h=h, src=src, en=en,
                             enabled=t.get("m_Enabled", 1), driven=driven(gid, rpid), parent_w=parent_w(rpid),
                             runtime=runtime_texts(path), room=room_right(rpid)))

print(len(rows), "UI.Text labels;", sum(1 for r in rows if JP.search(r["src"])), "with Japanese;",
      sum(1 for r in rows if r["en"]), "with our English", file=sys.stderr)


def ja_fit_share(k):
    ok = n = 0
    for r in rows:
        if JP.search(r["src"]) and r["font"] in FONT_FILES and r["w"] > 4 and r["h"] > 4:
            CUR[0] = r["font"]
            n += 1
            ok += best_size(r["src"], r["fd"], r["w"], r["h"], k)[1]
    return ok / max(n, 1), n


if a.calibrate:
    for k in (0.9, 1.0, 1.05, 1.1, 1.15, 1.2, 1.3, 1.5, 2.0):
        print("line factor %.2f: Japanese labels that fit %.1f%% of %d" % ((k,) + tuple(x * (100 if i == 0 else 1) for i, x in enumerate(ja_fit_share(k)))))
    sys.exit()

K = a.line or 1.1
UNITY_W = 0.97  # Unity rounds glyph advances at the scaled render size: wrap checks keep 3% spare


def max_size(fd):
    return fd["m_MaxSize"] if fd["m_BestFit"] else fd["m_FontSize"]


def texts_of(r):
    """Our English for a label: its baked or BaseTextBox text, and the rows it shows at runtime."""
    return sorted({s for s in ([r["en"]] if r["en"] is not None else []) + r["runtime"] if s.strip()})


def one_line(s):
    return "\\n" not in s and "\n" not in s


def natural(s, sz):
    return max(width(l, sz) for l in RICH.sub("", s).replace("\\n", "\n").split("\n"))


def line_fd(fd):
    """FontData as the server ships a lineFit label: wrap and truncate, best fit down to half size
    (or the designer's smaller minimum)."""
    m = max_size(fd)
    half = min(max(8, m // 2), m)
    mn = min(fd["m_MinSize"], half) if fd["m_BestFit"] and fd["m_MinSize"] >= 1 else half
    return dict(fd, m_BestFit=1, m_MinSize=mn, m_MaxSize=m, m_HorizontalOverflow=0, m_VerticalOverflow=0)


def plan(r):
    """Layout fix for labels whose English runs out of a box the Japanese fit on one line:
    lineFit (wrap with vertical overflow in a one-line box, no fitter), lineBox for runtime labels
    sized by a vertical fitter, widthBox (sized by a horizontal fitter, past its frame), and widen
    (left-anchored labels with room up to their right-hand neighbour)."""
    fd = r["fd"]
    if r["font"] not in FONT_FILES or fd["m_FontSize"] <= 0 or r["font"] in BITMAP:
        return None
    CUR[0] = r["font"]
    m = max_size(fd)
    texts = [s for s in texts_of(r) if one_line(s)]
    if not texts or (JP.search(r["src"]) and not one_line(r["src"])):
        return None
    left = fd["m_Alignment"] % 3 == 0
    if (fd["m_HorizontalOverflow"] == 0 and fd["m_VerticalOverflow"] == 1 and not r["driven"] and r["w"] >= 16
            and m * 0.5 <= r["h"] < 2 * m * K and not (JP.search(r["src"]) and len(wrap(r["src"], m, r["w"])) > 1)):
        kind, w, h, f2 = "lineFit", r["w"], max(r["h"], m * 7 / 6), line_fd(fd)
    elif (r["runtime"] and r["driven"] == "fitter-v" and fd["m_HorizontalOverflow"] == 0 and 16 <= r["w"] < 600
          and fd["m_LineSpacing"] >= 1.0 - 1e-6):
        size = fd["m_FontSize"]
        kind, w, h = "lineBox", r["w"], size * 7 / 6
        f2 = dict(fd, m_BestFit=1, m_MinSize=min(max(8, size // 2), size), m_MaxSize=size, m_HorizontalOverflow=0, m_VerticalOverflow=0)
        m = size
    elif r["driven"] == "fitter-h" and r["room"] and left and r["room"] >= 16:
        size = fd["m_FontSize"]
        # never tighter than the room the Japanese took (the neighbour may sit closer than 8 px)
        room = max(r["room"], natural(r["src"], size) + 2 if JP.search(r["src"]) else 0)
        if max(natural(s, size) for s in texts) <= room * UNITY_W:
            return None
        r = dict(r, room=room)
        f2 = dict(fd, m_BestFit=1, m_MinSize=min(max(8, size // 2), size), m_MaxSize=size, m_HorizontalOverflow=0, m_VerticalOverflow=0)
        kind, w, h, m = "widthBox", int(r["room"]), size * 7 / 6, size
    else:
        return None
    long_ = [s for s in texts if natural(s, m) > w * UNITY_W]
    if not long_:
        return None
    widen = None
    if kind != "widthBox" and left and r["room"] and r["room"] > w + 1:
        widen = int(min(r["room"], max(natural(s, m) for s in long_) / UNITY_W + 2))
        w = widen
    worst = min((best_size(s, f2, w * UNITY_W, h, K) for s in long_), key=lambda x: (x[1], x[0]))
    return dict(kind=kind, texts=long_, width=w, widen=widen, size=worst[0], ok=worst[1], design=m)


for r in rows:
    r["fix"] = plan(r)

out = open(a.out, "w")
out.write("key\tbundle\tpath\tfont\tsize\tbestfit\tmin\tmax\thwrap\tvtrunc\tw\th\tja_fits\tja_size\ten_size_now\ten_fits_now\ten_fits_at_half\tproblem\tdriven\tparent_w\tjapanese_baked\tenglish\n")
for r in rows:
    fd = dict(r["fd"])
    CUR[0] = r["font"]
    ja = JP.search(r["src"]) is not None
    jsz, jok = best_size(r["src"], fd, r["w"], r["h"], K) if ja and r["font"] in FONT_FILES else (0, "")
    problem = []
    esz = eok = half = ""
    if r["en"] is not None:
        en = r["en"]
        if r["font"] in BITMAP:
            missing = sorted({c for c in en if not c.isspace()} - BITMAP[r["font"]])
            if missing:
                problem.append("bitmap font lacks " + "".join(missing))
        # what the server ships: best fit down to half size if the label had none
        fd2 = dict(fd)
        if not fd["m_BestFit"] and fd["m_FontSize"] > 0:
            fd2.update(m_BestFit=1, m_MinSize=max(8, fd["m_FontSize"] // 2), m_MaxSize=fd["m_FontSize"])
        esz, eok = best_size(en, fd2, r["w"], r["h"], K)
        half = eok
        if r["driven"] == "fitter-h" and ja and r["font"] in FONT_FILES:
            # grows with its text: compare natural widths (the Japanese fit the layout)
            nat = lambda s: max(width(l, fd["m_FontSize"]) for l in RICH.sub("", s).replace("\\n", "\n").split("\n"))
            wj, we = nat(r["src"]), nat(en)
            room = max(wj * 1.2, wj + 16, r["parent_w"] * 0.85 if r["parent_w"] > wj else 0)
            if we > room:
                problem.append("grows: %.0f px vs Japanese %.0f px, room %.0f px" % (we, wj, room))
        if r["font"] in FONT_FILES and r["driven"] not in ("fitter-h", "fitter-v", "layout"):
            if not eok:
                problem.append("overflows even at min size" + (" (truncated: may show nothing)" if fd["m_VerticalOverflow"] == 0 else ""))
            elif esz < 0.75 * fd["m_FontSize"]:
                problem.append("shrinks to %d%%" % round(100 * esz / fd["m_FontSize"]))
            if fd["m_HorizontalOverflow"] == 1 and r["w"] > 4:
                wd = max(width(l, esz) for l in en.replace("\\n", "\n").split("\n"))
                if wd > r["w"] * 1.15:
                    problem.append("no wrap: %.0f px wide in a %.0f px box" % (wd, r["w"]))
    fx = r["fix"]
    if fx:
        problem.append("%s: %d text(s) wrap at the design size%s; fixed: %d px (%d%%)%s" % (
            fx["kind"], len(fx["texts"]), ", widened to %d px" % fx["widen"] if fx["widen"] else "", fx["size"],
            round(100 * fx["size"] / fx["design"]), "" if fx["ok"] else ", overflows even at min size"))
    out.write("\t".join(str(x) for x in [r["key"], r["bundle"], r["path"], r["font"], fd["m_FontSize"], fd["m_BestFit"], fd["m_MinSize"], fd["m_MaxSize"],
                                          int(fd["m_HorizontalOverflow"] == 0), int(fd["m_VerticalOverflow"] == 0), round(r["w"]), round(r["h"]),
                                          jok, jsz, esz, eok, half, "; ".join(problem), r["driven"], round(r["parent_w"]), int(ja), (r["en"] or "").replace("\t", " ").replace("\n", "\\n")]) + "\n")
print("report:", a.out, file=sys.stderr)

if a.hints:
    best, wrapl = set(), set()
    for r in rows:
        fd = r["fd"]
        if r["font"] not in FONT_FILES or fd["m_FontSize"] <= 0:
            continue
        if fd["m_HorizontalOverflow"] == 0 and fd["m_VerticalOverflow"] == 0 and not fd["m_BestFit"]:
            best.add(r["id"])
        # labels that spill sideways with our English, in a real box: wrap + truncate + best fit
        if (r["en"] is not None and fd["m_HorizontalOverflow"] == 1 and not r["driven"] and r["w"] >= 16
                and r["h"] >= fd["m_FontSize"] * 0.6):
            CUR[0] = r["font"]
            wd = max(width(l, fd["m_FontSize"]) for l in RICH.sub("", r["en"]).replace("\\n", "\n").split("\n"))
            if wd > r["w"] + 1:
                wrapl.add(r["id"])
    # statue name labels sized by a vertical ContentSizeFitter: the fitter grows them by a line for long
    # English names (best fit never runs), and they spill onto the tabs below. The server gives them a
    # fixed box instead (two lines at most) so best fit shrinks the full name.
    names = set()
    for r in rows:
        fd = r["fd"]
        if (r["path"].rsplit("/", 1)[-1].startswith("Txt_StatueName") and r["driven"] == "fitter-v" and fd["m_BestFit"]
                and fd["m_HorizontalOverflow"] == 0 and abs(fd["m_LineSpacing"] - 1.0) < 1e-6 and fd["m_FontSize"] > 0):
            names.add(r["id"])
    # one-line labels sized by a vertical ContentSizeFitter: the Japanese takes one line at the design
    # size, so the fitter keeps them one line tall; longer English wraps and the fitter grows them over
    # what lies above and below. The server gives them a fixed one-line box with best fit. Not: boxes 600 px
# or wider (cautions and descriptions filled at runtime) and tight line spacing (laid out for two lines).
    lines1 = set()
    for r in rows:
        fd = r["fd"]
        if (r["id"] in names or r["driven"] != "fitter-v" or fd["m_HorizontalOverflow"] != 0 or r["font"] not in FONT_FILES
                or fd["m_FontSize"] <= 0 or not 16 <= r["w"] < 600 or fd["m_LineSpacing"] < 1.0 - 1e-6 or not JP.search(r["src"]) or "\\n" in r["src"] or "\n" in r["src"]):
            continue
        CUR[0] = r["font"]
        if len(wrap(r["src"], fd["m_FontSize"], r["w"])) == 1:
            lines1.add(r["id"])
            if a.list_boxes:
                print("lineBox", r["id"], r["path"].rsplit("/", 2)[-2:], r["w"], fd["m_FontSize"], file=sys.stderr)
    # Build 44 (plan()): runtime labels in a vertical fitter join lineBox; lineFit, widthBox and widen.
    linefit, widthbox, widen = set(), {}, {}
    for r in rows:
        fx = r["fix"]
        if not fx or r["id"] in names:
            continue
        if fx["kind"] == "lineBox":
            lines1.add(r["id"])
        elif fx["kind"] == "lineFit":
            linefit.add(r["id"])
        elif fx["kind"] == "widthBox":
            widthbox[r["id"]] = fx["width"]
        if fx["widen"]:
            widen[r["id"]] = fx["widen"]
        if a.list_boxes:
            print(fx["kind"], r["id"], r["path"].rsplit("/", 2)[-2:], fx["width"], fx["size"], file=sys.stderr)
    doc = {"about": "UI.Text layout hints from tools/l10n/ui_fit.py (label ids bundle:file:pathId; no game text). "
                    "bestFit: labels that wrap and truncate get best fit (down to half size); wrap: labels that spill "
                    "sideways with our English get wrap, truncate and best fit; nameBox: statue name labels that a "
                    "ContentSizeFitter grows by a line get a fixed box of up to two lines and best fit; lineBox: one-line "
                    "labels a ContentSizeFitter grows when the English wraps get a fixed one-line box and best fit; "
                    "lineFit: one-line labels whose vertical overflow let the English wrap out of the box get "
                    "truncate, best fit and a box one line tall; widthBox: labels a ContentSizeFitter widens past "
                    "their frame get a fixed box of this width (one line, best fit); widen: left-anchored labels get "
                    "this width (room up to their right-hand neighbour). See docs/LOCALIZATION.md.",
           "bestFit": sorted(best), "wrap": sorted(wrapl), "nameBox": sorted(names), "lineBox": sorted(lines1),
           "lineFit": sorted(linefit), "widthBox": dict(sorted(widthbox.items())), "widen": dict(sorted(widen.items()))}
    json.dump(doc, open(a.hints, "w"), indent=0)
    print("hints:", len(best), "best fit,", len(wrapl), "wrap,", len(names), "name boxes,", len(lines1), "line boxes,",
          len(linefit), "line fits,", len(widthbox), "width boxes,", len(widen), "widened ->", a.hints, file=sys.stderr)
