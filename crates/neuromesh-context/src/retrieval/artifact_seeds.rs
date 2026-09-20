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
use std::collections::HashSet;

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
///
/// `already_seeded` is what the name-based engine resolved before this ran.
/// When one of those nodes already *is* the kind the prompt asks for — the
/// question said "training loop" and `training` resolved to the `train`
/// TrainLoop — that kind is answered, and the other TrainLoops in the
/// project (`bench`) are not seeded on the kind alone; only a node the prompt
/// names outright (`GPT.forward` for a Layer, `bench` for a TrainLoop) still
/// joins. Without a name-resolved node of the kind, every node of the kind is
/// a candidate as before (F26).
pub fn resolve_artifact_seeds(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    already_seeded: &[NodeId],
) -> Vec<(NodeId, f32, String)> {
    let wanted = wanted_kinds(&signature.raw_prompt);
    if wanted.is_empty() {
        return Vec::new();
    }
    let available = graph.artifact_nodes();
    if available.is_empty() {
        return Vec::new();
    }
    let covered: HashSet<NodeType> = available
        .iter()
        .filter(|(id, _)| already_seeded.contains(id))
        .map(|(_, ty)| *ty)
        .collect();
    // Names of the nodes the question resolved by name (`GPT`): a candidate
    // whose parent is one of them is the member the question meant.
    let seeded_names: HashSet<String> = already_seeded
        .iter()
        .filter_map(|id| graph.get_node(id))
        .map(|n| n.name.to_lowercase())
        .collect();
    // The prompt spelled out a code identifier (`Mosaic`, `BasePredictor.stream_inference`)
    // and it resolved: the question is about that symbol, so a kind the prompt
    // merely implies ("training sample", "inference") seeds only nodes it names.
    let code_anchor = already_seeded
        .iter()
        .filter_map(|id| graph.get_node(id))
        .any(|n| {
            signature.identifiers.iter().any(|id| {
                looks_like_code(id)
                    && (id == &n.name
                        || id
                            .rsplit(['.', ':'])
                            .next()
                            .is_some_and(|last| last == n.name))
            })
        });

    let mut out: Vec<(NodeId, f32, String)> = Vec::new();
    for kind in &wanted {
        let mut taken = 0usize;
        let mut candidates: Vec<(NodeId, f32, String, bool)> = available
            .iter()
            .filter(|(_, ty)| ty == kind)
            .filter_map(|(id, ty)| {
                let node = graph.get_node(id)?;
                // An ML artifact in a test fixture or example is not what a
                // question about the product's own code means ("positive
                // feedback" → `train.py` in tests/fixtures/ml-*; F75-E).
                if crate::selector::is_noise_path_in(&node.file_path, graph.examples_are_core())
                    && !signature.raw_prompt.to_lowercase().contains("fixture")
                    && !signature.raw_prompt.to_lowercase().contains("example")
                {
                    return None;
                }
                let named = prompt_names_node(&node.name, node.parent.as_deref(), signature);
                if covered.contains(kind) && !named {
                    return None;
                }
                let owned_by_seed = node
                    .parent
                    .as_deref()
                    .is_some_and(|p| seeded_names.contains(&p.to_lowercase()));
                // The question's `CTCLayer` already resolved to one file; the
                // same-named class in a sibling example is a homonym, not a second seed.
                if !already_seeded.contains(id) && seeded_names.contains(&node.name.to_lowercase())
                {
                    return None;
                }
                if code_anchor && !named && !owned_by_seed {
                    return None;
                }
                Some((
                    id.clone(),
                    kind_score(&node.name, signature),
                    format!("artifact:{ty:?}→{}", node.name),
                    owned_by_seed,
                ))
            })
            .collect();
        // `GPT.forward` when `GPT` is a seed, not every module's `forward`.
        if candidates.iter().any(|c| c.3) {
            candidates.retain(|c| c.3);
        }
        let mut candidates: Vec<(NodeId, f32, String)> = candidates
            .into_iter()
            .map(|(id, score, reason, _)| (id, score, reason))
            .collect();
        // Named in the prompt first, then whatever else that kind has.
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| a.0.as_str().cmp(b.0.as_str()))
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

/// `Mosaic`, `stream_inference`, `Owner.member` — not the English word "training".
fn looks_like_code(id: &str) -> bool {
    id.chars()
        .any(|c| c.is_ascii_uppercase() || c == '_' || c == '.' || c == ':')
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

/// Whether the prompt names this node outright. A method counts only under
/// its owner (`GPT.forward`): the bare `forward` names every Layer in a
/// PyTorch file and therefore none of them.
fn prompt_names_node(name: &str, parent: Option<&str>, signature: &TaskSignature) -> bool {
    match parent {
        Some(owner) => {
            let qualified = format!("{owner}.{name}").to_lowercase();
            signature
                .identifiers
                .iter()
                .any(|id| id.to_lowercase() == qualified)
                || signature.raw_prompt.to_lowercase().contains(&qualified)
        }
        None => {
            signature
                .identifiers
                .iter()
                .any(|id| id.eq_ignore_ascii_case(name))
                || (looks_like_code(name) && contains_word(&signature.raw_prompt, name))
        }
    }
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
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::{Path, PathBuf};

    const MODEL: &str = r#"
import torch
import torch.nn as nn

class LayerNorm(nn.Module):
    def __init__(self, ndim):
        super().__init__()
        self.weight = nn.Parameter(torch.ones(ndim))

    def forward(self, x):
        return x * self.weight

class GPT(nn.Module):
    def __init__(self, config):
        super().__init__()
        self.ln_f = LayerNorm(config.n_embd)

    def forward(self, idx):
        return self.ln_f(idx)
"#;

    const LOOP: &str = r#"
import torch
from model import GPT

model = GPT(config)
optimizer = torch.optim.AdamW(model.parameters())
while True:
    logits, loss = model(X, Y)
    loss.backward()
    optimizer.step()
    optimizer.zero_grad(set_to_none=True)
"#;

    fn graph() -> NeuralProjectGraph {
        let graph = NeuralProjectGraph::new(ProjectId::new("p"));
        for (rel, src) in [("model.py", MODEL), ("train.py", LOOP), ("bench.py", LOOP)] {
            let file = IndexedFile {
                project_id: ProjectId::new("p"),
                relative_path: PathBuf::from(rel),
                full_path: PathBuf::from(rel),
                blake3_hash: rel.to_string(),
                byte_size: src.len() as u64,
                token_count: 50,
                language: SourceLanguage::Python,
                last_modified: chrono::Utc::now(),
            };
            let ast = CodeIntelligenceEngine::analyze(Path::new(rel), src, SourceLanguage::Python);
            graph.ingest_ast(&file, &ast);
        }
        graph.finalize_links();
        graph
    }

    fn signature(prompt: &str, identifiers: &[&str]) -> TaskSignature {
        let mut sig = neuromesh_task::TaskSignatureExtractor::extract(prompt);
        sig.identifiers = identifiers.iter().map(|s| s.to_string()).collect();
        sig
    }

    fn named(graph: &NeuralProjectGraph, name: &str, file: &str) -> NodeId {
        graph
            .nodes_named(name)
            .into_iter()
            .find(|n| n.file_path.to_string_lossy().contains(file))
            .unwrap_or_else(|| panic!("{name} in {file}"))
            .id
    }

    fn seeded(graph: &NeuralProjectGraph, already: &[NodeId], sig: &TaskSignature) -> Vec<String> {
        let mut out: Vec<String> = resolve_artifact_seeds(graph, sig, already)
            .into_iter()
            .map(|(id, _, _)| id.as_str().to_string())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn a_kind_the_question_already_resolved_by_name_is_not_seeded_again_by_kind() {
        let g = graph();
        let sig = signature(
            "How does the training loop call the model forward?",
            &["training"],
        );
        let train = named(&g, "train", "train.py");
        assert_eq!(g.get_node(&train).unwrap().node_type, NodeType::TrainLoop);

        let with_nothing = seeded(&g, &[], &sig);
        assert!(with_nothing.iter().any(|id| id.contains("bench.py")));
        assert!(with_nothing.iter().any(|id| id.contains("train.py:train")));

        let with_train = seeded(&g, &[train], &sig);
        assert!(
            !with_train.iter().any(|id| id.contains("bench.py")),
            "{with_train:?}"
        );
    }

    #[test]
    fn a_node_the_prompt_names_outright_joins_even_when_its_kind_is_covered() {
        let g = graph();
        let sig = signature(
            "How does the training loop differ from bench?",
            &["training", "bench"],
        );
        let train = named(&g, "train", "train.py");
        let with_train = seeded(&g, &[train], &sig);
        assert!(
            with_train.iter().any(|id| id.contains("bench.py")),
            "{with_train:?}"
        );
    }

    #[test]
    fn a_member_of_a_seeded_owner_outranks_every_other_member_of_the_kind() {
        let g = graph();
        let sig = signature("How does the GPT model forward work?", &["GPT"]);
        let gpt = named(&g, "GPT", "model.py");
        let layers: Vec<String> = seeded(&g, &[gpt], &sig)
            .into_iter()
            .filter(|id| id.ends_with("forward"))
            .collect();
        assert_eq!(layers, vec!["sym:model.py:GPT.forward"]);

        let unowned: Vec<String> = seeded(&g, &[], &sig)
            .into_iter()
            .filter(|id| id.ends_with("forward"))
            .collect();
        assert!(
            unowned.contains(&"sym:model.py:LayerNorm.forward".to_string()),
            "{unowned:?}"
        );
    }

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
