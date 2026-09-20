# Self dogfood set (session 13)

Ten real questions asked about this repository itself, with the files the
author of the question expected. Not a ratchet and not in CI: it is the
batch-probe input for dogfood findings (F73, F74). Run it as the "private"
set pointed at this checkout:

```bash
NM_THIRD_PARTY=1 NM_PRIVATE_SET_DIR=tests/third_party/self NM_PRIVATE_DIR=.. \
  NM_EXPLAIN=1 NM_EXPLAIN_MAX_PRECISION=1.01 NM_INDEX_CACHE=0 \
  cargo test -p neuromesh-context --test third_party_private_gold -- --nocapture
```

(`NM_PRIVATE_DIR/<name>` must be this checkout; with `name = "repo"` and the
checkout at `../repo` that is `NM_PRIVATE_DIR=..`.)

Dev-class by construction: every fix probed here was tuned on it.
