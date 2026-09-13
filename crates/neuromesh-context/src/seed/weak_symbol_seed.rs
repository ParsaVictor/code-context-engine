//! A weak seed that reached a symbol only by substring is a guess, not an
//! anchor — the sibling of `weak_file_seed` one level down.
//!
//! `concept:config` — an alias expansion of "configurator" — has no exact
//! symbol, so the resolver's last resort is prefix search, which lands on
//! `GPT.configure_optimizers` in a file the question never touched; the
//! same path takes `concept:token` to `Tokenizer`. When the question already
//! has a strong seed (a named symbol, an explicit file), a weak seed whose
//! resolved symbol is *not* the name it asked for is kept only if the prompt
//! says that symbol's full name (`configure optimizers`, not `config`). With
//! no strong seed nothing is pruned: the guess is then the only anchor.

use super::weak_file_seed::{prefix, prompt_words, strip_plural, STRONG, WEAK};
use neuromesh_core::{NodeId, NodeType, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};

fn body(query: &str) -> &str {
    query.split_once(':').map(|(_, b)| b).unwrap_or(query)
}

/// The name the query asked for: the member of an `owner.member` pair,
/// the whole query otherwise.
fn asked_name(query: &str) -> String {
    let b = body(query);
    b.rsplit('.').next().unwrap_or(b).to_ascii_lowercase()
}

fn is_strong(seed: &SeedResolution) -> bool {
    STRONG.contains(&prefix(&seed.query))
        || seed
            .resolution_tier
            .as_deref()
            .is_some_and(|t| t.starts_with("hierarchical:file"))
}

fn symbol_named_in_prompt(name: &str, words: &HashSet<String>) -> bool {
    let tokens: Vec<String> = neuromesh_parser::tokenize_ident(name)
        .into_iter()
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| t.len() >= 3)
        .collect();
    !tokens.is_empty() && tokens.iter().all(|t| words.contains(strip_plural(t)))
}

pub(crate) fn prune_weak_substring_symbol_seeds(
    graph: &NeuralProjectGraph,
    seeds: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
    prompt: &str,
) {
    let has_strong = seeds
        .iter()
        .any(|s| s.resolved_id.is_some() && is_strong(s));
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
        if node.node_type == NodeType::File {
            return true;
        }
        if node.name.eq_ignore_ascii_case(&asked_name(&seed.query)) {
            return true;
        }
        if symbol_named_in_prompt(&node.name, &words) {
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
                "configurator.py",
                "import sys\nfor arg in sys.argv[1:]:\n    print(arg)\n",
            ),
            (
                "model.py",
                "class GPT:\n    def configure_optimizers(self, wd):\n        return wd\n\n    def config(self):\n        return 1\n",
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

    fn seed(
        graph: &NeuralProjectGraph,
        query: &str,
        name: &str,
        tier: Option<&str>,
    ) -> SeedResolution {
        let id = match graph.nodes_named(name).into_iter().next() {
            Some(n) => n.id,
            None => graph.file_id_for_path(Path::new(name)).expect("node"),
        };
        SeedResolution {
            query: query.into(),
            resolved_id: Some(id),
            confidence: 0.8,
            resolution_tier: tier.map(str::to_string),
            embedding_score: None,
        }
    }

    fn run(g: &NeuralProjectGraph, seeds: &mut Vec<SeedResolution>, prompt: &str) -> Vec<String> {
        let mut e: HashMap<NodeId, f32> = seeds
            .iter()
            .map(|s| (s.resolved_id.clone().unwrap(), 1.0))
            .collect();
        let mut r = HashMap::new();
        prune_weak_substring_symbol_seeds(g, seeds, &mut e, &mut r, prompt);
        assert_eq!(e.len(), seeds.len());
        seeds.iter().map(|s| s.query.clone()).collect()
    }

    #[test]
    fn substring_hit_in_another_file_is_dropped_next_to_a_strong_seed() {
        let g = graph();
        let mut seeds = vec![
            seed(
                &g,
                "configurator.py",
                "configurator.py",
                Some("hierarchical:file"),
            ),
            seed(&g, "concept:config", "configure_optimizers", None),
        ];
        assert_eq!(
            run(
                &g,
                &mut seeds,
                "How does configurator.py override config globals?"
            ),
            vec!["configurator.py"]
        );
    }

    #[test]
    fn exact_name_hit_and_prompt_named_symbol_are_kept() {
        let g = graph();
        let mut seeds = vec![
            seed(&g, "file:configurator.py", "configurator.py", None),
            seed(&g, "concept:config", "config", None),
            seed(&g, "concept:optim", "configure_optimizers", None),
        ];
        assert_eq!(
            run(
                &g,
                &mut seeds,
                "How does configurator.py set config and configure optimizers?"
            )
            .len(),
            3
        );
    }

    #[test]
    fn nothing_is_pruned_without_a_strong_seed() {
        let g = graph();
        let mut seeds = vec![seed(&g, "concept:config", "configure_optimizers", None)];
        assert_eq!(run(&g, &mut seeds, "where is config read?").len(), 1);
    }
}
