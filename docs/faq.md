# FAQ

Short answers to the questions people actually ask before trying this. Every
number here is from [measured.md](measured.md) and reproducible with one
command.

## What is code-context-engine?

A local-first [MCP](https://modelcontextprotocol.io) server that answers "which
files does this task need?" for an AI coding agent. It indexes a repository once
into a per-project code graph, resolves the task's own words to symbols, files,
config keys and string literals, expands along call and import edges, and
returns a folded evidence packet instead of whole files. It is written in Rust,
runs entirely on your machine, and makes no network calls in its default engine.

## How is this different from embedding-based code search (RAG)?

Vector search ranks text by similarity; this engine follows the structure of the
code. A route handler reaches its repository because of a call edge, a config
file reaches its reader because of a `Parameterizes` edge, a decorated object
(`fastify.decorate('knex', …)`) reaches its user because the name is spelled in
a string literal. Embeddings were measured here as an alternative and scored
*worse* than the lexical-plus-graph path on the same questions (finding F79), so
they stayed optional. There is no index of your code in anyone's cloud.

## Does it send my code anywhere?

No. The default `fast` engine is graph plus lexical expansion with no model and
no network. The optional `hybrid` / `deep` engines run a MiniLM embedding model
locally, downloaded once with `neuromesh install embed minilm`. Each project's
graph is isolated by a stable project id, and cross-project leakage is a tested
invariant, not a convention.

## How accurate is it, really?

On four repository families it was **never tuned on** — Go/Python, C/C++,
Scala/R/Julia, Hugging Face/Keras — recall is **1.000**: every hand-written gold
file reaches the packet. Precision on those sets is 0.589–0.700, which means the
packet is on average one file larger than a single-file gold answer. Against the
engine this was forked from, on the same 112 tasks and the same day: recall
0.735 → **0.938**, with roughly three times the precision.

## Does that translate into fewer tokens in a real session?

Yes, but not by the factors marketing pages quote. Same task, same model, with
and without the MCP server, measured from the CLI's own usage report: on Django
(3,516 files) a session used **45% fewer context tokens and 15% less money**; on
a 68-file Fastify repository, 20% fewer tokens. Figures like "99% reduction"
compare a packet against the *whole repository*, which no agent ever sends.

## Does a smaller packet make the model worse?

It was measured with a real model rather than assumed. A separate judge model
graded answers against the real source, with patch tasks verified by applying
the patch and running the tests. On a Go holdout the packet scored **1.000**
task success versus 0.900 for shipping the whole gold files, and on a vision
holdout 0.800 versus 0.700 — the packet is not a lossy summary of the answer,
it is the answer without the neighbours.

## Which languages does it support?

Twelve tree-sitter grammars — TypeScript/JavaScript, Python, Rust, Go, Java,
Kotlin, Swift, C, C++, C#, PHP, Ruby, Dart, Scala — plus light parsers for YAML,
JSON, SQL, shell, PowerShell and Jupyter notebooks. Config-to-code edges cover
YAML, Hydra and argparse, so "the `masks` flag" reaches both the file that
defines it and the code that reads it.

## Which agents and editors can use it?

Any MCP client over stdio: Claude Code, Cursor, VS Code, Codex, Zed, Windsurf
and others. `neuromesh connect --global --agent-rules` writes the client config
and an agent rule for you.

## How much does it cost to run?

Nothing. It is MIT-licensed, runs locally, and uses no paid API. Indexing is
seconds on most repositories and about a minute on a 3,500-file monorepo.

## How do I verify the claims myself?

```bash
bash scripts/benchmark-holdout.sh          # nine public gold sets, pinned checkouts
NM_EXPLAIN=1 bash scripts/benchmark-holdout.sh holdout-c   # plus the reason for every file
```

Gold files are written by reading the source *before* the engine runs, holdout
sets are never tuned on, and every published number names the commit it came
from.

## How does it relate to NeuroMesh?

It is an independent derivative of [NeuroMesh](https://github.com/pinoox/neuromesh)
v0.9.0 (MIT), which began as a context engine for web codebases. This project
continued it with project isolation, language coverage beyond the web stack, and
a measurement discipline built on third-party holdouts. Fixes that apply to the
original are contributed back upstream. See [NOTICE](../NOTICE) and
[ATTRIBUTION.md](../ATTRIBUTION.md).
