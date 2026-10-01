"""Rank files for every gold question with two outside baselines, same gold.

    python scripts/compare_baselines.py <gold_tasks.toml> <checkout> [--aider]

- bm25: plain file-level BM25 over whitespace/identifier words (k1 1.2, b 0.75),
  English stopwords dropped — what a search box over the repository does.
- aider: Aider's own RepoMap (PageRank over definition/reference tags,
  personalised by the identifiers and file names the question mentions — the
  same inputs aider derives from a chat message). Needs `pip install aider-chat`.

Prints recall@k and precision@k (k = 1, 3, 5) per baseline. Our engine's
numbers come from the gold harness (`scripts/benchmark-fast.sh`), which ships a
variable-size packet; compare its recall with recall@3/@5 here.
"""
import math
import os
import re
import subprocess
import sys
from collections import Counter

STOP = set(
    "a an and are as at be by can could do does for from has have how i if in into is it its "
    "of on or that the their then there these this those to was we were what when where which "
    "while who why will with without would you your".split()
)
SOURCE_EXT = {
    ".rs", ".py", ".js", ".jsx", ".ts", ".tsx", ".go", ".c", ".h", ".cc", ".cpp", ".hpp",
    ".java", ".kt", ".scala", ".rb", ".php", ".cs", ".swift", ".jl", ".r", ".R", ".vue",
    ".svelte", ".lua", ".sh", ".toml", ".yaml", ".yml", ".json", ".html", ".css", ".scss",
}


def load_gold(path):
    text = open(path, encoding="utf-8").read()
    tasks = []
    for block in text.split("[[task]]")[1:]:
        tid = re.search(r'^id\s*=\s*"([^"]+)"', block, re.M).group(1)
        prompt = re.search(r'^prompt\s*=\s*"((?:[^"\\]|\\.)*)"', block, re.M).group(1)
        gold = re.findall(r'"([^"]+)"', re.search(r"^gold_files\s*=\s*\[(.*?)\]", block, re.M | re.S).group(1))
        tasks.append((tid, prompt.replace('\\"', '"'), gold))
    return tasks


def repo_files(root):
    out = subprocess.run(["git", "-C", root, "ls-files"], capture_output=True, text=True, check=True)
    return [f for f in out.stdout.splitlines() if os.path.splitext(f)[1] in SOURCE_EXT]


def words(text):
    out = []
    for raw in re.split(r"[^A-Za-z0-9_]+", text):
        for part in re.split(r"_|(?<=[a-z0-9])(?=[A-Z])", raw):
            p = part.lower()
            if len(p) >= 2 and p.isalpha() and p not in STOP:
                out.append(p)
    return out


class Bm25:
    def __init__(self, root, files):
        self.files = files
        self.tf = []
        df = Counter()
        for f in files:
            try:
                text = open(os.path.join(root, f), encoding="utf-8", errors="ignore").read()
            except OSError:
                text = ""
            c = Counter(words(text) + words(f))
            self.tf.append(c)
            df.update(c.keys())
        self.n = len(files)
        self.avg = sum(sum(c.values()) for c in self.tf) / max(self.n, 1)
        self.idf = {w: math.log((self.n - d + 0.5) / (d + 0.5) + 1) for w, d in df.items()}

    def rank(self, prompt):
        q = set(words(prompt))
        scores = []
        for f, c in zip(self.files, self.tf):
            length = sum(c.values()) or 1
            s = 0.0
            for w in q:
                t = c.get(w, 0)
                if t:
                    s += self.idf[w] * t * 2.2 / (t + 1.2 * (0.25 + 0.75 * length / self.avg))
            if s > 0:
                scores.append((s, f))
        scores.sort(reverse=True)
        return [f for _, f in scores]


class AiderRank:
    def __init__(self, root, files):
        from aider.io import InputOutput
        from aider.repomap import RepoMap

        class Tokens:
            def token_count(self, text):
                return len(text) // 4

        self.root = root
        self.files = [os.path.join(root, f) for f in files]
        self.rm = RepoMap(map_tokens=4096, root=root, main_model=Tokens(), io=InputOutput(yes=True, pretty=False))

    def rank(self, prompt):
        idents = set(re.split(r"\W+", prompt)) - {""}
        mentioned = {f for f in self.files if os.path.basename(f) in prompt}
        tags = self.rm.get_ranked_tags([], self.files, mentioned, idents)
        seen, order = set(), []
        for tag in tags:
            rel = tag[0] if isinstance(tag, tuple) else tag.rel_fname
            rel = rel.replace("\\", "/")
            if rel not in seen:
                seen.add(rel)
                order.append(rel)
        return order


def score(ranked, gold, k):
    top = [f.replace("\\", "/") for f in ranked[:k]]
    hits = sum(1 for g in gold if any(t.endswith(g) for t in top))
    return hits / len(gold), (hits / len(top) if top else 0.0)


def main():
    gold_path, root = sys.argv[1], sys.argv[2]
    tasks = load_gold(gold_path)
    files = repo_files(root)
    rankers = {"bm25": Bm25(root, files)}
    if "--aider" in sys.argv:
        rankers["aider-repomap"] = AiderRank(root, files)
    for name, ranker in rankers.items():
        tot = {k: [0.0, 0.0] for k in (1, 3, 5)}
        for _, prompt, gold in tasks:
            ranked = ranker.rank(prompt)
            for k in tot:
                r, p = score(ranked, gold, k)
                tot[k][0] += r
                tot[k][1] += p
        n = len(tasks)
        cells = "  ".join(f"R@{k} {tot[k][0] / n:.3f} P@{k} {tot[k][1] / n:.3f}" for k in (1, 3, 5))
        print(f"{name:<14} {n} q  {cells}")


if __name__ == "__main__":
    main()
