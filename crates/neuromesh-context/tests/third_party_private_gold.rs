//! Phase 5b: a private repository as holdout. Nothing about it lives in this
//! repository — the manifest, the gold and the checkout are all outside, named
//! by two environment variables; only the summary numbers are ever written
//! down (`docs/planning/stage5-findings.fa.md`, anonymised).
//!
//! ```text
//! NM_THIRD_PARTY=1 NM_PRIVATE_SET_DIR=/path/to/private-holdout NM_PRIVATE_DIR=/path/to/checkouts \
//!   cargo test -p neuromesh-context --test third_party_private_gold -- --nocapture
//! ```
//!
//! `NM_PRIVATE_SET_DIR` holds `repos.toml` (only `name =` is read) and
//! `<name>/gold_tasks.toml`; `NM_PRIVATE_DIR/<name>` is the checkout. Same
//! target-only gate as the public holdouts, not a ratchet, not in CI.

#[path = "support/gold_set.rs"]
mod gold_set;

const MIN_MEAN_RECALL: f32 = 0.90;
const MIN_MEAN_PRECISION: f32 = 0.60;
const MAX_FORBIDDEN_HITS: usize = 0;

#[test]
fn private_repository_gold_and_task_oracle() {
    if std::env::var("NM_THIRD_PARTY").is_err() || std::env::var("NM_PRIVATE_SET_DIR").is_err() {
        eprintln!("third_party_private_gold: NM_THIRD_PARTY or NM_PRIVATE_SET_DIR not set; skipping");
        return;
    }
    let s = gold_set::run_gold_set("private");
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
