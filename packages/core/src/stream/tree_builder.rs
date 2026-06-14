use std::collections::HashMap;

use super::tree_patch::TreePatch;
use crate::core::{
    CoreError, NodeId, ParseError, SemType, StructuralSpanIndex, TreeNode, TreeNodeKind, TreeStore,
};
use crate::formats::DecodedDocument;

use super::streaming_events::{EventSink, Meta, StreamingEvent};

#[derive(Debug, Clone)]
pub struct Builder {
    store: TreeStore,
    root: Option<NodeId>,
    stack: Vec<Frame>,
    current_document: u32,
    current_filename: String,
    current_file_index: i32,
    /// Collects parse errors encountered during event processing instead of
    /// ignores `parse_error` events.
    parse_errors: Vec<String>,
    /// Tracks anchor names to their node IDs for duplicate detection and
    /// alias value resolution.
    anchors: HashMap<String, NodeId>,
    /// Optional buffer for collecting TreePatch events during push().
    /// When Some, every structural push also records the corresponding patch.
    /// Drained with take_patches().
    patch_buffer: Option<Vec<TreePatch>>,
    span_index: StructuralSpanIndex,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            store: TreeStore::default(),
            root: None,
            stack: Vec::new(),
            current_document: 0,
            current_filename: String::new(),
            current_file_index: 0,
            parse_errors: Vec::new(),
            anchors: HashMap::new(),
            patch_buffer: None,
            span_index: StructuralSpanIndex::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    id: NodeId,
    kind: TreeNodeKind,
    pending_key: Option<NodeId>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable patch recording mode. After calling this, every `push()` also
    /// appends a `TreePatch` to an internal buffer.
    pub fn enable_patches(&mut self) {
        self.patch_buffer = Some(Vec::new());
    }

    /// Drain and return all patches recorded since the last call to
    /// `take_patches()` (or `enable_patches()`).
    pub fn take_patches(&mut self) -> Vec<TreePatch> {
        self.patch_buffer
            .as_mut()
            .map(|buf| std::mem::take(buf))
            .unwrap_or_default()
    }

    /// Consumes the builder and returns the constructed document.
    /// Returns an error if no root was built or if parse errors were collected.
    pub fn into_document(self) -> Result<DecodedDocument, CoreError> {
        let Some(root) = self.root else {
            return Err(CoreError::Parse(ParseError::InvalidSyntax));
        };
        Ok(DecodedDocument::new(self.store, root))
    }
    /// Like [`into_document`](Self::into_document), but works on a `&mut self`
    /// reference by taking the internal state, leaving a default empty builder.
    pub fn take_document(&mut self) -> Result<DecodedDocument, CoreError> {
        let root = self
            .root
            .take()
            .ok_or(CoreError::Parse(ParseError::InvalidSyntax))?;
        let store = std::mem::take(&mut self.store);
        Ok(DecodedDocument::new(store, root))
    }
    pub fn parse_errors(&self) -> &[String] {
        &self.parse_errors
    }

    /// Returns the incrementally-maintained structural span index.
    pub fn span_index(&self) -> &StructuralSpanIndex {
        &self.span_index
    }

    /// Called after a scalar node is attached (currently a no-op but kept
    /// for future sequence-level bookkeeping if needed).
    fn record_scalar_path(&mut self, _id: NodeId) {}

    /// Returns a read-only reference to the current partial tree state
    /// without cloning. Use this instead of `snapshot_tree()` when the
    /// caller only needs read access during streaming chunk processing.
    pub fn tree_ref(&self) -> Option<(&TreeStore, NodeId)> {
        self.root.map(|root| (&self.store, root))
    }
    /// Clone the current partial tree state without consuming the builder.
    /// Returns the cloned store and root if a root has been established.
    pub fn snapshot_tree(&self) -> Option<(TreeStore, NodeId)> {
        self.root.map(|root| (self.store.clone(), root))
    }

