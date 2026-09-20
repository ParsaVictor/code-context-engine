//! Seeds whose symbol is defined in several files land in the same file.
//!
//! vit-pytorch defines `class SimpleViT` in 14 files and `posemb_sincos_2d`
//! in 18; the per-symbol ranking (body size, degree) sent the two seeds of
//! one question to two different variants, neither the one the question
//! named. Two signals that need no knowledge of the repository: the file
//! where the most of the question's seeds are defined together, and a file
//! stem the prompt fully accounts for (`simple_vit` when the prompt says
//! SimpleViT; not `simple_vit_with_fft`, whose `fft` the prompt never said).

use neuromesh_core::{NodeId, NodeType, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_parser::tokenize_ident as tokenize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Sub-word tokens of every prompt word (`SimpleViT` → `simple`, `vit`).
fn prompt_token_set(prompt: &str) -> HashSet<String> {
    prompt
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
        .flat_map(tokenize)
        .map(|t| t.to_lowercase())
        .collect()
}

fn stem_covered(path: &std::path::Path, prompt_tokens: &HashSet<String>) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let toks = tokenize(stem);
    !toks.is_empty()
        && toks
            .iter()
            .all(|t| prompt_tokens.contains(&t.to_lowercase()))
}

/// (seeds defined together, stem named, dir words, body words)
type FileScore = (usize, bool, usize, usize);

struct Twinned {
    seed_idx: usize,
    by_file: HashMap<PathBuf, NodeId>,
}

/// Directory components of `file` the prompt names ("On Unix" →
/// `src/unix/getaddrinfo.c` over `src/win/getaddrinfo.c`). Ranks above body
/// words: the platform variant a question names is settled by its path, and
/// the other variant's body can hold more of the prompt's words simply by
/// being longer (holdout-c, libuv).
fn dir_word_hits(file: &std::path::Path, prompt_tokens: &HashSet<String>) -> usize {
    let mut comps: Vec<&str> = file
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    comps.pop();
    comps
        .iter()
        .flat_map(|c| tokenize(c))
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 3 && prompt_tokens.contains(t))
        .count()
}

/// Distinct prompt words (≥4 letters, not a seed's own name) found in the
/// bodies of the twins that live in `file`. Django defines `Template` in
/// `template/base.py` and again as a thin wrapper in
/// `template/backends/django.py`; both hold `Template` and `Template.render`,
/// so `together` ties. "compile a parsed node list into rendered output"
/// names what only the real one does — `nodelist`, `compile`, `render` are
/// in its body, not the wrapper's (F64).
fn body_word_hits(
    graph: &NeuralProjectGraph,
    file: &PathBuf,
    twinned: &[Twinned],
    prompt_tokens: &HashSet<String>,
    seed_names: &HashSet<String>,
) -> usize {
    let Some(source) = graph.read_source(file) else {
        return 0;
    };
    let lines: Vec<&str> = source.lines().collect();
    let mut body_tokens: HashSet<String> = HashSet::new();
    for t in twinned {
        let Some(id) = t.by_file.get(file) else {
            continue;
        };
        let Some(range) = graph.get_node(id).and_then(|n| n.line_range) else {
            continue;
        };
        let start = range.start.saturating_sub(1).min(lines.len());
        let end = range.end.min(lines.len());
        for line in &lines[start..end] {
            for w in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                for tok in tokenize(w) {
                    body_tokens.insert(tok.to_lowercase());
                }
            }
        }
    }
    prompt_tokens
        .iter()
        .filter(|t| t.len() >= 4 && !seed_names.contains(*t) && body_tokens.contains(*t))
        .count()
}

