//! A bare member named next to its owner belongs to that owner.
//!
//! "How does BasePredictor.stream_inference ... and how does postprocess turn
//! raw predictions into results?" yields the bare identifier `postprocess`.
//! Every predictor subclass defines one, so the resolver picks whichever
//! `postprocess` ranks first in isolation (`sam/predict.py`) — a different
//! file from the owner the same question named. When another seed of the
//! question resolved to an owner that has a member of that name, the bare
//! member is re-pointed at that member.

use neuromesh_core::{NodeId, NodeType, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashMap;

fn bare_identifier(query: &str) -> Option<&str> {
    let body = match query.split_once(':') {
        Some(("identifier", body)) => body,
        Some(_) => return None,
        None => query,
    };
    (!body.contains(['.', '/', '\\', ':'])).then_some(body)
}

pub(crate) fn repoint_bare_members_to_seeded_owners(
    graph: &NeuralProjectGraph,
    seeds: &mut [SeedResolution],
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
) {
    let owners: Vec<String> = seeds
        .iter()
        .filter_map(|s| s.resolved_id.as_ref())
        .filter_map(|id| graph.get_node(id))
        .filter(|n| n.node_type != NodeType::File && n.parent.is_none())
        .map(|n| n.name.clone())
        .collect();
    if owners.is_empty() {
        return;
    }
    let mut moves: Vec<(NodeId, NodeId)> = Vec::new();
    for seed in seeds.iter_mut() {
        let Some(member) = bare_identifier(&seed.query) else {
            continue;
        };
        let Some(current) = seed.resolved_id.clone() else {
            continue;
        };
        let Some(node) = graph.get_node(&current) else {
            continue;
        };
        let Some(parent) = node.parent.as_deref() else {
            continue;
        };
        if owners.iter().any(|o| o.eq_ignore_ascii_case(parent)) {
            continue;
        }
        let mut owned: Vec<NodeId> = owners
            .iter()
            .flat_map(|owner| graph.members_of_owner(owner, member))
            .collect();
        owned.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        owned.dedup();
        if owned.len() != 1 {
            continue;
        }
        let target = owned.remove(0);
        if target == current {
            continue;
        }
        seed.resolved_id = Some(target.clone());
        moves.push((current, target));
    }
    for (from, to) in moves {
        if let Some(energy) = seed_energies.remove(&from) {
            seed_energies.entry(to.clone()).or_insert(energy);
        }
        if let Some(reason) = seed_reasons.remove(&from) {
            seed_reasons.entry(to).or_insert(reason);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::{Path, PathBuf};

    const BASE: &str = r#"
class BasePredictor:
    def stream_inference(self):
        return self.postprocess()

    def postprocess(self):
        return 1
"#;

    const SAM: &str = r#"
class Predictor:
    def postprocess(self):
        return 2
"#;

    fn graph() -> NeuralProjectGraph {
        let graph = NeuralProjectGraph::new(ProjectId::new("p"));
        for (rel, src) in [
            ("engine/predictor.py", BASE),
            ("models/sam/predict.py", SAM),
        ] {
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

    fn seed(query: &str, id: &str) -> SeedResolution {
        SeedResolution {
            query: query.into(),
            resolved_id: Some(NodeId::new(id)),
            confidence: 1.0,
            resolution_tier: None,
            embedding_score: None,
        }
    }

    #[test]
    fn a_bare_member_moves_to_the_owner_the_question_named() {
        let g = graph();
        let mut seeds = vec![
            seed(
                "identifier:BasePredictor",
                "sym:engine/predictor.py:BasePredictor",
            ),
            seed(
                "identifier:postprocess",
                "sym:models/sam/predict.py:Predictor.postprocess",
            ),
        ];
        let mut e: HashMap<NodeId, f32> = seeds
            .iter()
            .map(|s| (s.resolved_id.clone().unwrap(), 1.0))
            .collect();
        let mut r = HashMap::new();
        repoint_bare_members_to_seeded_owners(&g, &mut seeds, &mut e, &mut r);
        assert_eq!(
            seeds[1].resolved_id.as_ref().unwrap().as_str(),
            "sym:engine/predictor.py:BasePredictor.postprocess"
        );
        assert!(e.contains_key(&NodeId::new(
            "sym:engine/predictor.py:BasePredictor.postprocess"
        )));
        assert!(!e.contains_key(&NodeId::new(
            "sym:models/sam/predict.py:Predictor.postprocess"
        )));
    }

    #[test]
    fn without_a_seeded_owner_nothing_moves() {
        let g = graph();
        let mut seeds = vec![seed(
            "identifier:postprocess",
            "sym:models/sam/predict.py:Predictor.postprocess",
        )];
        let mut e = HashMap::new();
        let mut r = HashMap::new();
        repoint_bare_members_to_seeded_owners(&g, &mut seeds, &mut e, &mut r);
        assert_eq!(
            seeds[0].resolved_id.as_ref().unwrap().as_str(),
            "sym:models/sam/predict.py:Predictor.postprocess"
        );
    }
}
