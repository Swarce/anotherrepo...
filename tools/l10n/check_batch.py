#!/usr/bin/env python3
"""Check a translated batch against its input before import (same rules as `jdr-tool l10n import`).

    python3 -I tools/l10n/check_batch.py --in local/l10n/b23/tables/000.tsv --out local/l10n/b23/out/tables-000.tsv

Checks: same rows in the same order with key, hash and japanese unchanged; every english cell
filled; no Japanese script in english; the same {placeholders}; the source hash; a warning
when the number of \\n line breaks changes. Prints every
problem with its line number and exits 1 if there is any.
"""
import argparse, csv, hashlib, re, sys

ap = argparse.ArgumentParser()
ap.add_argument("--in", dest="inp", required=True)
ap.add_argument("--out", required=True)
a = ap.parse_args()
csv.field_size_limit(sys.maxsize)


def read(p):
    return list(csv.reader(open(p, encoding="utf-8"), delimiter="\t", quoting=csv.QUOTE_NONE))


def unesc(s):
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            n = s[i + 1]
            out.append({"t": "\t", "n": "\n", "r": "\r"}.get(n, n))
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


JP = re.compile("[぀-ヿ㐀-䶿一-鿿ｦ-ﾟ]")
PH = re.compile(r"\{([A-Za-z0-9_]{1,24})\}")


def ph(s):
    return sorted(m.group(0) for m in PH.finditer(s))


src, dst = read(a.inp), read(a.out)
problems, warnings = [], []
if not dst or dst[0][:4] != ["key", "hash", "english", "japanese"]:
    problems.append("line 1: header must be key, hash, english, japanese")
if len(src) != len(dst):
    problems.append(f"{len(dst) - 1} rows, expected {len(src) - 1}")
for i, (s, d) in enumerate(zip(src[1:], dst[1:]), start=2):
    if len(d) < 4:
        problems.append(f"line {i}: {len(d)} columns")
        continue
    if (s[0], s[1], s[3]) != (d[0], d[1], d[3]):
        problems.append(f"line {i}: key/hash/japanese changed (expected {s[0]})")
        continue
    en, jp = unesc(d[2]), unesc(d[3])
    if not en:
        problems.append(f"line {i}: {d[0]}: english is empty")
        continue
    if JP.search(en):
        problems.append(f"line {i}: {d[0]}: Japanese characters in english")
    if ph(en) != ph(jp):
        problems.append(f"line {i}: {d[0]}: placeholders {ph(jp)} -> {ph(en)}")
    if hashlib.sha256(jp.encode("utf-8")).hexdigest()[:8] != d[1]:
        problems.append(f"line {i}: {d[0]}: hash does not match the japanese column")
    if "\t" in d[2] or "\r" in d[2]:
        problems.append(f"line {i}: {d[0]}: raw tab or CR in english")
    if jp.count("\n") != en.count("\n"):
        warnings.append(
            f"line {i}: {d[0]}: {jp.count(chr(10))} line breaks -> {en.count(chr(10))}")
for w in warnings[:50]:
    print("warning:", w)
for p in problems:
    print(p)
print(f"{len(dst) - 1} rows, {len(problems)} problems, {len(warnings)} warnings")
sys.exit(1 if problems else 0)
