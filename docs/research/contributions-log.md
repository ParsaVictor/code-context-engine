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
| Cross-encoder rerank (jina-reranker-v1-turbo-en) on top of jina-code fusion | ripgrep 0.667/0.347 → 0.375/0.215; click 0.958/0.439 → 0.917/0.550; self 0.786/0.429 → 0.857/0.500 | English-text reranker; precision up, recall collapses on the true holdout. Code-trained v2 reranker queued |

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

## 8. SWE-bench Lite file localisation (in progress, 2026-10-01)

First 61 instances (flask, requests, seaborn, xarray, pylint, sphinx, astropy partial), v1.1.0+ lexical
engine, issue text as the query, gold = the one file the reference patch edits:

| method | hit@1 | hit@3 | hit@5 | whole packet |
|---|---|---|---|---|
| ours (packet, best-first) | 0.180 | 0.328 | 0.410 | 0.492 (4.5 files, ~9.4k tokens) |
| plain BM25 over the checkout | 0.295 | 0.525 | 0.639 | — |

Finding: on long issue reports (tracebacks, code snippets) the identifier-seeding pipeline scatters
across many anchors and fills the packet with the wrong files (including test fixtures); reading the
whole text (BM25) does better. This is the first standard-benchmark result and it is a weakness —
the next engine work targets it (whole-question ranking for long reports, traceback file paths as
anchors). Published reference points to beat: Agentless and LocAgent file-level Acc@k (LLM-based).

### 8.1 Session 17 (2026-10-08): protocol, reference points, first full test-split number

**Protocol.** Tuning set = SWE-bench *dev* split (225 issues; pvlib, pydicom, sqlfluff, astroid,
pyvista, marshmallow — no repository shared with Lite test). Iteration subset `dev-fast` = first 10
instances per dev repository by id (59). Holdout = SWE-bench Lite *test* (300). The 24 Lite-test
instances from flask/requests/seaborn/xarray/pylint that session 16 tuned issue mode on are
dev-class and reported separately. Metric = file-level Acc@k as in LocAgent (all gold files in the
top k).

**Published reference points** (LocAgent, arXiv 2503.09089, Table 4; SWE-bench Lite, file level):

| method | Acc@1 | Acc@3 | Acc@5 |
|---|---|---|---|
| BM25 | 38.69 | 51.82 | 61.68 |
| Jina-Code-v2 (embedding) | 43.43 | 71.17 | 80.29 |
| CodeRankEmbed (embedding) | 52.55 | 77.74 | 84.67 |
| Agentless (Claude-3.5) | 72.63 | 79.20 | 79.56 |
| SWE-agent (Claude-3.5) | 77.37 | 87.23 | 90.15 |
| LocAgent (Claude-3.5) | 77.74 | 91.97 | 94.16 |

**Lite test, v1.1.0 + issue mode (#131), single run, 237 of 276 non-dev instances** (39 lost to
git worktree lock contention between harness shards, re-run separately):

| | hit@1 | hit@3 | hit@5 | packet |
|---|---|---|---|---|
| ours (packet order) | 0.344 | 0.506 | 0.534 | 0.538 |
| ours BM25F ranking alone | 0.259 | 0.470 | 0.571 | — |
| plain BM25 (our implementation) | 0.295 | 0.506 | 0.578 | — |

Honest reading: on real issues the engine is level with BM25 and well below dense retrievers and
LLM agents. Hit@5 ≈ packet recall because the packet holds 1–5 files: Acc@5 is capped by packet
size, which motivates a separate localisation list (packet first, then ranked runners-up).

**Dense retrieval cost (measured, negative for local-first use).** Jina-Code-v2 (int8 ONNX, CPU,
6 threads, loaded machine) embedded astroid's 3,364 function/class chunks (256 tokens) in 1,314 s
— 2.6 chunks/s. At ~70 GFLOP per 256-token chunk for a 137–161M encoder, whole-repository chunk
embedding of django (~40k chunks) is hours on a laptop CPU. The published embedding numbers above
are bought with GPU-scale compute; a local engine has to get there lexically/structurally or embed
only a candidate short list.

### 8.2 Index speed (C10)

