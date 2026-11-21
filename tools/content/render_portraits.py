#!/usr/bin/env python3
"""Portraits (the 128x128 `st_<statueId>` thumbnails) for roster statues the player's files have
none for, made from the player's own files (docs/research/28-portraits.md):

  1. frame: each portrait keeps one real roster thumbnail's frame, stars, badge and assist mark,
     pixel for pixel, and only its inside is replaced. The donor has the same frame rarity, badge
     attribute and mark, and no empty star slots where one exists. A donor pixel is kept when it
     is art its group shares: at least 60% of the group (frame rarity; rarity without empty
     slots; attribute; assist-only) equal the group median there, within TOL. The inside is what
     is not kept, connected to the middle, cut back to the frame's inner edge. The background is
     a diamond lattice in the attribute's colour: its geometry is fixed (LATTICE, measured with
     --fit-lattice), its colour field is fitted per attribute.
  2. faces: the statue's model in its menu pose (posing.py, the figure clip), rendered unlit with an
     outline by render_portraits.js, framed on the HEAD bone like the real thumbnails (FRAMING,
     calibrated against them).
  3. compose: background and face in the opening, the donor's pixels everywhere else.

Inputs: decrypted bundles (statue_*, animation_statue_common_*) and the decrypted thumbnail
bundles (`st_*` from ui_statue_00/01, each decrypted with `jdr-tool decrypt`). Output: one PNG per
statue and `portraits.zip` (`replacements/ui_statue_01/thumbs/st_<id>.png`), an art pack for
`jdr-patch patch ... --art portraits.zip`; private (game-derived art stays out of the repository).

usage: python3 -I tools/content/render_portraits.py --dec DIR --thumbs DIR --statues server/content/statues.json \
         --three three.min.js --out local/portraits [--only ID,...] [--fit-lattice]
"""
import argparse, base64, io, json, os, subprocess, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import posing  # noqa: E402

# Diamond lattice of the thumbnail background: cell width, height, x and y offset (px). Measured
# on the player's thumbnails (--fit-lattice; three thumbnails of two attributes agree to 0.2 px).
LATTICE = (18.8, 20.5, 6.75, 14.75)
# Face framing: a 20-degree camera 0.95 m in front of the HEAD bone, aimed 2.4 cm above it and
# 0.5 cm to the side (median of the real thumbnails matched to renders of the same statues).
FRAMING = {"fov": 20, "dist": 0.95, "dy": 0.024, "dx": -0.005, "outline": 0.0015, "size": 512}

# The thumbnail bundle new portraits are added to (the smaller of ui_statue_00/01).
PACK_BUNDLE = "ui_statue_01"
# Star centres along the strip (px): x = STAR_X0 + k * STAR_DX, measured on a 6-star thumbnail.
STAR_X0, STAR_DX = 17.1, 12.65

yy, xx = np.mgrid[:128, :128]


def statue_rarity(d):
    """As the server sends it (collection.rs: wiki rarity, else 4)."""
    return d.get("rarity") or 4


def statue_type(d):
    """As the server sends it (collection.rs: wiki statueType, else 1 + psid mod 5)."""
    return d.get("statueType") or 1 + int(d["statueId"][:4]) % 5


# ---------- masks
def mimg(m):
    return Image.fromarray((m * 255).astype(np.uint8))


def marr(im):
    return np.asarray(im) > 127


def dil(m, k):
    return marr(mimg(m).filter(ImageFilter.MaxFilter(k)))


def ero(m, k):
    return marr(mimg(m).filter(ImageFilter.MinFilter(k)))


def flood(m, seed):
    im = mimg(m).convert("L")
    ImageDraw.floodfill(im, seed, 128)
    return np.asarray(im) == 128


def fill_holes(m):
    edge = np.pad(np.zeros((126, 126), bool), 1, constant_values=True)
    return ~flood(~m | edge, (0, 0))


# ---------- kit
def load_thumbs(d):
    import UnityPy
    names, imgs = [], []
    for f in sorted(os.listdir(d)):
        env = UnityPy.load(os.path.join(d, f))
        for o in env.objects:
            if o.type.name == "Texture2D":
                t = o.read()
                names.append(t.m_Name[3:] if t.m_Name.startswith("st_") else t.m_Name)
                imgs.append(np.asarray(t.image.convert("RGBA"), float))
                break
    return names, np.stack(imgs)


