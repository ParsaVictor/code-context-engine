# What we measured — and what we did not

Every number here comes from one command on a pinned checkout:

```bash
bash scripts/benchmark-holdout.sh          # all seven public sets, ~30 min on a laptop
bash scripts/benchmark-holdout.sh holdout  # one set
```

The rule behind the table: **a number from a repository the engine was tuned on is
not the project's number.** Only the holdout rows are.

## Gold sets (2026-09-14, main after PR #71)

| set | repos | role | recall | precision | forbidden | oracle reachable / strict |
|---|---|---|---|---|---|---|
| dev-4 | nanoGPT, express, vit-pytorch, full-stack-fastapi | tuning set | 1.000 | 0.906 | 0 | 21/21 · 20 |
| large | django (3.5k files), ultralytics | tuning set, large | 0.950 | 0.502 | 0 | 19/20 · 14 |
| **holdout-2** | gin (Go), torchvision (Python) | never tuned on | **1.000** | **0.496** | **0** | 20/20 · 16 |
| **holdout-c** | libuv (C), fmt (C++) | never tuned on | **1.000** | **0.656** | **0** | 16/16 · 10 |
| **holdout-lang** | os-lib (Scala), r-lib/cli (R), Flux.jl (Julia) | never tuned on | **1.000** | **0.502** | **1** | 14/15 · 9 |
| holdout-ml | keras-io examples (Keras), setfit (Hugging Face) | **tuned on in D-3** (5 iterations read its numbers) — dev-class since then; fresh ML holdout is `holdout-ml2` | **1.000** | **0.360** | **0** | 10/10 · 8 |
| **holdout-cfg** | lightning-hydra-template (Hydra YAML), detr (argparse) | never tuned on; Config→Code questions | **0.879** | **0.610** | **0** | — |
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
