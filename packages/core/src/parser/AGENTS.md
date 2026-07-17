# Parser Boundary

This directory owns lexical analysis and parsing of Treease expressions into registry expression nodes.

## Public Contracts

- Token model and lexer entry points: `lexer.rs`
- Participle lexer compatibility: `lexer_participle.rs`
- Expression parser and diagnostics: `parser.rs`
- Parsed expression model: `../registry/expression.rs`
- Operator metadata consumed by parsing: `../registry/operation_defs.rs`

## Boundary Rules

- Keep tokenization, precedence handling, and parse diagnostics here; do not make evaluators or UI code parse expressions independently.
- Preserve `parse_expression` and `parse_expression_with_diagnostics` result semantics for callers.
- Change operator syntax together with its registry metadata and parser tests.
- Keep lexer compatibility isolated to `lexer_participle.rs`; do not spread duplicate token rules.
