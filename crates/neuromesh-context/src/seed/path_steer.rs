//! Prompt words that name a directory steer seed resolution.
//!
//! nanoGPT has three `data/*/prepare.py`. The question "how does the
//! shakespeare_char prepare script build the vocabulary" names the directory
//! outright, but `shakespeare_char` is neither a symbol nor a file, so every
//! resolver passed on it, the lexical fallback took the first `prepare.py` the
//! index yielded (`openwebtext`), and the gold file scored recall 0. Nothing
//! here knows about nanoGPT: a bare query that equals a directory segment of
//! indexed files becomes a candidate set, and the other prompt words pick the
//! file inside it.

use neuromesh_core::NodeId;
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_task::{is_prompt_stopword, normalize_prompt_tokens};
use std::path::{Path, PathBuf};

fn norm(s: &str) -> String {
    s.replace('-', "_").to_lowercase()
}

/// Path relative to the workspace, so an absolute index never exposes the
/// checkout's own directory names (`nanoGPT`, `repo`) as steerable segments.
fn relative_to_workspace(graph: &NeuralProjectGraph, path: &Path) -> PathBuf {
    match graph.workspace_root() {
        Some(root) => path.strip_prefix(&root).unwrap_or(path).to_path_buf(),
        None => path.to_path_buf(),
    }
}

fn dir_segments(rel: &Path) -> Vec<String> {
    let mut segs: Vec<String> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .map(norm)
        .collect();
    segs.pop();
    segs
}

fn file_stem_norm(rel: &Path) -> Option<String> {
    rel.file_stem().and_then(|s| s.to_str()).map(norm)
}

fn steer_tokens(prompt: &str, exclude: &str) -> Vec<String> {
    normalize_prompt_tokens(prompt)
        .into_iter()
        .map(|t| norm(&t))
        .filter(|t| t.len() >= 3 && t != exclude && !is_prompt_stopword(t))
        .collect()
}

/// Among `candidates`, the one whose stem or directory the prompt also names.
/// `None` unless exactly one candidate scores best and above zero.
fn pick_by_prompt(candidates: &[(NodeId, PathBuf)], tokens: &[String]) -> Option<NodeId> {
    let mut best: Option<(usize, &NodeId)> = None;
    let mut tied = false;
    for (id, rel) in candidates {
        let stem = file_stem_norm(rel);
        let segs = dir_segments(rel);
        let score = tokens
            .iter()
            .filter(|t| stem.as_deref() == Some(t.as_str()) || segs.iter().any(|s| s == *t))
            .count();
        match best {
            Some((b, _)) if score < b => {}
            Some((b, _)) if score == b => tied = true,
            _ => {
                best = Some((score, id));
                tied = false;
            }
        }
    }
    match best {
        Some((score, id)) if score > 0 && !tied => Some(id.clone()),
        _ => None,
    }
}

/// A bare query that equals a directory segment resolves to the single file
/// under that directory the rest of the prompt points at.
pub(crate) fn resolve_dir_segment_seed(
    graph: &NeuralProjectGraph,
    query: &str,
    prompt: &str,
) -> Option<NodeId> {
    let q = norm(query.trim());
    if q.len() < 4 || q.contains(['/', '\\', '.', ':']) || is_prompt_stopword(&q) {
        return None;
    }
    let under: Vec<(NodeId, PathBuf)> = graph
        .file_node_paths()
        .into_iter()
        .map(|(id, p)| (id, relative_to_workspace(graph, &p)))
        .filter(|(_, rel)| dir_segments(rel).iter().any(|s| s == &q))
        .collect();
    match under.len() {
        0 => None,
        1 => Some(under[0].0.clone()),
        _ => pick_by_prompt(&under, &steer_tokens(prompt, &q)),
    }
}

