use std::sync::{LazyLock, Mutex};

use crate::core::registry::RegistryOwner;
use crate::core::{LineIndex, TokenSpan, TreeStore};
use crate::formats::DecodedDocument;

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
// The canonical RegistryOwner lives in `crate::core::registry::RegistryOwner`.
// We re-export it here so the WASM layer has a single import surface for all
// runtime types.

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

pub(crate) static GRAPH_ARENA: LazyLock<Mutex<Option<Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(None));

pub(crate) static TREE_ARENA: LazyLock<Mutex<Option<Vec<u8>>>> = LazyLock::new(|| Mutex::new(None));

pub(crate) static COMPARE_ARENA: LazyLock<Mutex<Option<Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(None));

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// Deinitialize the graph arena, freeing all temporary graph data.
pub(crate) fn clear_graph_arena() {
    if let Ok(mut arena) = GRAPH_ARENA.lock() {
        *arena = None;
    }
}

/// Deinitialize the tree arena, freeing all temporary tree data.
pub(crate) fn clear_tree_arena() {
    if let Ok(mut arena) = TREE_ARENA.lock() {
        *arena = None;
    }
}

/// Deinitialize the compare arena, freeing all temporary diff data.
pub(crate) fn clear_compare_arena() {
    if let Ok(mut arena) = COMPARE_ARENA.lock() {
        *arena = None;
    }
}

// ---------------------------------------------------------------------------

/// Global tree store for cached document analyses.
pub(crate) static GLOBAL_STORE: LazyLock<Mutex<TreeStore>> =
    LazyLock::new(|| Mutex::new(TreeStore::new()));

// ---------------------------------------------------------------------------

/// Global registry owner, initialized once during `init()`.
pub(crate) static REGISTRY_OWNER: LazyLock<Mutex<Option<RegistryOwner>>> =
    LazyLock::new(|| Mutex::new(None));