    pub fn push(&mut self, event: &StreamingEvent) -> Result<(), CoreError> {
        match event {
            StreamingEvent::DocStart(meta) => {
                self.current_document = meta.document;
                self.current_filename = meta.filename.clone();
                self.current_file_index = meta.file_index;
                Ok(())
            }
            StreamingEvent::DocEnd(_) => Ok(()),
            StreamingEvent::MapStart(meta) => {
                let id =
                    self.store
                        .add(self.container_node(TreeNodeKind::Mapping, SemType::Map, meta));
                self.register_anchor(id, meta);
                self.attach_value(id)?;
                self.stack.push(Frame {
                    id,
                    kind: TreeNodeKind::Mapping,
                    pending_key: None,
                });
                self.emit_patch(TreePatch::NodeInserted {
                    node_id: id,
                    parent: None,
                    key: None,
                    sequence_index: None,
                    kind: TreeNodeKind::Mapping as i32,
                    sem_type: SemType::Map as i32,
                    tag: tag_or_default(meta, SemType::Map),
                    value: String::new(),
                });
                Ok(())
            }
            StreamingEvent::SeqStart(meta) => {
                let id =
                    self.store
                        .add(self.container_node(TreeNodeKind::Sequence, SemType::Seq, meta));
                self.register_anchor(id, meta);
                self.attach_value(id)?;
                self.stack.push(Frame {
                    id,
                    kind: TreeNodeKind::Sequence,
                    pending_key: None,
                });
                self.emit_patch(TreePatch::NodeInserted {
                    node_id: id,
                    parent: None,
                    key: None,
                    sequence_index: None,
                    kind: TreeNodeKind::Sequence as i32,
                    sem_type: SemType::Seq as i32,
                    tag: tag_or_default(meta, SemType::Seq),
                    value: String::new(),
                });
                Ok(())
            }
            StreamingEvent::MapKey { value, meta } => {
                let parent_id = {
                    let Some(frame) = self.stack.last() else {
                        return Err(CoreError::Parse(ParseError::InvalidSyntax));
                    };
                    if frame.kind != TreeNodeKind::Mapping || frame.pending_key.is_some() {
                        return Err(CoreError::Parse(ParseError::InvalidSyntax));
                    }
                    frame.id
                };
                let mut key = self.scalar_node(SemType::Str, value, meta);
                key.is_map_key = true;
                key.parent = Some(parent_id);
                let key_id = self.store.add(key);
                if let Some(frame) = self.stack.last_mut() {
                    frame.pending_key = Some(key_id);
                }
                self.emit_patch(TreePatch::KeyInserted {
                    key_id,
                    parent: parent_id,
                    key_text: value.clone(),
                    tag: tag_or_default(meta, SemType::Str),
                });
                Ok(())
            }
            StreamingEvent::MapEnd(meta) => {
                let frame = self
                    .stack
                    .pop()
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?;
                if frame.kind != TreeNodeKind::Mapping || frame.pending_key.is_some() {
                    return Err(CoreError::Parse(ParseError::InvalidSyntax));
                }
                self.update_end_byte(frame.id, meta.end_byte)?;
                self.emit_patch(TreePatch::NodeSealed { node_id: frame.id });
                Ok(())
            }
            StreamingEvent::SeqEnd(meta) => {
                let frame = self
                    .stack
                    .pop()
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?;
                if frame.kind != TreeNodeKind::Sequence {
                    return Err(CoreError::Parse(ParseError::InvalidSyntax));
                }
                self.update_end_byte(frame.id, meta.end_byte)?;
                self.emit_patch(TreePatch::NodeSealed { node_id: frame.id });
                Ok(())
            }
            StreamingEvent::Scalar { value, meta } => {
                let sem_type = meta.sem_type.unwrap_or(SemType::Str);
                let id = self.store.add(self.scalar_node(
                    sem_type,
                    &scalar_value(value, sem_type),
                    meta,
                ));
                self.register_anchor(id, meta);
                self.attach_value(id)?;
                self.record_scalar_path(id);
                self.span_index
                    .insert_scalar(id, meta.start_byte, meta.end_byte);
                self.emit_patch(TreePatch::NodeInserted {
                    node_id: id,
                    parent: None,
                    key: None,
                    sequence_index: None,
                    kind: TreeNodeKind::Scalar as i32,
                    sem_type: sem_type as i32,
                    tag: tag_or_default(meta, sem_type),
                    value: scalar_value(value, sem_type),
                });
                // scalars are implicitly sealed
                self.emit_patch(TreePatch::NodeSealed { node_id: id });
                Ok(())
            }
            StreamingEvent::Alias { anchor, meta } => {
                // Resolve alias: look up the anchor to copy the target's value.
                let resolved_value = self
                    .anchors
                    .get(anchor.as_str())
                    .and_then(|&target_id| self.store.get(target_id))
                    .map(|target| target.value.clone())
                    .unwrap_or_default();

                let node = self.store.add(TreeNode {
                    kind: TreeNodeKind::Alias,
                    sem_type: None,
                    tag: meta.tag.clone(),
                    value: resolved_value,
                    start_byte: meta.start_byte,
                    end_byte: meta.end_byte,
                    anchor: anchor.clone(),
                    head_comment: meta.head_comment.clone(),
                    line_comment: meta.line_comment.clone(),
                    foot_comment: meta.foot_comment.clone(),
                    document: self.meta_document(meta),
                    filename: self.meta_filename(meta),
                    line: meta.line,
                    column: meta.column,
                    file_index: self.meta_file_index(meta),
                    ..TreeNode::default()
                });
                self.attach_value(node)
            }
            StreamingEvent::ParseError { message, .. } => {
                // Collect parse errors gracefully instead of aborting,
                self.parse_errors.push(message.clone());
                self.emit_patch(TreePatch::DiagnosticAdded {
                    message: message.clone(),
                    line: 0,
                    column: 0,
                    byte_offset: None,
                });
                Ok(())
            }
        }
    }