/// A resolved file that shares its name with other indexed files moves to
/// the twin whose directory the prompt names, when exactly one does.
pub(crate) fn steer_same_name_file(
    graph: &NeuralProjectGraph,
    resolved: &NodeId,
    prompt: &str,
) -> Option<NodeId> {
    let node = graph.get_node(resolved)?;
    if node.node_type != neuromesh_core::NodeType::File {
        return None;
    }
    let rel = relative_to_workspace(graph, &node.file_path);
    let name = norm(rel.file_name()?.to_str()?);
    let twins: Vec<(NodeId, PathBuf)> = graph
        .file_node_paths()
        .into_iter()
        .map(|(id, p)| (id, relative_to_workspace(graph, &p)))
        .filter(|(_, r)| {
            r.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| norm(n) == name)
        })
        .collect();
    if twins.len() < 2 {
        return None;
    }
    let tokens = steer_tokens(prompt, "");
    // Only directory words decide here; the shared stem scores every twin alike.
    let mut best: Option<(usize, &NodeId)> = None;
    let mut tied = false;
    for (id, r) in &twins {
        let segs = dir_segments(r);
        let score = tokens
            .iter()
            .filter(|t| segs.iter().any(|s| s == *t))
            .count();
        match best {
            Some((b, _)) if score < b => {}
            Some((b, _)) if score == b => tied = true,
            _ => {
                best = Some((score, id));
                tied = false;
            }
        }
    }
    match best {
        Some((score, id)) if score > 0 && !tied && id != resolved => Some(id.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};

    fn graph_with(paths: &[&str]) -> NeuralProjectGraph {
        let graph = NeuralProjectGraph::new(ProjectId::new("steer"));
        for p in paths {
            let file = IndexedFile {
                project_id: ProjectId::new("steer"),
                relative_path: PathBuf::from(p),
                full_path: PathBuf::from(p),
                blake3_hash: p.to_string(),
                byte_size: 100,
                token_count: 40,
                language: SourceLanguage::Python,
                last_modified: chrono::Utc::now(),
            };
            graph.add_file_node(&file, Some(String::new()));
        }
        graph
    }

    fn path_of(graph: &NeuralProjectGraph, id: &NodeId) -> String {
        graph
            .get_node(id)
            .unwrap()
            .file_path
            .to_string_lossy()
            .replace('\\', "/")
    }

    #[test]
    fn directory_word_plus_stem_picks_the_twin() {
        let graph = graph_with(&[
            "data/openwebtext/prepare.py",
            "data/shakespeare/prepare.py",
            "data/shakespeare_char/prepare.py",
            "data/shakespeare_char/readme.md",
            "model.py",
        ]);
        let id = resolve_dir_segment_seed(
            &graph,
            "shakespeare_char",
            "How does the shakespeare_char prepare script build the character vocabulary?",
        )
        .expect("steered");
        assert_eq!(path_of(&graph, &id), "data/shakespeare_char/prepare.py");
    }

    #[test]
    fn directory_word_alone_is_ambiguous_across_files() {
        let graph = graph_with(&[
            "data/shakespeare_char/prepare.py",
            "data/shakespeare_char/readme.md",
        ]);
        assert!(resolve_dir_segment_seed(&graph, "shakespeare_char", "shakespeare_char").is_none());
    }

    #[test]
    fn a_lone_file_under_the_directory_needs_no_other_word() {
        let graph = graph_with(&["app/billing/invoice.py", "app/main.py"]);
        let id = resolve_dir_segment_seed(&graph, "billing", "what does billing do").unwrap();
        assert_eq!(path_of(&graph, &id), "app/billing/invoice.py");
    }

    #[test]
    fn stopwords_short_words_and_paths_are_not_directory_queries() {
        let graph = graph_with(&["data/x/prepare.py", "data/y/prepare.py"]);
        assert!(resolve_dir_segment_seed(&graph, "the", "the data").is_none());
        assert!(resolve_dir_segment_seed(&graph, "dat", "dat").is_none());
        assert!(resolve_dir_segment_seed(&graph, "data/x", "data/x").is_none());
    }

    #[test]
    fn same_name_file_moves_to_the_directory_the_prompt_names() {
        let graph = graph_with(&[
            "data/openwebtext/prepare.py",
            "data/shakespeare/prepare.py",
            "data/shakespeare_char/prepare.py",
        ]);
        let wrong = graph
            .file_node_paths()
            .into_iter()
            .find(|(_, p)| p.to_string_lossy().contains("openwebtext"))
            .unwrap()
            .0;
        let moved = steer_same_name_file(
            &graph,
            &wrong,
            "How does the shakespeare_char prepare script encode characters?",
        )
        .expect("moved");
        assert_eq!(path_of(&graph, &moved), "data/shakespeare_char/prepare.py");
    }

    #[test]
    fn same_name_file_stays_when_the_prompt_names_no_directory() {
        let graph = graph_with(&["data/openwebtext/prepare.py", "data/shakespeare/prepare.py"]);
        let first = graph.file_node_paths()[0].0.clone();
        assert!(steer_same_name_file(&graph, &first, "how does prepare tokenize").is_none());
    }

    #[test]
    fn same_name_file_stays_when_it_already_matches() {
        let graph = graph_with(&[
            "data/openwebtext/prepare.py",
            "data/shakespeare_char/prepare.py",
        ]);
        let right = graph
            .file_node_paths()
            .into_iter()
            .find(|(_, p)| p.to_string_lossy().contains("shakespeare_char"))
            .unwrap()
            .0;
        assert!(steer_same_name_file(&graph, &right, "shakespeare_char prepare").is_none());
    }
}
