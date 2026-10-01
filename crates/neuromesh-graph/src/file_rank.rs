//! Field-weighted BM25 (BM25F) over files, for plain-language questions.
//!
//! A question that names no identifier ("where are uploaded images
//! resized?") used to be resolved one word at a time, and a lone word lands
//! on whatever symbol happens to share it. Here the question is read as a
//! whole against four fields of every file: its *path* (the author's own
//! one-line summary), the *names it defines*, its *comments* (the prose
//! written about the code, closest to how a question is phrased) and its
//! *body*. Words are stemmed (Snowball English), so `resized` meets
//! `resize` and `images` meets `image`.
//!
//! The index is derived from the graph (`word_index`, `comment_index`,
//! `file_to_nodes`) and memoized against the mesh revision; a query is a
//! pass over the postings of its own terms.

use crate::intern::GraphData;
use neuromesh_core::{NodeId, NodeType};
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::{HashMap, HashSet};

/// Field weights: the path is the author's own one-line summary of a file,
/// defined names the next best, the body (every word it spells) the weakest.
const W_PATH: f32 = 3.0;
const W_SYMBOL: f32 = 2.0;
const W_BODY: f32 = 1.0;
const W_COMMENT: f32 = 2.0;
const B_COMMENT: f32 = 0.75;
const K1: f32 = 1.2;
const B_BODY: f32 = 0.75;
const B_SYMBOL: f32 = 0.5;

/// English function words and question scaffolding. Code-generic words
/// (`file`, `code`, `data`) are left to idf.
const STOPWORDS: &[&str] = &[
    "a",
    "about",
    "after",
    "all",
    "also",
    "an",
    "and",
    "any",
    "are",
    "as",
    "at",
    "be",
    "been",
    "before",
    "being",
    "between",
    "both",
    "but",
    "by",
    "can",
    "could",
    "did",
    "do",
    "does",
    "doing",
    "done",
    "each",
    "for",
    "from",
    "get",
    "gets",
    "got",
    "had",
    "has",
    "have",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "it",
    "its",
    "just",
    "like",
    "many",
    "more",
    "most",
    "much",
    "my",
    "no",
    "not",
    "of",
    "on",
    "once",
    "only",
    "or",
    "other",
    "our",
    "out",
    "over",
    "own",
    "same",
    "should",
    "since",
    "so",
    "some",
    "such",
    "than",
    "that",
    "the",
    "their",
    "them",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "too",
    "under",
    "until",
    "up",
    "us",
    "use",
    "used",
    "uses",
    "using",
    "very",
    "via",
    "was",
    "way",
    "we",
    "were",
    "what",
    "when",
    "where",
    "whether",
    "which",
    "while",
    "who",
    "why",
    "will",
    "with",
    "within",
    "without",
    "would",
    "you",
    "your",
    "there",
    "here",
    "happens",
    "happen",
    "work",
    "works",
    "system",
    "tool",
    "program",
    "codebase",
    "logic",
    "part",
    "am",
    "go",
    "he",
    "me",
    "piece",
    "thing",
    "things",
    "somewhere",
    "someone",
    "handled",
    "handle",
    "handles",
    "implemented",
    "implement",
    "implementation",
    "decide",
    "decides",
    "determine",
    "determines",
    "make",
    "makes",
    "made",
    "turn",
    "turned",
    "turns",
    "time",
    "again",
];

/// One ranked file: its node id, relative path, score and how many distinct
/// query terms it matched in any field.
#[derive(Debug, Clone)]
pub struct RankedFile {
    pub id: NodeId,
    pub path: std::path::PathBuf,
    pub score: f32,
    pub matched: usize,
}

#[derive(Default)]
pub(crate) struct FileRankIndex {
    ids: Vec<NodeId>,
    paths: Vec<std::path::PathBuf>,
    /// term → docs whose path spells it (tf 1).
    path: HashMap<String, Vec<u32>>,
    /// term → (doc, count of defined names spelling it).
    symbol: HashMap<String, Vec<(u32, u16)>>,
    /// term → docs whose body spells it (binary, as `word_index` is).
    body: HashMap<String, Vec<u32>>,
    /// term → docs whose comments spell it (binary).
    comment: HashMap<String, Vec<u32>>,
    comment_len: Vec<u32>,
    avg_comment_len: f32,
    symbol_len: Vec<u32>,
    body_len: Vec<u32>,
    avg_symbol_len: f32,
    avg_body_len: f32,
}

