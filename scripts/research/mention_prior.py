"""Prototype: files a report names (traceback frames, paths, unique file names) as
a third vote in the localisation list.

On SWE-bench dev, 50 single-file issues name the gold file in the text, yet only
28 had it first. A traceback's deepest repository frame, or a file named in
prose, is the report's own pointer. This scores the engine's stored lists
(`loc_files`) fused with that pointer list, offline: the repository tree comes
from `git ls-tree` at the base commit (no checkout needed).

    py -3 scripts/research/mention_prior.py --results run.jsonl --data rows.json --repos <clones>
        [--weight 1.0] [--subset ids.json]
"""
import argparse
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from eval_loc import acc  # noqa: E402

PATH = re.compile(r"[\w.\\/-]+\.py\b")
FRAME = re.compile(r'File "([^"]+\.py)", line \d+')


def tree(clone, commit):
    r = subprocess.run(["git", "-C", clone, "ls-tree", "-r", "--name-only", commit],
                       capture_output=True, text=True)
    return [f for f in r.stdout.splitlines() if f.endswith(".py")]


def resolve(token, files, by_name):
    t = token.replace("\\", "/").lstrip("./")
    parts = t.split("/")
    for i in range(len(parts)):
        suffix = "/".join(parts[i:])
        hits = [f for f in files if f == suffix or f.endswith("/" + suffix)]
        if len(hits) == 1:
            return hits[0]
        if hits and i < len(parts) - 1:
            continue
        if len(hits) > 1:
            return None
    cands = by_name.get(parts[-1], [])
    return cands[0] if len(cands) == 1 else None


def mention_list(text, files):
    by_name = {}
    for f in files:
        if "/test" in "/" + f or f.startswith("test"):
            continue
        by_name.setdefault(f.rsplit("/", 1)[-1], []).append(f)
    out = []
    frames = FRAME.findall(text)
    for tok in reversed(frames):  # deepest frame first
        f = resolve(tok, files, by_name)
        if f and f not in out:
            out.append(f)
    for tok in PATH.findall(text):
        f = resolve(tok, files, by_name)
        if f and f not in out:
            out.append(f)
    return out


def rrf(lists, weights, k=60):
    s = {}
    for lst, w in zip(lists, weights):
        for i, f in enumerate(lst):
            s[f] = s.get(f, 0.0) + w / (k + i + 1)
    return [f for f, _ in sorted(s.items(), key=lambda kv: (-kv[1], kv[0]))]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--weights", default="0.5,1,2")
    ap.add_argument("--subset", default="")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    keep = None
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if r.get("loc_files") and (keep is None or r["instance_id"] in keep)]
    weights = [float(w) for w in args.weights.split(",")]
    base = {k: 0 for k in (1, 3, 5, 10)}
    fused = {w: {k: 0 for k in (1, 3, 5, 10)} for w in weights}
    named = 0
    for r in res:
        row = rows[r["instance_id"]]
        files = tree(os.path.join(args.repos, row["repo"].split("/")[1]), row["base_commit"])
        m = mention_list(row["problem_statement"], files)
        named += bool(m)
        for k in base:
            base[k] += acc(r["loc_files"], r["gold"], k)
        for w in weights:
            lst = rrf([r["loc_files"], m], [1.0, w])
            for k in base:
                fused[w][k] += acc(lst, r["gold"], k)
    n = len(res)
    print(f"instances {n}, naming at least one repository file: {named}")
    print("loc        " + "  ".join(f"@{k} {v / n:.3f}" for k, v in base.items()))
    for w in weights:
        print(f"+mention {w:<3}" + "  ".join(f"@{k} {v / n:.3f}" for k, v in fused[w].items()))


if __name__ == "__main__":
    main()
