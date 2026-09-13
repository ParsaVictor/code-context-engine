//! A weak seed that lands on a whole file is a stem guess, not an anchor.
//!
//! `concept:model` — an alias expansion the prompt never contained — resolved
//! to `backend/app/models.py` by file stem, and `expand_file_seeds_to_symbols`
//! then turned that one guess into eight required class seeds. When the
//! question already has a strong seed (a named symbol, an explicit file, a
//! client keyword), a weak seed that resolved to a *file* node is kept only
//! if the prompt actually says that file's stem (inflection-tolerant:
//! `model` ~ `models`). With no strong seed at all nothing is pruned: the
//! guess is then the only anchor we have.

use neuromesh_core::{NodeId, NodeType, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};

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

fn prefix(query: &str) -> &str {
    query.split(':').next().unwrap_or("")
}

fn strip_plural(w: &str) -> &str {
    for suffix in ["ies", "es", "s"] {
        if let Some(base) = w.strip_suffix(suffix) {
            if base.len() >= 3 {
                return base;
            }
        }
    }
    w
}

fn prompt_words(prompt: &str) -> HashSet<String> {
    prompt
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .flat_map(|tok| tok.split('_'))
        .filter(|w| w.len() >= 3)
        .map(|w| strip_plural(&w.to_ascii_lowercase()).to_string())
        .collect()
}

fn stem_named_in_prompt(stem: &str, words: &HashSet<String>) -> bool {
    neuromesh_parser::tokenize_ident(stem)
        .into_iter()
        .map(|t| t.to_ascii_lowercase())
        .any(|t| t.len() >= 3 && words.contains(strip_plural(&t)))
}

pub(crate) fn prune_weak_file_seeds_unnamed_in_prompt(
    graph: &NeuralProjectGraph,
    seeds: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
    prompt: &str,
) {
    let has_strong = seeds
        .iter()
        .any(|s| s.resolved_id.is_some() && STRONG.contains(&prefix(&s.query)));
    if !has_strong {
        return;
    }
    let words = prompt_words(prompt);
    let mut dropped: Vec<NodeId> = Vec::new();
    seeds.retain(|seed| {
        if !WEAK.contains(&prefix(&seed.query)) {
            return true;
        }
        let Some(id) = seed.resolved_id.as_ref() else {
            return true;
        };
        let Some(node) = graph.get_node(id) else {
            return true;
        };
        if node.node_type != NodeType::File {
            return true;
        }
        let stem = node
            .file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if stem_named_in_prompt(stem, &words) {
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
    use std::path::{Path, PathBuf};

    fn graph() -> NeuralProjectGraph {
        let graph = NeuralProjectGraph::new(ProjectId::new("p"));
        for (rel, src) in [
            (
                "backend/app/core/config.py",
                "class Settings:\n    def enforce(self):\n        return 1\n",
            ),
            (
                "backend/app/models.py",
                "class UserBase:\n    pass\n\nclass ItemBase:\n    pass\n",
            ),
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

    fn symbol_seed(graph: &NeuralProjectGraph, query: &str, name: &str) -> SeedResolution {
        let id = graph.nodes_named(name).into_iter().next().unwrap().id;
        SeedResolution {
            query: query.into(),
            resolved_id: Some(id),
            confidence: 1.0,
            resolution_tier: None,
            embedding_score: None,
        }
    }

    fn file_seed(graph: &NeuralProjectGraph, query: &str, path: &str) -> SeedResolution {
        let id = graph.file_id_for_path(Path::new(path)).expect("file node");
        SeedResolution {
            query: query.into(),
            resolved_id: Some(id),
            confidence: 0.8,
            resolution_tier: None,
            embedding_score: None,
        }
    }

    fn run(g: &NeuralProjectGraph, seeds: &mut Vec<SeedResolution>, prompt: &str) -> usize {
        let mut e: HashMap<NodeId, f32> = seeds
            .iter()
            .map(|s| (s.resolved_id.clone().unwrap(), 1.0))
            .collect();
        let mut r = HashMap::new();
        prune_weak_file_seeds_unnamed_in_prompt(g, seeds, &mut e, &mut r, prompt);
        assert_eq!(e.len(), seeds.len());
        seeds.len()
    }

    #[test]
    fn weak_file_seed_unnamed_in_prompt_is_dropped() {
        let g = graph();
        let mut seeds = vec![
            symbol_seed(&g, "identifier:Settings", "Settings"),
            file_seed(&g, "concept:model", "backend/app/models.py"),
        ];
        assert_eq!(
            run(&g, &mut seeds, "How does Settings build the database URI?"),
            1
        );
        assert!(seeds[0].query.starts_with("identifier:"));
    }

    #[test]
    fn weak_file_seed_named_in_prompt_is_kept() {
        let g = graph();
        let mut seeds = vec![
            symbol_seed(&g, "identifier:Settings", "Settings"),
            file_seed(&g, "concept:model", "backend/app/models.py"),
        ];
        assert_eq!(
            run(&g, &mut seeds, "How do the models use Settings?"),
            2,
            "`models` is in the prompt (plural tolerated)"
        );
    }

    #[test]
    fn without_a_strong_seed_the_guess_stays() {
        let g = graph();
        let mut seeds = vec![file_seed(&g, "concept:model", "backend/app/models.py")];
        assert_eq!(run(&g, &mut seeds, "where is the database schema"), 1);
    }

    #[test]
    fn weak_symbol_seed_is_not_touched() {
        let g = graph();
        let mut seeds = vec![
            symbol_seed(&g, "identifier:Settings", "Settings"),
            symbol_seed(&g, "concept:user", "UserBase"),
        ];
        assert_eq!(run(&g, &mut seeds, "How does Settings work?"), 2);
    }
}
