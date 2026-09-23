# Security policy

## Scope

This engine runs locally and reads the source tree you point it at. The parts
worth reporting:

- **Cross-project leakage** — a packet containing files from a project other
  than the one indexed. Project isolation is enforced by `ProjectId` and tested
  in `crates/neuromesh-graph/src/isolation_tests.rs`; a leak is a security bug,
  not a quality bug.
- **Path escape** — indexing or serving a file outside the workspace root.
- **MCP surface** — a crafted prompt or tool argument that makes the server
  read, write, or execute outside its documented behaviour.
- Anything that sends source code off the machine. The `fast` engine makes no
  network calls; `neuromesh install embed` downloads a model and nothing else.

## Reporting

Open a [private security advisory](https://github.com/ParsaVictor/code-context-engine/security/advisories/new).
Please do not open a public issue first. A reply should reach you within a week.

## Supported versions

The latest release on `main`. There is no backport branch yet.
