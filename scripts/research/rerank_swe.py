"""Cross-encoder rerank of the engine's localisation list on SWE-bench issues.

Reads a harness result file (needs `loc_files`), checks out each instance,
reranks the top N candidates with the issue text (boilerplate stripped
the engine's way: comments, links, checklists, headings) and writes the new
order as `rr` (pure reranker) and `rrb` (50/50 blend with first-stage rank).

    py -3 scripts/research/rerank_swe.py --results run.jsonl --data rows.json --repos <clones>
        --work <dir> --out rr.jsonl [--model jina-reranker-v2] [--top 10]
"""
import argparse
import json
import os
import re
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.dirname(__file__))
from swebench_localize import checkout, remove_worktrees  # noqa: E402
from rerank_test import Reranker, digest  # noqa: E402


def strip_boilerplate(text):
    text = re.sub(r"(?s)<!--.*?-->", " ", text)
    text = re.sub(r"https?://\S+", " ", text)
    out, fenced = [], False
    for line in text.splitlines():
        t = line.lstrip()
        if t.startswith("```"):
            fenced = not fenced
            out.append(line)
            continue
        if re.match(r"[-*+] \[[ xX]\]", t) or (not fenced and re.match(r"#+ ", t)):
            continue
        out.append(line)
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--model", default="jina-reranker-v2")
    ap.add_argument("--top", type=int, default=10)
    ap.add_argument("--subset", default="")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    keep = None
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if "loc_files" in r and (keep is None or r["instance_id"] in keep)]
    rr = Reranker(args.model)
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8")}
    try:
        with open(args.out, "a", encoding="utf-8") as out:
            for r in res:
                if r["instance_id"] in done:
                    continue
                row = rows[r["instance_id"]]
                name = row["repo"].split("/")[1]
                dest = os.path.join(args.work, name)
                checkout(os.path.join(args.repos, name), row["base_commit"], dest)
                cands = r["loc_files"][: args.top]
                q = strip_boilerplate(row["problem_statement"])
                t0 = time.time()
                s = rr.scores(q, [digest(dest, c) for c in cands])
                secs = time.time() - t0
                lo, hi = min(s), max(s)
                span = (hi - lo) or 1.0
                order = [c for _, c in sorted(zip(s, cands), key=lambda x: -x[0])]
                blend = {c: 0.5 * (si - lo) / span + 0.5 / (1 + i) for i, (c, si) in enumerate(zip(cands, s))}
                rec = {"instance_id": r["instance_id"], "repo": r["repo"], "gold": r["gold"],
                       "loc": r["loc_files"], "rr": order + r["loc_files"][args.top:],
                       "rrb": sorted(cands, key=lambda c: -blend[c]) + r["loc_files"][args.top:],
                       "rr_secs": round(secs, 2)}
                out.write(json.dumps(rec) + "\n")
                out.flush()
    finally:
        remove_worktrees(args.repos, args.work)


if __name__ == "__main__":
    main()
