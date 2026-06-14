use std::collections::HashMap;

use super::errors::{CoreError, EvalError};
use super::graph_builder::GraphModel as BuilderGraphModel;
use super::graph_fragment_index::GraphFragmentIndex;
use super::line_index::LineIndex;
use super::sem_type::SemType;
use super::tree_node::{NodeId, ParsedKey, TreeNode, TreeNodeKind};
use crate::wasm_types::PathSpan;

// ---------------------------------------------------------------------------
// TokenSpan
// ---------------------------------------------------------------------------

/// A span of tokens identified during semantic token collection.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenSpan {
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
    pub token_type: u32,
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

/// A stored document tree entry, keyed by document cache key.
///
#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub key: String,
    pub language: String,
    pub root: NodeId,
    pub ts_tree: Option<tree_sitter::Tree>,
    pub source: String,
    pub token_spans: Vec<TokenSpan>,
    pub diagnostics_raw: Vec<u32>,
    pub semantic_tokens_encoded: Vec<u32>,
    pub value_json: String,
    pub line_index: LineIndex,
    pub incremental_edit_count: u32,
    pub incremental_replaced_bytes: usize,
}

/// A borrowed view of a [`TreeEntry`], used for returning document analysis
/// results without cloning the entire entry.
///
#[derive(Debug, Clone)]
pub struct DocumentAnalysis<'a> {
    pub language: &'a str,
    pub source: &'a str,
    pub root: NodeId,
    pub ts_tree: Option<&'a tree_sitter::Tree>,
    pub token_spans: &'a [TokenSpan],
    pub diagnostics_raw: &'a [u32],
    pub semantic_tokens_encoded: &'a [u32],
    pub value_json: &'a str,
}

/// A stored graph view entry, keyed by document cache key.
///
#[derive(Debug, Clone)]
pub struct GraphEntry {
    pub key: String,
    pub model: BuilderGraphModel,
    pub fragment_index: Option<GraphFragmentIndex>,
}

// ---------------------------------------------------------------------------
// TreeStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TreeStore {
    // -- Node container (existing) -------------------------------------------
    nodes: Vec<TreeNode>,

    // -- Document-level tree entries (new) -----------------------------------
    /// Full document entries keyed by document cache key.
    trees: HashMap<String, TreeEntry>,

    // -- Graph view entries (new) --------------------------------------------
    /// Graph view entries keyed by document cache key.
    views: HashMap<String, GraphEntry>,

    // -- Legacy lightweight caches (kept for backward compatibility) ---------
    /// Cached token spans keyed by document cache key.
    /// Used by the semantic-tokens pipeline when a full TreeEntry is not yet
    /// available.
    token_spans_cache: HashMap<String, Vec<TokenSpan>>,
    /// Cached encoded semantic tokens (u32 delta-encoded) keyed by document
    /// cache key.
    semantic_tokens_cache: HashMap<String, Vec<u32>>,
}

// ============================================================================
// Node-container methods (existing – unchanged)
// ============================================================================

impl TreeStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, node: TreeNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    pub fn get(&self, id: NodeId) -> Option<&TreeNode> {
        self.nodes.get(id.0)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut TreeNode> {
        self.nodes.get_mut(id.0)
    }

