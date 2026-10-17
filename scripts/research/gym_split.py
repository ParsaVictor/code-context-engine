"""Second tuning split from SWE-Gym (session 19, phase B) — pre-registered.

SWE-bench dev (225 issues, 6 repositories) is too small to separate +0.02 effects
(one issue in dev-fast is 0.017), and its function gold is unlike Lite's (one
edited function in 45% of dev issues vs 86% of Lite). SWE-Gym (Pan et al. 2024,
2438 issues) uses 11 repositories disjoint from SWE-bench Lite/Verified/dev.

Selection, fixed before any engine run on it:
  repositories  pydantic, hydra, conan, dvc, dask, moto, mypy (the four largest
                checkouts — pandas, MONAI, modin, bokeh — left out for disk/IO)
  instances     patch edits only existing non-test .py files (no new files)
  sample        at most 40 per repository, random.Random(19) shuffle of sorted ids
  halves        gym-a / gym-b alternate within each repository's sample
Decision bar (same as dev): file Acc@1 or func Acc@5 >= +0.02 on BOTH halves;
an idea that was a near miss on dev is kept only if it also clears it here.

    py -3 gym_split.py --parquet gym/gym.parquet --out-dir devset
"""
import argparse
import json
import os
import random
import re

REPOS = ["pydantic/pydantic", "facebookresearch/hydra", "conan-io/conan", "iterative/dvc",
         "dask/dask", "getmoto/moto", "python/mypy"]
CAP = 40
SEED = 19


def eligible(patch):
    files = re.findall(r"^diff --git a/(\S+) b/(\S+)", patch, re.M)
    if not files or "\nnew file mode" in patch or "\ndeleted file mode" in patch:
        return False
    return all(a.endswith(".py") and "test" not in a.lower() for a, _ in files)


def main():
    import pandas as pd
    ap = argparse.ArgumentParser()
    ap.add_argument("--parquet", required=True)
    ap.add_argument("--out-dir", required=True)
    args = ap.parse_args()
    df = pd.read_parquet(args.parquet)
    a, b = [], []
    for repo in REPOS:
        rows = [r for r in df[df.repo == repo].to_dict("records") if eligible(r["patch"])]
        rows.sort(key=lambda r: r["instance_id"])
        random.Random(SEED).shuffle(rows)
        for i, r in enumerate(rows[:CAP]):
            r = {k: (v.tolist() if hasattr(v, "tolist") else v) for k, v in r.items()}
            (a if i % 2 == 0 else b).append(r)
        print(f"{repo:28s} eligible {len(rows):4d} taken {min(CAP, len(rows))}")
    for name, part in (("gym-a", a), ("gym-b", b), ("gym-all", a + b)):
        json.dump(part, open(os.path.join(args.out_dir, name + ".json"), "w", encoding="utf-8"))
        print(name, len(part))


if __name__ == "__main__":
    main()
