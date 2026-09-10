//! Audit the artifact layer against a real checkout.
//!
//! The fixture gate in `ml_artifact_graph.rs` proves the chain works on a
//! project we wrote, which is exactly the test that cannot fail. This one runs
//! the same pipeline over somebody else's repository and prints what came out,
//! so a person can read it against the source and judge it.
//!
//! It found, on its first run: every `nn.Module` subclass typed as a Model
//! (vit-pytorch produced 388 of them), module-level training loops missed
//! entirely (nanoGPT had zero TrainLoops), `no_grad` alone promoting a
//! weight-copying constructor to an EvalLoop, a checkpoint attributing itself
//! to a function, and a Python symbol binding to a class in a `.cpp` file.
//!
//! Ignored by default because it needs a checkout to point at:
//!
//! ```text
//! git clone --depth 1 https://github.com/karpathy/nanoGPT /tmp/nanogpt
//! NM_AUDIT_PATH=/tmp/nanogpt cargo test -p neuromesh-context \
//!     --test artifact_audit -- --ignored --nocapture
//! ```
//!
//! Repositories worth keeping in rotation, because each breaks a different
//! assumption: `karpathy/nanoGPT` (module-level loop), `pytorch/examples`
//! (many small independent training scripts), `lucidrains/vit-pytorch` (a
//! library of layers with almost no training code).

use neuromesh_core::{stable_project_id, NodeId, NodeType};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

