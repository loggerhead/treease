# Operators Boundary

This directory owns Treease operator implementations, operator registration tables, and evaluation-side compatibility models.

## Public Contracts

- Operator registration: `registry.rs`, `registry_tables_ops.rs`, `registry_tables_formats.rs`
- Operator context and compatibility types: `compat.rs`
- Operator helpers and values: `core_helpers.rs`, `operator_helpers.rs`, `value.rs`
- Expression-facing types: `../registry/expression.rs`, `../registry/operation.rs`
- User-facing capability reference: `../../../../docs/references/supported-syntax-and-operators.md`

## Boundary Rules

- Register an operator in the table that owns its availability; implementation alone is not a supported operator.
- Keep precedence, arity, traversal, and handler metadata consistent with parser and registry contracts.
- Put reusable operator semantics in local shared helpers, not in parser, WASM, or application callers.
- Preserve `lite`: always-compiled modules must not depend on feature-excluded operators or formats.
- Treat `compat.rs` as migration surface; new code should prefer the current `crate::core` types when they express the contract.
