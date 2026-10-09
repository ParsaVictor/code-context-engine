"""Offline prototypes of LLM-free priors for issue localisation (session 18, phase 1).

Each prior reads the issue text (and, where it needs one, the repository at the
issue's base commit — never anything later) and returns ranked votes:
files and/or functions. The votes are fused by RRF into the engine's stored
lists (`loc_files`, `definitions` from `swebench_localize.py`), and file- and
function-level Acc@k are printed per half of the SWE-bench dev split, so a prior
is kept only if it helps on both halves.

    py -3 scripts/research/priors.py --results dev-func300-all.jsonl --data devset/swe-dev.json
        --repos repos --prior traceback [--weights 0.5,1,2,4] [--gold-cache func-gold-dev.json]

Priors:
  codeonly   function list without non-code "definitions" (YAML/JSON/TOML keys)
  traceback  frames `File "x.py", line N, in f` → the function at base commit (name, then line)
  repro      identifiers called / imported in the issue's code blocks → their definitions
  history    BugLocator: past commits (ancestors of base) whose message is like the issue
             → the files they changed
  testlink   tests the issue names → modules those tests import
"""
import argparse
import ast
import json
import math
import os
import re
import subprocess
import sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(__file__))
from eval_loc import acc  # noqa: E402
from func_eval import boot_ci, defs_with_names, git_show, load_gold, same  # noqa: E402
from mention_prior import resolve  # noqa: E402

CODE_EXT = (".py", ".pyx", ".pyi")


# ---------------------------------------------------------------- repository access

class Repo:
    """Read-only view of a clone at one commit (object store only, no checkout)."""

    _trees = {}
    _shows = {}

    def __init__(self, clone, commit):
        self.clone, self.commit = clone, commit

    def files(self):
        key = (self.clone, self.commit)
        if key not in Repo._trees:
            out = subprocess.run(["git", "-C", self.clone, "ls-tree", "-r", "--name-only", self.commit],
                                 capture_output=True, text=True, encoding="utf-8", errors="ignore").stdout
            Repo._trees[key] = out.splitlines()
        return Repo._trees[key]

    def py_files(self):
        return [f for f in self.files() if f.endswith(".py")]

    def show(self, path):
        key = (self.clone, self.commit, path)
        if key not in Repo._shows:
            Repo._shows[key] = git_show(self.clone, self.commit, path)
        return Repo._shows[key]

    def defs(self, path):
        return defs_with_names(self.show(path))


def by_name_index(files):
    idx = {}
    for f in files:
        if "/test" in "/" + f or f.startswith("test"):
            continue
        idx.setdefault(f.rsplit("/", 1)[-1], []).append(f)
    return idx


def is_test(path):
    p = "/" + path
    return "/test" in p or "/tests/" in p or path.rsplit("/", 1)[-1].startswith("test_")


# ---------------------------------------------------------------- issue text

FENCE = re.compile(r"```[^\n]*\n(.*?)```", re.S)


def code_blocks(text):
    """Fenced blocks, doctest lines and 4-space indented runs."""
    blocks = FENCE.findall(text)
    rest = FENCE.sub("", text)
    doc = [l.strip()[4:] for l in rest.splitlines() if l.strip().startswith((">>> ", "... "))]
    if doc:
        blocks.append("\n".join(doc))
    indented, cur = [], []
    for line in rest.splitlines():
        if line.startswith(("    ", "\t")) and line.strip():
            cur.append(line.strip())
        else:
            if len(cur) >= 2:
                indented.append("\n".join(cur))
            cur = []
    if len(cur) >= 2:
        indented.append("\n".join(cur))
    return blocks + indented


# ---------------------------------------------------------------- priors

def prior_codeonly(row, res, repo):
    defs = [(d["path"], d["name"]) for d in res.get("definitions", [])]
    return None, [d for d in defs if d[0].endswith(CODE_EXT)], "replace"


FRAME = re.compile(r'File "([^"]+\.py)", line (\d+), in ([\w<>.]+)')
PYTEST_FRAME = re.compile(r"^([\w./\\-]+\.py):(\d+):(?: in ([\w<>.]+))?", re.M)


def enclosing(defs, line, name=None):
    """Innermost def containing `line`; with `name`, prefer defs of that name."""
    named = [d for d in defs if name and d[2].split(".")[-1] == name]
    if named:
        inside = [d for d in named if d[0] <= line <= d[1]]
        pick = min(inside or named, key=lambda d: (abs(d[0] - line) if not inside else d[1] - d[0]))
        return pick[2]
    inside = [d for d in defs if d[0] <= line <= d[1]]
    return min(inside, key=lambda d: d[1] - d[0])[2] if inside else None


