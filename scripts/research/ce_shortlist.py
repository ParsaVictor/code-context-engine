"""Cross-encoder re-ranking of the engine's function short list (session 19, P1b).

Pre-registered in docs/research/prereg-s19-crossencoder.md before any number.
jina-reranker-v2 (int8 ONNX, local) scores (issue, function) pairs for the engine's
top `--k` definitions — same texts as dense_shortlist.py (path, name, body; 1500
chars) — and writes the order as `dense_defs` so `paired.py` / `priors.py --prior ext`
score it as a vote unchanged. The issue keeps its first 2000 characters; pairs are
truncated longest-first to `--max-len` tokens.

    py -3 ce_shortlist.py --results dev-s18a-all.jsonl --data devset/swe-dev.json --repos repos \
        --out ce-sl-dev.jsonl [--k 30] [--max-len 1024] [--threads 6]
"""
import argparse
import json
import os
import sys
import time

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
from func_eval import git_show  # noqa: E402

MODELS = os.path.join(os.environ.get("LOCALAPPDATA", ""), "neuromesh", "models")


class CrossEncoder:
    def __init__(self, model, max_len, threads):
        import onnxruntime as ort
        from tokenizers import Tokenizer

        d = os.path.join(MODELS, model)
        so = ort.SessionOptions()
        if threads:
            so.intra_op_num_threads = threads
        self.sess = ort.InferenceSession(os.path.join(d, "model_quantized.onnx"), so,
                                         providers=["CPUExecutionProvider"])
        self.tok = Tokenizer.from_file(os.path.join(d, "tokenizer.json"))
        self.tok.enable_truncation(max_len, strategy="longest_first")
        self.names = {i.name for i in self.sess.get_inputs()}

    def scores(self, query, docs, batch=8):
        enc = [self.tok.encode(query, d) for d in docs]
        out = np.zeros(len(docs), dtype=np.float32)
        for s in range(0, len(enc), batch):
            part = enc[s:s + batch]
            n = max(len(e.ids) for e in part)
            ids = np.zeros((len(part), n), dtype=np.int64)
            mask = np.zeros_like(ids)
            types = np.zeros_like(ids)
            for j, e in enumerate(part):
                ids[j, :len(e.ids)] = e.ids
                mask[j, :len(e.ids)] = 1
                types[j, :len(e.ids)] = e.type_ids
            feed = {"input_ids": ids, "attention_mask": mask}
            if "token_type_ids" in self.names:
                feed["token_type_ids"] = types
            out[s:s + len(part)] = self.sess.run(None, feed)[0].reshape(len(part), -1)[:, 0]
        return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--model", default="jina-reranker-v2")
    ap.add_argument("--k", type=int, default=30)
    ap.add_argument("--max-len", type=int, default=1024)
    ap.add_argument("--threads", type=int, default=6)
    args = ap.parse_args()
    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    res = []
    for line in open(args.results, encoding="utf-8"):
        if line.strip():
            r = json.loads(line)
            if "error" not in r and r.get("definitions") and r["instance_id"] in rows:
                res.append(r)
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8") if l.strip()}
    ce = CrossEncoder(args.model, args.max_len, args.threads)
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
            t0 = time.time()
            s = ce.scores(row["problem_statement"][:2000], texts)
            order = sorted(range(len(defs)), key=lambda i: -s[i])
            out.write(json.dumps({"instance_id": r["instance_id"],
                                  "dense_defs": [[defs[i]["path"], defs[i]["name"]] for i in order],
                                  "scores": [round(float(s[i]), 4) for i in order],
                                  "secs": round(time.time() - t0, 1)}) + "\n")
            out.flush()
            print(f"[{n}/{len(res)}] {r['instance_id']} {time.time() - t0:.1f}s", flush=True)


if __name__ == "__main__":
    main()
