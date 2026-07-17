# Registry Boundary

This directory owns the runtime registry that connects parsed operations and format names to their handlers and factories.

## Public Contracts

- Registry ownership and handles: `registry.rs`
- Operator registration lookup: `operator_registry.rs`
- Format registration lookup: `format_registry.rs`
- Expression and operation model: `expression.rs`, `operation.rs`, `operation_defs.rs`
- Expression/traversal builders: `expression_builder.rs`, `traversal_builder.rs`

## Boundary Rules

- Keep registry construction and lifecycle centralized in `Registry` and `RegistryOwner`.
- Keep operator execution metadata in the operator registry and format construction metadata in the format registry.
- Preserve handle ownership semantics; do not create hidden global registries or bypass explicit lifecycle APIs.
- Update parsers, operators, formats, generated capability docs, and tests together when changing supported registrations.
