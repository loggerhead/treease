# Tree Boundary

This directory owns the canonical in-memory tree, node identity, paths, structural edits, and source-location maintenance.

## Public Contracts

- Tree and value model: `tree_node.rs`, `tree_store.rs`
- Path parsing and lookup: `tree_path.rs`, `tree_path_index.rs`
- Navigation and structural operations: `tree_navigator.rs`, `tree_ops.rs`
- Incremental source edits: `incremental_edit.rs`
- Canonical value edits: `value_edit.rs`

## Boundary Rules

- Use `TreeStore` and `NodeId` as the canonical document representation; do not invent module-local tree copies.
- Preserve parent/child links, path resolution, token spans, and source offsets together when changing structure.
- Keep source-edit reconciliation in `incremental_edit.rs`; callers must not patch offsets independently.
- Keep graph projection in `../graph/` and text syntax handling in `../formats/` or `../language/`.
