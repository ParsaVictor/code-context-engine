"""LLM file choice over the engine's candidate list (Agentless-style, one call).

Agentless asks a model to pick the files to edit from the repository tree; LocAgent
lets an agent walk a code graph. Here the engine has already cut the repository
to a short list (`loc_files` from `swebench_localize.py`, recall@10 ~0.8 on Lite),
so one call over that list with a compact skeleton of each file — class and
function signatures, nesting kept — asks the model only what lexical ranking
cannot do: read the report and judge.

    py -3 scripts/research/llm_localize.py --results run.jsonl --data rows.json --repos <clones>
        --work <short dir> --out llm.jsonl [--endpoint http://127.0.0.1:8089/v1] [--top 15]

Any OpenAI-compatible endpoint works (llama.cpp `llama-server`, a hosted API).
Writes `llm` (model's order, then the rest of the candidates) plus token usage
and latency per issue, so Acc@k and cost per issue can be reported side by side.
"""
import argparse
import json
import os
import re
import sys
import time
import urllib.request

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.dirname(__file__))
from swebench_localize import checkout, remove_worktrees  # noqa: E402
from rerank_swe import strip_boilerplate  # noqa: E402

DEF = re.compile(r"^(\s*)(async\s+def|def|class)\s+(\w+)\s*(\([^)]*\)?)?")

PROMPT = """You are localising the code change a GitHub issue needs.

### Issue
{issue}

### Candidate files (signatures only)
{skeletons}

Which of the candidate files must be edited to resolve the issue? Answer with up
to {k} paths from the list above, most likely first, one per line, inside a
single ``` block, and nothing else."""


def skeleton(root, path, max_lines=30, max_chars=900):
    try:
        text = open(os.path.join(root, path), encoding="utf-8", errors="ignore").read()
    except OSError:
        return path
    out = []
    for line in text.splitlines():
        m = DEF.match(line)
        if m:
            indent = len(m.group(1).expandtabs(4)) // 4
            out.append("  " * indent + f"{m.group(2)} {m.group(3)}{(m.group(4) or '')[:60]}")
            if len(out) >= max_lines:
                out.append("  ...")
                break
    body = "\n".join(out)[:max_chars]
    return f"{path}\n{body}" if body else path


def chat(endpoint, prompt, max_tokens=200):
    req = urllib.request.Request(
        endpoint.rstrip("/") + "/chat/completions",
        data=json.dumps({
            "model": "local",
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0,
            "max_tokens": max_tokens,
        }).encode(),
        headers={"Content-Type": "application/json",
                 "Authorization": "Bearer " + os.environ.get("LLM_API_KEY", "none")},
    )
    with urllib.request.urlopen(req, timeout=1800) as r:
        data = json.loads(r.read())
    return data["choices"][0]["message"]["content"], data.get("usage", {})


def parse(answer, candidates):
    picked = []
    for line in answer.splitlines():
        line = line.strip().strip("`").strip().lstrip("-*0123456789. ").strip()
        if not line:
            continue
        for c in candidates:
            if c not in picked and (line == c or c.endswith("/" + line) or line.endswith("/" + c)):
                picked.append(c)
                break
    return picked


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--endpoint", default="http://127.0.0.1:8089/v1")
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("--k", type=int, default=5)
    ap.add_argument("--issue-chars", type=int, default=6000)
    ap.add_argument("--subset", default="")
    ap.add_argument("--shuffle", action="store_true",
                    help="list candidates alphabetically, hiding the engine's order from the model")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    keep = None
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if r.get("loc_files") and (keep is None or r["instance_id"] in keep)]
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8")}
    try:
        with open(args.out, "a", encoding="utf-8") as out:
            for n, r in enumerate(res, 1):
                if r["instance_id"] in done:
                    continue
                row = rows[r["instance_id"]]
                name = row["repo"].split("/")[1]
                dest = os.path.join(args.work, name)
                checkout(os.path.join(args.repos, name), row["base_commit"], dest)
                cands = r["loc_files"][: args.top]
                issue = strip_boilerplate(row["problem_statement"])[: args.issue_chars]
                shown = sorted(cands) if args.shuffle else cands
                prompt = PROMPT.format(issue=issue, k=args.k,
                                       skeletons="\n\n".join(skeleton(dest, c) for c in shown))
                t0 = time.time()
                try:
                    answer, usage = chat(args.endpoint, prompt)
                except Exception as e:  # recorded, not fatal
                    answer, usage = f"ERROR {e}", {}
                secs = time.time() - t0
                picked = parse(answer, cands)
                rec = {"instance_id": r["instance_id"], "repo": r["repo"], "gold": r["gold"],
                       "loc": r["loc_files"], "llm": picked + [c for c in r["loc_files"] if c not in picked],
                       "picked": picked, "answer": answer[:500], "usage": usage, "secs": round(secs, 1)}
                out.write(json.dumps(rec) + "\n")
                out.flush()
                print(f"[{n}/{len(res)}] {r['instance_id']} picked={len(picked)} "
                      f"prompt_tokens={usage.get('prompt_tokens')} {secs:.0f}s", flush=True)
    finally:
        remove_worktrees(args.repos, args.work)


if __name__ == "__main__":
    main()
