"""File-level localisation on SWE-bench Lite: does the packet contain the file the
reference patch edits?

    python scripts/swebench_localize.py --data lite.json --repos <dir with blobless clones>
        --bin <neuromesh binary> [--limit N] [--repo django/django] [--out results.jsonl]

For every instance: check out `base_commit` into a scratch worktree, run
`neuromesh packet --json --query <problem_statement>` there (fresh NEUROMESH_HOME), and
record the packet's files in order; plain file-level BM25 over the same checkout ranks
the same query as the baseline. Gold = the files `patch` edits (exactly one per Lite
instance). Reports hit@k (k = 1, 3, 5, packet) — the "file-level Acc@k" used by
Agentless and LocAgent — plus packet size and tokens.

Results stream to --out as JSON lines, so an interrupted run resumes where it stopped.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import zlib

sys.path.insert(0, os.path.dirname(__file__))
from compare_baselines import Bm25, repo_files  # noqa: E402


def gold_files(patch):
    return sorted(set(re.findall(r"^diff --git a/(\S+)", patch, re.M)))


def checkout(clone, commit, dest):
    """Fresh worktree of `commit` at `dest`. Parallel shards share one clone and
    git's worktree bookkeeping takes locks, so a failed add is retried (39 of the
    first 276 test-split instances were lost to that, not to the engine)."""
    clone, dest = os.path.abspath(clone), os.path.abspath(dest)
    # An existing worktree is moved to the commit in place: only files that
    # differ are rewritten. Re-creating a django worktree per instance (6k
    # files through the virus scanner) made a run IO-bound at minutes each.
    if os.path.isdir(os.path.join(dest, ".git")) or os.path.isfile(os.path.join(dest, ".git")):
        r = subprocess.run(["git", "-C", dest, "checkout", "--force", "--detach", commit],
                           capture_output=True, text=True)
        subprocess.run(["git", "-C", dest, "clean", "-ffdxq"], capture_output=True)
        ok = subprocess.run(["git", "-C", dest, "rev-parse", "HEAD"], capture_output=True, text=True)
        if r.returncode == 0 and ok.stdout.strip().startswith(commit[:12]):
            return
    last = ""
    for attempt in range(5):
        if os.path.isdir(dest):
            subprocess.run(["git", "-C", clone, "worktree", "remove", "--force", dest], capture_output=True)
            shutil.rmtree(dest, ignore_errors=True)
        subprocess.run(["git", "-C", clone, "worktree", "prune"], capture_output=True)
        r = subprocess.run(["git", "-C", clone, "worktree", "add", "--detach", dest, commit],
                           capture_output=True, text=True)
        ok = subprocess.run(["git", "-C", dest, "rev-parse", "HEAD"], capture_output=True, text=True)
        if r.returncode == 0 and ok.stdout.strip().startswith(commit[:12]):
            return
        last = r.stderr.strip()[-300:]
        time.sleep(3 + 5 * attempt)
    raise RuntimeError(last)


def remove_worktrees(repos, work):
    """Delete the scratch worktrees a run left (a checkout of django is ~100 MB
    and every shard keeps one per repository)."""
    if not os.path.isdir(work):
        return
    for name in os.listdir(work):
        dest = os.path.abspath(os.path.join(work, name))
        clone = os.path.abspath(os.path.join(repos, name))
        subprocess.run(["git", "-C", clone, "worktree", "remove", "--force", dest], capture_output=True)
        shutil.rmtree(dest, ignore_errors=True)
        subprocess.run(["git", "-C", clone, "worktree", "prune"], capture_output=True)


def run_packet(binary, workspace, query):
    home = tempfile.mkdtemp(prefix="nmhome-")
    env = dict(os.environ, NEUROMESH_HOME=home, NEUROMESH_NO_BROWSER="1", PWD=workspace)
    try:
        r = subprocess.run(
            [binary, "packet", "--json", "--query", query],
            cwd=workspace, env=env, capture_output=True, text=True, timeout=900,
        )
        out = r.stdout
        start = out.find("{")
        data = json.loads(out[start:]) if start >= 0 else {}
        return data
    finally:
        shutil.rmtree(home, ignore_errors=True)


def hit(files, gold, k=None):
    top = files if k is None else files[:k]
    top = [f.replace("\\", "/") for f in top]
    same = lambda t, g: t == g or t.endswith("/" + g) or g.endswith("/" + t)  # noqa: E731
    return int(all(any(same(t, g) for t in top) for g in gold))


def main():
    try:
        _main()
    finally:
        args = _ARGS.get("args")
        if args is not None:
            remove_worktrees(args.repos, args.work)


_ARGS = {}


def _main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", default="results.jsonl")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--repo", default="")
    ap.add_argument("--only", default="", help="comma-separated repos to keep")
    ap.add_argument("--shard", default="", help="i/n: this process takes every n-th instance starting at i")
    ap.add_argument("--work", default=os.path.join(tempfile.gettempdir(), "swe-wt"))
    args = ap.parse_args()
    _ARGS["args"] = args

    rows = json.load(open(args.data, encoding="utf-8"))
    if args.only:
        keep = set(args.only.split(","))
        rows = [r for r in rows if r["repo"] in keep]
    if args.repo:
        rows = [r for r in rows if r["repo"] == args.repo]
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8") if l.strip()}
    todo = [r for r in rows if r["instance_id"] not in done]
    if args.shard:
        i, n = (int(x) for x in args.shard.split("/"))
        # By a stable hash of the id, not by position in what is left: shards
        # that start while another run is still writing the same files would
        # otherwise see different leftovers and overlap.
        todo = [r for r in todo if zlib.crc32(r["instance_id"].encode()) % n == i]
    if args.limit:
        todo = todo[: args.limit]
    os.makedirs(args.work, exist_ok=True)
    with open(args.out, "a", encoding="utf-8") as out:
        for i, row in enumerate(todo, 1):
            name = row["repo"].split("/")[1]
            clone = os.path.join(args.repos, name)
            dest = os.path.join(args.work, name)
            gold = gold_files(row["patch"])
            rec = {"instance_id": row["instance_id"], "repo": row["repo"], "gold": gold}
            try:
                checkout(clone, row["base_commit"], dest)
                pkt = run_packet(args.bin, dest, row["problem_statement"])
                files = pkt.get("selected_paths") or pkt.get("selected_files", [])
                rec.update(
                    ours_files=files,
                    ours_tokens=pkt.get("packet_tokens"),
                    ours_latency_ms=pkt.get("latency_ms"),
                    ours_hit=hit(files, gold),
                    **{f"ours_hit@{k}": hit(files, gold, k) for k in (1, 3, 5)},
                )
                if pkt.get("definitions"):
                    rec["definitions"] = pkt["definitions"]
                loc = pkt.get("localization") or []
                if loc:
                    rec["loc_files"] = loc
                    rec.update(**{f"loc_hit@{k}": hit(loc, gold, k) for k in (1, 3, 5, 10)})
                ranked = pkt.get("ranked_paths") or []
                if ranked:
                    rec.update(**{f"bm25f_hit@{k}": hit(ranked, gold, k) for k in (1, 3, 5, 10)})
                bm = Bm25(dest, repo_files(dest)).rank(row["problem_statement"])
                rec.update(**{f"bm25_hit@{k}": hit(bm, gold, k) for k in (1, 3, 5, 10)})
            except Exception as e:  # recorded, not fatal: one bad checkout must not stop the run
                rec["error"] = str(e)[-300:]
            out.write(json.dumps(rec) + "\n")
            out.flush()
            print(f"[{i}/{len(todo)}] {rec['instance_id']} ours={rec.get('ours_hit')} "
                  f"files={len(rec.get('ours_files', []))} bm25@1={rec.get('bm25_hit@1')} {rec.get('error', '')[:80]}",
                  flush=True)


if __name__ == "__main__":
    main()
