//! Gold set on real third-party repositories.
//!
//! The 54 fixture cases are on code we wrote, which is the test that cannot
//! fail. These cases run on pinned checkouts of four repositories nobody
//! wrote for us (`tests/third_party/repos.toml`), through the production
//! signature, and score both file-level gold (recall, precision, forbidden)
//! and symbol-level sufficiency (the task oracle).
//!
//! Gated behind `NM_THIRD_PARTY=1` because it needs the checkouts:
//!
//! ```text
//! bash scripts/fetch-third-party.sh          # into target/third_party
//! NM_THIRD_PARTY=1 cargo test -p neuromesh-context --test third_party_gold -- --nocapture
//! ```
//!
//! With the variable set and a checkout missing, the test fails — silently
//! passing on an absent repository is the failure mode this file exists to
//! remove. The thresholds are a ratchet: set at the measured value when the
//! gate was written, raised when the engine improves, never lowered to make
//! a red run green.

use neuromesh_context::gold::{
    evaluate_view, gold_file_hit, load_gold_tasks, packet_file_names, packet_paths,
    production_signature, GoldTask,
};
use neuromesh_context::task_harness::{oracle_outcome, parse_task_toml, Presence};
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Ratchet. First measured 2026-09-12 on the commit that added this gate
/// (recall 0.90, precision 0.146, forbidden 12, oracle 14/21); each constant
/// moves up with the engine, never down.
const MIN_MEAN_RECALL: f32 = 0.94; // measured 0.95 (path steer)
const MIN_MEAN_PRECISION: f32 = 0.19; // measured 0.196 — the real number, not the fixture 0.89
const MAX_FORBIDDEN_HITS: usize = 11; // measured 11 (path steer)
const MIN_TASK_REACHABLE: f32 = 0.6; // measured 14/21

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root")
}

fn checkout_dir(root: &Path) -> PathBuf {
    std::env::var("NM_THIRD_PARTY_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("target").join("third_party"))
}

fn repo_names(root: &Path) -> Vec<String> {
    let raw = std::fs::read_to_string(root.join("tests/third_party/repos.toml"))
        .expect("tests/third_party/repos.toml");
    raw.lines()
        .filter_map(|l| l.trim().strip_prefix("name"))
        .filter_map(|rest| rest.split_once('='))
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
        .collect()
}

#[test]
fn real_repositories_gold_and_task_oracle() {
    if std::env::var("NM_THIRD_PARTY").is_err() {
        eprintln!("third_party_gold: NM_THIRD_PARTY not set; skipping (run scripts/fetch-third-party.sh first)");
        return;
    }
    let root = workspace_root();
    let checkouts = checkout_dir(&root);
    let names = repo_names(&root);
    assert!(!names.is_empty(), "no repositories in repos.toml");

    let mut recalls = Vec::new();
    let mut precisions = Vec::new();
    let mut forbidden_hits = 0usize;
    let mut task_total = 0usize;
    let mut task_reachable = 0usize;
    let mut task_strict = 0usize;
    let mut lines: Vec<String> = Vec::new();

    for name in &names {
        let repo = checkouts.join(name);
        assert!(
            repo.is_dir(),
            "{name}: checkout missing at {} — run scripts/fetch-third-party.sh",
            repo.display()
        );
        let pid = ProjectId::new(name);
        let graph = NeuralProjectGraph::new(pid.clone());
        graph.set_workspace(&repo);
        let scanned = ProjectWalker::new(repo.clone(), pid)
            .scan()
            .unwrap_or_else(|e| panic!("{name}: scan failed: {e}"));
        graph.ingest_workspace(&scanned);
        lines.push(format!("== {name}: {} files", scanned.len()));

        let gold_path = root
            .join("tests/third_party")
            .join(name)
            .join("gold_tasks.toml");
        let gold: Vec<GoldTask> = if gold_path.exists() {
            load_gold_tasks(&gold_path)
        } else {
            Vec::new()
        };
        for task in &gold {
            let activator = ContextActivator::new(Arc::new(ReversibleContextRegistry::new()));
            let view = activator.activate(
                &graph,
                &production_signature(&task.prompt),
                OptimizationMode::Balanced,
            );
            let metrics = evaluate_view(task, &view, 0);
            let names_in = packet_file_names(&view);
            let paths_in = packet_paths(&view);
            let hit: Vec<&String> = task
                .forbidden_files
                .iter()
                .filter(|f| gold_file_hit(f, &names_in, &paths_in))
                .collect();
            forbidden_hits += hit.len();
            recalls.push(metrics.recall);
            precisions.push(metrics.precision);
            let mut files: Vec<String> = paths_in.into_iter().collect();
            files.sort();
            lines.push(format!(
                "  gold {:<24} recall={:.2} prec={:.2} tokens={:>6} forbidden={:?} files={:?}",
                task.id, metrics.recall, metrics.precision, view.active_tokens, hit, files
            ));
        }

        let tasks_path = root.join("tests/third_party").join(name).join("tasks.toml");
        let cases = std::fs::read_to_string(&tasks_path)
            .map(|raw| parse_task_toml(&raw))
            .unwrap_or_default();
        for case in &cases {
            let registry = Arc::new(ReversibleContextRegistry::new());
            let activator = ContextActivator::new(registry.clone());
            let view = activator.activate(
                &graph,
                &production_signature(&case.prompt),
                OptimizationMode::Balanced,
            );
            let outcome = oracle_outcome(case, &view, &registry, 0);
            task_total += 1;
            if outcome.success {
                task_reachable += 1;
            }
            if outcome.strict_success {
                task_strict += 1;
            }
            let needs: Vec<String> = outcome
                .needs
                .iter()
                .filter(|n| n.presence != Presence::Unfolded)
                .map(|n| format!("{}={:?}", n.need, n.presence))
                .collect();
            lines.push(format!(
                "  task {:<24} {:<4} strict={:<5} effective={:>6} {}{}",
                case.id,
                if outcome.success { "ok" } else { "FAIL" },
                outcome.strict_success,
                outcome.effective_tokens,
                needs.join(", "),
                if outcome.forbidden_hit.is_empty() {
                    String::new()
                } else {
                    format!(" FORBIDDEN={:?}", outcome.forbidden_hit)
                }
            ));
        }
    }

    let n = recalls.len().max(1) as f32;
    let mean_recall = recalls.iter().sum::<f32>() / n;
    let mean_precision = precisions.iter().sum::<f32>() / n;
    let reachable_rate = task_reachable as f32 / task_total.max(1) as f32;
    for line in &lines {
        eprintln!("{line}");
    }
    eprintln!(
        "third_party_gold: {} gold cases mean recall {mean_recall:.3} precision {mean_precision:.3} forbidden hits {forbidden_hits}; {task_total} task cases reachable {task_reachable} strict {task_strict}",
        recalls.len()
    );
    assert!(
        mean_recall >= MIN_MEAN_RECALL,
        "mean recall {mean_recall:.3} < {MIN_MEAN_RECALL}"
    );
    assert!(
        mean_precision >= MIN_MEAN_PRECISION,
        "mean precision {mean_precision:.3} < {MIN_MEAN_PRECISION}"
    );
    assert!(
        forbidden_hits <= MAX_FORBIDDEN_HITS,
        "{forbidden_hits} forbidden files shipped (max {MAX_FORBIDDEN_HITS})"
    );
    assert!(
        reachable_rate >= MIN_TASK_REACHABLE,
        "task oracle reachable {reachable_rate:.3} < {MIN_TASK_REACHABLE}"
    );
}
