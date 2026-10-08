"""Research harness: file-level localisation on SWE-bench with lexical, dense and
fused rankers, scores kept so fusion can be studied offline.

    py -3 scripts/research/dense_loc.py --data rows.json --repos <clones> --out run.jsonl
        [--only repo1,repo2] [--single-gold] [--shard i/n] [--model jina-code-v2]

Per instance: check out `base_commit` (git worktree), split every .py file into
chunks (module head, each function, each method; Python `ast`, line windows
when a file does not parse), embed the chunks with a code embedding model (ONNX,
mean pooling), and rank files by their best chunk against the issue text.
Embeddings are cached per chunk text across commits (`--cache`), so one repository
is embedded once and later commits only embed what changed.

Each output line keeps the top-100 files with scores for: bm25 (file text +
path, as `compare_baselines.py`), dense (max chunk cosine), so fusions can be
computed without re-running. Gold = files the reference patch edits.
"""
import argparse
import ast
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from compare_baselines import Bm25  # noqa: E402

MODELS = os.path.join(os.environ.get("LOCALAPPDATA", ""), "neuromesh", "models")


def gold_files(patch):
    return sorted(set(re.findall(r"^diff --git a/(\S+)", patch, re.M)))


def checkout(clone, commit, dest):
    if os.path.isdir(dest):
        subprocess.run(["git", "-C", clone, "worktree", "remove", "--force", dest], capture_output=True)
        shutil.rmtree(dest, ignore_errors=True)
    subprocess.run(["git", "-C", clone, "worktree", "prune"], capture_output=True)
    r = subprocess.run(["git", "-C", clone, "worktree", "add", "--detach", dest, commit],
                       capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip()[-300:])


def py_files(root):
    r = subprocess.run(["git", "-C", root, "ls-files", "*.py"], capture_output=True, text=True)
    return [f for f in r.stdout.splitlines() if f]


def chunks_of(path, text, max_chars=1500):
    """(label, text) chunks of one Python file."""
    lines = text.splitlines()
    out = []
    head = "\n".join(lines[:40])
    out.append(("<module>", f"{path}\n{head}"[:max_chars]))
    try:
        tree = ast.parse(text)
    except (SyntaxError, ValueError):
        for i in range(40, len(lines), 60):
            out.append((f"L{i}", f"{path}\n" + "\n".join(lines[i:i + 60])[:max_chars]))
        return out

    def seg(node):
        end = getattr(node, "end_lineno", node.lineno)
        return "\n".join(lines[node.lineno - 1:end])

    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            out.append((node.name, f"{path}\n{seg(node)}"[:max_chars]))
        elif isinstance(node, ast.ClassDef):
            body = [n for n in node.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))]
            head_end = body[0].lineno - 1 if body else getattr(node, "end_lineno", node.lineno)
            out.append((node.name, f"{path}\n" + "\n".join(lines[node.lineno - 1:head_end])[:max_chars]))
            for m in body:
                out.append((f"{node.name}.{m.name}", f"{path}\nclass {node.name}:\n{seg(m)}"[:max_chars]))
    return out


class Embedder:
    def __init__(self, model, max_tokens, threads=0):
        import onnxruntime as ort
        from tokenizers import Tokenizer

        d = os.path.join(MODELS, model)
        so = ort.SessionOptions()
        if threads:
            so.intra_op_num_threads = threads
        self.sess = ort.InferenceSession(os.path.join(d, "model_quantized.onnx"), so,
                                         providers=["CPUExecutionProvider"])
        self.tok = Tokenizer.from_file(os.path.join(d, "tokenizer.json"))
        self.max_tokens = max_tokens

    def embed(self, texts, max_tokens=None, batch=16):
        max_tokens = max_tokens or self.max_tokens
        enc = [self.tok.encode(t) for t in texts]
        order = sorted(range(len(texts)), key=lambda i: len(enc[i].ids))
        out = np.zeros((len(texts), 768), dtype=np.float32)
        for s in range(0, len(order), batch):
            idx = order[s:s + batch]
            ids = [enc[i].ids[:max_tokens] for i in idx]
            n = max(len(x) for x in ids)
            arr = np.zeros((len(ids), n), dtype=np.int64)
            mask = np.zeros((len(ids), n), dtype=np.int64)
            for j, x in enumerate(ids):
                arr[j, :len(x)] = x
                mask[j, :len(x)] = 1
            h = self.sess.run(None, {"input_ids": arr, "attention_mask": mask})[0]
            m = mask[..., None].astype(np.float32)
            v = (h * m).sum(1) / np.maximum(m.sum(1), 1)
            v /= np.maximum(np.linalg.norm(v, axis=1, keepdims=True), 1e-9)
            out[idx] = v
        return out


