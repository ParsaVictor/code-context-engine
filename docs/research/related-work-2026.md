# Related work and competitor table (session 19, 2026-10-17)

Companion to `paper-draft.md` §2 and §5.1. Research only: no benchmark was run for this file. Every
number below was read in the paper it is attributed to, and the table and section number are given.
"–" means the paper does not report that figure. It does not mean zero. Numbers are fractions
(papers report %, ×0.01 here) and are copied without rounding.

**Read this before comparing rows.** Rows measure different things.

- **Subset.** LocAgent (Table 4), CoRNStack (Table 6), SweRank (Table 1) and SWE-Adept (function
  columns) use the **274** Lite instances whose patch edits an existing function. This is the same
  subset as our §5.1. CoSIL, OrcaLoca, Agentless, RepoGraph and KGCompass use all **300** Lite
  instances. On Verified, CoSIL uses 500, CoRet 435 and FuseSearch 386.
- **Metric.** LocAgent/SweRank Acc@k = all gold locations in the top k, the same as ours. CoSIL
  reports Top-k. RepoGraph, Agentless and OrcaLoca report the share of issues whose single final
  location set covers the gold ("match rate"). KGCompass uses Jaccard-based accuracy. ToolTrain uses
  Recall@k. CodeScout and FuseSearch use F1. Only rows marked **=** are directly comparable to our
  §5.1 numbers.
- **Model of record.** Agent rows depend strongly on the LLM. SWE-Adept re-ran LocAgent on Lite with
  GPT-5.2 and got function Acc@5 0.646, against the 0.734 that LocAgent reports with Claude-3.5. That
  gap of about 0.09 is between-run and between-model variation, and it is larger than many of the
  deltas papers claim.

## 1. SWE-bench Lite: methods comparable to our §5.1 (274 instances, LocAgent Acc@k) **=**

