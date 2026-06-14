use std::collections::{HashMap, HashSet};

use super::graph_builder::{BuilderConfig, GraphNode, PathSeg};
use super::{NodeId, TreeNodeKind, TreeStore};

pub(crate) type GraphHandle = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SequencePresentationState {
    EmptyOpen,
    EmptyClosed,
    PendingHeaderSchema,
    HeaderlessTable,
    HeaderTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphRole {
    GraphNode,
    InlineValue,
    FoldedTableRow {
        table_node_id: NodeId,
        row_node_id: NodeId,
        row_index: usize,
    },
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SequenceState {
    pub presentation: SequencePresentationState,
    pub first_child: Option<NodeId>,
    pub first_child_kind: Option<TreeNodeKind>,
    pub header_key_count: usize,
    pub closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GraphChildEdge {
    pub child: GraphHandle,
    pub from_row: i32,
    pub to_row: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DirtyEdge {
    pub from: GraphHandle,
    pub to: GraphHandle,
    pub from_row: i32,
    pub to_row: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TableRowRef {
    pub table_handle: GraphHandle,
    pub table_node_id: NodeId,
    pub row_node_id: NodeId,
    pub row_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphSlot {
    pub node_id: NodeId,
    pub children: Vec<GraphChildEdge>,
    pub path: Vec<PathSeg>,
    pub depth: u32,
    pub shape: Option<GraphNode>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DirtySet {
    added_handles: Vec<GraphHandle>,
    shape_handles: Vec<GraphHandle>,
    layout_handles: Vec<GraphHandle>,
    table_handles: Vec<GraphHandle>,
    added_edges: Vec<DirtyEdge>,
    table_rows: Vec<TableRowRef>,
    added_seen: HashSet<GraphHandle>,
    shape_seen: HashSet<GraphHandle>,
    layout_seen: HashSet<GraphHandle>,
    table_seen: HashSet<GraphHandle>,
    edge_seen: HashSet<DirtyEdge>,
    table_row_seen: HashSet<NodeId>,
}

impl DirtySet {
    pub(crate) fn clear(&mut self) {
        self.added_handles.clear();
        self.shape_handles.clear();
        self.layout_handles.clear();
        self.table_handles.clear();
        self.added_edges.clear();
        self.table_rows.clear();
        self.added_seen.clear();
        self.shape_seen.clear();
        self.layout_seen.clear();
        self.table_seen.clear();
        self.edge_seen.clear();
        self.table_row_seen.clear();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.added_handles.is_empty()
            && self.shape_handles.is_empty()
            && self.layout_handles.is_empty()
            && self.table_handles.is_empty()
            && self.added_edges.is_empty()
            && self.table_rows.is_empty()
    }

    pub(crate) fn added_handles(&self) -> &[GraphHandle] {
        &self.added_handles
    }

    pub(crate) fn shape_handles(&self) -> &[GraphHandle] {
        &self.shape_handles
    }

    pub(crate) fn added_edges(&self) -> &[DirtyEdge] {
        &self.added_edges
    }

    pub(crate) fn table_rows(&self) -> &[TableRowRef] {
        &self.table_rows
    }

    pub(crate) fn mark_added(&mut self, handle: GraphHandle) {
        if self.added_seen.insert(handle) {
            self.added_handles.push(handle);
        }
        self.mark_shape(handle);
        self.mark_layout(handle);
    }

    pub(crate) fn mark_shape(&mut self, handle: GraphHandle) {
        if self.shape_seen.insert(handle) {
            self.shape_handles.push(handle);
        }
    }

    pub(crate) fn mark_layout(&mut self, handle: GraphHandle) {
        if self.layout_seen.insert(handle) {
            self.layout_handles.push(handle);
        }
    }

    pub(crate) fn mark_table(&mut self, handle: GraphHandle) {
        if self.table_seen.insert(handle) {
            self.table_handles.push(handle);
        }
        self.mark_shape(handle);
        self.mark_layout(handle);
    }

    pub(crate) fn mark_edge(&mut self, edge: DirtyEdge) {
        if self.edge_seen.insert(edge) {
            self.added_edges.push(edge);
        }
    }

    pub(crate) fn mark_table_row(&mut self, row: TableRowRef) {
        if self.table_row_seen.insert(row.row_node_id) {
            self.table_rows.push(row);
        }
        self.mark_table(row.table_handle);
    }

    pub(crate) fn finish(&mut self) {
        self.added_handles.sort_unstable();
        self.shape_handles.sort_unstable();
        self.layout_handles.sort_unstable();
        self.table_handles.sort_unstable();
        self.added_edges
            .sort_by_key(|edge| (edge.from, edge.from_row, edge.to, edge.to_row));
        self.table_rows.sort_by_key(|row| {
            (
                row.table_handle,
                row.row_index,
                row.table_node_id.0,
                row.row_node_id.0,
            )
        });
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TopologyMetrics {
    pub path_rebuilds: usize,
    pub path_segment_pushes: usize,
}

#[derive(Debug, Default)]
struct PathCursor {
    segments: Vec<PathSeg>,
}

impl PathCursor {
    fn as_slice(&self) -> &[PathSeg] {
        &self.segments
    }

    fn push_child(&mut self, store: &TreeStore, child: NodeId) -> usize {
        let old_len = self.segments.len();
        if let Some(segment) = path_segment_for_node(store, child) {
            self.segments.push(segment);
        }
        old_len
    }

    fn truncate(&mut self, len: usize) {
        self.segments.truncate(len);
    }
}

fn path_segment_for_node(store: &TreeStore, id: NodeId) -> Option<PathSeg> {
    let node = store.get(id)?;
    if node.is_map_key {
        return Some(PathSeg::Key(node.value.clone()));
    }
    if let Some(key_id) = node.key {
        return store
            .get(key_id)
            .map(|key_node| PathSeg::Key(key_node.value.clone()));
    }
    node.sequence_index
        .and_then(|value| usize::try_from(value).ok())
        .map(PathSeg::Index)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GraphTopology {
    root: Option<NodeId>,
    node_to_handle: HashMap<NodeId, GraphHandle>,
    handle_to_node: Vec<NodeId>,
    slots: Vec<GraphSlot>,
    sequence_states: HashMap<NodeId, SequenceState>,
    folded_rows: HashMap<NodeId, TableRowRef>,
    dirty: DirtySet,
    #[cfg(test)]
    metrics: TopologyMetrics,
}

impl GraphTopology {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn root_handle(&self) -> Option<GraphHandle> {
        self.root.and_then(|root| self.handle_for(root))
    }

    pub(crate) fn handle_for(&self, node_id: NodeId) -> Option<GraphHandle> {
        self.node_to_handle.get(&node_id).copied()
    }

    pub(crate) fn slot(&self, handle: GraphHandle) -> Option<&GraphSlot> {
        self.slots.get(handle as usize)
    }

    pub(crate) fn slot_mut(&mut self, handle: GraphHandle) -> Option<&mut GraphSlot> {
        self.slots.get_mut(handle as usize)
    }

    pub(crate) fn slots(&self) -> &[GraphSlot] {
        &self.slots
    }

    pub(crate) fn child_edges(&self, handle: GraphHandle) -> &[GraphChildEdge] {
        self.slot(handle)
            .map(|slot| slot.children.as_slice())
            .unwrap_or(&[])
    }

    #[cfg(test)]
    pub(crate) fn metrics(&self) -> TopologyMetrics {
        self.metrics
    }

    pub(crate) fn apply(
        &mut self,
        store: &TreeStore,
        root: NodeId,
        patches: &[crate::stream::tree_patch::TreePatch],
        config: &BuilderConfig,
    ) -> DirtySet {
        self.dirty.clear();
        if self.root.is_none() {
            self.root = Some(root);
        }

        let mut anchors = Vec::new();
        let mut seen = HashSet::new();
        for patch in patches {
            self.collect_patch_anchors(store, patch, root, &mut anchors, &mut seen);
        }
        if anchors.is_empty() && self.handle_for(root).is_none() {
            push_anchor(root, &mut anchors, &mut seen);
        }
        anchors.sort_by_key(|id| id.0);
        let mut refreshed_sequences = HashSet::new();

        for &anchor in &anchors {
            self.refresh_sequence_ancestors(store, anchor, &mut refreshed_sequences);
        }
        for anchor in anchors {
            self.reconcile_from_anchor(store, anchor, config);
        }

        self.dirty.finish();
        self.dirty.clone()
    }

    pub(crate) fn build_full(
        &mut self,
        store: &TreeStore,
        root: NodeId,
        config: &BuilderConfig,
    ) -> DirtySet {
        *self = Self::new();
        self.root = Some(root);
        self.refresh_sequence_subtree(store, root);

        let mut visited = HashSet::new();
        let mut path = PathCursor::default();
        self.reconcile_full_with_path(store, root, config, &mut visited, &mut path);

        self.dirty.finish();
        self.dirty.clone()
    }

    fn collect_patch_anchors(
        &mut self,
        store: &TreeStore,
        patch: &crate::stream::tree_patch::TreePatch,
        root: NodeId,
        anchors: &mut Vec<NodeId>,
        seen: &mut HashSet<NodeId>,
    ) {
        use crate::stream::tree_patch::TreePatch;
        match patch {
            TreePatch::DocumentStarted { root } | TreePatch::DocumentEnded { root } => {
                push_anchor(*root, anchors, seen);
            }
            TreePatch::NodeInserted {
                node_id,
                parent,
                key,
                ..
            } => {
                push_anchor(*node_id, anchors, seen);
                if let Some(parent) = *parent {
                    push_anchor(parent, anchors, seen);
                }
                if let Some(key) = *key {
                    push_anchor(key, anchors, seen);
                }
                self.push_folded_or_sequence_anchor(store, *node_id, anchors, seen);
            }
            TreePatch::KeyInserted { key_id, parent, .. } => {
                push_anchor(*key_id, anchors, seen);
                push_anchor(*parent, anchors, seen);
                self.push_folded_or_sequence_anchor(store, *parent, anchors, seen);
            }
            TreePatch::NodeSealed { node_id } => {
                push_anchor(*node_id, anchors, seen);
                if let Some(parent) = store.get(*node_id).and_then(|node| node.parent) {
                    push_anchor(parent, anchors, seen);
                }
                self.push_folded_or_sequence_anchor(store, *node_id, anchors, seen);
            }
            TreePatch::DiagnosticAdded { .. } => {
                if self.root.is_none() {
                    push_anchor(root, anchors, seen);
                }
            }
        }
    }

    fn push_folded_or_sequence_anchor(
        &self,
        store: &TreeStore,
        node_id: NodeId,
        anchors: &mut Vec<NodeId>,
        seen: &mut HashSet<NodeId>,
    ) {
        if let Some((table_id, row_id)) = folded_table_row_context(store, node_id, self.root) {
            push_anchor(table_id, anchors, seen);
            push_anchor(row_id, anchors, seen);
            return;
        }
        let mut current = Some(node_id);
        while let Some(id) = current {
            let Some(node) = store.get(id) else {
                break;
            };
            if node.kind == TreeNodeKind::Sequence {
                push_anchor(id, anchors, seen);
                break;
            }
            current = node.parent;
        }
    }

    fn refresh_sequence_subtree(&mut self, store: &TreeStore, id: NodeId) {
        let Some(node) = store.get(id) else {
            return;
        };
        if node.kind == TreeNodeKind::Sequence {
            self.refresh_sequence_state(store, id);
        }
        for &child in &node.content {
            self.refresh_sequence_subtree(store, child);
        }
    }

    fn refresh_sequence_ancestors(
        &mut self,
        store: &TreeStore,
        mut id: NodeId,
        refreshed_sequences: &mut HashSet<NodeId>,
    ) {
        loop {
            let Some(node) = store.get(id) else {
                break;
            };
            if node.kind == TreeNodeKind::Sequence && refreshed_sequences.insert(id) {
                self.refresh_sequence_state(store, id);
            }
            let Some(parent) = node.parent else {
                break;
            };
            id = parent;
        }
    }

    fn refresh_sequence_state(&mut self, store: &TreeStore, id: NodeId) -> Option<SequenceState> {
        let state = sequence_state_from_store(store, id)?;
        self.sequence_states.insert(id, state.clone());
        Some(state)
    }

    fn sequence_state(&mut self, store: &TreeStore, id: NodeId) -> Option<SequenceState> {
        if let Some(state) = self.sequence_states.get(&id) {
            return Some(state.clone());
        }
        self.refresh_sequence_state(store, id)
    }

    fn reconcile_from_anchor(&mut self, store: &TreeStore, anchor: NodeId, config: &BuilderConfig) {
        let mut visited = HashSet::new();
        self.reconcile_one(store, anchor, &mut visited, config);
        if let Some(parent) = store.get(anchor).and_then(|node| node.parent) {
            self.reconcile_one(store, parent, &mut visited, config);
        }
        if let Some((table_id, row_id)) = folded_table_row_context(store, anchor, self.root) {
            self.reconcile_one(store, table_id, &mut visited, config);
            self.reconcile_one(store, row_id, &mut visited, config);
        }
    }

    fn reconcile_full_with_path(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        config: &BuilderConfig,
        visited: &mut HashSet<NodeId>,
        path: &mut PathCursor,
    ) {
        self.reconcile_one_with_path(store, id, visited, config, Some(path.as_slice()));
        let Some(node) = store.get(id) else {
            return;
        };
        if is_header_table_sequence(store, id) {
            for &child in &node.content {
                let old_len = path.push_child(store, child);
                #[cfg(test)]
                {
                    self.metrics.path_segment_pushes += usize::from(path.segments.len() > old_len);
                }
                self.reconcile_full_with_path(store, child, config, visited, path);
                path.truncate(old_len);
            }
            return;
        }
        match node.kind {
            TreeNodeKind::Mapping => {
                let mut index = 1;
                while index < node.content.len() {
                    let child = node.content[index];
                    let old_len = path.push_child(store, child);
                    #[cfg(test)]
                    {
                        self.metrics.path_segment_pushes +=
                            usize::from(path.segments.len() > old_len);
                    }
                    self.reconcile_full_with_path(store, child, config, visited, path);
                    path.truncate(old_len);
                    index += 2;
                }
            }
            TreeNodeKind::Sequence => {
                for &child in &node.content {
                    let old_len = path.push_child(store, child);
                    #[cfg(test)]
                    {
                        self.metrics.path_segment_pushes +=
                            usize::from(path.segments.len() > old_len);
                    }
                    self.reconcile_full_with_path(store, child, config, visited, path);
                    path.truncate(old_len);
                }
            }
            TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => {}
        }
    }

    fn reconcile_one(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
    ) -> Option<GraphHandle> {
        self.reconcile_one_with_path(store, id, visited, config, None)
    }

    fn reconcile_one_with_path(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
        path_override: Option<&[PathSeg]>,
    ) -> Option<GraphHandle> {
        if !visited.insert(id) {
            return self.handle_for(id);
        }
        let role = self.role_for(store, id, config);
        match role {
            GraphRole::Pending => None,
            GraphRole::InlineValue => {
                self.mark_inline_owner_dirty(store, id);
                None
            }
            GraphRole::FoldedTableRow {
                table_node_id,
                row_node_id,
                row_index,
            } => {
                let table_handle = self
                    .handle_for(table_node_id)
                    .or_else(|| self.reconcile_one(store, table_node_id, visited, config));
                if let Some(table_handle) = table_handle {
                    let row = TableRowRef {
                        table_handle,
                        table_node_id,
                        row_node_id,
                        row_index,
                    };
                    self.folded_rows.insert(row_node_id, row);
                    self.dirty.mark_table_row(row);
                }
                None
            }
            GraphRole::GraphNode => {
                self.ensure_graph_node_with_path(store, id, visited, config, path_override)
            }
        }
    }

    fn role_for(&mut self, store: &TreeStore, id: NodeId, config: &BuilderConfig) -> GraphRole {
        let Some(node) = store.get(id) else {
            return GraphRole::Pending;
        };
        if Some(id) == self.root {
            if node.kind == TreeNodeKind::Sequence
                && matches!(
                    self.sequence_state(store, id)
                        .map(|state| state.presentation),
                    Some(
                        SequencePresentationState::EmptyOpen
                            | SequencePresentationState::PendingHeaderSchema
                    )
                )
            {
                return GraphRole::Pending;
            }
            return GraphRole::GraphNode;
        }

        if !config.expand_header_table_rows {
            if let Some((table_node_id, row_node_id)) =
                folded_table_row_context(store, id, self.root)
            {
                let row_index = sequence_row_index(store, row_node_id)
                    .or_else(|| {
                        store
                            .get(table_node_id)
                            .map(|table| row_index(table, row_node_id))
                    })
                    .unwrap_or(0);
                return GraphRole::FoldedTableRow {
                    table_node_id,
                    row_node_id,
                    row_index,
                };
            }
        }

        let Some(parent_id) = node.parent else {
            return GraphRole::GraphNode;
        };
        let Some(parent) = store.get(parent_id) else {
            return GraphRole::Pending;
        };

        match parent.kind {
            TreeNodeKind::Mapping => {
                if node.is_map_key {
                    GraphRole::InlineValue
                } else if value_node_builds_child(node) {
                    GraphRole::GraphNode
                } else {
                    GraphRole::InlineValue
                }
            }
            TreeNodeKind::Sequence => match self.sequence_state(store, parent_id) {
                Some(SequenceState {
                    presentation:
                        SequencePresentationState::EmptyOpen
                        | SequencePresentationState::PendingHeaderSchema,
                    ..
                }) => GraphRole::Pending,
                Some(SequenceState {
                    presentation: SequencePresentationState::HeaderlessTable,
                    ..
                }) => {
                    if table_children_can_expand(store, parent_id, config)
                        && value_node_builds_child(node)
                    {
                        GraphRole::GraphNode
                    } else {
                        GraphRole::InlineValue
                    }
                }
                Some(SequenceState {
                    presentation: SequencePresentationState::HeaderTable,
                    ..
                }) => {
                    if node.kind == TreeNodeKind::Mapping {
                        if config.expand_header_table_rows {
                            if value_node_builds_child(node) {
                                GraphRole::GraphNode
                            } else {
                                GraphRole::InlineValue
                            }
                        } else {
                            let row_index = sequence_row_index(store, id)
                                .unwrap_or_else(|| row_index(parent, id));
                            GraphRole::FoldedTableRow {
                                table_node_id: parent_id,
                                row_node_id: id,
                                row_index,
                            }
                        }
                    } else if table_children_can_expand(store, parent_id, config)
                        && value_node_builds_child(node)
                    {
                        GraphRole::GraphNode
                    } else {
                        GraphRole::InlineValue
                    }
                }
                Some(SequenceState {
                    presentation: SequencePresentationState::EmptyClosed,
                    ..
                })
                | None => GraphRole::InlineValue,
            },
            _ => GraphRole::InlineValue,
        }
    }

    fn ensure_graph_node_with_path(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
        path_override: Option<&[PathSeg]>,
    ) -> Option<GraphHandle> {
        if let Some(handle) = self.handle_for(id) {
            self.dirty.mark_shape(handle);
            self.dirty.mark_layout(handle);
            self.reconcile_expandable_sequence_children(store, id, visited, config);
            return Some(handle);
        }

        let parent = if Some(id) == self.root {
            None
        } else {
            self.visible_graph_parent(store, id, visited, config)
        };
        if Some(id) != self.root && store.get(id)?.parent.is_some() && parent.is_none() {
            return None;
        }

        let handle = self.slots.len() as GraphHandle;
        let path = match path_override {
            Some(path) => path.to_vec(),
            None => self.path_from_root(store, id),
        };
        let depth = path.len() as u32;
        let row_in_parent = if Some(id) == self.root {
            0
        } else {
            store
                .get(id)
                .and_then(|node| node.parent)
                .map(|parent_id| parent_row_index(store, parent_id, id))
                .unwrap_or(0)
        };
        let slot = GraphSlot {
            node_id: id,
            children: Vec::new(),
            path,
            depth,
            shape: None,
        };
        self.node_to_handle.insert(id, handle);
        self.handle_to_node.push(id);
        self.slots.push(slot);
        self.dirty.mark_added(handle);

        if let Some(parent_handle) = parent {
            let edge = DirtyEdge {
                from: parent_handle,
                to: handle,
                from_row: row_in_parent,
                to_row: 0,
            };
            self.dirty.mark_edge(edge);
            if let Some(parent_slot) = self.slot_mut(parent_handle) {
                let child_edge = GraphChildEdge {
                    child: handle,
                    from_row: row_in_parent,
                    to_row: 0,
                };
                if !parent_slot.children.contains(&child_edge) {
                    parent_slot.children.push(child_edge);
                    parent_slot
                        .children
                        .sort_by_key(|child| (child.from_row, child.child));
                }
            }
        }
        if let Some(parent_id) = store.get(id).and_then(|node| node.parent) {
            if let Some(parent_node) = store.get(parent_id) {
                if parent_node.kind == TreeNodeKind::Sequence
                    && matches!(
                        self.sequence_state(store, parent_id)
                            .map(|state| state.presentation),
                        Some(SequencePresentationState::HeaderlessTable)
                    )
                    && let Some(table_handle) = self.handle_for(parent_id)
                {
                    let row_index =
                        sequence_row_index(store, id).unwrap_or_else(|| row_index(parent_node, id));
                    self.dirty.mark_table_row(TableRowRef {
                        table_handle,
                        table_node_id: parent_id,
                        row_node_id: id,
                        row_index,
                    });
                }
            }
        }
        self.reconcile_expandable_sequence_children(store, id, visited, config);
        Some(handle)
    }

    fn reconcile_expandable_sequence_children(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
    ) {
        let Some(node) = store.get(id) else {
            return;
        };
        if node.kind != TreeNodeKind::Sequence || !table_children_can_expand(store, id, config) {
            return;
        }
        let children = node.content.clone();
        for child in children {
            if store.get(child).is_some_and(value_node_builds_child) {
                self.reconcile_one(store, child, visited, config);
            }
        }
    }

    fn visible_graph_parent(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
    ) -> Option<GraphHandle> {
        let mut current = store.get(id).and_then(|node| node.parent);
        while let Some(parent_id) = current {
            if let Some(handle) = self.handle_for(parent_id) {
                return Some(handle);
            }
            if matches!(
                self.role_for(store, parent_id, config),
                GraphRole::GraphNode
            ) {
                if let Some(handle) = self.reconcile_one(store, parent_id, visited, config) {
                    return Some(handle);
                }
            }
            if Some(parent_id) == self.root {
                break;
            }
            current = store.get(parent_id).and_then(|node| node.parent);
        }
        None
    }

    fn path_from_root(&mut self, store: &TreeStore, id: NodeId) -> Vec<PathSeg> {
        #[cfg(test)]
        {
            self.metrics.path_rebuilds += 1;
        }
        path_from_root(store, self.root, id)
    }

    fn mark_inline_owner_dirty(&mut self, store: &TreeStore, id: NodeId) {
        let Some(node) = store.get(id) else {
            return;
        };
        if let Some(parent_id) = node.parent {
            if let Some(parent) = store.get(parent_id) {
                if parent.kind == TreeNodeKind::Sequence
                    && matches!(
                        self.sequence_state(store, parent_id)
                            .map(|state| state.presentation),
                        Some(SequencePresentationState::HeaderlessTable)
                    )
                {
                    if let Some(table_handle) = self.handle_for(parent_id) {
                        let row_index =
                            sequence_row_index(store, id).unwrap_or_else(|| row_index(parent, id));
                        self.dirty.mark_table_row(TableRowRef {
                            table_handle,
                            table_node_id: parent_id,
                            row_node_id: id,
                            row_index,
                        });
                        return;
                    }
                }
            }
        }

        let mut current = node.parent;
        while let Some(parent_id) = current {
            if let Some(handle) = self.handle_for(parent_id) {
                self.dirty.mark_shape(handle);
                self.dirty.mark_layout(handle);
                return;
            }
            current = store.get(parent_id).and_then(|parent| parent.parent);
        }
    }
}

fn push_anchor(id: NodeId, anchors: &mut Vec<NodeId>, seen: &mut HashSet<NodeId>) {
    if seen.insert(id) {
        anchors.push(id);
    }
}

pub(crate) fn sequence_presentation_state(
    store: &TreeStore,
    id: NodeId,
) -> Option<SequencePresentationState> {
    sequence_state_from_store(store, id).map(|state| state.presentation)
}

fn sequence_state_from_store(store: &TreeStore, id: NodeId) -> Option<SequenceState> {
    let node = store.get(id)?;
    if node.kind != TreeNodeKind::Sequence {
        return None;
    }

    let first_child = node.content.first().copied();
    let first_child_kind = first_child.and_then(|child| store.get(child).map(|node| node.kind));
    let header_key_count =
        if matches!(first_child_kind, Some(TreeNodeKind::Mapping)) && !node.sequence_closed {
            header_key_count(store, id)
        } else {
            0
        };
    let presentation = match (first_child_kind, node.sequence_closed, header_key_count) {
        (None, true, _) => SequencePresentationState::EmptyClosed,
        (None, false, _) => SequencePresentationState::EmptyOpen,
        (Some(TreeNodeKind::Mapping), true, _) => SequencePresentationState::HeaderTable,
        (Some(TreeNodeKind::Mapping), false, 0) => SequencePresentationState::PendingHeaderSchema,
        (Some(TreeNodeKind::Mapping), false, _) => SequencePresentationState::HeaderTable,
        (Some(_), _, _) => SequencePresentationState::HeaderlessTable,
    };

    Some(SequenceState {
        presentation,
        first_child,
        first_child_kind,
        header_key_count,
        closed: node.sequence_closed,
    })
}

pub(crate) fn is_header_table_sequence(store: &TreeStore, id: NodeId) -> bool {
    matches!(
        sequence_presentation_state(store, id),
        Some(SequencePresentationState::HeaderTable)
    )
}

pub(crate) fn folded_table_row_context(
    store: &TreeStore,
    mut id: NodeId,
    boundary_root: Option<NodeId>,
) -> Option<(NodeId, NodeId)> {
    loop {
        if Some(id) == boundary_root {
            return None;
        }
        let node = store.get(id)?;
        let parent_id = node.parent?;
        let parent = store.get(parent_id)?;
        if node.kind == TreeNodeKind::Mapping
            && parent.kind == TreeNodeKind::Sequence
            && is_header_table_sequence(store, parent_id)
        {
            return Some((parent_id, id));
        }
        id = parent_id;
    }
}

fn table_children_can_expand(
    store: &TreeStore,
    sequence_id: NodeId,
    config: &BuilderConfig,
) -> bool {
    let Some(node) = store.get(sequence_id) else {
        return false;
    };
    if node.kind != TreeNodeKind::Sequence || !node.sequence_closed {
        return false;
    }
    let Some(state) = sequence_state_from_store(store, sequence_id) else {
        return false;
    };
    let row_count = node.content.len() as i32;
    let base_rows = row_count.max(1);
    let total_height = match state.presentation {
        SequencePresentationState::HeaderTable => {
            config.table_header_height + config.table_row_height * base_rows
        }
        SequencePresentationState::HeaderlessTable => config.row_height * base_rows,
        SequencePresentationState::EmptyClosed
        | SequencePresentationState::EmptyOpen
        | SequencePresentationState::PendingHeaderSchema => return false,
    };
    total_height <= config.table_max_height
}

fn header_key_count(store: &TreeStore, sequence_id: NodeId) -> usize {
    let Some(sequence) = store.get(sequence_id) else {
        return 0;
    };
    for &row_id in &sequence.content {
        let Some(row) = store.get(row_id) else {
            continue;
        };
        if row.kind == TreeNodeKind::Mapping && !row.content.is_empty() {
            return 1;
        }
    }
    0
}

fn value_node_builds_child(node: &super::tree_node::TreeNode) -> bool {
    matches!(node.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) && !node.content.is_empty()
}

fn sequence_row_index(store: &TreeStore, child: NodeId) -> Option<usize> {
    store
        .get(child)?
        .sequence_index
        .and_then(|value| usize::try_from(value).ok())
}

fn row_index(parent: &super::tree_node::TreeNode, child: NodeId) -> usize {
    parent
        .content
        .iter()
        .position(|&candidate| candidate == child)
        .unwrap_or(parent.content.len())
}

pub(crate) fn parent_row_index(store: &TreeStore, parent_id: NodeId, child: NodeId) -> i32 {
    let Some(parent) = store.get(parent_id) else {
        return 0;
    };
    match parent.kind {
        TreeNodeKind::Mapping => {
            let mut i = 0;
            let mut row = 0i32;
            while i + 1 < parent.content.len() {
                if parent.content[i + 1] == child {
                    return row;
                }
                i += 2;
                row += 1;
            }
            0
        }
        TreeNodeKind::Sequence => sequence_row_index(store, child)
            .map(|position| position as i32)
            .unwrap_or_else(|| {
                parent
                    .content
                    .iter()
                    .position(|&candidate| candidate == child)
                    .map(|position| position as i32)
                    .unwrap_or(0)
            }),
        _ => 0,
    }
}

fn path_from_root(store: &TreeStore, root: Option<NodeId>, id: NodeId) -> Vec<PathSeg> {
    if Some(id) == root {
        return Vec::new();
    }
    let Some(node) = store.get(id) else {
        return Vec::new();
    };
    let Some(parent_id) = node.parent else {
        return Vec::new();
    };
    let mut path = path_from_root(store, root, parent_id);
    if node.is_map_key {
        path.push(PathSeg::Key(node.value.clone()));
    } else if let Some(key_id) = node.key {
        if let Some(key_node) = store.get(key_id) {
            path.push(PathSeg::Key(key_node.value.clone()));
        }
    } else if let Some(index) = node
        .sequence_index
        .and_then(|value| usize::try_from(value).ok())
    {
        path.push(PathSeg::Index(index));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{Decode, JsonDecoder};

    impl GraphTopology {
        pub(crate) fn folded_row(&self, row_id: NodeId) -> Option<TableRowRef> {
            self.folded_rows.get(&row_id).copied()
        }
    }

    fn store_from_json(source: &str) -> (TreeStore, NodeId) {
        let decoded = JsonDecoder
            .decode_str(source)
            .expect("json fixture should decode");
        (decoded.store, decoded.root)
    }

    fn nested_json_object(depth: usize) -> String {
        let mut source = String::from(r#"{"leaf":0}"#);
        for index in (0..depth).rev() {
            source = format!(r#"{{"k{index}":{source}}}"#);
        }
        source
    }

    #[test]
    fn build_full_keeps_deep_paths_correct_without_recursive_path_rebuilds() {
        let source = nested_json_object(64);
        let (store, root) = store_from_json(&source);
        let mut topology = GraphTopology::new();
        let cfg = crate::core::graph_builder::default_config();

        let dirty = topology.build_full(&store, root, &cfg);

        assert!(!dirty.added_handles().is_empty());
        let deepest = topology
            .slots()
            .iter()
            .enumerate()
            .max_by_key(|(_, slot)| slot.depth)
            .map(|(handle, slot)| (handle as u32, slot))
            .expect("full topology should create at least the root graph node");
        assert_eq!(deepest.1.path.len(), deepest.1.depth as usize);
        assert_eq!(deepest.1.path.first(), Some(&PathSeg::Key("k0".to_owned())));
        assert_eq!(deepest.1.path.last(), Some(&PathSeg::Key("k63".to_owned())));

        let metrics = topology.metrics();
        assert_eq!(metrics.path_rebuilds, 0);
        assert!(
            metrics.path_segment_pushes <= store.len(),
            "full build should push each TreeStore path segment at most once per DFS visit"
        );
    }

    #[test]
    fn graph_topology_apply_folds_header_table_rows_and_tracks_dirty_handles() {
        let (store, root) = store_from_json(r#"{"rows":[{"a":1,"nested":{"b":2}},{"a":3}]}"#);
        let mut topology = GraphTopology::new();

        let cfg = crate::core::graph_builder::default_config();
        let dirty = topology.build_full(&store, root, &cfg);

        assert_eq!(dirty.added_handles(), &[0, 1]);
        assert!(
            dirty
                .added_edges()
                .iter()
                .any(|edge| edge.from == 0 && edge.to == 1)
        );
        assert!(topology.handle_for(root).is_some());
        let rows = store
            .get(root)
            .and_then(|node| node.content.get(1))
            .copied()
            .and_then(|rows_id| store.get(rows_id).map(|_| rows_id))
            .expect("rows sequence exists");
        let first_row = store
            .get(rows)
            .and_then(|node| node.content.first())
            .copied()
            .unwrap();
        let nested = store
            .get(first_row)
            .and_then(|row| row.content.get(3))
            .copied()
            .unwrap();
        assert!(topology.handle_for(first_row).is_none());
        assert!(topology.handle_for(nested).is_none());
        assert!(topology.folded_row(first_row).is_some());
    }
}
