"""Lexical ablation on SWE-bench-style issues: which parts of BM25 matter for
long bug reports? Variants share one tokenizer (`compare_baselines.words`):

  tf        classic BM25 (k1 1.2, b 0.75), body + path words, query as a set
  binary    term present/absent, length = distinct words (what our body field does)
  binary4   binary, words of 4+ letters only (our body field's vocabulary)
  qtf       tf, and a query word counts as often as the report repeats it
  path3     tf, path words weighted x3 as a separate field (BM25F-style)

    py -3 scripts/research/lexical_ablation.py --data rows.json --repos <clones> --out abl.jsonl
"""
import argparse
import json
import math
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from compare_baselines import repo_files, words  # noqa: E402
from swebench_localize import checkout, gold_files, hit  # noqa: E402

K1, B = 1.2, 0.75


class Index:
    def __init__(self, root, files):
        self.files = files
        self.body = []
        self.path = []
        for f in files:
            try:
                text = open(os.path.join(root, f), encoding="utf-8", errors="ignore").read()
            except OSError:
                text = ""
            self.body.append(Counter(words(text)))
            self.path.append(Counter(words(f.rsplit(".", 1)[0])))
        self.n = len(files)

    def rank(self, query, variant):
        qwords = words(query)
        q = Counter(qwords) if variant == "qtf" else Counter(set(qwords))
        docs = []
        for body, path in zip(self.body, self.path):
            if variant in ("binary", "binary4"):
                d = {w: 1 for w in body if variant == "binary" or len(w) >= 4}
                for w in path:
                    d[w] = 1
            elif variant == "path3":
                d = dict(body)
                for w, c in path.items():
                    d[w] = d.get(w, 0) + 3 * c
            else:
                d = dict(body)
                for w, c in path.items():
                    d[w] = d.get(w, 0) + c
            docs.append(d)
        df = Counter()
        for d in docs:
            df.update(d.keys())
        lens = [sum(d.values()) if variant not in ("binary", "binary4") else len(d) for d in docs]
        avg = sum(lens) / max(len(lens), 1)
        scores = []
        for f, d, ln in zip(self.files, docs, lens):
            s = 0.0
            for w, qc in q.items():
                t = d.get(w, 0)
                if t:
                    idf = math.log((self.n - df[w] + 0.5) / (df[w] + 0.5) + 1)
                    s += qc * idf * t * (K1 + 1) / (t + K1 * (1 - B + B * ln / max(avg, 1)))
            if s > 0:
                scores.append((-s, f))
        scores.sort()
        return [f for _, f in scores]


VARIANTS = ["tf", "binary", "binary4", "qtf", "path3"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--only", default="")
    ap.add_argument("--work", required=True)
    args = ap.parse_args()
    rows = json.load(open(args.data, encoding="utf-8"))
    if args.only:
        rows = [r for r in rows if r["repo"] in set(args.only.split(","))]
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8") if l.strip()}
    with open(args.out, "a", encoding="utf-8") as out:
        for row in rows:
            if row["instance_id"] in done:
                continue
            name = row["repo"].split("/")[1]
            dest = os.path.join(args.work, name)
            gold = gold_files(row["patch"])
            rec = {"instance_id": row["instance_id"], "repo": row["repo"], "gold": gold}
            try:
                checkout(os.path.join(args.repos, name), row["base_commit"], dest)
                idx = Index(dest, repo_files(dest))
                for v in VARIANTS:
                    ranked = idx.rank(row["problem_statement"], v)
                    rec[v] = {f"@{k}": hit(ranked, gold, k) for k in (1, 3, 5, 10)}
            except Exception as e:  # recorded, not fatal
                rec["error"] = str(e)[-300:]
            out.write(json.dumps(rec) + "\n")
            out.flush()


if __name__ == "__main__":
    main()
