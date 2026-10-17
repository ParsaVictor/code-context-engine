"""Function Acc@k per half for several dense short-list runs side by side
(engine, engine + vote at w, dense order alone), and seconds per issue.
Session 19: jina-code-v2 vs SweRankEmbed-Small.

    py -3 dense_compare.py --results dev-s18a-all.jsonl --gold func-gold-dev.json
        --halves devset/dev-fast.json,devset/dev-rest.json
        --run jina=dense-sl-dev.jsonl --run swerank=dense-sw-dev.jsonl [--w 0.5,1]
"""
import argparse
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(__file__))
from paired import hit, load, rrf  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--gold", required=True)
    ap.add_argument("--halves", required=True)
    ap.add_argument("--run", action="append", required=True)
    ap.add_argument("--w", default="0.5,1")
    a = ap.parse_args()
    res = load(a.results)
    gold = {i: {tuple(x) for x in g} for i, g in json.load(open(a.gold, encoding="utf-8")).items() if g}
    runs = {name: load(p) for name, p in (s.split("=", 1) for s in a.run)}
    ws = [float(w) for w in a.w.split(",")]
    lists = {"engine": {}}
    for iid, r in res.items():
        lists["engine"][iid] = [(d["path"], d["name"]) for d in r.get("definitions", [])]
    for name, ext in runs.items():
        for w in ws:
            lists[f"{name} w={w:g}"] = {}
        lists[f"{name} alone"] = {}
        for iid, base in lists["engine"].items():
            dense = [tuple(d) for d in ext.get(iid, {}).get("dense_defs", [])]
            for w in ws:
                lists[f"{name} w={w:g}"][iid] = rrf([base, dense], [1.0, w]) if dense else base
            lists[f"{name} alone"][iid] = dense or base
    halves = [(os.path.basename(h).replace(".json", ""), {r["instance_id"] for r in json.load(open(h, encoding="utf-8"))})
              for h in a.halves.split(",")]
    halves.append(("all", set(res)))
    for hname, ids in halves:
        ids = sorted(i for i in ids if i in res and i in gold)
        print(f"-- {hname}: {len(ids)} issues with function gold")
        base5 = sum(hit(lists["engine"][i], gold[i], 5) for i in ids) / len(ids)
        for v, lst in lists.items():
            acc = {k: sum(hit(lst[i], gold[i], k) for i in ids) / len(ids) for k in (1, 5, 10)}
            print(f"   {v:16s} func @1 {acc[1]:.3f}  @5 {acc[5]:.3f}  @10 {acc[10]:.3f}   d@5 {acc[5] - base5:+.3f}")
    for name, ext in runs.items():
        secs = [r["secs"] for r in ext.values() if r["instance_id"] in res]
        print(f"secs/issue {name}: n={len(secs)} p50 {statistics.median(secs):.1f}  mean {statistics.mean(secs):.1f}"
              f"  p90 {sorted(secs)[int(0.9 * len(secs))]:.1f}")


if __name__ == "__main__":
    main()
