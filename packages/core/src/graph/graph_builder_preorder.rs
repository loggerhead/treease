use std::collections::{HashSet, VecDeque};

use crate::errors::CoreError;
use crate::operators::compat::SemType as CompatSemType;
use crate::operators::{NodeKind, TreeNode};
use crate::stream::streaming_events::{EventSink, Meta, StreamingEvent};

use super::graph_builder::{
    BuilderConfig, GraphBuilder, GraphCell, GraphEdge, GraphLanguage, GraphModel, GraphNode,
    GraphRow, PathSeg, SequencePresentation, graph_kind_for_node, graph_node_key,
};
use super::graph_builder::shared::sequence_has_header_table;
use crate::tree::{NodeId, TreeNodeKind, TreeStore};

// ---------------------------------------------------------------------------
// GraphDelta -- incremental update payload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphTableCellPatch {
    pub table_render_handle: u32,
    pub row_index: u32,
    pub column_index: u32,
    pub cell: GraphCell,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphDelta {
    pub clear: bool,
    pub nodes_added: Vec<GraphNode>,
    pub nodes_updated: Vec<GraphNode>,
    pub nodes_removed: Vec<u32>,
    pub edges_added: Vec<GraphEdge>,
    pub edges_removed: Vec<GraphEdge>,
    pub table_cell_patches: Vec<GraphTableCellPatch>,
}

// ---------------------------------------------------------------------------
// Frame -- one level of the event stack
// ---------------------------------------------------------------------------

struct Frame {
    /// Index into `node_pool` for the event node.
    node_index: usize,
    node_render_handle: u32,
    path: Vec<PathSeg>,
    /// Index into `node_pool` for the node that "owns" the view.
    view_source_index: usize,
    view_path: Vec<PathSeg>,
    depth: u32,
    owns_view: bool,
    row_index: i32,
    next_child_y: i32,
    subtree_bottom: i32,
    pending_key_value: Option<String>,
}

// ---------------------------------------------------------------------------
// Builder -- event-driven preorder graph builder
// ---------------------------------------------------------------------------

pub struct Builder {
    config: BuilderConfig,
    view: GraphBuilder,
    pub root_offset_x: i32,
    pub root_offset_y: i32,
    /// All created TreeNodes (node pool).
    node_pool: Vec<TreeNode>,
    /// All graph nodes indexed by render_handle.
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    /// Pending delta accumulators.
    pending_nodes: Vec<GraphNode>,
    pending_nodes_updated: Vec<GraphNode>,
    pending_edges_added: Vec<GraphEdge>,
    pending_edges_removed: Vec<GraphEdge>,
    /// Deferred refresh queue (render_handles) plus an in-queue dedupe set.
    deferred_refresh_nodes: VecDeque<u32>,
    deferred_refresh_set: HashSet<u32>,
    /// Event frame stack.
    stack: Vec<Frame>,
    /// Reverse index: node_render_handle -> outgoing edge indices.
    edges_from_index: Vec<Vec<usize>>,
    /// Reverse index: node_render_handle -> incoming edge indices.
    edges_to_index: Vec<Vec<usize>>,
    root_render_handle: Option<u32>,
    pub release_view_data_on_cleanup: bool,
}

impl EventSink for Builder {
    type Error = CoreError;

    fn emit(&mut self, event: StreamingEvent) -> Result<(), Self::Error> {
        self.on_event(&event).map_err(CoreError::Io)
    }
}

impl Builder {
    // ------------------------------------------------------------------
    // Construction / teardown
    // ------------------------------------------------------------------

    pub fn new(config: BuilderConfig, language: GraphLanguage) -> Self {
        Self {
            view: GraphBuilder::new(config.clone(), language),
            root_offset_x: 0,
            root_offset_y: 0,
            config,
            node_pool: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            pending_nodes: Vec::new(),
            pending_nodes_updated: Vec::new(),
            pending_edges_added: Vec::new(),
            pending_edges_removed: Vec::new(),
            deferred_refresh_nodes: VecDeque::new(),
            deferred_refresh_set: HashSet::new(),
            stack: Vec::new(),
            edges_from_index: Vec::new(),
            edges_to_index: Vec::new(),
            root_render_handle: None,
            release_view_data_on_cleanup: true,
        }
    }

    /// Current count of graph nodes, useful for determining batch root handles.
    pub fn node_count(&self) -> u32 {
        self.nodes.len() as u32
    }
    pub fn reset(&mut self) {
        if self.release_view_data_on_cleanup {
            self.free_all_node_owned_data();
        }
        self.nodes.clear();
        self.edges.clear();
        self.pending_nodes.clear();
        self.pending_nodes_updated.clear();
        self.pending_edges_added.clear();
        self.pending_edges_removed.clear();
        self.deferred_refresh_nodes.clear();
        self.deferred_refresh_set.clear();
        self.stack.clear();
        Self::clear_edge_index(&mut self.edges_from_index);
        Self::clear_edge_index(&mut self.edges_to_index);
        self.node_pool.clear();
        self.root_render_handle = None;
    }

    fn free_all_node_owned_data(&self) {
        for node in &self.nodes {
            self.view.free_node_owned_data(node, None);
        }
    }

    // ------------------------------------------------------------------
    // Edge index helpers
    // ------------------------------------------------------------------

    fn clear_edge_index(index: &mut Vec<Vec<usize>>) {
        for bucket in index.iter_mut() {
            bucket.clear();
        }
    }

    fn ensure_edge_bucket(index: &mut Vec<Vec<usize>>, node_render_handle: u32) -> &mut Vec<usize> {
        let idx = node_render_handle as usize;
        while index.len() <= idx {
            index.push(Vec::new());
        }
        &mut index[idx]
    }

    fn register_edge_index(&mut self, edge_index: usize, edge: &GraphEdge) {
        let bucket = Self::ensure_edge_bucket(&mut self.edges_from_index, edge.from_render_handle);
        bucket.push(edge_index);
        let bucket = Self::ensure_edge_bucket(&mut self.edges_to_index, edge.to_render_handle);
        bucket.push(edge_index);
    }

    fn unregister_last_edge_index(&mut self, edge_index: usize, edge: &GraphEdge) {
        let from_idx = edge.from_render_handle as usize;
        if from_idx < self.edges_from_index.len() {
            let bucket = &mut self.edges_from_index[from_idx];
            if let Some(pos) = bucket.iter().rposition(|&i| i == edge_index) {
                bucket.remove(pos);
            }
        }
        let to_idx = edge.to_render_handle as usize;
        if to_idx < self.edges_to_index.len() {
            let bucket = &mut self.edges_to_index[to_idx];
            if let Some(pos) = bucket.iter().rposition(|&i| i == edge_index) {
                bucket.remove(pos);
            }
        }
    }

    // ------------------------------------------------------------------
    // Event dispatch
    // ------------------------------------------------------------------

