#!/usr/bin/env python3
"""Split a `jdr-tool l10n export` TSV into translation batches of unique source strings.

Each Japanese string is listed once (the first key that has it); `jdr-tool l10n import
--expand-with <export> [--replace]` gives every other row with the same source the same English.
Rows are grouped by text table, UI bundle or story scene, so a translator sees related strings
together; a story scene is never split across batches.

    python3 -I tools/l10n/make_batches.py --in local/l10n/work.tsv --out local/l10n/b23/in \
        [--chars 24000] [--only story|tables]

Writes <out>/NNN.tsv (key, hash, english, japanese; `english` holds the current translation as a
reference) and <out>/index.tsv (batch, rows, chars, groups). Everything written contains game
text: keep it under local/.
"""
import argparse, csv, os, re, sys

ap = argparse.ArgumentParser()
ap.add_argument("--in", dest="inp", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--chars", type=int, default=24000, help="Japanese characters per batch (about)")
ap.add_argument("--only", choices=["story", "tables"])
a = ap.parse_args()

csv.field_size_limit(sys.maxsize)
rows = list(csv.reader(open(a.inp, encoding="utf-8"), delimiter="\t", quoting=csv.QUOTE_NONE))
head, rows = rows[0], rows[1:]


def group(key):
    kind, rest = key.split(":", 1)
    if kind == "text":
        return "text:" + rest.split(":", 1)[0]
    if kind == "script":
        return "script:" + rest.split(":", 1)[0]
    if kind == "ui":
        return "ui:" + rest.split(":", 1)[0]
    return kind


def line_no(key):
    m = re.search(r":(\d+)$", key)
    return int(m.group(1)) if m else 0


seen = set()
groups = {}
for r in rows:
    if len(r) < 4 or not r[3]:
        continue
    story = r[0].startswith("script:")
    if (a.only == "story" and not story) or (a.only == "tables" and story):
        continue
    if r[3] in seen:
        continue
    seen.add(r[3])
    groups.setdefault(group(r[0]), []).append(r[:4])

os.makedirs(a.out, exist_ok=True)
batches, cur, cur_chars, cur_groups = [], [], 0, []
for g in sorted(groups):
    items = groups[g]
    if g.startswith("script:"):
        items.sort(key=lambda r: line_no(r[0]))
    size = sum(len(r[3]) for r in items)
    # Large tables are split on their own; small groups share a batch.
    if cur and cur_chars + size > a.chars:
        batches.append((cur, cur_chars, cur_groups))
        cur, cur_chars, cur_groups = [], 0, []
    if size > a.chars and not g.startswith("script:"):
        chunk, n = [], 0
        for r in items:
            chunk.append(r)
            n += len(r[3])
            if n >= a.chars:
                batches.append((chunk, n, [g]))
                chunk, n = [], 0
        if chunk:
            cur, cur_chars, cur_groups = chunk, n, [g]
        continue
    cur += items
    cur_chars += size
    cur_groups.append(g)
if cur:
    batches.append((cur, cur_chars, cur_groups))

with open(os.path.join(a.out, "index.tsv"), "w", encoding="utf-8") as ix:
    ix.write("batch\trows\tchars\tgroups\n")
    for i, (items, n, gs) in enumerate(batches):
        name = f"{i:03d}.tsv"
        with open(os.path.join(a.out, name), "w", encoding="utf-8") as f:
            f.write("key\thash\tenglish\tjapanese\n")
            for r in items:
                f.write("\t".join(r) + "\n")
        ix.write(f"{name}\t{len(items)}\t{n}\t{','.join(gs[:6])}{',...' if len(gs) > 6 else ''}\n")
print(len(batches), "batches,", sum(len(b[0]) for b in batches), "unique strings")
