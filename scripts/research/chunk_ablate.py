"""Which part of the Python definition-level BM25 (`chunk_prf.py`) makes its
function list better than the engine's `rank_definitions`? Same checkout, same
query, one index per variant; function lists written per variant for
`priors.py --prior ext` / standalone scoring.

  base       chunk_prf's chunks: method text prefixed with `class Name:`
  noclass    methods without the class line
  owner2     class name of a method counted twice (owner-name boost)
  query_tf   query terms weighted by frequency (log), not set membership

    py -3 scripts/research/chunk_ablate.py --data devset/swe-dev.json --repos repos --work C:/w/abl
        --out chunk-abl-dev.jsonl [--subset devset/dev-fast.json]
"""
import argparse
import json
import math
import os
import sys
from collections import Counter

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.dirname(__file__))
from compare_baselines import repo_files, words  # noqa: E402
from swebench_localize import checkout, gold_files  # noqa: E402
from lex_variants import Bm25  # noqa: E402
from chunk_prf import ranked  # noqa: E402
from dense_loc import chunks_of  # noqa: E402


def variant_docs(files, dest):
    """Per variant: (docs, owners, labels)."""
    out = {v: ([], [], []) for v in ("base", "noclass", "owner2")}
    for f in files:
        try:
            text = open(os.path.join(dest, f), encoding="utf-8", errors="ignore").read()
        except OSError:
            text = ""
        path_words = Counter(words(f.rsplit(".", 1)[0]))
        if not f.endswith(".py"):
            for v in out:
                out[v][0].append(Counter(words(text)) + path_words)
                out[v][1].append(f)
                out[v][2].append("<module>")
            continue
        for label, ch in chunks_of(f, text, max_chars=20000):
            base = Counter(words(ch)) + path_words
            cls = label.split(".")[0] if "." in label else None
            if cls:
                body = ch.split("\n", 2)[2] if ch.count("\n") >= 2 else ch
                noclass = Counter(words(f + "\n" + body)) + path_words
                owner2 = base + Counter(words(cls))
            else:
                noclass, owner2 = base, base
            for v, d in (("base", base), ("noclass", noclass), ("owner2", owner2)):
                out[v][0].append(d)
                out[v][1].append(f)
                out[v][2].append(label)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--out", required=True)
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
                text = row["problem_statement"]
                title = text.strip().splitlines()[0] if text.strip() else ""
                q = Counter(set(words(text)))
                for w in set(words(title)):
                    q[w] += 2
                qtf = Counter({w: 1 + math.log(c) for w, c in Counter(words(text)).items()})
                for w in set(words(title)):
                    qtf[w] += 2
                for v, (docs, owners, labels) in variant_docs(files, dest).items():
                    idx = Bm25(docs)
                    rec[f"{v}_files"], rec[f"{v}_defs"] = ranked(idx.scores(q), owners, labels)
                    if v == "base":
                        rec["query_tf_files"], rec["query_tf_defs"] = ranked(idx.scores(qtf), owners, labels)
            except Exception as e:  # recorded, not fatal
                rec["error"] = str(e)[-300:]
            out.write(json.dumps(rec) + "\n")
            out.flush()


if __name__ == "__main__":
    main()
