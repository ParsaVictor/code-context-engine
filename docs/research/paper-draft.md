# Paper draft (living) — *Where to look: LLM-free, CPU-only code localisation for coding agents, measured on holdouts*

Status: full draft of every section except the LLM-stage row (blocked on API credit), numbers
through session 18. Every number traces to `contributions-log.md`; the SWE-bench tables are rebuilt
from the result files by `scripts/research/paper_tables.py`. TODO marks what is not measured yet.

## Abstract (draft)

Coding agents spend most of their budget finding the code a task needs. Strong localisers today
either run an LLM agent over the repository (Agentless, LocAgent, SWE-agent) or a GPU-scale dense
retriever (CodeRankEmbed). We ask how far a local engine gets with neither: a per-project code
graph, field-weighted lexical ranking, a definition-level ranking of long reports, and explicit
report hygiene, all on a laptop CPU. On SWE-bench Lite, run once after tuning only on the SWE-bench
dev split, the engine reaches file-level Acc@1/3/5 of 0.507/0.717/0.750 on 276 held-out issues
(plain BM25 0.301/0.507/0.587) in 2.9 s per issue including a cold index — above a code embedding
model at Acc@1 and level with Agentless+GPT-4o at Acc@5; on SWE-bench Verified's 407 issues outside
Lite, 0.435/0.671/0.732 (BM25 0.194/0.373/0.477). We report every component's effect,
negative results (dense retrieval on CPU, cross-encoder reranking, blind graph expansion, commit
history, pseudo-relevance feedback, documentation bridges), and a holdout protocol that caught two
results that looked-at sets had suggested and confirmed a third on a fresh repository. At function
level the engine lists the edited function among its first five on 0.394 of LocAgent's 274 Lite
instances (BM25 0.318; CodeRankEmbed 0.518) with no model at all, 0.429 on Verified.

## 1. Introduction

A coding agent asked to fix an issue first has to find where the fix goes. On SWE-bench that step —
localisation — is where agent pipelines spend much of their budget: Agentless prompts a model with
the repository tree, then with file skeletons, then with code; LocAgent and SWE-agent run multi-step
LLM searches. The strongest model-free alternative published, CodeRankEmbed, embeds every function
of the repository with a 137M-parameter encoder — hours of compute per large repository on a
laptop CPU (we measured 2.6 chunks/s for a comparable model, §6). Local-first assistants (Aider's
RepoMap, Continue, Cody) ship structural or lexical retrieval but are not measured on standard
localisation benchmarks.

We ask a narrow question: **how far does a local engine get with no LLM and no GPU**, if every
design decision is measured on a tuning split and confirmed once on held-out data? The engine
(open source) builds a per-project code graph in seconds, ranks files with field-weighted BM25,
ranks *definitions* for long reports, cleans issue-template scaffolding out of the query, and
reads the report's own pointers — the files and functions its traceback runs through.

Contributions:
1. **A holdout protocol for retrieval engines** — gold locked by commit before any run, a tuning
   split kept apart from every evaluation set, one run per released version on Lite and Verified,
   and per-set labels for what has been looked at (§4). It caught two results that looked-at sets
   suggested (a cross-encoder reranker, a neighbour vote) and confirmed one on a fresh repository.
2. **An LLM-free localiser** that reaches file-level Acc@1 0.507 on 276 held-out SWE-bench Lite
   issues (BM25 0.301) at 2.9 s per issue on a laptop CPU, above a code-embedding retriever at
   Acc@1, and lists the edited function in its top five on 0.460 of LocAgent's subset (§5).
3. **Ablations and costs** for every component, on the holdout (§5.3, §5.5).
4. **Negative results with numbers** — a dozen ideas from the bug-localisation and retrieval
   literature that did not survive the protocol, and why (§6).

## 2. Related work

**LLM localisers.** Agentless narrows hierarchically (files from the repository tree, then
classes/functions from skeletons, then lines) with one model call per level and sampling + voting.
LocAgent indexes a heterogeneous code graph (files, classes, functions; contain/import/invoke/
inherit edges) and lets an LLM agent search it with entity-content BM25 and graph traversal tools;
it is the strongest published localiser on SWE-bench Lite (file Acc@5 0.942 with Claude-3.5).
SWE-agent, OpenHands and MoatlessTools search through shell or retrieval tools inside an agent loop.
Our engine borrows LocAgent's entity-level content ranking and Agentless's file→function narrowing,
without the model.

**Dense retrieval.** CodeRankEmbed and Jina code embeddings rank function chunks by cosine with the
issue; LocAgent reports them as the strongest model-free baselines. We measure the CPU cost of
whole-repository chunk embedding and a short-list variant that embeds only the engine's top
definitions (§6).

**Classic bug localisation.** IR-based bug localisation (BugLocator: rVSM plus similar fixed bugs;
Locus: change hunks; Rahman & Roy: query reformulation) uses report text, history and structure. We
test the history and reformulation (RM3) ideas under the same protocol and report them negative on
SWE-bench dev.