    /// Record a patch if patch mode is enabled.
    fn emit_patch(&mut self, patch: TreePatch) {
        if let Some(buf) = self.patch_buffer.as_mut() {
            buf.push(patch);
        }
    }

    /// Registers a node's anchor for duplicate detection and alias resolution.
    /// If the same anchor name is seen again, logs it as a parse error.
    fn register_anchor(&mut self, node_id: NodeId, meta: &Meta) {
        if meta.anchor.is_empty() {
            return;
        }
        if self.anchors.contains_key(meta.anchor.as_str()) {
            self.parse_errors.push(format!(
                "duplicate anchor '{}' at line {} column {}",
                meta.anchor, meta.line, meta.column
            ));
        }
        self.anchors.insert(meta.anchor.clone(), node_id);
    }

    fn attach_value(&mut self, node: NodeId) -> Result<(), CoreError> {
        let Some(frame) = self.stack.last().copied() else {
            if self.root.is_some() {
                return Err(CoreError::Parse(ParseError::InvalidSyntax));
            }
            self.root = Some(node);
            return Ok(());
        };
        match frame.kind {
            TreeNodeKind::Sequence => {
                let sequence_index = self
                    .store
                    .get(frame.id)
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?
                    .content
                    .len() as i64;
                let value = self
                    .store
                    .get_mut(node)
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?;
                value.parent = Some(frame.id);
                value.sequence_index = Some(sequence_index);
                self.store
                    .get_mut(frame.id)
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?
                    .content
                    .push(node);
                Ok(())
            }
            TreeNodeKind::Mapping => {
                let pending_key = self
                    .stack
                    .last_mut()
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?
                    .pending_key
                    .take()
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?;
                {
                    let value = self
                        .store
                        .get_mut(node)
                        .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?;
                    value.parent = Some(frame.id);
                    value.key = Some(pending_key);
                    value.sequence_index = None;
                }
                let map = self
                    .store
                    .get_mut(frame.id)
                    .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?;
                map.content.push(pending_key);
                map.content.push(node);
                Ok(())
            }
            _ => Err(CoreError::Parse(ParseError::InvalidSyntax)),
        }
    }

    fn container_node(&self, kind: TreeNodeKind, sem_type: SemType, meta: &Meta) -> TreeNode {
        TreeNode {
            kind,
            sequence_closed: kind != TreeNodeKind::Sequence,
            sem_type: Some(sem_type),
            tag: tag_or_default(meta, sem_type),
            start_byte: meta.start_byte,
            end_byte: meta.end_byte,
            anchor: meta.anchor.clone(),
            head_comment: meta.head_comment.clone(),
            line_comment: meta.line_comment.clone(),
            foot_comment: meta.foot_comment.clone(),
            document: self.meta_document(meta),
            filename: self.meta_filename(meta),
            line: meta.line,
            column: meta.column,
            file_index: self.meta_file_index(meta),
            ..TreeNode::default()
        }
    }