def prior_traceback(row, res, repo):
    text = row["problem_statement"]
    frames = [(p, int(n), f) for p, n, f in FRAME.findall(text)]
    frames += [(p, int(n), f or None) for p, n, f in PYTEST_FRAME.findall(text)]
    if not frames:
        return [], [], "fuse"
    files = repo.py_files()
    idx = by_name_index(files)
    out_files, out_funcs = [], []
    for path, line, func in reversed(frames):  # deepest first
        f = resolve(path, files, idx)
        if not f or is_test(f):
            continue
        if f not in out_files:
            out_files.append(f)
        name = enclosing(repo.defs(f), line, None if func in (None, "<module>") else func)
        if name and (f, name) not in out_funcs:
            out_funcs.append((f, name))
    return out_files, out_funcs, "fuse"


CALL = re.compile(r"(?<![\w.])(\w+)\s*\(|\.(\w+)\s*\(|\.(\w+)\b")
IMPORT = re.compile(r"^\s*from\s+([\w.]+)\s+import\s+\(?([\w,\s]+)\)?|^\s*import\s+([\w.]+)", re.M)
STOP = set("print len str int float list dict set tuple range open isinstance type super self cls "
           "append extend items keys values get format join split strip shape dtype copy "
           "array arange zeros ones linspace DataFrame Series plot show figure subplots "
           "assert raise return import from".split())


def module_file(mod, files):
    p = mod.replace(".", "/")
    for cand in (p + ".py", p + "/__init__.py"):
        hits = [f for f in files if f == cand or f.endswith("/" + cand)]
        if len(hits) == 1:
            return hits[0]
        if hits:
            return min(hits, key=len)
    return None


def prior_repro(row, res, repo):
    blocks = code_blocks(row["problem_statement"])
    if not blocks:
        return [], [], "fuse"
    code = "\n".join(blocks)
    files = repo.py_files()
    out_files, names = [], []
    for m in IMPORT.finditer(code):
        mod = m.group(1) or m.group(3)
        f = module_file(mod, files)
        if f and not is_test(f) and f not in out_files:
            out_files.append(f)
        for n in (m.group(2) or "").replace("\n", " ").split(","):
            n = n.strip().split(" as ")[0].strip()
            if n:
                names.append(n)
                sub = module_file(mod + "." + n, files)
                if sub and not is_test(sub) and sub not in out_files:
                    out_files.append(sub)
    for m in CALL.finditer(code):
        n = m.group(1) or m.group(2) or m.group(3)
        if n and n not in STOP and not n.startswith("__") and len(n) > 2:
            names.append(n)
    counts = Counter(names)
    # Definitions with those names, among the engine's deep list (the engine port
    # looks them up in its own symbol table).
    defs = [(d["path"], d["name"]) for d in res.get("definitions", []) if d["path"].endswith(CODE_EXT)]
    by_last = defaultdict(list)
    for p, n in defs:
        by_last[n.split(".")[-1]].append((p, n))
    out_funcs = []
    for n, _ in sorted(counts.items(), key=lambda kv: -kv[1]):
        for d in by_last.get(n, [])[:3]:
            if d not in out_funcs and not is_test(d[0]):
                out_funcs.append(d)
    for p, _ in out_funcs:
        if p not in out_files:
            out_files.append(p)
    return out_files, out_funcs, "fuse"


WORD = re.compile(r"[A-Za-z][A-Za-z0-9]+")


def toks(text):
    out = []
    for w in WORD.findall(text):
        for part in re.sub(r"([a-z0-9])([A-Z])", r"\1 \2", w).split():
            p = part.lower()
            if len(p) > 2:
                out.append(p)
    return out


class History:
    """Commit log of one clone: message tokens and changed .py files per commit.
    `--no-renames`: rename detection reads blobs, which a blobless clone fetches one
    by one over the network (it returned 214 of 1,878 pvlib commits in 53 s)."""

    _cache = {}

    @classmethod
    def of(cls, clone):
        if clone not in cls._cache:
            cls._cache[clone] = cls(clone)
        return cls._cache[clone]

    def __init__(self, clone):
        out = subprocess.run(["git", "-C", clone, "log", "--all", "--no-merges", "--no-renames", "--format=%x01%H%x02%s%n%b%x03",
                              "--name-only"], capture_output=True, text=True, encoding="utf-8",
                             errors="ignore").stdout
        self.commits = {}
        for chunk in out.split("\x01")[1:]:
            head, _, rest = chunk.partition("\x02")
            msg, _, names = rest.partition("\x03")
            fs = [l.strip() for l in names.splitlines() if l.strip().endswith(".py")]
            self.commits[head.strip()] = (Counter(toks(msg)), fs)
        self.clone = clone

    def ancestors(self, commit):
        out = subprocess.run(["git", "-C", self.clone, "rev-list", commit], capture_output=True,
                             text=True).stdout
        return [h for h in out.split() if h in self.commits]