class Cache:
    def __init__(self, path):
        self.path = path
        self.vec = {}
        if os.path.exists(path):
            z = np.load(path, allow_pickle=False)
            for k, v in zip(z["keys"], z["vecs"]):
                self.vec[str(k)] = v
        self.dirty = False

    def save(self):
        if not self.dirty:
            return
        keys = np.array(list(self.vec.keys()))
        vecs = np.stack(list(self.vec.values())).astype(np.float16)
        tmp = self.path + ".tmp.npz"
        np.savez(tmp, keys=keys, vecs=vecs)
        os.replace(tmp, self.path)
        self.dirty = False


def key(text):
    return hashlib.sha1(text.encode("utf-8", "ignore")).hexdigest()


def top(scores, n=100):
    items = sorted(scores.items(), key=lambda kv: (-kv[1], kv[0]))[:n]
    return [[f, round(float(s), 5)] for f, s in items]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--only", default="")
    ap.add_argument("--shard", default="")
    ap.add_argument("--single-gold", action="store_true")
    ap.add_argument("--model", default="jina-code-v2")
    ap.add_argument("--max-tokens", type=int, default=256)
    ap.add_argument("--query-tokens", type=int, default=1024)
    ap.add_argument("--threads", type=int, default=0)
    ap.add_argument("--cache", default="")
    ap.add_argument("--work", default="")
    args = ap.parse_args()

    rows = json.load(open(args.data, encoding="utf-8"))
    if args.only:
        keep = set(args.only.split(","))
        rows = [r for r in rows if r["repo"] in keep]
    if args.single_gold:
        rows = [r for r in rows if len(gold_files(r["patch"])) == 1]
    done = set()
    if os.path.exists(args.out):
        done = {json.loads(l)["instance_id"] for l in open(args.out, encoding="utf-8") if l.strip()}
    todo = [r for r in rows if r["instance_id"] not in done]
    if args.shard:
        i, n = (int(x) for x in args.shard.split("/"))
        todo = todo[i::n]
    emb = Embedder(args.model, args.max_tokens, args.threads)
    cache_dir = args.cache or os.path.join(os.path.dirname(args.out) or ".", "emb-cache")
    os.makedirs(cache_dir, exist_ok=True)
    work = os.path.abspath(args.work or os.path.join(os.path.dirname(args.out) or ".", "wt-dense"))
    caches = {}
    with open(args.out, "a", encoding="utf-8") as out:
        for n_done, row in enumerate(todo, 1):
            name = row["repo"].split("/")[1]
            t0 = time.time()
            rec = {"instance_id": row["instance_id"], "repo": row["repo"], "gold": gold_files(row["patch"])}
            try:
                dest = os.path.join(work, name)
                checkout(os.path.abspath(os.path.join(args.repos, name)), row["base_commit"], dest)
                files = py_files(dest)
                cache = caches.get(name)
                if cache is None:
                    cache = caches[name] = Cache(os.path.join(cache_dir, f"{name}-{args.model}-{args.max_tokens}.npz"))
                labels, keys, missing = [], [], {}
                for f in files:
                    try:
                        text = open(os.path.join(dest, f), encoding="utf-8", errors="ignore").read()
                    except OSError:
                        continue
                    for label, ch in chunks_of(f, text):
                        k = key(ch)
                        labels.append((f, label))
                        keys.append(k)
                        if k not in cache.vec:
                            missing[k] = ch
                t_emb = time.time()
                if missing:
                    ks = list(missing)
                    vecs = emb.embed([missing[k] for k in ks])
                    for k, v in zip(ks, vecs):
                        cache.vec[k] = v.astype(np.float16)
                    cache.dirty = True
                rec["embedded"] = len(missing)
                rec["chunks"] = len(keys)
                rec["embed_s"] = round(time.time() - t_emb, 1)
                q = emb.embed([row["problem_statement"]], max_tokens=args.query_tokens)[0]
                mat = np.stack([cache.vec[k] for k in keys]).astype(np.float32)
                sims = mat @ q
                dense = {}
                best_chunk = {}
                for (f, label), s in zip(labels, sims):
                    if s > dense.get(f, -2):
                        dense[f] = float(s)
                        best_chunk[f] = label
                rec["dense"] = top(dense)
                rec["dense_chunk"] = {f: best_chunk[f] for f, _ in rec["dense"][:10]}
                bm = Bm25(dest, files)
                ranked = bm.rank(row["problem_statement"])
                rec["bm25"] = [[f, None] for f in ranked[:100]]
                cache.save()
            except Exception as e:  # recorded, not fatal
                rec["error"] = str(e)[-300:]
            rec["secs"] = round(time.time() - t0, 1)
            out.write(json.dumps(rec) + "\n")
            out.flush()
            g = rec["gold"]
            pos = next((i for i, (f, _) in enumerate(rec.get("dense", [])) if f in g), None)
            print(f"[{n_done}/{len(todo)}] {rec['instance_id']} dense_pos={pos} chunks={rec.get('chunks')} "
                  f"new={rec.get('embedded')} {rec['secs']}s {rec.get('error', '')[:80]}", flush=True)


if __name__ == "__main__":
    main()
