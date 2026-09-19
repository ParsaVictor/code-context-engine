//! Batch probe for the gold harnesses: with `NM_EXPLAIN=1`, every gold task
//! whose precision is below `NM_EXPLAIN_MAX_PRECISION` (default 0.6) or that
//! shipped a forbidden file gets a block in `target/explain-<set>.txt`:
//! the prompt, the gold, every packet file with its `expansion_reason` and
//! sidecar flag, and every resolved seed with its query. This is the dump
//! F59 and F61 were found with by hand (temporary `eprintln!`s that twice
//! broke the file when removed); one run over all dev sets now gives the
//! whole picture, so root causes can be clustered and fixed per cluster.
//!
//! Extra files are usually *seeds* (`utility:8.50`, `sidecar=false`), not
//! fills — read the seed lines first.

use neuromesh_core::{ContextView, NodeType};
use neuromesh_graph::NeuralProjectGraph;
use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

fn enabled() -> bool {
    std::env::var("NM_EXPLAIN").is_ok_and(|v| v != "0")
}

fn max_precision() -> f32 {
    std::env::var("NM_EXPLAIN_MAX_PRECISION")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.6)
}

#[allow(clippy::too_many_arguments)]
pub fn record(
    set: &str,
    task_id: &str,
    prompt: &str,
    gold_files: &[String],
    precision: f32,
    forbidden_hit: &[&String],
    graph: &NeuralProjectGraph,
    view: &ContextView,
) {
    if !enabled() || (precision >= max_precision() && forbidden_hit.is_empty()) {
        return;
    }
    let mut out = String::new();
    let _ = writeln!(out, "=== {set}/{task_id}  precision={precision:.2}");
    let _ = writeln!(out, "prompt: {prompt}");
    let _ = writeln!(out, "gold:   {gold_files:?}");
    if !forbidden_hit.is_empty() {
        let _ = writeln!(out, "FORBIDDEN shipped: {forbidden_hit:?}");
    }
    let _ = writeln!(out, "-- packet files (reason / sidecar / tokens)");
    for n in &view.active_nodes {
        if n.node.node_type != NodeType::File {
            continue;
        }
        let path = n.node.file_path.to_string_lossy().replace('\\', "/");
        let in_gold = gold_files
            .iter()
            .any(|g| path == *g || path.ends_with(&format!("/{g}")));
        let _ = writeln!(
            out,
            "  {} {:<60} {:<22} sidecar={:<5} tokens={}",
            if in_gold { "✓" } else { "✗" },
            path,
            n.expansion_reason.as_deref().unwrap_or("-"),
            n.sidecar,
            n.node.token_cost
        );
    }
    let _ = writeln!(out, "-- seeds (query → node @ file)");
    for seed in &view.seeds {
        match seed.resolved_id.as_ref().and_then(|id| graph.get_node(id)) {
            Some(node) => {
                let _ = writeln!(
                    out,
                    "  {:<40} → {} ({:?}) @ {}",
                    seed.query,
                    node.name,
                    node.node_type,
                    node.file_path.to_string_lossy().replace('\\', "/")
                );
            }
            None => {
                let _ = writeln!(out, "  {:<40} → (unresolved)", seed.query);
            }
        }
    }
    let _ = writeln!(out);
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("explain-{set}.txt"));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(out.as_bytes());
    }
}