def lattice_dist(lat=LATTICE):
    W, H, OX, OY = lat
    u = (xx - OX) / W + (yy - OY) / H
    v = (xx - OX) / W - (yy - OY) / H
    k = np.sqrt((1 / W) ** 2 + (1 / H) ** 2)
    return np.minimum(np.abs(u - np.round(u)), np.abs(v - np.round(v))) / k


def basis(x, y):
    x, y = (x - 64) / 64, (y - 64) / 64
    return np.stack([np.ones_like(x), x, y, x * x, y * y, x * y], -1)


def fit_lattice(img):
    """Fit LATTICE on one thumbnail's visible background (high-passed, against thin dark lines)."""
    I = Image.fromarray(img[..., :3].astype(np.uint8))
    im, bl = img[..., :3], np.asarray(I.filter(ImageFilter.GaussianBlur(3)), float)
    m = np.zeros((128, 128), bool)
    m[18:72, 18:40] = m[18:55, 92:110] = m[18:30, 18:110] = True
    m &= (np.abs(im - bl).mean(-1) < 40) & (im.max(-1) - im.min(-1) > 30)
    r = (im - bl).mean(-1)[m]
    best = (-1,)
    for W in np.arange(14, 24, 0.5):
        for H in np.arange(15, 27, 0.5):
            for ox in range(int(W)):
                for oy in range(int(H)):
                    L = np.exp(-lattice_dist((W, H, ox, oy))[m] ** 2 / 0.8)
                    L -= L.mean()
                    s = -(L * r).sum() / (np.sqrt((L * L).sum() * (r * r).sum()) + 1e-9)
                    if s > best[0]:
                        best = (s, W, H, ox, oy)
    return best


# Two pixels are "the same" within this much per channel (texture compression noise).
TOL = 24


def same_as(img, ref):
    return (np.abs(img[..., :3] - ref[..., :3]).max(-1) <= TOL) & ((img[..., 3] >= 128) == (ref[..., 3] >= 128))


def fill_small_holes(m, most=40):
    """Fill holes in `m` of at most `most` pixels (compression specks), keeping larger openings."""
    holes = fill_holes(m) & ~m
    out = m.copy()
    seen = np.zeros_like(m)
    for y, x in zip(*np.nonzero(holes)):
        if seen[y, x]:
            continue
        comp = flood(holes, (int(x), int(y))) & holes
        seen |= comp
        if comp.sum() <= most:
            out |= comp
    return out


def build_kit(names, X, statues):
    """Groups of the real roster thumbnails by frame rarity, badge attribute and assist mark, with
    the per-pixel medians that tell their shared pixels (frame, stars, badge, mark) from the face."""
    st = {d["statueId"]: d for d in statues}
    roster = [i for i, n in enumerate(names) if n in st]
    # rarity from the frame: nearest mean border of each wiki rarity (the wiki is wrong for a few)
    ring = (np.minimum(np.minimum(xx, 127 - xx), np.minimum(yy, 127 - yy)) < 7) & ~((xx > 88) & (yy > 88))
    F = X[:, ring][:, :, :3].reshape(len(X), -1)
    cent = {r: F[[i for i in roster if st[names[i]].get("rarity") == r]].mean(0) for r in (3, 4, 5, 6)}
    rar = {i: min(cent, key=lambda r: ((F[i] - cent[r]) ** 2).mean()) for i in roster}
    # attribute from the badge: nearest mean badge centre of the statues whose attribute is known
    # (right for all of them, leave-one-out)
    core = ((yy - 108) ** 2 + (xx - 110) ** 2) <= 81
    B = X[:, core][:, :, :3].reshape(len(X), -1)
    lab = [(i, st[names[i]]["statueType"]) for i in roster if st[names[i]].get("statueType")]
    bc = {t: B[[i for i, tt in lab if tt == t]].mean(0) for t in range(1, 6)}
    typ = {i: min(bc, key=lambda t: ((B[i] - bc[t]) ** 2).mean()) for i in roster}
    assist = {i: bool(st[names[i]].get("assistOnly")) for i in roster}
    band = (yy >= 99) & (yy <= 121) & (xx >= 8) & (xx <= 90)

    def empties(i):
        # empty star slots (grey, neutral) past the last gold star
        r = rar[i]
        past = band & (xx > STAR_X0 + STAR_DX * (r - 1) + STAR_DX / 2) & (yy >= 102) & (yy <= 114)
        D = X[i]
        mx, mn = D[..., :3].max(-1), D[..., :3].min(-1)
        return ((mx > 25) & (mx < 110) & (mx - mn < 18))[past].mean() if past.any() else 0.0
    clean = {i: empties(i) < 0.12 for i in roster}

    def med(idx):
        """(median, shared): the per-pixel median of a group and where it is shared art, i.e.
        where at least 60% of the group equal the median (a face never is)."""
        if not idx:
            return None
        m = np.median(X[idx], 0)
        share = np.mean([same_as(X[i], m) for i in idx], 0)
        return m, share >= 0.6
    M = {
        "rarity": {r: med([i for i in roster if rar[i] == r]) for r in (3, 4, 5, 6)},
        "clean": {r: med([i for i in roster if rar[i] == r and clean[i]]) for r in (3, 4, 5, 6)},
        "type": {t: med([i for i in roster if typ[i] == t]) for t in range(1, 6)},
        "assist": med([i for i in roster if assist[i]]),
    }
    return {"names": names, "X": X, "roster": roster, "rar": rar, "typ": typ, "assist": assist,
            "clean": clean, "M": M, "rarity_by_frame": {names[i]: r for i, r in rar.items()}}


