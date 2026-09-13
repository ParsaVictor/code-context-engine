//! Gold set on the current holdout repositories (`tests/third_party/holdout/`)
//! — repos the engine has never been tuned on. Rotation 2 (plan revision
//! 2026-09-14): gin + torchvision. Rotation 1 (django + ultralytics) moved to
//! `tests/third_party/large/` once we decided to fix what it exposed.
//!
//! Separate file and separate gate from the dev sets on purpose: a holdout
//! regression must never be able to hide behind a dev improvement.
//!
//! ```text
//! bash scripts/fetch-third-party.sh tests/third_party/holdout/repos.toml holdout
//! NM_THIRD_PARTY=1 cargo test -p neuromesh-context --test third_party_holdout_gold -- --nocapture
//! ```
//!
//! The thresholds are the phase-A/B gate from the plan revision, not a
//! ratchet from a measurement: this test is *expected* to fail until the
//! generalization fixes (F33–F36) land, and it is not wired into CI for that
//! reason. When it passes, it becomes the number the project reports.

#[path = "support/gold_set.rs"]
mod gold_set;

const MIN_MEAN_RECALL: f32 = 0.90;
const MIN_MEAN_PRECISION: f32 = 0.60;
const MAX_FORBIDDEN_HITS: usize = 0;

#[test]
fn holdout_repositories_gold_and_task_oracle() {
    if std::env::var("NM_THIRD_PARTY").is_err() {
        eprintln!("third_party_holdout_gold: NM_THIRD_PARTY not set; skipping (run scripts/fetch-third-party.sh tests/third_party/holdout/repos.toml holdout first)");
        return;
    }
    let s = gold_set::run_gold_set("holdout");
    assert!(
        s.mean_recall >= MIN_MEAN_RECALL,
        "mean recall {:.3} < {MIN_MEAN_RECALL} (holdout gate)",
        s.mean_recall
    );
    assert!(
        s.mean_precision >= MIN_MEAN_PRECISION,
        "mean precision {:.3} < {MIN_MEAN_PRECISION} (holdout gate)",
        s.mean_precision
    );
    #[allow(clippy::absurd_extreme_comparisons)]
    let within_budget = s.forbidden_hits <= MAX_FORBIDDEN_HITS;
    assert!(
        within_budget,
        "{} forbidden files shipped (max {MAX_FORBIDDEN_HITS})",
        s.forbidden_hits
    );
}
