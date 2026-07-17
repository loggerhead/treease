# Evaluator Boundary

This directory owns evaluation of parsed Treease expressions against all-at-once and streaming inputs.

## Public Contracts

- Value and evaluation errors: `mod.rs`
- All-at-once evaluation: `all_at_once_evaluator.rs`
- Streaming evaluation: `stream_evaluator.rs`
- Parsed operations: `../registry/expression.rs`, `../registry/operation.rs`
- Operator handlers: `../operators/`

## Boundary Rules

- Evaluate the parser and registry expression model; do not introduce a second expression representation.
- Keep all-at-once and streaming paths semantically aligned while allowing their input mechanics to differ.
- Put operator-specific behavior in `../operators/`, not evaluator dispatch.
- Preserve `Value` and `EvaluationError` variants as caller-visible result contracts.
