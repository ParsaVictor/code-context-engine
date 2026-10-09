"""Dense re-ranking of the engine's own short list (phase 1, prior 6).

Whole-repository chunk embedding is hours per large repository on a laptop CPU
(2.6 chunks/s, §8.1); CodeRankEmbed-style gains need it only where the lexical
ranking already looks. This embeds the engine's top `--k` definitions (from a
harness run with `NM_DEFINITIONS` ≥ k) and the issue with jina-code-v2 (int8
ONNX), reorders them by cosine, and writes `dense_defs` / `dense_files` for
`priors.py --prior ext`. Source text comes from the clone at the base commit
(`git show`), so no checkout is needed. Embeddings are cached by text hash.

    py -3 scripts/research/dense_shortlist.py --results dev-s18a-all.jsonl --data devset/swe-dev.json
        --repos repos --out dense-sl-dev.jsonl [--k 30] [--cache emb-cache/shortlist.npz]
"""
import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from dense_loc import Cache, Embedder, key  # noqa: E402
from func_eval import git_show  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--k", type=int, default=30)
    ap.add_argument("--cache", default="")
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--subset", default="")
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if "error" not in r and r.get("definitions")]
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
        res = [r for r in res if r["instance_id"] in keep]
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8") if l.strip()}
    emb = Embedder("jina-code-v2", 512, threads=args.threads)
    cache = Cache(args.cache) if args.cache else None
    with open(args.out, "a", encoding="utf-8") as out:
        for n, r in enumerate(res, 1):
            if r["instance_id"] in done:
                continue
            row = rows[r["instance_id"]]
            clone = os.path.join(args.repos, row["repo"].split("/")[1])
            defs = r["definitions"][: args.k]
            texts, files = [], {}
            for d in defs:
                if d["path"] not in files:
                    files[d["path"]] = git_show(clone, row["base_commit"], d["path"]).splitlines()
                lines = files[d["path"]]
                a, b = d["lines"]
                body = "\n".join(lines[max(a - 1, 0):b])
                texts.append(f"{d['path']}\n{d['name']}\n{body}"[:1500])
            query = row["problem_statement"][:4000]
            t0 = time.time()
            todo = [t for t in [query] + texts if cache is None or key(t) not in cache.vec]
            if todo:
                vecs = emb.embed(todo)
                if cache is not None:
                    for t, v in zip(todo, vecs):
                        cache.vec[key(t)] = v
                    cache.dirty = True
                fresh = dict(zip(todo, vecs))
            else:
                fresh = {}
            get = lambda t: fresh[t] if t in fresh else cache.vec[key(t)]  # noqa: E731
            q = get(query).astype("float32")
            sims = [float(get(t).astype("float32") @ q) for t in texts]
            order = sorted(range(len(defs)), key=lambda i: -sims[i])
            dense_defs = [[defs[i]["path"], defs[i]["name"]] for i in order]
            dense_files = []
            for p, _ in dense_defs:
                if p not in dense_files:
                    dense_files.append(p)
            out.write(json.dumps({"instance_id": r["instance_id"], "dense_defs": dense_defs,
                                  "dense_files": dense_files, "sims": [round(sims[i], 4) for i in order],
                                  "secs": round(time.time() - t0, 1)}) + "\n")
            out.flush()
            if cache is not None and n % 10 == 0:
                cache.save()
            print(f"[{n}/{len(res)}] {r['instance_id']} {time.time() - t0:.1f}s", flush=True)
    if cache is not None:
        cache.save()


if __name__ == "__main__":
    main()
