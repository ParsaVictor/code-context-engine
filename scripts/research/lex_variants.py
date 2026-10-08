"""Cheap lexical variants for issue → file localisation, positions kept per gold
file so variants can be compared and fused offline.

  file      BM25 over whole files (body + path words)
  chunk     BM25 over functions/classes/module heads (Python `ast`), a file
            scores its best chunk (LocAgent indexes entity contents this way)
  *_title   same, with the issue's first line counted three times

    py -3 scripts/research/lex_variants.py --data rows.json --repos <clones> --out lex.jsonl --work <dir>
"""
import argparse
import json
import math
import os
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from compare_baselines import repo_files, words  # noqa: E402
from swebench_localize import checkout, gold_files  # noqa: E402
from dense_loc import chunks_of  # noqa: E402

K1, B = 1.2, 0.75


class Bm25:
    def __init__(self, docs):
        self.docs = docs  # list of Counter
        self.lens = [sum(d.values()) for d in docs]
        self.avg = sum(self.lens) / max(len(docs), 1)
        self.df = Counter()
        for d in docs:
            self.df.update(d.keys())
        self.n = len(docs)
        self.post = defaultdict(list)
        for i, d in enumerate(docs):
            for w, c in d.items():
                self.post[w].append((i, c))

    def scores(self, q):
        out = defaultdict(float)
        for w, qc in q.items():
            if w not in self.post:
                continue
            idf = math.log((self.n - self.df[w] + 0.5) / (self.df[w] + 0.5) + 1)
            for i, t in self.post[w]:
                out[i] += qc * idf * t * (K1 + 1) / (t + K1 * (1 - B + B * self.lens[i] / max(self.avg, 1)))
        return out


def ranked_files(scores, owners):
    best = {}
    for i, s in scores.items():
        f = owners[i]
        if s > best.get(f, 0):
            best[f] = s
    return [f for f, _ in sorted(best.items(), key=lambda kv: (-kv[1], kv[0]))]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--work", required=True)
    args = ap.parse_args()
    rows = json.load(open(args.data, encoding="utf-8"))
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
                files = repo_files(dest)
                fdocs, cdocs, owners = [], [], []
                for f in files:
                    try:
                        text = open(os.path.join(dest, f), encoding="utf-8", errors="ignore").read()
                    except OSError:
                        text = ""
                    path_words = Counter(words(f.rsplit(".", 1)[0]))
                    fdocs.append(Counter(words(text)) + path_words)
                    if f.endswith(".py"):
                        for _, ch in chunks_of(f, text, max_chars=20000):
                            cdocs.append(Counter(words(ch)) + path_words)
                            owners.append(f)
                    else:
                        cdocs.append(fdocs[-1])
                        owners.append(f)
                fidx, cidx = Bm25(fdocs), Bm25(cdocs)
                text = row["problem_statement"]
                title = text.strip().splitlines()[0] if text.strip() else ""
                q = Counter(set(words(text)))
                qt = Counter(set(words(text)))
                for w in set(words(title)):
                    qt[w] += 2
                lists = {
                    "file": ranked_files(fidx.scores(q), files),
                    "file_title": ranked_files(fidx.scores(qt), files),
                    "chunk": ranked_files(cidx.scores(q), owners),
                    "chunk_title": ranked_files(cidx.scores(qt), owners),
                }
                for k, lst in lists.items():
                    rec[k] = lst[:50]
            except Exception as e:  # recorded, not fatal
                rec["error"] = str(e)[-300:]
            out.write(json.dumps(rec) + "\n")
            out.flush()


if __name__ == "__main__":
    main()
