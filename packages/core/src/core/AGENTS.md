# Core Facade Boundary

This directory owns the compatibility facade that re-exports the established Core API from its implementing modules.

## Public Contracts

- Compatibility facade: `mod.rs`
- Crate root: `../lib.rs`
- Implementing modules: `../analysis/`, `../graph/`, `../io/`, `../language/`, `../registry/`, `../tree/`

## Boundary Rules

- Keep this facade as re-exports only; place new behavior in the owning module.
- Preserve existing re-export names when they are public compatibility contracts.
- Do not route new internal dependencies through the facade when a direct owning-module import is available.