fn stemmer() -> Stemmer {
    Stemmer::create(Algorithm::English)
}

/// Lowercase, split identifiers into words, drop short/non-alphabetic
/// pieces and stopwords, stem.
fn terms_of(text: &str, st: &Stemmer, keep_stopwords: bool) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        if raw.is_empty() {
            continue;
        }
        for part in neuromesh_parser::tokenize_ident(raw) {
            let w = part.to_lowercase();
            if w.len() < 2 || !w.chars().all(|c| c.is_ascii_alphabetic()) {
                continue;
            }
            if !keep_stopwords && STOPWORDS.contains(&w.as_str()) {
                continue;
            }
            out.push(st.stem(&w).into_owned());
        }
    }
    out
}

/// Query terms, stemmed and deduplicated in prompt order.
pub fn query_terms(prompt: &str) -> Vec<String> {
    weighted_query_terms(prompt)
        .into_iter()
        .filter(|(_, w)| *w >= 1.0)
        .map(|(t, _)| t)
        .collect()
}

/// Weight of a word the question did not say but a thesaurus cluster
/// offers for one it did.
const SYNONYM_WEIGHT: f32 = 0.35;

/// Question terms (weight 1) plus thesaurus stand-ins (weight
/// [`SYNONYM_WEIGHT`]): a question word in a cluster also searches the
/// rest of its cluster.
pub fn weighted_query_terms(prompt: &str) -> Vec<(String, f32)> {
    let st = stemmer();
    let mut out: Vec<(String, f32)> = Vec::new();
    let mut seen = HashSet::new();
    for t in terms_of(prompt, &st, false) {
        if seen.insert(t.clone()) {
            out.push((t, 1.0));
        }
    }
    let lower = prompt.to_lowercase();
    let mut extra: Vec<String> = Vec::new();
    if lower.contains("how many") || lower.contains("number of") {
        extra.push("count".into());
    }
    let words: Vec<String> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    for cluster in clusters() {
        let stems: Vec<String> = cluster.iter().map(|w| st.stem(w).into_owned()).collect();
        let hit = words
            .iter()
            .any(|w| cluster.contains(&w.as_str()) || stems.contains(&st.stem(w).into_owned()));
        if hit {
            extra.extend(cluster.iter().map(|w| w.to_string()));
        }
    }
    for w in extra {
        let t = st.stem(&w).into_owned();
        if seen.insert(t.clone()) {
            out.push((t, SYNONYM_WEIGHT));
        }
    }
    out
}

/// How people say it vs how code spells it: one cluster of interchangeable
/// words per line of `thesaurus.txt` (general software vocabulary, not
/// written for any one repository). Kept out of the source so a repository
/// that indexes this crate does not see every synonym in one file.
fn clusters() -> &'static [Vec<&'static str>] {
    static CLUSTERS: std::sync::OnceLock<Vec<Vec<&'static str>>> = std::sync::OnceLock::new();
    CLUSTERS.get_or_init(|| {
        include_str!("thesaurus.txt")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| {
                l.split(',')
                    .map(str::trim)
                    .filter(|w| !w.is_empty())
                    .collect()
            })
            .collect()
    })
}

