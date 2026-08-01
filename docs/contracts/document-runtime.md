---
summary: "Document Runtime terms, authority, and invariants for Core and Web changes."
read_when:
  - Changing DocumentJob, DocumentSnapshot, EventBatch, mainGraph, or snapshot-bound reads
  - Changing Worker/WASM behavior that commits or consumes document runtime state
---

# Document Runtime Contract

## Authority

`Document Runtime` owns document semantics. UI and Worker transport results; they do not re-decide freshness, snapshot authority, or parse-failure behavior.

## Core Terms

- `Document` is text identified by `document_key`.
- `DocumentJob` is the only state-advance module: `AnalyzeSource` or `ApplyEdits`.
- `DocumentSnapshot` is an immutable semantic unit for one `document_key`; the runtime assigns its `snapshot_id`.
- `EventBatch` returns `events`, optional lifecycle-only `terminal`, and `request_seq` for one job advance.
- `SnapshotReady` commits the authoritative snapshot and its `mainGraph`.
- `ParseFailed` commits a diagnostics-only snapshot and clears the graph in the same batch.
- `AnalysisDelta` is transient visibility, never an authoritative baseline.

## Invariants

- `ApplyEdits` requires a `base_snapshot` for the same document. Reject a missing base; do not silently start `AnalyzeSource`.
- Blank or whitespace close commits an authoritative clear snapshot.
- Hover and subgraph projection do not advance authoritative state.
- Every read is snapshot-bound: it accepts `snapshotId`, creates no snapshot, and never falls back from `documentKey` to the latest snapshot.
- Snapshot-bound reads return either ready data or `snapshotNotReady`; an empty result cannot stand in for the latter.
- The main graph comes only from job streaming events and `SnapshotReady.mainGraph`.
- Raw projection deltas are normalized once by the Web shared projection-normalization path before any renderer consumes them. Renderers, including shared graph runtimes and extension surfaces, consume the normalized projection shape and must not fork a partial normalizer.

## Seams

- `packages/core/src/document/protocol.rs` is the protocol source of truth.
- `packages/core/src/wasm_document.rs` is the Document Runtime WASM seam.
- Rust owns authority. Worker owns transport, request correlation, UI fan-out, and UI-visible freshness guards.
