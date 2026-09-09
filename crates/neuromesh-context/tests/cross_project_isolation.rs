//! P0 definition of done.
//!
//! One runtime serves project A, then project B. Nothing from A may appear in
//! B's packet. The two fixtures deliberately share `scripts/util.py`
//! byte-for-byte, because that is the case the baseline got wrong:
//! `ingest_file_keep` returns early when a path's stored hash equals the
//! incoming hash, so without a clean swap A's nodes for that path survive into
//! B's graph, still carrying A's project id.
//!
//! Runs as a single test on purpose — the scenarios share fixture slots under
//! one `NEUROMESH_HOME`, and splitting them would race on `save_persisted`.

use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{stable_project_id, NodeType, OptimizationMode};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::TaskSignatureExtractor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Keep `save_persisted` out of the developer's real `~/.neuromesh`.
fn use_temp_neuromesh_home() {
    let home = std::env::temp_dir().join(format!("nm-iso-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    std::env::set_var("NEUROMESH_HOME", &home);
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(name)
        .canonicalize()
        .unwrap_or_else(|err| panic!("fixture {name} must exist: {err}"))
}

/// Exactly what `adopt_workspace_from_initialize` does on a workspace change.
fn swap_to(graph: &NeuralProjectGraph, root: &Path) {
    let pid = stable_project_id(root);
    graph.clear(Some(pid.clone()));
    graph.set_workspace(root);
    graph.reindex_incremental(root, pid, Some(500));
}

fn packet_files(graph: &NeuralProjectGraph, prompt: &str) -> Vec<String> {
    let registry = Arc::new(ReversibleContextRegistry::new());
    let activator = ContextActivator::new(registry);
    let mut sig = TaskSignatureExtractor::extract(prompt);
    apply_auto_extract_keywords(&mut sig, prompt, true);
    let view = activator.activate(graph, &sig, OptimizationMode::Balanced);
    view.active_nodes
        .iter()
        .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn indexed_files(graph: &NeuralProjectGraph) -> Vec<String> {
    let mut files: Vec<String> = graph
        .get_nodes_map()
        .into_values()
        .filter(|node| node.node_type == NodeType::File)
        .map(|node| node.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    files.sort();
    files
}

fn file_node_project_id(graph: &NeuralProjectGraph, rel: &str) -> Option<String> {
    graph
        .get_nodes_map()
        .into_values()
        .find(|node| {
            node.node_type == NodeType::File
                && node.file_path.to_string_lossy().replace('\\', "/") == rel
        })
        .map(|node| node.project_id.to_string())
}

#[test]
fn switching_projects_leaks_nothing_into_the_next_packet() {
    use_temp_neuromesh_home();

    let web = fixture("iso-web-rust");
    let ml = fixture("iso-ml-python");
    assert_ne!(
        stable_project_id(&web),
        stable_project_id(&ml),
        "fixtures must be distinct projects"
    );

    let web_pid = stable_project_id(&web).to_string();
    let ml_pid = stable_project_id(&ml).to_string();
    let graph = NeuralProjectGraph::new(stable_project_id(&web));

    // ---- project A: the Rust web service ----
    swap_to(&graph, &web);
    assert!(
        file_node_project_id(&graph, "src/router.rs").is_some(),
        "project A must actually index; indexed files: {:?}",
        indexed_files(&graph)
    );
    assert_eq!(graph.assert_single_project(), Ok(()));

    // ---- the switch, then project B: the PyTorch detection project ----
    swap_to(&graph, &ml);

    // The invariant is the strongest statement: no node anywhere in the graph
    // belongs to another project.
    assert_eq!(
        graph.assert_single_project(),
        Ok(()),
        "project A nodes survived the swap"
    );
    let after_swap = indexed_files(&graph);
    assert!(
        after_swap.iter().all(|f| !f.ends_with(".rs")),
        "project A files are still in the graph after the swap: {after_swap:?}"
    );

    let ml_files = packet_files(&graph, "why did the model mAP drop during evaluation");
    let leaked: Vec<&String> = ml_files.iter().filter(|f| f.ends_with(".rs")).collect();
    assert!(
        leaked.is_empty(),
        "cross-project leak into project B's packet: {leaked:?} (full packet {ml_files:?})"
    );
    assert!(
        !ml_files.is_empty(),
        "project B packet must not be empty, or the check above proves nothing"
    );

    // ---- the shared byte-identical file must now belong to project B ----
    assert_eq!(
        file_node_project_id(&graph, "scripts/util.py").as_deref(),
        Some(ml_pid.as_str()),
        "the file both projects share must be owned by the current project"
    );

    // ---- swapping back is symmetric ----
    swap_to(&graph, &web);
    assert_eq!(graph.assert_single_project(), Ok(()));
    assert_eq!(
        file_node_project_id(&graph, "scripts/util.py").as_deref(),
        Some(web_pid.as_str()),
        "swapping back must re-own the shared file"
    );

    // ---- re-indexing the same project is a no-op for the invariant ----
    swap_to(&graph, &web);
    assert_eq!(graph.assert_single_project(), Ok(()));
}