    pub fn on_event(&mut self, ev: &StreamingEvent) -> Result<(), String> {
        match ev {
            StreamingEvent::DocStart(_) | StreamingEvent::DocEnd(_) => Ok(()),
            StreamingEvent::MapStart(meta) => self.on_map_start(meta),
            StreamingEvent::MapKey { value, meta: _ } => self.on_map_key(value),
            StreamingEvent::MapEnd(_) => self.on_map_end(),
            StreamingEvent::SeqStart(meta) => self.on_seq_start(meta),
            StreamingEvent::SeqEnd(_) => self.on_seq_end(),
            StreamingEvent::Scalar { value, meta } => self.on_scalar(value, meta),
            StreamingEvent::Alias { anchor, meta } => self.on_alias(anchor, meta),
            StreamingEvent::ParseError { .. } => Ok(()),
        }
    }

    // ------------------------------------------------------------------
    // Flush / finish
    // ------------------------------------------------------------------

    pub fn flush(&mut self) -> Result<GraphDelta, String> {
        self.apply_deferred_refreshes()?;
        let delta = GraphDelta {
            clear: false,
            nodes_added: self.pending_nodes.clone(),
            nodes_updated: self.pending_nodes_updated.clone(),
            nodes_removed: Vec::new(),
            edges_added: self.pending_edges_added.clone(),
            edges_removed: self.pending_edges_removed.clone(),
            table_cell_patches: Vec::new(),
        };
        self.pending_nodes.clear();
        self.pending_nodes_updated.clear();
        self.pending_edges_added.clear();
        self.pending_edges_removed.clear();
        Ok(delta)
    }

    pub fn finish(&mut self) -> Result<GraphModel, String> {
        self.apply_deferred_refreshes()?;
        self.release_view_data_on_cleanup = false;
        let mut model = GraphModel {
            nodes: std::mem::take(&mut self.nodes),
            edges: std::mem::take(&mut self.edges),
            ..Default::default()
        };
        model.rebuild_edge_index();
        let root_render_handle = self.root_render_handle.unwrap_or(0);
        crate::layout::layout_engine::LayoutEngine::new(self.config.clone())
            .layout_full(&mut model, root_render_handle);
        model.rebuild_edge_index();
        Ok(model)
    }

    pub fn finish_delta(&mut self) -> Result<GraphDelta, String> {
        self.flush()
    }

    /// Convenience: do a full rebuild from a TreeNode by walking the tree
    pub fn emit_tree_store_preorder_stack(
        &mut self,
        store: &TreeStore,
        root: NodeId,
    ) -> Result<(), String> {
        self.emit_tree_store_preorder_stack_with_root_path(store, root, &[])
    }

    pub fn emit_tree_store_preorder_stack_with_root_path(
        &mut self,
        store: &TreeStore,
        root: NodeId,
        root_path: &[PathSeg],
    ) -> Result<(), String> {
        enum EmitFrame {
            Visit(NodeId),
            MapKey { value: String },
            MapEnd,
            SeqEnd,
        }

        self.on_event(&StreamingEvent::DocStart(Meta::default()))?;

        let mut stack = vec![EmitFrame::Visit(root)];
        while let Some(frame) = stack.pop() {
            match frame {
                EmitFrame::Visit(id) => {
                    let node = store
                        .get(id)
                        .ok_or_else(|| format!("missing tree-store node {}", id.0))?;
                    let meta = self.tree_store_node_meta(
                        store,
                        id,
                        if id == root { root_path } else { &[] },
                    );
                    match node.kind {
                        TreeNodeKind::Mapping => {
                            self.on_event(&StreamingEvent::MapStart(meta))?;
                            stack.push(EmitFrame::MapEnd);
                            let mut i = node.content.len();
                            while i > 1 {
                                let key_id = node.content[i - 2];
                                let value_id = node.content[i - 1];
                                let _key_node = store.get(key_id).ok_or_else(|| {
                                    format!("missing tree-store node {}", key_id.0)
                                })?;
                                stack.push(EmitFrame::Visit(value_id));
                                stack.push(EmitFrame::MapKey {
                                    value: store
                                        .value_string_for(key_id)
                                        .map_err(|_| format!("missing key value {}", key_id.0))?,
                                });
                                i -= 2;
                            }
                        }
                        TreeNodeKind::Sequence => {
                            self.on_event(&StreamingEvent::SeqStart(meta))?;
                            stack.push(EmitFrame::SeqEnd);
                            let mut i = node.content.len();
                            while i > 0 {
                                i -= 1;
                                stack.push(EmitFrame::Visit(node.content[i]));
                            }
                        }
                        TreeNodeKind::Scalar => {
                            self.on_event(&StreamingEvent::Scalar {
                                value: store
                                    .value_string_for(id)
                                    .map_err(|_| format!("missing scalar value {}", id.0))?,
                                meta,
                            })?;
                        }
                        TreeNodeKind::Alias => {
                            self.on_event(&StreamingEvent::Alias {
                                anchor: store.anchor_for(id).unwrap_or_default().to_owned(),
                                meta,
                            })?;
                        }
                        TreeNodeKind::Unknown => {}
                    }
                }
                EmitFrame::MapKey { value } => {
                    self.on_event(&StreamingEvent::MapKey {
                        value,
                        meta: Meta::default(),
                    })?;
                }
                EmitFrame::MapEnd => {
                    self.on_event(&StreamingEvent::MapEnd(Meta::default()))?;
                }
                EmitFrame::SeqEnd => {
                    self.on_event(&StreamingEvent::SeqEnd(Meta::default()))?;
                }
            }
        }

        self.on_event(&StreamingEvent::DocEnd(Meta::default()))?;
        Ok(())
    }

    /// and emitting synthetic events. Returns the resulting GraphDelta.
    pub fn build_from_tree(&mut self, root: &TreeNode) -> Result<GraphDelta, String> {
        self.reset();
        self.emit_tree_preorder(root, &[])?;
        self.finish_delta()
    }

    /// Walk a TreeNode in preorder, emitting synthetic StreamingEvents.
    fn emit_tree_preorder(&mut self, node: &TreeNode, path: &[PathSeg]) -> Result<(), String> {
        let meta = self.tree_node_meta(node, path);
        match node.kind {
            NodeKind::Mapping => {
                self.on_map_start(&meta)?;
                let mut i = 0;
                while i + 1 < node.content.len() {
                    let key_node = &node.content[i];
                    let value_node = &node.content[i + 1];
                    let key_path = self.append_path_vec(path, PathSeg::Key(key_node.value.clone()));
                    self.on_map_key(&key_node.value)?;
                    self.emit_tree_preorder(value_node, &key_path)?;
                    i += 2;
                }
                self.on_map_end()?;
            }
            NodeKind::Sequence => {
                self.on_seq_start(&meta)?;
                for (idx, child) in node.content.iter().enumerate() {
                    let child_path = self.append_path_vec(path, PathSeg::Index(idx));
                    self.emit_tree_preorder(child, &child_path)?;
                }
                self.on_seq_end()?;
            }
            NodeKind::Scalar => {
                let mut scalar_meta = meta;
                scalar_meta.sem_type = node.sem_type.map(Into::into);
                self.on_scalar(&node.value, &scalar_meta)?;
            }
            NodeKind::Alias => {
                let mut alias_meta = meta;
                alias_meta.sem_type = node.sem_type.map(Into::into);
                self.on_alias(&node.anchor, &alias_meta)?;
            }
            NodeKind::Unknown => {}
        }
        Ok(())
    }