`NM_TIMING=1` on a django checkout (3.5k files) located the cold-start delay the upstream author
reported: `finalize_links` was 147 s of a ~200 s first packet. Causes and fixes (#132):

| stage | before | after | cause |
|---|---|---|---|
| resolve_file_hint | 84 s | 2 s | every hint normalised every path (4.7k × 3.5k) |
| imported_files_of | 22 s | 0.3 s | neighbour walk per relation; now once per file per pass |
| call resolution | 58 s | 7 s | same (file, name) resolved repeatedly; memoised per pass |
| scan | 16 s | 3 s | serial canonicalize (root re-canonicalised per file) + serial reads |
| whole cold packet | 3m20s | 26 s | |

Same graph: the nine sets and three concept sets did not move. Paper use: indexing cost table
(item 5.6) and the claim that a structural index is cheap enough to build per checkout.

### 8.3 Issue-mode ranking on SWE-bench dev (session 17, dev-class)

`dev-fast` = 59 issues (10 per dev repository), paired runs, file-level Acc@k (all gold files in
top k; dev issues average 1.9 gold files, so numbers sit below Lite's single-file ones). 95% bootstrap
CI on 57 issues is about ±0.12, so only differences of ~0.05+ that repeat across k are read as real.

| run | change | Acc@1 | Acc@3 | Acc@5 | Acc@10 |
|---|---|---|---|---|---|
| plain BM25 (file) | — | 0.175 | 0.386 | 0.474 | 0.526 |
| a (v1.1 + #131 + #132) | packet order | 0.246 | 0.421 | 0.456 | — |
| b | + issue boilerplate stripped, code tokens (`L031`), localisation list (packet, then ranked runners-up, tests/docs last) | 0.211 | 0.456 | 0.544 | 0.667 |
| c | b + real body term frequency in BM25F | 0.193 | 0.439 | 0.509 | 0.649 |
| **d** | b + definition-level BM25 (title ×3), RRF with the list | **0.263** | **0.509** | **0.596** | **0.719** |
| e | d + the same fusion choosing the long-report seeds | = d | = d | = d | = d |

Prototype first (Python, `scripts/research/lex_variants.py`): definition-level BM25 with the title
counted three times, fused by RRF with b's list, gave 0.298/0.544/0.614/0.702 — the engine port (d)
reproduces it within noise. Kept: b, d (C11, C12). Rejected with numbers: c (body tf; also lowered
nothing on the twelve sets but did not help), e (no change).

Failure classes read on dev (sqlfluff): rule codes `L031`/`LT02` dropped by every word tokenizer
(ours and BM25) although the gold file is `rules/L031.py`; issue-template scaffolding ("Search before
asking", "found no similar issues", "Code of Conduct", links) ranked `CODE_OF_CONDUCT.md` and the CLI
module; test fixtures (`.sql`) entering the list. All three are generic, not sqlfluff-specific.

Twelve sets unchanged with d (dev 0.938, large 0.675, holdout 0.700, holdout-c 0.589, holdout-lang
0.589, holdout-ml 0.478, holdout-ml2 0.632, holdout-cfg 0.767, holdout-web 0.917/0.643, concept
0.679/0.398, concept-holdout 0.500/0.156, concept-holdout2 0.958/0.342).

### 8.4 Cross-encoder reranker v2 (jina-reranker-v2, int8 ONNX, 278M) — split verdict

Rerank of the engine's top-10 localisation list, file digest = path + signature/doc lines
(`scripts/research/rerank_test.py`, `rerank_swe.py`):

| set | first stage R@3 | reranker alone | 50/50 blend |
|---|---|---|---|
| ripgrep plain-language (holdout, 12 q) | 0.208 | 0.583 | 0.667 |
| click plain-language (holdout, 12 q) | 0.750 | 0.958 | 0.792 |
| this repo plain-language (dev, 14 q) | 0.643 | 0.679 | 0.643 |
| SWE-bench dev-fast issues (59), Acc@3 | 0.492 | 0.373 | 0.458 |

v1-turbo on ripgrep: 0.458 alone, 0.583 blend. Latency on issues: 8.0 s p50 / 14.5 s p90 for 10
pairs on CPU. Verdict: a code-trained cross-encoder orders *short plain-language questions* better
than lexical ranking on all three sets, and hurts on *long issue reports* (truncated 512-token query
loses the report's specifics; the first stage already reads the whole report). The earlier v1
rejection (§4) was a different experiment — it changed packet *content* (recall collapsed); this one
only reorders. Paper: the query-length split is a finding in itself.

Further rejected on dev-fast (session 17, numbers vs d = 0.254/0.492/0.576/0.695, 59 issues):

| idea | Acc@1 | Acc@3 | Acc@5 | Acc@10 | verdict |
|---|---|---|---|---|---|
| data/CI files (`.yml`, `.json`, `.toml`…) moved behind code in the list | = | = | = | = | no effect, not kept |
| graph neighbours of the top three files as a third RRF vote, equal weight (LocAgent-style expansion) | 0.136 | 0.373 | 0.525 | 0.661 | rejected |
| same, weight 0.25 | 0.136 | 0.475 | 0.576 | 0.678 | rejected |

Graph expansion pulls in hubs the top files call (base classes, utils) ahead of the edited module;
LocAgent gets value from the graph through an LLM choosing which edges to follow, not from a
blind neighbour vote.

### 8.5 Holdout: SWE-bench Lite test, one run (2026-10-09, binary main d6854b6 = v1.2.0 engine)

| | Acc@1 | Acc@3 | Acc@5 | Acc@10 |
|---|---|---|---|---|
| **ours, strict holdout (276; excludes the 24 session-16 dev-class instances)** | **0.486** [0.43,0.55] | **0.710** [0.65,0.76] | **0.750** [0.70,0.80] | **0.804** |
| ours, all 300 | 0.473 | 0.703 | 0.747 | 0.810 |
| ours, packet order only (276) | 0.366 | 0.536 | 0.558 | – |
| plain BM25 (276) | 0.301 | 0.507 | 0.587 | 0.721 |
| ours before session 17 (v1.1 + #131, packet, 258) | 0.337 | 0.504 | 0.535 | – |

Cost: CPU only, no LLM, no embeddings; p50 2.8 s / p90 11.4 s per issue *including a cold index of
the checkout*; packet p50 11.1k tokens.

Reading against LocAgent Table 4 (different filtered subset — context, not a head-to-head): above
Jina-Code-v2 at @1 (0.486 vs 0.434), equal at @3 (0.710 vs 0.712), below at @5 (0.750 vs 0.803);
below CodeRankEmbed everywhere; at @5 level with Agentless+GPT-4o (0.750 vs 0.745) and below every
LLM method at @1. Dev → holdout transfer: the gain over BM25 on dev (+0.09/+0.14/+0.17 @1/3/5) held
on holdout (+0.19/+0.20/+0.16) — no sign of overfitting to the dev split.

Harness lessons (for the paper's reproducibility section): blobless clones need network at checkout
time; Windows MAX_PATH broke 14 scikit-learn checkouts under a deep temp path (fixed with a short
work dir); worktree re-creation per instance made the run IO-bound (now reused); positional
sharding overlapped when a resumed run started (now hash sharding).

Paper next: (1) head-to-head on LocAgent's exact 274-instance subset; (2) an LLM stage over
`where_to_look` (Agentless-style file choice) to measure Acc@1 at a fraction of agent tokens;
(3) ablation table from §8.3 on the holdout (one run per component removed).

### 8.6 Plain-language questions: two fresh holdouts, reranker verdict, list fusion (2026-10-09)

Two new holdouts, gold written from source and committed before any run: **concept-holdout3**
(spf13/cobra v1.8.1, Go, 12 q) and **concept-holdout4** (axios v1.7.7, JavaScript, 12 q).

Cross-encoder (jina-reranker-v2) on cobra, run once: R@3 of the localisation list 0.792 → **0.708**
(engine), 0.625 (pure rerank). It had helped ripgrep and click (§8.4) — it does not generalise.
**Rejected**; code kept on branch `phase-o/rerank-v2` only. This is the holdout principle catching a
result two looked-at sets suggested.

Short-question list fusion (S1): the localisation list for a short question is now the RRF of the
packet order and the whole-question ranking (a packet's activation order puts hub files like
cobra's `command.go` first). R@3, decided on four sets that had been looked at:

| set | before | S1 | S2 (+definition ranking) | BM25 R@3 |
|---|---|---|---|---|
| ripgrep | 0.208 | **0.500** | 0.417 | 0.500 |
| click | 0.750 | **0.917** | 0.917 | 0.750 |
| this repo | 0.643 | 0.643 | 0.643 | 0.357 |
| cobra (looked at after its single run) | 0.792 | **0.875** | 0.792 | 0.875 |
| **axios (fresh, run once)** | 0.833 | 0.833 | — | 0.583 |

Kept S1: gains on three sets, no loss anywhere, neutral on the fresh holdout (so the gain is
dev-class until another holdout confirms it). Packets are unchanged (list order only); fourteen sets
unchanged; packet recall/precision on the new sets: cobra 0.833/0.357, axios 0.667/0.528.

**Evaluation bug found and fixed:** the Python baselines matched gold by plain `endswith`, so a gold
`completions.go` was "found" by `zsh_completions.go`. Only sets with root-level gold files are
affected: cobra BM25 R@3 is 0.875, not the 0.958 first printed. Re-run on every published set
(holdout-2/c/ml2/lang, ripgrep, click, this repo): identical numbers. SWE-bench Lite has no root-level
gold file (dev: 4 of 225, Verified: 1 of 500), and the Rust gold harness matches bare names exactly.