def pick_donor(kit, r, t, asst):
    """The real thumbnail to take the frame from: same frame rarity, same badge, the same assist
    mark, no empty star slots, in that order of preference (a regular portrait never takes an
    assist-only donor: it would bring the mark). Returns (frame donor, badge donor, mark donor)."""
    R, X = kit["roster"], kit["X"]
    rar, typ, ast, clean, M = kit["rar"], kit["typ"], kit["assist"], kit["clean"], kit["M"]
    ringm = (np.minimum(np.minimum(xx, 127 - xx), np.minimum(yy, 127 - yy)) < 12)

    def best(c):
        c = list(c)
        if not c:
            return None
        ref = M["rarity"][r]
        return min(c, key=lambda i: np.abs(X[i][ringm][:, :3] - ref[0][ringm][:, :3]).mean())
    ok_mark = (lambda i: ast[i] == asst) if not asst else (lambda i: True)
    frame = (best(i for i in R if rar[i] == r and typ[i] == t and ast[i] == asst and clean[i])
             or best(i for i in R if rar[i] == r and typ[i] == t and ok_mark(i) and clean[i])
             or best(i for i in R if rar[i] == r and ok_mark(i) and clean[i] and not ast[i])
             or best(i for i in R if rar[i] == r and not ast[i]))
    badge = frame if typ[frame] == t else (
        best(i for i in R if rar[i] == r and typ[i] == t) or best(i for i in R if typ[i] == t))
    mark = None
    if asst and not ast[frame]:
        mark = best(i for i in R if ast[i] and rar[i] == r) or best(i for i in R if ast[i])
    return frame, badge, mark


def keep_mask(kit, D, r, t, with_mark):
    """The donor's pixels that are the shared art (frame, gold stars, badge, mark): those equal,
    within the compression tolerance, to the median of a group that shares that art."""
    M = kit["M"]

    def ok(g, zone=True):
        if g is None:
            return np.zeros((128, 128), bool)
        m, shared = g
        return zone & shared & same_as(D, m)
    k = ok(M["rarity"][r]) | ok(M["clean"][r])
    k |= ok(M["type"][t], (xx > 80) & (yy > 80))
    if with_mark:
        k |= ok(M["assist"], (xx > 86) & (yy > 56) & (yy < 100))
    # outside the thumbnail's outline: transparent where the group is
    return k


