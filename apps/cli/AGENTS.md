---
summary: "apps/cli ownership boundaries, test entry points, and reuse constraints."
read_when:
  - Confirming verification boundaries and protocol reuse for CLI changes
---

# apps/cli Guide

## Scope

- This directory owns the standalone CLI crate, CLI acceptance tests, and related documentation contracts.
- The Rust CLI entry point is `src/main.rs`; primary implementation lives in `src/lib.rs` and `src/{parser,spec,catalog,errors}.rs`.

## Boundary Rules

- Do not duplicate Core parsing, formatting, operator, or evaluation implementations here.
- The CLI reuses execution, format, and operator capabilities through `treease-core`; do not move CLI parsing, help, or error contracts back into `packages/core/`.
- User-visible CLI behavior changes must account for stdout, stderr, exit codes, and file writes together.
- Keep documentation, commands, and test entry points aligned with `Cargo.toml` and `tests/acceptance/run.sh`.
