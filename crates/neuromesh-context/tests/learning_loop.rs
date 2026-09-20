//! F57 follow-up: does feedback change the *next* packet for the same
//! question? The MCP `neuromesh_record_feedback` path (reinforce node
//! access, neural spike, STDP + pheromone on the touched path) is replayed
//! here on the gold sets, offline. Two directions, each measured on its own
//! fresh graph per task so nothing leaks between questions:
//!
//! - **positive**: the gold files are marked useful (`task_success=true`);
//!   the second packet should keep recall and not lose precision.
//! - **negative**: the packet's non-gold files are marked not useful
//!   (`task_success=false`); the second packet should shed them — this is
//!   the precision the loop is supposed to buy back.
//!
//! Reports mean recall / precision before → after per direction. It is a
//! measurement, not a ratchet: the numbers go to `docs/measured.md`.
//!
//! ```text
//! NM_THIRD_PARTY=1 cargo test -p neuromesh-context --test learning_loop -- --nocapture
//! NM_LEARNING_SETS=dev,large   # default: dev
//! ```

#[path = "support/index_cache.rs"]
mod index_cache;

use neuromesh_context::gold::{
    evaluate_view, load_gold_tasks, packet_paths, production_signature, GoldTask,
};
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{NodeId, OptimizationMode};
use neuromesh_graph::NeuralProjectGraph;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root")
}

fn repo_names(manifest: &Path) -> Vec<String> {
    std::fs::read_to_string(manifest)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("name"))
        .filter_map(|rest| rest.split_once('='))
        .map(|(_, v)| v.trim().trim_matches('"').to_string())
        .collect()
}

/// (manifest, checkout dir, gold dir, cache set name) per set.
fn set_layout(root: &Path, set: &str) -> (PathBuf, PathBuf, PathBuf) {
    match set {
        "dev" => (
            root.join("tests/third_party/repos.toml"),
            root.join("target/third_party"),
            root.join("tests/third_party"),
        ),
        other => (
            root.join("tests/third_party")
                .join(other)
                .join("repos.toml"),
            root.join("target/third_party").join(other),
            root.join("tests/third_party").join(other),
        ),
    }
}

fn packet(graph: &NeuralProjectGraph, task: &GoldTask) -> (f32, f32, Vec<String>) {
    let activator = ContextActivator::new(Arc::new(ReversibleContextRegistry::new()));
    let view = activator.activate(
        graph,
        &production_signature(&task.prompt),
        OptimizationMode::Balanced,
    );
    let m = evaluate_view(task, &view, 0);
    let mut files: Vec<String> = packet_paths(&view).into_iter().collect();
    files.sort();
    (m.recall, m.precision, files)
}

/// What `neuromesh_record_feedback` does for a list of touched files.
fn feedback(graph: &NeuralProjectGraph, files: &[String], success: bool) {
    let mut path: Vec<NodeId> = Vec::new();
    for f in files {
        let Some(id) = graph.file_id_for_path(Path::new(f)) else {
            continue;
        };
        graph.reinforce_node_access(&id, success);
        graph.record_neural_spike(id.clone(), true, success);
        path.push(id);
    }
    graph.apply_stdp_on_path(&path);
    graph.reinforce_path(&path, success);
    graph.reinforce_callee_edges(&path, success);
}

fn is_gold(task: &GoldTask, file: &str) -> bool {
    task.gold_files
        .iter()
        .any(|g| file == g || file.ends_with(&format!("/{g}")))
}

#[derive(Default)]
struct Sum {
    n: usize,
    recall_before: f32,
    recall_after: f32,
    prec_before: f32,
    prec_after: f32,
    changed: usize,
}

impl Sum {
    fn add(&mut self, before: (f32, f32, Vec<String>), after: (f32, f32, Vec<String>)) {
        self.n += 1;
        self.recall_before += before.0;
        self.recall_after += after.0;
        self.prec_before += before.1;
        self.prec_after += after.1;
        if before.2 != after.2 {
            self.changed += 1;
        }
    }
    fn line(&self, label: &str) -> String {
        let n = self.n.max(1) as f32;
        format!(
            "{label:<9} n={:<3} recall {:.3} -> {:.3}   precision {:.3} -> {:.3}   packets changed {}/{}",
            self.n,
            self.recall_before / n,
            self.recall_after / n,
            self.prec_before / n,
            self.prec_after / n,
            self.changed,
            self.n
        )
    }
}

#[test]
fn feedback_changes_the_next_packet() {
    if std::env::var("NM_THIRD_PARTY").is_err() {
        eprintln!("learning_loop: NM_THIRD_PARTY not set; skipping");
        return;
    }
    let root = workspace_root();
    let sets = std::env::var("NM_LEARNING_SETS").unwrap_or_else(|_| "dev".into());
    for set in sets.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (manifest, checkouts, gold_root) = set_layout(&root, set);
        let mut positive = Sum::default();
        let mut negative = Sum::default();
        let mut lines = Vec::new();
        for name in repo_names(&manifest) {
            let repo = checkouts.join(&name);
            assert!(
                repo.is_dir(),
                "{name}: checkout missing at {}",
                repo.display()
            );
            let gold_path = gold_root.join(&name).join("gold_tasks.toml");
            if !gold_path.exists() {
                continue;
            }
            for task in load_gold_tasks(&gold_path) {
                // positive: gold files were useful
                let (graph, _, _) = index_cache::graph_for_checkout(&root, set, &name, &repo);
                let before = packet(&graph, &task);
                feedback(&graph, &task.gold_files, true);
                let after = packet(&graph, &task);
                let added: Vec<&String> =
                    after.2.iter().filter(|f| !before.2.contains(f)).collect();
                lines.push(format!(
                    "  + {:<26} recall {:.2}->{:.2} prec {:.2}->{:.2}{}",
                    task.id,
                    before.0,
                    after.0,
                    before.1,
                    after.1,
                    if added.is_empty() {
                        String::new()
                    } else {
                        format!("  added {added:?}")
                    }
                ));
                positive.add(before.clone(), after);

                // negative: the packet's non-gold files were not useful
                let extras: Vec<String> = before
                    .2
                    .iter()
                    .filter(|f| !is_gold(&task, f))
                    .cloned()
                    .collect();
                if extras.is_empty() {
                    continue;
                }
                let (graph, _, _) = index_cache::graph_for_checkout(&root, set, &name, &repo);
                feedback(&graph, &extras, false);
                let after = packet(&graph, &task);
                lines.push(format!(
                    "  - {:<26} recall {:.2}->{:.2} prec {:.2}->{:.2} (marked {} extra)",
                    task.id,
                    before.0,
                    after.0,
                    before.1,
                    after.1,
                    extras.len()
                ));
                negative.add(before, after);
            }
        }
        for l in &lines {
            eprintln!("{l}");
        }
        eprintln!("learning_loop[{set}] {}", positive.line("positive"));
        eprintln!("learning_loop[{set}] {}", negative.line("negative"));
    }
}
