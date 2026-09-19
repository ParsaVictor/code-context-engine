//! Phase D (session 12) fresh ML holdout — peft + keras-hub, library-shaped; holdout-ml was tuned on in D-3 (`tests/third_party/holdout-ml2/`):
//! peft (Hugging Face adapter library), keras-hub (Keras 3 model library). Gold written from the source before any engine run.
//! Target-only gate (phase-A/B thresholds), not a ratchet and not in CI.
//!
//! ```text
//! bash scripts/fetch-third-party.sh tests/third_party/holdout-ml2/repos.toml holdout-ml2
//! NM_THIRD_PARTY=1 cargo test -p neuromesh-context --test third_party_ml_holdout_gold -- --nocapture
//! ```

#[path = "support/gold_set.rs"]
mod gold_set;

const MIN_MEAN_RECALL: f32 = 0.90;
const MIN_MEAN_PRECISION: f32 = 0.60;
const MAX_FORBIDDEN_HITS: usize = 0;

#[test]
fn ml2_holdout_repositories_gold_and_task_oracle() {
    if std::env::var("NM_THIRD_PARTY").is_err() {
        eprintln!("third_party_ml_holdout_gold: NM_THIRD_PARTY not set; skipping (run scripts/fetch-third-party.sh tests/third_party/holdout-ml2/repos.toml holdout-ml2 first)");
        return;
    }
    let s = gold_set::run_gold_set("holdout-ml2");
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