pub(crate) fn cohere_twin_definitions(
    graph: &NeuralProjectGraph,
    seeds: &mut [SeedResolution],
    seed_energies: &mut HashMap<NodeId, f32>,
    prompt: &str,
) {
    let mut twinned: Vec<Twinned> = Vec::new();
    let mut anchor_files: HashSet<PathBuf> = HashSet::new();
    for (idx, seed) in seeds.iter().enumerate() {
        let Some(id) = seed.resolved_id.as_ref() else {
            continue;
        };
        let Some(node) = graph.get_node(id) else {
            continue;
        };
        // Data and config files (a `config` key in components.json) neither
        // anchor code seeds nor count as twins of a code symbol.
        if crate::seed::lang_cohere::family(&node.file_path).is_none() {
            continue;
        }
        if node.node_type == NodeType::File {
            anchor_files.insert(node.file_path.clone());
            continue;
        }
        // Same name under the same parent is a twin whatever the node type:
        // the `User` class in models.py and the `User` component in
        // User.tsx are both what the prompt's `User` could mean.
        let mut by_file: HashMap<PathBuf, NodeId> = HashMap::new();
        for twin in graph.nodes_named(&node.name) {
            if twin.node_type == NodeType::File
                || twin.parent != node.parent
                || crate::seed::lang_cohere::family(&twin.file_path).is_none()
            {
                continue;
            }
            by_file.entry(twin.file_path).or_insert(twin.id);
        }
        if by_file.len() >= 2 {
            twinned.push(Twinned {
                seed_idx: idx,
                by_file,
            });
        } else {
            anchor_files.insert(node.file_path.clone());
        }
    }
    if twinned.is_empty() {
        return;
    }

    let prompt_tokens = prompt_token_set(prompt);
    let mut candidates: HashSet<&PathBuf> = HashSet::new();
    for t in &twinned {
        candidates.extend(t.by_file.keys());
    }
    let seed_names: HashSet<String> = twinned
        .iter()
        .filter_map(|t| t.by_file.values().next())
        .filter_map(|id| graph.get_node(id))
        .flat_map(|n| tokenize(&n.name))
        .map(|t| t.to_lowercase())
        .collect();
    let score = |file: &PathBuf| -> FileScore {
        let together = twinned
            .iter()
            .filter(|t| t.by_file.contains_key(file))
            .count()
            + usize::from(anchor_files.contains(file));
        (
            together,
            stem_covered(file, &prompt_tokens),
            dir_word_hits(file, &prompt_tokens),
            body_word_hits(graph, file, &twinned, &prompt_tokens, &seed_names),
        )
    };
    let mut ranked: Vec<(&PathBuf, FileScore)> =
        candidates.into_iter().map(|f| (f, score(f))).collect();
    ranked.sort_by(|a, b| {
        b.1 .0
            .cmp(&a.1 .0)
            .then_with(|| b.1 .1.cmp(&a.1 .1))
            .then_with(|| b.1 .2.cmp(&a.1 .2))
            .then_with(|| b.1 .3.cmp(&a.1 .3))
            .then_with(|| a.0.cmp(b.0))
    });
    let (best_file, best_score) = ranked[0];
    // Without a second seed or a named stem there is nothing to prefer; the
    // per-symbol ranking stands. Body words only break a tie between files
    // that already qualify.
    if best_score.0 < 2 && !best_score.1 {
        return;
    }
    if ranked.len() > 1 && ranked[1].1 == best_score {
        return;
    }

    for t in &twinned {
        let Some(target) = t.by_file.get(best_file) else {
            continue;
        };
        let seed = &mut seeds[t.seed_idx];
        let Some(current) = seed.resolved_id.clone() else {
            continue;
        };
        if &current == target {
            continue;
        }
        let energy = seed_energies.remove(&current).unwrap_or(0.9);
        seed_energies
            .entry(target.clone())
            .and_modify(|e| *e = e.max(energy))
            .or_insert(energy);
        seed.resolved_id = Some(target.clone());
        seed.resolution_tier = Some("twin_cohere".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use std::path::Path;

    fn graph_with(files: &[(&str, &str)]) -> NeuralProjectGraph {
        let graph = NeuralProjectGraph::new(ProjectId::new("twins"));
        for (rel, src) in files {
            let file = IndexedFile {
                project_id: ProjectId::new("twins"),
                relative_path: PathBuf::from(rel),
                full_path: PathBuf::from(rel),
                blake3_hash: rel.to_string(),
                byte_size: src.len() as u64,
                token_count: 80,
                language: SourceLanguage::Python,
                last_modified: chrono::Utc::now(),
            };
            let ast = CodeIntelligenceEngine::analyze(Path::new(rel), src, SourceLanguage::Python);
            graph.ingest_file(&file, &ast, Some(src));
        }
        graph.finalize_links();
        graph
    }

    fn seed_in(graph: &NeuralProjectGraph, file: &str, name: &str) -> SeedResolution {
        let id = graph
            .nodes_named(name)
            .into_iter()
            .find(|n| n.file_path.to_string_lossy().replace('\\', "/") == file)
            .map(|n| n.id)
            .expect("node exists");
        SeedResolution {
            query: name.into(),
            resolved_id: Some(id),
            confidence: 1.0,
            resolution_tier: Some("L1_exact".into()),
            embedding_score: None,
        }
    }

    fn file_of(graph: &NeuralProjectGraph, seed: &SeedResolution) -> String {
        graph
            .get_node(seed.resolved_id.as_ref().unwrap())
            .unwrap()
            .file_path
            .to_string_lossy()
            .replace('\\', "/")
    }

    const SIMPLE: &str = "def posemb_sincos_2d(h, w, dim):\n    return h\n\nclass SimpleViT:\n    def forward(self, img):\n        return img\n";
    const FFT: &str = "def posemb_sincos_2d(h, w, dim):\n    return w\n\nclass SimpleViT:\n    def forward(self, img):\n        return img * 2\n";
    const JUMBO: &str = "def posemb_sincos_2d(h, w, dim):\n    return dim\n\nclass JumboViT:\n    def forward(self, img):\n        return img\n";

    #[test]
    fn two_seeds_land_in_the_file_the_prompt_names() {
        let graph = graph_with(&[
            ("vit/simple_vit.py", SIMPLE),
            ("vit/simple_vit_with_fft.py", FFT),
            ("vit/jumbo_vit.py", JUMBO),
        ]);
        let mut seeds = vec![
            seed_in(&graph, "vit/simple_vit_with_fft.py", "SimpleViT"),
            seed_in(&graph, "vit/jumbo_vit.py", "posemb_sincos_2d"),
        ];
        let mut energies: HashMap<NodeId, f32> = seeds
            .iter()
            .map(|s| (s.resolved_id.clone().unwrap(), 1.0))
            .collect();
        cohere_twin_definitions(
            &graph,
            &mut seeds,
            &mut energies,
            "How does SimpleViT build the 2D sincos positional embedding with posemb_sincos_2d?",
        );
        assert_eq!(file_of(&graph, &seeds[0]), "vit/simple_vit.py");
        assert_eq!(file_of(&graph, &seeds[1]), "vit/simple_vit.py");
        assert_eq!(
            energies.len(),
            2,
            "energy moved with the seeds: {energies:?}"
        );
        assert!(energies
            .keys()
            .all(|id| id.as_str().contains("simple_vit.py")));
    }

    #[test]
    fn tied_twins_go_to_the_body_the_prompt_describes_then_to_the_named_dir() {
        // F64: `Template` + `Template.render` exist in both files (together
        // ties); the real one compiles a nodelist, the wrapper delegates.
        let real = "class Template:\n    def render(self, context):\n        nodelist = self.compile_nodelist()\n        return nodelist.render(context)\n";
        let wrapper = "class Template:\n    def render(self, context=None, request=None):\n        return self.template.render(context)\n";
        let graph = graph_with(&[
            ("template/base.py", real),
            ("template/backends/django.py", wrapper),
        ]);
        let mut seeds = vec![
            seed_in(&graph, "template/backends/django.py", "Template"),
            seed_in(&graph, "template/backends/django.py", "render"),
        ];
        let mut energies: HashMap<NodeId, f32> = seeds
            .iter()
            .map(|s| (s.resolved_id.clone().unwrap(), 1.0))
            .collect();
        cohere_twin_definitions(
            &graph,
            &mut seeds,
            &mut energies,
            "How does Template.render compile a parsed nodelist into rendered output for a context?",
        );
        assert_eq!(file_of(&graph, &seeds[0]), "template/base.py");
        assert_eq!(file_of(&graph, &seeds[1]), "template/base.py");

        // A directory the prompt names beats body words: "On Unix" picks
        // src/unix even when the win body holds more of the prompt's words.
        let unix = "def uv_getaddrinfo(loop):\n    return 0\n";
        let win = "def uv_getaddrinfo(loop):\n    hostname = service = hints = buffer = validate = 0\n    return 0\n";
        let graph = graph_with(&[
            ("src/unix/getaddrinfo.py", unix),
            ("src/win/getaddrinfo.py", win),
        ]);
        let mut seeds = vec![seed_in(&graph, "src/win/getaddrinfo.py", "uv_getaddrinfo")];
        let mut energies: HashMap<NodeId, f32> = seeds
            .iter()
            .map(|s| (s.resolved_id.clone().unwrap(), 1.0))
            .collect();
        cohere_twin_definitions(
            &graph,
            &mut seeds,
            &mut energies,
            "On Unix, how does uv_getaddrinfo validate the hostname, service and hints buffer?",
        );
        assert_eq!(file_of(&graph, &seeds[0]), "src/unix/getaddrinfo.py");
    }

    #[test]
    fn a_lone_twin_with_no_named_stem_keeps_the_ranking() {
        let graph = graph_with(&[
            ("vit/simple_vit.py", SIMPLE),
            ("vit/simple_vit_with_fft.py", FFT),
        ]);
        let mut seeds = vec![seed_in(&graph, "vit/simple_vit_with_fft.py", "forward")];
        let mut energies = HashMap::new();
        cohere_twin_definitions(&graph, &mut seeds, &mut energies, "how does forward work");
        assert_eq!(file_of(&graph, &seeds[0]), "vit/simple_vit_with_fft.py");
    }

    #[test]
    fn a_tie_between_files_changes_nothing() {
        let graph = graph_with(&[
            ("vit/simple_vit.py", SIMPLE),
            ("vit/simple_vit_with_fft.py", FFT),
        ]);
        let mut seeds = vec![seed_in(
            &graph,
            "vit/simple_vit_with_fft.py",
            "posemb_sincos_2d",
        )];
        let mut energies = HashMap::new();
        cohere_twin_definitions(&graph, &mut seeds, &mut energies, "posemb_sincos_2d?");
        assert_eq!(file_of(&graph, &seeds[0]), "vit/simple_vit_with_fft.py");
    }

    #[test]
    fn a_file_seed_anchors_the_twins() {
        let graph = graph_with(&[
            ("vit/simple_vit.py", SIMPLE),
            ("vit/simple_vit_with_fft.py", FFT),
            ("vit/jumbo_vit.py", JUMBO),
        ]);
        let file_id = graph
            .file_node_paths()
            .into_iter()
            .find(|(_, p)| p.to_string_lossy().contains("with_fft"))
            .unwrap()
            .0;
        let mut seeds = vec![
            SeedResolution {
                query: "simple_vit_with_fft.py".into(),
                resolved_id: Some(file_id),
                confidence: 0.95,
                resolution_tier: None,
                embedding_score: None,
            },
            seed_in(&graph, "vit/jumbo_vit.py", "posemb_sincos_2d"),
        ];
        let mut energies = HashMap::new();
        cohere_twin_definitions(
            &graph,
            &mut seeds,
            &mut energies,
            "posemb_sincos_2d in simple_vit_with_fft.py",
        );
        assert_eq!(file_of(&graph, &seeds[1]), "vit/simple_vit_with_fft.py");
    }
}
