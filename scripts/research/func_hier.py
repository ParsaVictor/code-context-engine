"""Function-level orderings built from the file list (Agentless localises files
first, then functions inside them). Offline over a harness run made with
NM_DEFINITIONS=300 (definitions + localization stored per instance).

  global      definitions as ranked (no file prior)
  file-major  best files first (loc order), each file's definitions by score
  interleave  round-robin: best definition of file 1, of file 2, ... then seconds
  rrf         definition rank fused with its file's rank (k = 60)

    py -3 scripts/research/func_hier.py --results run.jsonl --data rows.json --repos <clones> [--files 5]
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from func_eval import gold_functions, same  # noqa: E402


def orderings(r, nfiles):
    defs = [(d["path"], d["name"]) for d in r["definitions"]]
    loc = r["loc_files"][:nfiles]
    by_file = {f: [d for d in defs if d[0] == f] for f in loc}
    file_major = [d for f in loc for d in by_file[f]]
    inter = []
    depth = max((len(v) for v in by_file.values()), default=0)
    for i in range(depth):
        for f in loc:
            if i < len(by_file[f]):
                inter.append(by_file[f][i])
    frank = {f: i for i, f in enumerate(r["loc_files"])}
    scored = []
    for i, d in enumerate(defs):
        s = 1 / (60 + i + 1)
        if d[0] in frank:
            s += 1 / (60 + frank[d[0]] + 1)
        scored.append((s, i, d))
    rrf = [d for _, _, d in sorted(scored, key=lambda x: (-x[0], x[1]))]
    return {"global": defs, "file-major": file_major, "interleave": inter, "rrf": rrf}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--files", type=int, default=5)
    ap.add_argument("--subset", default="")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    keep = None
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if r.get("definitions") and r.get("loc_files")
           and (keep is None or r["instance_id"] in keep)]
    ks = (1, 5, 10)
    score = {}
    n = 0
    for r in res:
        row = rows[r["instance_id"]]
        gold = gold_functions(os.path.join(args.repos, row["repo"].split("/")[1]),
                              row["base_commit"], row["patch"])
        if not gold:
            continue
        n += 1
        for name, lst in orderings(r, args.files).items():
            s = score.setdefault(name, {k: 0 for k in ks})
            for k in ks:
                s[k] += all(any(same(p, g) for p in lst[:k]) for g in gold)
    print(f"instances with function gold: {n}")
    for name, s in score.items():
        print(f"{name:11s}" + "  ".join(f"@{k} {v / max(n, 1):.3f}" for k, v in s.items()))


if __name__ == "__main__":
    main()