    /// Build a Meta from a TreeNode for synthetic event emission.
    fn tree_node_meta(&self, node: &TreeNode, _path: &[PathSeg]) -> Meta {
        Meta {
            tag: node.tag.clone(),
            sem_type: node.sem_type.map(Into::into),
            start_byte: node.start_byte,
            end_byte: node.end_byte,
            line: node.line,
            column: node.column,
            path: String::new(),
            path_supplier: None,
            document: node.document,
            filename: node.filename.clone(),
            file_index: node.file_index,
            anchor: node.anchor.clone(),
            head_comment: node.head_comment.clone(),
            line_comment: node.line_comment.clone(),
            foot_comment: node.foot_comment.clone(),
        }
    }

    fn tree_store_node_meta(&self, store: &TreeStore, id: NodeId, path: &[PathSeg]) -> Meta {
        let node = store
            .get(id)
            .unwrap_or_else(|| panic!("missing tree-store node {}", id.0));
        Meta {
            tag: node.tag.to_string_value(),
            sem_type: node.sem_type.map(Into::into),
            start_byte: node.start_byte,
            end_byte: node.end_byte,
            line: node.line,
            column: node.column,
            path: format_graph_path(path),
            path_supplier: None,
            document: node.document,
            filename: store.filename_for(id).unwrap_or_default().to_owned(),
            file_index: store.file_index_for(id).unwrap_or_default(),
            anchor: store.anchor_for(id).unwrap_or_default().to_owned(),
            head_comment: store.head_comment_for(id).unwrap_or_default().to_owned(),
            line_comment: store.line_comment_for(id).unwrap_or_default().to_owned(),
            foot_comment: store.foot_comment_for(id).unwrap_or_default().to_owned(),
        }
    }

