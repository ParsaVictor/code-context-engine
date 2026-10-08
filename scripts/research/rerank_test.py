"""Does a cross-encoder reorder the engine's candidates better? (F92 follow-up)

For every task of a gold set: the engine's localisation list (top 10) is the
first stage; a cross-encoder (ONNX under %LOCALAPPDATA%/neuromesh/models/<model>)
scores (question, file digest) pairs; the final order blends both 50/50 (min-max
reranker score + reciprocal first-stage rank), as the rejected v1-turbo WIP did.
Reports R@3 / P@3 for first stage, reranker alone, and the blend.

    py -3 scripts/research/rerank_test.py --gold tests/.../gold_tasks.toml --repo <checkout>
        --bin <neuromesh> --model jina-reranker-v2
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
try:
    import tomllib
except ImportError:  # Python < 3.11
    import tomli as tomllib

import numpy as np

MODELS = os.path.join(os.environ.get("LOCALAPPDATA", ""), "neuromesh", "models")
SIG = re.compile(r"^\s*(pub |async |def |class |fn |struct |enum |trait |impl |type |interface |export |///|//!|#\s|\"\"\")")


def digest(root, path, max_chars=1800):
    try:
        text = open(os.path.join(root, path), encoding="utf-8", errors="ignore").read()
    except OSError:
        return path
    keep = [l.strip() for l in text.splitlines() if SIG.match(l)]
    return (path + "\n" + "\n".join(keep))[:max_chars]


class Reranker:
    def __init__(self, model, max_len=512):
        import onnxruntime as ort
        from tokenizers import Tokenizer

        d = os.path.join(MODELS, model)
        self.sess = ort.InferenceSession(os.path.join(d, "model_quantized.onnx"),
                                         providers=["CPUExecutionProvider"])
        self.tok = Tokenizer.from_file(os.path.join(d, "tokenizer.json"))
        self.tok.enable_truncation(max_len)
        self.names = {i.name for i in self.sess.get_inputs()}

    def scores(self, query, docs):
        out = []
        for doc in docs:
            e = self.tok.encode(query, doc)
            feed = {"input_ids": np.array([e.ids], dtype=np.int64),
                    "attention_mask": np.array([e.attention_mask], dtype=np.int64)}
            if "token_type_ids" in self.names:
                feed["token_type_ids"] = np.array([e.type_ids], dtype=np.int64)
            out.append(float(self.sess.run(None, feed)[0].reshape(-1)[0]))
        return out


def packet(binary, repo, prompt):
    home = tempfile.mkdtemp(prefix="nmhome-")
    try:
        r = subprocess.run([binary, "packet", "--json", "--query", prompt], cwd=repo,
                           env=dict(os.environ, NEUROMESH_HOME=home, NEUROMESH_NO_BROWSER="1"),
                           capture_output=True, text=True, timeout=900)
        out = r.stdout
        return json.loads(out[out.find("{"):])
    finally:
        shutil.rmtree(home, ignore_errors=True)


def at3(order, gold):
    top = order[:3]
    hit = sum(1 for g in gold if g in top)
    return hit / len(gold), hit / max(len(top), 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gold", required=True)
    ap.add_argument("--repo", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--model", default="jina-reranker-v2")
    args = ap.parse_args()
    tasks = tomllib.load(open(args.gold, "rb"))["task"]
    rr = Reranker(args.model)
    agg = {"first": [0, 0], "rerank": [0, 0], "blend": [0, 0]}
    for t in tasks:
        pkt = packet(args.bin, args.repo, t["prompt"])
        cands = (pkt.get("localization") or pkt.get("selected_paths") or [])[:10]
        if not cands:
            continue
        s = rr.scores(t["prompt"], [digest(args.repo, c) for c in cands])
        lo, hi = min(s), max(s)
        span = (hi - lo) or 1.0
        rer = [c for _, c in sorted(zip(s, cands), key=lambda x: -x[0])]
        blend_score = {c: 0.5 * (si - lo) / span + 0.5 * (1.0 / (1 + i)) for i, (c, si) in enumerate(zip(cands, s))}
        blend = sorted(cands, key=lambda c: -blend_score[c])
        gold = t["gold_files"]
        for name, order in (("first", cands), ("rerank", rer), ("blend", blend)):
            r, p = at3(order, gold)
            agg[name][0] += r
            agg[name][1] += p
        print(t["id"], "first", [c.split('/')[-1] for c in cands[:3]], "blend", [c.split('/')[-1] for c in blend[:3]])
    n = len(tasks)
    for name, (r, p) in agg.items():
        print(f"{name:7s} R@3 {r / n:.3f}  P@3 {p / n:.3f}")


if __name__ == "__main__":
    main()
