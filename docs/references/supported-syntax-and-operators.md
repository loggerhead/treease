# Supported Syntax and Operators

## Purpose

- Provide a hand-written reference for Core expression capabilities.
- Explain the relationship between the recognized expression syntax and registered operator capabilities.
- Provide a stable entry point for reading code, updating documentation, and comparing capabilities.

## Scope

- This document covers the capability groupings that the current hand-written overview introduces first.
- A capability listed here does not imply that `docs/operators/` has a dedicated page for it.
- The files actually present in `docs/operators/` determine whether a dedicated page exists.
- `../generated/core-registry-capabilities.md` and the actual registry determine whether the current build registers a capability.

## Current Code Entry Points

- Lexing: `packages/core/src/parser/lexer_participle.rs`
- Token post-processing: `packages/core/src/parser/lexer.rs`
- Expression parsing / infix-to-postfix conversion: `packages/core/src/parser/parser.rs`
- Expression tree construction: `packages/core/src/registry/expression_builder.rs`
- Core operation definitions: `packages/core/src/registry/operation.rs`
- Operator registration: `packages/core/src/operators/registry.rs`
- Operator registry table: `packages/core/src/operators/registry_tables_ops.rs`
- Codec registry table: `packages/core/src/operators/registry_tables_formats.rs`

## Expression Syntax Overview

### Comments and Whitespace

- Spaces, tabs, line feeds, and carriage returns are supported.
- `#` line comments are supported.

### Identifiers and Literals

- Identifiers start with a letter or underscore and may subsequently contain digits, underscores, and `-`.
- `true`, `false`, and `null` are supported.
- Integers, floating-point numbers, and double-quoted strings are supported.
- Strings support basic escaping and syntax entry points related to interpolation.

### Grouping and Collections

- `(` `)`: grouping
- `[ ... ]`: array construction and collection
- `{ ... }`: object construction and collection
- `:`: key-value pair construction

### Path Traversal

- `.`：self
- `.name` / `.name?`: traverse by key
- `.[]` / `.[]?`: traverse arrays or collections
- `..` / `...`: recursive descent

### Variables

- `$foo`: variable reference
- `as`: variable binding

## Operator Capability Overview

### Composition and Control

- `|`
- `;`
- `,`
- `reduce`
- `with`
- `empty`

### Assignment

- `=`
- `=c`
- `|=`
- `+=` / `-=` / `*=`

### Arithmetic and Comparison

- `+` `-` `*` `/` `%`
- `//`
- `==` `!=`
- `<` `<=` `>` `>=`

### Collections and Traversal

- `select`
- `map` / `map_values`
- `filter`
- `keys` / `key`
- `has`
- `contains`
- `pick`
- `omit`
- `compact`
- `flatten`
- `group_by`
- `unique` / `unique_by`
- `sort` / `sort_by`
- `sort_keys`
- `reverse`
- `shuffle`
- `first`
- `delpaths`

### Strings and Conversion

- `sub`
- `match`
- `capture`
- `test`
- `join`
- `split`
- `trim`
- `to_string`
- `to_number`
- `upcase` / `downcase`

### Metadata and Paths

- `path`
- `setpath`
- `parent`
- `parents`
- `tag` / `type`
- `kind`

### Encoding and Decoding

- Registry-backed codecs: `yaml`, `json`, `csv`, `base64`, `toml`, `python`, `javascript`
- Shorthands: `@yaml` / `@json` / `@csv` / `@base64`
- Shorthand decoding: `@yamld` / `@jsond` / `@csvd` / `@base64d`
- Functions: `to_yaml` / `to_json` / `to_csv`
- Decoding functions: `from_yaml` / `from_json` / `from_csv`
- Note: `toml`, `python`, and `javascript` are registered in the codec registry, but the current lexical shorthand and `to_...` / `from_...` lists do not yet cover them. `@uri` / `@urid` only have lexical entry points and are not currently in the registry-backed codec list.

## Responsibilities of the Hand-Written Reference and Generated Snapshot

- This document explains capability groupings, entry-point files, and reading paths.
- The generated registered-capability snapshot is at `../generated/core-registry-capabilities.md`.
- To determine whether the current build enables an operator, prefer the generated snapshot and the actual registry.

## Maintenance Rules

- When updating syntax or operator implementations, also check that this document still reflects the main path.
- To verify the supported list precisely, check the generated `docs/generated/core-registry-capabilities.md` from `packages/core/src/tools/export_registry_doc.rs` as well.
