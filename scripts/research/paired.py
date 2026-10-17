"""Paired comparison of two function lists per instance (exact sign test on
discordant pairs + paired bootstrap CI of the difference), pooling several
(results, ext, gold) sets. Session 19: dense short-list vote vs engine.

    py -3 paired.py --set dev-s18a-all.jsonl:dense-sl-dev.jsonl:func-gold-dev.json \
        --set gym-v140-all.jsonl:dense-sl-gym.jsonl:func-gold-gym.json [--w 0.5] [--k 5]

A fourth field (`results:ext:gold:ext0`) compares against the engine fused with
`ext0` at the same weight instead of the bare engine (vote vs vote).
"""
import argparse
import json
import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
from func_eval import same  # noqa: E402


def rrf(lists, weights, k=60):
    s = {}
    for lst, w in zip(lists, weights):
        for i, f in enumerate(lst):
            s[f] = s.get(f, 0.0) + w / (k + i + 1)
    return [f for f, _ in sorted(s.items(), key=lambda kv: -kv[1])]


def hit(preds, gold, k):
    return int(all(any(same(p, g) for p in preds[:k]) for g in gold))


def load(p):
    return {r["instance_id"]: r for r in map(json.loads, filter(str.strip, open(p, encoding="utf-8")))
            if "error" not in r}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--set", action="append", required=True)
    ap.add_argument("--w", type=float, default=0.5)
    ap.add_argument("--k", type=int, default=5)
    a = ap.parse_args()
    pairs = []
    for spec in a.set:
        rp, ep, gp, *e0 = spec.split(":")
        res, ext, gold = load(rp), load(ep), json.load(open(gp, encoding="utf-8"))
        ext0 = load(e0[0]) if e0 else None
        for iid, r in res.items():
            g = {tuple(x) for x in gold.get(iid, [])}
            if not g:
                continue
            base = [(d["path"], d["name"]) for d in r.get("definitions", [])]
            dense = [tuple(d) for d in ext.get(iid, {}).get("dense_defs", [])]
            new = rrf([base, dense], [1.0, a.w]) if dense else base
            if ext0 is not None:
                d0 = [tuple(d) for d in ext0.get(iid, {}).get("dense_defs", [])]
                base = rrf([base, d0], [1.0, a.w]) if d0 else base
            pairs.append((hit(base, g, a.k), hit(new, g, a.k)))
    n = len(pairs)
    b = sum(x for x, _ in pairs) / n
    c = sum(y for _, y in pairs) / n
    up = sum(1 for x, y in pairs if y > x)
    down = sum(1 for x, y in pairs if x > y)
    m = up + down
    p = min(1.0, 2 * sum(math.comb(m, i) for i in range(min(up, down) + 1)) / 2 ** m) if m else 1.0
    rnd = random.Random(0)
    diffs = sorted(sum(y - x for x, y in (rnd.choice(pairs) for _ in range(n))) / n for _ in range(4000))
    print(f"n={n}  func@{a.k} {b:.3f} -> {c:.3f}  (+{c - b:.3f}, 95% CI [{diffs[100]:+.3f},{diffs[3899]:+.3f}])"
          f"  gained {up} lost {down}  sign-test p={p:.4f}")


if __name__ == "__main__":
    main()
