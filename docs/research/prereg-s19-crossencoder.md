# Pre-registration: cross-encoder vote on the function short list (session 19, P1b)

Committed before any number from `scripts/research/ce_shortlist.py`.

**Question.** Does a cross-encoder (jina-reranker-v2, int8 ONNX, local, Apache-2.0 base; no GPU)
re-ranking the engine's top-30 functions improve the function list, as the dense votes did (§8.18,
§8.19)? Earlier cross-encoder tests (§6 of the paper) were file-level reranks of reports and
short questions — this is function-level and a vote, not a replacement.

**Fixed.** Engine lists: dev `dev-s18a-all.jsonl`, SWE-Gym `gym-v140-all.jsonl` (300 entries).
k=30; texts as dense_shortlist.py; issue first 2000 characters; pair max 1024 tokens
(longest-first); RRF k=60, weight **w=0.5** (primary).

**Bar.** func Acc@5 of engine + cross-encoder vote ≥ engine + 0.02 on each of dev-fast, dev-rest,
gym-a, gym-b. Secondary (reported, not decisive): w=1, cross-encoder order alone, paired vs the
SweRankEmbed vote (§8.19).

**Holdout rule.** If the bar passes, Lite/Verified run once with exactly this variant over the
regenerated 300-entry lists (§8.20). If it fails, nothing is run on the holdouts.
