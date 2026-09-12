//! Weak seeds do not cross into a language the question's anchors never touch.
//!
//! "How does get_current_user decode the JWT into TokenPayload and load the
//! User from the session?" names three Python symbols. The concept seeds
//! `auth` and `login` then matched the generated TypeScript client
//! (`auth.gen.ts:Auth`, `login.tsx:Login`), and Physarum hung three more
//! frontend files off them: half the packet answered a question nobody
//! asked. When every strong seed lives in one language family, a weak seed
//! in another family is dropped. With no strong seed, or strong seeds
//! spanning both sides, nothing is pruned — a full-stack question keeps both.

use neuromesh_core::{NodeId, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const STRONG: &[&str] = &[
    "identifier",
    "entity",
    "file",
    "client_keyword",
    "path_hint",
];
const WEAK: &[&str] = &[
    "concept",
    "alias_code",
    "alias_gap_fill",
    "token",
    "fallback",
    "client_expansion",
    "entity_type",
];

fn reason_prefix(query: &str) -> &str {
    query.split(':').next().unwrap_or("")
}

pub(crate) fn family(path: &Path) -> Option<&'static str> {
    neuromesh_core::language_family(path)
}

/// Families of the files the strong seeds resolved to.
pub(crate) fn strong_families(
    graph: &NeuralProjectGraph,
    seeds: &[SeedResolution],
) -> HashSet<&'static str> {
    let mut out = HashSet::new();
    for seed in seeds {
        if !STRONG.contains(&reason_prefix(&seed.query)) {
            continue;
        }
        if let Some(fam) = seed
            .resolved_id
            .as_ref()
            .and_then(|id| graph.get_node(id))
            .and_then(|n| family(&n.file_path))
        {
            out.insert(fam);
        }
    }
    out
}

/// A code file in a family none of the strong seeds live in.
pub(crate) fn off_family(
    graph: &NeuralProjectGraph,
    id: &NodeId,
    families: &HashSet<&'static str>,
) -> bool {
    !families.is_empty()
        && graph
            .get_node(id)
            .and_then(|n| family(&n.file_path))
            .is_some_and(|fam| !families.contains(fam))
}

pub(crate) fn prune_off_family_weak_seeds(
    graph: &NeuralProjectGraph,
    seeds: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
) {
    let file_of = |id: &NodeId| graph.get_node(id).map(|n| n.file_path);
    let strong_families = strong_families(graph, seeds);
    if strong_families.is_empty() {
        return;
    }
    let mut dropped: Vec<NodeId> = Vec::new();
    seeds.retain(|seed| {
        if !WEAK.contains(&reason_prefix(&seed.query)) {
            return true;
        }
        let Some(id) = seed.resolved_id.as_ref() else {
            return true;
        };
        let Some(fam) = file_of(id).and_then(|p| family(&p)) else {
            return true;
        };
        if strong_families.contains(fam) {
            return true;
        }
        dropped.push(id.clone());
        false
    });
    for id in dropped {
        if seeds.iter().any(|s| s.resolved_id.as_ref() == Some(&id)) {
            continue;
        }
        seed_energies.remove(&id);
        seed_reasons.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::PathBuf;

    fn graph_with(files: &[(&str, &str, SourceLanguage)]) -> NeuralProjectGraph {
        let graph = NeuralProjectGraph::new(ProjectId::new("stack"));
        for (rel, src, lang) in files {
            let file = IndexedFile {
                project_id: ProjectId::new("stack"),
                relative_path: PathBuf::from(rel),
                full_path: PathBuf::from(rel),
                blake3_hash: rel.to_string(),
                byte_size: src.len() as u64,
                token_count: 50,
                language: *lang,
                last_modified: chrono::Utc::now(),
            };
            let ast = CodeIntelligenceEngine::analyze(Path::new(rel), src, *lang);
            graph.ingest_ast(&file, &ast);
        }
        graph.finalize_links();
        graph
    }

    fn seed(graph: &NeuralProjectGraph, query: &str, name: &str) -> SeedResolution {
        let id = graph
            .nodes_named(name)
            .into_iter()
            .next()
            .map(|n| n.id)
            .expect("node exists");
        SeedResolution {
            query: query.into(),
            resolved_id: Some(id),
            confidence: 1.0,
            resolution_tier: None,
            embedding_score: None,
        }
    }

    fn fixture() -> NeuralProjectGraph {
        graph_with(&[
            (
                "backend/app/api/deps.py",
                "def get_current_user(token):\n    return token\n",
                SourceLanguage::Python,
            ),
            (
                "frontend/src/client/auth.gen.ts",
                "export function Auth(token: string) {\n  return token;\n}\n",
                SourceLanguage::TypeScript,
            ),
        ])
    }

    #[test]
    fn concept_seed_in_the_other_family_is_dropped() {
        let graph = fixture();
        let mut seeds = vec![
            seed(&graph, "identifier:get_current_user", "get_current_user"),
            seed(&graph, "concept:auth", "Auth"),
        ];
        let auth_id = seeds[1].resolved_id.clone().unwrap();
        let mut energies: HashMap<NodeId, f32> = seeds
            .iter()
            .map(|s| (s.resolved_id.clone().unwrap(), 1.0))
            .collect();
        let mut reasons: HashMap<NodeId, String> = HashMap::new();
        reasons.insert(auth_id.clone(), "concept:auth".into());
        prune_off_family_weak_seeds(&graph, &mut seeds, &mut energies, &mut reasons);
        assert_eq!(seeds.len(), 1);
        assert!(seeds[0].query.starts_with("identifier:"));
        assert!(!energies.contains_key(&auth_id));
        assert!(!reasons.contains_key(&auth_id));
    }

    #[test]
    fn strong_seed_in_the_other_family_is_kept() {
        let graph = fixture();
        let mut seeds = vec![
            seed(&graph, "identifier:get_current_user", "get_current_user"),
            seed(&graph, "identifier:Auth", "Auth"),
        ];
        let mut energies = HashMap::new();
        let mut reasons = HashMap::new();
        prune_off_family_weak_seeds(&graph, &mut seeds, &mut energies, &mut reasons);
        assert_eq!(seeds.len(), 2, "two named symbols: a full-stack question");
    }

    #[test]
    fn without_a_strong_seed_nothing_is_pruned() {
        let graph = fixture();
        let mut seeds = vec![seed(&graph, "concept:auth", "Auth")];
        let mut energies = HashMap::new();
        let mut reasons = HashMap::new();
        prune_off_family_weak_seeds(&graph, &mut seeds, &mut energies, &mut reasons);
        assert_eq!(seeds.len(), 1);
    }

    #[test]
    fn weak_seed_in_the_same_family_is_kept() {
        let graph = graph_with(&[
            (
                "backend/app/api/deps.py",
                "def get_current_user(token):\n    return token\n",
                SourceLanguage::Python,
            ),
            (
                "backend/app/core/security.py",
                "def verify_password(a, b):\n    return a == b\n",
                SourceLanguage::Python,
            ),
        ]);
        let mut seeds = vec![
            seed(&graph, "identifier:get_current_user", "get_current_user"),
            seed(&graph, "concept:verify_password", "verify_password"),
        ];
        let mut energies = HashMap::new();
        let mut reasons = HashMap::new();
        prune_off_family_weak_seeds(&graph, &mut seeds, &mut energies, &mut reasons);
        assert_eq!(seeds.len(), 2);
    }
}