**Ranking machinery.** BM25F (Robertson et al.), reciprocal rank fusion (Cormack et al. 2009) and
convex lexical/dense fusion (Bruch et al. 2023). Local-first assistants: Aider's RepoMap (PageRank
over tags; we measure it as a retriever, R@5 0.04–0.48 on our sets), Continue and Cody (retrieve +
rerank; a code-trained cross-encoder did not survive our holdout).

## 3. System

3.1 Index: per-project graph (files, definitions, calls, imports), built in 26 s for django
(3.5k files) after the link-resolution fixes (C10).
3.2 Questions: prompt anchors → seeds → activation → packet; BM25F over path/symbols/comments/body.
3.3 Reports (≥60 words): hygiene (template scaffolding, links, checklists, headings), code-like
tokens, definition-level BM25 with title ×3, RRF with the packet order → `where_to_look`.
3.4 Short questions: RRF of packet order and whole-question ranking.
3.5 Functions to look at (every prompt): the definition ranking fused (RRF) with each
definition's file rank (Agentless's file → function step without an LLM), plus the functions a
traceback runs through — each frame resolved to an indexed file by its longest unique path suffix,
then to the definition named in the frame (or the innermost one containing its line), test frames
skipped, voting at double weight deepest-first.

## 4. Evaluation protocol

- **Tuning split:** SWE-bench dev (225 issues; pvlib, pydicom, sqlfluff, astroid, pyvista,
  marshmallow — no repository shared with Lite or Verified), halves dev-fast (59, ten per repository)
  and dev-rest (166). A change is kept only if file Acc@1 or function Acc@5 rises by at least 0.02
  in *both* halves — a bar fixed before each measurement. Every idea is first a Python prototype over
  stored engine outputs, then an engine port re-measured on the full split.
- **Holdouts:** SWE-bench Lite test (300) and Verified (500), each run once per released version with
  the release binary, never inspected per instance. The 24 Lite instances looked at in an early
  session are excluded from the strict row (276); LocAgent's 274-instance subset (patches that edit an
  existing function, rebuilt exactly) gives the head-to-head; 407 Verified issues are outside Lite.
- **Metrics:** file-level Acc@k (all gold files in the top k) and function-level Acc@k (all edited
  functions in the top k; an instance without a function list counts as a miss), 95% bootstrap CIs.
- **Plain-language questions:** five holdouts (ripgrep/Rust, click/Python, cobra/Go, axios/JS,
  jsoup/Java), 12 questions each, gold written from source and locked by commit before the first run;
  metric = R@3 of the localisation list and packet recall/precision.

## 5. Results

### 5.1 SWE-bench Lite — head-to-head on LocAgent's 274 instances (holdout, run once)

| method | LLM | file Acc@1 | Acc@3 | Acc@5 | func Acc@5 | func Acc@10 |
|---|---|---|---|---|---|---|
| BM25 (ours) | – | 0.299 | 0.522 | 0.606 | – | – |
| BM25 † | – | 0.387 | 0.518 | 0.617 | 0.318 | 0.369 |
| engine v1.2.0 | – | 0.474 | 0.708 | 0.752 | – | – |
| engine v1.3.0 | – | 0.500 [0.44,0.56] | 0.723 | 0.755 | 0.394 | 0.482 |
| **engine v1.4.0** | – | **0.500** [0.44,0.56] | **0.723** | **0.755** | **0.460** [0.40,0.52] | **0.540** |
| Jina-Code-v2 † | – | 0.434 | 0.712 | 0.803 | – | – |
| CodeRankEmbed † | – | 0.526 | 0.777 | 0.847 | 0.518 | 0.588 |
| Agentless + GPT-4o † | ✓ | 0.672 | 0.745 | 0.745 | – | – |
| Agentless + Claude-3.5 † | ✓ | 0.726 | 0.792 | 0.796 | 0.588 | – |
| LocAgent + Qwen2.5-7B (ft) † | ✓ | 0.708 | 0.847 | 0.883 | – | – |
| LocAgent + Claude-3.5 † | ✓ | 0.777 | 0.920 | 0.942 | 0.734 | 0.774 |
| engine + local 3B LLM (D1, dev pilot) | ✓ (CPU) | rejected on dev (§6) | | | | |
| engine + API LLM (Agentless-style) | ✓ | TODO (no funded key) | | | | |

† LocAgent Table 4. Strict holdout (276, excludes 24 instances looked at in development):
0.507/0.717/0.750/0.815 (BM25 0.301/0.507/0.587/0.721).

### 5.2 SWE-bench dev (225) and Verified (500)

