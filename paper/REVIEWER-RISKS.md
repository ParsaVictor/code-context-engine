# Reviewer risks: the 10 hardest objections

These are written the way a hostile but fair reviewer would put them. Each one gives our answer
as the paper stands today and, where the answer is not enough, the experiment that would close
the gap. Numbers come from `docs/research/contributions-log.md` (cited as log §x) and
`docs/research/related-work-2026.md`. They are ordered by how likely each objection is to sink
the paper.

---

## 1. "This is below a 137M retriever on every metric. Why publish a weaker method?"

SweRankEmbed-Small, used as a whole-repository retriever, has file Acc@1 0.664 and function Acc@5
0.631 on the same 274 instances. Ours are 0.500 and 0.460 for the engine alone. With a vote from a
short list of 30 definitions, function Acc@5 is 0.485 (jina-code, shippable) or 0.496
(SweRankEmbed-Small itself, research only). The gap to the same model used over the whole
repository is therefore −0.135 at function Acc@5. CodeRankEmbed is also ahead at file Acc@1 (0.526),
but our hybrid's function Acc@5 (0.496) is now only −0.022 behind its 0.518.

**Our answer.** We do not claim state of the art, and the abstract says so. Our claim is a
different point on the cost/accuracy curve:

- no trained model at query time;
- no embedding of the whole repository;
- zero tokens;
- 2.9 s per issue on a laptop CPU, cold index included;
- every number from a holdout run once.

None of the methods in Table 1 sits at this point. The strongest new answer is the **rerank
ceiling** (log §8.20). The engine's top 300 definitions contain every edited function for 0.828 of
Lite-274 issues (Verified 0.790; top 30: 0.672 / 0.627). That is above the function Acc@5 of
SweRank's best retrieve-and-rerank system (0.814), although below SweRank+ (0.854) and SWE-Adept
(0.878). The engine is a CPU candidate generator whose candidate set is large enough for the best
published accuracy. What it lacks is ordering, and we show that cheap short-list reranking closes
part of that gap, on both holdouts:

- SweRank vote, Lite: +0.036 [0.011, 0.062], p = 0.013;
- SweRank vote, Verified: +0.044 [0.024, 0.066], p = 0.0001.

**Expect the counter-objection.** "Recall@300 is not accuracy; comparing it with Acc@5 is
apples to oranges." The paper says this explicitly ("for scale, not as a like-for-like
comparison"). Do not put 0.828 next to 0.814 without that sentence.

**What is still missing.**

- (a) A strong reranker over our top 30–300 that actually *reaches* the ceiling. This could be a
  listwise LLM reranker (SweRankLLM-Small 7B or an API model) over the engine's list. It would show
  how much of the 0.828 is reachable, and at what cost compared with whole-repository retrieval. This
  is the same experiment as #3, and it is now the single most valuable one.
