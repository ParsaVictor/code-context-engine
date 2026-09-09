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

## P0 — Isolation first (stop the bug) — **done**

- [x] Deterministic `ProjectId`, derived from the canonical project root (sha256, matching the managed-store slot naming — nothing written into the user's repo)
- [ ] Central `registry.sqlite` (id ↔ path ↔ slot ↔ lang profile) — deferred; not needed to close the bug, it is P1 infrastructure
- [x] Single-project invariant enforced on the live graph (`assert_single_project` / `evict_foreign_nodes`), rather than namespacing every derived index key
- [x] `graph.clear()` before every hot-swap; the `same_workspace_path` early-return now also requires the same `project_id`; workspace root locked once per index
- [ ] Monorepo / multi-root detection + `sub_projects` config — one repo is still one project
- [x] **Leakage gate in CI** — two projects sharing a byte-identical file, assert zero cross-project files in the packet
- [x] Ignore rules scoped to the project root, and a guessed workspace with no project marker is refused
- [ ] (optional) `LruCache<ProjectId, ProjectContext>` for concurrent multi-project serving

**Definition of done — met:** open project A (Rust web) then project B (Python CV) in one
MCP process; every B query returns zero A files; CI proves it.
Offered upstream as [pinoox/neuromesh#34](https://github.com/pinoox/neuromesh/pull/34).

## P1 — Universal Artifact Graph (grow the knowledge)

- [x] Add generic + ML `NodeType` / `EdgeType` (Artifact IR)
- [ ] tree-sitter grammars: C, C++, R, Julia, Scala, Lua, Bash, TOML
- [ ] `tree-sitter-stack-graphs` name resolution (Python / JS / TS first)
- [ ] SCIP index ingestion when present
- [ ] `.ipynb` parser (ordered cells + cross-cell DEF-USE)
- [ ] Config→Code layer (YAML / Hydra / argparse → `Hyperparameter` / `Parameterizes`)
- [x] **ML framework overlays — PyTorch object detection first** (`nn.Module`, `forward`, `DataLoader`, train loop, checkpoint, mAP metric)
- [ ] `tantivy` BM25 replaces hand-rolled lexical retrieval
- [ ] Reranker + query-conditioned pruning
- [ ] Gold dataset: 1 web + 1 NLP + 1 CV project; extend `neuromesh eval`

**Killer demo — the walk exists,** gated in CI by
`crates/neuromesh-context/tests/ml_artifact_graph.rs` over artifact edges only:

```
val/mAP <-Produces- evaluate -Evaluates-> Detector <-CheckpointOf- runs/last.pt
        <-Produces- main -Consumes-> CocoDetection <-Transforms- build_transforms
```

Still to measure: the token saving against opening the repo, once the gold
dataset and `neuromesh eval` cover a CV project.

## P2 — Deeper compression & knowledge

- [ ] Optional LLMLingua-2 post-fold compression
- [ ] Knowledge layer (community detection + module summaries, GraphRAG-style)
- [ ] Federated multi-project retrieval (explicit scope)
- [ ] Independent benchmark on SWE-bench Verified subset; publish methodology

## Relationship to upstream

We keep `upstream/main` as a remote and merge fixes. P0 work is designed to be
offerable as clean PRs to `pinoox/neuromesh`. The ML / universal layer (P1+) is
this project's own direction.