    pub fn nodes(&self) -> &[TreeNode] {
        &self.nodes
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn create_child(&mut self, parent: NodeId) -> Result<NodeId, CoreError> {
        self.add_child(parent, TreeNode::default())
    }

    pub fn add_child(&mut self, parent: NodeId, raw_child: TreeNode) -> Result<NodeId, CoreError> {
        let sequence_index = {
            let parent_node = self.node(parent)?;
            if parent_node.kind == TreeNodeKind::Sequence {
                Some(parent_node.content.len() as i64)
            } else {
                None
            }
        };

        let mut child = raw_child;
        child.parent = Some(parent);
        child.is_map_key = false;
        child.sequence_index = sequence_index;
        let child_id = self.add(child);
        self.node_mut(parent)?.content.push(child_id);
        Ok(child_id)
    }

    pub fn add_key_value_child(
        &mut self,
        parent: NodeId,
        raw_key: TreeNode,
        raw_value: TreeNode,
    ) -> Result<(NodeId, NodeId), CoreError> {
        let mut key = raw_key;
        key.parent = Some(parent);
        key.is_map_key = true;
        key.sequence_index = None;
        let key_id = self.add(key);

        let mut value = raw_value;
        value.parent = Some(parent);
        value.is_map_key = false;
        value.key = Some(key_id);
        value.sequence_index = None;
        let value_id = self.add(value);

        let parent_node = self.node_mut(parent)?;
        parent_node.content.push(key_id);
        parent_node.content.push(value_id);
        Ok((key_id, value_id))
    }

    pub fn document_for(&self, id: NodeId) -> Result<u32, CoreError> {
        let node = self.node(id)?;
        match node.parent {
            Some(parent) => self.document_for(parent),
            None => Ok(node.document),
        }
    }

    pub fn filename_for(&self, id: NodeId) -> Result<&str, CoreError> {
        let node = self.node(id)?;
        match node.parent {
            Some(parent) => self.filename_for(parent),
            None => Ok(&node.filename),
        }
    }

    pub fn file_index_for(&self, id: NodeId) -> Result<i32, CoreError> {
        let node = self.node(id)?;
        match node.parent {
            Some(parent) => self.file_index_for(parent),
            None => Ok(node.file_index),
        }
    }

    pub fn parsed_key_for(&self, id: NodeId) -> Result<Option<ParsedKey>, CoreError> {
        let node = self.node(id)?;
        if node.is_map_key {
            return Ok(Some(ParsedKey::Str(node.value.clone())));
        }
        if let Some(key_id) = node.key {
            let key_node = self.node(key_id)?;
            if key_node.resolved_sem_type() == Some(SemType::Str) {
                return Ok(Some(ParsedKey::Str(key_node.value.clone())));
            }
            return Ok(Some(match key_node.value.parse::<i64>() {
                Ok(index) => ParsedKey::Int(index),
                Err(_) => ParsedKey::Str(key_node.value.clone()),
            }));
        }
        if self.sequence_index_for(id)?.is_some() {
            return Ok(node.sequence_index.map(ParsedKey::Int));
        }
        Ok(None)
    }

    pub fn path_for(&self, id: NodeId) -> Result<Vec<ParsedKey>, CoreError> {
        let node = self.node(id)?;
        let key = self.parsed_key_for(id)?;
        let mut path = if let Some(parent) = node.parent {
            self.path_for(parent)?
        } else {
            Vec::new()
        };
        if let Some(key) = key {
            path.push(key);
        }
        Ok(path)
    }

    pub fn nice_path_for(&self, id: NodeId) -> Result<String, CoreError> {
        let mut out = String::new();
        for (index, element) in self.path_for(id)?.iter().enumerate() {
            match element {
                ParsedKey::Int(value) => out.push_str(&format!("[{value}]")),
                ParsedKey::Str(value) if index == 0 => out.push_str(value),
                ParsedKey::Str(value) if value.contains('.') => out.push_str(&format!("[{value}]")),
                ParsedKey::Str(value) => out.push_str(&format!(".{value}")),
            }
        }
        Ok(out)
    }

    pub fn find_descendant_by_path(
        &self,
        root: NodeId,
        path: &[ParsedKey],
        prefer_key: bool,
    ) -> Result<Option<NodeId>, CoreError> {
        let mut current = root;
        for (index, segment) in path.iter().enumerate() {
            let node = self.node(current)?;
            let is_last = index + 1 == path.len();
            current = match (node.kind, segment) {
                (TreeNodeKind::Mapping, ParsedKey::Str(key)) => {
                    let Some((key_id, value_id)) = self.find_map_entry(current, key)? else {
                        return Ok(None);
                    };
                    if is_last && prefer_key {
                        key_id
                    } else {
                        value_id
                    }
                }
                (TreeNodeKind::Mapping, ParsedKey::Int(index_value)) => {
                    let key_text = index_value.to_string();
                    let Some((key_id, value_id)) = self.find_map_entry(current, &key_text)? else {
                        return Ok(None);
                    };
                    if is_last && prefer_key {
                        key_id
                    } else {
                        value_id
                    }
                }
                (TreeNodeKind::Sequence, ParsedKey::Int(index_value)) if *index_value >= 0 => {
                    let next_index = usize::try_from(*index_value).ok();
                    let Some(next_index) = next_index else {
                        return Ok(None);
                    };
                    let Some(next) = node.content.get(next_index) else {
                        return Ok(None);
                    };
                    *next
                }
                _ => return Ok(None),
            };
        }
        Ok(Some(current))
    }

    pub fn path_span_for(
        &self,
        root: NodeId,
        path: &[ParsedKey],
        prefer_key: bool,
    ) -> Result<PathSpan, CoreError> {
        let Some(node_id) = self.find_descendant_by_path(root, path, prefer_key)? else {
            return Ok(PathSpan::EMPTY);
        };
        let node = self.node(node_id)?;
        Ok(PathSpan {
            start_byte: i32::try_from(node.start_byte).unwrap_or(-1),
            end_byte: i32::try_from(node.end_byte).unwrap_or(-1),
            row: node.line.saturating_sub(1),
            column: node.column.saturating_sub(1),
        })
    }

    fn sequence_index_for(&self, id: NodeId) -> Result<Option<i64>, CoreError> {
        let node = self.node(id)?;
        let Some(parent_id) = node.parent else {
            return Ok(None);
        };
        let parent = self.node(parent_id)?;
        if node.is_map_key || parent.kind != TreeNodeKind::Sequence {
            return Ok(None);
        }
        Ok(node.sequence_index)
    }

    fn node(&self, id: NodeId) -> Result<&TreeNode, CoreError> {
        self.get(id)
            .ok_or(CoreError::Eval(EvalError::MissingTreeNode))
    }

    fn node_mut(&mut self, id: NodeId) -> Result<&mut TreeNode, CoreError> {
        self.get_mut(id)
            .ok_or(CoreError::Eval(EvalError::MissingTreeNode))
    }

    fn find_map_entry(
        &self,
        parent: NodeId,
        expected_key: &str,
    ) -> Result<Option<(NodeId, NodeId)>, CoreError> {
        let node = self.node(parent)?;
        let mut index = 0;
        while index + 1 < node.content.len() {
            let key_id = node.content[index];
            let value_id = node.content[index + 1];
            let key_node = self.node(key_id)?;
            if key_node.is_map_key && key_node.value == expected_key {
                return Ok(Some((key_id, value_id)));
            }
            index += 2;
        }
        Ok(None)
    }
}

// ============================================================================
// Legacy lightweight cache methods (used by semantic_tokens pipeline)
// ============================================================================

impl TreeStore {
    /// Retrieve cached token spans from the lightweight cache.
    ///
    /// Returns `None` when no spans have been cached for `cache_key`.
    pub fn get_cached_token_spans(&self, cache_key: &str) -> Option<&[TokenSpan]> {
        self.token_spans_cache.get(cache_key).map(|v| v.as_slice())
    }

