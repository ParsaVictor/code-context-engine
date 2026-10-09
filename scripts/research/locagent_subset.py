"""Rebuild LocAgent's SWE-bench Lite subset: drop instances whose reference patch
modifies no existing function (LocAgent §5.1, following Suresh et al. 2024).

A patch touches an existing function when a removed line, or the insertion
point of an added line, lies inside a function/method span of the file at
`base_commit` (Python `ast`).

    py -3 scripts/research/locagent_subset.py lite.json repos out.json
"""
import ast
import json
import re
import subprocess
import sys


def spans(src):
    try:
        tree = ast.parse(src)
    except (SyntaxError, ValueError):
        return []
    return [(n.lineno, n.end_lineno) for n in ast.walk(tree)
            if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))]


def touched_old_lines(patch):
    out = {}
    cur = None
    for line in patch.splitlines():
        m = re.match(r"^diff --git a/(\S+)", line)
        if m:
            cur = m.group(1)
            out.setdefault(cur, set())
            continue
        h = re.match(r"^@@ -(\d+)(?:,\d+)? \+\d+(?:,\d+)? @@", line)
        if h:
            old = int(h.group(1))
            continue
        if cur is None or line.startswith(("---", "+++")):
            continue
        if line.startswith("-"):
            out[cur].add(old)
            old += 1
        elif line.startswith("+"):
            out[cur].add(max(old - 1, 1))
        elif line.startswith(" "):
            old += 1
    return out


def main():
    data, repos, dest = sys.argv[1:4]
    rows = json.load(open(data, encoding="utf-8"))
    keep = []
    for r in rows:
        clone = f"{repos}/{r['repo'].split('/')[1]}"
        hit = False
        for path, lines in touched_old_lines(r["patch"]).items():
            src = subprocess.run(["git", "-C", clone, "show", f"{r['base_commit']}:{path}"],
                                 capture_output=True, text=True, encoding="utf-8", errors="ignore").stdout
            sp = spans(src)
            if any(a <= l <= b for l in lines for a, b in sp):
                hit = True
                break
        if hit:
            keep.append({"instance_id": r["instance_id"]})
    json.dump(keep, open(dest, "w", encoding="utf-8"))
    print(len(keep), "of", len(rows))


if __name__ == "__main__":
    main()
