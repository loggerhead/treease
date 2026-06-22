# treease-core

`treease-core` is the Rust runtime behind Treease.

It provides the reusable parsing, formatting, evaluation, document-analysis, and graph-building layers shared by the Treease CLI and Web runtime.

## Scope

- Parse and decode structured document formats
- Evaluate Treease expressions
- Re-encode formatted output
- Produce document snapshots and graph data
- Export the WASM-facing runtime used by the Web app

## Repository

- Source: <https://github.com/loggerhead/treease>
- CLI crate: `treease-cli`

## Local Verification

```bash
cd packages/core
cargo nextest run --locked
```
