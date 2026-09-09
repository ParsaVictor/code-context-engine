//! P1 definition of done: "why did mAP drop?" has a chain to walk.
//!
//! A real PyTorch detection project is indexed end to end, and the graph is
//! asked the two things that matter:
//!
//!   1. Are the artifacts *typed* — is `Detector` a Model rather than a Class,
//!      is `train_one_epoch` a TrainLoop rather than a Function, is `val/mAP` a
//!      Metric at all?
//!   2. Are they *connected* — can a walk that starts at the metric reach the
//!      dataset that produced the numbers, through the model and the checkpoint
//!      in between?
//!
//! Without (2) the node types are decoration. The walk is what turns the
//! question into ~4 files instead of the whole repository.

use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{stable_project_id, ContextNode, NodeId, NodeType, OptimizationMode};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::TaskSignatureExtractor;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FIXTURE: &str = "ml-pytorch-detection";

fn use_temp_neuromesh_home() {
    let home = std::env::temp_dir().join(format!("nm-ml-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    std::env::set_var("NEUROMESH_HOME", &home);
}

/// Staged outside the repository for the same reason the isolation gate does
/// it: `stable_project_id` resolves to the nearest enclosing git repository, so
/// a fixture left in place would carry this repo's identity.
fn staged_fixture(name: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(name);
    assert!(src.is_dir(), "fixture {name} must exist at {src:?}");

    let dst = std::env::temp_dir().join(format!("nm-ml-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_tree(&src, &dst);
    dst.canonicalize().expect("staged fixture path")
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create staged dir");
    for entry in std::fs::read_dir(src).expect("read fixture dir") {
        let entry = entry.expect("fixture entry");
        let to = dst.join(entry.file_name());
        if entry.file_type().expect("entry type").is_dir() {
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).expect("copy fixture file");
        }
    }
}

fn indexed(root: &Path) -> NeuralProjectGraph {
    let pid = stable_project_id(root);
    let graph = NeuralProjectGraph::new(pid.clone());
    graph.set_workspace(root);
    graph.reindex_incremental(root, pid, Some(500));
    graph
}

fn node_named(graph: &NeuralProjectGraph, name: &str) -> Option<ContextNode> {
    graph.get_nodes_map().into_values().find(|n| n.name == name)
}

fn type_of(graph: &NeuralProjectGraph, name: &str) -> Option<NodeType> {
    node_named(graph, name).map(|n| n.node_type)
}

/// Symbol nodes are file-scoped, so a name that appears in two files — `main`,
/// or a checkpoint path that one module writes and another reads — is two
/// nodes. Every lookup here names the file it means, or it would depend on
/// hash-map order.
fn id_in(graph: &NeuralProjectGraph, name: &str, file: &str) -> NodeId {
    graph
        .get_nodes_map()
        .into_values()
        .find(|n| {
            n.name == name
                && n.file_path
                    .to_string_lossy()
                    .replace('\\', "/")
                    .ends_with(file)
        })
        .unwrap_or_else(|| {
            panic!(
                "no node named {name} in {file}; artifact nodes present: {:?}",
                artifact_inventory(graph)
            )
        })
        .id
}

fn artifact_inventory(graph: &NeuralProjectGraph) -> Vec<String> {
    let mut found: Vec<String> = graph
        .get_nodes_map()
        .into_values()
        .filter(|n| n.node_type.is_artifact())
        .map(|n| format!("{} ({:?})", n.name, n.node_type))
        .collect();
    found.sort();
    found
}

/// Hop count between two nodes over artifact edges only — the language-level
/// edges are excluded on purpose, so a path here means the artifact layer
/// carries it and not `Contains` through a shared file.
fn artifact_hops(graph: &NeuralProjectGraph, from: &NodeId, to: &NodeId) -> Option<usize> {
    let mut seen: HashSet<NodeId> = HashSet::from([from.clone()]);
    let mut queue = VecDeque::from([(from.clone(), 0usize)]);
    while let Some((node, depth)) = queue.pop_front() {
        if node == *to {
            return Some(depth);
        }
        if depth >= 8 {
            continue;
        }
        for (neighbor, edge) in graph.get_connected_neighbors(&node) {
            if !edge.edge_type.is_artifact() || !seen.insert(neighbor.clone()) {
                continue;
            }
            queue.push_back((neighbor, depth + 1));
        }
    }
    None
}

/// Every artifact edge, rendered by name — the thing you actually want printed
/// when a connectivity assertion fails.
fn artifact_edges(graph: &NeuralProjectGraph) -> Vec<String> {
    let nodes = graph.get_nodes_map();
    let mut edges: Vec<String> = graph
        .get_edges_map()
        .into_values()
        .filter(|e| e.edge_type.is_artifact())
        .map(|e| {
            let name = |id: &NodeId| {
                nodes
                    .get(id)
                    .map(|n| n.name.clone())
                    .unwrap_or_else(|| id.to_string())
            };
            format!(
                "{} -{:?}-> {}",
                name(&e.source),
                e.edge_type,
                name(&e.target)
            )
        })
        .collect();
    edges.sort();
    edges
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

#[test]
fn a_pytorch_project_indexes_into_a_walkable_artifact_graph() {
    use_temp_neuromesh_home();
    let root = staged_fixture(FIXTURE);
    let graph = indexed(&root);

    // ---- 1. the artifacts are typed as artifacts ----
    let inventory = artifact_inventory(&graph);
    assert!(
        !inventory.is_empty(),
        "the PyTorch overlay produced no artifact nodes at all"
    );

    assert_eq!(
        type_of(&graph, "Detector"),
        Some(NodeType::Model),
        "an nn.Module subclass must be a Model, not a Class (got {inventory:?})"
    );
    assert_eq!(
        type_of(&graph, "CocoDetection"),
        Some(NodeType::Dataset),
        "a Dataset subclass must be a Dataset (got {inventory:?})"
    );
    assert_eq!(
        type_of(&graph, "train_one_epoch"),
        Some(NodeType::TrainLoop),
        "a def that calls .backward() is a TrainLoop (got {inventory:?})"
    );
    assert_eq!(
        type_of(&graph, "evaluate"),
        Some(NodeType::EvalLoop),
        "a def under no_grad is an EvalLoop (got {inventory:?})"
    );
    assert_eq!(
        type_of(&graph, "build_transforms"),
        Some(NodeType::Transform),
        "a def that builds a Compose is a Transform (got {inventory:?})"
    );
    assert_eq!(
        type_of(&graph, "runs/last.pt"),
        Some(NodeType::Checkpoint),
        "torch.save's path literal names the Checkpoint (got {inventory:?})"
    );
    assert_eq!(
        type_of(&graph, "val/mAP"),
        Some(NodeType::Metric),
        "the logged key is the Metric (got {inventory:?})"
    );
    assert_eq!(
        type_of(&graph, "Detector.backbone"),
        Some(NodeType::Layer),
        "a model's nn submodule attribute is a Layer (got {inventory:?})"
    );

    // ---- 2. the chain the demo walks actually exists ----
    let metric = id_in(&graph, "val/mAP", "evaluate.py");
    let eval_loop = id_in(&graph, "evaluate", "evaluate.py");
    let model = id_in(&graph, "Detector", "detector.py");
    let checkpoint = id_in(&graph, "runs/last.pt", "train.py");
    let train_loop = id_in(&graph, "train_one_epoch", "train.py");
    let transform = id_in(&graph, "build_transforms", "dataset.py");
    let dataset = id_in(&graph, "CocoDetection", "dataset.py");
    let edges = artifact_edges(&graph);

    for (from, to, label) in [
        (&metric, &eval_loop, "metric -> eval loop"),
        (&eval_loop, &model, "eval loop -> model"),
        (&train_loop, &model, "train loop -> model"),
        (&model, &checkpoint, "model -> checkpoint"),
        (&transform, &dataset, "transform -> dataset"),
    ] {
        assert!(
            artifact_hops(&graph, from, to).is_some(),
            "{label} is not connected by artifact edges\n  nodes {inventory:?}\n  edges {edges:?}"
        );
    }

    // The whole question in one walk: from the number that moved to the data
    // that produced it, without leaving the artifact layer.
    assert!(
        artifact_hops(&graph, &metric, &dataset).is_some(),
        "the metric cannot reach the dataset over artifact edges — the \
         'why did mAP drop?' walk has no path\n  nodes {inventory:?}\n  edges {edges:?}"
    );

    // ---- 3. and the question returns the files a person would open ----
    let files = packet_files(&graph, "why did the model's mAP drop during evaluation");
    assert!(
        files.iter().any(|f| f.ends_with("evaluate.py")),
        "the eval loop's file is missing from the packet: {files:?}"
    );
    assert!(
        files.iter().any(|f| f.ends_with("detector.py")),
        "the model's file is missing from the packet: {files:?}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