    fn scalar_node(&self, sem_type: SemType, value: &str, meta: &Meta) -> TreeNode {
        TreeNode {
            kind: TreeNodeKind::Scalar,
            sem_type: Some(sem_type),
            tag: tag_or_default(meta, sem_type),
            value: value.to_owned(),
            start_byte: meta.start_byte,
            end_byte: meta.end_byte,
            anchor: meta.anchor.clone(),
            head_comment: meta.head_comment.clone(),
            line_comment: meta.line_comment.clone(),
            foot_comment: meta.foot_comment.clone(),
            document: self.meta_document(meta),
            filename: self.meta_filename(meta),
            line: meta.line,
            column: meta.column,
            file_index: self.meta_file_index(meta),
            ..TreeNode::default()
        }
    }

    fn update_end_byte(&mut self, node: NodeId, end_byte: u32) -> Result<(), CoreError> {
        let node_ref = self
            .store
            .get_mut(node)
            .ok_or_else(|| CoreError::Parse(ParseError::InvalidSyntax))?;
        node_ref.end_byte = end_byte;
        if node_ref.kind == TreeNodeKind::Sequence {
            node_ref.sequence_closed = true;
        }
        Ok(())
    }

    fn meta_document(&self, meta: &Meta) -> u32 {
        if meta.document == 0 {
            self.current_document
        } else {
            meta.document
        }
    }

    fn meta_filename(&self, meta: &Meta) -> String {
        if meta.filename.is_empty() {
            self.current_filename.clone()
        } else {
            meta.filename.clone()
        }
    }

    fn meta_file_index(&self, meta: &Meta) -> i32 {
        if meta.file_index == 0 {
            self.current_file_index
        } else {
            meta.file_index
        }
    }
}

impl EventSink for Builder {
    type Error = CoreError;

    fn emit(&mut self, event: StreamingEvent) -> Result<(), Self::Error> {
        self.push(&event)
    }
}

pub fn decode_events(events: &[StreamingEvent]) -> Result<DecodedDocument, CoreError> {
    let mut builder = Builder::new();
    for event in events {
        builder.push(event)?;
    }
    builder.into_document()
}

fn scalar_value(value: &str, sem_type: SemType) -> String {
    if sem_type == SemType::Nil {
        String::new()
    } else {
        value.to_owned()
    }
}

fn tag_or_default(meta: &Meta, sem_type: SemType) -> String {
    if meta.tag.is_empty() {
        sem_type.to_string()
    } else {
        meta.tag.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{SemType, TreeNodeKind, get_map_entry};
    use crate::stream::decode;

    use super::decode_events;

    #[test]
    fn tree_builder_converts_streaming_json_events_into_tree_store() {
        let events = decode("json", r#"{"name":"Ada","items":[1,true,null]}"#).unwrap();
        let document = decode_events(&events).unwrap();
        let root = document.store.get(document.root).unwrap();
        assert_eq!(root.kind, TreeNodeKind::Mapping);
        let items = get_map_entry(&document.store, document.root, "items")
            .unwrap()
            .unwrap()
            .value;
        assert_eq!(
            document.store.get(items).unwrap().kind,
            TreeNodeKind::Sequence
        );
    }

    #[test]
    fn tree_builder_preserves_scalar_sem_types() {
        let events = decode("json", r#"{"count":1,"active":true,"missing":null}"#).unwrap();
        let document = decode_events(&events).unwrap();
        for (key, expected) in [
            ("count", Some(SemType::Int)),
            ("active", Some(SemType::Boolean)),
            ("missing", Some(SemType::Nil)),
        ] {
            let value = get_map_entry(&document.store, document.root, key)
                .unwrap()
                .unwrap()
                .value;
            assert_eq!(document.store.get(value).unwrap().sem_type, expected);
        }
    }
}
