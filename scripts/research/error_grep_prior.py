"""Prototype: the file that raises the error a report quotes.

Agents (SWE-agent, OpenHands) grep the repository for an error message before
anything else. Here: every `SomethingError: message` / `Warning: message` line
of the report gives a literal (the message's first words, quotes and object
reprs cut off); files whose source contains it at the base commit (`git grep`
on the commit, no checkout) form a list, fewest matches first; it joins the
named-file list and the engine's list by RRF.

    py -3 scripts/research/error_grep_prior.py --results run.jsonl --data rows.json --repos <clones>
"""
import argparse
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from eval_loc import acc  # noqa: E402
from mention_prior import mention_list, rrf, tree  # noqa: E402

ERR = re.compile(r"^\s*(?:\w+\.)*(\w*(?:Error|Exception|Warning))\s*:\s*(.+)$", re.M)


def literals(text):
    out = []
    for _, msg in ERR.findall(text):
        msg = re.split(r"['\"<{(\[]|\d", msg.strip())[0].strip()
        words = msg.split()
        if len(words) >= 3:
            lit = " ".join(words[:6])
            if lit not in out:
                out.append(lit)
    return out


def grep(clone, commit, lit):
    r = subprocess.run(["git", "-C", clone, "grep", "-l", "-F", "--", lit, commit, "--", "*.py"],
                       capture_output=True, text=True, encoding="utf-8", errors="ignore")
    files = [l.split(":", 1)[1] for l in r.stdout.splitlines() if ":" in l]
    return [f for f in files if "/test" not in "/" + f and not f.startswith("test")]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--subset", default="")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    keep = None
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if r.get("loc_files") and (keep is None or r["instance_id"] in keep)]
    ks = (1, 3, 5, 10)
    score = {name: {k: 0 for k in ks} for name in ("loc", "+named", "+named+error", "+error")}
    with_lit = 0
    for r in res:
        row = rows[r["instance_id"]]
        clone = os.path.join(args.repos, row["repo"].split("/")[1])
        text = row["problem_statement"]
        named = mention_list(text, tree(clone, row["base_commit"]))
        errs = []
        for lit in literals(text):
            hits = grep(clone, row["base_commit"], lit)
            if 0 < len(hits) <= 5:
                errs.extend(h for h in sorted(hits, key=len) if h not in errs)
        with_lit += bool(errs)
        lists = {
            "loc": r["loc_files"],
            "+named": rrf([r["loc_files"], named], [1, 2]),
            "+named+error": rrf([rrf([r["loc_files"], named], [1, 2]), errs], [1, 2]),
            "+error": rrf([r["loc_files"], errs], [1, 2]),
        }
        for name, lst in lists.items():
            for k in ks:
                score[name][k] += acc(lst, r["gold"], k)
    n = len(res)
    print(f"instances {n}, with a grep-able error message: {with_lit}")
    for name, s in score.items():
        print(f"{name:14s}" + "  ".join(f"@{k} {v / n:.3f}" for k, v in s.items()))


if __name__ == "__main__":
    main()
