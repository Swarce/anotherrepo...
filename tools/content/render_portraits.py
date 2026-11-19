#!/usr/bin/env python3
"""Portraits (the 128x128 `st_<statueId>` thumbnails) for roster statues the player's files have
none for, made from the player's own files (docs/research/28-portraits.md):

  1. kit: the real thumbnails have the frame, stars, attribute badge and assist mark drawn in.
     Each layer is the per-pixel mode of the thumbnails that share it (the mean of those in the
     winning colour bin, which averages out their texture compression): frame, bottom bar and
     gold stars from the thumbnails of that rarity with no empty star slots (the server sends
     rareMax = rare); the face opening is what they do not share, connected to the middle; the
     badge from the attribute's thumbnails, inside a circle fitted to its firm core; the assist
     mark from the assist-only ones. The background is a diamond lattice in the attribute's
     colour: its geometry is fixed (LATTICE, measured with --fit-lattice), its colour field is
     fitted per attribute.
  2. faces: the statue's model in its menu pose (posing.py, the figure clip), rendered unlit with an
     outline by render_portraits.js, framed on the HEAD bone like the real thumbnails (FRAMING,
     calibrated against them).
  3. compose: background, face, frame, badge corner, stars, assist mark, outline alpha.

Inputs: decrypted bundles (statue_*, animation_statue_common_*) and the decrypted thumbnail
bundles (`st_*` from ui_statue_00/01, each decrypted with `jdr-tool decrypt`). Output: one PNG per
statue, private (game-derived art stays out of the repository).

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


def mode_layer(G, step=20):
    """Per pixel: the most common colour (quantised, transparent apart; the mean of the thumbnails
    in that bin, which also averages out their compression noise) and its share."""
    q = (G[..., :3] // step).astype(int)
    key = (q[..., 0] * 1000 + q[..., 1] * 31 + q[..., 2]) + (G[..., 3] < 128) * 10 ** 7
    out, agree = np.zeros((128, 128, 4)), np.zeros((128, 128))
    for y in range(128):
        for x in range(128):
            k = key[:, y, x]
            u, c = np.unique(k, return_counts=True)
            j = c.argmax()
            out[y, x] = G[k == u[j], y, x].mean(0)
            agree[y, x] = c[j] / len(k)
    return out, agree


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


def build_kit(names, X, statues):
    st = {d["statueId"]: d for d in statues}
    roster = [i for i, n in enumerate(names) if n in st]
    # rarity from the frame: nearest mean border of each wiki rarity (the wiki is wrong for a few)
    ring = (np.minimum(np.minimum(xx, 127 - xx), np.minimum(yy, 127 - yy)) < 7) & ~((xx > 88) & (yy > 88))
    F = X[:, ring][:, :, :3].reshape(len(X), -1)
    cent = {r: F[[i for i in roster if st[names[i]].get("rarity") == r]].mean(0) for r in (3, 4, 5, 6)}
    rar = {names[i]: min(cent, key=lambda r: ((F[i] - cent[r]) ** 2).mean()) for i in roster}
    assist = {names[i]: bool(st[names[i]].get("assistOnly")) for i in roster}
    stype = {names[i]: st[names[i]].get("statueType") for i in roster}
    idx_of = {n: i for i, n in enumerate(names)}

    def group(f):
        return [idx_of[n] for n in rar if f(n)]

    def donor(idx, mode, mask):
        errs = [np.abs(X[i][mask][:, :3] - mode[mask][:, :3]).mean() for i in idx]
        j = int(np.argmin(errs))
        return idx[j]

    kit = {"rarity_by_frame": rar}
    T = {t: mode_layer(X[group(lambda n, t=t: stype[n] == t)]) for t in range(1, 6)}
    A = mode_layer(X[group(lambda n: assist[n])])
    # the star strip: six stars (gold, or dark for an upgrade not reached) on a black backing,
    # centred at x = STAR_X0 + k * STAR_DX
    band = (yy >= 99) & (yy <= 121) & (xx >= 8) & (xx <= 90)
    layers = {}
    for r in (3, 4, 5, 6):
        # Thumbnails of this rarity with no empty star slots (statues at their top rarity, as the
        # server sends every statue): their frame, bottom bar and gold stars are the same pixels
        # in all of them. Empty slots are grey and neutral; look for them past the last gold star.
        cut = STAR_X0 + STAR_DX * (r - 1) + STAR_DX / 2
        past = band & (xx > cut) & (yy >= 102) & (yy <= 114)

        def empties(i):
            D = X[i]
            mx, mn = D[..., :3].max(-1), D[..., :3].min(-1)
            return ((mx > 25) & (mx < 110) & (mx - mn < 18))[past].mean() if past.any() else 0.0
        idx = group(lambda n, r=r: rar[n] == r and not assist[n])   # no assist mark
        clean = [i for i in idx if empties(i) < 0.12] or idx
        c, a = mode_layer(X[clean], step=28)
        same = (a >= 0.55) & (c[..., 3] > 128)
        # the badge corner: frame only where thumbnails of every attribute agree (each group
        # leans to some attributes, whose badges would otherwise leave slivers)
        zone = (xx > 74) & (yy > 74)
        mixed = [i for i in idx if (stype[names[i]] or 0) > 0]
        by_t = {t: [i for i in mixed if stype[names[i]] == t] for t in range(1, 6)}
        pick = [i for t in range(1, 6) for i in by_t[t][:4]]     # an even mix of attributes
        if pick:
            _, a_mix = mode_layer(X[pick], step=28)
            same &= ~zone | (a_mix >= 0.7)
        # the opening: what they do not share, connected to the middle; pinholes closed
        win = flood(~dil(same, 5), (64, 50))        # gaps in the ring sealed for the fill
        win = fill_holes(dil(win, 5) & ~same)       # grown back to the frame's own edge
        chrome = ~win & (c[..., 3] > 128)
        frame = np.zeros((128, 128, 4))
        frame[chrome] = c[chrome]
        frame[chrome, 3] = 255
        alpha = win | chrome
        layers[r] = {"frame": frame, "alpha": alpha}
        print(f"rarity {r}: frame from {len(clean)} of {len(idx)} thumbnails (no empty slots)", file=sys.stderr)
    badges = {}
    for t in range(1, 6):
        c, a = T[t]
        # the badge alone: what its attribute group agrees on firmly (the frame corner under it
        # differs by rarity), holes filled, not grown
        core = (a >= 0.7) & (xx >= 89) & (yy > 74) & (c[..., 3] > 128)
        core = fill_holes(ero(dil(core, 3), 3))
        cy, cx = np.argwhere(core).mean(0)
        rad = np.sqrt(((np.argwhere(core) - [cy, cx]) ** 2).sum(1)).max()
        # the round badge with its rim: a circle a little past the core, opaque pixels only
        m = ((yy - cy) ** 2 + (xx - cx) ** 2 <= (rad + 2.5) ** 2) & (c[..., 3] > 128)
        m &= a >= 0.5        # inside the circle, what is not the badge is the frame beneath
        b = np.zeros((128, 128, 4))
        b[m] = c[m]          # averaged over the attribute's thumbnails
        b[m, 3] = 255
        badges[t] = b
    c, a = A
    m = (a > 0.5) & (xx > 88) & (yy > 58) & (yy < 100) & (c[..., 3] > 128)
    m = fill_holes(ero(dil(m, 3), 3)) & (xx > 88) & (yy > 58) & (yy < 96)
    D = X[donor(group(lambda n: assist[n]), c, m)]
    amark = np.zeros((128, 128, 4))
    amark[m] = np.where((a[m] >= 0.5)[:, None], c[m], D[m])
    amark[m, 3] = 255
    # background colour field per attribute, from pixels three or more thumbnails agree on
    Dl = lattice_dist()
    inner = (xx > 17) & (xx < 111) & (yy > 17) & (yy < 96)
    bgs = {}
    for t in range(1, 6):
        G = X[group(lambda n, t=t: stype[n] == t)][..., :3]
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
    kit.update(layers=layers, badges=badges, amark=amark, bgs=bgs)
    return kit


def compose(kit, face, r, t, asst):
    L = kit["layers"][r]
    img = Image.fromarray(np.dstack([kit["bgs"][t], np.full((128, 128), 255)]).astype(np.uint8))
    img.alpha_composite(face)

    def put(a):
        img.alpha_composite(Image.fromarray(a.clip(0, 255).astype(np.uint8)))
    put(L["frame"])              # frame, bottom bar and gold stars
    put(kit["badges"][t])
    if asst:
        put(kit["amark"])
    shape = L["alpha"] | (kit["badges"][t][..., 3] > 0)
    if asst:
        shape |= kit["amark"][..., 3] > 0        # the mark reaches past the frame's outline
    out = np.asarray(img).copy()
    out[..., 3] = np.where(shape, out[..., 3], 0)
    return Image.fromarray(out)


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
        compose(kit, face, r, t, asst).save(os.path.join(a.out, f"st_{sid}.png"))
        report.append(f"{sid}\trarity {r}\ttype {t}\t{'assist' if asst else ''}\t{note}")
    open(os.path.join(a.out, "report.tsv"), "w").write("\n".join(report) + "\n")
    print(f"{sum(1 for r in report if 'EMPTY' not in r)} portraits in {a.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
