# Document Boundary

This directory owns the authoritative document runtime: jobs turn input into snapshots, projections, reads, and protocol events.

## Public Contracts

- Protocol source and generated TypeScript input: `protocol.rs`
- Runtime lifecycle and snapshot commits: `runtime.rs`, `runtime/commit.rs`
- Job lifecycle: `job/entry.rs`, `job/engine.rs`, `job/batch.rs`, `job/streaming.rs`
- Snapshot and projection APIs: `snapshot.rs`, `projection/`, `reads/`
- Value-edit planning: `value_edit/`

## Boundary Rules

- `protocol.rs` is the only definition of document wire types; update the generator instead of hand-editing `../../wasm/document-protocol.generated.ts`.
- Keep job orchestration in `job/`, projection materialization in `projection/`, and format-specific edit logic in `value_edit/`.
- Preserve snapshot IDs, event order, terminal states, and query result variants; these cross the Rust/WASM boundary.
