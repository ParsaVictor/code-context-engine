"""Learned (LLM-free) reranking of the engine's function list — session 19 prototype.

Ordering is the largest function-level failure on dev (func_miss.py: 45% of
instances have every gold function in the 300-entry list but not in the top 5;
median rank 36). This reranks the top-N candidates with a small linear model over
features read from the stored engine output and the issue text only (no checkout):
engine rank and score, file rank, rank within the file, mentions of the name /
class / file in the issue, traceback frames, name-token overlap, length, kind.

Protocol (fixed before the first measurement): train on one dev half, score the
other, both directions; kept only if func Acc@5 (or file Acc@1, untouched here)
rises by >= 0.02 on BOTH halves. `--apply` trains on all of dev and writes a
reranked results file for a once-per-version holdout run.

    py -3 func_ltr.py --results dev-s18a-all.jsonl --repos repos --gold-cache func-gold-dev.json \
        --fast devset/dev-fast.json --rest devset/dev-rest.json [--top 100] [--model logreg|gbt]
"""
import argparse
import json
import math
import os
import re
import sys
from collections import Counter, defaultdict

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
from func_eval import boot_ci, load_gold, same  # noqa: E402

WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
FRAME = re.compile(r'File "([^"]+)", line (\d+), in ([A-Za-z_][A-Za-z0-9_<>]*)')