def prior_history(row, res, repo, top=20, max_files=10):
    hist = History.of(repo.clone)
    anc = hist.ancestors(repo.commit)
    docs = [(h, hist.commits[h]) for h in anc if 0 < len(hist.commits[h][1]) <= max_files]
    if not docs:
        return [], [], "fuse"
    n = len(docs)
    df = Counter()
    for _, (c, _) in docs:
        df.update(c.keys())
    avg = sum(sum(c.values()) for _, (c, _) in docs) / n
    q = Counter(toks(row["problem_statement"]))
    scored = []
    for h, (c, fs) in docs:
        ln = sum(c.values())
        s = 0.0
        for w, qc in q.items():
            t = c.get(w)
            if t:
                idf = math.log((n - df[w] + 0.5) / (df[w] + 0.5) + 1)
                s += idf * t * 2.2 / (t + 1.2 * (0.25 + 0.75 * ln / max(avg, 1)))
        if s > 0:
            scored.append((s, h, fs))
    scored.sort(reverse=True)
    present = set(repo.files())
    votes = defaultdict(float)
    for s, h, fs in scored[:top]:
        for f in fs:
            if f in present and not is_test(f):
                votes[f] += s / len(fs)
    return [f for f, _ in sorted(votes.items(), key=lambda kv: -kv[1])], [], "fuse"


TESTNAME = re.compile(r"\b(test_\w+)\b")
TESTPATH = re.compile(r"[\w./\\-]*tests?[\w./\\-]*\.py\b")


def prior_testlink(row, res, repo):
    text = row["problem_statement"]
    files = repo.py_files()
    tests = []
    for tok in TESTPATH.findall(text):
        f = resolve(tok, files, {})
        if f and is_test(f) and f not in tests:
            tests.append(f)
    names = set(TESTNAME.findall(text))
    if names and not tests:
        # A test named without its file: test modules defining it (bounded scan).
        for f in files:
            if is_test(f) and f.rsplit("/", 1)[-1].startswith("test"):
                src = repo.show(f)
                if any(f"def {n}(" in src for n in names):
                    tests.append(f)
                    if len(tests) >= 3:
                        break
    out = []
    for t in tests:
        try:
            tree = ast.parse(repo.show(t))
        except (SyntaxError, ValueError):
            continue
        for node in ast.walk(tree):
            mod = None
            if isinstance(node, ast.ImportFrom) and node.module:
                mod = node.module
                for a in node.names:
                    sub = module_file(mod + "." + a.name, files)
                    if sub and not is_test(sub) and sub not in out:
                        out.append(sub)
            elif isinstance(node, ast.Import):
                mod = node.names[0].name
            if mod:
                f = module_file(mod, files)
                if f and not is_test(f) and f not in out and not f.endswith("__init__.py"):
                    out.append(f)
    return out, [], "fuse"


_EXT = {}


def prior_ext(row, res, repo):
    """Lists another script stored (`--ext file.jsonl:files_key:defs_key`)."""
    path, fkey, dkey = EXT_SPEC
    if path not in _EXT:
        _EXT[path] = {}
        for line in open(path, encoding="utf-8"):
            if line.strip():
                r = json.loads(line)
                if "error" not in r:
                    _EXT[path][r["instance_id"]] = r
    r = _EXT[path].get(row["instance_id"], {})
    files = r.get(fkey, []) if fkey else []
    defs = [tuple(d) for d in r.get(dkey, [])] if dkey else []
    return files, defs, "fuse"


EXT_SPEC = ("", "", "")
PRIORS = {"ext": prior_ext, "codeonly": prior_codeonly, "traceback": prior_traceback, "repro": prior_repro,
          "history": prior_history, "testlink": prior_testlink}


# ---------------------------------------------------------------- scoring

def rrf(lists, weights, k=60):
    s = {}
    for lst, w in zip(lists, weights):
        for i, f in enumerate(lst):
            s[f] = s.get(f, 0.0) + w / (k + i + 1)
    return [f for f, _ in sorted(s.items(), key=lambda kv: -kv[1])]