| method | source | LLM at inference | GPU | file Acc@1 | file Acc@3 | file Acc@5 | func Acc@5 | func Acc@10 | cost / issue |
|---|---|---|---|---|---|---|---|---|---|
| BM25 | LocAgent T4 | – | – | 0.3869 | 0.5182 | 0.6168 | 0.3175 | 0.3686 | – |
| E5-base-v2 (110M) | LocAgent T4 | – | optional | 0.4964 | 0.7445 | 0.8029 | 0.3942 | 0.5109 | – |
| Jina-Code-v2 (161M) | LocAgent T4 / SweRank T1 | – | optional | 0.4343 | 0.7117 | 0.8029 | 0.4234 | 0.5219 | – |
| CodeSage-large-v2 (1.3B) | LocAgent T4 / SweRank T1 | – | yes (practically) | 0.4781 | 0.6934 | 0.7810 | 0.3394 | 0.4453 | – |
| CodeRankEmbed (137M) | LocAgent T4 / SweRank T1 | – | optional | 0.5255 | 0.7774 | 0.8467 | 0.5182 | 0.5876 | – |
| CodeRankEmbed (137M) | CoRNStack T6 (own run) | – | optional | 0.518 | 0.766 | – | 0.500 | 0.591 | – |
| CodeRankEmbed + CodeRankLLM (7B rerank) | CoRNStack T6 | 7B reranker | yes | 0.682 | 0.850 | – | 0.675 | 0.737 | – |
| SFR-Embedding-2 (7B) | SweRank T1 | – | yes | 0.5803 | 0.8029 | 0.8394 | 0.5620 | 0.6423 | – |
| GTE-Qwen2-7B-Instruct (7B) | SweRank T1 | – | yes | 0.6533 | 0.8285 | 0.8978 | 0.6314 | 0.7044 | – |
| **SweRankEmbed-Small (137M)** | SweRank T1 | – | optional | **0.6642** | 0.8650 | 0.9088 | **0.6314** | 0.7445 | – |
| SweRankEmbed-Large (7B) | SweRank T1 | – | yes | 0.7263 | 0.9124 | 0.9416 | 0.7190 | 0.8212 | – |
| SweRankEmbedMulti-Small (0.6B) | SweRank+ T2 | – | optional | – | – | – | 0.6679 | 0.7628 | – |
| SweRankEmbedMulti-Large (8B) | SweRank+ T2 | – | yes | – | – | – | 0.7737 | 0.8686 | – |
| SweRankEmbed-L + SweRankLLM-Small (7B) | SweRank T1 | 7B reranker | yes (A100, 12.5 s) | 0.7810 | 0.9234 | 0.9453 | 0.7956 | 0.8613 | $0.011 (T5) |
| SweRankEmbed-L + SweRankLLM-Large (32B) | SweRank T1 | 32B reranker | yes | 0.8321 | 0.9489 | 0.9599 | 0.8139 | 0.8869 | $0.015 (T5) |
| SweRankEmbed-L + GPT-4.1 rerank | SweRank T1 | GPT-4.1 | API | 0.8212 | 0.9562 | 0.9708 | 0.8175 | 0.8796 | $0.16 (T5) |
| **SweRankEmbedMulti-L + SweRankLLMMulti-L (32B)** | SweRank+ T3 | 32B reranker | yes | – | – | – | **0.8540** | 0.8978 | – |
| Agentless | LocAgent T4 | GPT-4o | API | 0.6715 | 0.7445 | 0.7445 | 0.5547 | 0.5547 | – |
| Agentless | LocAgent T4 | Claude-3.5 | API | 0.7263 | 0.7920 | 0.7956 | 0.5876 | 0.5876 | – |
| MoatlessTools | LocAgent T4 | GPT-4o | API | 0.7336 | 0.8431 | 0.8504 | 0.5730 | 0.5949 | $0.46 (LocAgent T5) |
| MoatlessTools | LocAgent T4 | Claude-3.5 | API | 0.7263 | 0.8577 | 0.8613 | 0.6460 | 0.6496 | $0.46 |
| SWE-agent | LocAgent T4 | GPT-4o | API | 0.5730 | 0.6496 | 0.6898 | 0.4599 | 0.4635 | $0.56 |
| SWE-agent | LocAgent T4 | Claude-3.5 | API | 0.7737 | 0.8723 | 0.9015 | 0.6423 | 0.6460 | $0.67 |
| OpenHands | LocAgent T4 | GPT-4o | API | 0.6095 | 0.7190 | 0.7372 | 0.4964 | 0.5036 | $0.83 |
| OpenHands | LocAgent T4 | Claude-3.5 | API | 0.7628 | 0.8978 | 0.9015 | 0.6825 | 0.7007 | $0.79 |
| LocAgent | LocAgent T4 | Qwen2.5-7B (ft) | yes | 0.7080 | 0.8467 | 0.8832 | 0.6423 | 0.7153 | $0.05 |
| LocAgent | LocAgent T4 | Qwen2.5-32B (ft) | yes | 0.7591 | 0.9051 | 0.9270 | 0.7190 | 0.7701 | $0.09 |
| **LocAgent** | LocAgent T4 | Claude-3.5 | API | **0.7774** | 0.9197 | **0.9416** | 0.7336 | 0.7737 | $0.66 |
| SWE-Adept | SWE-Adept T1 | GPT-5.2 | API | – | 0.923 | – | 0.708 | – | 197k tokens; $0.42 full pipeline |
| **SWE-Adept** | SWE-Adept T1 | Claude-Sonnet-4.5 | API | – | **0.970** | – | **0.878** | – | 348k tokens; $1.79 full pipeline |
| LocAgent (re-run) | SWE-Adept T1 | Claude-Sonnet-4.5 | API | – | 0.935 | – | 0.818 | – | 380k tokens |
| OrcaLoca (re-run) | SWE-Adept T1 | Claude-Sonnet-4.5 | API | – | 0.947 | – | 0.834 | – | 468k tokens |
| RepoGraph (re-run) | SWE-Adept T1 | Claude-Sonnet-4.5 | API | – | 0.920 | – | 0.792 | – | 412k tokens |
| **NeuroMesh v1.4.0 (ours)** | paper-draft §5.1 | **none** | **no** (6-core laptop CPU) | 0.500 | 0.723 | 0.755 | 0.460 | 0.540 | **0 tokens, $0; p50 2.9 s** |

SWE-Adept's file column is Acc@3 on all 300 instances (its App. A.1). Its function column uses the
274 subset. SWE-Adept also lists CodeRankEmbed at 0.766 / 0.500 and CodeSage-Large at 0.712 / 0.401,
copied from CoRNStack.

"GPU optional" means a model of at most about 0.6B parameters that could run on a CPU. None of the
papers report CPU latency for whole-repository indexing with such a model. Our §6 measures that cost
for jina-code.