    /// Store token spans in the lightweight cache.
    pub fn set_cached_token_spans(&mut self, cache_key: &str, spans: Vec<TokenSpan>) {
        self.token_spans_cache.insert(cache_key.to_string(), spans);
    }

    /// Retrieve cached encoded semantic tokens from the lightweight cache.
    ///
    /// Returns `None` when no encoded tokens have been cached for `cache_key`.
    pub fn get_cached_semantic_tokens(&self, cache_key: &str) -> Option<&[u32]> {
        self.semantic_tokens_cache
            .get(cache_key)
            .map(|v| v.as_slice())
    }

    /// Store encoded semantic tokens in the lightweight cache.
    pub fn set_cached_semantic_tokens(&mut self, cache_key: &str, tokens: Vec<u32>) {
        self.semantic_tokens_cache
            .insert(cache_key.to_string(), tokens);
    }
}

// ============================================================================
// ============================================================================

impl TreeStore {
    // -- set_tree ------------------------------------------------------------

    /// Store (or replace) a full document tree entry.
    ///
    /// When an entry already exists for `document_key`, the old tree-sitter
    /// tree is dropped and the entry is replaced in-place (the key allocation
    /// is reused).
    ///
    pub fn set_tree(
        &mut self,
        document_key: &str,
        language: &str,
        root: NodeId,
        ts_tree: Option<tree_sitter::Tree>,
        source: &str,
        token_spans: Vec<TokenSpan>,
    ) {
        let line_index = LineIndex::build(source);

        if let Some(entry) = self.trees.get_mut(document_key) {
            // Drop the old tree-sitter tree (if any).
            drop(entry.ts_tree.take());
            entry.language = language.to_string();
            entry.root = root;
            entry.ts_tree = ts_tree;
            entry.source = source.to_string();
            entry.token_spans = token_spans;
            entry.diagnostics_raw.clear();
            entry.semantic_tokens_encoded.clear();
            entry.value_json.clear();
            entry.line_index = line_index;
            entry.incremental_edit_count = 0;
            entry.incremental_replaced_bytes = 0;
            return;
        }

        self.trees.insert(
            document_key.to_string(),
            TreeEntry {
                key: document_key.to_string(),
                language: language.to_string(),
                root,
                ts_tree,
                source: source.to_string(),
                token_spans,
                diagnostics_raw: Vec::new(),
                semantic_tokens_encoded: Vec::new(),
                value_json: String::new(),
                line_index,
                incremental_edit_count: 0,
                incremental_replaced_bytes: 0,
            },
        );
    }

