//! On-disk graph cache for the third-party gold harnesses.
//!
//! Re-indexing django (3.5k files) from scratch on every measurement made a
//! `large` run ~10 minutes; the activations themselves are a fraction of
//! that. The graph already serializes (`save_to` / `load_from`, the same
//! snapshot the CLI persists), so a checkout is walked once and reused.
//!
//! Correctness: the cache key is the checkout's git revision plus a hash of
//! every source file of the crates that build the graph (`neuromesh-core`,
//! `-parser`, `-index`, `-graph`). Any change to how a file is parsed or
//! linked produces a different key and a fresh walk; a change confined to
//! `neuromesh-context` (seeds, folds, selection) reuses the graph, which is
//! exactly the part that runs unchanged either way. `read_source` falls
//! back to the checkout on disk, so folding sees the same bytes as a fresh
//! index. `NM_INDEX_CACHE=0` disables it; the cache lives under
//! `target/third_party_index_cache/`.

use neuromesh_core::ProjectId;
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_index::ProjectWalker;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const GRAPH_CRATES: &[&str] = &[
    "neuromesh-core",
    "neuromesh-parser",
    "neuromesh-index",
    "neuromesh-graph",
];

/// Hash of the graph-building crates' sources, computed once per process.
fn code_hash(workspace_root: &Path) -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        let mut files: Vec<PathBuf> = Vec::new();
        for krate in GRAPH_CRATES {
            collect_sources(&workspace_root.join("crates").join(krate), &mut files);
        }
        files.sort();
        let mut hasher = blake3::Hasher::new();
        for f in &files {
            hasher.update(f.to_string_lossy().replace('\\', "/").as_bytes());
            if let Ok(bytes) = std::fs::read(f) {
                hasher.update(&bytes);
            }
        }
        hasher.finalize().to_hex()[..16].to_string()
    })
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_sources(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e == "rs" || e == "scm" || e == "toml")
        {
            out.push(path);
        }
    }
}

fn checkout_rev(repo: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let rev = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!rev.is_empty()).then_some(rev)
}

/// Build (or load) the graph for `repo`. Returns the graph and how many
/// files it holds, plus `true` when it came from the cache.
pub fn graph_for_checkout(
    workspace_root: &Path,
    set: &str,
    name: &str,
    repo: &Path,
) -> (NeuralProjectGraph, usize, bool) {
    let pid = ProjectId::new(name);
    let graph = NeuralProjectGraph::new(pid.clone());
    graph.set_workspace(repo);
    let enabled = std::env::var("NM_INDEX_CACHE").map_or(true, |v| v != "0");
    let cache_path = checkout_rev(repo).filter(|_| enabled).map(|rev| {
        workspace_root
            .join("target")
            .join("third_party_index_cache")
            .join(set)
            .join(format!("{name}-{rev}-{}.bin", code_hash(workspace_root)))
    });
    if let Some(path) = &cache_path {
        if graph.load_from(path).unwrap_or(false) {
            graph.set_workspace(repo);
            let files = graph.file_node_paths().len();
            return (graph, files, true);
        }
    }
    let scanned = ProjectWalker::new(repo.to_path_buf(), pid)
        .scan()
        .unwrap_or_else(|e| panic!("{name}: scan failed: {e}"));
    graph.ingest_workspace(&scanned);
    if let Some(path) = &cache_path {
        if let Err(e) = graph.save_to(path) {
            eprintln!("index cache: could not write {}: {e}", path.display());
        }
    }
    (graph, scanned.len(), false)
}
