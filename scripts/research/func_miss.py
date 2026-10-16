"""Where do function-level misses come from? (session 19, analysis only — dev split)

For every instance with function gold: rank of each gold function in the engine's
`definitions` list (or absent), rank of its file in `ours_files`, list length.
Buckets: hit@5 / gold present but ranked >5 (ordering) / gold file ranked but
function absent from the list (coverage) / file not ranked (file miss).

    py -3 func_miss.py --results dev-s18a-all.jsonl --data devset/swe-dev.json --repos repos \
        --gold-cache func-gold-dev.json
"""
import argparse
import json
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(__file__))
from func_eval import load_gold, same  # noqa: E402


def file_rank(files, gp):
    for i, f in enumerate(files):
        if f == gp or f.endswith("/" + gp) or gp.endswith("/" + f):
            return i
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--gold-cache", default="")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if "error" not in r]
    golds = load_gold(rows, [r["instance_id"] for r in res], args.repos, args.gold_cache)
    buckets, lens, ranks, ngold = Counter(), [], [], Counter()
    for r in res:
        gold = golds[r["instance_id"]]
        if not gold:
            continue
        ngold[min(len(gold), 3)] += 1
        preds = [(d["path"], d["name"]) for d in r.get("definitions", [])]
        lens.append(len(preds))
        worst = "hit@5"
        for g in gold:
            rk = next((i for i, p in enumerate(preds) if same(p, g)), None)
            fr = file_rank(r.get("ours_files", []), g[0])
            if rk is not None and rk < 5:
                b = "hit@5"
            elif rk is not None:
                b = "ordering"
                ranks.append(rk)
            elif fr is not None:
                b = "coverage(file ranked %s)" % ("top1" if fr == 0 else "top5" if fr < 5 else ">5")
            else:
                b = "file-miss"
            order = ["hit@5", "ordering", "coverage(file ranked top1)", "coverage(file ranked top5)",
                     "coverage(file ranked >5)", "file-miss"]
            if order.index(b) > order.index(worst):
                worst = b
        buckets[worst] += 1
    n = sum(buckets.values())
    print(f"instances {n}; gold size 1/2/3+: {dict(sorted(ngold.items()))}")
    for b, c in buckets.most_common():
        print(f"  {b:32s} {c:4d}  {c / n:.3f}")
    lens.sort()
    print(f"list length median {lens[len(lens) // 2]}  min {lens[0]}  max {lens[-1]}")
    if ranks:
        ranks.sort()
        print(f"ordering misses: rank median {ranks[len(ranks) // 2]}, <=10 {sum(x < 10 for x in ranks)}, "
              f"<=20 {sum(x < 20 for x in ranks)}, n {len(ranks)}")


if __name__ == "__main__":
    main()
