//! P1 definition of done for `.ipynb`: a notebook indexes like source.
//!
//! The failure this gate exists to catch is not "the parser crashed". It is the
//! quieter one the artifact overlay already taught us once — a feature that
//! runs, passes its own unit tests, and changes nothing downstream. For
//! notebooks there are four distinct ways that can happen, so each is asserted
//! against a real index rather than against the normalizer in isolation:
//!
//!   1. **The notebook is skipped.** `.ipynb` used to be an unknown extension,
//!      which meant the walker never even read it. If nothing in the graph has
//!      a `.ipynb` path, everything below is vacuous.
//!   2. **The JSON is indexed instead of the code.** Then `Detector` is not a
//!      symbol, the packet is a wall of base64, and the token budget is gone.
//!   3. **Cells are read but not joined.** A `def` in cell 4 and its call in
//!      cell 8 are the whole point; if each cell were parsed alone, neither the
//!      type nor the link would survive.
//!   4. **Ordering is invented.** Every notebook in a repo has a `cell3`. If
//!      cell nodes resolved by name, two unrelated notebooks would be spliced
//!      into one chain.

use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{stable_project_id, ContextNode, EdgeType, NodeType, OptimizationMode};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::TaskSignatureExtractor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FIXTURE: &str = "ml-notebook";
const NOTEBOOK: &str = "train_detector.ipynb";

