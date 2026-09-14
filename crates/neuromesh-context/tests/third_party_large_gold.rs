//! Gold set on the large dev repositories (`tests/third_party/large/`):
//! django (3513 files) and ultralytics (931 files). They were the first
//! holdout pair (phase 5a, 2026-09-13) and rotated into dev once we decided
//! to fix what they exposed — F33–F36 in docs/planning/stage5-findings.fa.md.
//!
//! Why a third file instead of adding them to `third_party_gold.rs`: the four
//! small dev repos sit at precision 0.856 and these two at 0.146. One mean
//! over all six would let a large-repo regression hide behind a small-repo
//! gain (or the reverse). Each set ratchets on its own.
//!
//! ```text
//! bash scripts/fetch-third-party.sh tests/third_party/large/repos.toml large
//! NM_THIRD_PARTY=1 cargo test -p neuromesh-context --test third_party_large_gold -- --nocapture
//! ```
//!
//! Ratchet: set at the value measured on the commit that added this file
//! (after correcting two gold-authoring mistakes from the holdout run —
//! recorder.py and metrics.py are real dependencies, not decoys). Each
//! constant moves up as F33–F36 land, never down.
//!
//! Raised twice: F33 (owner-qualified dunder members) fixed
//! `django_wsgi_entry`'s `asgi.py` forbidden hit — precision 0.173 -> 0.257,
//! forbidden 2 -> 1, oracle reachable 17/20 -> 18/20. F34 (bare single-hump
//! capitalised words dropped from the no-structural-signal identifier scan,
//! see `neuromesh-parser/src/identifiers.rs`) raised precision again,
//! 0.257 -> 0.269, no other metric moved.

#[path = "support/gold_set.rs"]
mod gold_set;

const MIN_MEAN_RECALL: f32 = 0.94; // measured 0.950
const MIN_MEAN_PRECISION: f32 = 0.50; // measured 0.504 (F45)
const MAX_FORBIDDEN_HITS: usize = 0; // measured 0 (F44: trainer.py homonym seed gone)
const MIN_TASK_REACHABLE: f32 = 0.94; // measured 19/20 (F44)

#[test]
fn large_repositories_gold_and_task_oracle() {
    if std::env::var("NM_THIRD_PARTY").is_err() {
        eprintln!("third_party_large_gold: NM_THIRD_PARTY not set; skipping (run scripts/fetch-third-party.sh tests/third_party/large/repos.toml large first)");
        return;
    }
    let s = gold_set::run_gold_set("large");
    assert!(
        s.mean_recall >= MIN_MEAN_RECALL,
        "mean recall {:.3} < {MIN_MEAN_RECALL}",
        s.mean_recall
    );
    assert!(
        s.mean_precision >= MIN_MEAN_PRECISION,
        "mean precision {:.3} < {MIN_MEAN_PRECISION}",
        s.mean_precision
    );
    #[allow(clippy::absurd_extreme_comparisons)]
    let within_budget = s.forbidden_hits <= MAX_FORBIDDEN_HITS;
    assert!(
        within_budget,
        "{} forbidden files shipped (max {MAX_FORBIDDEN_HITS})",
        s.forbidden_hits
    );
    assert!(
        s.reachable_rate() >= MIN_TASK_REACHABLE,
        "task oracle reachable {:.3} < {MIN_TASK_REACHABLE}",
        s.reachable_rate()
    );
}
