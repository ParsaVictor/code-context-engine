# Roadmap

> Full design docs (Persian): [`docs/planning/`](docs/planning/)
> مستندات کامل طراحی و تحلیل به فارسی در پوشه‌ی `docs/planning/`.

## Why this fork exists

The baseline (NeuroMesh v0.9.0) is a strong local-first MCP context engine, but two
gaps block it from being a *general* project context engine:

1. **Project isolation is coarse.** One live in-RAM graph per MCP process; a
   mis-detected workspace or an un-indexed second project can contaminate the
   graph. `NodeId`/`EdgeId` carry no project namespace, and queries do not filter
   by `project_id`.
2. **It is web-centric.** No `.ipynb` parsing, no config→code layer, no ML node
   types (Dataset / Model / Checkpoint / Experiment / Metric), no ML framework
   overlays. It cannot properly reason about a PyTorch / HuggingFace codebase.

Goal: **Maximum task success per token**, with **cross-project leakage = 0**, on
**web + ML** codebases alike.

## P0 — Isolation first (stop the bug)

- [ ] Deterministic `ProjectId = blake3(canonical git root)`, persisted in `<ws>/.neuromesh/id`
- [ ] Central `registry.sqlite` (id ↔ path ↔ slot ↔ lang profile)
- [ ] Namespace graph keys by `(ProjectId, NodeId)` across all derived indexes
- [ ] Mandatory `project_id` filter in `search` / `trace` / `spreading_activation` + `debug_assert` no cross-scope nodes in a `ContextView`
- [ ] `graph.clear()` before every hot-swap; drop the unsafe `same_workspace_path` early-return when `project_id` differs; lock workspace root once per index
- [ ] Monorepo / multi-root detection + `sub_projects` config
- [ ] **Leakage benchmark in CI** — 4 scenarios, assert `cross_project_files == 0`
- [ ] (optional) `LruCache<ProjectId, ProjectContext>` for concurrent multi-project serving

**Definition of done:** open project A (Rust web) then project B (Python CV) in one
MCP process; every B query returns zero A files; CI proves it.

## P1 — Universal Artifact Graph (grow the knowledge)

- [ ] Add generic + ML `NodeType` / `EdgeType` (Artifact IR)
- [ ] tree-sitter grammars: C, C++, R, Julia, Scala, Lua, Bash, TOML
- [ ] `tree-sitter-stack-graphs` name resolution (Python / JS / TS first)
- [ ] SCIP index ingestion when present
- [ ] `.ipynb` parser (ordered cells + cross-cell DEF-USE)
- [ ] Config→Code layer (YAML / Hydra / argparse → `Hyperparameter` / `Parameterizes`)
- [ ] **ML framework overlays — PyTorch object detection first** (`nn.Module`, `forward`, `DataLoader`, train loop, checkpoint, mAP metric)
- [ ] `tantivy` BM25 replaces hand-rolled lexical retrieval
- [ ] Reranker + query-conditioned pruning
- [ ] Gold dataset: 1 web + 1 NLP + 1 CV project; extend `neuromesh eval`

**Killer demo:** *"Why did the model's mAP drop?"* → the engine walks
`Metric(mAP) → EvalLoop → Model → Checkpoint → Transform → Dataset`, ~95% fewer
tokens than opening the repo.

## P2 — Deeper compression & knowledge

- [ ] Optional LLMLingua-2 post-fold compression
- [ ] Knowledge layer (community detection + module summaries, GraphRAG-style)
- [ ] Federated multi-project retrieval (explicit scope)
- [ ] Independent benchmark on SWE-bench Verified subset; publish methodology

## Relationship to upstream

We keep `upstream/main` as a remote and merge fixes. P0 work is designed to be
offerable as clean PRs to `pinoox/neuromesh`. The ML / universal layer (P1+) is
this project's own direction.
