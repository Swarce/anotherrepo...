#!/usr/bin/env python3
"""Check (and optionally fix) that recurring terms are rendered one way across translated batches.

    python3 -I tools/l10n/term_check.py --terms local/l10n/b23/terms.tsv local/l10n/b23/out/*.tsv [--fix]

`terms.tsv`: japanese<TAB>canonical English<TAB>variant|variant|... (one term per line; '#' comments).
For every row whose Japanese contains the term, a variant found in the English is reported; with
--fix it is replaced by the canonical form (whole-word, case-sensitive) in place. Rows that hold
neither the canonical form nor a variant are listed as "check" (the term may be paraphrased).
Contains game text only in its inputs and outputs (keep them under local/).
"""
import argparse, csv, re, sys

ap = argparse.ArgumentParser()
ap.add_argument("--terms", required=True)
ap.add_argument("files", nargs="+")
ap.add_argument("--fix", action="store_true")
ap.add_argument("--show", type=int, default=5)
a = ap.parse_args()
csv.field_size_limit(sys.maxsize)

terms = []
for line in open(a.terms, encoding="utf-8"):
    if not line.strip() or line.startswith("#"):
        continue
    jp, canon, variants = (line.rstrip("\n").split("\t") + ["", ""])[:3]
    terms.append((jp, canon, [v for v in variants.split("|") if v]))

for f in a.files:
    rows = list(csv.reader(open(f, encoding="utf-8"), delimiter="\t", quoting=csv.QUOTE_NONE))
    changed = 0
    for jp, canon, variants in terms:
        hits, check = [], []
        for r in rows[1:]:
            if len(r) < 4 or jp not in r[3]:
                continue
            en = r[2]
            bad = [v for v in variants if re.search(r"(?<![A-Za-z])" + re.escape(v) + r"(?![A-Za-z])", en)]
            if bad:
                hits.append((r[0], bad, en))
                if a.fix:
                    for v in bad:
                        en = re.sub(r"(?<![A-Za-z])" + re.escape(v) + r"(?![A-Za-z])", canon, en)
                    r[2] = en
                    changed += 1
            elif canon.lower() not in en.lower():
                check.append((r[0], en))
        if hits or check:
            print(f"{f.split('/')[-1]}: {jp} -> {canon}: {len(hits)} variant rows, {len(check)} to check")
            for k, bad, en in hits[: a.show]:
                print(f"   variant {bad}: {k}: {en[:110]}")
            for k, en in check[: a.show]:
                print(f"   check: {k}: {en[:110]}")
    if a.fix and changed:
        with open(f, "w", encoding="utf-8") as out:
            for r in rows:
                out.write("\t".join(r) + "\n")
        print(f"{f}: {changed} rows fixed")
