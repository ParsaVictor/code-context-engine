# What we measured — and what we did not

Every number here comes from one command on a pinned checkout:

```bash
bash scripts/benchmark-fast.sh             # the nine public sets, optimised and in parallel (~1.5 min warm)
bash scripts/benchmark-holdout.sh          # same numbers, debug build, one set at a time (~40 min)
bash scripts/benchmark-holdout.sh holdout  # one set
```

The rule behind the table: **a number from a repository the engine was tuned on is
not the project's number.** Only the holdout rows are.

## Gold sets (2026-10-01, v1.1.0)

| set | repos | role | recall | precision | forbidden | oracle reachable / strict |
|---|---|---|---|---|---|---|
| dev-4 | nanoGPT, express, vit-pytorch, full-stack-fastapi | tuning set | 1.000 | 0.938 | 0 | 21/21 · 20 |
| large | django (3.5k files), ultralytics | tuning set, large | 1.000 | 0.675 | 0 | 20/20 · 20 |
| **holdout-2** | gin (Go), torchvision (Python) | never tuned on | **1.000** | **0.700** | **0** | 20/20 · 18 |
| **holdout-c** | libuv (C), fmt (C++) | never tuned on | **1.000** | **0.589** | **0** | 16/16 · 14 |
| **holdout-lang** | os-lib (Scala), r-lib/cli (R), Flux.jl (Julia) | never tuned on | **1.000** | **0.589** | **1** | 14/15 · 12 |
| holdout-ml | keras-io examples (Keras), setfit (Hugging Face) | **tuned on in D-3** (5 iterations read its numbers) — dev-class since then; fresh ML holdout is `holdout-ml2` | **1.000** | **0.478** | **0** | 10/10 · 10 |
| **holdout-ml2** | peft (Hugging Face adapter library), keras-hub (Keras 3 model library) | never tuned on; first run after gold lock (session 12) | **1.000** | **0.632** | **0** | 10/10 · 6 |
| **holdout-cfg** | lightning-hydra-template (Hydra YAML), detr (argparse) | D-4 gold; F55 (session 12) + F71/F72 (session 13) tuned on it — dev-class for config questions | **1.000** | **0.767** | **0** | — |
| holdout-web (30 q) | fastify/demo (Fastify API), shadcn-ui/taxonomy (Next.js app router) | dev-class for the web domain (fixed on since session 13; 10 blind questions added in G4 scored 0.58 before fixes) | **0.917** | **0.643** | **0** | — |
| **private** | one closed-source B2B backend+frontend (Fastify/Drizzle + Next.js, ~1.2k files) | never tuned on; gold and checkout live outside this repo | **1.000** | **0.587** | **0** | — |
| concept (14 q) | this repository, plain-language questions (no identifier in the prompt) | dev-class (written 2026-09-30 from the upstream author's report, tuned on in session 16) | 0.679 | 0.392 | 0 | — |
| **concept-holdout** (12 q) | ripgrep 14.1.1 (Rust), plain-language questions | never tuned on; gold locked before any run; 1.0.0 scored recall **0.042** | **0.500** | **0.156** | **0** | — |
| **concept-holdout2** (12 q) | click 8.1.7 (Python, 16 source files), plain-language questions | never tuned on; gold locked before the single run (2026-10-01) | **0.958** | **0.342** | **0** | — |
| concept-holdout3 (12 q) | cobra v1.8.1 (Go), plain-language questions | gold locked before its single run (2026-10-09); looked at since | 0.833 | 0.357 | 0 | — |
| concept-holdout4 (12 q) | axios v1.7.7 (JavaScript), plain-language questions | gold locked before its single run (2026-10-09); looked at since | 0.667 | 0.528 | 0 | — |
| **concept-holdout5** (12 q) | jsoup 1.18.1 (Java), plain-language questions | gold locked before its single run (2026-10-10); list R@3 0.750 vs BM25 0.667 (pre-S1 v1.2.0: 0.417) | **0.667** | **0.235** | **0** | — |

- **recall / precision** are file-level against a hand-written gold (`gold_files`) per question.
  A forbidden file in the packet zeroes that question's precision.
- **oracle** is symbol-level: *reachable* = every needed symbol is in the packet, unfolded or one
  fold-expansion away; *strict* = unfolded as shipped. It is a sufficiency check, not a model run.
- Holdout gold is written by reading the source **before any engine run** and never edited
  afterwards, even when a choice turned out to be arguable. Every `forbidden` file was checked
  by grep to be neither imported by nor called from the gold files.

### How to read the precision gap

Holdout golds are mostly **single-file** ("the file that defines X"); the engine ships that file
plus 1–4 neighbours (callees, importers, same-package siblings). Recall says the right file is
almost always there; precision says the neighbourhood is usually larger than a single-file gold
wants. Three attempts to tighten the neighbourhood by score were rejected because they broke the
tuning sets, whose golds *do* want the neighbours — see `docs/planning/stage5-findings.fa.md`
(F36′, F46, F47). Per-language, the gap is widest on flat package namespaces (R: 0.24) and
narrowest where a question names one object (Scala: 0.80).

## SWE-bench Lite file localisation (2026-10-09, v1.2.0, single holdout run)

Question = the issue text as filed; gold = the files the reference patch edits; metric = file-level
Acc@k (every gold file in the top k), as LocAgent reports it. The engine's list is `localization` in
`neuromesh packet --json` (`where_to_look` over MCP). No LLM, no embeddings, CPU only; p50 2.8 s per
issue including a cold index of the checkout.

Tuning used only the SWE-bench **dev** split (225 issues, six other repositories). Lite test was run
once with the release binary. The 24 Lite instances from flask/requests/seaborn/xarray/pylint were
looked at in session 16 and are excluded from the strict row.

| Lite test | Acc@1 | Acc@3 | Acc@5 | Acc@10 |
|---|---|---|---|---|
| **this engine, strict holdout (276)** | **0.486** | **0.710** | **0.750** | **0.804** |
| this engine, all 300 | 0.473 | 0.703 | 0.747 | 0.810 |
| plain BM25 over the checkout (276) | 0.301 | 0.507 | 0.587 | 0.721 |
| 1.1.0 + issue mode, packet order (258, session 16) | 0.337 | 0.504 | 0.535 | – |

95% bootstrap intervals for the strict row: @1 [0.43, 0.55], @3 [0.65, 0.76], @5 [0.70, 0.80].

Published reference points (LocAgent, arXiv 2503.09089, Table 4; their filtered Lite subset, not the
same instances, so read as context): BM25 0.387 / 0.518 / 0.617; Jina-Code-v2 embeddings 0.434 /
0.712 / 0.803; CodeRankEmbed 0.526 / 0.777 / 0.847; Agentless with GPT-4o 0.672 / 0.745 / 0.745,
with Claude-3.5 0.726 / 0.792 / 0.796; LocAgent with Claude-3.5 0.777 / 0.920 / 0.942. LLM agents
remain ahead at Acc@1; the engine is an LLM-free first stage they can start from.

SWE-bench dev (tuning set, 225): plain BM25 0.160 / 0.347 / 0.427 / 0.538 → 0.249 / 0.484 / 0.600 /
0.680.

**SWE-bench Verified** (500 human-validated issues, v1.2.0 engine, run once; 496 scored, 4 lost to
git checkout errors): plain BM25 0.216 / 0.391 / 0.490 / 0.641 → **0.405 / 0.669 / 0.732 / 0.804**
(Acc@1/3/5/10). On the 403 Verified issues that are not in Lite: BM25 0.194 / 0.372 / 0.476 / 0.620
→ 0.392 / 0.655 / 0.727 / 0.809.

**Which part does the work** (Lite strict holdout, one run per removed component, Acc@1/3/5):
definition-level ranking removed 0.388 / 0.569 / 0.652; report hygiene removed 0.467 / 0.707 /
0.743; packet order only 0.366 / 0.536 / 0.558; full 0.486 / 0.710 / 0.750. Raw results: `swebench/results-final-test.jsonl` (outside the repository); harness
`scripts/swebench_localize.py`, scoring `scripts/research/eval_loc.py`.

### v1.3.0, run once on Lite and Verified (2026-10-09/10)

v1.3.0 adds files the report names (traceback frames, paths) as a vote and the function list
(`functions_to_look`). Same protocol: release binary, one run per benchmark, never inspected per
instance. 95% bootstrap intervals in brackets.

| file level | n | Acc@1 | Acc@3 | Acc@5 | Acc@10 | BM25 Acc@1/3/5/10 |
|---|---|---|---|---|---|---|
| Lite strict holdout | 276 | **0.507** [0.45,0.57] | **0.717** [0.66,0.77] | **0.750** [0.70,0.80] | **0.815** | 0.301 / 0.507 / 0.587 / 0.721 |
| Lite, LocAgent's subset | 274 | **0.500** [0.44,0.56] | **0.723** [0.67,0.77] | **0.755** [0.70,0.81] | **0.828** | 0.299 / 0.522 / 0.606 / 0.734 |
| Verified, all | 500 | **0.446** [0.40,0.49] | **0.680** [0.64,0.72] | **0.736** [0.69,0.77] | **0.814** | 0.216 / 0.392 / 0.490 / 0.642 |
| Verified, not in Lite | 407 | **0.435** [0.38,0.48] | **0.671** [0.62,0.71] | **0.732** [0.69,0.78] | **0.811** | 0.194 / 0.373 / 0.477 / 0.622 |

v1.2.0 → v1.3.0 on the same instances: Lite 276 Acc@1 0.486 → 0.507; Verified-not-Lite (the 403
v1.2.0 scored) 0.392 → 0.437; Acc@5 unchanged within ±0.005. p50 2.9 s / p90 10.6 s per Lite issue (cold index included).

| function level (all edited functions in top k) | n | Acc@1 | Acc@5 | Acc@10 |
|---|---|---|---|---|
| Lite, LocAgent's subset | 274 | 0.168 [0.12,0.22] | **0.394** [0.34,0.46] | **0.482** [0.42,0.54] |
| Lite strict holdout, with function gold | 250 | 0.168 | 0.400 | 0.492 |
| Verified, all, with function gold | 459 | 0.163 | 0.346 | 0.416 |
| Verified not in Lite, with function gold | 375 | 0.157 | 0.312 | 0.381 |

Function gold = innermost function containing a removed line or insertion point of the reference
patch (Python `ast`); the 274 instances that have one are exactly LocAgent's subset. Published
(LocAgent Table 4, function Acc@5/@10): BM25 0.318/0.369, CodeRankEmbed 0.518/0.588,
Agentless+Claude-3.5 0.588, LocAgent+Claude-3.5 0.734/0.774. The engine lists functions only for
reports of 60+ words; shorter reports count as misses here (30 of 274). Scoring them out, as the
session-17 dev numbers did, gives 0.443/0.541 on 244 — the smaller denominator flatters.

### v1.4.0, run once on Lite and Verified (2026-10-10)

v1.4.0 adds the functions a traceback runs through to the function list and lists functions for
every prompt. File level is identical to v1.3.0 on every instance (it was not touched). Function
level, every edited function in the top k, 95% bootstrap CI for Acc@5:

| function level | n | v1.3.0 Acc@1 / @5 / @10 | **v1.4.0 Acc@1 / @5 / @10** |
|---|---|---|---|
| Lite, LocAgent's subset | 274 | 0.168 / 0.394 / 0.482 | **0.223 / 0.460** [0.40,0.52] **/ 0.540** |
| Lite strict holdout, with function gold | 250 | 0.168 / 0.400 / 0.492 | **0.228 / 0.472 / 0.556** |
| Verified, with function gold | 459 | 0.163 / 0.346 / 0.416 | **0.198 / 0.429** [0.38,0.47] **/ 0.497** |
| Verified not in Lite | 375 | 0.157 / 0.312 / 0.381 | **0.189 / 0.397 / 0.464** |

Published (LocAgent Table 4, function Acc@5/@10): BM25 0.318/0.369, CodeRankEmbed 0.518/0.588,
Agentless+Claude-3.5 0.588, LocAgent+Claude-3.5 0.734/0.774. p50 2.8 s / p90 11.2 s per Lite issue.
Tables regenerate with `scripts/research/paper_tables.py --swe <swebench dir>`.

## Speed (same machine, same hour, release binary without embeddings)

ultralytics, 931 files: index ≈3.6 s; one question end-to-end p50 ≈0.70 s (n=10, cold process each
time). Internal `l1_p95` gate on this repository: 47 ms (ceiling 50 ms). The 2026-09-13 numbers
(index 2.1 s, p50 0.28 s) were taken under different load; compare like with like.

## What changed on 2026-09-20 (session 12) and how to read the table

- **holdout-c 0.656 → 0.573 is a corrected measurement, not a regression.** A fresh index used to
  register the same symbol id once per same-named definition in a file (F63); a snapshot-loaded graph
  (the MCP server after a restart) never did. The two paths disagreed, the duplicates flattered
  holdout-c, and the harness now measures the deduplicated graph both paths share.
- **holdout-ml (keras-io + setfit) and holdout-cfg are dev-class now**: phase D-3 iterated on the
  former, F55 was traced on the latter's two hydra tasks. `holdout-ml2` (peft + keras-hub) is the
  fresh ML holdout and was never tuned on; its gate passed on the first run.
- **Strict oracle numbers before F59 were under-reported**: an unqualified need (`trainer.py::train`)
  counted as folded whenever any same-named fold existed. The current column is right.
- The harness caches each checkout's graph under `target/third_party_index_cache/` (keyed on git rev +
  the graph crates' source hash); `NM_INDEX_CACHE=0` rebuilds. `NM_EXPLAIN=1` writes, per low-precision
  task, every packet file with its reason and every resolved seed to `target/explain-<set>.txt`.

## Task success with a real model (2026-09-15, main after PR #79)

`neuromesh eval --tasks --executor model`, answerer `deepseek-ai/DeepSeek-V4-Flash-0731`, a separate
judge model `zai-org/GLM-5.3`, real `verify` (patch applied + test run) for patch tasks, QA-judge for
explanatory ones. 26 dev tasks (`fixtures`) plus two fresh holdout sets never used to tune the
engine, 10 tasks each: `holdout-gin` (Go) and `holdout-vision`.

| set | packet | whole-gold-files | grep |
|---|---|---|---|
| fixtures (dev, 26 tasks) | 0.769 (strict 0.731) | 0.885 (strict 0.769) | 0.577 (strict 0.500) |
| **holdout-gin** (Go, 10 tasks) | **1.000** (0.800) | 0.900 (0.800) | 0.900 (0.800) |
| **holdout-vision** (10 tasks) | 0.800 (0.700) | 0.700 (0.600) | 0.800 (0.600) |

Forbidden hits: 0 in all nine cells. `success per 1k tokens` is higher for `packet` than for
`whole-gold-files` in all three sets (e.g. fixtures 0.164 vs 0.106) — the packet context is more
token-efficient at the same or better success rate; `grep` is worse than `packet` on token efficiency
in every set (e.g. holdout-gin 0.131 vs 0.041), consistently. All nine cells clear the 0.5 task-success
gate. `grep`'s first run of `holdout-gin` initially scored 0.400 — traced to a real bug in the `grep`
baseline (a byte cap that could be exhausted by one large, generically-matching file before ever
reaching the file the task actually needed), fixed and rerun; see F56 in
`docs/planning/stage5-findings.fa.md`. Not run: the "before vs after phase B" baseline comparison
called for in `docs/planning/06-plan-revision-2026-09-14.fa.md` (row C) — this run only covers the
current `main`. Full trace: `docs/planning/stage5-findings.fa.md` ("فاز C").

## Task success with a real model — second run (2026-09-20, main after PR #105)

Same harness, answerer `deepseek-ai/DeepSeek-V4-Pro` (a reasoning model), judge `zai-org/GLM-5.3`, via Baseten.
Eight engine PRs (#84–#105) sit between the two runs.

| set | packet (success / strict / mean tokens / per-1k) | whole-gold-files | grep |
|---|---|---|---|
| fixtures (dev, 26) | **0.808** / 0.769 / 4181 / **0.193** | 0.808 / 0.769 / 8729 / 0.093 | 0.577 / 0.538 / 4897 / 0.118 |
| **holdout-gin** (10) | **1.000** / 0.800 / 8102 / **0.123** | 1.000 / 0.800 / 11103 / 0.090 | 0.800 / 0.700 / 22400 / 0.036 |
| **holdout-vision** (10) | **1.000** / **1.000** / 7762 / **0.129** | 0.800 / 0.800 / 8710 / 0.092 | 0.600 / 0.600 / 20531 / 0.029 |

Forbidden hits 0 in all nine cells; all three gates pass in all nine (success ≥0.5; per-1k packet >
whole-gold-files, 1.4–2.1×; packet ≥ grep). Versus the first run: holdout-vision/packet 0.800→1.000
(strict 0.700→1.000), fixtures/packet 0.769→0.808. Of the five fixture failures under `packet`, two
were the harness, not the model: the patch was right but its hunk header was `@@ ... @@`, which `git
apply` rejects — fixed in F76 (header rebuilt from the file). The rerun of those two cells hit Baseten
`402 Payment Required` (credit exhausted by the reasoning model) and is pending; the table above is the
pre-F76 measurement. One failure is a real packet defect: `rs_router_handle` passes with whole files
and fails with the packet (a fold hid `extract_route`'s body) — open.

## Not measured

- **Private/enterprise repositories.** No number. The holdout protocol for one is written
  (`docs/planning/05-phases-holdout-to-release.fa.md`, phase 5b) and waits on a team-authored gold.
- **C++ under heavy macros / templates.** fmt parses and yields symbols, but coverage of
  `FMT_FUNC`-style definitions was not counted.
- **Languages without a holdout**: Java, Kotlin, PHP, C#, Dart, Swift, Ruby, TypeScript have
  grammars and fixture tests but no third-party holdout of their own yet.

## Reproducing a single question

```bash
NM_AUDIT_PATH=/path/to/checkout NM_PROBE="How does X do Y?" NM_PROBE_EDGES=1 NM_PROBE_RANK=1 \
  cargo test -p neuromesh-context --test artifact_audit packet_probe -- --ignored --nocapture
```

prints the files shipped, every seed with its resolution, each seed's outgoing call edges with
caller counts, and every ranked candidate with its score breakdown — the tooling every finding in
`stage5-findings.fa.md` was made with.

## Baseline v0.9.0 vs this fork (2026-09-21)

112 gold tasks, 15 repositories, both binaries on the same machine and day, scored by file name from each engine's `optimize` output (`scripts/compare-baseline.sh`; raw rows in `baseline-vs-fork-2026-09-21.txt`). Baseline recall 0.735 / precision 0.206 / 12 forbidden; fork 0.938 / 0.633 / 7. Baseline has no parser for Scala, R, Julia (recall 0 there), skips `.sh`/`.ps1`/`.ipynb`, and answers config questions at 0.5–0.75 recall. Where the baseline already worked (Go, Python), recall is equal and precision is ~3× higher. Both are weak on the Fastify + Next.js set (0.65 recall).

Claude Code session A/B (`claude -p`, Sonnet, same task): Django 3.5k files — plain 8 turns / 233k context tokens / $0.239, with the engine 5 turns / 129k / $0.204; fastify/demo 68 files — 244k / $0.408 vs 194k / $0.391. The engine only shrinks the code-context share of a session; system prompt and tool schemas are re-read every turn.

## Against outside baselines (2026-10-01)

Same gold, same checkouts, `python scripts/compare_baselines.py <gold> <checkout> --aider`.
Our engine ships a packet of variable size; the baselines are cut at k files.
**bm25** is plain file-level BM25 over the source text (what a search box does).
**aider-repomap** is Aider 0.86.2's own `RepoMap.get_ranked_tags` — PageRank over
definition/reference tags, personalised by the identifiers and file names the question
mentions, the same inputs Aider derives from a chat message. It was built to give a model a
map of the whole repository, not to answer one question, and it shows.

| set | ours recall / precision | bm25 R@1 / P@1 | bm25 R@3 / P@3 | aider R@5 / P@5 |
|---|---|---|---|---|
| holdout-2 (gin, torchvision) | **1.000 / 0.700** | 0.675 / 0.700 | 0.950 / 0.333 | 0.475 / 0.110 |
| holdout-c (libuv, fmt) | **1.000 / 0.589** | 0.594 / 0.625 | 0.938 / 0.334 | 0.406 / 0.087 |
| holdout-ml2 (peft, keras-hub) | **1.000 / 0.632** | 0.400 / 0.700 | 0.683 / 0.400 | 0.167 / 0.080 |
| holdout-lang (os-lib, cli, Flux.jl) | 1.000 / 0.589 | **0.933 / 0.933** | 1.000 / 0.333 | 0.400 / 0.080 ¹ |
| concept-holdout (ripgrep, plain language) | 0.500 / 0.156 | 0.250 / 0.333 | 0.500 / **0.222** | 0.042 / 0.017 |
| concept-holdout2 (click, plain language) | **0.958 / 0.342** | 0.417 / 0.500 | 0.750 / 0.306 | 0.333 / 0.067 |
| concept-holdout, ours **+ jina-code** | **0.667 / 0.347** | | | |
| concept-holdout2, ours **+ jina-code** | **0.958 / 0.439** | | | |

Where we stand, plainly: on questions that name code, the engine gets every gold file at a
precision no fixed cut of BM25 reaches on three of four holdouts. On holdout-lang the single top
BM25 file is right 93% of the time, so our wider packet costs precision there. On plain-language
questions the picture splits: level with BM25 on ripgrep (12 crates, terse names), ahead of it
on click (recall 0.958 at precision 0.342 against 0.750 at 0.306 for three files) — a small
repository, which helps every method.

¹ Aider's tree-sitter query for Julia fails to load (`Invalid node type: module`); Flux.jl's
five questions are left out of its row.

**With the optional code-aware model** (`neuromesh install embed jina-code`,
`NEUROMESH_EMBED_MODEL=jina_code_v2`): plain-language questions fuse the lexical file ranking with
the model's file vectors. Same build, model loaded, fusion off → on: ripgrep 0.500 / 0.156 →
0.667 / 0.347, click 0.958 / 0.342 → 0.958 / 0.439, this repository 0.679 / 0.404 → 0.786 / 0.429.
The fusion weights (0.6 lexical / 0.4 dense) were set from the literature before the run, not fitted
to these sets; both holdouts had been run before (ripgrep several times while the lexical ranker was
built), so read them as dev-adjacent, not blind. MiniLM fused the same way lowered every set (ripgrep
0.375, this repository 0.607) and is never fused.
