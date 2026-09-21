# Roadmap

> Full design docs, findings and session handoffs (Persian): [`docs/planning/`](docs/planning/).
> Numbers and what is not measured: [`docs/measured.md`](docs/measured.md).

## Goal

**Maximum task success per token, on any codebase, with zero cross-project leakage.**
Concretely: recall ≥ 0.95 and precision ≥ 0.75 on repositories the engine was never tuned on, with a real-model task-success benchmark that stays above "open the right files by hand".

## Where the project stands (2026-09-21)

| Phase | Scope | State |
|---|---|---|
| **P0 — Isolation** | deterministic `ProjectId`, single-project invariant, leakage gate in CI | ✅ done, [offered upstream](https://github.com/pinoox/neuromesh/pull/34) |
| **P1 — Universal graph** | notebooks, config→code (YAML / Hydra / argparse), ML artifact overlay, shell scripts, string-literal index | ✅ done for Python / JS / TS / Go / C / C++ / Rust / Scala / R / Julia / Kotlin / PHP; `tantivy` BM25 and stack-graphs not started |
| **Holdout proof** | gold sets on repositories never tuned on (Go, Python, C, C++, Scala, R, Julia, HF / Keras libraries) | ✅ recall 1.00 on all four; precision 0.54–0.63 |
| **Task success with a real model** | packet vs whole gold files vs grep, separate judge, real `verify` | ✅ all nine cells pass all three gates (packet 1.00 / 1.00 on the two holdouts) |
| **Web-stack coverage** | Fastify + Next.js app-router holdout (the shape of a typical B2B product) | 🟡 recall 0.64 → the open front; see below |
| **Private B2B holdout** | the team's own repository, gold written by the team before the engine runs | ⏸ waiting for the gold |
| **Release** | v1.0 binary, docs, upstream PRs | ⏳ after the web front closes |

## What is next, in order

1. **Recall on web / route-shaped repositories.** Untuned recall on Fastify + Next.js was 0.54. Known, measured causes with fixes recorded but not yet shipped: call edges from same-named handlers hang off the file node (F81 — needs stricter callee seating first); prompt words that are directory names; path-word seeds next to an anchor. Each is in `docs/planning/stage5-findings.fa.md` with the number it moved.
2. **Callee seating by prompt evidence.** The selector gives up to three required seats to a seed's callees; with a correct call graph that floods. Seats should require the prompt to name the callee or its file.
3. **Private holdout (5b).** Thirty real questions from the team on their repository; the engine never sees a packet before the gold is frozen. This is the only true holdout for the product's own domain.
4. **Precision on unseen repositories.** Every holdout carries 1–3 neighbour files too many. Score-tuning has never moved this; structural findings (F61, F73, F82) have. Continue by probe, not by thresholds.
5. **Release.** Binary via the upstream installer path, `docs/measured.md` as the source of truth, upstream PRs for the isolation and config→code layers.

## Measured dead ends (do not re-try without a new idea)

- **Embedding retrieval (MiniLM, `hybrid` engine)** on the self set: recall 0.50 / precision 0.12 vs 0.68 / 0.41 for the lexical engine; none of the concept-only misses gained (F79).
- **Score and threshold tuning** across two sessions: no holdout moved; every gain came from a structural bug found by reading the explain dump.
- **Path-word file seeds next to an anchor**: +0.10 recall on the web set, −0.08 precision on C and config sets (F80).

## Later

- `tantivy` BM25 for the lexical layer; `tree-sitter-stack-graphs` name resolution (Python / JS / TS first); SCIP ingestion when present.
- Knowledge layer (module summaries, GraphRAG-style communities) and optional post-fold compression.
- Independent benchmark on a SWE-bench Verified subset with the same three-way (packet / whole files / grep) protocol.

## Relationship to upstream

`upstream/main` stays a remote; fixes are merged. Isolation and config→code work is designed to be offerable as clean PRs to `pinoox/neuromesh`. The universal / ML layer and the benchmark discipline are this project's own direction.