def func_hit(preds, gold, k):
    top = preds[:k]
    return int(all(any(same(p, g) for p in top) for g in gold))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--prior", required=True, choices=sorted(PRIORS))
    ap.add_argument("--weights", default="0.5,1,2,4")
    ap.add_argument("--gold-cache", default="")
    ap.add_argument("--halves", default="devset/dev-fast.json,devset/dev-rest.json")
    ap.add_argument("--dump", default="", help="write per-instance votes (jsonl) for the engine port check")
    ap.add_argument("--ci", action="store_true")
    ap.add_argument("--within", type=int, default=0,
                    help="keep only prior votes for files/functions already in the base top N (rerank, no new candidates)")
    ap.add_argument("--ext", default="", help="file.jsonl:files_key:defs_key for --prior ext")
    args = ap.parse_args()
    if args.ext:
        global EXT_SPEC
        parts = args.ext.split(":")
        EXT_SPEC = (":".join(parts[:-2]), parts[-2], parts[-1])

    rows = {r["instance_id"]: r for r in json.load(open(args.data, encoding="utf-8"))}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if "error" not in r and r["instance_id"] in rows]
    golds = load_gold(rows, [r["instance_id"] for r in res], args.repos, args.gold_cache)
    halves = []
    for h in args.halves.split(","):
        ids = {r["instance_id"] for r in json.load(open(h, encoding="utf-8"))}
        halves.append((os.path.basename(h).replace(".json", ""), ids))
    halves.append(("all", {r["instance_id"] for r in res}))

    prior = PRIORS[args.prior]
    weights = [float(w) for w in args.weights.split(",")]
    variants = {"base": {}, **{f"w={w:g}": {} for w in weights}}
    fired = 0
    dump = open(args.dump, "w", encoding="utf-8") if args.dump else None
    for r in res:
        row = rows[r["instance_id"]]
        repo = Repo(os.path.join(args.repos, row["repo"].split("/")[1]), row["base_commit"])
        files = r.get("loc_files") or []
        funcs = [(d["path"], d["name"]) for d in r.get("definitions", [])]
        pf, pfun, mode = prior(row, r, repo)
        if args.within and mode == "fuse":
            top_f, top_fun = set(files[:args.within]), set(funcs[:args.within])
            pf = [f for f in pf if f in top_f]
            pfun = [f for f in pfun if f in top_fun]
        if pf or pfun:
            fired += 1
        if dump:
            dump.write(json.dumps({"instance_id": r["instance_id"], "files": pf, "funcs": pfun}) + "\n")
        variants["base"][r["instance_id"]] = (files, funcs)
        for w in weights:
            if mode == "replace":
                nf = files if pf is None else pf
                nfun = funcs if pfun is None else pfun
            else:
                nf = rrf([files, pf], [1.0, w]) if pf else files
                nfun = rrf([funcs, pfun], [1.0, w]) if pfun else funcs
            variants[f"w={w:g}"][r["instance_id"]] = (nf, nfun)
    print(f"prior {args.prior}: fired on {fired} of {len(res)}")
    for name, ids in halves:
        fg = [i for i in ids if i in variants["base"]]
        fung = [i for i in fg if golds.get(i)]
        print(f"-- {name}: {len(fg)} issues, {len(fung)} with function gold")
        for v, lists in variants.items():
            fa = {k: [acc(lists[i][0], rows_gold(r_by_id(res, i)), k) for i in fg] for k in (1, 3, 5, 10)}
            fu = {k: [func_hit(lists[i][1], golds[i], k) for i in fung] for k in (1, 5, 10)}
            cell = "  ".join(f"@{k} {sum(fa[k]) / max(len(fg), 1):.3f}" for k in (1, 3, 5, 10))
            cell += "  | func " + "  ".join(f"@{k} {sum(fu[k]) / max(len(fung), 1):.3f}" for k in (1, 5, 10))
            if args.ci and v == "base":
                lo, hi = boot_ci(fa[1])
                cell += f"  (file@1 CI [{lo:.2f},{hi:.2f}])"
            print(f"   {v:8s} {cell}")


_BY_ID = {}


def r_by_id(res, iid):
    if not _BY_ID:
        _BY_ID.update({r["instance_id"]: r for r in res})
    return _BY_ID[iid]


def rows_gold(r):
    return [g.replace("\\", "/") for g in r["gold"]]


if __name__ == "__main__":
    main()
