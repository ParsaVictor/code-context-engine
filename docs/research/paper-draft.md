# Paper draft (living) — *Where to look: LLM-free, CPU-only code localisation for coding agents, measured on holdouts*

Status: skeleton with measured numbers (session 17). Every number traces to
`contributions-log.md`; TODO marks what is not measured yet.

## Abstract (draft)

Coding agents spend most of their budget finding the code a task needs. Strong localisers today
either run an LLM agent over the repository (Agentless, LocAgent, SWE-agent) or a GPU-scale dense
retriever (CodeRankEmbed). We ask how far a local engine gets with neither: a per-project code
graph, field-weighted lexical ranking, a definition-level ranking of long reports, and explicit
report hygiene, all on a laptop CPU. On SWE-bench Lite, run once after tuning only on the SWE-bench
dev split, the engine reaches file-level Acc@1/3/5 of 0.486/0.710/0.750 on 276 held-out issues
(plain BM25 0.301/0.507/0.587) in 2.8 s per issue including a cold index — above a code embedding
model at Acc@1 and level with Agentless+GPT-4o at Acc@5. We report every component's effect,
negative results (dense retrieval on CPU, cross-encoder reranking, blind graph expansion), and a
holdout protocol that caught two results that looked-at sets had suggested. TODO: LLM stage (D1),
Verified, ablations.

## 1. Introduction

- Problem: context for coding agents; cost of reading; localisation as the first step.
- Gap: LLM-heavy or GPU-heavy localisers; local-first tools (Aider RepoMap, Continue) are not
  measured on standard localisation benchmarks; tuning on the evaluation set is common.
- Contributions:
  1. A holdout protocol for retrieval engines (gold locked before runs; dev vs holdout labelled per
     set; one run per holdout) and evidence it matters (§6).
  2. An LLM-free engine: prompt-evidence seeding, BM25F with a comment field, definition-level
     ranking for reports, report hygiene, localisation list (C2–C12 in the log).
  3. Results on SWE-bench Lite/dev/Verified and on four plain-language holdouts in four languages.
  4. Negative results with numbers.

## 2. Related work

Agentless (hierarchical LLM localisation), LocAgent (graph + LLM agent), SWE-agent / OpenHands /
MoatlessTools (agentic search), CodeRankEmbed / Jina code embeddings (dense), BM25, Aider RepoMap
(PageRank over tags), Continue/Cody (retrieve + rerank), BM25F (Robertson et al.), hybrid fusion
(Bruch et al. 2023), RRF (Cormack et al. 2009).

## 3. System

3.1 Index: per-project graph (files, definitions, calls, imports), built in 26 s for django
(3.5k files) after the link-resolution fixes (C10).
3.2 Questions: prompt anchors → seeds → activation → packet; BM25F over path/symbols/comments/body.
3.3 Reports (≥60 words): hygiene (template scaffolding, links, checklists, headings), code-like
tokens, definition-level BM25 with title ×3, RRF with the packet order → `where_to_look`.
3.4 Short questions: RRF of packet order and whole-question ranking.

## 4. Evaluation protocol

- SWE-bench dev (225) = tuning; dev-fast (59) for iteration; Lite test (300) = holdout run once;
  the 24 instances looked at in an earlier session excluded from the strict row (276).
- Metric: file-level Acc@k (all gold files in top k), bootstrap 95% CIs.
- Plain-language: four holdouts (ripgrep/Rust, click/Python, cobra/Go, axios/JS), 12 q each, gold
  locked by commit before any run.

## 5. Results

### 5.1 SWE-bench Lite (holdout, 276)

| method | LLM | Acc@1 | Acc@3 | Acc@5 |
|---|---|---|---|---|
| BM25 (ours) | – | 0.301 | 0.507 | 0.587 |
| **engine** | – | **0.486** | **0.710** | **0.750** |
| Jina-Code-v2 † | – | 0.434 | 0.712 | 0.803 |
| CodeRankEmbed † | – | 0.526 | 0.777 | 0.847 |
| Agentless + GPT-4o † | ✓ | 0.672 | 0.745 | 0.745 |
| LocAgent + Claude-3.5 † | ✓ | 0.777 | 0.920 | 0.942 |
| engine + local LLM (D1) | ✓ (3B, CPU) | TODO | TODO | TODO |

† LocAgent Table 4, their 274-instance subset (head-to-head on the same subset: TODO).

### 5.2 SWE-bench dev (225) and Verified (500, TODO)

dev: BM25 0.160/0.347/0.427/0.538 → engine 0.249/0.484/0.600/0.680 (Acc@1/3/5/10).

### 5.3 Ablation (Lite holdout) — TODO (runs in progress)

| removed | Acc@1 | Acc@3 | Acc@5 |
|---|---|---|---|
| nothing | 0.486 | 0.710 | 0.750 |
| report hygiene + code tokens | TODO | | |
| definition-level ranking | TODO | | |
| localisation list (packet order only) | 0.366 | 0.536 | 0.558 |

### 5.4 Plain-language holdouts (R@3)

| set | BM25 | engine |
|---|---|---|
| ripgrep (Rust) | 0.500 | 0.500 |
| click (Python) | 0.750 | 0.917 |
| cobra (Go) | 0.875 | 0.875 |
| axios (JS, fresh) | 0.583 | 0.833 |

### 5.5 Cost

p50 2.8 s / p90 11.4 s per issue on a 6-core laptop CPU, cold index included; packet p50 11.1k
tokens; no network, no GPU.

## 6. Negative results and what the holdouts caught

| idea | effect |
|---|---|
| dense chunk retrieval on CPU | 2.6 chunks/s — hours per large repo |
| cross-encoder rerank (v2) on reports | Acc@3 0.492 → 0.373 |
| cross-encoder on short questions | +0.375/+0.208 on two looked-at sets, **−0.084 on fresh cobra** |
| blind graph-neighbour vote | Acc@1 0.254 → 0.136 |
| body term frequency in BM25F | Acc@5 0.544 → 0.509 |
| MiniLM fusion | lowered every plain-language set |
| evaluation: suffix match without segment boundary | inflated a BM25 baseline (0.958 vs 0.875) |

## 7. Threats to validity

Different instance subsets vs published numbers; single annotator for plain-language gold; 12
questions per plain-language holdout; Windows-only timing; blobless clones / network.

## 8. Conclusion — TODO
