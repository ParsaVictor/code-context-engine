# Research log — contributions, evidence, and the paper plan

One place for every idea this project tried, kept with its numbers, so a paper can be written from
it later. Rule for this file: **every claim carries the measurement that supports it, and every
rejected idea stays here with the number that rejected it.** Negative results are part of the paper.

Working title: *Task-conditioned context packets for coding agents: retrieving the files a question
needs, measured on holdouts*.

---

## 1. Problem and thesis

Coding agents (Claude Code, Cursor, Copilot agents) spend most of their tokens reading code to find
what a task needs. The question this project answers: **given a task in plain text and a repository,
which minimal set of files/definitions does the model need, and how do we prove the answer on
repositories we never tuned on?**

Thesis: a graph-and-lexicon engine with explicit evidence rules (a file enters the packet only for a
word the prompt wrote), plus hybrid lexical/dense ranking for plain-language questions, reaches
recall 1.0 at precision 0.6–0.7 on code-naming questions and beats plain BM25 and Aider's RepoMap on
plain-language ones — at 97–99% fewer tokens than the workspace.

## 2. Contributions (candidate list for the paper)

| # | Contribution | Where | Evidence |
|---|---|---|---|
| C1 | **Holdout discipline for retrieval engines**: gold written from source before any run, locked by commit, run once; a set that is looked at becomes "dev-class" and is labelled so | `docs/measured.md`, `tests/third_party/*` | 11 sets, 9 languages; dev vs holdout gap reported per set |
| C2 | **Prompt-evidence seeding**: every packet file must be justified by a word the prompt wrote (identifier, quoted literal, route, config key, stem, directory convention, body word) | `neuromesh-context/src/activator_seed.rs`, `seed/` | holdouts recall 1.000, precision 0.589–0.700 |
| C3 | **Prose words are guesses, not anchors** (F89): an English word that happens to be a symbol name ("How does this *tool*…") is demoted unless the prompt marks it as code (backticks, `()`, `!`, `.`, `::`, fragment of another identifier) | `is_prose_word` | fixed upstream author's `root_fs_safety` miss; no regression on 9 sets |
| C4 | **Whole-question BM25F file ranking** with a separate **comment field** (comments are the prose closest to how people ask), Snowball stemming, general software thesaurus kept out of the indexed source | `neuromesh-graph/src/file_rank.rs`, `thesaurus.txt` | plain-language self set 0.286 → 0.679; ripgrep holdout 0.042 → 0.500 |
| C5 | **Code-aware hybrid fusion** for plain-language questions only: convex combination (0.6 lexical / 0.4 dense, Bruch et al. 2023) of the BM25F ranking with Jina code v2 file vectors; the general-purpose model (MiniLM) is shown to *hurt* and is excluded | `fuse_with_embeddings` | ripgrep 0.500/0.156 → 0.667/0.347; click 0.958/0.342 → 0.958/0.439; MiniLM: ripgrep 0.375 |
| C6 | **Self-measurement contamination** finding: an engine measured on its own repository is biased by its own source (a thesaurus in a `.rs` file ranked first on 4 of 14 questions; doc comments quoting gold questions) | session 16 notes | 4/14 questions flipped until the thesaurus moved to `.txt` |
| C7 | **Cold-start correctness**: an MCP server must never answer from a half-built index ("no seed" from an empty graph); "ready" must not wait for optional embeddings | `wait_for_index`, `has_complete_index` | first answer 3.6 s → 2.6 s; empty answers eliminated |
| C8 | **Task-level isolation** (P0): one graph per project id, no cross-project leakage, CI leak test | `docs/isolation.md` | merged upstream (pinoox/neuromesh#34) |
| C9 | **Fast, reproducible benchmark loop**: optimised harnesses run in parallel | `scripts/benchmark-fast.sh` | 9 sets in 1.5–5 min instead of ~40 |

## 3. Results to report (current)

### 3.1 Code-naming questions (holdouts never tuned on)

| set | ours R / P | BM25 R@1/P@1 | BM25 R@3/P@3 | Aider RepoMap R@5/P@5 |
|---|---|---|---|---|
| gin + torchvision | 1.000 / 0.700 | 0.675 / 0.700 | 0.950 / 0.333 | 0.475 / 0.110 |
| libuv + fmt | 1.000 / 0.589 | 0.594 / 0.625 | 0.938 / 0.334 | 0.406 / 0.087 |
| peft + keras-hub | 1.000 / 0.632 | 0.400 / 0.700 | 0.683 / 0.400 | 0.167 / 0.080 |
| os-lib + cli + Flux.jl | 1.000 / 0.589 | 0.933 / 0.933 | 1.000 / 0.333 | 0.400 / 0.080 |

### 3.2 Plain-language questions

| set | BM25 R@3/P@3 | ours lexical | ours + jina-code fusion |
|---|---|---|---|
| ripgrep (Rust, 12 q) | 0.500 / 0.222 | 0.500 / 0.156 | **0.667 / 0.347** |
| click (Python, 12 q, fresh) | 0.750 / 0.306 | 0.958 / 0.342 | **0.958 / 0.439** |
| this repository (dev) | 0.357 / 0.143 | 0.679 / 0.404 | **0.786 / 0.429** |

### 3.3 Earlier measured (sessions 9–15)

- Task success with a real model (DeepSeek-V4-Flash, GLM judge): all gates pass on two holdouts.
- Token reduction vs whole workspace: 97.5–99.8%.
- Against upstream NeuroMesh v0.9.0 on 112 tasks: recall 0.735 → 0.938, ~3× precision.

## 4. Negative results (keep — they are part of the paper)

| Idea | Result | Why it failed |
|---|---|---|
| MiniLM as primary retrieval (F79) | worse than lexical on every set | general paraphrase model; code vocabulary |
| MiniLM fused with BM25F | ripgrep 0.500 → 0.375, self 0.679 → 0.607 | same |
| Seed big files by their best-matching definitions | ripgrep 0.500 → 0.375 | definition seeds lost downstream; packet not smaller |
| Lexical-relevance gate on packet fill | recall 0.679 → 0.643, precision flat | extra files are seeds, not fills |
| Precision tuning on dev sets (twice) | gains did not transfer to holdouts | overfitting to dev; why C1 exists |
| Aider RepoMap as a question→file retriever | R@5 0.04–0.48 | built as a whole-repo map, not a retriever |

## 5. What a top-venue paper still needs (honest gap list)

1. **Standard benchmarks**, not only ours: file-level localisation on **SWE-bench Lite / Verified**
   (gold = files touched by the reference patch), plus RepoBench or Long Code Arena retrieval tasks.
2. **Strong published baselines** on the same data: Agentless localisation, LocAgent / CodeRAG-style
   retrievers, SWE-agent's search tools, a dense code retriever (e.g. Jina/Voyage code) alone, BM25.
3. **End-to-end task success per token** with at least two models (the project's actual claim):
   resolve rate on SWE-bench Lite subset with vs without the packet, tokens spent.
4. **Ablations** for every component in §2 (C2–C5), each on holdouts.
5. **Statistical treatment**: confidence intervals (bootstrap over questions), more questions per set
   (≥ 50 per holdout), inter-annotator agreement on gold (two people write gold independently).
6. **Latency and cost** table (index time, p50/p95 per query, model sizes).

Realistic venues: now — a workshop or industry track (LLM4Code, MSR/FSE industry, ICSE SEIP).
Top-tier (ICSE/FSE/ASE main track, or NeurIPS/ICLR datasets & benchmarks track for the holdout
methodology) once items 1–5 are done.

## 6. Timeline of sessions (for the paper's "development" section)

- Sessions 1–5: fork, P0 isolation, task-success harness, real-repo gold (precision 0.146 → baseline).
- Sessions 6–8: stage 4 precision 0.146 → 0.856 on dev; ratchets.
- Sessions 9–15: holdout phases, generality (C/C++/Scala/R/Julia grammars), model task success, v1.0.0.
- Session 16 (2026-09-30 → 10-01): upstream author's report → plain-language class discovered;
  C3–C7, C9; v1.1.0; baselines; jina-code fusion.

## 7. Next experiments queued

- Cross-encoder reranking (jina-reranker-v1-turbo, the Continue/Cody pattern) on top of C5, aimed at
  precision on plain-language packets (F92).
- SWE-bench Lite file localisation run (item 5.1) — the first step toward a top-venue paper.