fn use_temp_neuromesh_home() {
    let home = std::env::temp_dir().join(format!("nm-nbgraph-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    std::env::set_var("NEUROMESH_HOME", &home);
}

/// Staged outside the repository: `stable_project_id` resolves to the nearest
/// enclosing git root, so a fixture left in place would carry this repo's id.
fn staged_fixture(tag: &str) -> PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(FIXTURE);
    assert!(src.is_dir(), "fixture must exist at {src:?}");

    let dst = std::env::temp_dir().join(format!("nm-nb-{}-{tag}", std::process::id()));
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

fn nodes_in(graph: &NeuralProjectGraph, file: &str) -> Vec<ContextNode> {
    let mut found: Vec<ContextNode> = graph
        .get_nodes_map()
        .into_values()
        .filter(|n| {
            n.file_path
                .to_string_lossy()
                .replace('\\', "/")
                .ends_with(file)
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

fn inventory(graph: &NeuralProjectGraph, file: &str) -> Vec<String> {
    nodes_in(graph, file)
        .iter()
        .map(|n| format!("{} ({:?})", n.name, n.node_type))
        .collect()
}

fn type_of(graph: &NeuralProjectGraph, file: &str, name: &str) -> Option<NodeType> {
    nodes_in(graph, file)
        .into_iter()
        .find(|n| n.name == name)
        .map(|n| n.node_type)
}

#[test]
fn a_notebook_indexes_as_python_not_as_json() {
    use_temp_neuromesh_home();
    let root = staged_fixture("index");
    let graph = indexed(&root);

    // ---- 1. the notebook was read at all ----
    let present = inventory(&graph, NOTEBOOK);
    assert!(
        !present.is_empty(),
        "nothing from {NOTEBOOK} reached the graph — the walker skipped it"
    );

    // ---- 2. what got indexed is the code, not the JSON ----
    let source = graph
        .read_source(Path::new(NOTEBOOK))
        .expect("the notebook must be readable through the graph");
    assert!(
        !source.contains("\"cell_type\""),
        "raw notebook JSON reached the engine"
    );
    assert!(
        !source.contains("iVBORw0KGgo"),
        "a base64 output image reached the engine"
    );
    assert!(
        source.contains("class Detector(nn.Module):"),
        "the code is missing from the source view:\n{source}"
    );
    // Magics are not Python and must not be left to break the parse.
    assert!(
        source.contains("# %matplotlib inline") && source.contains("# !pip install"),
        "IPython syntax was left as code:\n{source}"
    );
    // The markdown headings survive, because "Evaluate mAP" is exactly the
    // phrase a question about this notebook will use.
    assert!(source.contains("Evaluate mAP"), "{source}");
    // The error a cell ended on is worth one line and no more.
    assert!(
        source.contains("# %% error: RuntimeError: CUDA out of memory"),
        "{source}"
    );

    // The view is a fraction of the file it came from.
    let raw = std::fs::read_to_string(root.join(NOTEBOOK)).unwrap();
    assert!(
        source.len() * 2 < raw.len(),
        "the view ({} bytes) should be far smaller than the notebook ({} bytes)",
        source.len(),
        raw.len()
    );

    // ---- 3. cells were joined, so the overlay can see the whole module ----
    assert_eq!(
        type_of(&graph, NOTEBOOK, "Detector"),
        Some(NodeType::Model),
        "an nn.Module defined in a cell must be a Model, got {present:?}"
    );
    assert_eq!(
        type_of(&graph, NOTEBOOK, "build_transforms"),
        Some(NodeType::Transform),
        "got {present:?}"
    );
    assert_eq!(
        type_of(&graph, NOTEBOOK, "evaluate"),
        Some(NodeType::EvalLoop),
        "got {present:?}"
    );

    // The training loop lives at module scope in cell 7, under no `def` — the
    // nanoGPT shape — and it consumes names defined four cells earlier.
    let loop_node = nodes_in(&graph, NOTEBOOK)
        .into_iter()
        .find(|n| n.node_type == NodeType::TrainLoop)
        .unwrap_or_else(|| panic!("no TrainLoop in the notebook; nodes: {present:?}"));

    let reaches = |name: &str| {
        graph
            .get_connected_neighbors(&loop_node.id)
            .into_iter()
            .filter_map(|(id, _)| graph.get_node(&id))
            .any(|n| n.name == name)
    };
    assert!(
        reaches("Detector"),
        "cross-cell DEF-USE failed: the loop in cell 7 does not reach the class \
         defined in cell 4. Neighbours: {:?}",
        graph
            .get_connected_neighbors(&loop_node.id)
            .into_iter()
            .filter_map(|(id, _)| graph.get_node(&id))
            .map(|n| format!("{} ({:?})", n.name, n.node_type))
            .collect::<Vec<_>>()
    );

    // ---- 4. cells are ordered, and only within their own notebook ----
    let cells: Vec<ContextNode> = nodes_in(&graph, NOTEBOOK)
        .into_iter()
        .filter(|n| n.node_type == NodeType::NotebookCell)
        .collect();
    assert!(
        cells.len() >= 4,
        "expected a node per code cell, got {:?}",
        cells.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    for cell in &cells {
        let range = cell.line_range.clone().expect("a cell spans lines");
        assert!(
            range.end > range.start,
            "{} has an empty span {range:?}",
            cell.name
        );
    }

    let nodes = graph.get_nodes_map();
    let precedes: Vec<(ContextNode, ContextNode)> = graph
        .get_edges_map()
        .into_values()
        .filter(|e| e.edge_type == EdgeType::Precedes)
        .filter_map(|e| Some((nodes.get(&e.source)?.clone(), nodes.get(&e.target)?.clone())))
        .collect();
    assert!(
        !precedes.is_empty(),
        "cell order was recorded nowhere; cells: {:?}",
        cells.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    let render = |edge: &(ContextNode, ContextNode)| {
        format!(
            "{}:{} ({:?}) -> {}:{} ({:?})",
            file_of(&edge.0),
            edge.0.name,
            edge.0.node_type,
            file_of(&edge.1),
            edge.1.name,
            edge.1.node_type
        )
    };
    for edge in &precedes {
        let (source, target) = edge;
        // Both ends must be cells. Anchoring one end on the file node — what
        // the generic resolver does — would make "what runs before this?"
        // unanswerable while still looking like an ordered graph.
        assert_eq!(
            source.node_type,
            NodeType::NotebookCell,
            "Precedes must run cell to cell: {}",
            render(edge)
        );
        assert_eq!(
            target.node_type,
            NodeType::NotebookCell,
            "Precedes must run cell to cell: {}",
            render(edge)
        );
        assert_eq!(
            file_of(source),
            file_of(target),
            "a Precedes edge crossed notebooks: {}. Every notebook has a \
             cell3; ordering must resolve inside one file.",
            render(edge)
        );
        // …and it must run forwards.
        let number = |n: &ContextNode| {
            n.name
                .trim_start_matches("cell")
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("unexpected cell name {}", n.name))
        };
        assert!(
            number(source) < number(target),
            "cell order runs forwards: {}",
            render(edge)
        );
    }
    // Both notebooks contribute their own chain.
    assert!(
        precedes.iter().any(|(s, _)| file_of(s).ends_with(NOTEBOOK)),
        "{:?}",
        precedes.iter().map(render).collect::<Vec<_>>()
    );
    assert!(
        precedes
            .iter()
            .any(|(s, _)| file_of(s).ends_with("sanity_check.ipynb")),
        "the second notebook has cells too: {:?}",
        precedes.iter().map(render).collect::<Vec<_>>()
    );
}

fn file_of(node: &ContextNode) -> String {
    node.file_path.to_string_lossy().replace('\\', "/")
}

#[test]
fn a_question_about_the_notebook_reaches_the_notebook() {
    use_temp_neuromesh_home();
    let root = staged_fixture("packet");
    let graph = indexed(&root);

    let registry = Arc::new(ReversibleContextRegistry::new());
    // Off the wall clock: the Physarum sidecar only contributes when its solve
    // beats an SLA, which makes the packet vary between runs on a loaded CI
    // machine. See issue #15.
    let activator = ContextActivator::new(registry).without_physarum_sidecar();
    let prompt = "why did val/mAP drop after the Detector training loop changed?";
    let mut signature = TaskSignatureExtractor::extract(prompt);
    apply_auto_extract_keywords(&mut signature, prompt, true);
    let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);

    let files: Vec<String> = view
        .active_nodes
        .iter()
        .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(
        files.iter().any(|f| f.ends_with(NOTEBOOK)),
        "a question about this notebook's model and metric did not retrieve it; \
         packet was {files:?}"
    );
}
