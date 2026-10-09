//! Definition-level BM25 for long reports (issues, bug templates).
//!
//! A whole-file ranker dilutes a report's words over every definition of a
//! big module; read definition by definition, the function the report is
//! about scores on its own (LocAgent indexes entity contents the same way).
//! Chunks are the file head (first 40 lines) and every definition the graph
//! recorded with a line span, each carrying its file's path words; a file
//! scores its best chunk. The report's first line — an issue's title — counts
//! three times.
//!
//! SWE-bench dev (`dev-fast`, 57 issues): fused by reciprocal rank with the
//! packet's localisation list, Acc@1 0.211 → 0.298, @5 0.544 → 0.614, @10
//! 0.667 → 0.702 (prototype, `scripts/research/lex_variants.py`).
//!
//! Built lazily from the sources on disk on the first long report, cached
//! with the file ranker's key.

use crate::file_rank::{stemmer, terms_of, RankedFile};
use neuromesh_core::NodeId;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

const K1: f32 = 1.2;
const B: f32 = 0.75;
const HEAD_LINES: usize = 40;
const MAX_CHUNK_LINES: usize = 400;
const TITLE_EXTRA: f32 = 2.0;

/// One file as the index needs it: id, path, source, definition spans
/// (1-based lines, end inclusive).
pub(crate) struct ChunkSource {
    pub id: NodeId,
    pub path: PathBuf,
    pub source: String,
    pub spans: Vec<std::ops::Range<usize>>,
    /// The definition each span is (`Class.method`), same order as `spans`.
    pub names: Vec<String>,
    /// Whether each span holds other definitions (a class): it still votes
    /// for its file, but a function-level list names what is inside it.
    pub containers: Vec<bool>,
}

/// One definition ranked for a report: where it is and how well it matched.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RankedDefinition {
    pub path: String,
    pub name: String,
    pub lines: (usize, usize),
    pub score: f32,
}

/// A file's chunks as term counts, before they are numbered.
type FileChunks = (NodeId, PathBuf, Vec<(HashMap<String, u16>, Label)>);

/// A chunk's definition name, first and last line, and whether it is a container.
type Label = (String, usize, usize, bool);

#[derive(Default)]
pub(crate) struct ChunkRankIndex {
    files: Vec<(NodeId, PathBuf)>,
    file_of: Vec<u32>,
    len: Vec<u32>,
    avg_len: f32,
    post: HashMap<String, Vec<(u32, u16)>>,
    /// Per chunk: definition name (`<head>` for the file head) and lines.
    label: Vec<Label>,
}

impl ChunkRankIndex {
    pub(crate) fn build(sources: Vec<ChunkSource>) -> Self {
        // Per file: its chunks' term counts, computed in parallel.
        let per_file: Vec<FileChunks> = sources
            .into_par_iter()
            .map(|f| {
                let st = stemmer();
                let rel = f.path.to_string_lossy().replace('\\', "/");
                let stem_path = match rel.rsplit_once('.') {
                    Some((head, ext)) if !ext.contains('/') => head.to_string(),
                    _ => rel.clone(),
                };
                let path_terms = terms_of(&stem_path, &st, true);
                let lines: Vec<&str> = f.source.lines().collect();
                let head = (1..HEAD_LINES.min(lines.len()), ("<head>".to_string(), true));
                let mut chunks = Vec::new();
                let named = f.spans.iter().cloned().zip(
                    f.names
                        .iter()
                        .cloned()
                        .zip(f.containers.iter().copied().chain(std::iter::repeat(false))),
                );
                for (span, (name, container)) in std::iter::once(head).chain(named) {
                    let start = span.start.max(1) - 1;
                    let end = span.end.min(lines.len()).min(start + MAX_CHUNK_LINES);
                    if start >= end {
                        continue;
                    }
                    let mut counts: HashMap<String, u16> = HashMap::new();
                    for t in terms_of(&lines[start..end].join("\n"), &st, false)
                        .into_iter()
                        .chain(path_terms.iter().cloned())
                    {
                        let c = counts.entry(t).or_insert(0);
                        *c = c.saturating_add(1);
                    }
                    if !counts.is_empty() {
                        chunks.push((counts, (name, start + 1, end, container)));
                    }
                }
                (f.id, f.path, chunks)
            })
            .collect();
        let mut idx = ChunkRankIndex::default();
        for (id, path, chunks) in per_file {
            let file = idx.files.len() as u32;
            idx.files.push((id, path));
            for (counts, label) in chunks {
                let chunk = idx.file_of.len() as u32;
                idx.file_of.push(file);
                idx.label.push(label);
                idx.len.push(counts.values().map(|&c| c as u32).sum());
                for (t, c) in counts {
                    idx.post.entry(t).or_default().push((chunk, c));
                }
            }
        }
        let n = idx.len.len().max(1) as f32;
        idx.avg_len = (idx.len.iter().map(|&l| l as f32).sum::<f32>() / n).max(1.0);
        idx
    }

