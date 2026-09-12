//! CI gate: every task case's evidence is in its packet, at symbol level.
//!
//! File recall was 1.00 while the function a question named was missing
//! from the packet — the artifact overlay had retyped it, the skeletonizer
//! only knew `Function` spans, and the body left the packet without a fold
//! marker. This gate reads `tests/tasks/fixtures.toml` through the same
//! oracle `neuromesh eval --tasks` uses and fails on the first need that is
//! neither shipped nor foldable, or on any forbidden file.

use neuromesh_context::gold::production_signature;
use neuromesh_context::task_harness::{
    load_task_dir, oracle_outcome, summarize, Presence, TaskOutcome,
};
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{OptimizationMode, ProjectId};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Folds are allowed (an agent can expand them) but most needs should ship
/// open: below this the packets are markers, not context.
const MIN_STRICT_RATE: f32 = 0.9;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root")
}

#[test]
fn every_fixture_task_is_answerable_from_its_packet() {
    let root = workspace_root();
    let cases: Vec<_> = load_task_dir(&root.join("tests").join("tasks"))
        .into_iter()
        .filter(|c| c.repo.starts_with("tests/fixtures/"))
        .collect();
    assert!(
        !cases.is_empty(),
        "no fixture task cases under tests/tasks/"
    );

    let mut graphs: BTreeMap<String, Arc<NeuralProjectGraph>> = BTreeMap::new();
    let mut outcomes: Vec<TaskOutcome> = Vec::new();
    for case in &cases {
        let repo = root.join(&case.repo);
        assert!(
            repo.is_dir(),
            "{}: missing repository {}",
            case.id,
            case.repo
        );
        let graph = graphs
            .entry(case.repo.clone())
            .or_insert_with(|| {
                let pid = ProjectId::new(&case.repo);
                let graph = NeuralProjectGraph::new(pid.clone());
                graph.set_workspace(&repo);
                let scanned = ProjectWalker::new(repo.clone(), pid)
                    .scan()
                    .expect("scan fixture");
                graph.ingest_workspace(&scanned);
                Arc::new(graph)
            })
            .clone();
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry.clone());
        let signature = production_signature(&case.prompt);
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        outcomes.push(oracle_outcome(case, &view, &registry, 0));
    }

    let failed: Vec<String> = outcomes
        .iter()
        .filter(|o| !o.success)
        .map(|o| {
            let needs: Vec<String> = o
                .needs
                .iter()
                .filter(|n| !matches!(n.presence, Presence::Unfolded | Presence::Folded))
                .map(|n| format!("{}={:?}", n.need, n.presence))
                .collect();
            format!(
                "{}: {} forbidden={:?}",
                o.id,
                needs.join(", "),
                o.forbidden_hit
            )
        })
        .collect();
    assert!(
        failed.is_empty(),
        "{} of {} task cases cannot be finished from their packet:\n  {}",
        failed.len(),
        outcomes.len(),
        failed.join("\n  ")
    );

    let summary = summarize(&outcomes);
    assert!(
        summary.strict_success_rate >= MIN_STRICT_RATE,
        "strict success {:.3} below {MIN_STRICT_RATE}: too many needs shipped folded",
        summary.strict_success_rate
    );
    eprintln!(
        "task_success: {}/{} reachable, strict {:.3}, mean effective tokens {:.0}, success per 1k tokens {:.3}",
        summary.succeeded,
        summary.cases,
        summary.strict_success_rate,
        summary.mean_effective_tokens,
        summary.success_per_1k_tokens
    );
}
