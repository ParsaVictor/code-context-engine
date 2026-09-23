## What this changes

<!-- One paragraph: the behaviour, not the diff. -->

## Why — the signal, not the threshold

<!--
A retrieval change is accepted when it names the signal it uses (a call edge, an
import, a config key, a literal, a path word), not when a score was raised until
a set went green. If you tuned a number, say which one and why it is principled.
-->

## Measurement

<!--
Required for anything that touches retrieval. Run `bash scripts/benchmark-holdout.sh`
and paste the table before and after. Holdout sets (holdout-2, holdout-c,
holdout-lang, holdout-ml2) must not drop — they are never tuned on.
-->

| set | before (recall / precision / forbidden) | after |
|---|---|---|
|  |  |  |

- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo fmt --all --check` clean
- [ ] No holdout set dropped; no new `forbidden` file
