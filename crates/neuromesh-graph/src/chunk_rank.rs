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
}

/// A file's chunks as term counts, before they are numbered.
type FileChunks = (NodeId, PathBuf, Vec<HashMap<String, u16>>);

#[derive(Default)]
pub(crate) struct ChunkRankIndex {
    files: Vec<(NodeId, PathBuf)>,
    file_of: Vec<u32>,
    len: Vec<u32>,
    avg_len: f32,
    post: HashMap<String, Vec<(u32, u16)>>,
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
                let head = 1..HEAD_LINES.min(lines.len());
                let mut chunks = Vec::new();
                for span in std::iter::once(head).chain(f.spans.iter().cloned()) {
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
                        chunks.push(counts);
                    }
                }
                (f.id, f.path, chunks)
            })
            .collect();
        let mut idx = ChunkRankIndex::default();
        for (id, path, chunks) in per_file {
            let file = idx.files.len() as u32;
            idx.files.push((id, path));
            for counts in chunks {
                let chunk = idx.file_of.len() as u32;
                idx.file_of.push(file);
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

    /// Files by their best chunk for `prompt`, best first.
    pub(crate) fn rank(&self, prompt: &str, limit: usize) -> Vec<RankedFile> {
        let n = self.len.len() as f32;
        if n == 0.0 {
            return Vec::new();
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
        let mut score: HashMap<u32, f32> = HashMap::new();
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
        let mut best: HashMap<u32, f32> = HashMap::new();
        for (chunk, s) in score {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(id: &str, path: &str, source: &str, spans: Vec<std::ops::Range<usize>>) -> ChunkSource {
        ChunkSource {
            id: NodeId::new(id),
            path: PathBuf::from(path),
            source: source.to_string(),
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