    /// Functions and methods by score for `prompt`, best first. File heads and
    /// containers (a class) are left out: on SWE-bench dev a class body outranked
    /// the method the patch edits (func Acc@5 0.268 -> 0.311 without them).
    pub(crate) fn rank_definitions(&self, prompt: &str, limit: usize) -> Vec<RankedDefinition> {
        let mut scored: Vec<(u32, f32)> = self
            .scores(prompt)
            .into_iter()
            .filter(|(c, _)| !self.label[*c as usize].3)
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored
            .into_iter()
            .take(limit)
            .map(|(c, score)| {
                let (name, start, end, _) = &self.label[c as usize];
                RankedDefinition {
                    path: self.files[self.file_of[c as usize] as usize]
                        .1
                        .to_string_lossy()
                        .replace('\\', "/"),
                    name: name.clone(),
                    lines: (*start, *end),
                    score,
                }
            })
            .collect()
    }

    /// Files by their best chunk for `prompt`, best first.
    pub(crate) fn rank(&self, prompt: &str, limit: usize) -> Vec<RankedFile> {
        let mut best: HashMap<u32, f32> = HashMap::new();
        for (chunk, s) in self.scores(prompt) {
            let f = self.file_of[chunk as usize];
            let e = best.entry(f).or_insert(0.0);
            if s > *e {
                *e = s;
            }
        }
        let mut ranked: Vec<RankedFile> = best
            .into_iter()
            .map(|(f, s)| {
                let (id, path) = &self.files[f as usize];
                RankedFile {
                    id: id.clone(),
                    path: path.clone(),
                    score: s,
                    matched: 0,
                }
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

    /// BM25 score of every chunk sharing a term with `prompt` (boilerplate
    /// stripped, the title counted [`TITLE_EXTRA`] more times).
    fn scores(&self, prompt: &str) -> HashMap<u32, f32> {
        let n = self.len.len() as f32;
        let mut score: HashMap<u32, f32> = HashMap::new();
        if n == 0.0 {
            return score;
        }
        let st = stemmer();
        let body = neuromesh_parser::strip_issue_boilerplate(prompt);
        let title = body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("");
        let mut q: HashMap<String, f32> = HashMap::new();
        for t in terms_of(&body, &st, false) {
            q.insert(t, 1.0);
        }
        for t in terms_of(title, &st, false) {
            *q.entry(t).or_insert(0.0) += TITLE_EXTRA;
        }
        let mut terms: Vec<(String, f32)> = q.into_iter().collect();
        terms.sort_by(|a, b| a.0.cmp(&b.0));
        terms.truncate(256);
        for (t, w) in &terms {
            let Some(list) = self.post.get(t) else {
                continue;
            };
            let df = list.len() as f32;
            let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
            for &(chunk, tf) in list {
                let tf = tf as f32;
                let norm = 1.0 - B + B * self.len[chunk as usize] as f32 / self.avg_len;
                *score.entry(chunk).or_insert(0.0) += w * idf * tf * (K1 + 1.0) / (tf + K1 * norm);
            }
        }
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(id: &str, path: &str, source: &str, spans: Vec<std::ops::Range<usize>>) -> ChunkSource {
        ChunkSource {
            id: NodeId::new(id),
            path: PathBuf::from(path),
            source: source.to_string(),
            names: spans.iter().map(|s| format!("def_{}", s.start)).collect(),
            containers: vec![false; spans.len()],
            spans,
        }
    }

    #[test]
    fn a_report_reaches_the_definition_it_describes_inside_a_big_file() {
        // `big.py` spells the report's words once each across unrelated
        // definitions; `small.py` has one definition that is all about them.
        let mut big = String::new();
        for i in 0..60 {
            big.push_str(&format!("def helper_{i}():\n    return {i}\n"));
        }
        big.push_str("def parse_header():\n    timezone = 0\n");
        let small = "def normalize_timezone_offset(header):\n    \
                     # parse the timezone offset of a header\n    \
                     return header.timezone.offset\n";
        let index = ChunkRankIndex::build(vec![
            src(
                "big",
                "pkg/big.py",
                &big,
                vec![std::ops::Range {
                    start: 121,
                    end: 122,
                }],
            ),
            src(
                "small",
                "pkg/dates.py",
                small,
                vec![std::ops::Range { start: 1, end: 3 }],
            ),
        ]);
        let ranked = index.rank("Timezone offset in header is parsed wrong", 5);
        assert_eq!(ranked[0].path, PathBuf::from("pkg/dates.py"));
    }
}
