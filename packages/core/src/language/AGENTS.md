# Language Boundary

This directory owns supported-language specifications, language detection, Tree-sitter integration, semantic tokens, and scalar edit rules.

## Public Contracts

- Language specification and capabilities: `lang_spec.rs`, `capability.rs`
- Language detection: `guess_language.rs`
- Tree-sitter parsing and queries: `tree_sitter_support.rs`
- Semantic tokens: `semantic_tokens.rs`
- Scalar edit parsing: `edit_rules.rs`

## Boundary Rules

- Define language capabilities once in `lang_spec.rs`; formats, streaming, graph, and WASM consume that definition.
- Keep Tree-sitter parser/query lifecycle in `tree_sitter_support.rs`.
- Preserve `lite` language availability and make related WASM exports follow it.
- Keep editor token rendering and UI edit policy outside this module.