    // -- set_document_analysis -----------------------------------------------

    /// Full document analysis: calls [`set_tree`] followed by
    /// [`set_diagnostics_raw`], [`set_semantic_tokens_encoded`], and
    /// [`set_value_json`].
    ///
    pub fn set_document_analysis(
        &mut self,
        document_key: &str,
        language: &str,
        root: NodeId,
        ts_tree: Option<tree_sitter::Tree>,
        source: &str,
        token_spans: Vec<TokenSpan>,
        diagnostics_raw: Vec<u32>,
        semantic_tokens_encoded: Vec<u32>,
        value_json: String,
    ) {
        self.set_tree(document_key, language, root, ts_tree, source, token_spans);
        self.set_diagnostics_raw(document_key, diagnostics_raw);
        self.set_semantic_tokens_encoded_for_entry(document_key, semantic_tokens_encoded);
        self.set_value_json(document_key, value_json);
    }

    /// Alias for [`set_document_analysis`].
    ///
    pub fn store_document_analysis_owned(
        &mut self,
        document_key: &str,
        language: &str,
        root: NodeId,
        ts_tree: Option<tree_sitter::Tree>,
        source: &str,
        token_spans: Vec<TokenSpan>,
        diagnostics_raw: Vec<u32>,
        semantic_tokens_encoded: Vec<u32>,
        value_json: String,
    ) {
        self.set_document_analysis(
            document_key,
            language,
            root,
            ts_tree,
            source,
            token_spans,
            diagnostics_raw,
            semantic_tokens_encoded,
            value_json,
        );
    }

    // -- get_tree / get_tree_entry -------------------------------------------

    /// Return the root [`NodeId`] for a document, if it exists.
    ///
    pub fn get_tree(&self, document_key: &str) -> Option<NodeId> {
        self.trees.get(document_key).map(|entry| entry.root)
    }

    /// Return a reference to the full [`TreeEntry`] for a document.
    ///
    pub fn get_tree_entry(&self, document_key: &str) -> Option<&TreeEntry> {
        self.trees.get(document_key)
    }

    /// Return a mutable reference to the full [`TreeEntry`] for a document.
    pub fn get_tree_entry_mut(&mut self, document_key: &str) -> Option<&mut TreeEntry> {
        self.trees.get_mut(document_key)
    }

    // -- incremental-edit counters -------------------------------------------

    /// Increment the `incremental_edit_count` for a document by 1.
    ///
    /// This is a cumulative counter that tracks how many incremental edits
    /// have been applied since the last full parse.  Callers should reset it
    /// to 0 on a full re-parse (which [`set_tree`] already does).
    ///
    pub fn increment_edit_count(&mut self, document_key: &str) {
        if let Some(entry) = self.trees.get_mut(document_key) {
            entry.incremental_edit_count = entry.incremental_edit_count.saturating_add(1);
        }
    }

    /// Add `bytes` to the `incremental_replaced_bytes` accumulator for a
    /// document.
    ///
    /// This tracks the total number of replaced bytes across incremental
    /// edits.  Callers should reset it to 0 on a full re-parse (which
    /// [`set_tree`] already does).
    ///
    pub fn add_replaced_bytes(&mut self, document_key: &str, bytes: usize) {
        if let Some(entry) = self.trees.get_mut(document_key) {
            entry.incremental_replaced_bytes =
                entry.incremental_replaced_bytes.saturating_add(bytes);
        }
    }

    /// Return the current `incremental_edit_count` for a document.
    ///
    /// Returns `0` when no entry exists for `document_key`.
    pub fn incremental_edit_count(&self, document_key: &str) -> u32 {
        self.trees
            .get(document_key)
            .map(|e| e.incremental_edit_count)
            .unwrap_or(0)
    }

