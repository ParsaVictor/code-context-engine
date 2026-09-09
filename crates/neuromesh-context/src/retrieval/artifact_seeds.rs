//! Seed retrieval from ML artifact *kinds* rather than symbol names.
//!
//! Everything else in the seed pipeline resolves a name: the prompt mentions
//! `ProductCard`, the graph has a `ProductCard`, done. ML questions are not
//! shaped like that. "Where is the checkpoint saved?" names no symbol — the
//! answer is whatever node the overlay typed `Checkpoint`, whether the file
//! calls it `ckpt.pt`, `last.pth`, or `out_dir`.
//!
//! Measured before this existed, on a real checkout of nanoGPT:
//!
//! ```text
//! "where is the checkpoint saved and which model does it belong to"
//!     seeds: database MISS, repository MISS, datenbank MISS, req.query MISS,
//!            parseurl MISS, querystring MISS, request MISS, parse MISS
//! "how does evaluation compute the loss on the validation split"
//!     seeds: evaluation MISS, validation MISS, schema MISS, Validator MISS,
//!            loss MISS  ->  no_seed_resolved, empty packet
//! ```
//!
//! The concept expansion is web-shaped: it reads "model" as a database model,
//! "validation" as a schema validator. It is not wrong for a web codebase, and
//! nothing here changes it — this adds a second, typed path that only fires
//! when the graph actually holds artifact nodes, so a project with none behaves
//! exactly as it did.

use neuromesh_core::{NodeId, NodeType, TaskSignature};
use neuromesh_graph::NeuralProjectGraph;

/// Cap per kind, so one Model-heavy library cannot flood the packet.
const MAX_PER_KIND: usize = 4;
const MAX_TOTAL: usize = 10;

/// Prompt vocabulary to artifact kind. Order matters only for readability;
/// every matching term contributes.
const VOCABULARY: &[(&[&str], &[NodeType])] = &[
    (
        &[
            "checkpoint",
            "ckpt",
            "state_dict",
            "weights",
            "resume",
            "pretrained",
        ],
        &[NodeType::Checkpoint],
    ),
    (
        &[
            "loss",
            "map",
            "accuracy",
            "metric",
            "f1",
            "perplexity",
            "bleu",
            "auc",
            "score",
        ],
        &[NodeType::Metric, NodeType::EvalLoop],
    ),
    (
        &[
            "train",
            "training",
            "epoch",
            "optimizer",
            "gradient",
            "backward",
            "learning rate",
            "lr",
        ],
        &[NodeType::TrainLoop],
    ),
    (
        &[
            "eval",
            "evaluate",
            "evaluation",
            "validation",
            "inference",
            "val split",
            "test set",
        ],
        &[NodeType::EvalLoop],
    ),
    (
        &[
            "dataset",
            "dataloader",
            "data loader",
            "samples",
            "batch",
            "corpus",
        ],
        &[NodeType::Dataset],
    ),
    (
        &[
            "transform",
            "augment",
            "augmentation",
            "preprocess",
            "preprocessing",
            "normalize",
            "resize",
        ],
        &[NodeType::Transform],
    ),
    (
        &[
            "model",
            "network",
            "architecture",
            "backbone",
            "head",
            "layer",
            "module",
        ],
        &[NodeType::Model, NodeType::Layer],
    ),
    (
        &[
            "hyperparameter",
            "hyperparameters",
            "batch size",
            "sweep",
            "experiment",
            "run config",
        ],
        &[NodeType::Hyperparameter, NodeType::Experiment],
    ),
];

/// Seeds for the artifact kinds this prompt is asking about.
///
/// Returns nothing at all when the graph holds no artifact nodes, which is the
/// case for every non-ML project — the cost there is one cheap graph read.
pub fn resolve_artifact_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
) -> Vec<(NodeId, f32, String)> {
    let wanted = wanted_kinds(&signature.raw_prompt);
    if wanted.is_empty() {
        return Vec::new();
    }
    let available = graph.artifact_nodes();
    if available.is_empty() {
        return Vec::new();
    }

    let mut out: Vec<(NodeId, f32, String)> = Vec::new();
    for kind in &wanted {
        let mut taken = 0usize;
        let mut candidates: Vec<(NodeId, f32, String)> = available
            .iter()
            .filter(|(_, ty)| ty == kind)
            .filter_map(|(id, ty)| {
                let node = graph.get_node(id)?;
                Some((
                    id.clone(),
                    kind_score(&node.name, signature),
                    format!("artifact:{ty:?}→{}", node.name),
                ))
            })
            .collect();
        // Named in the prompt first, then whatever else that kind has.
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.2.cmp(&b.2))
        });
        for candidate in candidates {
            if taken >= MAX_PER_KIND || out.len() >= MAX_TOTAL {
                break;
            }
            if out.iter().any(|(id, _, _)| *id == candidate.0) {
                continue;
            }
            out.push(candidate);
            taken += 1;
        }
    }
    out
}

/// Which artifact kinds this prompt is about. A prompt that mentions none of
/// the vocabulary gets none, so `handle_tool_call` on a Rust service is
/// untouched.
fn wanted_kinds(prompt: &str) -> Vec<NodeType> {
    let prompt = prompt.to_lowercase();
    let mut kinds: Vec<NodeType> = Vec::new();
    for (terms, types) in VOCABULARY {
        if !terms.iter().any(|t| contains_word(&prompt, t)) {
            continue;
        }
        for ty in *types {
            if !kinds.contains(ty) {
                kinds.push(*ty);
            }
        }
    }
    kinds
}

/// Whole-word match, so `map` does not fire on `mapper` and `lr` does not fire
/// inside `clr`. Multi-word terms are matched as a phrase.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let mut from = 0usize;
    while let Some(found) = haystack[from..].find(needle) {
        let at = from + found;
        let before_ok = at == 0
            || !haystack[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let after = at + needle.len();
        let after_ok = after >= haystack.len()
            || !haystack[after..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        if before_ok && after_ok {
            return true;
        }
        from = at + needle.len();
    }
    false
}

/// A node whose name the prompt actually says outranks one it merely implies.
fn kind_score(name: &str, signature: &TaskSignature) -> f32 {
    let name_l = name.to_lowercase();
    let mut score = 0.86_f32;
    if signature
        .identifiers
        .iter()
        .any(|id| id.eq_ignore_ascii_case(name))
    {
        score += 0.10;
    } else if signature
        .raw_prompt
        .to_lowercase()
        .contains(name_l.as_str())
    {
        score += 0.05;
    }
    score.min(0.99)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ml_vocabulary_maps_to_the_kinds_that_answer_it() {
        assert_eq!(
            wanted_kinds("where is the checkpoint saved"),
            vec![NodeType::Checkpoint]
        );
        assert_eq!(
            wanted_kinds("why did the training loss stop going down"),
            vec![NodeType::Metric, NodeType::EvalLoop, NodeType::TrainLoop]
        );
    }

    #[test]
    fn a_prompt_with_no_ml_vocabulary_asks_for_nothing() {
        assert!(wanted_kinds("how does handle_request extract a route").is_empty());
        assert!(wanted_kinds("add a quantity stepper to the checkout view").is_empty());
    }

    #[test]
    fn matching_is_whole_word() {
        // `map` must not fire on `mapper`, or every web codebase becomes ML.
        assert!(wanted_kinds("rename the mapper in the serializer").is_empty());
        assert!(!wanted_kinds("why did map drop after retraining").is_empty());
    }
}
