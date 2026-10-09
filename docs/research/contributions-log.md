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

### 8.7 Head-to-head on LocAgent's exact Lite subset (274)

LocAgent §5.1 keeps the 274 Lite instances whose patch modifies an existing function. Rebuilt with
`scripts/research/locagent_subset.py` (removed line or insertion point inside a function span of
the base file, Python `ast`): **exactly 274 of 300**. Same v1.2.0 results file, scored on that set:

| method (274, same instances) | LLM | Acc@1 | Acc@3 | Acc@5 |
|---|---|---|---|---|
| BM25 (LocAgent's) | – | 0.387 | 0.518 | 0.617 |
| BM25 (ours) | – | 0.299 | 0.522 | 0.606 |
| Jina-Code-v2 | – | 0.434 | 0.712 | 0.803 |
| **engine v1.2.0 (CPU)** | – | **0.474** [0.42,0.53] | **0.708** [0.65,0.76] | **0.752** [0.70,0.80] |
| CodeRankEmbed | – | 0.526 | 0.777 | 0.847 |
| Agentless + GPT-4o | ✓ | 0.672 | 0.745 | 0.745 |
| Agentless + Claude-3.5 | ✓ | 0.726 | 0.792 | 0.796 |
| LocAgent + Qwen2.5-7B (fine-tuned) | ✓ | 0.708 | 0.847 | 0.883 |
| LocAgent + Claude-3.5 | ✓ | 0.777 | 0.920 | 0.942 |

Caveat: all 24 session-16 dev-class instances are among the 274 (measured) (flask/requests/seaborn/
xarray/pylint); without them (250 strict holdout instances) the engine scores 0.488 / 0.716 /
0.756 and BM25 0.304 / 0.512 / 0.592 — the looked-at instances do not flatter the result.

### 8.8 Files the report names (C13) and quoted error messages (session 17)

Inspired by what agents do first (open the frame of a traceback, grep the error message). On
SWE-bench dev, 50 single-file issues name the gold file in their text, yet only 28 had it first.

- **Named files** (`named_files` in `gold.rs`): traceback frames deepest first, then paths and
  unique file names in prose, resolved against the index by longest unique path suffix; a third
  RRF vote at weight 2 for reports. Prototype (`mention_prior.py`) on dev 225: Acc@1/3/5/10
  0.249/0.484/0.600/0.680 → **0.298/0.516/0.609/0.689**; both halves agree (dev-fast 0.254 → 0.271
  @1, dev-rest 0.247 → 0.307 @1; weight 1/2/4 swept, 2 kept for @3). Engine port on dev-fast
  reproduces the prototype exactly (0.271/0.525/0.576/0.695). Fourteen sets unchanged.
- **Error-message grep** (`error_grep_prior.py`): fires on 15 of 225 dev issues, fixes 2 (+0.009
  @1). Small and positive; not shipped.

### 8.9 Local LLM stage (D1, Agentless-style) — pilot on a laptop CPU

Setup: llama.cpp b11172 (official release), Qwen2.5-Coder-3B-Instruct Q4_K_M (official GGUF,
sha256 verified), one call per issue over the engine's top-10 localisation candidates with a
signature skeleton of each (`scripts/research/llm_localize.py`): ~2.2k prompt tokens per issue
(Agentless sends the repository structure). Hardware: Ryzen 5 7530U (6 cores), no discrete GPU;
the Vulkan iGPU path was slower than CPU (18 vs 20 prompt tok/s) → ~105 s per issue.

Pilot, 15 random dev-fast issues, candidates in engine order: Acc@1/3/5 0.200/0.467/0.600 →
identical. The 3B model returns the list in the order shown (position bias). Next: candidates in
alphabetical order (`--shuffle`).

Shuffle variant (candidates alphabetical, same 15 issues): Acc@1/3/5 0.200/0.467/0.600 →
**0.067/0.400/0.600**. Without the engine's order the 3B model judges worse than the lexical +
structural ranking; with it, it copies it. **Rejected** for a 3B model on CPU: the LLM stage needs a
stronger judge (an API model, or a 7B fine-tuned one as LocAgent's Qwen2.5-7B(ft) at 0.708 Acc@1),
which this laptop runs at ~5 min per issue. Finding for the paper: the engine's ranking already
beats a small local LLM as a judge, so the LLM budget is better spent on a strong model at one call
per issue over `where_to_look` than on a local small one.

### 8.10 Function-level output (C14)

`chunk_rank` already scores definitions; `packet --json` now lists them for a report
(`definitions`: path, `Class.method`, lines; 20 deep, `NM_DEFINITIONS` for research runs).
Function gold = innermost function containing a removed line / insertion point of the reference
patch at the base commit (`scripts/research/func_eval.py`, LocAgent's metric: all edited
functions in the top k). dev-fast (48 issues with function gold): func Acc@1/5/10/20
0.062/0.229/0.292/0.396 (raw definition ranking, no file prior). Reference (LocAgent Table 4, Lite,
function level Acc@5/@10): BM25 0.318/0.369, CodeRankEmbed 0.518/0.588, Agentless+Claude 0.588,
LocAgent+Claude 0.734/0.774. File-prior orderings (`func_hier.py`) pending the full-dev run.

### 8.11 SWE-bench Verified (second standard benchmark) and the ablation table (2026-10-09)

**Verified, v1.2.0 engine, run once** (496 of 500; 4 lost to git checkout errors):

| | Acc@1 | Acc@3 | Acc@5 | Acc@10 |
|---|---|---|---|---|
| engine, 496 | **0.405** [0.36,0.45] | **0.669** [0.63,0.71] | **0.732** [0.69,0.77] | **0.804** |
| BM25, 496 | 0.216 | 0.391 | 0.490 | 0.641 |
| engine, 403 never seen before (not in Lite) | 0.392 | 0.655 | 0.727 | 0.809 |
| BM25, same 403 | 0.194 | 0.372 | 0.476 | 0.620 |

**Ablation on the Lite strict holdout (276)**, one run per removed component (`NM_ABLATE` switch
build, not shipped):

| removed | Acc@1 | Acc@3 | Acc@5 | Acc@10 |
|---|---|---|---|---|
| nothing (v1.2.0) | 0.486 | 0.710 | 0.750 | 0.804 |
| report hygiene + code tokens (C11) | 0.467 | 0.707 | 0.743 | 0.801 |
| definition-level ranking (C12) | **0.388** | **0.569** | **0.652** | 0.764 |
| localisation list — packet order only | 0.366 | 0.536 | 0.558 | – |

Reading: the definition-level ranking (LocAgent's entity-content BM25 idea, made LLM-free) carries
most of the gain over the packet (+0.10 @1, +0.14 @3). Hygiene is small on Lite (−0.019 @1 when
removed) — its dev gains came mostly from sqlfluff's template-heavy issues; reported as is.

### 8.12 Function level: classes out, file prior in (engine, dev 225)

Classes (a definition another one names as its parent) stay in the file ranking but leave the
function list; the list is fused (RRF) with each definition's file rank in `where_to_look`.
Engine on SWE-bench dev (189 issues with function gold), func Acc@1/5/10/20: 0.137/0.268/0.337/–
→ **0.175/0.323/0.402/0.460** (prototype predicted 0.174/0.321/0.400). Same run, file level with
everything since v1.2.0 (named files): 0.249/0.484/0.600/0.680 → **0.308/0.527/0.621/0.692**.
MCP: reports now also get `functions_to_look` (five, `path:Class.method Lx-Ly`).

### 8.13 v1.3.0 run once on Lite and Verified; function-level denominator fix (session 18, 2026-10-10)

Release binary v1.3.0 (tag `6e94b91`), one run per benchmark (`swebench/lite-v130-*.jsonl`,
`verified-v130-*.jsonl`; 300/300 and 500/500 scored, no checkout errors this time).

| file level | n | Acc@1 | Acc@3 | Acc@5 | Acc@10 | BM25 |
|---|---|---|---|---|---|---|
| Lite strict holdout | 276 | **0.507** [0.45,0.57] | **0.717** [0.66,0.77] | 0.750 [0.70,0.80] | 0.815 | 0.301/0.507/0.587/0.721 |
| LocAgent subset | 274 | **0.500** [0.44,0.56] | **0.723** [0.67,0.77] | 0.755 [0.70,0.81] | 0.828 | 0.299/0.522/0.606/0.734 |
| Verified, all | 500 | **0.446** [0.40,0.49] | 0.680 [0.64,0.72] | 0.736 [0.69,0.77] | 0.814 | 0.216/0.392/0.490/0.642 |
| Verified not in Lite | 407 | **0.435** [0.38,0.48] | 0.671 [0.62,0.71] | 0.732 [0.69,0.78] | 0.811 | 0.194/0.373/0.477/0.622 |

v1.2.0 → v1.3.0, same instances: Lite-276 Acc@1 0.486 → 0.507, Verified-not-Lite (the 403 v1.2.0 scored;
the subset has 407) 0.392 → 0.437
(+0.045; the named-files vote, C13, dev gain was +0.049 — it transferred); Acc@5 within ±0.005.
Latency p50 2.9 s / p90 10.6 s (Lite), 2.5 s / 8.8 s (Verified), cold index included.

**Function level** (`func_eval.py`, now with `--ci`, `--gold-cache`):

| | n | Acc@1 | Acc@5 | Acc@10 | Acc@20 |
|---|---|---|---|---|---|
| LocAgent subset (= all Lite instances with function gold) | 274 | 0.168 [0.12,0.22] | **0.394** [0.34,0.46] | **0.482** [0.42,0.54] | 0.544 |
| Lite strict, with function gold | 250 | 0.168 | 0.400 | 0.492 | 0.548 |
| Verified, with function gold | 459 | 0.163 | 0.346 | 0.416 | 0.468 |
| Verified not in Lite | 375 | 0.157 | 0.312 | 0.381 | 0.435 |

Reference (LocAgent Table 4, function Acc@5/@10): BM25 0.318/0.369 · CodeRankEmbed 0.518/0.588 ·
Agentless+Claude-3.5 0.588 · LocAgent+Claude-3.5 0.734/0.774. Engine: above BM25 by +0.08/+0.11,
below the dense retriever.

**Evaluation fix (denominator).** `func_eval.py` used to score only instances whose result *has* a
`definitions` list; the engine emits it only for reports of ≥ 60 words, so short reports left the
denominator. Measured effect on LocAgent's 274: skipping them gives 244 instances and Acc@5/@10
0.443/0.541; counting them as misses (correct) gives 274 and 0.394/0.482. The session-17 dev
function numbers (189 issues, 0.323 Acc@5) used the old rule; phase 1 re-baselines dev with the
correct one. The 274 count now matches LocAgent's subset exactly, which also validates the
function-gold extraction. Engine follow-up: emit the function list for short reports too.
`git show` failures in gold extraction now retry and raise instead of reading as "no gold".

**Phase 2 blocker (2026-10-10).** Five Baseten keys (Kimi-K3, GLM-5.2-Fast, GLM-5.3,
DeepSeek-V4-Flash, DeepSeek-V4-Pro) authenticate (`/v1/models` 200) but every chat call returns
HTTP 402 "please check your current payment status" — the accounts have no credit. The strong-LLM
stage stays unmeasured until one is funded.

### 8.14 Phase 1: LLM-free priors from the bug-localisation literature (session 18, dev only)

Protocol: each prior first as an offline Python prototype over stored engine lists
(`scripts/research/priors.py`: RRF vote into `loc_files` / `definitions`, weights swept), scored
on both halves of SWE-bench dev (dev-fast 59 / dev-rest 166); kept only if file Acc@1 or func
Acc@5 rises ≥ 0.02 in both halves. Function metric with the corrected denominator (§8.13): 210
dev issues have function gold. Baseline = v1.3.0 engine (`dev-func2-all.jsonl`): file
0.308/0.527/0.621/0.692, func Acc@1/5/10 0.158/0.292/0.364.

**Bookkeeping slip, caught:** the first prototype round used `dev-func300-all.jsonl` as its base
— a deeper run made *before* #140 removed classes from the function list (func 0.124/0.243/0.305).
Every function-level delta below is re-measured against the true v1.3.0 run.

| prior (inspiration) | fires on | file Acc@1 fast / rest | func Acc@5 fast / rest | verdict |
|---|---|---|---|---|
| traceback frame → enclosing function (agents open the deepest frame) | 43 | = / = (w=1) | 0.269→0.288 / 0.299→0.312 (proto) | **kept; engine port below** |
| non-code "definitions" (YAML keys) out of the function list | 203 | = | = (10 of 2,030 top-10 slots) | no effect |
| identifiers in the issue's code blocks → their definitions (repro code) | 115 | 0.271→0.254..0.271 / 0.319→0.271..0.313 | +0.019 / +0.019 (base pre-#140) | rejected: file level drops, func gain < 0.02 |
| commit history (BugLocator / Locus): past commits whose message is like the issue → their files; ancestors of base only | 224 | 0.271→0.169 / 0.319→0.289 even at w=0.1 | = | **rejected** |
| test named in the issue → modules it imports | 0 | – | – | no effect: dev reports name no repository tests (5 name a `test_` word, all the reporter's own) |
| RM3 pseudo-relevance feedback on definition BM25 (5 docs, 20 terms, λ=0.6) | 225 | +0.017 / +0.018 (w=0.5); @5 −0.018 / −0.018 | 0.269→0.346 / 0.299→0.306 | rejected: alone worse than plain definition BM25 at @1 (0.038 vs 0.135 fast); as a vote no better than the plain one |
| plain Python definition BM25 (unstemmed, method text prefixed with its class) as a second vote | 225 | +0.017 / +0.024 (w=1); @5 −0.017 / −0.036 | 0.269→0.327 / 0.299→0.312 (w=1) | not shipped: standalone it is level with the engine at @1 and below at @5/@10 (fast 0.135/0.250/0.327 vs 0.135/0.269/0.346; rest 0.171/0.272/0.329 vs 0.165/0.297/0.367) — the gain is diversity; the engine-side variants below do not reproduce it |

History details: `git log --no-renames` is required on blobless clones — rename detection fetches
blobs one by one (it returned 214 of 1,878 pvlib commits in 53 s before the fix).

**Engine port (kept):** `traceback_definitions` (frames → file by longest unique suffix → the
definition named in the frame, or the innermost one containing the line; test frames skipped),
voting at weight 2 in `definition_order`, plus a function list for *every* prompt in
`packet --json` (short reports were automatic misses). Full dev, engine run:

| | func Acc@1 | Acc@5 | Acc@10 | Acc@20 |
|---|---|---|---|---|
| v1.3.0, dev-fast (52) | 0.135 | 0.269 | 0.346 | 0.385 |
| **+ traceback + short reports, dev-fast** | **0.173** | **0.308** | **0.365** | 0.423 |
| v1.3.0, dev-rest (158) | 0.166 | 0.299 | 0.369 | 0.427 |
| **+ traceback + short reports, dev-rest** | **0.196** | **0.329** | **0.399** | 0.456 |
| all 210 | 0.158 → **0.190** | 0.292 → **0.324** | 0.364 → **0.390** | 0.416 → 0.448 |

File level unchanged on every instance (0.308/0.527/0.621/0.692). Of 20 short dev reports with
function gold, 4 now have every edited function in the top 5 (0 before).

Also fixed in passing: the definition ranker sorted query terms alphabetically and kept the first
256, so a long report lost every term after about "p" (4 of 225 dev issues, 2 of 300 Lite issues
exceed 256 distinct terms); it now keeps title terms, then the rarest, up to 1,024.

### 8.15 Phase 3: plain-language questions through the docs (rejected)

Idea (how people ask vs how code names things): the project's own doc sections that read most
like the question (BM25 over Markdown/rST sections) name the flags and identifiers involved; the
source files that contain those identifiers (rarity-weighted) vote next to the engine's list
(RRF). Prototype `scripts/research/doc_bridge.py`, engine lists from v1.3.0 cached
(`plain-lists-v130.jsonl`), R@3 on the four looked-at plain-language sets:

| set | engine (v1.3.0) | doc bridge alone | fused w=0.5 | w=1 | w=2 |
|---|---|---|---|---|---|
| ripgrep (Rust) | 0.500 | 0.125 | 0.375 | 0.208 | 0.167 |
| click (Python) | 0.917 | 0.500 | 0.958 | 0.875 | 0.833 |
| cobra (Go) | 0.875 | 0.458 | 0.792 | 0.708 | 0.625 |
| axios (JS) | 0.833 | 0.083 | 0.583 | 0.417 | 0.250 |

Rejected: worse on three of four sets at every weight. Doc sections mention many identifiers
(examples, other flags, changelog entries), and an identifier→file vote by containment spreads
over every file that uses it — the bridge is far noisier than the question itself.

**Why does a second definition ranker help? (`NM_DIAG=1`, one engine run, dev 225).** The engine
emits its function list under research settings next to the shipped one
(`diag_definitions`; `scripts/research/diag_eval.py`). Shipped = traceback vote + file prior.

| variant | func Acc@1 fast / rest | func Acc@5 fast / rest | verdict |
|---|---|---|---|
| shipped (v1.4 candidate) | 0.173 / 0.196 | 0.308 / 0.329 | – |
| no traceback vote | 0.135 / 0.171 | 0.288 / 0.316 | traceback's share: @1 +0.038/+0.025 |
| no traceback, no file prior (raw ranking) | 0.135 / 0.171 | 0.288 / 0.304 | file prior's share: @5 0/+0.012 |
| exact words instead of stems | 0.192 / 0.184 | 0.288 / 0.335 | rejected (mixed) |
| method chunks carry their class name ("owner") | 0.192 / 0.209 | 0.308 / 0.342 | rejected: @5 0/+0.013 < bar |
| RRF of owner + owner-unstemmed (two indices) | 0.212 / 0.222 | 0.308 / 0.342 | rejected by the pre-set bar (func @5 0/+0.013), although @1 rises +0.039/+0.026 — noted for a later round with a fresh dev split |

The bar was fixed before the run (file Acc@1 or func Acc@5 ≥ +0.02 in both halves); the two-index
fusion clears it only at func Acc@1, a metric the bar does not name, so it is not shipped.

### 8.16 Fifth plain-language holdout: jsoup (Java), run once (2026-10-10)

`concept-holdout5`: jhy/jsoup 1.18.1, 12 plain-language questions, gold written from the source
and committed (`2b9f4a3` on the pushed branch, `b86026c` after rebase) before any engine run. Purpose fixed in advance: confirm the
short-question list fusion (S1, §8.6), which had been decided on looked-at sets and was only
neutral on axios. Run once with three binaries chosen beforehand (v1.2.0 = before S1, v1.3.0 =
S1, the v1.4 candidate) and BM25:

| | list R@3 | packet recall / precision |
|---|---|---|
| plain BM25 | 0.667 (R@1 0.500, R@5 0.917) | – |
| v1.2.0 (no S1) | 0.417 | – |
| **v1.3.0 (S1)** | **0.750** | – |
| v1.4 candidate | 0.750 (list unchanged for short questions) | 0.667 / 0.235 |

S1 holds on a language and repository never run before: +0.333 R@3 over the packet-order list and
+0.083 over BM25. jsoup is now looked at.

**Fourteen sets with the v1.4 candidate** (traceback vote, function list for every prompt, query
term cap fix): identical to the numbers on record (docs/measured.md) on all fourteen, including `concept`
0.679 / 0.392 (§8.3's 0.398 was an older reading of this self-measuring set).

**Dense on the short list only (prior 6, CodeRankEmbed/Jina-inspired).** jina-code-v2 (int8 ONNX,
CPU, 4 threads) embeds the issue and the engine's top-30 definitions (text from the base commit),
reorders them by cosine (`scripts/research/dense_shortlist.py`); the order votes by RRF. Dev 225,
base = v1.4 candidate:

| | file Acc@1 fast / rest | file Acc@3 | file Acc@5 | func Acc@1 | func Acc@5 |
|---|---|---|---|---|---|
| base | 0.271 / 0.319 | 0.525 / 0.524 | 0.576 / 0.633 | 0.173 / 0.196 | 0.308 / 0.329 |
| + dense vote w=0.5 | 0.288 / 0.343 | 0.525 / 0.554 | 0.593 / 0.651 | 0.212 / 0.215 | 0.327 / 0.342 |
| + dense vote w=1 | 0.271 / 0.361 | 0.559 / 0.554 | 0.610 / 0.645 | 0.212 / 0.190 | 0.308 / 0.329 |
| dense order alone (all 225) | file 0.356 / 0.524 / 0.596 (@1/3/5) | | | func 0.162 / 0.276 (@1/5) | |

Cost: 10.6 s p50 / 27.7 s p90 per issue on top of the engine (CPU shared with other runs).
Verdict: **not shipped by default** — positive on almost every cell but below the pre-set bar in
dev-fast (file Acc@1 +0.017 = one issue of 59; func Acc@5 +0.019/+0.013). Noted: the dense order
alone puts the right file first more often than the engine (0.356 vs 0.307) while losing depth —
a cheap re-ranker of the top few files, not a retriever. Candidate for an opt-in deep mode, to be
decided on a larger split.

### 8.17 v1.4.0 run once on Lite and Verified (2026-10-10)

Release binary v1.4.0 (tag on `ee6cc27`, #144), one run each (`swebench/lite-v140-*.jsonl`,
`verified-v140-*.jsonl`; 300/300 and 500/500 scored, `--no-bm25`). File level identical to v1.3.0
on every instance. Function level (all edited functions in top k):

| | n | v1.3.0 Acc@1/5/10 | v1.4.0 Acc@1/5/10 | Δ@5 |
|---|---|---|---|---|
| LocAgent subset | 274 | 0.168/0.394/0.482 | **0.223/0.460/0.540** (@5 CI [0.40,0.52]) | +0.066 |
| Lite strict, with function gold | 250 | 0.168/0.400/0.492 | **0.228/0.472/0.556** | +0.072 |
| Verified, with function gold | 459 | 0.163/0.346/0.416 | **0.198/0.429/0.497** (@5 CI [0.38,0.47]) | +0.083 |
| Verified not in Lite | 375 | 0.157/0.312/0.381 | **0.189/0.397/0.464** | +0.085 |

Dev predicted +0.034 at func Acc@5 (0.290 → 0.324); the holdouts gained about twice that. Not
because the holdouts have more tracebacks or short reports (aggregate input statistics: frames in
20% / 20% / 14% of dev / Lite / Verified issues, under 60 words 10% / 12% / 14%), but because their
function gold is smaller: one edited function in 45% of dev issues (mean 3.7) vs 86% of Lite
(mean 1.17) and 72% of Verified (1.87) — when every edited function must be in the top k, a
correctly placed function pays off far more often on single-function gold. Dev understates
function-level effects on Lite-like data. Against published function-level numbers (LocAgent Table 4, Lite): above BM25
(0.318/0.369) by +0.14/+0.17, below CodeRankEmbed (0.518/0.588) by −0.06/−0.05 — the gap to the
GPU-scale dense retriever halved (it was −0.12/−0.11 with v1.3.0).
Installed as the dogfood binary (v1.3.0 kept as `neuromesh-v1.3.0-backup.exe`); MCP banner 1.4.0,
exits on stdin EOF (probe with `NEUROMESH_NO_BROWSER=1`).
