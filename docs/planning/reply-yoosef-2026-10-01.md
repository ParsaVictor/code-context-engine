# Re: your v1.0.0 test on code-context-engine

Thank you for testing 1.0.0 against this repository. Your battery found a class of question
none of our sets measured, and a real protocol bug. Here is what we checked and what changed.

## Your findings, one by one

| Finding | Verdict | What changed |
|---|---|---|
| Dashboard banner on stdout in `mcp` mode | **Real.** It printed on every start | Banner → stderr. A test spawns the real binary and fails on any non-JSON stdout line (it failed on 1.0.0 with the banner line) |
| `root_fs_safety` → `descriptors.rs` | **Real** | "How does this *tool*…" made the English word `tool` a strong identifier anchor (there is a function called `tool`). Prose words are guesses now → `confine.rs` |
| `reinforcement`, `max_files_cap` → wrong file | **Real** | Same class: plain-language questions. New whole-question file ranking (below) |
| `retry_negative` | **Not a defect in this fork** | This fork implements retry with exponential backoff (`crates/neuromesh-provider/src/anthropic.rs`, `openai.rs`). The expected "no match" came from upstream, which has none |
| ~26 s latency | **Not reproduced as a per-query cost; two cold-start bugs found** | Server-side retrieval is 0.1–0.55 s per query here. The first query in a fresh clone includes the index build (3.6 s → 2.6 s). Fixed: a query during the first index build got `no_seed_resolved` from an empty graph, and "ready" waited for the optional embedding refresh. Note that a shell client reading the reply with `read` is itself slow on 100 KB+ lines; that cost 5–20 s in our first measurements |

## Why our numbers missed it

All 30 questions in our self-repo set named an identifier (`resolve_cluster_noun_seeds`, `select()`).
Yours are plain language. So we wrote two new sets, with gold committed before any engine run:

- **concept**: 14 plain-language questions on this repository (your four included; `retry` with
  the provider files as gold).
- **concept-holdout**: 12 plain-language questions on ripgrep 14.1.1, never tuned on and run once
  after the code was frozen.

## Numbers (file-level recall)

| Set | 1.0.0 | 1.1.0 |
|---|---|---|
| concept (this repo, 14 q) | 0.286 | **0.679** |
| concept-holdout (ripgrep, 12 q, never tuned on) | 0.042 | **0.500** |
| concept-holdout2 (click 8.1.7, 12 q, written and run once after 1.1.0) | — | **0.958** (precision 0.342) |
| the nine existing sets | — | no recall lost; precision unchanged or better |

Against outside baselines on the same gold (`scripts/compare_baselines.py`): plain file-level
BM25 cut at three files gets 0.500 / 0.222 on ripgrep (level with us on recall, better on
precision) and 0.750 / 0.306 on click (we are ahead on both). Aider's RepoMap, used the way
Aider feeds it a chat message, reaches 0.042 and 0.333 at five files: it maps a repository, it
does not answer a question. On the code-naming holdouts we keep recall 1.000 at precision
0.59–0.70, which no fixed BM25 cut reaches, except holdout-lang where BM25's single top file is
right 93% of the time. Full table: `docs/measured.md`.

Honest gaps: half the ripgrep questions still miss. Most use words the code never spells ("clickable
link" vs `hyperlink`, "machine-readable" vs `json`). Plain-language packets are also wide
(precision 0.15). Both are the next work.

## What changed in the engine

- Field-weighted BM25 over every file's path, defined names, comments and body, Snowball-stemmed;
  comments are a new index field. The best file is seeded, plus runners-up within 70%.
- A general software thesaurus (49 clusters) at a third of the weight. None of its clusters came from
  the ripgrep questions.
- A lowercase word counts as an identifier only when the prompt marks it as code.

Reproduce: `bash scripts/benchmark-holdout.sh concept concept-holdout`; `NM_TIMING=1` prints
stage timings.