    /// Return the current `incremental_replaced_bytes` for a document.
    ///
    /// Returns `0` when no entry exists for `document_key`.
    pub fn incremental_replaced_bytes(&self, document_key: &str) -> usize {
        self.trees
            .get(document_key)
            .map(|e| e.incremental_replaced_bytes)
            .unwrap_or(0)
    }

    // -- get_document_analysis -----------------------------------------------

    /// Return a borrowed [`DocumentAnalysis`] view of a stored tree entry.
    ///
    pub fn get_document_analysis(&self, document_key: &str) -> Option<DocumentAnalysis<'_>> {
        let entry = self.trees.get(document_key)?;
        Some(DocumentAnalysis {
            language: &entry.language,
            source: &entry.source,
            root: entry.root,
            ts_tree: entry.ts_tree.as_ref(),
            token_spans: &entry.token_spans,
            diagnostics_raw: &entry.diagnostics_raw,
            semantic_tokens_encoded: &entry.semantic_tokens_encoded,
            value_json: &entry.value_json,
        })
    }

    // -- get_token_spans (document-level, with source check) -----------------

    /// Return cached token spans for a document, verifying that the stored
    /// source matches the provided `source`.
    ///
    /// Returns `None` when no entry exists, no spans are stored, or the source
    /// does not match.
    ///
    pub fn get_token_spans(&self, document_key: &str, source: &str) -> Option<&[TokenSpan]> {
        let entry = self.trees.get(document_key)?;
        if entry.token_spans.is_empty() {
            return None;
        }
        if entry.source != source {
            return None;
        }
        Some(&entry.token_spans)
    }

    // -- set_diagnostics_raw / get_diagnostics_raw ---------------------------

    /// Store raw diagnostics (u32 quintuples) for a document.
    ///
    /// The document must already have a tree entry (via [`set_tree`]).
    ///
    pub fn set_diagnostics_raw(&mut self, document_key: &str, raw: Vec<u32>) {
        if let Some(entry) = self.trees.get_mut(document_key) {
            entry.diagnostics_raw = raw;
        }
    }

    /// Retrieve raw diagnostics for a document.
    ///
    /// Returns `None` when no entry exists or no diagnostics are stored.
    ///
    pub fn get_diagnostics_raw(&self, document_key: &str) -> Option<&[u32]> {
        let entry = self.trees.get(document_key)?;
        if entry.diagnostics_raw.is_empty() {
            return None;
        }
        Some(&entry.diagnostics_raw)
    }

    /// Alias for [`get_diagnostics_raw`].
    ///
    pub fn get_diagnostics(&self, document_key: &str) -> Option<&[u32]> {
        self.get_diagnostics_raw(document_key)
    }

    // -- set / get semantic_tokens_encoded (document-level, with source check)

    /// Store encoded semantic tokens on an existing tree entry.
    ///
    /// The document must already have a tree entry (via [`set_tree`]).
    ///
    pub fn set_semantic_tokens_encoded_for_entry(&mut self, document_key: &str, raw: Vec<u32>) {
        if let Some(entry) = self.trees.get_mut(document_key) {
            entry.semantic_tokens_encoded = raw;
        }
    }

    /// Retrieve encoded semantic tokens for a document, verifying that the
    /// stored source matches the provided `source`.
    ///
    /// Returns `None` when no entry exists, no tokens are stored, or the
    /// source does not match.
    ///
    pub fn get_semantic_tokens_encoded(&self, document_key: &str, source: &str) -> Option<&[u32]> {
        let entry = self.trees.get(document_key)?;
        if entry.semantic_tokens_encoded.is_empty() {
            return None;
        }
        if entry.source != source {
            return None;
        }
        Some(&entry.semantic_tokens_encoded)
    }

    /// Alias for [`get_semantic_tokens_encoded`].
    ///
    pub fn get_semantic_tokens(&self, document_key: &str, source: &str) -> Option<&[u32]> {
        self.get_semantic_tokens_encoded(document_key, source)
    }

    // -- set_value_json / get_value_json -------------------------------------

    /// Store the normalised JSON value representation for a document.
    ///
    /// The document must already have a tree entry (via [`set_tree`]).
    ///
    pub fn set_value_json(&mut self, document_key: &str, raw: String) {
        if let Some(entry) = self.trees.get_mut(document_key) {
            entry.value_json = raw;
        }
    }

