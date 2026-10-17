# Pre-registration: SweRankEmbed-Small as the dense short-list embedder (session 19, 2026-10-17)

Committed before any SweRankEmbed-Small number is computed on dev or SWE-Gym.

**Question.** Does Salesforce SweRankEmbed-Small (137M NomicBERT, CLS pooling, query prefix
"Represent this query for searching relevant code: ", 512-token cut; run in fp32 torch on CPU by
`scripts/research/swerank_embed.py`) beat the engine as the embedder of the dense short-list vote,
in place of jina-code-v2 (§8.18)?

**Fixed setup (identical to §8.18 except the embedder).** Engine runs `dev-s18a-all.jsonl` (dev, 225)
and `gym-v140-all.jsonl` (SWE-Gym, 280); top k=30 definitions; same definition texts
(`path\nname\nbody`, 1500 chars) and issue text (4000 chars); RRF k=60, vote weight w=0.5;
`scripts/research/dense_shortlist.py --model swerank-embed-small --threads 6`, own cache file.

**Primary bar.** func Acc@5 of engine + SweRank vote (w=0.5) ≥ engine + 0.02 on **each** of the four
halves: dev-fast, dev-rest, gym-a, gym-b (`priors.py --prior ext`). Pass on all four → SweRank is a
candidate replacement for jina-code; any half below +0.02 → rejected as the vote embedder.

**Secondary (reported, not decisive).** w=1; dense order alone (SweRank cosine order of the top 30,
no fusion); pooled dev+gym paired comparison vs engine and vs the jina vote (`paired.py`, sign test,
bootstrap CI); seconds per issue (p50) on CPU.

**Out of scope here.** Lite and Verified are holdouts: no run on them in this branch; the main session
decides whether a Lite/Verified run follows.

## Addendum (k=100), committed after the k=30 result and before any k=100 number

k=30 result: the SweRank vote cleared the bar on all four halves (§8.19). The engine's top 30 holds
every gold function for only 0.50 / 0.47 of dev / gym issues; the top 100 holds them for 0.63 / 0.62.
Second test, same embedder, texts, query, RRF k=60 and w=0.5, only the short list widened to the
engine's top **k=100** definitions (`--k 100`, same cache file, `--threads 3`).

**Bar (unchanged):** func Acc@5 of engine + SweRank vote over the top 100 (w=0.5) ≥ engine + 0.02 on
each of dev-fast, dev-rest, gym-a, gym-b. **Secondary:** SweRank order alone over the top 100; w=1;
paired vs the k=30 SweRank vote (pooled); seconds per issue with the text cache warm from k=30.
The k=30 decision is not revisited by this test.