## 2. SWE-bench Lite and Verified: other protocols (not directly comparable)

| method (venue) | source | LLM | set (n) | metric | file | function | cost / issue |
|---|---|---|---|---|---|---|---|
| Agentless (FSE 2025) | Agentless T1 | GPT-4o | Lite (300) | correct-location rate | 0.697 | 0.520 | $0.70; 78,166 tokens |
| RepoGraph + Agentless (ICLR 2025) | RepoGraph T3 | GPT-4o | Lite (300) | correct-location rate | 0.743 | 0.540 | $0.39 (T2) |
| RepoGraph + SWE-agent | RepoGraph T3 | GPT-4o | Lite (300) | correct-location rate | 0.673 | 0.493 | $2.69 (T2) |
| OrcaLoca (ICML 2025) | OrcaLoca T1 | Claude-3.5-Sonnet | Lite (300) | match rate | 0.8333 | 0.6533 | ~$0.87 search (§4.1.3) |
| CoSIL (ASE 2025) | CoSIL T-II | Qwen2.5-Coder-32B | Lite (300) | Top-1 / 3 / 5 | 0.613 / 0.780 / 0.837 | 0.433 / 0.547 / 0.580 | – |
| Agentless-FL (re-run) | CoSIL T-II | Qwen2.5-Coder-32B | Lite (300) | Top-1 / 3 / 5 | 0.583 / 0.730 / 0.773 | 0.247 / 0.397 / 0.427 | – |
| LocAgent (re-run) | CoSIL T-II | Qwen2.5-Coder-32B | Lite (300) | Top-1 / 3 / 5 | 0.647 / 0.727 / 0.740 | 0.103 / 0.377 / 0.447 | – |
| CoSIL | CoSIL T-III | Qwen2.5-Coder-32B | Verified (500) | Top-1 / 3 / 5 | 0.640 / 0.822 / 0.864 | 0.446 / 0.606 / 0.650 | $0.022 (DeepSeek-V3) / $0.341 (GPT-4o), 50-instance mini set (T-VII) |
| KGCompass | KGCompass T1/T2 | Claude-4 Sonnet | Lite (300) | Jaccard top-1 acc. | 0.836 | 0.560 | ~$0.20 |
| SHERLOC (EMNLP 2026) | abstract | reasoning LLM (~30B) | Lite / Verified | acc@1 / recall@1 (level not stated) | 0.8433 / 0.8127 | – | – |
| RepoMem | RepoMem T1 | GPT-4o | Verified (500) | file Acc@1 / 3 / 5 | 0.686 / 0.745 / 0.765 | – | $0.54–0.89 (T3) |
| LocAgent (re-run) | RepoMem T1 | GPT-4o | Verified (500) | file Acc@1 / 3 / 5 | 0.648 / 0.704 / 0.716 | – | – |
| Agentless (re-run) | RepoMem T1 | GPT-4o | Verified (500) | file Acc@1 / 3 / 5 | 0.533 / 0.678 / 0.714 | – | – |
| CodeRankEmbed (re-run) | RepoMem T1 | – | Verified (500) | file Acc@1 / 3 / 5 | 0.296 / 0.451 / 0.543 | – | – |
| RepoSearcher + ToolTrain-32B (ICSE 2026) | ToolTrain T2 | ToolTrain-32B (ft) | Verified (500) | Recall@1 / 5 | 0.6803 / 0.8859 | 0.4994 / 0.6855 | – |
| RepoSearcher | ToolTrain T2 | Claude-3.7-Sonnet | Verified (500) | Recall@1 / 5 | 0.6948 / 0.8924 | 0.4855 / 0.6608 | – |
| CoSIL | ToolTrain T2 | Claude-3.7-Sonnet | Verified (500) | Recall@1 / 5 | 0.7069 / 0.8913 | 0.5001 / 0.6638 | – |
| CodeScout-14B | CodeScout T3 | 14B (RL, 8×H100 training) | Verified | F1 | 0.6857 | 0.4032 | – |
| FuseSearch-4B (Findings ACL 2026) | abstract | 4B (SFT+RL) | Verified (386) | F1 | 0.847 | 0.564 | −68.9% tokens vs. baseline agents |
| CoRet (ACL 2025 short, ~130M) | CoRet T1 | – | Verified (435) | chunk perfect-recall@5 | – | 0.54 (chunk) | – |
| BM25 (chunk) | CoRet T2 | – | Verified (435) | chunk acc@5 | – | 0.15 | – |
| SWE-Fixer retriever (Findings ACL 2025) | SWE-Fixer T3 | 7B (ft) | Lite (300) | file precision / recall | 0.694 / 0.697 | – | – |
| BM25 top-3 | SWE-Fixer T3 | – | Lite (300) | file precision / recall | 0.189 / 0.567 | – | – |
| BLAZE (TSE 2025) | BLAZE T3 | – (fine-tuned CodeSage, V100) | SWE-bench full test (2,294) | file Top-1 / 5 / 10 | 0.29 / 0.68 / 0.79 | – | – |
| **NeuroMesh v1.3/1.4 (ours)** | paper-draft §5.2 | none | Verified (500) | file Acc@1 / 3 / 5 | **0.446 / 0.680 / 0.736** | func Acc@5 0.429 (459) | 0 tokens; p50 2.5 s |