def compose(kit, face, bg, r, t, asst):
    """A real thumbnail with everything inside its frame cut away, down to the frame's inner edge;
    the background and the face go in the opening. Frame, stars, badge and mark are the donor's
    own pixels, unchanged."""
    X = kit["X"]
    fd, bd, md = pick_donor(kit, r, t, asst)
    D = X[fd].copy()
    keep = keep_mask(kit, D, r, t, asst and kit["assist"][fd])
    corner = (xx > 80) & (yy > 80)
    if bd != fd:
        # another attribute's badge: take this badge from a thumbnail that has it
        Bd = X[bd]
        tm, tsh = kit["M"]["type"][t]
        bk = corner & tsh & same_as(Bd, tm)                  # the badge as its attribute has it
        om, osh = kit["M"]["type"][kit["typ"][fd]]
        keep &= ~(corner & osh & same_as(D, om))             # the donor's own badge goes
        D[bk] = Bd[bk]
        keep |= bk
    if md is not None:
        Md = X[md]
        am, ash = kit["M"]["assist"]
        rm, rsh = kit["M"]["rarity"][kit["rar"][md]]
        mk = (xx > 86) & (yy > 56) & (yy < 100) & ash & same_as(Md, am) & ~(rsh & same_as(Md, rm))
        D[mk] = Md[mk]
        keep |= mk
    keep = fill_small_holes(keep)
    # the opening: what is not shared art, connected to the middle (gaps in the frame sealed for
    # the fill, then grown back to its own edge)
    win = flood(~dil(keep, 5), (64, 50))
    win = dil(win, 5) & ~keep
    win = fill_small_holes(win, 20)      # stray donor pixels left inside the opening go too
    img = np.zeros((128, 128, 4))
    inside = Image.fromarray(np.dstack([bg, np.full((128, 128), 255)]).astype(np.uint8))
    inside.alpha_composite(face)
    ins = np.asarray(inside, float)
    img[win] = ins[win]
    img[~win] = D[~win]
    return Image.fromarray(img.clip(0, 255).astype(np.uint8)), (kit["names"][fd], kit["names"][bd],
                                                                kit["names"][md] if md is not None else "")


