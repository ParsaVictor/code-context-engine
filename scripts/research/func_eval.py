"""Function-level Acc@k for the engine's `definitions` list (LocAgent's
function-level metric: every edited function in the top k).

Gold = the innermost function or method (as `Class.method` or `func`) that
contains a removed line, or the insertion point of an added line, of the
reference patch, in the file at `base_commit` (Python `ast`). Instances whose
patch edits no existing function have no function gold and are skipped (that is
how LocAgent arrives at 274 of Lite's 300).

    py -3 scripts/research/func_eval.py --results run.jsonl --data rows.json --repos <clones> [--subset ids.json]
"""
import argparse
import ast
import json
import os
import subprocess
import sys

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


def gold_functions(clone, commit, patch):
    gold = set()
    for path, lines in touched_old_lines(patch).items():
        src = subprocess.run(["git", "-C", clone, "show", f"{commit}:{path}"], capture_output=True,
                             text=True, encoding="utf-8", errors="ignore").stdout
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
    res = [r for r in res if "definitions" in r and (keep is None or r["instance_id"] in keep)]
    ks = (1, 5, 10, 20)
    hits = {k: 0 for k in ks}
    n = 0
    for r in res:
        row = rows[r["instance_id"]]
        clone = os.path.join(args.repos, row["repo"].split("/")[1])
        gold = gold_functions(clone, row["base_commit"], row["patch"])
        if not gold:
            continue
        n += 1
        preds = [(d["path"], d["name"]) for d in r["definitions"]]
        for k in ks:
            top = preds[:k]
            hits[k] += all(any(same(p, g) for p in top) for g in gold)
    print(f"instances with function gold: {n}")
    print("  ".join(f"func Acc@{k} {hits[k] / max(n, 1):.3f}" for k in ks))


if __name__ == "__main__":
    main()