Venues were confirmed as follows. LocAgent is ACL 2025 (aclanthology 2025.acl-long.426). SweRank is
ICLR 2026 (proceedings.iclr.cc). CoRNStack is ICLR 2025 (iclr.cc poster 28662). RepoGraph is ICLR
2025 (proceedings). Agentless is FSE 2025 (conf.researchr.org program). CoSIL is ASE 2025
(researchr). OrcaLoca is ICML 2025: SlidesLive lists an ICML 2025 talk, but no proceedings page was
found. ToolTrain is ICSE 2026, from a researchr listing under the retitled "Enhancing Issue
Localization Agent with Tool-Interactive Training". SWE-Fixer is Findings ACL 2025 (aclanthology
2025.findings-acl.62). CoRet is ACL 2025 short. SHERLOC is EMNLP 2026 and FuseSearch is Findings ACL
2026, both from the arXiv comment line. BLAZE is TSE 2025, inferred from DOI 10.1109/TSE.2025.3579574.
KGCompass, RepoMem, SweRank+, SWE-Adept, CodeScout and MULocBench were arXiv-only when they were read.

**MULocBench** (arXiv 2509.25242) is relevant because it measures issues that are not code. On its
Python subset (T1), LocAgent + Claude-3.5 reaches file Acc@1 / @5 0.285 / 0.352 and BM25 reaches
0.094 / 0.094. Even the strongest agents drop sharply outside SWE-bench-style issues. This supports
our use of plain-language holdouts (§5.4).

## 3. The three strongest competitors

1. **SweRank / SweRank+ (retrieve + LLM rerank, ICLR 2026).** On the same 274 subset and metric,
   SweRankEmbedMulti-Large + SweRankLLMMulti-Large reaches function Acc@5 **0.854**, Acc@10 0.898
   (SweRank+ T3). The original SweRankEmbed-L + SweRankLLM-Large reaches file Acc@1 / @5
   **0.832 / 0.960** and function Acc@5 0.814 (SweRank T1). It needs a 7B or 32B reranker on a GPU,
   at $0.011–0.015 per issue (T5).
2. **LocAgent + Claude-3.5 (ACL 2025).** File Acc@1 / @3 / @5 **0.777 / 0.920 / 0.942** and function
   Acc@5 / @10 0.734 / 0.774 (T4), at $0.66 per issue (T5). Successors on newer LLMs go further:
   SWE-Adept + Claude-Sonnet-4.5 reaches function Acc@5 0.878 at 348k tokens per issue (T1).
3. **SweRankEmbed-Small (137M, no LLM at inference).** This is the closest competitor in our own
   class. File Acc@1 / @3 / @5 **0.664 / 0.865 / 0.909** and function Acc@5 / @10 **0.631 / 0.745**
   (SweRank T1). It needs no LLM at query time, but it was contrastively fine-tuned on 8×GH200 and
   must embed every function in the repository.

## 4. Honest positioning (one paragraph, for the paper)

