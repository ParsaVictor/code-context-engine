"""Function-level Acc@k of each `NM_DIAG` definition variant (and RRF fusions
of two of them) per half of SWE-bench dev.

    py -3 scripts/research/diag_eval.py --results dev-s18b-all.jsonl --data devset/swe-dev.json
        --repos repos --gold-cache func-gold-dev.json [--fuse definitions+no_stem]
"""
import argparse
import json
import os
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(__file__))
from func_eval import load_gold, same  # noqa: E402


def lists_of(r):
    out = {"definitions": [(d["path"], d["name"]) for d in r.get("definitions", [])]}
    for k, v in (r.get("diag_definitions") or {}).items():
        out[k] = [(d["path"], d["name"]) for d in v]
    return out


def rrf(a, b, k=60):
    s = defaultdict(float)
    for lst in (a, b):
        for i, d in enumerate(lst):
            s[d] += 1 / (k + i + 1)
    return [d for d, _ in sorted(s.items(), key=lambda kv: -kv[1])]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--gold-cache", default="")
    ap.add_argument("--fuse", action="append", default=[])
    ap.add_argument("--halves", default="devset/dev-fast.json")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if "error" not in r]
    golds = load_gold(rows, [r["instance_id"] for r in res], args.repos, args.gold_cache)
    fast = {r["instance_id"] for r in json.load(open(args.halves, encoding="utf-8"))}
    for half, sel in (("dev-fast", lambda i: i in fast), ("dev-rest", lambda i: i not in fast),
                      ("all", lambda i: True)):
        rs = [r for r in res if sel(r["instance_id"]) and golds.get(r["instance_id"])]
        if not rs:
            continue
        variants = defaultdict(list)
        for r in rs:
            ls = lists_of(r)
            for f in args.fuse:
                a, b = f.split("+")
                if a in ls and b in ls:
                    ls[f] = rrf(ls[a], ls[b])
            for name, lst in ls.items():
                variants[name].append((lst, golds[r["instance_id"]]))
        print(f"-- {half}: {len(rs)} with function gold")
        for name, items in variants.items():
            cells = []
            for k in (1, 5, 10):
                hit = sum(all(any(same(p, g) for p in lst[:k]) for g in gold) for lst, gold in items)
                cells.append(f"@{k} {hit / len(items):.3f}")
            print(f"   {name:24s} " + "  ".join(cells))


if __name__ == "__main__":
    main()
