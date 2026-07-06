use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use super::tree_node::{
    CommentBlock, NodeExtra, NodeExtraId, NodeId, NodeValueRef, ParsedKey, TreeNode, TreeNodeKind,
    ValueId,
};
use crate::analysis::line_index::LineIndex;
use crate::errors::{CoreError, EvalError, ParseError};
use crate::graph::graph_builder::GraphModel as BuilderGraphModel;
use crate::graph::graph_fragment_index::GraphFragmentIndex;
use crate::language::SemType;
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentMeta {
    pub filename: String,
    pub file_index: i32,
}

#[derive(Debug, Clone)]
enum ValueBucket {
    One(ValueId),
    Many(Vec<ValueId>),
}

impl ValueBucket {
    fn push(&mut self, value_id: ValueId) {
        match self {
            ValueBucket::One(existing) => {
                *self = ValueBucket::Many(vec![*existing, value_id]);
            }
            ValueBucket::Many(values) => values.push(value_id),
        }
    }
}

// ---------------------------------------------------------------------------
// TreeStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TreeStore {
    // -- Node container (existing) -------------------------------------------
    nodes: Vec<TreeNode>,
    document_meta: Vec<DocumentMeta>,
    node_extras: Vec<NodeExtra>,
    values: Vec<Box<str>>,
    value_index: Option<HashMap<u64, ValueBucket>>,

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeStoreStats {
    pub node_count: usize,
    pub node_capacity: usize,
    pub scalar_node_count: usize,
    pub mapping_node_count: usize,
    pub sequence_node_count: usize,
    pub alias_node_count: usize,
    pub unknown_node_count: usize,
    pub nodes_with_content_count: usize,
    pub total_content_slots: usize,
    pub nodes_with_stored_value_count: usize,
    pub nodes_with_missing_value_count: usize,
    pub document_meta_count: usize,
    pub document_meta_capacity: usize,
    pub node_extra_count: usize,
    pub node_extra_capacity: usize,
    pub value_count: usize,
    pub value_capacity: usize,
    pub interned_value_bytes: usize,
    pub value_index_entry_count: usize,
    pub tree_entry_count: usize,
    pub graph_entry_count: usize,
    pub token_spans_cache_count: usize,
    pub semantic_tokens_cache_count: usize,
}

impl Default for TreeStore {
    fn default() -> Self {
        let empty_id = ValueId(0);
        let mut value_index = HashMap::new();
        value_index.insert(value_hash(""), ValueBucket::One(empty_id));
        Self {
            nodes: Vec::new(),
            document_meta: Vec::new(),
            node_extras: Vec::new(),
            values: vec![Box::<str>::from("")],
            value_index: Some(value_index),
            trees: HashMap::new(),
            views: HashMap::new(),
            token_spans_cache: HashMap::new(),
            semantic_tokens_cache: HashMap::new(),
        }
    }
}

// ============================================================================
// Node-container methods (existing – unchanged)
// ============================================================================

