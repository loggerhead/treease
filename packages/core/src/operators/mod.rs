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

pub mod kind;
pub mod operator_helpers;
pub mod registry;
pub mod registry_tables_formats;
pub mod registry_tables_ops;
pub mod value;

pub mod add;
pub mod alternative;
pub mod assign;
pub mod booleans;
pub mod collect;
pub mod collect_object;
pub mod compact;
pub mod contains;
pub mod create_map;
pub mod delete;
pub mod divide;
pub mod encoder_decoder;
pub mod entries;
pub mod equals;
pub mod expression;
pub mod filter;
pub mod first;
pub mod flatten;
pub mod group_by;
pub mod has;
pub mod keys;
pub mod length;
pub mod map;
pub mod modulo;
pub mod multiply;
pub mod omit;
pub mod parent;
pub mod path;
pub mod pick;
pub mod pipe;
pub mod recursive_descent;
pub mod reduce;
pub mod relational;
pub mod reverse;
pub mod select;
pub mod shuffle;
pub mod sort;
pub mod sort_keys;
pub mod strings;
pub mod subtract;
pub mod tag;
pub mod to_number;
pub mod traverse_path;
pub mod union;
pub mod unique;
pub mod variables;
pub mod with;

// ── Re-exports ───────────────────────────────────────────────────

pub use kind::*;
pub use operator_helpers::*;
pub use registry::*;
pub use registry_tables_formats::*;
pub use registry_tables_ops::*;
pub use value::*;
