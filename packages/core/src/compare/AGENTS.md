# Compare Boundary

This directory owns text and structured comparison, diff classification, and the diff algorithms that support them.

## Public Contracts

- Text comparison: `compare.rs`
- Structured comparison and diff: `structured.rs`
- Diff model and classification: `diff.rs`
- Algorithms: `algorithms.rs`, `histogram.rs`, `myers.rs`
- WASM consumers: `../wasm/compat_exports.rs`

## Boundary Rules

- Keep algorithm mechanics behind the comparison entry points; callers consume classified results, not algorithm-specific state.
- Preserve diff ordering and `DiffType` semantics across text and structured comparisons.
- Keep presentation, color, and UI grouping outside this module.