impl TreeStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, mut node: TreeNode) -> NodeId {
        node.value = match node.value {
            NodeValueRef::Missing => NodeValueRef::Missing,
            NodeValueRef::Stored(id) => NodeValueRef::Stored(id),
            NodeValueRef::Inline(value) => NodeValueRef::Stored(self.intern_boxed_value(value)),
        };
        let id = NodeId::from_index(self.nodes.len());
        self.nodes.push(node);
        id
    }

    pub fn add_node_extra(&mut self, extra: NodeExtra) -> NodeExtraId {
        let id = NodeExtraId::from_index(self.node_extras.len());
        self.node_extras.push(extra);
        id
    }

    pub fn node_extra(&self, id: NodeExtraId) -> Option<&NodeExtra> {
        self.node_extras.get(id.index())
    }

    pub fn node_extra_mut(&mut self, id: NodeExtraId) -> Option<&mut NodeExtra> {
        self.node_extras.get_mut(id.index())
    }

    pub fn ensure_node_extra(&mut self, node: NodeId) -> Result<&mut NodeExtra, CoreError> {
        let extra_id = match self.node(node)?.extra() {
            Some(extra_id) => extra_id,
            None => {
                let extra_id = self.add_node_extra(NodeExtra::default());
                self.node_mut(node)?.set_extra(Some(extra_id));
                extra_id
            }
        };
        self.node_extra_mut(extra_id)
            .ok_or(CoreError::Eval(EvalError::MissingTreeNode))
    }

    pub fn set_anchor(&mut self, node: NodeId, anchor: impl Into<String>) -> Result<(), CoreError> {
        let anchor = anchor.into();
        let extra = self.ensure_node_extra(node)?;
        extra.anchor = (!anchor.is_empty()).then(|| Box::new(anchor));
        self.prune_empty_extra(node)
    }

    pub fn set_leading_content(
        &mut self,
        node: NodeId,
        leading_content: impl Into<String>,
    ) -> Result<(), CoreError> {
        let leading_content = leading_content.into();
        let extra = self.ensure_node_extra(node)?;
        extra.leading_content = (!leading_content.is_empty()).then(|| Box::new(leading_content));
        self.prune_empty_extra(node)
    }

    pub fn set_comments(
        &mut self,
        node: NodeId,
        head: impl Into<String>,
        line: impl Into<String>,
        foot: impl Into<String>,
    ) -> Result<(), CoreError> {
        let head = head.into();
        let line = line.into();
        let foot = foot.into();
        let extra = self.ensure_node_extra(node)?;
        let comments = super::tree_node::CommentBlock {
            head: (!head.is_empty()).then(|| Box::new(head)),
            line: (!line.is_empty()).then(|| Box::new(line)),
            foot: (!foot.is_empty()).then(|| Box::new(foot)),
        };
        extra.comments = (!comments.is_empty()).then_some(comments);
        self.prune_empty_extra(node)
    }

    pub fn set_document_meta_if_absent(&mut self, document: u32, filename: &str, file_index: i32) {
        let meta = self.ensure_document_meta(document);
        if meta.filename.is_empty() && !filename.is_empty() {
            meta.filename = filename.to_owned();
        }
        if meta.file_index == 0 && file_index != 0 {
            meta.file_index = file_index;
        }
    }

    fn prune_empty_extra(&mut self, node: NodeId) -> Result<(), CoreError> {
        let Some(extra_id) = self.node(node)?.extra() else {
            return Ok(());
        };
        let should_clear = self
            .node_extra(extra_id)
            .is_some_and(super::tree_node::NodeExtra::is_empty);
        if should_clear {
            self.node_mut(node)?.set_extra(None);
        }
        Ok(())
    }

    pub fn ensure_document_meta(&mut self, document: u32) -> &mut DocumentMeta {
        let index = document as usize;
        if self.document_meta.len() <= index {
            self.document_meta
                .resize(index + 1, DocumentMeta::default());
        }
        &mut self.document_meta[index]
    }

    pub fn set_document_meta(
        &mut self,
        document: u32,
        filename: impl Into<String>,
        file_index: i32,
    ) {
        let meta = self.ensure_document_meta(document);
        meta.filename = filename.into();
        meta.file_index = file_index;
    }

    pub fn document_meta(&self, document: u32) -> Option<&DocumentMeta> {
        self.document_meta.get(document as usize)
    }

    pub fn document_meta_for_node(&self, id: NodeId) -> Result<&DocumentMeta, CoreError> {
        let document = self.document_for(id)?;
        self.document_meta(document)
            .ok_or(CoreError::Eval(EvalError::MissingTreeNode))
    }

    pub fn get(&self, id: NodeId) -> Option<&TreeNode> {
        self.nodes.get(id.index())
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut TreeNode> {
        self.nodes.get_mut(id.index())
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

    pub fn stats(&self) -> TreeStoreStats {
        let mut scalar_node_count = 0;
        let mut mapping_node_count = 0;
        let mut sequence_node_count = 0;
        let mut alias_node_count = 0;
        let mut unknown_node_count = 0;
        let mut nodes_with_content_count = 0;
        let mut total_content_slots = 0;
        let mut nodes_with_stored_value_count = 0;
        let mut nodes_with_missing_value_count = 0;

        for node in &self.nodes {
            match node.kind {
                TreeNodeKind::Scalar => scalar_node_count += 1,
                TreeNodeKind::Mapping => mapping_node_count += 1,
                TreeNodeKind::Sequence => sequence_node_count += 1,
                TreeNodeKind::Alias => alias_node_count += 1,
                TreeNodeKind::Unknown => unknown_node_count += 1,
            }

            if !node.content.is_empty() {
                nodes_with_content_count += 1;
                total_content_slots += node.content.len();
            }

            match node.value {
                NodeValueRef::Stored(_) => nodes_with_stored_value_count += 1,
                NodeValueRef::Missing => nodes_with_missing_value_count += 1,
                NodeValueRef::Inline(_) => {}
            }
        }

        TreeStoreStats {
            node_count: self.nodes.len(),
            node_capacity: self.nodes.capacity(),
            scalar_node_count,
            mapping_node_count,
            sequence_node_count,
            alias_node_count,
            unknown_node_count,
            nodes_with_content_count,
            total_content_slots,
            nodes_with_stored_value_count,
            nodes_with_missing_value_count,
            document_meta_count: self.document_meta.len(),
            document_meta_capacity: self.document_meta.capacity(),
            node_extra_count: self.node_extras.len(),
            node_extra_capacity: self.node_extras.capacity(),
            value_count: self.values.len(),
            value_capacity: self.values.capacity(),
            interned_value_bytes: self.values.iter().map(|value| value.len()).sum(),
            value_index_entry_count: self.value_index.as_ref().map_or(0, HashMap::len),
            tree_entry_count: self.trees.len(),
            graph_entry_count: self.views.len(),
            token_spans_cache_count: self.token_spans_cache.len(),
            semantic_tokens_cache_count: self.semantic_tokens_cache.len(),
        }
    }

    pub fn create_child(&mut self, parent: NodeId) -> Result<NodeId, CoreError> {
        self.add_child(parent, TreeNode::default())
    }

    pub fn add_child(&mut self, parent: NodeId, raw_child: TreeNode) -> Result<NodeId, CoreError> {
        let sequence_index = {
            let parent_node = self.node(parent)?;
            if parent_node.kind == TreeNodeKind::Sequence {
                Some(parent_node.content.len() as u32)
            } else {
                None
            }
        };

        let mut child = raw_child;
        child.parent = Some(parent);
        child.is_map_key = false;
        child.set_sequence_index(sequence_index);
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
        key.set_sequence_index(None);
        let key_id = self.add(key);

        let mut value = raw_value;
        value.parent = Some(parent);
        value.is_map_key = false;
        value.set_key(Some(key_id));
        value.set_sequence_index(None);
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
        Ok(&self.document_meta_for_node(id)?.filename)
    }

    pub fn file_index_for(&self, id: NodeId) -> Result<i32, CoreError> {
        Ok(self.document_meta_for_node(id)?.file_index)
    }

    pub fn resolved_sem_type_for(&self, id: NodeId) -> Result<Option<SemType>, CoreError> {
        Ok(self.node(id)?.resolved_sem_type())
    }

    pub fn value_for(&self, id: NodeId) -> Result<&str, CoreError> {
        let value_id = self.value_id_for(id)?;
        Ok(self.value_text(value_id))
    }

    pub fn value_string_for(&self, id: NodeId) -> Result<String, CoreError> {
        Ok(self.value_for(id)?.to_owned())
    }

    pub fn set_value(&mut self, id: NodeId, value: impl Into<String>) -> Result<(), CoreError> {
        let value_ref = NodeValueRef::Stored(self.intern_value(value));
        let node = self.node_mut(id)?;
        node.value = value_ref;
        Ok(())
    }

    pub fn clear_value(&mut self, id: NodeId) -> Result<(), CoreError> {
        let empty_value_id = self.empty_value_id();
        let node = self.node_mut(id)?;
        node.value = NodeValueRef::Stored(empty_value_id);
        Ok(())
    }

    pub fn remove_value(&mut self, id: NodeId) -> Result<(), CoreError> {
        let node = self.node_mut(id)?;
        node.value = NodeValueRef::Missing;
        Ok(())
    }

    pub fn guess_tag_from_custom_type(&self, id: NodeId) -> Result<String, CoreError> {
        let node = self.node(id)?;
        Ok(super::tree_node::infer_scalar_tag(node.tag_str(), self.value_for(id)?).to_owned())
    }

    pub fn value_rep_for(&self, id: NodeId) -> Result<super::tree_node::ValueRep, CoreError> {
        match SemType::from_string(&self.guess_tag_from_custom_type(id)?) {
            Some(SemType::Int) => match self.value_for(id)?.parse::<i64>() {
                Ok(v) => Ok(super::tree_node::ValueRep::Int(v)),
                Err(_) => Err(CoreError::ParseMessage {
                    line: 0,
                    column: 0,
                    message: format!(
                        "integer value '{}' out of range for target format",
                        self.value_for(id)?
                    ),
                }),
            },
            Some(SemType::Float) => self
                .value_for(id)?
                .parse::<f64>()
                .map(super::tree_node::ValueRep::Float)
                .map_err(|_| ParseError::InvalidSyntax.into()),
            Some(SemType::Boolean) => Ok(super::tree_node::ValueRep::Boolean(matches!(
                self.value_for(id)?.to_ascii_lowercase().as_str(),
                "true" | "y" | "yes" | "on" | "1"
            ))),
            Some(SemType::Nil) => Ok(super::tree_node::ValueRep::Nil),
            _ => Ok(super::tree_node::ValueRep::Str(self.value_string_for(id)?)),
        }
    }

    pub fn anchor_for(&self, id: NodeId) -> Option<&str> {
        let extra = self.get(id)?.extra()?;
        self.node_extra(extra)?
            .anchor
            .as_deref()
            .map(String::as_str)
    }

    pub fn leading_content_for(&self, id: NodeId) -> Option<&str> {
        let extra = self.get(id)?.extra()?;
        self.node_extra(extra)?
            .leading_content
            .as_deref()
            .map(String::as_str)
    }

    pub fn comments_for(&self, id: NodeId) -> Option<&CommentBlock> {
        let extra = self.get(id)?.extra()?;
        self.node_extra(extra)?.comments.as_ref()
    }

    pub fn head_comment_for(&self, id: NodeId) -> Option<&str> {
        self.comments_for(id)?.head.as_deref().map(String::as_str)
    }

    pub fn line_comment_for(&self, id: NodeId) -> Option<&str> {
        self.comments_for(id)?.line.as_deref().map(String::as_str)
    }

    pub fn foot_comment_for(&self, id: NodeId) -> Option<&str> {
        self.comments_for(id)?.foot.as_deref().map(String::as_str)
    }

    pub fn intern_value(&mut self, value: impl Into<String>) -> ValueId {
        self.intern_boxed_value(Box::new(value.into()))
    }

    fn intern_boxed_value(&mut self, value: Box<String>) -> ValueId {
        let hash = value_hash(value.as_str());
        if let Some(existing) = self.ensure_value_index().get(&hash).cloned() {
            match existing {
                ValueBucket::One(value_id) => {
                    if self.value_text(value_id) == value.as_str() {
                        return value_id;
                    }
                }
                ValueBucket::Many(value_ids) => {
                    for value_id in value_ids {
                        if self.value_text(value_id) == value.as_str() {
                            return value_id;
                        }
                    }
                }
            }
        }
        let id = ValueId::from_index(self.values.len());
        self.values.push(value.into_boxed_str());
        self.index_value(hash, id);
        id
    }

    fn index_value(&mut self, hash: u64, value: ValueId) {
        self.ensure_value_index_mut()
            .entry(hash)
            .and_modify(|bucket| bucket.push(value))
            .or_insert(ValueBucket::One(value));
    }

    pub fn discard_value_index(&mut self) {
        self.value_index = None;
    }

    fn ensure_value_index(&mut self) -> &HashMap<u64, ValueBucket> {
        if self.value_index.is_none() {
            self.rebuild_value_index();
        }
        self.value_index
            .as_ref()
            .expect("value index should exist after rebuild")
    }

    fn ensure_value_index_mut(&mut self) -> &mut HashMap<u64, ValueBucket> {
        if self.value_index.is_none() {
            self.rebuild_value_index();
        }
        self.value_index
            .as_mut()
            .expect("value index should exist after rebuild")
    }

    fn rebuild_value_index(&mut self) {
        let mut value_index = HashMap::new();
        for (index, value) in self.values.iter().enumerate() {
            value_index
                .entry(value_hash(value))
                .and_modify(|bucket: &mut ValueBucket| bucket.push(ValueId::from_index(index)))
                .or_insert(ValueBucket::One(ValueId::from_index(index)));
        }
        self.value_index = Some(value_index);
    }

    pub fn parsed_key_for(&self, id: NodeId) -> Result<Option<ParsedKey>, CoreError> {
        let node = self.node(id)?;
        if node.is_map_key {
            return Ok(Some(ParsedKey::Str(self.value_string_for(id)?)));
        }
        if let Some(key_id) = node.key() {
            let key_node = self.node(key_id)?;
            if key_node.resolved_sem_type() == Some(SemType::Str) {
                return Ok(Some(ParsedKey::Str(self.value_string_for(key_id)?)));
            }
            return Ok(Some(match self.value_for(key_id)?.parse::<i64>() {
                Ok(index) => ParsedKey::Int(index),
                Err(_) => ParsedKey::Str(self.value_string_for(key_id)?),
            }));
        }
        if self.sequence_index_for(id)?.is_some() {
            return Ok(node
                .sequence_index()
                .map(|index| ParsedKey::Int(index as i64)));
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
                ParsedKey::Str(value) if index == 0 => out.push_str(&value),
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
        Ok(node.sequence_index().map(|index| index as i64))
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
            if key_node.is_map_key && self.value_for(key_id)? == expected_key {
                return Ok(Some((key_id, value_id)));
            }
            index += 2;
        }
        Ok(None)
    }

    pub fn value_id_for(&self, id: NodeId) -> Result<ValueId, CoreError> {
        Ok(match self.node(id)?.value {
            NodeValueRef::Stored(value_id) => value_id,
            NodeValueRef::Missing => self.empty_value_id(),
            NodeValueRef::Inline(_) => {
                return Err(CoreError::Eval(EvalError::MissingTreeNode));
            }
        })
    }

    pub fn value_ref_for(&self, id: NodeId) -> Result<Option<ValueId>, CoreError> {
        Ok(match self.node(id)?.value {
            NodeValueRef::Stored(value_id) => Some(value_id),
            NodeValueRef::Missing => None,
            NodeValueRef::Inline(_) => {
                return Err(CoreError::Eval(EvalError::MissingTreeNode));
            }
        })
    }

    fn value_text(&self, value_id: ValueId) -> &str {
        self.values
            .get(value_id.index())
            .map(|value| value.as_ref())
            .unwrap_or_default()
    }

    fn empty_value_id(&self) -> ValueId {
        ValueId(0)
    }
}

fn value_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
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
