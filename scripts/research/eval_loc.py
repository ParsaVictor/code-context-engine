"""File-level Acc@k (all gold files in the top k, as LocAgent reports) for ranked
lists stored in result jsonl files, on the instances every given run covers.

    py -3 scripts/research/eval_loc.py RUN.jsonl:key[,key...] [RUN2.jsonl:key ...]
        [--subset rows.json] [--single-gold] [--k 1,3,5,10] [--fuse a+b]

A key names a list field (`loc_files`, `ours_files`, `chunk`, `file`, ...) or a
precomputed hit field prefix (`bm25_hit`, `ours_hit`, `loc_hit`). `--fuse`
adds reciprocal-rank fusion of two list keys (k = 60).
"""
import argparse
import json
import random
from collections import defaultdict


def load(path):
    out = {}
    for line in open(path, encoding="utf-8"):
        if line.strip():
            r = json.loads(line)
            if "error" not in r:
                out[r["instance_id"]] = r
    return out


def norm(f):
    return f.replace("\\", "/")


def acc(lst, gold, k):
    top = [norm(f) for f in lst[:k]]
    return int(all(any(t.endswith(g) or g.endswith(t) for t in top) for g in gold))


def rrf(a, b, k=60):
    s = defaultdict(float)
    for lst in (a, b):
        for i, f in enumerate(lst):
            s[norm(f)] += 1 / (k + i + 1)
    return [f for f, _ in sorted(s.items(), key=lambda kv: (-kv[1], kv[0]))]


def boot_ci(vals, n=2000, seed=0):
    rnd = random.Random(seed)
    m = len(vals)
    means = sorted(sum(rnd.choice(vals) for _ in range(m)) / m for _ in range(n))
    return means[int(0.025 * n)], means[int(0.975 * n)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("runs", nargs="+")
    ap.add_argument("--subset", default="")
    ap.add_argument("--single-gold", action="store_true")
    ap.add_argument("--k", default="1,3,5,10")
    ap.add_argument("--fuse", action="append", default=[])
    ap.add_argument("--ci", action="store_true")
    args = ap.parse_args()
    ks = [int(x) for x in args.k.split(",")]
    series = []  # (label, {iid: list or hits})
    for spec in args.runs:
        path, keys = spec.rsplit(":", 1)
        data = load(path)
        for key in keys.split(","):
            series.append((f"{path.split('/')[-1]}:{key}", key, data))
    ids = None
    for _, key, data in series:
        have = {i for i, r in data.items() if isinstance(r.get(key), list) or f"{key}@1" in r}
        ids = have if ids is None else ids & have
    if args.subset:
        ids &= {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
    any_data = series[0][2]
    if args.single_gold:
        ids = {i for i in ids if len(any_data[i]["gold"]) == 1}
    ids = sorted(ids)
    print(f"instances: {len(ids)}")
    rows = []
    for label, key, data in series:
        vals = {}
        for k in ks:
            if ids and isinstance(data[ids[0]].get(key), list):
                v = [acc(data[i][key], data[i]["gold"], k) for i in ids]
            else:
                v = [data[i].get(f"{key}@{k}", 0) for i in ids]
            vals[k] = v
        rows.append((label, vals))
    for fuse in args.fuse:
        a, b = fuse.split("+")
        la = next(s for s in series if s[1] == a)
        lb = next(s for s in series if s[1] == b)
        vals = {k: [acc(rrf(la[2][i][a], lb[2][i][b]), la[2][i]["gold"], k) for i in ids] for k in ks}
        rows.append((f"rrf({a},{b})", vals))
    for label, vals in rows:
        cells = []
        for k in ks:
            v = vals[k]
            m = sum(v) / max(len(v), 1)
            if args.ci and v:
                lo, hi = boot_ci(v)
                cells.append(f"@{k} {m:.3f} [{lo:.2f},{hi:.2f}]")
            else:
                cells.append(f"@{k} {m:.3f}")
        print(f"{label:40s} " + "  ".join(cells))


if __name__ == "__main__":
    main()
