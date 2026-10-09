"""Function-level Acc@k for the engine's `definitions` list (LocAgent's
function-level metric: every edited function in the top k).

Gold = the innermost function or method (as `Class.method` or `func`) that
contains a removed line, or the insertion point of an added line, of the
reference patch, in the file at `base_commit` (Python `ast`). Instances whose
patch edits no existing function have no function gold and are skipped (that is
how LocAgent arrives at 274 of Lite's 300).

    py -3 scripts/research/func_eval.py --results run.jsonl --data rows.json --repos <clones> [--subset ids.json]
        [--ci] [--gold-cache gold.json] [--key definitions]

`--gold-cache` stores the function gold per instance (computing it runs `git show`
per patched file); `--key` scores another list field of `{path, name}` entries.
An instance whose result has no list (the engine emits `definitions` only for
reports of 60+ words) counts as a miss; `--skip-missing` drops it instead, which
is how numbers before session 18 were computed (a smaller, easier denominator).
"""
import argparse
import ast
import json
import os
import random
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from locagent_subset import touched_old_lines  # noqa: E402


def defs_with_names(src):
    out = []
    try:
        tree = ast.parse(src)
    except (SyntaxError, ValueError):
        return out

    def walk(node, stack):
        for child in ast.iter_child_nodes(node):
            if isinstance(child, ast.ClassDef):
                walk(child, stack + [child.name])
            elif isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                out.append((child.lineno, child.end_lineno, ".".join(stack + [child.name])))
                walk(child, stack + [child.name])
            else:
                walk(child, stack)

    walk(tree, [])
    return out


def git_show(clone, commit, path, tries=4):
    """File at `commit`, or "" when the patch creates it. Blobless clones fetch the
    blob over the network here; a failed fetch must not read as "no function gold"
    (the instance would silently leave the denominator), so failures retry, then raise."""
    for attempt in range(tries):
        p = subprocess.run(["git", "-C", clone, "show", f"{commit}:{path}"], capture_output=True,
                           text=True, encoding="utf-8", errors="ignore")
        if p.returncode == 0:
            return p.stdout
        if "does not exist in" in p.stderr or "exists on disk, but not in" in p.stderr:
            return ""
        time.sleep(2 * (attempt + 1))
    raise RuntimeError(f"git show {commit}:{path} failed in {clone}: {p.stderr.strip()[:200]}")


def gold_functions(clone, commit, patch):
    gold = set()
    for path, lines in touched_old_lines(patch).items():
        src = git_show(clone, commit, path)
        defs = defs_with_names(src)
        for line in lines:
            inside = [d for d in defs if d[0] <= line <= d[1]]
            if inside:
                a, b, name = min(inside, key=lambda d: d[1] - d[0])
                gold.add((path, name))
    return gold


def same(pred, gold):
    (pp, pn), (gp, gn) = pred, gold
    if not (pp == gp or pp.endswith("/" + gp) or gp.endswith("/" + pp)):
        return False
    return pn == gn or pn.split(".")[-1] == gn.split(".")[-1] and (
        "." not in pn or "." not in gn or pn == gn)


def boot_ci(vals, n=2000, seed=0):
    rnd = random.Random(seed)
    means = sorted(sum(rnd.choice(vals) for _ in vals) / len(vals) for _ in range(n))
    return means[int(0.025 * n)], means[int(0.975 * n) - 1]


def load_gold(rows, ids, repos, cache_path):
    cache = {}
    if cache_path and os.path.exists(cache_path):
        cache = json.load(open(cache_path, encoding="utf-8"))
    missing = [i for i in ids if i not in cache]
    for iid in missing:
        row = rows[iid]
        clone = os.path.join(repos, row["repo"].split("/")[1])
        cache[iid] = sorted(gold_functions(clone, row["base_commit"], row["patch"]))
    if cache_path and missing:
        json.dump(cache, open(cache_path, "w", encoding="utf-8"))
    return {i: {tuple(g) for g in cache[i]} for i in ids}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--subset", default="")
    ap.add_argument("--ci", action="store_true")
    ap.add_argument("--gold-cache", default="")
    ap.add_argument("--key", default="definitions")
    ap.add_argument("--skip-missing", action="store_true")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    keep = None
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if "error" not in r and (keep is None or r["instance_id"] in keep)]
    if args.skip_missing:
        res = [r for r in res if args.key in r]
    golds = load_gold(rows, [r["instance_id"] for r in res], args.repos, args.gold_cache)
    ks = (1, 5, 10, 20)
    hits = {k: [] for k in ks}
    for r in res:
        gold = golds[r["instance_id"]]
        if not gold:
            continue
        preds = [(d["path"], d["name"]) for d in r.get(args.key, [])]
        for k in ks:
            top = preds[:k]
            hits[k].append(int(all(any(same(p, g) for p in top) for g in gold)))
    n = len(hits[1])
    print(f"instances with function gold: {n}")
    cells = []
    for k in ks:
        cell = f"func Acc@{k} {sum(hits[k]) / max(n, 1):.3f}"
        if args.ci and n:
            lo, hi = boot_ci(hits[k])
            cell += f" [{lo:.2f},{hi:.2f}]"
        cells.append(cell)
    print("  ".join(cells))


if __name__ == "__main__":
    main()