dev, file level: BM25 0.160/0.347/0.427/0.538 → engine v1.3.0 0.308/0.527/0.621/0.692 (Acc@1/3/5/10;
v1.4.0 identical). Function level (210 issues with function gold), v1.3.0 → v1.4.0: Acc@1
0.157 → 0.190, @5 0.290 → 0.324, @10 0.362 → 0.390; per half fast 0.135/0.269 → 0.173/0.308, rest
0.165/0.297 → 0.196/0.329 (Acc@1/@5).
Verified (run once per version): BM25 0.216/0.392/0.490/0.642 → v1.2.0 0.405/0.669/0.732/0.804 (496)
→ **v1.3.0 0.446/0.680/0.736/0.814** (500). On the Verified issues not in Lite (407; v1.2.0 lost 4 to checkout errors): BM25
0.194/0.372/0.476/0.620 → v1.2.0 0.392/0.655/0.727/0.809 (403 scored) → v1.3.0 0.435/0.671/0.732/0.811
(all 407; BM25 on 407: 0.194/0.373/0.477/0.622).
Function level (all edited functions in top k), v1.3.0 → v1.4.0: Verified 459 with function gold
0.163/0.346/0.416 → 0.198/0.429/0.497 (Acc@1/5/10); not in Lite (375) 0.157/0.312/0.381 →
0.189/0.397/0.464. On Lite the gain (+0.066 @5) was twice the dev prediction (+0.034): dev's function gold is
larger (one edited function in 45% of dev issues vs 86% of Lite), so dev understates
function-level effects on Lite-like data.

### 5.3 Ablation (Lite holdout, 276)

| removed | Acc@1 | Acc@3 | Acc@5 |
|---|---|---|---|
| nothing | 0.486 | 0.710 | 0.750 |
| report hygiene + code tokens | 0.467 | 0.707 | 0.743 |
| definition-level ranking | 0.388 | 0.569 | 0.652 |
| localisation list (packet order only) | 0.366 | 0.536 | 0.558 |

### 5.4 Plain-language holdouts (R@3)

| set | BM25 | engine before S1 (v1.2.0) | engine (S1) |
|---|---|---|---|
| ripgrep (Rust) | 0.500 | 0.208 | 0.500 |
| click (Python) | 0.750 | 0.750 | 0.917 |
| cobra (Go) | 0.875 | 0.792 | 0.875 |
| axios (JS, fresh at its run) | 0.583 | – | 0.833 |
| **jsoup (Java, fresh; gold locked; run once)** | **0.667** | **0.417** | **0.750** |

S1 (fusing the packet order with the whole-question ranking for short questions) was decided on
the first three (looked at), was neutral on axios, and is confirmed on jsoup (+0.333 over the
packet order, +0.083 over BM25).

### 5.5 Cost

p50 2.9 s / p90 10.6 s per Lite issue (Verified 2.5 s / 8.8 s) on a 6-core laptop CPU (Ryzen 5
7530U), cold index included; packet p50 11.1k tokens; no network, no GPU.

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
| commit-history prior (BugLocator: similar past commit messages → their files) | dev file Acc@1 0.271 → 0.169 even at weight 0.1 |
| identifiers in the issue's code blocks → their definitions | file Acc@1 drops; func Acc@5 +0.019 (below the 0.02 bar) |
| RM3 pseudo-relevance feedback on the definition ranking | alone worse at @1 (0.135 → 0.038, dev-fast); as a vote no better than plain BM25 |
| doc sections as a bridge from question words to identifiers (plain-language) | R@3 down on 3 of 4 sets (ripgrep 0.500 → 0.375, axios 0.833 → 0.583) |
| evaluation: function-level denominator without short reports | 244 instead of 274 instances, Acc@5 0.443 instead of 0.394 |

## 7. Threats to validity

The tuning split differs from the holdouts in gold size (dev: 1.9 gold files and 3.7 gold functions
per issue on average; Lite is single-file with 1.17 gold functions), so dev numbers are lower and
function-level gains transfer larger than predicted.

Bookkeeping is itself a threat: two of our own evaluation slips were caught only by re-deriving
counts against an external reference (LocAgent's 274) — a denominator that silently dropped short
reports, and a prototype baseline taken from an older run. Both are reported with their effect.

Different instance subsets vs published numbers; single annotator for plain-language gold; 12
questions per plain-language holdout; Windows-only timing; blobless clones / network.

## 8. Conclusion

A model-free engine on a laptop CPU localises SWE-bench Lite issues at file Acc@1 0.507 (276
held-out issues), above a code-embedding retriever and below LLM agents, at a few seconds and no
tokens per issue; at function level it is above BM25 and below the dense retriever. The holdout
protocol mattered: two of the ideas that looked best on looked-at sets lost on fresh data, and most
literature priors we tried (history, query expansion, documentation bridges, dense re-ranking of a
short list) did not clear a bar fixed before measuring. What remains for a main-track paper: an LLM
stage on top of `where_to_look` measured in tokens and Acc@1 against Agentless and LocAgent (blocked
on API credit at the time of writing), and plain-language holdouts of at least fifty questions with
a second annotator.