def make_backgrounds(kit):
    """Background per attribute: a colour field fitted to the pixels three or more thumbnails agree
    on, with the diamond lattice drawn over it."""
    X, R, typ = kit["X"], kit["roster"], kit["typ"]
    Dl = lattice_dist()
    inner = (xx > 17) & (xx < 111) & (yy > 17) & (yy < 96)
    bgs = {}
    for t in range(1, 6):
        G = X[[i for i in R if typ[i] == t]][..., :3]
        q = (G // 10).astype(int)
        key = q[..., 0] * 10000 + q[..., 1] * 100 + q[..., 2]
        bg, cnt = np.zeros((128, 128, 3)), np.zeros((128, 128))
        for y in range(128):
            for x in range(128):
                k = key[:, y, x]
                u, cc = np.unique(k, return_counts=True)
                j = cc.argmax()
                cnt[y, x] = cc[j]
                bg[y, x] = G[k == u[j], y, x].mean(0)
        off = (cnt >= 3) & inner & (Dl > 2.0)
        fields = [np.linalg.lstsq(basis(xx[off], yy[off]), bg[..., ch][off], rcond=None)[0] for ch in range(3)]
        base = np.stack([basis(xx, yy) @ f for f in fields], -1)
        bgs[t] = (base * (1 - 0.30 * np.exp(-(Dl / 0.75) ** 2))[..., None]
                  + 10 * np.exp(-((Dl - 1.4) / 0.5) ** 2)[..., None]).clip(0, 255)
    return bgs


# ---------- faces
def figure_clip(bundles, model, sid, rarity):
    """The statue's figure clip; else the base clip of its rarity; else any figure clip."""
    want = [f"{model}_{posing.clip_name(sid, rarity)}", f"{model}_{posing.POSE[rarity]}"]
    env = bundles.get("animation_statue_common_" + model[:4])
    have = sorted(o.peek_name() for o in env.objects if o.type.name == "AnimationClip") if env else []
    for w in want:
        if w in have:
            return w
    figs = [h for h in have if h.startswith(model + "_figure_")]
    return figs[0] if figs else want[0]


def export_statue(bundles, d, path):
    sid, r = d["statueId"], statue_rarity(d)
    parts, heads, note = [], [], []
    for mi, model in enumerate(d["modelDataList"]):
        mdl = posing.load_model(bundles, model, textures=True)
        if mdl is None:
            continue
        clip = figure_clip(bundles, model, sid, r)
        ok = posing.apply_clip(bundles, mdl, clip)
        note.append(f"{model}:{clip.split(model + '_')[-1] if ok else 'unposed'}")
        h = posing.bone_matrix(mdl, "HEAD")
        heads.append(h.tolist() if h is not None else None)
        for p in posing.skinned_parts(mdl):
            png = None
            if p["tex"]:
                buf = io.BytesIO()
                p["tex"][1].save(buf, "PNG")
                png = base64.b64encode(buf.getvalue()).decode()
            parts.append({"model": mi, "png": png,
                          "pos": base64.b64encode(p["verts"].astype("<f4").tobytes()).decode(),
                          "uv": base64.b64encode(p["uv"].astype("<f4").tobytes()).decode(),
                          "idx": base64.b64encode(p["tris"].astype("<u4").tobytes()).decode()})
    if not heads or heads[0] is None:
        return None
    json.dump({"sid": sid, "head": heads[0], "heads": heads, "parts": parts}, open(path, "w"))
    return " ".join(note)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dec", required=True)
    ap.add_argument("--thumbs", required=True)
    ap.add_argument("--statues", required=True)
    ap.add_argument("--three", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--only")
    ap.add_argument("--fit-lattice", action="store_true")
    a = ap.parse_args()
    statues = json.load(open(a.statues))["statues"]
    names, X = load_thumbs(a.thumbs)
    print(f"{len(names)} thumbnails", file=sys.stderr)
    if a.fit_lattice:
        for n in names[:3]:
            print(n, "corr %.2f W %.2f H %.2f ox %.2f oy %.2f" % fit_lattice(X[names.index(n)]))
        return
    have = set(names)
    targets = [d for d in statues if d["statueId"] not in have]
    if a.only:
        only = set(a.only.split(","))
        targets = [d for d in statues if d["statueId"] in only]
    os.makedirs(os.path.join(a.out, "work"), exist_ok=True)
    kit = build_kit(names, X, statues)
    bgs = make_backgrounds(kit)
    bundles = posing.Bundles(a.dec)
    jobs, rows = [], []
    for d in targets:
        sid = d["statueId"]
        j = os.path.join(a.out, "work", sid + ".json")
        note = export_statue(bundles, d, j)
        if note is None:
            print(sid, "no model", file=sys.stderr)
            continue
        jobs.append({"in": j, "out": os.path.join(a.out, "work", sid + "_face.png"), "opt": dict(FRAMING, onlyModel=0)})
        rows.append((d, note))
    jp = os.path.join(a.out, "work", "jobs.json")
    json.dump(jobs, open(jp, "w"))
    subprocess.run(["node", os.path.join(os.path.dirname(os.path.abspath(__file__)), "render_portraits.js"), a.three, jp], check=True)
    # a character the pose hides (Vanilla Ice inside Cream): frame the Stand's head instead
    redo = []
    for job, (d, note) in zip(jobs, rows):
        face = Image.open(job["out"]).convert("RGBA")
        data = json.load(open(job["in"]))
        if np.asarray(face)[..., 3].mean() < 5 and len(data["heads"]) > 1 and data["heads"][1]:
            data["head"] = data["heads"][1]
            json.dump(data, open(job["in"], "w"))
            redo.append(dict(job, opt=dict(job["opt"], onlyModel=1)))
            rows[rows.index((d, note))] = (d, note + " (Stand framed)")
    if redo:
        json.dump(redo, open(jp, "w"))
        subprocess.run(["node", os.path.join(os.path.dirname(os.path.abspath(__file__)), "render_portraits.js"), a.three, jp], check=True)
    report = []
    for d, note in rows:
        sid = d["statueId"]
        r, t, asst = statue_rarity(d), statue_type(d), bool(d.get("assistOnly"))
        face = Image.open(os.path.join(a.out, "work", sid + "_face.png")).convert("RGBA").resize((128, 128), Image.LANCZOS)
        if np.asarray(face)[..., 3].mean() < 20:
            report.append(f"{sid}\tEMPTY\t{note}")
            print(sid, "empty render", note, file=sys.stderr)
            continue
        img, donors = compose(kit, face, bgs[t], r, t, asst)
        img.save(os.path.join(a.out, f"st_{sid}.png"))
        report.append(f"{sid}\trarity {r}\ttype {t}\t{'assist' if asst else ''}\tframe/badge/mark from "
                      f"{'/'.join(donors)}\t{note}")
    open(os.path.join(a.out, "report.tsv"), "w").write("\n".join(report) + "\n")
    # the art pack: jdr-patch ... --art portraits.zip adds them to ui_statue_01 on the phone
    import zipfile
    with zipfile.ZipFile(os.path.join(a.out, "portraits.zip"), "w", zipfile.ZIP_STORED) as z:
        for f in sorted(os.listdir(a.out)):
            if f.startswith("st_") and f.endswith(".png"):
                z.write(os.path.join(a.out, f), f"replacements/{PACK_BUNDLE}/thumbs/{f}")
    print(f"{sum(1 for r in report if 'EMPTY' not in r)} portraits in {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