#[test]
#[ignore = "needs NM_AUDIT_PATH pointing at a checkout"]
fn audit_a_real_repository() {
    let path = std::env::var("NM_AUDIT_PATH")
        .expect("set NM_AUDIT_PATH to the repository you want audited");
    let root = Path::new(&path)
        .canonicalize()
        .expect("NM_AUDIT_PATH must exist");

    let home = std::env::temp_dir().join(format!("nm-audit-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    std::env::set_var("NEUROMESH_HOME", &home);

    let pid = stable_project_id(&root);
    let graph = NeuralProjectGraph::new(pid.clone());
    graph.set_workspace(&root);
    graph.reindex_incremental(&root, pid, Some(5000));

    let nodes = graph.get_nodes_map();
    let files: Vec<String> = nodes
        .values()
        .filter(|n| n.node_type == NodeType::File)
        .map(|n| n.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    // Notebooks count as Python here: they reach the graph as a source view,
    // so an `.ipynb` that trains something and produces no artifact node is
    // the same silence as a `.py` that does.
    let py_files: Vec<&String> = files
        .iter()
        .filter(|f| f.ends_with(".py") || f.ends_with(".ipynb"))
        .collect();
    let notebooks = py_files.iter().filter(|f| f.ends_with(".ipynb")).count();

    println!("\n=== artifact audit: {}", root.display());
    println!(
        "files {} (python {}, of which notebooks {notebooks})",
        files.len(),
        py_files.len()
    );

    // Per-notebook yield. A notebook that indexes to a File node and nothing
    // else means the source view reached the walker but not the parser — the
    // failure that looks like success.
    if notebooks > 0 {
        println!("\nnotebooks:");
        for file in py_files.iter().filter(|f| f.ends_with(".ipynb")) {
            let symbols = nodes
                .values()
                .filter(|n| {
                    n.node_type != NodeType::File
                        && n.file_path.to_string_lossy().replace('\\', "/") == **file
                })
                .count();
            let cells = nodes
                .values()
                .filter(|n| {
                    n.node_type == NodeType::NotebookCell
                        && n.file_path.to_string_lossy().replace('\\', "/") == **file
                })
                .count();
            println!("    {symbols:>4} symbols ({cells} cells)  {file}");
        }
    }

    let mut by_type: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in nodes.values().filter(|n| n.node_type.is_artifact()) {
        by_type
            .entry(format!("{:?}", node.node_type))
            .or_default()
            .push(format!(
                "{}  [{}:{}]",
                node.name,
                node.file_path.to_string_lossy().replace('\\', "/"),
                node.line_range.as_ref().map(|r| r.start).unwrap_or(0)
            ));
    }
    println!(
        "\nartifact nodes: {}",
        by_type.values().map(Vec::len).sum::<usize>()
    );
    for (node_type, mut items) in by_type {
        items.sort();
        println!("  {} ({})", node_type, items.len());
        for item in &items {
            println!("    {item}");
        }
    }

    let label = |id: &NodeId| {
        nodes
            .get(id)
            .map(|n| {
                let file = n.file_path.to_string_lossy().replace('\\', "/");
                format!("{}@{}", n.name, file.rsplit('/').next().unwrap_or(&file))
            })
            .unwrap_or_else(|| id.to_string())
    };
    let mut edges: Vec<String> = graph
        .get_edges_map()
        .into_values()
        .filter(|e| e.edge_type.is_artifact())
        .map(|e| {
            format!(
                "{} -{:?}-> {}",
                label(&e.source),
                e.edge_type,
                label(&e.target)
            )
        })
        .collect();
    edges.sort();
    println!("\nartifact edges: {}", edges.len());
    for edge in &edges {
        println!("    {edge}");
    }

    // Recall surface: files that clearly train something and yielded nothing.
    let covered: HashSet<String> = nodes
        .values()
        .filter(|n| n.node_type.is_artifact())
        .map(|n| n.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    let mut silent: Vec<&String> = py_files
        .iter()
        .filter(|f| {
            if covered.contains(**f) {
                return false;
            }
            // Through the same reader the indexer used, so a notebook is
            // searched as code rather than as JSON.
            neuromesh_index::read_source_text(&root.join(f))
                .map(|src| src.contains("import torch") || src.contains("from torch"))
                .unwrap_or(false)
        })
        .copied()
        .collect();
    silent.sort();
    println!("\ntorch files with no artifact node: {}", silent.len());
    for file in silent {
        println!("    {file}");
    }
}

/// Which files a real question actually returns, on a real checkout.
///
/// The node counts above say a feature built something. This says whether the
/// packet changed — the only thing a user sees. Run it in a worktree at the
/// previous tag and again here, then diff the two lists: an identical packet
/// means the feature is inert, however many nodes it added.
///
/// ```text
/// NM_AUDIT_PATH=/path/to/checkout NM_PROBE="how does the notebook classify crops?" \
///   cargo test -p neuromesh-context --test artifact_audit probe -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs NM_AUDIT_PATH and NM_PROBE"]
fn packet_probe() {
    use neuromesh_context::retrieval::apply_auto_extract_keywords;
    use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
    use neuromesh_core::OptimizationMode;
    use neuromesh_task::TaskSignatureExtractor;
    use std::sync::Arc;

    let path = std::env::var("NM_AUDIT_PATH").expect("set NM_AUDIT_PATH");
    let prompt = std::env::var("NM_PROBE").expect("set NM_PROBE to the question");
    let root = Path::new(&path)
        .canonicalize()
        .expect("NM_AUDIT_PATH exists");

    let home = std::env::temp_dir().join(format!("nm-probe-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    std::env::set_var("NEUROMESH_HOME", &home);

    let pid = stable_project_id(&root);
    let graph = NeuralProjectGraph::new(pid.clone());
    graph.set_workspace(&root);
    graph.reindex_incremental(&root, pid, Some(5000));

    // Off the wall clock, or the same query returns different files between
    // runs on a loaded machine and the diff means nothing. See issue #15.
    let activator = ContextActivator::new(Arc::new(ReversibleContextRegistry::new()))
        .without_physarum_sidecar();
    let mut signature = TaskSignatureExtractor::extract(&prompt);
    apply_auto_extract_keywords(&mut signature, &prompt, true);
    let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);

    let mut files: Vec<String> = view
        .active_nodes
        .iter()
        .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    files.sort();
    files.dedup();
    println!("\n=== packet probe: {prompt:?}");
    println!(
        "=== {} nodes over {} files",
        view.active_nodes.len(),
        files.len()
    );
    for file in &files {
        println!("    {file}");
    }
}
