# What we measured — and what we did not

Every number here comes from one command on a pinned checkout:

```bash
bash scripts/benchmark-holdout.sh          # all seven public sets, ~30 min on a laptop
bash scripts/benchmark-holdout.sh holdout  # one set
```

The rule behind the table: **a number from a repository the engine was tuned on is
not the project's number.** Only the holdout rows are.

## Gold sets (2026-09-21, main after G4 — F86)

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
| holdout-web (30 q) | fastify/demo (Fastify API), shadcn-ui/taxonomy (Next.js app router) | dev-class for the web domain (fixed on since session 13; 10 blind questions added in G4 scored 0.58 before fixes) | **0.800** | **0.608** | **0** | — |
| **private** | one closed-source B2B backend+frontend (Fastify/Drizzle + Next.js, ~1.2k files) | never tuned on; gold and checkout live outside this repo | **1.000** | **0.587** | **0** | — |

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
