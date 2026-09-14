//! Config→Code: a YAML key is a node, the Python that reads it is linked to it
//! by a `Parameterizes` edge, and a question naming the key ships both files.
//!
//! The fixture is built in a temp dir here rather than under `tests/fixtures`
//! so it carries no repository identity (`stable_project_id` walks up to the
//! nearest git root).

use neuromesh_context::retrieval::apply_auto_extract_keywords;
use neuromesh_context::{ContextActivator, ReversibleContextRegistry};
use neuromesh_core::{stable_project_id, EdgeType, NodeType, OptimizationMode};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::TaskSignatureExtractor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn staged() -> PathBuf {
    let home = std::env::temp_dir().join(format!("nm-cfg-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    std::env::set_var("NEUROMESH_HOME", &home);

    let root = std::env::temp_dir().join(format!("nm-cfg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("configs")).unwrap();
    std::fs::create_dir_all(root.join("trainer")).unwrap();
    std::fs::write(
        root.join("configs/train.yaml"),
        "# training defaults\nseed: 42\noptimizer:\n  lr: 0.001 # initial learning rate\n  weight_decay: 0.0\nbatch_size: 32\nwarmup_steps: 100\n",
    )
    .unwrap();
    std::fs::write(
        root.join("trainer/optim.py"),
        "from omegaconf import OmegaConf\n\ncfg = OmegaConf.load(\"configs/train.yaml\")\n\n\ndef build_optimizer(model):\n    lr = cfg.optimizer.lr\n    wd = cfg.optimizer.weight_decay\n    return make(model, lr, wd)\n\n\ndef make(model, lr, wd):\n    return (model, lr, wd)\n",
    )
    .unwrap();
    std::fs::write(
        root.join("trainer/data.py"),
        "def load_batches(cfg):\n    return cfg.batch_size\n",
    )
    .unwrap();
    std::fs::write(
        root.join("trainer/unrelated.py"),
        "def tokenize(text):\n    return text.split()\n",
    )
    .unwrap();
    root.canonicalize().unwrap()
}

fn indexed(root: &Path) -> NeuralProjectGraph {
    let pid = stable_project_id(root);
    let graph = NeuralProjectGraph::new(pid.clone());
    graph.set_workspace(root);
    graph.reindex_incremental(root, pid, Some(500));
    graph
}

fn packet_files(graph: &NeuralProjectGraph, prompt: &str) -> Vec<String> {
    let activator = ContextActivator::new(Arc::new(ReversibleContextRegistry::new()));
    let mut sig = TaskSignatureExtractor::extract(prompt);
    apply_auto_extract_keywords(&mut sig, prompt, true);
    let view = activator.activate(graph, &sig, OptimizationMode::Balanced);
    let mut files: Vec<String> = view
        .active_nodes
        .iter()
        .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
        .collect();
    files.sort();
    files.dedup();
    files
}

#[test]
fn yaml_key_links_to_its_python_reader_and_both_ship() {
    let root = staged();
    let graph = indexed(&root);
    let nodes = graph.get_nodes_map();

    let key = nodes
        .values()
        .find(|n| n.name == "lr" && n.node_type == NodeType::Config)
        .expect("`lr` from train.yaml is a Config node");
    assert!(key
        .file_path
        .to_string_lossy()
        .replace('\\', "/")
        .ends_with("configs/train.yaml"));

    let reader = nodes
        .values()
        .find(|n| n.name == "build_optimizer")
        .expect("build_optimizer");
    let linked = graph.get_edges_map().into_values().any(|e| {
        e.edge_type == EdgeType::Parameterizes && e.source == key.id && e.target == reader.id
    });
    assert!(
        linked,
        "expected lr -Parameterizes-> build_optimizer; artifact edges: {:?}",
        graph
            .get_edges_map()
            .into_values()
            .filter(|e| e.edge_type.is_artifact())
            .map(|e| format!("{} -> {}", e.source, e.target))
            .collect::<Vec<_>>()
    );
    // A key read without a named file still binds when it is unique.
    let batch = nodes
        .values()
        .find(|n| n.name == "batch_size" && n.node_type == NodeType::Config)
        .expect("batch_size");
    let load = nodes.values().find(|n| n.name == "load_batches").unwrap();
    assert!(graph.get_edges_map().into_values().any(|e| {
        e.edge_type == EdgeType::Parameterizes && e.source == batch.id && e.target == load.id
    }));

    let files = packet_files(
        &graph,
        "How is the weight_decay from train.yaml applied when build_optimizer creates the optimizer?",
    );
    assert!(
        files.iter().any(|f| f.ends_with("configs/train.yaml")),
        "{files:?}"
    );
    assert!(
        files.iter().any(|f| f.ends_with("trainer/optim.py")),
        "{files:?}"
    );
    assert!(
        !files.iter().any(|f| f.ends_with("unrelated.py")),
        "{files:?}"
    );
}
