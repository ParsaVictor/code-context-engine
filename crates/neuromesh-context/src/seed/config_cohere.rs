//! A data file is not an anchor for a question about code.
//!
//! "How does Settings check for default secrets and build the database
//! URI?" is about a Python class. The client keyword `utils` still resolved
//! to `frontend/components.json:utils` — a JSON key — and the whole manifest
//! shipped as a seed file. JSON/YAML/TOML nodes sit outside every language
//! family, so `lang_cohere` never saw them as "off family". When at least one
//! strong seed is anchored in a code family and the prompt never asks for
//! config/json/yaml/… by name, a seed that resolved to a data node is dropped.
//! A seed the user gave as an explicit file path is kept: they asked for it.

use neuromesh_core::{NodeId, NodeType, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const DATA_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "ini", "env"];

/// Words that make a data file a legitimate anchor when they appear in the
/// prompt. Deliberately excludes `settings`: that is also a common class name.
const CONFIG_WORDS: &[&str] = &[
    "config",
    "configuration",
    "json",
    "yaml",
    "yml",
    "toml",
    "manifest",
    "package.json",
    "tsconfig",
    "pyproject",
    "dotenv",
    ".env",
];

fn is_data_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| DATA_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn prompt_asks_for_config(prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    CONFIG_WORDS.iter().any(|w| {
        if w.contains('.') {
            return p.contains(w);
        }
        p.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.')
            .any(|tok| tok.trim_matches('.') == *w)
    })
}

fn explicit_file_seed(query: &str) -> bool {
    let prefix = query.split(':').next().unwrap_or("");
    matches!(prefix, "file" | "path_hint")
}

/// Drop seeds that resolved to a data node when the question is anchored in
/// code and never mentions configuration.
pub(crate) fn prune_data_seeds_for_code_question(
    graph: &NeuralProjectGraph,
    seeds: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
    prompt: &str,
) {
    if prompt_asks_for_config(prompt) {
        return;
    }
    let code_anchored: HashSet<&'static str> = super::lang_cohere::strong_families(graph, seeds);
    if code_anchored.is_empty() {
        return;
    }
    let mut dropped: Vec<NodeId> = Vec::new();
    seeds.retain(|seed| {
        if explicit_file_seed(&seed.query) {
            return true;
        }
        let Some(id) = seed.resolved_id.as_ref() else {
            return true;
        };
        let Some(node) = graph.get_node(id) else {
            return true;
        };
        let data = node.node_type == NodeType::Config || is_data_path(&node.file_path);
        if !data {
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
            .unwrap_or_else(|| panic!("node {name} exists"));
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
                "backend/app/core/config.py",
                "class Settings:\n    def enforce(self):\n        return 1\n",
                SourceLanguage::Python,
            ),
            (
                "frontend/components.json",
                "{\n  \"utils\": \"src/lib/utils\",\n  \"style\": \"default\"\n}\n",
                SourceLanguage::JSON,
            ),
        ])
    }

    fn run(graph: &NeuralProjectGraph, seeds: &mut Vec<SeedResolution>, prompt: &str) {
        let mut energies: HashMap<NodeId, f32> = seeds
            .iter()
            .filter_map(|s| s.resolved_id.clone().map(|id| (id, 1.0)))
            .collect();
        let mut reasons: HashMap<NodeId, String> = HashMap::new();
        prune_data_seeds_for_code_question(graph, seeds, &mut energies, &mut reasons, prompt);
        for s in seeds.iter() {
            assert!(energies.contains_key(s.resolved_id.as_ref().unwrap()));
        }
    }

    #[test]
    fn json_key_seed_is_dropped_from_a_code_question() {
        let graph = fixture();
        let mut seeds = vec![
            seed(&graph, "identifier:Settings", "Settings"),
            seed(&graph, "client_keyword:utils", "utils"),
        ];
        run(
            &graph,
            &mut seeds,
            "How does Settings check for default secrets and build the database URI?",
        );
        assert_eq!(seeds.len(), 1);
        assert!(seeds[0].query.starts_with("identifier:"));
    }

    #[test]
    fn json_seed_is_kept_when_the_prompt_asks_for_config() {
        let graph = fixture();
        let mut seeds = vec![
            seed(&graph, "identifier:Settings", "Settings"),
            seed(&graph, "client_keyword:utils", "utils"),
        ];
        run(
            &graph,
            &mut seeds,
            "Where is the utils alias declared in the components.json config?",
        );
        assert_eq!(seeds.len(), 2);
    }

    #[test]
    fn json_seed_is_kept_without_a_code_anchor() {
        let graph = fixture();
        let mut seeds = vec![seed(&graph, "client_keyword:utils", "utils")];
        run(&graph, &mut seeds, "what is utils");
        assert_eq!(seeds.len(), 1, "no strong code seed: nothing to cohere to");
    }

    #[test]
    fn explicit_file_seed_is_kept() {
        let graph = fixture();
        let mut seeds = vec![
            seed(&graph, "identifier:Settings", "Settings"),
            seed(&graph, "file:frontend/components.json", "utils"),
        ];
        run(
            &graph,
            &mut seeds,
            "How does Settings relate to the frontend aliases?",
        );
        assert_eq!(seeds.len(), 2, "the user named the file");
    }

    #[test]
    fn settings_alone_does_not_count_as_a_config_word() {
        assert!(!prompt_asks_for_config(
            "How does Settings check for default secrets?"
        ));
        assert!(prompt_asks_for_config("where is the yaml config loaded"));
        assert!(prompt_asks_for_config("look at package.json scripts"));
    }
}