    /// Return a reference to the current nodes (for external inspection).
    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    /// Return a reference to the current edges (for external inspection).
    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }

    // ------------------------------------------------------------------
    // Event handlers
    // ------------------------------------------------------------------

    fn on_map_start(&mut self, meta: &Meta) -> Result<(), String> {
        let node = self.create_node(NodeKind::Mapping, meta, "");
        let node_index = self.node_pool.len();
        self.node_pool.push(node);
        // Set sem_type
        self.node_pool[node_index].sem_type = Some(CompatSemType::Map);
        let path = self.resolve_event_path(meta)?;
        let view_node = self.append_node(node_index, &path, true)?;
        if let Some(rendered) = view_node {
            let view_clone = self.nodes[rendered as usize].clone();
            self.push_frame(node_index, &view_clone, path);
        } else if let Some(frame) = self.current_frame() {
            let parent_view_source = frame.view_source_index;
            let parent_view_path = frame.view_path.clone();
            let parent_depth = frame.depth;
            let parent_render_handle = frame.node_render_handle;
            self.push_hidden_frame(
                node_index,
                path,
                parent_view_source,
                parent_view_path,
                parent_depth,
                parent_render_handle,
            );
        } else {
            return Err("InvalidSyntax: map_start without parent".into());
        }
        Ok(())
    }

    fn on_map_key(&mut self, value: &str) -> Result<(), String> {
        if let Some(frame) = self.current_frame_mut() {
            frame.pending_key_value = Some(value.to_string());
        }
        Ok(())
    }

    fn on_map_end(&mut self) -> Result<(), String> {
        if let Some(frame) = self.current_frame_mut() {
            frame.pending_key_value = None;
        }
        let finished = self
            .pop_frame()
            .ok_or("InvalidSyntax: map_end without frame")?;
        self.sync_finished_child_to_parent(&finished);
        if finished.owns_view && self.try_drop_inline_container(&finished)? {
            return Ok(());
        }
        if finished.owns_view {
            let refreshed_bottom = self.refresh_frame_view(
                finished.node_render_handle,
                finished.view_source_index,
                &finished.view_path,
                finished.depth,
            )?;
            self.adjust_parent_next_child_y(finished.subtree_bottom.max(refreshed_bottom));
        }
        if let Some(parent) = self.current_frame() {
            let parent_handle = parent.node_render_handle;
            self.queue_deferred_refresh(parent_handle)?;
        }
        Ok(())
    }

    fn on_seq_start(&mut self, meta: &Meta) -> Result<(), String> {
        let mut node = self.create_node(NodeKind::Sequence, meta, "");
        node.sem_type = Some(CompatSemType::Seq);
        node.sequence_closed = false;
        let node_index = self.node_pool.len();
        self.node_pool.push(node);
        let path = self.resolve_event_path(meta)?;
        let view_node = self.append_node(node_index, &path, true)?;
        if let Some(rendered) = view_node {
            let view_clone = self.nodes[rendered as usize].clone();
            self.push_frame(node_index, &view_clone, path);
        } else if let Some(frame) = self.current_frame() {
            let parent_view_source = frame.view_source_index;
            let parent_view_path = frame.view_path.clone();
            let parent_depth = frame.depth;
            let parent_render_handle = frame.node_render_handle;
            self.push_hidden_frame(
                node_index,
                path,
                parent_view_source,
                parent_view_path,
                parent_depth,
                parent_render_handle,
            );
        } else {
            return Err("InvalidSyntax: seq_start without parent".into());
        }
        Ok(())
    }

    fn on_seq_end(&mut self) -> Result<(), String> {
        let finished = self
            .pop_frame()
            .ok_or("InvalidSyntax: seq_end without frame")?;
        self.node_pool[finished.node_index].sequence_closed = true;
        self.sync_finished_child_to_parent(&finished);
        if finished.owns_view && self.try_drop_inline_container(&finished)? {
            return Ok(());
        }
        if finished.owns_view {
            let refreshed_bottom = self.refresh_frame_view(
                finished.node_render_handle,
                finished.view_source_index,
                &finished.view_path,
                finished.depth,
            )?;
            self.adjust_parent_next_child_y(finished.subtree_bottom.max(refreshed_bottom));
        }
        if let Some(parent) = self.current_frame() {
            let parent_handle = parent.node_render_handle;
            self.queue_deferred_refresh(parent_handle)?;
        }
        Ok(())
    }

    fn on_scalar(&mut self, value: &str, meta: &Meta) -> Result<(), String> {
        let mut node = self.create_node(NodeKind::Scalar, meta, value);
        node.sem_type = meta
            .sem_type
            .map(CompatSemType::from)
            .or(Some(CompatSemType::Str));
        let node_index = self.node_pool.len();
        self.node_pool.push(node);
        let path = self.resolve_event_path(meta)?;
        if let Some((parent_node_index, parent_handle, parent_kind)) =
            self.current_frame().map(|parent| {
                (
                    parent.node_index,
                    parent.node_render_handle,
                    self.node_pool[parent.node_index].kind,
                )
            })
        {
            if parent_kind == NodeKind::Mapping || parent_kind == NodeKind::Sequence {
                self.attach_to_parent(parent_node_index, node_index)?;
                // Update parent frame
                if let Some(parent_frame) = self.current_frame_mut() {
                    parent_frame.row_index += 1;
                }
                self.queue_deferred_refresh(parent_handle)?;
                return Ok(());
            }
        }
        self.append_node(node_index, &path, false)?;
        Ok(())
    }

    fn on_alias(&mut self, anchor: &str, meta: &Meta) -> Result<(), String> {
        let mut node = self.create_node(NodeKind::Alias, meta, "");
        node.sem_type = meta
            .sem_type
            .map(CompatSemType::from)
            .or(Some(CompatSemType::Str));
        node.anchor = anchor.to_string();
        let node_index = self.node_pool.len();
        self.node_pool.push(node);
        let path = self.resolve_event_path(meta)?;
        if let Some((parent_node_index, parent_handle, parent_kind)) =
            self.current_frame().map(|parent| {
                (
                    parent.node_index,
                    parent.node_render_handle,
                    self.node_pool[parent.node_index].kind,
                )
            })
        {
            if parent_kind == NodeKind::Mapping || parent_kind == NodeKind::Sequence {
                self.attach_to_parent(parent_node_index, node_index)?;
                if let Some(parent_frame) = self.current_frame_mut() {
                    parent_frame.row_index += 1;
                }
                self.queue_deferred_refresh(parent_handle)?;
                return Ok(());
            }
        }
        self.append_node(node_index, &path, false)?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Node creation
    // ------------------------------------------------------------------

    fn create_node(&self, kind: NodeKind, meta: &Meta, value: &str) -> TreeNode {
        TreeNode {
            kind,
            sem_type: meta.sem_type.map(CompatSemType::from),
            tag: meta.tag.clone(),
            value: value.to_string(),
            start_byte: meta.start_byte,
            end_byte: meta.end_byte,
            anchor: meta.anchor.clone(),
            head_comment: meta.head_comment.clone(),
            line_comment: meta.line_comment.clone(),
            foot_comment: meta.foot_comment.clone(),
            document: meta.document,
            filename: meta.filename.clone(),
            line: meta.line,
            column: meta.column,
            file_index: meta.file_index,
            ..TreeNode::default()
        }
    }

    // ------------------------------------------------------------------
    // Frame stack
    // ------------------------------------------------------------------

    fn push_frame(&mut self, node_index: usize, view_node: &GraphNode, path: Vec<PathSeg>) {
        let depth = view_node.depth;
        self.stack.push(Frame {
            node_index,
            node_render_handle: view_node.render_handle,
            path: path.clone(),
            view_source_index: node_index,
            view_path: path,
            depth,
            owns_view: true,
            row_index: 0,
            next_child_y: view_node.y,
            subtree_bottom: view_node.y + view_node.height,
            pending_key_value: None,
        });
    }

    fn push_hidden_frame(
        &mut self,
        node_index: usize,
        path: Vec<PathSeg>,
        view_source_index: usize,
        view_path: Vec<PathSeg>,
        depth: u32,
        node_render_handle: u32,
    ) {
        self.stack.push(Frame {
            node_index,
            node_render_handle,
            path,
            view_source_index,
            view_path,
            depth,
            owns_view: false,
            row_index: 0,
            next_child_y: 0,
            subtree_bottom: 0,
            pending_key_value: None,
        });
    }

    fn pop_frame(&mut self) -> Option<Frame> {
        self.stack.pop()
    }

    fn current_frame(&self) -> Option<&Frame> {
        self.stack.last()
    }

    fn current_frame_mut(&mut self) -> Option<&mut Frame> {
        self.stack.last_mut()
    }

    // ------------------------------------------------------------------
    // Sequence presentation helpers
    // ------------------------------------------------------------------

    fn sequence_presentation(&self, node_index: usize) -> SequencePresentation {
        let node = &self.node_pool[node_index];
        if node.kind != NodeKind::Sequence {
            return SequencePresentation::HeaderlessTable;
        }
        if sequence_has_header_table(node) {
            SequencePresentation::HeaderTable
        } else {
            SequencePresentation::HeaderlessTable
        }
    }

    fn sequence_uses_header_table(&self, node_index: usize) -> bool {
        let node = &self.node_pool[node_index];
        node.kind == NodeKind::Sequence
            && self.sequence_presentation(node_index) == SequencePresentation::HeaderTable
    }

    fn frame_hides_containers_in_parent_view(&self, frame: &Frame) -> bool {
        self.sequence_uses_header_table(frame.view_source_index)
    }

    // ------------------------------------------------------------------
    // Append node to graph
    // ------------------------------------------------------------------

    /// Append a graph node. Returns Some(render_handle) if the node was
    /// rendered (owns its own view), or None if it was hidden in the parent.
    fn append_node(
        &mut self,
        node_index: usize,
        path: &[PathSeg],
        is_container: bool,
    ) -> Result<Option<u32>, String> {
        let parent_info = self.current_frame().map(|p| {
            (
                p.node_index,
                p.node_render_handle,
                p.owns_view,
                p.row_index,
                p.next_child_y,
                p.depth,
            )
        });

        if let Some((parent_idx, parent_handle, parent_owns_view, _parent_row, _, _)) = parent_info
        {
            let hide = is_container
                && (!parent_owns_view
                    || self.should_hide_container_in_parent_view(parent_idx, node_index));
            if hide {
                self.attach_to_parent(parent_idx, node_index)?;
                if let Some(p) = self.current_frame_mut() {
                    p.row_index += 1;
                }
                self.queue_deferred_refresh(parent_handle)?;
                return Ok(None);
            }
        }

        let depth = if let Some((_, _, _, _, _, parent_depth)) = parent_info {
            parent_depth + 1
        } else {
            0
        };

        let render_handle = self.nodes.len() as u32;
        let node_ref = &self.node_pool[node_index];
        let mut view_node = self
            .view
            .build_node_only(node_ref, depth, path, render_handle);
        view_node.preorder_first = render_handle;
        view_node.preorder_last = render_handle;
        view_node.source = Some(node_index);

        let mut node_y: i32 = self.root_offset_y;
        let mut edge_index: Option<usize> = None;
        let mut pending_edge_index: Option<usize> = None;

        if let Some((parent_idx, parent_handle, _, parent_row, parent_next_child_y, _)) =
            parent_info
        {
            node_y = parent_next_child_y;
            if !is_container {
                let child_bottom = node_y + view_node.height;
                let v_gap = self.config.v_gap;
                if let Some(p) = self.current_frame_mut() {
                    p.next_child_y = child_bottom + v_gap;
                    p.subtree_bottom = p.subtree_bottom.max(child_bottom);
                }
            }
            self.attach_to_parent(parent_idx, node_index)?;
            if is_container
                || self.node_pool[node_index].kind == NodeKind::Scalar
                || self.node_pool[node_index].kind == NodeKind::Alias
            {
                let parent_key = graph_node_key(
                    graph_kind_for_node(&self.node_pool[parent_idx]),
                    &self.nodes[parent_handle as usize].path,
                );
                let child_key =
                    graph_node_key(graph_kind_for_node(&self.node_pool[node_index]), path);
                let edge = self.view.make_edge(
                    parent_handle,
                    parent_key,
                    parent_row,
                    render_handle,
                    child_key,
                    0,
                );
                edge_index = Some(self.edges.len());
                pending_edge_index = Some(self.pending_edges_added.len());
                self.edges.push(edge.clone());
                self.register_edge_index(self.edges.len() - 1, &edge);
                self.pending_edges_added.push(edge);
            }
            if let Some(p) = self.current_frame_mut() {
                p.row_index += 1;
            }
            self.queue_deferred_refresh(parent_handle)?;
        }
        if let Some((_, parent_handle, _, _, _, _)) = parent_info {
            let parent_view = &self.nodes[parent_handle as usize];
            view_node.x = parent_view.x + parent_view.width + self.config.h_gap;
        } else {
            view_node.x = self.root_offset_x;
        }
        view_node.y = node_y;
        self.view.apply_node_bounds_to(&mut view_node);
        view_node.preorder_first = render_handle;
        view_node.preorder_last = render_handle;
        self.nodes.push(view_node.clone());
        self.pending_nodes.push(view_node.clone());

        self.propagate_subtree_last_preorder(render_handle);

        if let Some((_, parent_handle, _, parent_row, _parent_next_child_y, _)) = parent_info {
            if let Some(idx) = edge_index {
                let edge = &mut self.edges[idx];
                self.view.apply_edge_bezier_args_to(&self.nodes, edge);
                if let Some(pending_idx) = pending_edge_index {
                    if pending_idx < self.pending_edges_added.len() {
                        self.pending_edges_added[pending_idx] = edge.clone();
                    }
                }
            }
            if parent_row == 0 {
                let parent_view = &self.nodes[parent_handle as usize];
                let delta = node_y - parent_view.y;
                if delta != 0 {
                    self.shift_subtree_y(parent_handle, delta, render_handle)?;
                }
            }
        } else {
            self.root_render_handle = Some(render_handle);
        }

        Ok(Some(render_handle))
    }

    // ------------------------------------------------------------------
    // Path resolution
    // ------------------------------------------------------------------

    fn consume_pending_path(&self) -> Result<Vec<PathSeg>, String> {
        if self.stack.is_empty() {
            return Ok(Vec::new());
        }
        let parent = self.current_frame().ok_or("InvalidSyntax")?;
        let parent_node = &self.node_pool[parent.node_index];
        if parent_node.kind == NodeKind::Sequence {
            let index = parent.row_index;
            return Ok(self.append_path_vec(&parent.path, PathSeg::Index(index as usize)));
        }
        if parent_node.kind == NodeKind::Mapping {
            let key = parent.pending_key_value.as_deref().unwrap_or("");
            return Ok(self.append_path_vec(&parent.path, PathSeg::Key(key.to_string())));
        }
        Ok(parent.path.clone())
    }

    fn resolve_event_path(&self, meta: &Meta) -> Result<Vec<PathSeg>, String> {
        if !meta.path.is_empty() {
            return self
                .parse_event_path(&meta.path)
                .or_else(|_| self.consume_pending_path());
        }
        if let Some(ref supplier) = meta.path_supplier {
            let supplied = supplier.get_path();
            if !supplied.is_empty() {
                return self
                    .parse_event_path(&supplied)
                    .or_else(|_| self.consume_pending_path());
            }
        }
        self.consume_pending_path()
    }

    fn parse_event_path(&self, raw: &str) -> Result<Vec<PathSeg>, String> {
        if raw.is_empty() || raw == "$" {
            return Ok(Vec::new());
        }
        if !raw.starts_with('$') {
            return Err("InvalidSyntax: path must start with '$'".into());
        }

        let mut segments = Vec::new();
        let bytes = raw.as_bytes();
        let mut i: usize = 1;
        while i < bytes.len() {
            match bytes[i] {
                b'.' => {
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
                        i += 1;
                    }
                    if i == start {
                        return Err("InvalidSyntax: empty path segment".into());
                    }
                    let key = std::str::from_utf8(&bytes[start..i])
                        .map_err(|_| "InvalidSyntax: invalid UTF-8 in path")?;
                    segments.push(PathSeg::Key(key.to_string()));
                }
                b'[' => {
                    i += 1;
                    if i >= bytes.len() {
                        return Err("InvalidSyntax: unclosed bracket".into());
                    }
                    if bytes[i] == b'"' {
                        let start = i;
                        i += 1;
                        let mut escaped = false;
                        while i < bytes.len() {
                            match bytes[i] {
                                b'\\' if !escaped => escaped = true,
                                b'"' if !escaped => break,
                                _ => escaped = false,
                            }
                            i += 1;
                        }
                        if i >= bytes.len() || bytes[i] != b'"' {
                            return Err("InvalidSyntax: unterminated quoted key".into());
                        }
                        let token = std::str::from_utf8(&bytes[start..=i])
                            .map_err(|_| "InvalidSyntax: invalid UTF-8 in path")?;
                        i += 1;
                        if i >= bytes.len() || bytes[i] != b']' {
                            return Err("InvalidSyntax: unclosed bracket".into());
                        }
                        i += 1;
                        let key = serde_json::from_str::<String>(token)
                            .map_err(|_| "InvalidSyntax: invalid quoted key in path")?;
                        segments.push(PathSeg::Key(key));
                    } else {
                        let start = i;
                        while i < bytes.len() && bytes[i] != b']' {
                            i += 1;
                        }
                        if i >= bytes.len() || i == start {
                            return Err("InvalidSyntax: unclosed bracket".into());
                        }
                        let token = std::str::from_utf8(&bytes[start..i])
                            .map_err(|_| "InvalidSyntax: invalid UTF-8 in path")?;
                        i += 1;
                        let index: i32 = token
                            .parse()
                            .map_err(|_| "InvalidSyntax: invalid index in path")?;
                        segments.push(PathSeg::Index(index as usize));
                    }
                }
                _ => return Err("InvalidSyntax: unexpected char in path".into()),
            }
        }
        Ok(segments)
    }

    // ------------------------------------------------------------------
    // Visibility helpers
    // ------------------------------------------------------------------

    fn should_hide_container_in_parent_view(&self, parent_index: usize, node_index: usize) -> bool {
        let parent_frame = match self.current_frame() {
            Some(f) => f,
            None => return false,
        };
        let parent_node = &self.node_pool[parent_index];
        // The first mapping row is still incomplete when MapStart arrives. Let it
        // render provisionally; try_drop_inline_container removes it once the
        // completed row proves that the sequence is a header table.
        let first_row_is_incomplete = parent_node.kind == NodeKind::Sequence
            && parent_node.content.len() <= 1
            && parent_node.content.first().is_none_or(|row| {
                row.kind == NodeKind::Mapping && row.content.is_empty()
            });
        if self.frame_hides_containers_in_parent_view(parent_frame) && !first_row_is_incomplete {
            return true;
        }
        let node = &self.node_pool[node_index];
        parent_frame.owns_view
            && parent_node.kind == NodeKind::Sequence
            && parent_node.content.is_empty()
            && node.kind == NodeKind::Mapping
            && !first_row_is_incomplete
    }

    // ------------------------------------------------------------------
    // Refresh
    // ------------------------------------------------------------------

    fn refresh_frame_view(
        &mut self,
        node_render_handle: u32,
        view_source_index: usize,
        view_path: &[PathSeg],
        depth: u32,
    ) -> Result<i32, String> {
        self.refresh_node(node_render_handle, view_source_index, view_path, depth)?;
        let node = &self.nodes[node_render_handle as usize];
        let bottom = node.y + node.height;
        if let Some(frame) = self.stack.last_mut() {
            if frame.node_render_handle == node_render_handle {
                frame.subtree_bottom = frame.subtree_bottom.max(bottom);
            }
        }
        Ok(bottom)
    }

    fn refresh_node(
        &mut self,
        node_render_handle: u32,
        source_index: usize,
        path: &[PathSeg],
        depth: u32,
    ) -> Result<(), String> {
        if source_index >= self.node_pool.len() {
            return Err(format!(
                "refresh_node: source_index {} out of bounds (node_pool len {})",
                source_index,
                self.node_pool.len()
            ));
        }
        let source_node = self.node_pool[source_index].clone();
        let mut rebuilt = self
            .view
            .build_node_only(&source_node, depth, path, node_render_handle);
        if node_render_handle as usize >= self.nodes.len() {
            return Err(format!(
                "refresh_node: render_handle {} out of bounds (nodes len {})",
                node_render_handle,
                self.nodes.len()
            ));
        }
        let prev = &self.nodes[node_render_handle as usize];
        let old = prev.clone();
        let old_right = prev.x + prev.width + self.config.h_gap;
        rebuilt.x = prev.x;
        rebuilt.y = prev.y;
        rebuilt.preorder_first = prev.preorder_first;
        rebuilt.preorder_last = prev.preorder_last;
        rebuilt.source = Some(source_index);
        self.view.apply_node_bounds_to(&mut rebuilt);
        self.nodes[node_render_handle as usize] = rebuilt.clone();
        let external_changed = !same_external_node_state(&old, &rebuilt);
        if !external_changed {
            replace_pending_node(&mut self.pending_nodes, &rebuilt);
            replace_pending_node(&mut self.pending_nodes_updated, &rebuilt);
        }
        self.view.free_node_owned_data(&old, Some(path));
        if external_changed {
            self.queue_node_state(&rebuilt)?;
        }
        self.update_frame_subtree_bottom(node_render_handle, &rebuilt);
        self.update_edges_for_node(node_render_handle)?;
        let new_right = rebuilt.x + rebuilt.width + self.config.h_gap;
        let delta_x = new_right - old_right;
        if delta_x != 0 {
            self.shift_subtree_x(node_render_handle, delta_x)?;
        }
        Ok(())
    }

    fn update_frame_subtree_bottom(&mut self, node_render_handle: u32, node: &GraphNode) {
        for frame in self.stack.iter_mut() {
            if frame.node_render_handle == node_render_handle {
                frame.subtree_bottom = frame.subtree_bottom.max(node.y + node.height);
                break;
            }
        }
    }

    // ------------------------------------------------------------------
    // Path helpers
    // ------------------------------------------------------------------

    fn append_path_vec(&self, base: &[PathSeg], seg: PathSeg) -> Vec<PathSeg> {
        let mut next = base.to_vec();
        next.push(seg);
        next
    }

    // ------------------------------------------------------------------
    // Attach child to parent
    // ------------------------------------------------------------------

    fn attach_to_parent(&mut self, parent_index: usize, node_index: usize) -> Result<(), String> {
        let parent_kind = self.node_pool[parent_index].kind;
        match parent_kind {
            NodeKind::Mapping => {
                let key_value = {
                    let frame = self.current_frame_mut().ok_or("InvalidSyntax")?;
                    frame
                        .pending_key_value
                        .take()
                        .ok_or("InvalidSyntax: missing map key")?
                };
                // Create key node
                let key_node = TreeNode {
                    kind: NodeKind::Scalar,
                    sem_type: Some(CompatSemType::Str),
                    value: key_value.clone(),
                    is_map_key: true,
                    ..TreeNode::default()
                };
                let key_index = self.node_pool.len();
                self.node_pool.push(key_node);
                // Push key and value to parent content
                let key_child = self.node_pool[key_index].clone();
                let value_child = self.node_pool[node_index].clone();
                self.node_pool[parent_index].content.push(key_child);
                self.node_pool[parent_index].content.push(value_child);
            }
            NodeKind::Sequence => {
                let index = self.node_pool[parent_index].content.len();
                self.node_pool[node_index].sequence_index = Some(index as i64);
                let child = self.node_pool[node_index].clone();
                self.node_pool[parent_index].content.push(child);
            }
            _ => {}
        }
        Ok(())
    }

    fn sync_finished_child_to_parent(&mut self, finished: &Frame) {
        let Some(parent) = self.current_frame() else {
            return;
        };
        let parent_index = parent.node_index;
        let child = self.node_pool[finished.node_index].clone();
        match self.node_pool[parent_index].kind {
            NodeKind::Sequence => {
                let index = child
                    .sequence_index
                    .and_then(|idx| usize::try_from(idx).ok())
                    .or_else(|| match finished.path.last() {
                        Some(PathSeg::Index(idx)) => Some(*idx),
                        _ => None,
                    });
                if let Some(index) = index {
                    if let Some(slot) = self.node_pool[parent_index].content.get_mut(index) {
                        *slot = child;
                    }
                }
            }
            NodeKind::Mapping => {
                let key = match finished.path.last() {
                    Some(PathSeg::Key(key)) => key.as_str(),
                    _ => return,
                };
                let content = &mut self.node_pool[parent_index].content;
                let mut i = content.len();
                while i >= 2 {
                    i -= 2;
                    if content[i].is_map_key && content[i].value == key {
                        content[i + 1] = child;
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Subtree Y/X shifting
    // ------------------------------------------------------------------

    fn shift_subtree_y(
        &mut self,
        root_render_handle: u32,
        delta: i32,
        exclude_id: u32,
    ) -> Result<(), String> {
        for i in 0..self.nodes.len() {
            let handle = self.nodes[i].render_handle;
            let should_shift = handle == root_render_handle
                || (handle != exclude_id && self.is_descendant(handle, root_render_handle));
            if should_shift {
                {
                    let node = &mut self.nodes[i];
                    node.y += delta;
                    self.view.apply_node_bounds_to(node);
                }
                let updated = self.nodes[i].clone();
                self.mark_node_updated(&updated)?;
                self.update_edges_for_node(handle)?;
            }
        }
        self.shift_frame_next_child_y(root_render_handle, delta);
        Ok(())
    }

    fn shift_subtree_x(&mut self, root_render_handle: u32, delta: i32) -> Result<(), String> {
        for i in 0..self.nodes.len() {
            let handle = self.nodes[i].render_handle;
            if handle == root_render_handle {
                continue;
            }
            if !self.is_descendant(handle, root_render_handle) {
                continue;
            }
            {
                let node = &mut self.nodes[i];
                node.x += delta;
                self.view.apply_node_bounds_to(node);
            }
            let updated = self.nodes[i].clone();
            self.mark_node_updated(&updated)?;
            self.update_edges_for_node(handle)?;
        }
        Ok(())
    }

    fn shift_frame_next_child_y(&mut self, node_render_handle: u32, delta: i32) {
        for frame in self.stack.iter_mut() {
            if frame.node_render_handle == node_render_handle {
                frame.next_child_y += delta;
                frame.subtree_bottom += delta;
                break;
            }
        }
    }

    fn adjust_parent_next_child_y(&mut self, child_bottom: i32) {
        let v_gap = self.config.v_gap;
        if let Some(parent) = self.current_frame_mut() {
            parent.next_child_y = child_bottom + v_gap;
            parent.subtree_bottom = parent.subtree_bottom.max(child_bottom);
        }
    }

    // ------------------------------------------------------------------
    // Try drop empty container
    // ------------------------------------------------------------------

    fn try_drop_inline_container(&mut self, finished: &Frame) -> Result<bool, String> {
        let (parent_handle, parent_view_source_index, parent_view_path, parent_depth) =
            match self.current_frame() {
                Some(p) => (
                    p.node_render_handle,
                    p.view_source_index,
                    p.view_path.clone(),
                    p.depth,
                ),
                None => return Ok(false),
            };
        let node = &self.node_pool[finished.node_index];
        let header_row_is_inline = self.node_pool[parent_view_source_index].kind == NodeKind::Sequence
            && sequence_has_header_table(&self.node_pool[parent_view_source_index])
            && matches!(finished.path.last(), Some(PathSeg::Index(0)));
        if !node.content.is_empty() && !header_row_is_inline {
            return Ok(false);
        }
        if finished.node_render_handle + 1 != self.nodes.len() as u32 {
            return Ok(false);
        }

        let dropped_render_handle = finished.node_render_handle;
        self.deferred_refresh_nodes
            .retain(|handle| *handle != dropped_render_handle);
        self.deferred_refresh_set.remove(&dropped_render_handle);
        let dropped_view = self.nodes.pop().unwrap();
        if self
            .pending_nodes
            .last()
            .is_some_and(|n| n.render_handle == dropped_render_handle)
        {
            self.pending_nodes.pop();
        }
        if self
            .edges
            .last()
            .is_some_and(|e| e.to_render_handle == dropped_render_handle)
        {
            let last_index = self.edges.len() - 1;
            let last_edge = self.edges[last_index].clone();
            self.unregister_last_edge_index(last_index, &last_edge);
            self.edges.pop();
            if self
                .pending_edges_added
                .last()
                .is_some_and(|e| e.to_render_handle == dropped_render_handle)
            {
                self.pending_edges_added.pop();
            }
        }
        if let Some(parent_frame) = self.current_frame_mut() {
            if parent_frame.owns_view {
                parent_frame.next_child_y = dropped_view.y;
                parent_frame.subtree_bottom = parent_frame.subtree_bottom.max(dropped_view.y);
            }
        }
        self.view.free_node_owned_data(&dropped_view, None);
        self.recompute_subtree_last_preorder(parent_handle);
        for i in (0..self.stack.len()).rev() {
            let frame = &self.stack[i];
            if frame.node_render_handle == parent_handle {
                continue;
            }
            if self.nodes[frame.node_render_handle as usize].preorder_last >= dropped_render_handle
            {
                self.recompute_subtree_last_preorder(frame.node_render_handle);
            }
        }
        self.refresh_frame_view(
            parent_handle,
            parent_view_source_index,
            &parent_view_path,
            parent_depth,
        )?;
        Ok(true)
    }

    // ------------------------------------------------------------------
    // Preorder helpers
    // ------------------------------------------------------------------

    fn propagate_subtree_last_preorder(&mut self, child_render_handle: u32) {
        for frame in self.stack.iter() {
            let parent_view = &mut self.nodes[frame.node_render_handle as usize];
            if child_render_handle > parent_view.preorder_last {
                parent_view.preorder_last = child_render_handle;
            }
        }
    }

    fn is_descendant(&self, node_render_handle: u32, root_render_handle: u32) -> bool {
        let root = &self.nodes[root_render_handle as usize];
        node_render_handle >= root.preorder_first && node_render_handle <= root.preorder_last
    }

    fn recompute_subtree_last_preorder(&mut self, node_render_handle: u32) {
        let node = &self.nodes[node_render_handle as usize];
        let first = node.preorder_first;
        let old_last = node.preorder_last;
        let mut last_render_handle = node_render_handle;
        for candidate in &self.nodes {
            if candidate.render_handle == node_render_handle {
                continue;
            }
            if candidate.preorder_first >= first && candidate.preorder_first <= old_last {
                if candidate.preorder_last > last_render_handle {
                    last_render_handle = candidate.preorder_last;
                }
            }
        }
        self.nodes[node_render_handle as usize].preorder_last = last_render_handle;
    }

    // ------------------------------------------------------------------
    // Deferred refresh
    // ------------------------------------------------------------------

    fn queue_deferred_refresh(&mut self, node_render_handle: u32) -> Result<(), String> {
        if !self.deferred_refresh_set.insert(node_render_handle) {
            return Ok(());
        }
        self.deferred_refresh_nodes.push_back(node_render_handle);
        Ok(())
    }

    /// Apply queued refreshes after the main traversal.
    ///
    /// Complexity contract: this queue drain should trend toward
    /// `O(deferred_refresh_nodes.len())` plus the cost of `refresh_node`.
    ///
    /// This queue uses a front-pop structure with an explicit in-queue set, so
    /// large refresh batches stay linear while still allowing a refreshed node
    /// to be queued again later in the same drain.
    fn apply_deferred_refreshes(&mut self) -> Result<(), String> {
        while let Some(node_render_handle) = self.deferred_refresh_nodes.pop_front() {
            self.deferred_refresh_set.remove(&node_render_handle);
            if node_render_handle as usize >= self.nodes.len() {
                return Err(format!(
                    "apply_deferred_refreshes: render_handle {} out of bounds (nodes len {})",
                    node_render_handle,
                    self.nodes.len()
                ));
            }
            let node = &self.nodes[node_render_handle as usize];
            let source_index = node.source.unwrap_or(0);
            if source_index >= self.node_pool.len() {
                return Err(format!(
                    "apply_deferred_refreshes: source_index {} out of bounds (node_pool len {}) for render_handle {}",
                    source_index,
                    self.node_pool.len(),
                    node_render_handle
                ));
            }
            let path = node.path.clone();
            let depth = node.depth;
            self.refresh_node(node_render_handle, source_index, &path, depth)?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Delta queue management
    // ------------------------------------------------------------------

    fn queue_node_state(&mut self, node: &GraphNode) -> Result<(), String> {
        if replace_pending_node(&mut self.pending_nodes, node) {
            return Ok(());
        }
        if replace_pending_node(&mut self.pending_nodes_updated, node) {
            return Ok(());
        }
        self.pending_nodes_updated.push(node.clone());
        Ok(())
    }

    pub fn mark_node_updated(&mut self, node: &GraphNode) -> Result<(), String> {
        self.queue_node_state(node)
    }

    fn queue_edge_update(
        &mut self,
        old_edge: &GraphEdge,
        new_edge: &GraphEdge,
    ) -> Result<(), String> {
        if replace_pending_edge(&mut self.pending_edges_added, new_edge) {
            return Ok(());
        }
        if find_pending_edge_index(&self.pending_edges_removed, old_edge).is_none() {
            self.pending_edges_removed.push(old_edge.clone());
        }
        self.pending_edges_added.push(new_edge.clone());
        Ok(())
    }

    // ------------------------------------------------------------------
    // Edge update
    // ------------------------------------------------------------------

    pub fn update_edges_for_node(&mut self, node_render_handle: u32) -> Result<(), String> {
        let idx = node_render_handle as usize;
        // Collect edge indices first to avoid borrow issues
        let from_indices: Vec<usize> = if idx < self.edges_from_index.len() {
            self.edges_from_index[idx].clone()
        } else {
            Vec::new()
        };
        let to_indices: Vec<usize> = if idx < self.edges_to_index.len() {
            self.edges_to_index[idx].clone()
        } else {
            Vec::new()
        };
        for edge_idx in from_indices {
            self.update_edge_at(edge_idx)?;
        }
        for edge_idx in to_indices {
            self.update_edge_at(edge_idx)?;
        }
        Ok(())
    }

    fn update_edge_at(&mut self, edge_idx: usize) -> Result<(), String> {
        let old_edge = self.edges[edge_idx].clone();
        self.view
            .apply_edge_bezier_args_to(&self.nodes, &mut self.edges[edge_idx]);
        let new_edge = self.edges[edge_idx].clone();
        self.queue_edge_update(&old_edge, &new_edge)
    }
}

fn format_graph_path(path: &[PathSeg]) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut out = String::from("$");
    for segment in path {
        match segment {
            PathSeg::Key(key) => {
                out.push('[');
                out.push_str(
                    &serde_json::to_string(key)
                        .expect("serializing graph path key should not fail"),
                );
                out.push(']');
            }
            PathSeg::Index(index) => {
                out.push('[');
                out.push_str(&index.to_string());
                out.push(']');
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Pending list helpers
// ---------------------------------------------------------------------------

fn replace_pending_node(list: &mut Vec<GraphNode>, node: &GraphNode) -> bool {
    for pending in list.iter_mut() {
        if pending.render_handle == node.render_handle {
            *pending = node.clone();
            return true;
        }
    }
    false
}

fn replace_pending_edge(list: &mut Vec<GraphEdge>, edge: &GraphEdge) -> bool {
    if let Some(idx) = find_pending_edge_index(list, edge) {
        list[idx] = edge.clone();
        return true;
    }
    false
}

fn find_pending_edge_index(edges: &[GraphEdge], edge: &GraphEdge) -> Option<usize> {
    edges
        .iter()
        .position(|pending| edge_identity_eql(pending, edge))
}

fn edge_identity_eql(a: &GraphEdge, b: &GraphEdge) -> bool {
    a.from_render_handle == b.from_render_handle
        && a.from_row == b.from_row
        && a.to_render_handle == b.to_render_handle
        && a.to_row == b.to_row
}

// ---------------------------------------------------------------------------
// External node state comparison (for detecting real changes)
// ---------------------------------------------------------------------------

fn same_external_node_state(a: &GraphNode, b: &GraphNode) -> bool {
    if a.kind != b.kind {
        return false;
    }
    if a.depth != b.depth {
        return false;
    }
    if !same_box_args(&a.box_args, &b.box_args) {
        return false;
    }
    if !same_path_slice(&a.path, &b.path) {
        return false;
    }
    if !same_graph_cell(&a.meta, &b.meta) {
        return false;
    }
    if !same_graph_rows(&a.rows, &b.rows) {
        return false;
    }
    if a.table.is_some() != b.table.is_some() {
        return false;
    }
    if let (Some(a_table), Some(b_table)) = (&a.table, &b.table) {
        if !same_graph_table(a_table, b_table) {
            return false;
        }
    }
    true
}

fn same_graph_table(
    a: &super::graph_builder::GraphTable,
    b: &super::graph_builder::GraphTable,
) -> bool {
    same_graph_cells(&a.columns, &b.columns)
        && same_graph_cell_rows(&a.rows, &b.rows)
        && a.header_height == b.header_height
        && a.total_height == b.total_height
        && a.view_height == b.view_height
        && a.row_height == b.row_height
}

fn same_graph_rows(a: &[GraphRow], b: &[GraphRow]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (left, right) in a.iter().zip(b.iter()) {
        if !same_graph_row(left, right) {
            return false;
        }
    }
    true
}

fn same_graph_row(a: &GraphRow, b: &GraphRow) -> bool {
    a.index == b.index
        && same_box_args(&a.box_args, &b.box_args)
        && same_box_args(&a.cell_box_args, &b.cell_box_args)
        && same_graph_cell(&a.key, &b.key)
        && same_graph_cell(&a.value, &b.value)
}

fn same_graph_cells(
    a: &[super::graph_builder::GraphCell],
    b: &[super::graph_builder::GraphCell],
) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (left, right) in a.iter().zip(b.iter()) {
        if !same_graph_cell(left, right) {
            return false;
        }
    }
    true
}

fn same_graph_cell_rows(
    a: &[super::graph_builder::GraphRow],
    b: &[super::graph_builder::GraphRow],
) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (left_row, right_row) in a.iter().zip(b.iter()) {
        if left_row.index != right_row.index
            || !same_box_args(&left_row.box_args, &right_row.box_args)
            || !same_box_args(&left_row.cell_box_args, &right_row.cell_box_args)
            || !same_graph_cells(&left_row.cells, &right_row.cells)
        {
            return false;
        }
    }
    true
}

fn same_graph_cell(
    a: &super::graph_builder::GraphCell,
    b: &super::graph_builder::GraphCell,
) -> bool {
    a.text == b.text
        && a.sem_type == b.sem_type
        && a.path == b.path
        && a.value == b.value
        && a.editable == b.editable
        && same_box_args(&a.box_args, &b.box_args)
        && same_cell_bounds(&a.bounds, &b.bounds)
        && same_cell_bounds(&a.text_bounds, &b.text_bounds)
}

fn same_cell_bounds(
    a: &super::graph_builder::CellBounds,
    b: &super::graph_builder::CellBounds,
) -> bool {
    a.x == b.x && a.y == b.y && a.width == b.width && a.height == b.height
}

fn same_box_args(a: &super::graph_builder::BoxArgs, b: &super::graph_builder::BoxArgs) -> bool {
    a.x == b.x
        && a.y == b.y
        && a.width == b.width
        && a.height == b.height
        && a.corner_radius == b.corner_radius
}

fn same_path_slice(a: &[PathSeg], b: &[PathSeg]) -> bool {
    a == b
}