NeuroMesh is the only method in these tables that uses no learned model at query time. It runs on a
laptop CPU in 2.9 s per issue (p50, cold index included) and costs zero tokens. The next-cheapest
setting with published cost is a fine-tuned 7B reranker on an A100 at 12.5 s and $0.011 per issue
(SweRank T5, App. G). LLM agents cost $0.46–0.87 per issue on Claude-3.5 / GPT-4o and 197k–468k
tokens on GPT-5.2 / Claude-4.5. On the shared 274-instance Lite protocol, NeuroMesh beats BM25 by
+0.11 file Acc@1 and +0.14 function Acc@5. At function Acc@5 / @10 (0.460 / 0.540) it is also ahead
of three general-purpose dense retrievers: E5-base (0.394 / 0.511), Jina-Code-v2 (0.423 / 0.522)
and CodeSage-large (0.339 / 0.445). It is level with E5-base at file Acc@1 (0.500 vs. 0.496) and
behind every dense retriever at file Acc@5 (0.755 vs. 0.781–0.847). Code-search-tuned dense
retrievers lead it clearly. CodeRankEmbed leads by +0.03 file Acc@1 and +0.06 function Acc@5, and
SweRankEmbed-Small leads by +0.16 and +0.17, at the same 137M size. Fine-tuned 7B rerankers and LLM
agents lead by 0.27–0.39 at function Acc@5, and frontier-model agents (Claude-3.5 and newer) lead by
0.28–0.33 at file Acc@1. We therefore do not claim state of the art. We claim the best measured
model-free point. It is a strong, cheap first stage: the engine's top-30 definitions are a short
list that a dense or LLM reranker can re-order (§8.18 dense vote, +0.032 function Acc@5 on dev+gym).
It is deterministic and auditable, and it runs on any machine. Two caveats apply to cross-paper
comparisons. Our Verified file Acc@5 (0.736) is above RepoMem's GPT-4o re-runs of LocAgent (0.716)
and Agentless (0.714), but agent numbers vary by about 0.09 between re-runs, as LocAgent's 0.734 vs.
0.646 function Acc@5 shows. And none of the competitors report a pre-registered holdout.

## 5. Drop-in replacement for `paper-draft.md` "## 2. Related work"

