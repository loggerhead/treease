# WASM Compatibility Boundary

This directory owns non-document WASM exports, shared WASM encoders/decoders, allocator setup, and per-thread runtime state.

## Public Contracts

- Compatibility exports: `compat_exports.rs`
- WASM initialization and shared runtime: `../wasm.rs`, `runtime.rs`
- Decode adapters: `decoders.rs`
- Shared payload encoders: `document_analysis_shared.rs`, `semantic_tokens_shared.rs`, `value_json_shared.rs`
- Document-runtime WASM API: `../wasm_document.rs`

## Boundary Rules

- Keep document job, snapshot, query, and graph-edit APIs in `../wasm_document.rs`; this directory only supports them or exposes non-document compatibility APIs.
- Initialize the WASM registry and allocator through `init_wasm`; do not create competing thread-local runtime owners.
- Convert external `JsValue` at the ABI edge and delegate semantics to Core modules.
- Keep exported payload shapes aligned with generated bindings and Web consumers; do not hand-maintain a duplicate protocol.
- Preserve `lite` feature gating in WASM exports and language support.
