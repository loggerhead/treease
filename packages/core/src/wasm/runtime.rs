use std::cell::RefCell;

use crate::analysis::LineIndex;
use crate::formats::DecodedDocument;
use crate::registry::registry::RegistryOwner;
use crate::tree::{TokenSpan, TreeStore};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct StoredAnalysis {
    pub(crate) key: String,
    pub(crate) language: String,
    pub(crate) source: String,
    pub(crate) source_byte_length: u32,
    pub(crate) document: Option<DecodedDocument>,
    pub(crate) ts_tree: Option<tree_sitter::Tree>,
    pub(crate) token_spans: Vec<TokenSpan>,
    pub(crate) diagnostics: Vec<u32>,
    pub(crate) semantic_tokens: Vec<u32>,
    pub(crate) value_json: String,
    pub(crate) line_index: LineIndex,
}
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
//
// The canonical RegistryOwner lives in `crate::registry::registry::RegistryOwner`.
// We re-export it here so the WASM layer has a single import surface for all
// runtime types.

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

thread_local! {
    pub(crate) static GRAPH_ARENA: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    pub(crate) static TREE_ARENA: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    pub(crate) static COMPARE_ARENA: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    pub(crate) static GLOBAL_STORE: RefCell<TreeStore> = RefCell::new(TreeStore::new());
    pub(crate) static REGISTRY_OWNER: RefCell<Option<RegistryOwner>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// Deinitialize the graph arena, freeing all temporary graph data.
pub(crate) fn clear_graph_arena() {
    GRAPH_ARENA.with(|arena| *arena.borrow_mut() = None);
}

/// Deinitialize the tree arena, freeing all temporary tree data.
pub(crate) fn clear_tree_arena() {
    TREE_ARENA.with(|arena| *arena.borrow_mut() = None);
}

/// Deinitialize the compare arena, freeing all temporary diff data.
pub(crate) fn clear_compare_arena() {
    COMPARE_ARENA.with(|arena| *arena.borrow_mut() = None);
}