impl FileRankIndex {
    pub(crate) fn build(data: &GraphData) -> Self {
        let st = stemmer();
        let mut idx = FileRankIndex::default();
        let mut doc_of: HashMap<NodeId, u32> = HashMap::new();
        for (path, node_ids) in &data.file_to_nodes {
            let Some(file_id) = node_ids.iter().find(|id| {
                data.mesh
                    .node(id)
                    .is_some_and(|n| n.node_type == NodeType::File)
            }) else {
                continue;
            };
            let doc = idx.ids.len() as u32;
            idx.ids.push(file_id.clone());
            idx.paths.push(path.clone());
            doc_of.insert(file_id.clone(), doc);

            let rel = path.to_string_lossy().replace('\\', "/");
            // Path words: every directory and the stem, not the extension.
            let without_ext = match rel.rsplit_once('.') {
                Some((head, ext)) if !ext.contains('/') => head.to_string(),
                _ => rel.clone(),
            };
            let mut seen = HashSet::new();
            for t in terms_of(&without_ext, &st, true) {
                if seen.insert(t.clone()) {
                    idx.path.entry(t).or_default().push(doc);
                }
            }

            let mut counts: HashMap<String, u16> = HashMap::new();
            let mut len = 0u32;
            for id in node_ids {
                let Some(node) = data.mesh.node(id) else {
                    continue;
                };
                if node.node_type == NodeType::File {
                    continue;
                }
                for t in terms_of(&node.name, &st, true) {
                    len += 1;
                    let c = counts.entry(t).or_insert(0);
                    *c = c.saturating_add(1);
                }
            }
            for (t, c) in counts {
                idx.symbol.entry(t).or_default().push((doc, c));
            }
            idx.symbol_len.push(len);
            idx.body_len
                .push(data.body_lengths.get(file_id).copied().unwrap_or(0));
        }
        for (word, ids) in &data.word_index {
            let t = st.stem(word).into_owned();
            let list = idx.body.entry(t).or_default();
            for id in ids {
                if let Some(&doc) = doc_of.get(id) {
                    list.push(doc);
                }
            }
        }
        for list in idx.body.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        idx.comment_len = vec![0; idx.ids.len()];
        for (word, ids) in &data.comment_index {
            let t = st.stem(word).into_owned();
            let list = idx.comment.entry(t).or_default();
            for id in ids {
                if let Some(&doc) = doc_of.get(id) {
                    list.push(doc);
                    idx.comment_len[doc as usize] += 1;
                }
            }
        }
        for list in idx.comment.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        let n = idx.ids.len().max(1) as f32;
        idx.avg_comment_len = (idx.comment_len.iter().map(|&l| l as f32).sum::<f32>() / n).max(1.0);
        idx.avg_symbol_len = (idx.symbol_len.iter().map(|&l| l as f32).sum::<f32>() / n).max(1.0);
        idx.avg_body_len = (idx.body_len.iter().map(|&l| l as f32).sum::<f32>() / n).max(1.0);
        idx
    }

    /// Files ranked by BM25F over `terms` (already stemmed), best first.
    pub(crate) fn rank(&self, terms: &[(String, f32)], limit: usize) -> Vec<RankedFile> {
        let n = self.ids.len() as f32;
        if n == 0.0 {
            return Vec::new();
        }
        // term → doc → weighted, length-normalised tf
        let mut scores: HashMap<u32, (f32, usize)> = HashMap::new();
        for (term, weight) in terms {
            let mut tf: HashMap<u32, f32> = HashMap::new();
            if let Some(docs) = self.path.get(term) {
                for &d in docs {
                    *tf.entry(d).or_insert(0.0) += W_PATH;
                }
            }
            if let Some(docs) = self.symbol.get(term) {
                for &(d, c) in docs {
                    let len = self.symbol_len[d as usize] as f32;
                    let norm = 1.0 - B_SYMBOL + B_SYMBOL * len / self.avg_symbol_len;
                    *tf.entry(d).or_insert(0.0) += W_SYMBOL * c as f32 / norm;
                }
            }
            if let Some(docs) = self.body.get(term) {
                for &d in docs {
                    let len = self.body_len[d as usize] as f32;
                    let norm = 1.0 - B_BODY + B_BODY * len / self.avg_body_len;
                    *tf.entry(d).or_insert(0.0) += W_BODY / norm;
                }
            }
            if let Some(docs) = self.comment.get(term) {
                for &d in docs {
                    let len = self.comment_len[d as usize] as f32;
                    let norm = 1.0 - B_COMMENT + B_COMMENT * len / self.avg_comment_len;
                    *tf.entry(d).or_insert(0.0) += W_COMMENT / norm;
                }
            }
            if tf.is_empty() {
                continue;
            }
            let df = tf.len() as f32;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            for (d, t) in tf {
                let e = scores.entry(d).or_insert((0.0, 0));
                e.0 += weight * idf * t / (K1 + t);
                if *weight >= 1.0 {
                    e.1 += 1;
                }
            }
        }
        let mut ranked: Vec<RankedFile> = scores
            .into_iter()
            .map(|(d, (score, matched))| RankedFile {
                id: self.ids[d as usize].clone(),
                path: self.paths[d as usize].clone(),
                score,
                matched,
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.path.cmp(&b.path))
        });
        ranked.truncate(limit);
        ranked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_terms_stem_and_drop_scaffolding() {
        let t = query_terms("Where are the uploaded images resized?");
        assert_eq!(t, vec!["upload", "imag", "resiz"]);
    }
}