```markdown
## 2. Related work

**LLM localisers.** Agentless [Xia et al. 2025] narrows hierarchically (repository tree → files →
class/function skeletons → lines) with one model call per level plus sampling and voting. On
SWE-bench Lite it covers the gold file in 69.7% and the gold function in 52.0% of issues with GPT-4o,
at $0.70 per issue. Agent frameworks (SWE-agent, OpenHands, MoatlessTools) search through shell or
retrieval tools inside an agent loop. LocAgent [Chen et al. 2025] indexes a heterogeneous code graph
(files, classes, functions; contain/import/invoke/inherit edges) and lets an LLM agent traverse it
with entity-level BM25. It sets the reference protocol used here: 274 Lite instances and Acc@k
requiring all gold locations in the top k. With Claude-3.5 it reaches file Acc@1/5 0.777/0.942 and
function Acc@5 0.734 at $0.66 per issue, and a fine-tuned Qwen2.5-7B reaches 0.708/0.883/0.642 at
$0.05. Later systems refine the search policy: graph search with a pruner (CoSIL [Jiang et al.
2025]), priority scheduling and context pruning (OrcaLoca [Yu et al. 2025]), repository knowledge
graphs (KGCompass [Yang et al. 2025]), repository memory (RepoMem [2025]), and code graphs as plug-ins
(RepoGraph [Ouyang et al. 2025]). Another line trains small open models for localisation with RL:
ToolTrain/RepoSearcher [2025], CodeScout [2026] and FuseSearch [2026]. With frontier models the
numbers keep rising. SWE-Adept [2026] reports function Acc@5 0.878 on the 274 subset with
Claude-Sonnet-4.5 at 348k tokens per issue. The same paper re-runs LocAgent with GPT-5.2 at 0.646,
against the 0.734 originally reported with Claude-3.5, a reminder that agent numbers move by about
0.1 with the model and the run. Our engine borrows LocAgent's entity-level content ranking and
Agentless's file → function narrowing, without the model.

**Dense retrieval and learned rerankers.** CodeRankEmbed (137M) [Suresh et al. 2025] ranks function
chunks by cosine with the issue. On the 274 subset it reaches file Acc@1 0.526 and function Acc@5
0.518 (LocAgent Table 4), and a 7B listwise reranker (CodeRankLLM) lifts function Acc@5 to 0.675.
SweRank [Reddy et al. 2026] fine-tunes retrievers and rerankers on issue-localisation data mined from
GitHub. SweRankEmbed-Small (137M) reaches 0.664 / 0.631 (file Acc@1 / function Acc@5) with no LLM at
query time. SweRankEmbed-Large + SweRankLLM-Large (7B + 32B) reaches 0.832 / 0.814, at $0.015 per
issue on GPU. SweRank+ [2025] extends this to several languages and reports function Acc@5 0.854.
CoRet [Fehr et al. 2025] adds call-graph context to a 130M retriever (chunk perfect-recall@5 0.54 on
435 Verified issues). SWE-Fixer [Xie et al. 2025] uses BM25 top-30 followed by a fine-tuned 7B file
selector. BLAZE [Chakraborty et al. 2025] fine-tunes CodeSage with dynamic chunking for
cross-project bug localisation (file Top-1 0.29 on SWE-bench). These methods are the strongest
non-agent localisers. All of them require training data close to the task, an embedding of every
function in the repository, and in practice a GPU. We measure the CPU cost of whole-repository
embedding, and of a short-list variant that embeds only the engine's top definitions (§6, §8.18).

**Classic bug localisation.** IR-based bug localisation (BugLocator: rVSM plus similar fixed bugs;
Locus: change hunks; Rahman & Roy: query reformulation) uses report text, history and structure. We
test the history and reformulation (RM3) ideas under the same protocol, and both are negative on
SWE-bench dev. MULocBench [2025] shows that localisation of issues that are not code (configuration,
documentation, build) is much harder. Even LocAgent + Claude-3.5 reaches only file Acc@5 0.352 there,
and BM25 0.094. This motivates our plain-language holdouts (§5.4).

**Ranking machinery.** BM25F (Robertson et al.), reciprocal rank fusion (Cormack et al. 2009) and
convex lexical/dense fusion (Bruch et al. 2023). Local-first assistants: Aider's RepoMap (PageRank
over tags; we measure it as a retriever, R@5 0.04–0.48 on our sets), Continue and Cody (retrieve +
rerank; a code-trained cross-encoder did not survive our holdout).

**Positioning.** No published method in this space is free of learned models at query time. All the
numbers above are single-split results; none is reported on a pre-registered holdout. We take the
other corner of the trade-off: no model, CPU only, zero tokens, about 3 s per issue, every number
from a holdout run once. Our engine trails code-tuned dense retrievers (−0.03 to −0.16 file Acc@1 on
the shared Lite protocol) and LLM agents (−0.28 and more). It is ahead of BM25 and of general-purpose
dense retrievers at function level (§5.1).
```

## 6. References (bibtex-ish)

