# Core Source Boundary

This directory assembles the Rust crate and owns the module-level boundary between Core semantics and WASM exports.

## Public Contracts

- Crate exports and internal compatibility surface: `lib.rs`
- Registry lifecycle: `core/mod.rs`, `registry/`
- Document runtime ABI: `wasm_document.rs`
- Compatibility and utility WASM ABI: `wasm.rs`, `wasm/`
- Shared errors and context: `errors.rs`, `context.rs`

## Boundary Rules

- Add behavior to its owning module; keep `lib.rs` an export and assembly surface.
- Keep `internal::*` re-exports aligned with their owners. Do not use them to create a second public API.
- Keep the document runtime on `wasm_document.rs`; non-document and compatibility exports belong in `wasm.rs` and `wasm/`.
- Preserve the `lite` feature boundary: JSON and graph support remain buildable without non-JSON language/operator modules.
- Read the child module's `AGENTS.md` before changing its implementation.
