// Operators module — Phase A.4 layout.
//
// exports so callers can reference `crate::operators::<name>` directly.
//
// Shared operator-side models live in `compat`; prefer `crate::core` types for
// new code. The `compat::*` glob re-export is retained for existing operator
// submodules that still depend on the compatibility surface.

pub mod compat;
pub mod core_helpers;
pub use crate::tree::tree_navigator;

pub use compat::*;
pub use tree_navigator::*;

// ── Always-compiled module declarations ──────────────────────────

pub mod kind;
pub mod operator_helpers;
pub mod registry;
pub mod registry_tables_formats;
pub mod value;

// ── Lite-excluded module declarations ─────────────────────────────
// These are individual operator transformations and their supporting
// infrastructure.  Lite mode (JSON + graph only) does not need them.

#[cfg(not(feature = "lite"))]
pub mod add;
#[cfg(not(feature = "lite"))]
pub mod alternative;
#[cfg(not(feature = "lite"))]
pub mod assign;
#[cfg(not(feature = "lite"))]
pub mod booleans;
#[cfg(not(feature = "lite"))]
pub mod collect;
#[cfg(not(feature = "lite"))]
pub mod collect_object;
#[cfg(not(feature = "lite"))]
pub mod contains;
#[cfg(not(feature = "lite"))]
pub mod create_map;
#[cfg(not(feature = "lite"))]
pub mod delete;
#[cfg(not(feature = "lite"))]
pub mod divide;
#[cfg(not(feature = "lite"))]
pub mod encoder_decoder;
#[cfg(not(feature = "lite"))]
pub mod entries;
#[cfg(not(feature = "lite"))]
pub mod equals;
#[cfg(not(feature = "lite"))]
pub mod expression;
#[cfg(not(feature = "lite"))]
pub mod filter;
#[cfg(not(feature = "lite"))]
pub mod first;
#[cfg(not(feature = "lite"))]
pub mod flatten;
#[cfg(not(feature = "lite"))]
pub mod group_by;
#[cfg(not(feature = "lite"))]
pub mod has;
#[cfg(not(feature = "lite"))]
pub mod keys;
#[cfg(not(feature = "lite"))]
pub mod length;
#[cfg(not(feature = "lite"))]
pub mod map;
#[cfg(not(feature = "lite"))]
pub mod modulo;
#[cfg(not(feature = "lite"))]
pub mod multiply;
#[cfg(not(feature = "lite"))]
pub mod omit;
#[cfg(not(feature = "lite"))]
pub mod parent;
#[cfg(not(feature = "lite"))]
pub mod path;
#[cfg(not(feature = "lite"))]
pub mod pick;
#[cfg(not(feature = "lite"))]
pub mod pipe;
#[cfg(not(feature = "lite"))]
pub mod recursive_descent;
#[cfg(not(feature = "lite"))]
pub mod reduce;
#[cfg(not(feature = "lite"))]
pub mod relational;
#[cfg(not(feature = "lite"))]
pub mod reverse;
#[cfg(not(feature = "lite"))]
pub mod select;
#[cfg(not(feature = "lite"))]
pub mod shuffle;
#[cfg(not(feature = "lite"))]
pub mod sort;
#[cfg(not(feature = "lite"))]
pub mod sort_keys;
#[cfg(not(feature = "lite"))]
pub mod strings;
#[cfg(not(feature = "lite"))]
pub mod subtract;
#[cfg(not(feature = "lite"))]
pub mod tag;
#[cfg(not(feature = "lite"))]
pub mod to_number;
#[cfg(not(feature = "lite"))]
pub mod traverse_path;
#[cfg(not(feature = "lite"))]
pub mod union;
#[cfg(not(feature = "lite"))]
pub mod unique;
#[cfg(not(feature = "lite"))]
pub mod variables;
#[cfg(not(feature = "lite"))]
pub mod with;

// ── Re-exports ───────────────────────────────────────────────────

pub use kind::*;
pub use registry::*;
pub use registry_tables_formats::*;
pub use value::*;
// operator_helpers and registry_tables_ops only exist in non-lite builds.
#[cfg(not(feature = "lite"))]
pub use operator_helpers::*;
#[cfg(not(feature = "lite"))]
pub mod registry_tables_ops;
#[cfg(not(feature = "lite"))]
pub use registry_tables_ops::*;