def split_ident(s):
    parts = []
    for w in re.split(r"[_\W]+", s):
        parts += re.findall(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+", w)
    return [p.lower() for p in parts if len(p) > 1]


def issue_view(text):
    words = WORD.findall(text)
    wset = set(words)
    lower = Counter(w.lower() for w in words)
    toks = Counter(t for w in words for t in split_ident(w))
    frames = FRAME.findall(text)
    frame_funcs = [f[2] for f in frames]
    frame_files = [f[0].replace("\\", "/") for f in frames]
    dotted = set(re.findall(r"[A-Za-z_][\w]*(?:\.[A-Za-z_]\w*)+", text))
    return dict(words=wset, lower=lower, toks=toks, frame_funcs=frame_funcs,
                frame_files=frame_files, dotted=dotted, text=text, ntok=sum(toks.values()) or 1)


def path_in(p, frame_files):
    return any(f.endswith("/" + p) or f.endswith(p) or p.endswith(f.split("site-packages/")[-1])
               for f in frame_files)


FEATS = ["lrank", "rrank", "score_rel", "frank", "ftop1", "fabsent", "inrank", "lininrank",
         "name_exact", "name_lower", "cls_exact", "qual_dotted", "file_base", "frame_func",
         "frame_file", "tok_overlap", "tok_idf", "llen", "is_test", "is_private", "is_dunder",
         "is_init", "is_method", "file_share20", "nm_frame_innermost"]


def candidates(r, iv, top, df):
    defs = r.get("definitions", [])[:top]
    files = r.get("ours_files", [])
    if not defs:
        return [], []
    smax = max(d.get("score", 0) for d in defs) or 1.0
    per_file = Counter()
    share20 = Counter(d["path"] for d in defs[:20])
    rows, keys = [], []
    innermost = iv["frame_funcs"][-1] if iv["frame_funcs"] else None
    for i, d in enumerate(defs):
        p, n = d["path"], d["name"]
        last = n.split(".")[-1]
        cls = n.split(".")[0] if "." in n else ""
        fr = next((j for j, f in enumerate(files) if f == p or f.endswith("/" + p) or p.endswith("/" + f)), None)
        within = per_file[p]
        per_file[p] += 1
        base = os.path.splitext(os.path.basename(p))[0]
        ntoks = split_ident(n)
        hit = [t for t in ntoks if t in iv["toks"]]
        idf = sum(math.log(1 + 300 / (1 + df.get(t, 0))) for t in hit) / (1 + len(ntoks))
        lines = d.get("lines") or [0, 0]
        f = dict(
            lrank=math.log1p(i), rrank=1.0 / (1 + i), score_rel=d.get("score", 0) / smax,
            frank=math.log1p(fr) if fr is not None else math.log1p(30), ftop1=float(fr == 0),
            fabsent=float(fr is None), inrank=1.0 / (1 + within), lininrank=math.log1p(within),
            name_exact=float(last in iv["words"]), name_lower=math.log1p(iv["lower"].get(last.lower(), 0)),
            cls_exact=float(bool(cls) and cls in iv["words"]),
            qual_dotted=float(any(x.endswith(n) or x.endswith("." + last) for x in iv["dotted"])),
            file_base=float(base in iv["words"] and base != "__init__"),
            frame_func=float(last in iv["frame_funcs"]), frame_file=float(path_in(p, iv["frame_files"])),
            tok_overlap=len(hit) / (1 + len(ntoks)), tok_idf=idf,
            llen=math.log1p(max(0, lines[1] - lines[0])),
            is_test=float("test" in p.lower()), is_private=float(last.startswith("_") and not last.startswith("__")),
            is_dunder=float(last.startswith("__")), is_init=float(last == "__init__"),
            is_method=float("." in n), file_share20=share20[p] / 20.0,
            nm_frame_innermost=float(innermost == last),
        )
        rows.append([f[k] for k in FEATS])
        keys.append((p, n))
    return rows, keys


def label(keys, gold):
    return [int(any(same(k, g) for g in gold)) for k in keys]


def acc_at(order_keys, gold, k):
    top = order_keys[:k]
    return int(all(any(same(p, g) for p in top) for g in gold))


def build(res, issues, golds, top, df):
    data = []
    for r in res:
        gold = golds.get(r["instance_id"])
        if not gold:
            continue
        iv = issue_view(issues[r["instance_id"]])
        X, keys = candidates(r, iv, top, df)
        data.append(dict(iid=r["instance_id"], X=np.array(X) if X else np.zeros((0, len(FEATS))),
                         keys=keys, y=label(keys, gold) if keys else [], gold=gold,
                         full=[(d["path"], d["name"]) for d in r.get("definitions", [])]))
    return data


def fit(train, kind, seed=0):
    X = np.vstack([d["X"] for d in train if len(d["keys"])])
    y = np.concatenate([d["y"] for d in train if len(d["keys"])])
    mu, sd = X.mean(0), X.std(0) + 1e-9
    if kind == "gbt":
        from sklearn.ensemble import HistGradientBoostingClassifier
        m = HistGradientBoostingClassifier(max_iter=200, learning_rate=0.05, max_leaf_nodes=15,
                                           l2_regularization=1.0, random_state=seed)
        m.fit(X, y)
        return lambda Z: m.predict_proba(Z)[:, 1]
    from sklearn.linear_model import LogisticRegression
    m = LogisticRegression(C=0.3, class_weight="balanced", max_iter=2000)
    m.fit((X - mu) / sd, y)
    fit.coef = dict(zip(FEATS, m.coef_[0]))
    return lambda Z: m.decision_function((Z - mu) / sd)


def rerank(d, scorer):
    if not d["keys"]:
        return d["full"]
    s = scorer(d["X"])
    order = sorted(range(len(d["keys"])), key=lambda i: (-s[i], i))
    head = [d["keys"][i] for i in order]
    return head + d["full"][len(head):]


def report(name, data, lists):
    out = {}
    for k in (1, 5, 10):
        v = [acc_at(l, d["gold"], k) for d, l in zip(data, lists)]
        out[k] = sum(v) / max(len(v), 1)
    print(f"  {name:28s} n={len(data):3d}  func@1 {out[1]:.3f}  @5 {out[5]:.3f}  @10 {out[10]:.3f}")
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--repos", required=True)
    ap.add_argument("--gold-cache", default="")
    ap.add_argument("--fast", required=True)
    ap.add_argument("--rest", required=True)
    ap.add_argument("--top", type=int, default=100)
    ap.add_argument("--model", default="logreg")
    ap.add_argument("--apply", default="", help="holdout results jsonl to rerank (train on all dev)")
    ap.add_argument("--apply-data", default="")
    ap.add_argument("--out", default="")
    args = ap.parse_args()
    halves = {}
    rows = {}
    for nm, path in (("fast", args.fast), ("rest", args.rest)):
        rr = json.load(open(path, encoding="utf-8"))
        halves[nm] = {r["instance_id"] for r in rr}
        rows.update({r["instance_id"]: r for r in rr})
    issues = {i: r["problem_statement"] for i, r in rows.items()}
    res = [json.loads(l) for l in open(args.results, encoding="utf-8") if l.strip()]
    res = [r for r in res if "error" not in r and r["instance_id"] in rows]
    golds = load_gold(rows, [r["instance_id"] for r in res], args.repos, args.gold_cache)
    df = Counter()
    for t in issues.values():
        df.update(set(x for w in WORD.findall(t) for x in split_ident(w)))
    data = build(res, issues, golds, args.top, df)
    parts = {h: [d for d in data if d["iid"] in halves[h]] for h in halves}
    print(f"model={args.model} top={args.top}")
    deltas = {}
    for test, train in (("fast", "rest"), ("rest", "fast")):
        print(f"[test {test}, train {train}]")
        base = report("engine", parts[test], [d["full"] for d in parts[test]])
        sc = fit(parts[train], args.model)
        new = report("reranked", parts[test], [rerank(d, sc) for d in parts[test]])
        deltas[test] = {k: new[k] - base[k] for k in new}
    for h, dd in deltas.items():
        print(f"  delta {h}: " + "  ".join(f"@{k} {v:+.3f}" for k, v in dd.items()))
    ok = all(dd[5] >= 0.02 for dd in deltas.values())
    print("BAR (func@5 >= +0.02 both halves):", "PASS" if ok else "fail")
    sc = fit(data, args.model)
    if args.model == "logreg":
        print("coef (all dev): " + ", ".join(f"{k} {v:+.2f}" for k, v in sorted(
            fit.coef.items(), key=lambda kv: -abs(kv[1]))))
    if args.apply:
        arows = {r["instance_id"]: r for r in json.load(open(args.apply_data, encoding="utf-8"))}
        ares = [json.loads(l) for l in open(args.apply, encoding="utf-8") if l.strip()]
        with open(args.out, "w", encoding="utf-8") as fo:
            for r in ares:
                if "error" not in r and r.get("definitions") and r["instance_id"] in arows:
                    iv = issue_view(arows[r["instance_id"]]["problem_statement"])
                    X, keys = candidates(r, iv, args.top, df)
                    d = dict(X=np.array(X), keys=keys, full=[(x["path"], x["name"]) for x in r["definitions"]])
                    by = {(x["path"], x["name"]): x for x in r["definitions"]}
                    r["definitions_ltr"] = [by[k] for k in rerank(d, sc)]
                fo.write(json.dumps(r) + "\n")
        print("wrote", args.out)


if __name__ == "__main__":
    main()
