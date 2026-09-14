# mini-orders

A small CommonJS fixture whose bugs are planted on purpose. Every task in
`tests/tasks/fixtures.toml` that points here carries a `verify` command that
`node` can run in a scratch copy, so `neuromesh eval --tasks --executor model`
measures a real patch, not a judge's opinion.
