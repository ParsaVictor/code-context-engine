<!-- code-context-engine — an independent derivative of NeuroMesh v0.9.0 (MIT, © 2026 yoosef alipour).
     Upstream: https://github.com/pinoox/neuromesh · baseline tag: baseline-v0.9.0 · see NOTICE and ATTRIBUTION.md -->

<div align="center">

# code-context-engine

**Ship the right code, not the whole repo.**
A local-first MCP context engine for AI coding agents: a per-project code graph, seed resolution from the task as written, and a folded evidence packet — measured on repositories it was never tuned on.

![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square&logo=rust)
![CI](https://github.com/ParsaVictor/code-context-engine/actions/workflows/ci.yml/badge.svg)
![MCP](https://img.shields.io/badge/MCP-stdio-5b21b6.svg?style=flat-square)
![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)

Cursor · Claude Code · Codex · VS Code · Zed · any MCP client

[Why](#why-this-fork) · [Measured](#what-is-measured) · [Install](#install) · [How it works](#how-it-works) · [Benchmarks](#run-the-benchmarks) · [Roadmap](ROADMAP.md) · [Docs](docs/README.md)

</div>

---

## Why this fork

[NeuroMesh](https://github.com/pinoox/neuromesh) is a strong local-first context engine for web codebases. This repository continues it with two goals that the baseline did not meet:

| Goal | Baseline v0.9.0 | This project |
|---|---|---|
| **Project isolation** — zero cross-project files in a packet | one live graph per process; a mis-detected workspace could contaminate it | deterministic `ProjectId`, single-project invariant on the live graph, leakage gate in CI ([offered upstream](https://github.com/pinoox/neuromesh/pull/34)) |
| **Universal codebases** — not only web | no notebooks, no config→code layer, no ML node types | `.ipynb`, YAML/JSON/argparse/Hydra config→code edges, ML artifact graph (Dataset · Model · Checkpoint · Metric · TrainLoop), shell scripts, string-literal index (routes, tool names, env vars) |
| **Honest numbers** | one-repo demo table | gold sets on 10 third-party repositories, four of them never tuned on; a model-executed task-success benchmark with a separate judge |

Everything the engine claims is in [docs/measured.md](docs/measured.md), with what is *not* measured listed next to it.

---

## What is measured

Gold sets are written from reading the source **before** the engine runs. Recall = the gold files the packet contains; precision = share of packet files that are gold; forbidden = a file the task must not ship.

**Repositories the engine was never tuned on** (2026-09-21, `main`):

| Holdout | Languages / stack | Recall | Precision | Forbidden |
|---|---|---:|---:|---:|
| gin + torchvision | Go, Python | 1.000 | 0.554 | 0 |
| libuv + fmt | C, C++ | 1.000 | 0.573 | 0 |
| os-lib + cli + Flux.jl | Scala, R, Julia | 1.000 | 0.541 | 1 |
| peft + keras-hub | Hugging Face / Keras 3 libraries | 1.000 | 0.632 | 0 |

**Sets that have been tuned on** (dev-class): nanoGPT / express / vit-pytorch / fastapi-template 1.000 / 0.938 · django + ultralytics 1.000 / 0.666 · Hydra + argparse configs 1.000 / 0.712 · Fastify + Next.js app router 0.642 / 0.602 · this repository (20 real questions) 0.675 / 0.406.

**Task success with a real model** (DeepSeek-V4-Pro answering, GLM-5.3 judging against the real source, real `verify` for patch tasks):

| Set | packet | whole gold files | naive grep |
|---|---:|---:|---:|
| fixtures (26 tasks) | 0.808 · 4.2k tokens | 0.808 · 8.7k tokens | 0.577 |
| gin holdout (10) | **1.000** · 8.1k | 1.000 · 11.1k | 0.800 |
| torchvision holdout (10) | **1.000** · 7.8k | 0.800 · 8.7k | 0.600 |

The packet matches or beats "just open the right files" at 1.4–2.1× the success per token, and beats grep everywhere. Recall is the strength; precision (1–3 neighbour files too many) is the open front — see [ROADMAP.md](ROADMAP.md).

---

## Install

Pre-built binaries come from upstream's installer; this fork is built from source until its first release.

```bash
git clone https://github.com/ParsaVictor/code-context-engine
cd code-context-engine
CARGO_BUILD_JOBS=2 cargo build --release -p neuromesh-cli      # target/release/neuromesh
```

Then, from the root of the project you want to index:

```bash
neuromesh doctor                          # binary + workspace check
neuromesh connect --global --agent-rules  # write MCP config + agent rule (Cursor, Claude, VS Code, …)
neuromesh index                           # build the graph (seconds on most repos)
```

Manual MCP config (any client):

```json
{ "mcpServers": { "neuromesh": { "command": "neuromesh", "args": ["mcp"] } } }
```

Per-client paths, engines and options: [docs/mcp.md](docs/mcp.md) · [docs/configuration.md](docs/configuration.md).

---

## How it works

```
task as written ──► seed resolution ──► graph activation ──► selection ──► folded packet
                    identifiers, files,   Calls / Imports /    required +   signatures kept,
                    config keys, string   Parameterizes /      callees +    bodies folded,
                    literals, path words  twins, pheromones    ranked fill  budget-capped
```

1. **Index** — tree-sitter (and light parsers for YAML/JSON/shell/notebooks) turns every file into nodes and edges; string literals that look like names (`"/api/users"`, `"neuromesh_record_feedback"`, `DATABASE_URL`) are indexed too.
2. **Seed** — the prompt's code names resolve to nodes; twins with the same name are settled by directory and body words; guesses that land in docs/tests/fixtures are dropped.
3. **Activate & select** — energy spreads along typed edges; seeds and the callees the prompt names get required seats; the rest is a ranked, budgeted fill.
4. **Fold** — bodies outside the question are folded to signatures; the agent can expand a fold on demand.
5. **Learn** — `record_feedback` reinforces the path a successful task walked, never a whole neighbourhood.

The MCP agent loop (`get_context_packet` → `search_symbols` / `expand_gap` → `expand_fold` → `trace` → `record_feedback`) is documented in [docs/agent-guide.md](docs/agent-guide.md).

---

## Run the benchmarks

```bash
bash scripts/benchmark-holdout.sh              # every gold set, one line each (checkouts fetched, graphs cached)
bash scripts/benchmark-holdout.sh holdout-web  # one set
NM_EXPLAIN=1 bash scripts/benchmark-holdout.sh dev   # + per-task reasons in target/explain-dev.txt
```

Rules the project holds itself to: holdout sets are never tuned on; ratchets only move up; a mixed result is reverted and recorded, never shipped. Every finding since the fork (F1–F82) is written down with its measurement in [docs/planning/stage5-findings.fa.md](docs/planning/stage5-findings.fa.md).

---

## Documentation

| Read this when you want to… | |
|---|---|
| wire an agent to the engine | [Agent guide](docs/agent-guide.md) · [MCP tools](docs/mcp.md) |
| use the CLI | [CLI](docs/cli.md) · [Configuration](docs/configuration.md) · [Engines](docs/engines.md) |
| understand the design | [Architecture](docs/architecture.md) · [Isolation](docs/isolation.md) |
| check the numbers | [Measured](docs/measured.md) · [Quality methodology](docs/quality.md) |
| follow or join the work | [ROADMAP.md](ROADMAP.md) · [Planning & findings (Persian)](docs/planning/) · [Contributing](docs/contributing.md) |

---

<sub>Independent derivative of <a href="https://github.com/pinoox/neuromesh">NeuroMesh</a> by yoosef alipour, MIT. Baseline tag <code>baseline-v0.9.0</code>. See <a href="NOTICE">NOTICE</a> and <a href="ATTRIBUTION.md">ATTRIBUTION.md</a>. MIT · <a href="LICENSE">LICENSE</a></sub>
