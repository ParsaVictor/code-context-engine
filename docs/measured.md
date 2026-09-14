# What we measured — and what we did not

Every number here comes from one command on a pinned checkout:

```bash
bash scripts/benchmark-holdout.sh          # all five sets, ~25 min on a laptop
bash scripts/benchmark-holdout.sh holdout  # one set
```

The rule behind the table: **a number from a repository the engine was tuned on is
not the project's number.** Only the holdout rows are.

## Gold sets (2026-09-14, main after PR #71)

| set | repos | role | recall | precision | forbidden | oracle reachable / strict |
|---|---|---|---|---|---|---|
| dev-4 | nanoGPT, express, vit-pytorch, full-stack-fastapi | tuning set | 1.000 | 0.906 | 0 | 21/21 · 19 |
| large | django (3.5k files), ultralytics | tuning set, large | 0.950 | 0.507 | 0 | 19/20 · 14 |
| **holdout-2** | gin (Go), torchvision (Python) | never tuned on | **1.000** | **0.496** | **0** | 20/20 · 16 |
| **holdout-c** | libuv (C), fmt (C++) | never tuned on | **1.000** | **0.656** | **0** | 16/16 · 10 |
| **holdout-lang** | os-lib (Scala), r-lib/cli (R), Flux.jl (Julia) | never tuned on | **1.000** | **0.502** | **1** | 14/15 · 9 |
| **holdout-ml** | keras-io examples (Keras), setfit (Hugging Face) | never tuned on | **1.000** | **0.360** | **0** | 10/10 · 8 |
| **private** | one closed-source B2B backend+frontend (Fastify/Drizzle + Next.js, ~1.2k files) | never tuned on; gold and checkout live outside this repo | **1.000** | **0.601** | **0** | — |

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

## Not measured

- **Task success with a real model.** The harness exists (`neuromesh eval --tasks --executor model`,
  separate judge, seven tasks with a real `verify` command) and has been exercised end to end with a
  mock provider. It has not been run with an API key. Nothing here should be read as "an agent
  finishes N% of tasks".
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
