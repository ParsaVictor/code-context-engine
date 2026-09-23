# Contributing

Build, test and style commands live in [docs/contributing.md](../docs/contributing.md).
This page is the short version of the one rule that matters here.

## Every retrieval change is measured

The project publishes recall and precision on repositories it was never tuned
on ([docs/measured.md](../docs/measured.md)). That only means something if the
rule holds for patches too:

```bash
bash scripts/benchmark-holdout.sh            # all nine public sets
bash scripts/benchmark-holdout.sh holdout-c  # one of them
```

- **Holdout sets are never tuned on.** `holdout-2`, `holdout-c`, `holdout-lang`
  and `holdout-ml2` are gates. If your change lowers one, it does not ship —
  find the signal it lost, or revert it.
- **A mixed result is not a win.** One set up and another down means a real
  signal is missing. `NM_EXPLAIN=1 NM_EXPLAIN_MAX_PRECISION=1.01` writes the
  reason for every file in every packet to `target/explain-<set>.txt`; the
  answer is in there, not in a threshold.
- **Name the signal.** A file belongs in a packet because something points at
  it: a call edge, an import, a config key, a quoted literal, a word of its
  path the prompt wrote. "The score went up" is not a reason.
- **Gold is written before the engine runs.** If you add a question, write the
  expected files by reading the source first, then run the engine. Never the
  other way round.

## Good first contributions

- A language the parser does not cover yet (`crates/neuromesh-parser/src/queries/*.scm`).
- A framework idiom that hides the real dependency — a DI container, a route
  table, a decorator. Open an issue with a public repository where it appears.
- A gold set for a repository family nobody has measured (see
  `tests/third_party/*/repos.toml` for the shape).

Commits are plain English, present tense, describing behaviour.