    /// Retrieve the normalised JSON value for a document.
    ///
    /// Returns `None` when no entry exists or no value JSON is stored.
    ///
    pub fn get_value_json(&self, document_key: &str) -> Option<&str> {
        let entry = self.trees.get(document_key)?;
        if entry.value_json.is_empty() {
            return None;
        }
        Some(&entry.value_json)
    }

    /// Alias for [`get_value_json`].
    ///
    pub fn get_value(&self, document_key: &str) -> Option<&str> {
        self.get_value_json(document_key)
    }

    // -- get_tree_data -------------------------------------------------------

    /// Return the root node and value JSON for a document.
    ///
    pub fn get_tree_data(&self, document_key: &str) -> Option<(NodeId, &str)> {
        let entry = self.trees.get(document_key)?;
        Some((entry.root, &entry.value_json))
    }

    // -- remove_tree ---------------------------------------------------------

    /// Remove a document tree entry, dropping the tree-sitter tree if present.
    ///
    /// Returns `true` if an entry was removed.
    ///
    pub fn remove_tree(&mut self, document_key: &str) -> bool {
        self.trees.remove(document_key).is_some()
    }

    // -- tree_count / has_tree -----------------------------------------------

    /// Return the number of stored document tree entries.
    pub fn tree_count(&self) -> usize {
        self.trees.len()
    }

    /// Return `true` if a tree entry exists for `document_key`.
    pub fn has_tree(&self, document_key: &str) -> bool {
        self.trees.contains_key(document_key)
    }
}

// ============================================================================
// ============================================================================

impl TreeStore {
    /// Store a graph model for a document (without a fragment index).
    ///
    pub fn set_graph(&mut self, document_key: &str, model: BuilderGraphModel) {
        self.set_graph_with_index(document_key, model, None);
    }

    /// Alias for [`set_graph`].
    ///
    pub fn set_view(&mut self, document_key: &str, model: BuilderGraphModel) {
        self.set_graph(document_key, model);
    }

    /// Store a graph model with an optional fragment index for a document.
    ///
    /// When an entry already exists for `document_key` it is replaced
    /// in-place.
    ///
    pub fn set_graph_with_index(
        &mut self,
        document_key: &str,
        model: BuilderGraphModel,
        fragment_index: Option<GraphFragmentIndex>,
    ) {
        if let Some(entry) = self.views.get_mut(document_key) {
            entry.model = model;
            entry.fragment_index = fragment_index;
            return;
        }
        self.views.insert(
            document_key.to_string(),
            GraphEntry {
                key: document_key.to_string(),
                model,
                fragment_index,
            },
        );
    }

    /// Return a reference to the stored [`BuilderGraphModel`] for a document.
    ///
    pub fn get_graph(&self, document_key: &str) -> Option<&BuilderGraphModel> {
        self.views.get(document_key).map(|entry| &entry.model)
    }

    /// Alias for [`get_graph`].
    ///
    pub fn get_view(&self, document_key: &str) -> Option<&BuilderGraphModel> {
        self.get_graph(document_key)
    }

    /// Return a reference to the stored [`GraphFragmentIndex`] for a document.
    ///
    pub fn get_graph_index(&self, document_key: &str) -> Option<&GraphFragmentIndex> {
        self.views
            .get(document_key)
            .and_then(|entry| entry.fragment_index.as_ref())
    }

    /// Remove a graph view entry.
    ///
    /// Returns `true` if an entry was removed.
    ///
    pub fn remove_graph(&mut self, document_key: &str) -> bool {
        self.views.remove(document_key).is_some()
    }

    /// Return the number of stored graph view entries.
    pub fn graph_count(&self) -> usize {
        self.views.len()
    }

    /// Return `true` if a graph entry exists for `document_key`.
    pub fn has_graph(&self, document_key: &str) -> bool {
        self.views.contains_key(document_key)
    }
}

// ============================================================================
// ============================================================================

impl TreeStore {
    /// Remove all document tree entries, dropping any owned tree-sitter trees.
    ///
    pub fn clear_trees(&mut self) {
        self.trees.clear();
    }

    /// Remove all graph view entries.
    ///
    pub fn clear_graphs(&mut self) {
        self.views.clear();
    }

    /// Remove all tree entries, graph entries, and lightweight caches.
    ///
    pub fn clear(&mut self) {
        self.clear_trees();
        self.clear_graphs();
        self.token_spans_cache.clear();
        self.semantic_tokens_cache.clear();
    }
}
