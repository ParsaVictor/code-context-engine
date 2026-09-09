# Attribution & Provenance

This project is a **combination of several existing ideas and codebases**, brought
together deliberately. This file tracks where each piece comes from and under
what license, so provenance is always clear.

## Base

| Component | Source | License | How it is used |
|---|---|---|---|
| Entire Rust codebase baseline | [pinoox/neuromesh](https://github.com/pinoox/neuromesh) @ v0.9.0 (`baseline-v0.9.0`) | MIT (© 2026 yoosef alipour) | Starting point. All 17 crates, MCP server, folding, tiered retrieval, tree-sitter integration. |

We track `upstream/main` as a git remote and periodically merge upstream fixes.
Where our P0 (project isolation) work is generally useful, we intend to offer it
back to upstream as pull requests.

## Planned / incorporated third-party components

Status legend: ⬜ planned · 🔨 in progress · ✅ integrated

| Component | Source | License | Role | Status |
|---|---|---|---|---|
| tree-sitter + grammars | [tree-sitter](https://github.com/tree-sitter/tree-sitter) | MIT | Syntax parsing (already in baseline; extend grammar coverage) | ✅ (baseline) |
| tree-sitter-stack-graphs | [github/stack-graphs](https://github.com/github/stack-graphs) | MIT / Apache-2.0 | Precise, incremental name resolution | ⬜ |
| SCIP format + `scip` crate | [sourcegraph/scip](https://github.com/sourcegraph/scip) | Apache-2.0 | Ingest precise indexes (rust-analyzer, scip-python, scip-typescript, scip-clang) | ⬜ |
| RepoMap ranking algorithm (personalized PageRank + token-budget binary search) | [Aider](https://github.com/Aider-AI/aider) `aider/repomap.py` | Apache-2.0 | Symbol seed ranking (algorithm reimplemented in Rust, not copied) | ⬜ |
| tantivy | [quickwit-oss/tantivy](https://github.com/quickwit-oss/tantivy) | MIT | BM25 lexical retrieval index | ⬜ |
| fastembed-rs | [Anush008/fastembed-rs](https://github.com/Anush008/fastembed-rs) | Apache-2.0 | Local embeddings (already in baseline) | ✅ (baseline) |
| redb or SQLite (rusqlite, bundled) | [cberner/redb](https://github.com/cberner/redb) / [SQLite](https://sqlite.org) | MIT / Public Domain | Per-project persistent store + central registry | ⬜ |
| LLMLingua-2 | [microsoft/LLMLingua](https://github.com/microsoft/LLMLingua) | MIT | Optional post-fold prompt compression (ONNX sidecar) | ⬜ |
| nbformat schema / jupytext approach | [jupyter/nbformat](https://github.com/jupyter/nbformat) / [mwouts/jupytext](https://github.com/mwouts/jupytext) | BSD / MIT | `.ipynb` normalization to ordered parseable cells | ⬜ |
| GraphRAG community-detection pattern | [microsoft/graphrag](https://github.com/microsoft/graphrag) | MIT | "Knowledge layer": module summaries, community detection (pattern only) | ⬜ |

## Research informing the design

- *Lost in the Middle* (Liu et al. 2023) — packet ordering: seeds first & last.
- Chroma *Context Rot* (2025) — "less but precise" as a design principle.
- *When Retrieval Hurts Code Completion* (2026) — invalidate stale sidecar/folds.
- *CodeRAG-Bench* (NAACL 2025) — scenario-based evaluation.
- *Stack Graphs* (Creager et al.) — name resolution at scale.
- CodeSage — precedent for per-project local index.

## License compliance notes

- Upstream `LICENSE` (MIT, © yoosef alipour) is retained verbatim.
- Any file substantially derived from an Apache-2.0 source will carry the
  required notice; algorithm reimplementations from Apache-2.0 sources are noted
  here and in the relevant source file's module doc comment.
- No code is copied from GPL/AGPL sources.
