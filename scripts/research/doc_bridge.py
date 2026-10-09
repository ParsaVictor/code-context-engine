"""Plain-language questions through the project's own documentation (phase 3).

People ask in the vocabulary of the docs ("smart case", "binary files"); the
code says `--smart-case`, `BinaryDetection`. The docs are the bridge: the doc
sections that read most like the question name the flags and identifiers
involved, and the files that define those identifiers get a vote next to the
engine's localisation list (RRF).

    py -3 scripts/research/doc_bridge.py --bin <neuromesh.exe> --cache plain-lists.jsonl
        --set ripgrep=<gold_tasks.toml>:<checkout> [--set ...] [--weights 0.5,1]

R@3 per set = share of gold files in the top 3 (as `rerank_test.py`). Engine lists
are cached (one `packet --json` per question).
"""
import argparse
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict

try:
    import tomllib
except ImportError:  # Python < 3.11
    import tomli as tomllib

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from compare_baselines import SOURCE_EXT, words  # noqa: E402

DOC_EXT = (".md", ".rst", ".txt", ".adoc")
CODE_EXT = tuple(e for e in SOURCE_EXT if e not in (".toml", ".yaml", ".yml", ".json", ".html", ".css", ".scss"))
TICK = re.compile(r"`([^`\n]{2,60})`")
FLAG = re.compile(r"(?<![\w-])--?[a-zA-Z][\w-]{2,40}")
IDENT = re.compile(r"\b(?:[a-z]+_[a-z0-9_]+|[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]+)+|[a-z]+[A-Z][A-Za-z0-9]+)\b")


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


def tracked(repo):
    out = subprocess.run(["git", "-C", repo, "ls-files"], capture_output=True, text=True).stdout
    return [f for f in out.splitlines() if f]


def sections(repo, files):
    """Doc sections: split at headings, long ones cut into 40-line pieces."""
    out = []
    for f in files:
        if not f.lower().endswith(DOC_EXT) or "/node_modules/" in f:
            continue
        try:
            lines = open(os.path.join(repo, f), encoding="utf-8", errors="ignore").read().splitlines()
        except OSError:
            continue
        cur = []
        for line in lines + ["# end"]:
            heading = line.startswith("#") or (cur and set(line.strip()) <= set("=-~^") and line.strip())
            if heading or len(cur) >= 40:
                if cur:
                    out.append((f, "\n".join(cur)))
                cur = []
            cur.append(line)
    return out


class Bm25:
    def __init__(self, docs):
        self.docs = [Counter(words(d)) for d in docs]
        self.lens = [sum(d.values()) for d in self.docs]
        self.avg = sum(self.lens) / max(len(self.docs), 1)
        self.df = Counter()
        for d in self.docs:
            self.df.update(d.keys())
        self.n = len(self.docs)

    def rank(self, q):
        qs = set(words(q))
        out = []
        for i, d in enumerate(self.docs):
            s = 0.0
            for w in qs:
                t = d.get(w)
                if t:
                    idf = math.log((self.n - self.df[w] + 0.5) / (self.df[w] + 0.5) + 1)
                    s += idf * t * 2.2 / (t + 1.2 * (0.25 + 0.75 * self.lens[i] / max(self.avg, 1)))
            if s > 0:
                out.append((s, i))
        return [i for _, i in sorted(out, reverse=True)]


def identifiers(text):
    found = []
    for m in TICK.findall(text):
        for tok in re.split(r"[\s(),=]+", m):
            tok = tok.strip(".:;'\"")
            if len(tok) >= 3:
                found.append(tok)
    found += FLAG.findall(text)
    found += IDENT.findall(text)
    return found


def bridge(repo, files, secs, idx, question, top_secs=3):
    ranked = idx.rank(question)[:top_secs]
    toks = Counter()
    for r, i in enumerate(ranked):
        for t in set(identifiers(secs[i][1])):
            toks[t] += 1.0 / (r + 1)
    if not toks:
        return []
    code = [f for f in files if f.endswith(CODE_EXT) and not re.search(r"(^|/)(tests?|testdata|examples?|docs?)/", f)]
    votes = defaultdict(float)
    # A file votes once per identifier it contains, scaled by how rare the identifier is.
    texts = {}
    for f in code:
        try:
            texts[f] = open(os.path.join(repo, f), encoding="utf-8", errors="ignore").read()
        except OSError:
            texts[f] = ""
    for t, w in toks.items():
        needle = t.lstrip("-") if t.startswith("-") else t
        if len(needle) < 3:
            continue
        holders = [f for f, s in texts.items() if needle in s]
        if not holders or len(holders) > 0.2 * len(code) + 3:
            continue
        for f in holders:
            votes[f] += w / math.log(2 + len(holders))
    return [f for f, _ in sorted(votes.items(), key=lambda kv: -kv[1])]


def rrf(lists, weights, k=60):
    s = defaultdict(float)
    for lst, w in zip(lists, weights):
        for i, f in enumerate(lst):
            s[f] += w / (k + i + 1)
    return [f for f, _ in sorted(s.items(), key=lambda kv: -kv[1])]


def r_at3(order, gold):
    top = [o.replace("\\", "/") for o in order[:3]]
    return sum(1 for g in gold if g in top) / len(gold)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--cache", required=True)
    ap.add_argument("--set", action="append", required=True, help="name=gold.toml:checkout")
    ap.add_argument("--weights", default="0.5,1")
    ap.add_argument("--top-secs", type=int, default=3)
    args = ap.parse_args()
    cache = {}
    if os.path.exists(args.cache):
        for line in open(args.cache, encoding="utf-8"):
            if line.strip():
                r = json.loads(line)
                cache[(r["set"], r["id"])] = r["localization"]
    weights = [float(w) for w in args.weights.split(",")]
    with open(args.cache, "a", encoding="utf-8") as out:
        for spec in args.set:
            name, rest = spec.split("=", 1)
            gold_path, repo = rest.rsplit(":", 1) if rest.count(":") > 1 else rest.split(":", 1)
            tasks = tomllib.load(open(gold_path, "rb"))["task"]
            files = tracked(repo)
            secs = sections(repo, files)
            idx = Bm25([s[1] for s in secs])
            res = {"engine": [], "doc": [], **{f"w={w:g}": [] for w in weights}}
            fired = 0
            for t in tasks:
                key = (name, t["id"])
                if key not in cache:
                    loc = packet(args.bin, repo, t["prompt"]).get("localization", [])
                    cache[key] = loc
                    out.write(json.dumps({"set": name, "id": t["id"], "localization": loc}) + "\n")
                    out.flush()
                loc = cache[key]
                doc = bridge(repo, files, secs, idx, t["prompt"], args.top_secs)
                fired += bool(doc)
                gold = t["gold_files"]
                res["engine"].append(r_at3(loc, gold))
                res["doc"].append(r_at3(doc, gold))
                for w in weights:
                    res[f"w={w:g}"].append(r_at3(rrf([loc, doc], [1.0, w]), gold))
            print(f"{name:8s} {len(tasks)} q, {len(secs)} doc sections, bridge fired {fired}: " +
                  "  ".join(f"{k} {sum(v) / len(v):.3f}" for k, v in res.items()))


if __name__ == "__main__":
    main()