- (b) The real CPU cost of the whole-repository alternative: SweRankEmbed-Small and CodeRankEmbed
  indexing every function of the Lite repositories on the same laptop (see #2). Today we have only
  2–3 texts/s from the short-list runs, which implies hours per large repository. A direct
  measurement would turn the cost argument from an estimate into a number.
- (c) An int8 ONNX export of SweRankEmbed-Small, so that its short-list cost (11–15 s per issue in
  fp32) can be compared fairly with jina's (about 6 s).

## 2. "The CPU cost of dense retrieval is exaggerated."

Our 2.6 chunks/s comes from one measurement: jina-code-v2, int8, 6 threads, 256-token chunks, on a
machine already loaded with other runs (log §8.1). A reviewer will point out four things:

- a 137M encoder with batching and ONNX/OpenVINO on an idle CPU is much faster than that;
- the index is built once per repository and updated incrementally;
- spread over many issues on the same repository, the per-issue cost is small;
- SWE-bench has only 12 Lite repositories.

**Our answer.** The measurement is what it is, and the paper calls it "hours for django-sized
repositories", not a precise figure. Today that is all we have.

**What is missing.** A clean cost table:

- idle machine, Linux and Windows;
- batch sizes 1, 8 and 32; fp32 and int8;
- CodeRankEmbed and SweRankEmbed-Small on 3 to 5 repositories of different sizes;
- the incremental re-index cost after a typical commit;
- peak memory.

Report both the amortised per-issue cost and the cold cost. This could shrink our cost argument, so
run it before a reviewer does.

## 3. "The paper is about coding agents, but there is no agent and no LLM stage."

The title and introduction motivate localisation as the first step of an agent. But no experiment
measures an agent, and no experiment measures an LLM reading `where_to_look`. The 3B local-LLM pilot
(15 issues) was negative. The API-model stage was blocked by unfunded keys (log §8.13, §8.9).

**Our answer.** The evidence that the engine is a good first stage is indirect:

- 45% of function misses have every gold function in the list but outside the top 5 (log §8.18), so
  ordering, not recall, is the bottleneck;
- the dense vote improves exactly that ordering.

The paper names the missing LLM stage as its main gap (§Threats).

**What is missing.** This is the most important experiment, in two parts.

- (a) An Agentless-style single LLM call that picks files and functions from our top 10 / top 30,
  using one open model (Qwen2.5-Coder-7B/32B) and one API model. Report Acc@1/5 and tokens per issue
  next to Agentless and LocAgent.
- (b) If budget allows, an end-to-end resolve rate. Take one agent (OpenHands or SWE-agent) with and
  without `where_to_look` in its first prompt, on a 100-issue Lite subset. Report tokens per resolved
  issue.

## 4. "'Pre-registered holdout' is a self-attested git commit. This is not pre-registration."

The gold, the split scripts and the dense-vote decision (608acb4) are committed before the runs.
But the repository is ours, and commits can be rewritten. "Run once" is also only our word.

**Our answer.** The commits were pushed to a public GitHub repository before each run, and GitHub
records push times. All result files, including the rejected runs, are in the repository. The
negative results table shows that we report what failed.

**What is missing.**

- Archive the gold and decision files on Zenodo or OSF, which gives each one a DOI and a timestamp
  that we cannot change.
- Add a table of commit hashes and push times next to each holdout run.
- For the final paper, consider a real pre-registration on OSF for one new holdout. A good candidate
  is SWE-bench-Live, or issues created after our release date.

Without these steps, call it a "commit-locked holdout", not "pre-registered".

## 5. "The dense vote went through forking paths, and on Lite p = 0.065 is not significant."

The vote failed the bar on dev. It was then re-measured on a *new* split until it passed, and only
then run on Lite. On Lite, the CI [0.004, 0.051] barely excludes zero, and the sign test (9 issues
gained, 2 lost) gives p = 0.065.

**Our answer.** The weight and k were not changed between dev and SWE-Gym. The decision was
committed before Lite and Verified. The effect is the same direction and size on every split
(+0.032 pooled on dev + gym, +0.026 on Lite, +0.028 on Verified). Verified, run once with the locked
decision, is significant on its own: function Acc@5 0.429 → 0.458, CI [0.009, 0.048], 17 issues
gained and 4 lost, sign test p = 0.007; at @1, +0.039, p = 0.005. The paper reports the dev failure
and states this threat explicitly. It is opt-in, not the default.

**What is missing.**

- The jina Lite and Verified numbers come from the Python prototype applied to the stored v1.4.0
  runs, which held only 20 candidates, so they are a lower bound for the decided k=30. The
  SweRankEmbed-Small vote, which was pre-registered and passed all four tuning halves, ran over
  regenerated 300-entry lists and is significant on both holdouts.
  A run of the Rust port (`NM_DENSE=1`) on both would remove "prototype vs shipped" as a follow-up
  objection.
- A Holm correction across the three holdouts (Lite, Verified, Gym-by-port) would pre-empt the
  multiple-comparisons version of this objection.

## 6. "Your BM25 is suspiciously weak."

On the same 274 instances our BM25 has file Acc@1 0.299, against 0.387 for LocAgent's BM25. A
reviewer may suspect a weak baseline that inflates "×2.3 over BM25" and "+0.21 over BM25".

**Our answer.** Table 1 reports both. Against LocAgent's stronger BM25 the gains are still +0.11
file Acc@1 and +0.14 function Acc@5. The SWE-Gym ×2.3 figure is relative to our own BM25 only.

**What is missing.**

- A third, standard BM25: Pyserini/Lucene with default k1 = 0.9, b = 0.4, over files and over
  functions with the same file aggregation.
- A sentence on *why* ours is lower. Possible causes are tokenisation of identifiers, scoring whole
  files versus taking the best function, and handling of test files.
- Recompute the SWE-Gym ratio against the standard BM25.

This is cheap and removes an easy attack.

## 7. "Python only, one benchmark family. The claims about generality are not supported."

Every SWE-bench-style set is Python. The plain-language holdouts cover five languages, but with 12
questions each, one annotator, and gold written by the authors. One question moves R@3 by 0.083.

**Our answer.** The paper reports plain-language results as "a direction, not a precise effect".
The engine's parsers cover Rust, TypeScript, Go, Java and others, but the SWE-bench-style numbers
apply only to Python.

**What is missing.**

- A multilingual issue benchmark: SWE-bench-java, Multi-SWE-bench, SWE-PolyBench, or SweRank+'s
  multilingual set, which also gives a head-to-head with SweRankEmbedMulti.
- Plain-language holdouts of 50 or more questions with a second annotator and inter-annotator
  agreement.
- MULocBench's non-code issues, because the related work cites it as motivation.

## 8. "Contamination and leakage: the engine was hand-tuned over many sessions by people who saw the benchmarks."

The heuristics were written by developers who knew SWE-bench well. That includes hygiene rules for
issue templates, rule-code tokens, and traceback parsing. 24 Lite instances were looked at. Django
and the other Lite repositories are well known.

**Our answer.**

- The strict row excludes the 24 looked-at instances, and the 250-instance subset shows they do not
  flatter the result.
- Tuning used only dev, whose repositories are disjoint from Lite.
- SWE-Gym's seven repositories were never seen before the v1.4.0 run, and the gain held there
  (×2.3).
- The engine has no learned weights, so pre-training contamination does not apply to it. It does
  apply to the encoders we compare against.

**What is missing.**

- A holdout made of issues *created after* the v1.4.0 release date, for example from SWE-bench-Live
  or new issues mined the same way as SWE-Gym. No heuristic could have been fitted to them.
- A list of every rule in the hygiene component, with where it came from (which dev issue).

## 9. "No research novelty: BM25F + RRF + traceback parsing is engineering."

**Our answer.** The contribution is the measurement, not the components:

- the ablation shows which LLM-free component carries the gain (definition-level ranking, −0.10
  Acc@1 when removed);
- the query-length split for cross-encoders;
- the finding that blind graph expansion hurts while LLM-guided expansion helps;
- the finding that a learned reranker over lexical features does not transfer (coefficients change
  sign);
- the explanation that dev understates function-level gains because of gold size;
- a dozen negative results with numbers.

Negative results and protocols are publishable at venues that value them (MSR, ICSE/FSE "new ideas
and emerging results", or the TOSEM/EMSE replication tracks). This paper is probably a better fit
there than at a main ML track.

**What is missing.**

- A sharper single insight in the abstract. The best candidate is "ordering, not recall, is the
  bottleneck for LLM-free localisers: 45% of misses rank the gold at about 36".
- A per-component failure taxonomy on the holdout (not only dev), so the analysis section stands on
  held-out data.

## 10. "The function-level gold may not be LocAgent's gold, so the head-to-head is not exact."

We rebuild function gold ourselves (the innermost function containing a removed line or insertion
point, from Python `ast`). The *count* matches LocAgent's 274 exactly, but the *per-instance* gold
sets might differ. For example, LocAgent may count class-level edits, decorators, or added
functions differently. A difference of a few instances moves Acc@5 by 0.01 to 0.02. That is the
same size as the dense-vote effect, and close to the gap to CodeRankEmbed.

**Our answer.** Matching the 274 count is evidence that the gold extraction is right, but it is not
proof.

**What is missing.**

- Score our stored outputs with LocAgent's released evaluation script and gold, if it is public.
- Otherwise, take its released per-instance predictions or gold, report the per-instance agreement
  rate of the gold sets, and re-score.

This is cheap and should be done before submission.

---

## Smaller objections to pre-empt in the text

- **Timing hygiene.** Timing comes from one Windows laptop that was shared with other jobs. Re-time
  on an idle Linux machine and report p50/p90 by repository size (part of #2).
- **CI overlap between versions.** v1.3 and v1.4 file-level results are identical, and the
  function-level CIs overlap. Paired tests are reported only for the dense vote. Add paired tests
  for v1.3 → v1.4 at function Acc@5 (gained/lost counts are in the stored results).
- **Ablation is on v1.2.0.** The ablation predates two votes. Re-run the ablation on v1.4.0 with the
  named-files and traceback votes as rows.
- **Packet size / tokens.** The packet's median of 11.1k tokens is reported, but packet precision
  is not. The agreed "next metric" (token_precision) would answer "how much of what you hand the
  agent is useful?"
- **Upstream credit.** NeuroMesh is a fork of pinoox/neuromesh. The acknowledgements credit the
  upstream project. The paper must make clear which components were added in this work (graph
  link-resolution fixes, report mode, definition ranking, named files, traceback vote, dense vote).