```bibtex
@inproceedings{chen2025locagent, title={LocAgent: Graph-Guided LLM Agents for Code Localization},
  author={Chen, Zhaoling and others}, booktitle={ACL}, year={2025}, note={arXiv:2503.09089; aclanthology 2025.acl-long.426}}
@inproceedings{xia2025agentless, title={Agentless: Demystifying LLM-based Software Engineering Agents},
  author={Xia, Chunqiu Steven and Deng, Yinlin and Dunn, Soren and Zhang, Lingming}, booktitle={FSE}, year={2025}, note={arXiv:2407.01489}}
@inproceedings{reddy2026swerank, title={SweRank: Software Issue Localization with Code Ranking},
  author={Gangi Reddy, Revanth and others}, booktitle={ICLR}, year={2026}, note={arXiv:2505.07849}}
@misc{swerankplus2025, title={SweRank+: Multilingual, Multi-Turn Code Ranking for Software Issue Localization}, year={2025}, note={arXiv:2512.20482}}
@inproceedings{suresh2025cornstack, title={CoRNStack: High-Quality Contrastive Data for Better Code Retrieval and Reranking},
  author={Suresh, Tarun and others}, booktitle={ICLR}, year={2025}, note={arXiv:2412.01007; CodeRankEmbed, CodeRankLLM}}
@inproceedings{jiang2025cosil, title={Issue Localization via LLM-Driven Iterative Code Graph Searching},
  author={Jiang, Zhonghao and Ren, Xiaoxue and Yan, Meng and Jiang, Wei and Li, Yong and Liu, Zhongxin}, booktitle={ASE}, year={2025}, note={arXiv:2503.22424}}
@inproceedings{yu2025orcaloca, title={OrcaLoca: An LLM Agent Framework for Software Issue Localization},
  author={Yu, Zhongming and Zhang, Hejia and Zhao, Yujie and Huang, Hanxian and Yao, Matrix and Ding, Ke and Zhao, Jishen}, booktitle={ICML}, year={2025}, note={arXiv:2502.00350}}
@inproceedings{ouyang2025repograph, title={RepoGraph: Enhancing AI Software Engineering with Repository-level Code Graph},
  author={Ouyang, Siru and others}, booktitle={ICLR}, year={2025}, note={arXiv:2410.14684}}
@misc{kgcompass2025, title={KGCompass: Enhancing Repository-Level Software Repair via Repository-Aware Knowledge Graphs}, year={2025}, note={arXiv:2503.21710}}
@misc{repomem2025, title={Improving Code Localization with Repository Memory}, year={2025}, note={arXiv:2510.01003}}
@inproceedings{tooltrain2026, title={Tool-integrated Reinforcement Learning for Repo Deep Search (Enhancing Issue Localization Agent with Tool-Interactive Training)}, booktitle={ICSE}, year={2026}, note={arXiv:2508.03012}}
@misc{codescout2026, title={CodeScout}, year={2026}, note={arXiv:2603.17829; OpenHands}}
@inproceedings{fusesearch2026, title={Learning Adaptive Parallel Execution for Efficient Code Localization},
  author={Xu, Ke and others}, booktitle={Findings of ACL}, year={2026}, note={arXiv:2601.19568}}
@misc{sweadept2026, title={SWE-Adept: An LLM-Based Agentic Framework for Deep Codebase Analysis and Structured Issue Resolution}, year={2026}, note={arXiv:2603.01327}}
@inproceedings{sherloc2026, title={SHERLOC: Structured Diagnostic Localization for Code Repair Agents},
  author={Tamoyan, Hovhannes and Narenthiran, Sean and Arakelyan, Erik and Mezini, Mira and Ginsburg, Boris}, booktitle={EMNLP}, year={2026}, note={arXiv:2606.24820}}
@inproceedings{fehr2025coret, title={CoRet: Improved Retriever for Code Editing},
  author={Fehr, Fabio and Sivaprasad, Prabhu Teja and Franceschi, Luca and Zappella, Giovanni}, booktitle={ACL (short)}, year={2025}, note={arXiv:2505.24715}}
@inproceedings{xie2025swefixer, title={SWE-Fixer: Training Open-Source LLMs for Effective and Efficient GitHub Issue Resolution},
  booktitle={Findings of ACL}, year={2025}, note={arXiv:2501.05040}}
@article{blaze2025, title={BLAZE: Cross-Language and Cross-Project Bug Localization via Dynamic Chunking and Hard Example Learning},
  journal={IEEE TSE}, year={2025}, note={arXiv:2407.17631; doi 10.1109/TSE.2025.3579574}}
@misc{mulocbench2025, title={A Benchmark for Localizing Code and Non-Code Issues in Software Projects}, year={2025}, note={arXiv:2509.25242}}
```

Author lists marked "and others" and the KGCompass/BLAZE full titles should be checked against the
arXiv abstract pages before camera-ready. They were not all visible in the pages read.

## 7. Source list (pages read 2026-10-17)

arxiv.org/html/2503.09089 (LocAgent T4, T5) · 2505.07849 (SweRank T1, T2, T5, T12, App. A/G) ·
2512.20482 (SweRank+ T2–T4) · 2412.01007 (CoRNStack T6) · 2503.22424 (CoSIL T-II, T-III, T-VII) ·
2502.00350 (OrcaLoca T1, T2, §4.1.3) · 2407.01489 (Agentless T1) · 2410.14684 (RepoGraph T2, T3) ·
2503.21710 (KGCompass T1, T2, T5) · 2501.05040 (SWE-Fixer T3) · 2407.17631 (BLAZE T3, T4) ·
2510.01003 (RepoMem T1, T3) · 2508.03012v2 (ToolTrain T2) · 2603.17829 (CodeScout T3, T5) ·
2603.01327v3 (SWE-Adept T1, App. A) · 2505.24715 (CoRet T1, T2) · abs/2606.24820 (SHERLOC abstract) ·
abs/2601.19568 (FuseSearch abstract) · 2509.25242 (MULocBench T1, T2).
