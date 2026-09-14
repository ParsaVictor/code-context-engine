//! Shared runner for the pinned-checkout gold sets that live beside the
//! four original dev repos: `tests/third_party/<set>/` holds a `repos.toml`
//! plus one directory per repo with `gold_tasks.toml` (file-level gold) and
//! `tasks.toml` (symbol-level oracle cases). Checkouts are under
//! `target/third_party/<set>/<name>` (or `$NM_<SET>_DIR`), fetched with
//! `bash scripts/fetch-third-party.sh tests/third_party/<set>/repos.toml <set>`.
//!
//! Each set keeps its own test file and its own ratchet constants; this
//! module only does the measuring, so two sets can never share a threshold
//! and a regression in one cannot hide behind an improvement in the other.
//! `third_party_gold.rs` (the four small dev repos) predates this module and
//! is deliberately left untouched — its ratchet is the project's oldest
//! promise and rewriting it buys nothing.

#![allow(dead_code)]

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

pub struct GoldSetSummary {
    pub gold_cases: usize,
    pub mean_recall: f32,
    pub mean_precision: f32,
    pub forbidden_hits: usize,
    pub task_total: usize,
    pub task_reachable: usize,
    pub task_strict: usize,
}

impl GoldSetSummary {
    pub fn reachable_rate(&self) -> f32 {
        self.task_reachable as f32 / self.task_total.max(1) as f32
    }
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root")
}

fn checkout_dir(root: &Path, set: &str) -> PathBuf {
    let var = format!("NM_{}_DIR", set.to_uppercase().replace('-', "_"));
    std::env::var(var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("target").join("third_party").join(set))
}

/// Where the set's `repos.toml` and per-repo gold live. Defaults to
/// `tests/third_party/<set>`; `NM_<SET>_SET_DIR` points it outside the
/// repository for a private holdout whose gold must never be committed here.
fn set_dir(root: &Path, set: &str) -> PathBuf {
    let var = format!("NM_{}_SET_DIR", set.to_uppercase().replace('-', "_"));
    std::env::var(var)
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("tests/third_party").join(set))
}

fn repo_names(root: &Path, set: &str) -> Vec<String> {
    let manifest = set_dir(root, set).join("repos.toml");
    let raw = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
    raw.lines()
        .filter_map(|l| l.trim().strip_prefix("name"))
        .filter_map(|rest| rest.split_once('='))
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
        .collect()
}

/// Index every repo in the set, score its gold and oracle cases through the
/// production signature, print one line per case, and return the totals.
/// Panics (with the fetch command) when a checkout is missing — a silently
/// skipped repo is the failure mode this exists to remove.
pub fn run_gold_set(set: &str) -> GoldSetSummary {
    let root = workspace_root();
    let checkouts = checkout_dir(&root, set);
    let names = repo_names(&root, set);
    assert!(
        !names.is_empty(),
        "no repositories in {}/repos.toml", set_dir(&root, set).display()
    );

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
            "{name}: checkout missing at {} — run: bash scripts/fetch-third-party.sh tests/third_party/{set}/repos.toml {set}",
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

        let set_dir = set_dir(&root, set).join(name);
        let gold_path = set_dir.join("gold_tasks.toml");
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
                "  gold {:<26} recall={:.2} prec={:.2} tokens={:>6} forbidden={:?} files={:?}",
                task.id, metrics.recall, metrics.precision, view.active_tokens, hit, files
            ));
        }

        let cases = std::fs::read_to_string(set_dir.join("tasks.toml"))
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
                "  task {:<26} {:<4} strict={:<5} effective={:>6} {}{}",
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
    let summary = GoldSetSummary {
        gold_cases: recalls.len(),
        mean_recall: recalls.iter().sum::<f32>() / n,
        mean_precision: precisions.iter().sum::<f32>() / n,
        forbidden_hits,
        task_total,
        task_reachable,
        task_strict,
    };
    for line in &lines {
        eprintln!("{line}");
    }
    eprintln!(
        "third_party_{set}: {} gold cases mean recall {:.3} precision {:.3} forbidden hits {}; {} task cases reachable {} strict {}",
        summary.gold_cases,
        summary.mean_recall,
        summary.mean_precision,
        summary.forbidden_hits,
        summary.task_total,
        summary.task_reachable,
        summary.task_strict
    );
    summary
}
