"""Definition-level ranking variants for phase 1 (session 18), offline on checkouts:

  chunk   definition-level BM25 with the issue title counted three times (the Python
          stand-in for the engine's `chunk_rank`, `lex_variants.py`)
  rm3     the same after RM3 pseudo-relevance feedback: the top `--fb-docs`
          definitions' term distribution (weighted by retrieval score and idf)
          adds `--fb-terms` words to the query, mixed with weight 1 - lambda
  dense   jina-code-v2 cosine over the top `--dense-k` definitions of `chunk`
          only (a short list, not the repository: CPU embeds ~2.6 chunks/s)

Writes file lists and definition lists (`[path, name]`) per issue; `priors.py
--prior ext` fuses any of them into the engine's stored lists and scores both
halves of the dev split.

    py -3 scripts/research/chunk_prf.py --data devset/swe-dev.json --repos repos --work C:/w/prf
        --out prf-dev.jsonl [--dense-k 30] [--no-dense]
"""
import argparse
import json
import math
import os
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.dirname(__file__))
from compare_baselines import repo_files, words  # noqa: E402
from swebench_localize import checkout, gold_files  # noqa: E402
from dense_loc import chunks_of  # noqa: E402
from lex_variants import Bm25  # noqa: E402


def ranked(scores, owners, labels, n_files=50, n_defs=100):
    best_f, best_d = {}, {}
    for i, s in scores.items():
        f = owners[i]
        if s > best_f.get(f, 0):
            best_f[f] = s
        d = (f, labels[i])
        if labels[i] != "<module>" and s > best_d.get(d, 0):
            best_d[d] = s
    files = [f for f, _ in sorted(best_f.items(), key=lambda kv: (-kv[1], kv[0]))][:n_files]
    defs = [list(d) for d, _ in sorted(best_d.items(), key=lambda kv: (-kv[1], kv[0]))][:n_defs]
    return files, defs


def rm3_query(idx, docs, q, scores, fb_docs, fb_terms, lam):
    top = sorted(scores.items(), key=lambda kv: -kv[1])[:fb_docs]
    if not top:
        return q
    z = sum(s for _, s in top)
    fb = defaultdict(float)
    for i, s in top:
        d = docs[i]
        ln = max(sum(d.values()), 1)
        for w, c in d.items():
            fb[w] += (s / z) * (c / ln)
    for w in list(fb):
        df = idx.df.get(w, 0)
        fb[w] *= math.log((idx.n - df + 0.5) / (df + 0.5) + 1)
    terms = sorted(fb.items(), key=lambda kv: -kv[1])[:fb_terms]
    tz = sum(v for _, v in terms) or 1.0
    qz = sum(q.values()) or 1.0
    out = Counter()
    for w, c in q.items():
        out[w] += lam * c / qz
    for w, v in terms:
        out[w] += (1 - lam) * v / tz
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--fb-docs", type=int, default=5)
    ap.add_argument("--fb-terms", type=int, default=20)
    ap.add_argument("--lam", type=float, default=0.6)
    ap.add_argument("--dense-k", type=int, default=30)
    ap.add_argument("--no-dense", action="store_true")
    ap.add_argument("--subset", default="")
    args = ap.parse_args()
    rows = json.load(open(args.data, encoding="utf-8"))
    if args.subset:
        keep = {r["instance_id"] for r in json.load(open(args.subset, encoding="utf-8"))}
        rows = [r for r in rows if r["instance_id"] in keep]
    rows.sort(key=lambda r: (r["repo"], r["instance_id"]))
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8") if l.strip()}
    emb = None
    if not args.no_dense:
        from dense_loc import Embedder
        emb = Embedder("jina-code-v2", 512, threads=4)
    with open(args.out, "a", encoding="utf-8") as out:
        for row in rows:
            if row["instance_id"] in done:
                continue
            name = row["repo"].split("/")[1]
            dest = os.path.join(args.work, name)
            rec = {"instance_id": row["instance_id"], "repo": row["repo"], "gold": gold_files(row["patch"])}
            try:
                checkout(os.path.join(args.repos, name), row["base_commit"], dest)
                files = repo_files(dest)
                docs, owners, labels, texts = [], [], [], []
                for f in files:
                    try:
                        text = open(os.path.join(dest, f), encoding="utf-8", errors="ignore").read()
                    except OSError:
                        text = ""
                    path_words = Counter(words(f.rsplit(".", 1)[0]))
                    if f.endswith(".py"):
                        for label, ch in chunks_of(f, text, max_chars=20000):
                            docs.append(Counter(words(ch)) + path_words)
                            owners.append(f)
                            labels.append(label)
                            texts.append(ch)
                    else:
                        docs.append(Counter(words(text)) + path_words)
                        owners.append(f)
                        labels.append("<module>")
                        texts.append(text[:1500])
                idx = Bm25(docs)
                text = row["problem_statement"]
                title = text.strip().splitlines()[0] if text.strip() else ""
                q = Counter(set(words(text)))
                for w in set(words(title)):
                    q[w] += 2
                s0 = idx.scores(q)
                rec["chunk_files"], rec["chunk_defs"] = ranked(s0, owners, labels)
                q1 = rm3_query(idx, docs, q, s0, args.fb_docs, args.fb_terms, args.lam)
                s1 = idx.scores(q1)
                rec["rm3_files"], rec["rm3_defs"] = ranked(s1, owners, labels)
                if emb is not None:
                    top = [i for i, _ in sorted(s0.items(), key=lambda kv: -kv[1])
                           if labels[i] != "<module>" or not owners[i].endswith(".py")][:args.dense_k]
                    vecs = emb.embed([text[:4000]] + [texts[i][:1500] for i in top])
                    sims = vecs[1:] @ vecs[0]
                    order = sorted(range(len(top)), key=lambda j: -float(sims[j]))
                    rec["dense_defs"] = [[owners[top[j]], labels[top[j]]] for j in order]
                    seen = []
                    for j in order:
                        if owners[top[j]] not in seen:
                            seen.append(owners[top[j]])
                    rec["dense_files"] = seen
            except Exception as e:  # recorded, not fatal
                rec["error"] = str(e)[-300:]
            out.write(json.dumps(rec) + "\n")
            out.flush()


if __name__ == "__main__":
    main()
