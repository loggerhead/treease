use std::collections::{HashMap, HashSet};

use super::graph_builder::{BuilderConfig, GraphNode, PathSeg};
use crate::tree::{NodeId, TreeNodeKind, TreeStore};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
                row.table_node_id.index(),
                row.row_node_id.index(),
            )
        });
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TopologyMetrics {
    pub path_rebuilds: usize,
    pub path_segment_pushes: usize,
    pub folded_context_misses: usize,
    pub table_expand_misses: usize,
    pub expandable_sequence_scans: usize,
}

#[derive(Debug, Clone, Copy)]
struct GenerationCacheEntry<T: Copy + Default> {
    generation: u32,
    value: T,
}

impl<T: Copy + Default> Default for GenerationCacheEntry<T> {
    fn default() -> Self {
        Self {
            generation: 0,
            value: T::default(),
        }
    }
}

#[derive(Debug, Clone)]
struct GenerationCache<T: Copy + Default> {
    generation: u32,
    entries: Vec<GenerationCacheEntry<T>>,
}

impl<T: Copy + Default> Default for GenerationCache<T> {
    fn default() -> Self {
        Self {
            generation: 1,
            entries: Vec::new(),
        }
    }
}

impl<T: Copy + Default> GenerationCache<T> {
    fn begin_pass(&mut self, node_capacity: usize) {
        if self.generation == u32::MAX {
            self.entries.clear();
            self.generation = 1;
        } else {
            self.generation += 1;
        }
        if self.entries.len() < node_capacity {
            self.entries
                .resize_with(node_capacity, GenerationCacheEntry::default);
        }
    }

    fn get(&self, id: NodeId) -> Option<T> {
        self.entries
            .get(id.index())
            .filter(|entry| entry.generation == self.generation)
            .map(|entry| entry.value)
    }

    fn set(&mut self, id: NodeId, value: T) {
        if id.index() >= self.entries.len() {
            self.entries
                .resize_with(id.index() + 1, GenerationCacheEntry::default);
        }
        if let Some(entry) = self.entries.get_mut(id.index()) {
            entry.generation = self.generation;
            entry.value = value;
        }
    }
}

#[derive(Debug, Clone, Default)]
struct TopologyPassCache {
    reconcile_results: HashMap<NodeId, Option<GraphHandle>>,
    table_expandability: HashMap<NodeId, bool>,
    expanded_sequences: HashSet<NodeId>,
    paths: HashMap<NodeId, Vec<PathSeg>>,
    nearest_sequence_ancestor_by_node: GenerationCache<Option<NodeId>>,
    row_index_by_node: GenerationCache<usize>,
}

impl TopologyPassCache {
    fn begin_pass(&mut self, node_capacity: usize) {
        self.reconcile_results.clear();
        self.table_expandability.clear();
        self.expanded_sequences.clear();
        self.paths.clear();
        self.nearest_sequence_ancestor_by_node
            .begin_pass(node_capacity);
        self.row_index_by_node.begin_pass(node_capacity);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReconcileIntent {
    Dirty,
    ParentLookup,
}

impl ReconcileIntent {
    fn marks_existing(self) -> bool {
        matches!(self, ReconcileIntent::Dirty)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PlannedTableRow {
    table_node_id: NodeId,
    row_node_id: NodeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PatchAttachment {
    parent: Option<NodeId>,
    sequence_index: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GraphWorksetOwner {
    DirtyTableRow(PlannedTableRow),
    DirtyInlineOwner { graph_node_id: NodeId },
    GraphFrontier { node_id: NodeId },
    StructuralSequence { sequence_id: NodeId },
    RootStructural,
}

#[derive(Debug, Clone, Copy)]
struct PlannedGraphOwner {
    owner: GraphWorksetOwner,
    table_row_index_hint: Option<(PlannedTableRow, usize)>,
}

impl PlannedGraphOwner {
    fn new(owner: GraphWorksetOwner) -> Self {
        Self {
            owner,
            table_row_index_hint: None,
        }
    }

    fn dirty_table_row(
        table_node_id: NodeId,
        row_node_id: NodeId,
        row_index: Option<usize>,
    ) -> Self {
        let row = PlannedTableRow {
            table_node_id,
            row_node_id,
        };
        Self {
            owner: GraphWorksetOwner::DirtyTableRow(row),
            table_row_index_hint: row_index.map(|index| (row, index)),
        }
    }
}

fn queue_planned_owner(
    planned: PlannedGraphOwner,
    seen: &mut HashSet<GraphWorksetOwner>,
    owners: &mut Vec<GraphWorksetOwner>,
    table_row_index_hints: &mut HashMap<PlannedTableRow, usize>,
    dirty_table_rows: &mut HashSet<PlannedTableRow>,
) {
    if let Some((row, index)) = planned.table_row_index_hint {
        table_row_index_hints.insert(row, index);
    }
    if let GraphWorksetOwner::DirtyTableRow(row) = planned.owner {
        dirty_table_rows.insert(row);
    }
    if seen.insert(planned.owner) {
        owners.push(planned.owner);
    }
}

#[derive(Debug, Clone, Copy)]
struct AttachmentFastPlan {
    current_role: GraphRole,
    owner: Option<PlannedGraphOwner>,
    needs_previous_role: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PlannerEpoch {
    Previous,
    Current,
}

#[derive(Debug)]
struct GraphPatchPlanner {
    touched_sequences: Vec<NodeId>,
    touched_nodes: Vec<NodeId>,
    workset: Vec<GraphWorksetOwner>,
    sequence_previous: Vec<Option<Option<SequenceState>>>,
    inserted_attachments: Vec<Option<PatchAttachment>>,
    table_row_index_hints: HashMap<PlannedTableRow, usize>,
    seen_touched_sequences: Vec<bool>,
    seen_touched_nodes: Vec<bool>,
    seen_workset: HashSet<GraphWorksetOwner>,
    inserted_nodes: Vec<bool>,
    sealed_nodes: Vec<bool>,
}

impl GraphPatchPlanner {
    fn new(node_capacity: usize, patch_count: usize) -> Self {
        let touched_capacity = patch_count.saturating_mul(2).min(node_capacity);
        Self {
            touched_sequences: Vec::with_capacity(touched_capacity),
            touched_nodes: Vec::with_capacity(touched_capacity),
            workset: Vec::with_capacity(touched_capacity),
            sequence_previous: vec![None; node_capacity],
            inserted_attachments: vec![None; node_capacity],
            table_row_index_hints: HashMap::with_capacity(touched_capacity),
            seen_touched_sequences: vec![false; node_capacity],
            seen_touched_nodes: vec![false; node_capacity],
            seen_workset: HashSet::with_capacity(touched_capacity),
            inserted_nodes: vec![false; node_capacity],
            sealed_nodes: vec![false; node_capacity],
        }
    }

    fn mark_dense(values: &mut [bool], id: NodeId) -> bool {
        if let Some(value) = values.get_mut(id.index()) {
            let was_new = !*value;
            *value = true;
            was_new
        } else {
            false
        }
    }

    fn contains_dense(values: &[bool], id: NodeId) -> bool {
        values.get(id.index()).copied().unwrap_or(false)
    }

    fn set_attachment(&mut self, id: NodeId, attachment: PatchAttachment) {
        if let Some(slot) = self.inserted_attachments.get_mut(id.index()) {
            *slot = Some(attachment);
        }
    }

    fn mark_inserted(&mut self, id: NodeId) {
        Self::mark_dense(&mut self.inserted_nodes, id);
    }

    fn mark_sealed(&mut self, id: NodeId) {
        Self::mark_dense(&mut self.sealed_nodes, id);
    }

    fn is_inserted_or_sealed(&self, id: NodeId) -> bool {
        Self::contains_dense(&self.inserted_nodes, id)
            || Self::contains_dense(&self.sealed_nodes, id)
    }

    fn add_touched_node(&mut self, id: NodeId) {
        if Self::mark_dense(&mut self.seen_touched_nodes, id) {
            self.touched_nodes.push(id);
        }
    }

    fn add_graph_frontier(&mut self, id: NodeId) {
        self.add_owner(GraphWorksetOwner::GraphFrontier { node_id: id });
    }

    fn add_root_structural(&mut self) {
        self.add_owner(GraphWorksetOwner::RootStructural);
    }

    fn add_touched_sequence(
        &mut self,
        previous_states: &HashMap<NodeId, SequenceState>,
        sequence_id: NodeId,
    ) {
        if Self::mark_dense(&mut self.seen_touched_sequences, sequence_id) {
            self.touched_sequences.push(sequence_id);
            if let Some(slot) = self.sequence_previous.get_mut(sequence_id.index()) {
                *slot = Some(previous_states.get(&sequence_id).copied());
            }
        }
    }

    fn add_owner(&mut self, owner: GraphWorksetOwner) {
        if self.seen_workset.insert(owner) {
            self.workset.push(owner);
        }
    }

    fn sort_touched(&mut self) {
        self.touched_sequences.sort_by_key(|id| id.index());
        self.touched_nodes.sort_by_key(|id| id.index());
    }

    fn sort_workset(&mut self) {
        self.workset.sort_by_key(|owner| match owner {
            GraphWorksetOwner::RootStructural => (0, 0, 0),
            GraphWorksetOwner::StructuralSequence { sequence_id } => (1, sequence_id.index(), 0),
            GraphWorksetOwner::GraphFrontier { node_id } => (2, node_id.index(), 0),
            GraphWorksetOwner::DirtyTableRow(row) => {
                (3, row.table_node_id.index(), row.row_node_id.index())
            }
            GraphWorksetOwner::DirtyInlineOwner { graph_node_id } => (4, graph_node_id.index(), 0),
        });
    }

    fn finish(&mut self) {
        self.sort_touched();
        self.sort_workset();
    }
}

type SequenceStateLookup<'a> = dyn FnMut(NodeId) -> Option<SequenceState> + 'a;

struct PlannerResolver<'a> {
    store: &'a TreeStore,
    root: Option<NodeId>,
    config: &'a BuilderConfig,
    sequence_previous: &'a [Option<Option<SequenceState>>],
    sequence_current: &'a HashMap<NodeId, SequenceState>,
    inserted_attachments: &'a [Option<PatchAttachment>],
    folded_context_by_node_previous: Vec<Option<Option<(NodeId, NodeId)>>>,
    folded_context_by_node_current: Vec<Option<Option<(NodeId, NodeId)>>>,
    role_by_node_previous: Vec<Option<GraphRole>>,
    role_by_node_current: Vec<Option<GraphRole>>,
    row_index_by_node: Vec<Option<usize>>,
    #[cfg(test)]
    folded_context_misses: usize,
}

impl<'a> PlannerResolver<'a> {
    fn new(
        store: &'a TreeStore,
        root: Option<NodeId>,
        config: &'a BuilderConfig,
        sequence_previous: &'a [Option<Option<SequenceState>>],
        sequence_current: &'a HashMap<NodeId, SequenceState>,
        inserted_attachments: &'a [Option<PatchAttachment>],
    ) -> Self {
        let node_capacity = store.len();
        Self {
            store,
            root,
            config,
            sequence_previous,
            sequence_current,
            inserted_attachments,
            folded_context_by_node_previous: vec![None; node_capacity],
            folded_context_by_node_current: vec![None; node_capacity],
            role_by_node_previous: vec![None; node_capacity],
            role_by_node_current: vec![None; node_capacity],
            row_index_by_node: vec![None; node_capacity],
            #[cfg(test)]
            folded_context_misses: 0,
        }
    }

    fn sequence_state(&self, sequence_id: NodeId, epoch: PlannerEpoch) -> Option<SequenceState> {
        match epoch {
            PlannerEpoch::Previous => {
                match self
                    .sequence_previous
                    .get(sequence_id.index())
                    .copied()
                    .flatten()
                {
                    Some(previous) => previous,
                    None => self.sequence_current.get(&sequence_id).copied(),
                }
            }
            PlannerEpoch::Current => self.sequence_current.get(&sequence_id).copied(),
        }
    }

    fn folded_context(&mut self, id: NodeId, epoch: PlannerEpoch) -> Option<(NodeId, NodeId)> {
        if let Some(cached) = self.folded_context_slot(epoch, id).copied().flatten() {
            return cached;
        }
        let mut trail = Vec::new();
        let mut current = id;
        let result = loop {
            if let Some(cached) = self.folded_context_slot(epoch, current).copied().flatten() {
                break cached;
            }
            trail.push(current);
            if Some(current) == self.root {
                break None;
            }
            let Some(node) = self.store.get(current) else {
                break None;
            };
            let Some(parent_id) = node.parent else {
                break None;
            };
            let Some(parent) = self.store.get(parent_id) else {
                break None;
            };
            if node.kind == TreeNodeKind::Mapping
                && parent.kind == TreeNodeKind::Sequence
                && matches!(
                    self.sequence_state(parent_id, epoch)
                        .map(|state| state.presentation),
                    Some(SequencePresentationState::HeaderTable)
                )
            {
                break Some((parent_id, current));
            }
            current = parent_id;
        };
        for visited in trail {
            if let Some(slot) = self.folded_context_slot_mut(epoch, visited) {
                *slot = Some(result);
            }
        }
        #[cfg(test)]
        {
            self.folded_context_misses += 1;
        }
        result
    }

    fn folded_context_slot(
        &self,
        epoch: PlannerEpoch,
        id: NodeId,
    ) -> Option<&Option<Option<(NodeId, NodeId)>>> {
        match epoch {
            PlannerEpoch::Previous => self.folded_context_by_node_previous.get(id.index()),
            PlannerEpoch::Current => self.folded_context_by_node_current.get(id.index()),
        }
    }

    fn folded_context_slot_mut(
        &mut self,
        epoch: PlannerEpoch,
        id: NodeId,
    ) -> Option<&mut Option<Option<(NodeId, NodeId)>>> {
        match epoch {
            PlannerEpoch::Previous => self.folded_context_by_node_previous.get_mut(id.index()),
            PlannerEpoch::Current => self.folded_context_by_node_current.get_mut(id.index()),
        }
    }

    fn role_slot(&self, epoch: PlannerEpoch, id: NodeId) -> Option<&Option<GraphRole>> {
        match epoch {
            PlannerEpoch::Previous => self.role_by_node_previous.get(id.index()),
            PlannerEpoch::Current => self.role_by_node_current.get(id.index()),
        }
    }

    fn role_slot_mut(&mut self, epoch: PlannerEpoch, id: NodeId) -> Option<&mut Option<GraphRole>> {
        match epoch {
            PlannerEpoch::Previous => self.role_by_node_previous.get_mut(id.index()),
            PlannerEpoch::Current => self.role_by_node_current.get_mut(id.index()),
        }
    }

    fn role_for(&mut self, id: NodeId, epoch: PlannerEpoch) -> GraphRole {
        let cached = self.role_slot(epoch, id).copied().flatten();
        if let Some(role) = cached {
            return role;
        }
        let role = self.compute_role_for(id, epoch);
        if let Some(slot) = self.role_slot_mut(epoch, id) {
            *slot = Some(role);
        }
        role
    }

    fn compute_role_for(&mut self, id: NodeId, epoch: PlannerEpoch) -> GraphRole {
        let Some(node) = self.store.get(id) else {
            return GraphRole::Pending;
        };
        if Some(id) == self.root {
            return if node.kind == TreeNodeKind::Sequence
                && matches!(
                    self.sequence_state(id, epoch)
                        .map(|state| state.presentation),
                    Some(
                        SequencePresentationState::EmptyOpen
                            | SequencePresentationState::PendingHeaderSchema
                    )
                ) {
                GraphRole::Pending
            } else {
                GraphRole::GraphNode
            };
        }

        if !self.config.expand_header_table_rows
            && let Some((table_node_id, row_node_id)) = self.folded_context(id, epoch)
        {
            let row_index = self
                .store
                .get(table_node_id)
                .map(|table| self.row_index_for_sequence_child(table, row_node_id, None))
                .unwrap_or(0);
            return GraphRole::FoldedTableRow {
                table_node_id,
                row_node_id,
                row_index,
            };
        }

        match node.parent {
            None => GraphRole::GraphNode,
            Some(parent_id) => match self.store.get(parent_id) {
                None => GraphRole::Pending,
                Some(parent) => match parent.kind {
                    TreeNodeKind::Mapping => {
                        if node.is_map_key {
                            GraphRole::InlineValue
                        } else if value_node_builds_child(node) {
                            GraphRole::GraphNode
                        } else {
                            GraphRole::InlineValue
                        }
                    }
                    TreeNodeKind::Sequence => match self.sequence_state(parent_id, epoch) {
                        Some(SequenceState {
                            presentation:
                                SequencePresentationState::EmptyOpen
                                | SequencePresentationState::PendingHeaderSchema,
                            ..
                        }) => GraphRole::Pending,
                        Some(
                            state @ SequenceState {
                                presentation: SequencePresentationState::HeaderlessTable,
                                ..
                            },
                        ) => {
                            if table_children_can_expand(parent, &state, self.config)
                                && value_node_builds_child(node)
                            {
                                GraphRole::GraphNode
                            } else {
                                GraphRole::InlineValue
                            }
                        }
                        Some(
                            state @ SequenceState {
                                presentation: SequencePresentationState::HeaderTable,
                                ..
                            },
                        ) => {
                            if node.kind == TreeNodeKind::Mapping {
                                if self.config.expand_header_table_rows {
                                    if value_node_builds_child(node) {
                                        GraphRole::GraphNode
                                    } else {
                                        GraphRole::InlineValue
                                    }
                                } else {
                                    let row_index =
                                        self.row_index_for_sequence_child(parent, id, None);
                                    GraphRole::FoldedTableRow {
                                        table_node_id: parent_id,
                                        row_node_id: id,
                                        row_index,
                                    }
                                }
                            } else if table_children_can_expand(parent, &state, self.config)
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
                },
            },
        }
    }

    fn inline_owner(&mut self, id: NodeId) -> Option<GraphWorksetOwner> {
        let node = self.store.get(id)?;
        if let Some(parent_id) = node.parent {
            if let Some(parent) = self.store.get(parent_id) {
                if parent.kind == TreeNodeKind::Sequence
                    && matches!(
                        self.sequence_state(parent_id, PlannerEpoch::Current)
                            .map(|state| state.presentation),
                        Some(SequencePresentationState::HeaderlessTable)
                    )
                {
                    return Some(GraphWorksetOwner::DirtyTableRow(PlannedTableRow {
                        table_node_id: parent_id,
                        row_node_id: id,
                    }));
                }
            }
        }

        let mut current = node.parent;
        while let Some(parent_id) = current {
            if self.role_for(parent_id, PlannerEpoch::Current) == GraphRole::GraphNode {
                return Some(GraphWorksetOwner::DirtyInlineOwner {
                    graph_node_id: parent_id,
                });
            }
            current = self.store.get(parent_id).and_then(|parent| parent.parent);
        }
        self.root
            .map(|graph_node_id| GraphWorksetOwner::DirtyInlineOwner { graph_node_id })
    }

    fn attachment_sequence_index(attachment: &PatchAttachment) -> Option<usize> {
        attachment
            .sequence_index
            .and_then(|value| usize::try_from(value).ok())
    }

    fn row_index_for_sequence_child(
        &mut self,
        parent: &crate::tree::tree_node::TreeNode,
        child: NodeId,
        index_hint: Option<usize>,
    ) -> usize {
        if let Some(index) = index_hint.or_else(|| sequence_row_index(self.store, child)) {
            if let Some(slot) = self.row_index_by_node.get_mut(child.index()) {
                *slot = Some(index);
            }
            return index;
        }
        if let Some(index) = self.row_index_by_node.get(child.index()).copied().flatten() {
            return index;
        }
        for (index, &candidate) in parent.content.iter().enumerate() {
            if let Some(slot) = self.row_index_by_node.get_mut(candidate.index()) {
                *slot = Some(index);
            }
        }
        self.row_index_by_node
            .get(child.index())
            .copied()
            .flatten()
            .unwrap_or(parent.content.len())
    }

    fn folded_row_attachment_plan(
        &mut self,
        row_node_id: NodeId,
        row_index_hint: Option<usize>,
    ) -> Option<AttachmentFastPlan> {
        let row_node = self.store.get(row_node_id)?;
        if row_node.kind != TreeNodeKind::Mapping {
            return None;
        }
        let table_node_id = row_node.parent?;
        let table = self.store.get(table_node_id)?;
        if table.kind != TreeNodeKind::Sequence
            || self.config.expand_header_table_rows
            || !matches!(
                self.sequence_state(table_node_id, PlannerEpoch::Current)
                    .map(|state| state.presentation),
                Some(SequencePresentationState::HeaderTable)
            )
        {
            return None;
        }

        let row_index = self.row_index_for_sequence_child(table, row_node_id, row_index_hint);
        Some(AttachmentFastPlan {
            current_role: GraphRole::FoldedTableRow {
                table_node_id,
                row_node_id,
                row_index,
            },
            owner: Some(PlannedGraphOwner::dirty_table_row(
                table_node_id,
                row_node_id,
                Some(row_index),
            )),
            needs_previous_role: false,
        })
    }

    fn owner_from_attachment_fast(&mut self, id: NodeId) -> Option<AttachmentFastPlan> {
        let attachment = self
            .inserted_attachments
            .get(id.index())
            .copied()
            .flatten()?;
        let parent_id = attachment
            .parent
            .or_else(|| self.store.get(id).and_then(|node| node.parent))?;
        let parent = self.store.get(parent_id)?;
        let node = self.store.get(id)?;

        match parent.kind {
            TreeNodeKind::Sequence => {
                let row_index_hint = Self::attachment_sequence_index(&attachment);
                let Some(state) = self.sequence_state(parent_id, PlannerEpoch::Current) else {
                    return Some(AttachmentFastPlan {
                        current_role: GraphRole::InlineValue,
                        owner: self.inline_owner(id).map(PlannedGraphOwner::new),
                        needs_previous_role: false,
                    });
                };
                match state.presentation {
                    SequencePresentationState::EmptyOpen
                    | SequencePresentationState::PendingHeaderSchema => Some(AttachmentFastPlan {
                        current_role: GraphRole::Pending,
                        owner: None,
                        needs_previous_role: false,
                    }),
                    SequencePresentationState::HeaderTable
                        if !self.config.expand_header_table_rows
                            && node.kind == TreeNodeKind::Mapping =>
                    {
                        self.folded_row_attachment_plan(id, row_index_hint)
                    }
                    SequencePresentationState::HeaderlessTable => {
                        let current_role = if table_children_can_expand(parent, &state, self.config)
                            && value_node_builds_child(node)
                        {
                            GraphRole::GraphNode
                        } else {
                            GraphRole::InlineValue
                        };
                        let owner = match current_role {
                            GraphRole::GraphNode => {
                                PlannedGraphOwner::new(GraphWorksetOwner::GraphFrontier {
                                    node_id: id,
                                })
                            }
                            GraphRole::InlineValue => {
                                let row_index =
                                    self.row_index_for_sequence_child(parent, id, row_index_hint);
                                PlannedGraphOwner::dirty_table_row(parent_id, id, Some(row_index))
                            }
                            _ => return None,
                        };
                        Some(AttachmentFastPlan {
                            current_role,
                            owner: Some(owner),
                            needs_previous_role: matches!(current_role, GraphRole::GraphNode),
                        })
                    }
                    SequencePresentationState::EmptyClosed => Some(AttachmentFastPlan {
                        current_role: GraphRole::InlineValue,
                        owner: self.inline_owner(id).map(PlannedGraphOwner::new),
                        needs_previous_role: false,
                    }),
                    SequencePresentationState::HeaderTable
                        if table_children_can_expand(parent, &state, self.config)
                            && value_node_builds_child(node) =>
                    {
                        Some(AttachmentFastPlan {
                            current_role: GraphRole::GraphNode,
                            owner: Some(PlannedGraphOwner::new(GraphWorksetOwner::GraphFrontier {
                                node_id: id,
                            })),
                            needs_previous_role: true,
                        })
                    }
                    SequencePresentationState::HeaderTable => None,
                }
            }
            TreeNodeKind::Mapping => {
                if let Some(plan) = self.folded_row_attachment_plan(parent_id, None) {
                    return Some(plan);
                }
                let current_role = if node.is_map_key {
                    GraphRole::InlineValue
                } else if value_node_builds_child(node) {
                    GraphRole::GraphNode
                } else {
                    GraphRole::InlineValue
                };
                match current_role {
                    GraphRole::GraphNode => Some(AttachmentFastPlan {
                        current_role,
                        owner: Some(PlannedGraphOwner::new(GraphWorksetOwner::GraphFrontier {
                            node_id: id,
                        })),
                        needs_previous_role: true,
                    }),
                    GraphRole::InlineValue
                        if self.role_for(parent_id, PlannerEpoch::Current)
                            == GraphRole::GraphNode =>
                    {
                        Some(AttachmentFastPlan {
                            current_role,
                            owner: Some(PlannedGraphOwner::new(
                                GraphWorksetOwner::DirtyInlineOwner {
                                    graph_node_id: parent_id,
                                },
                            )),
                            needs_previous_role: false,
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn owner_from_attachment(
        &mut self,
        id: NodeId,
        current_role: GraphRole,
    ) -> Option<PlannedGraphOwner> {
        let attachment = self
            .inserted_attachments
            .get(id.index())
            .copied()
            .flatten()?;
        let parent_id = attachment
            .parent
            .or_else(|| self.store.get(id).and_then(|node| node.parent))?;
        let parent = self.store.get(parent_id)?;
        let node = self.store.get(id)?;

        match parent.kind {
            TreeNodeKind::Sequence => {
                let state = self.sequence_state(parent_id, PlannerEpoch::Current)?;
                match state.presentation {
                    SequencePresentationState::HeaderTable
                        if !self.config.expand_header_table_rows
                            && node.kind == TreeNodeKind::Mapping =>
                    {
                        let row_index = self.row_index_for_sequence_child(
                            parent,
                            id,
                            Self::attachment_sequence_index(&attachment),
                        );
                        Some(PlannedGraphOwner::dirty_table_row(
                            parent_id,
                            id,
                            Some(row_index),
                        ))
                    }
                    SequencePresentationState::HeaderlessTable => match current_role {
                        GraphRole::GraphNode => {
                            Some(PlannedGraphOwner::new(GraphWorksetOwner::GraphFrontier {
                                node_id: id,
                            }))
                        }
                        GraphRole::InlineValue => {
                            let row_index = self.row_index_for_sequence_child(
                                parent,
                                id,
                                Self::attachment_sequence_index(&attachment),
                            );
                            Some(PlannedGraphOwner::dirty_table_row(
                                parent_id,
                                id,
                                Some(row_index),
                            ))
                        }
                        _ => None,
                    },
                    _ => None,
                }
            }
            TreeNodeKind::Mapping => match current_role {
                GraphRole::InlineValue => {
                    if self.role_for(parent_id, PlannerEpoch::Current) == GraphRole::GraphNode {
                        Some(PlannedGraphOwner::new(
                            GraphWorksetOwner::DirtyInlineOwner {
                                graph_node_id: parent_id,
                            },
                        ))
                    } else {
                        None
                    }
                }
                GraphRole::GraphNode => {
                    Some(PlannedGraphOwner::new(GraphWorksetOwner::GraphFrontier {
                        node_id: id,
                    }))
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn transition_owner_for_role(&self, node_id: NodeId, role: GraphRole) -> GraphWorksetOwner {
        let target = structural_escalation_target_for_role(self.store, node_id, role);
        if Some(target) == self.root {
            GraphWorksetOwner::RootStructural
        } else if self
            .store
            .get(target)
            .is_some_and(|node| node.kind == TreeNodeKind::Sequence)
        {
            GraphWorksetOwner::StructuralSequence {
                sequence_id: target,
            }
        } else {
            GraphWorksetOwner::GraphFrontier { node_id: target }
        }
    }

    fn has_transitioned_sequence_ancestor(
        &self,
        id: NodeId,
        transitioned_sequences: &HashSet<NodeId>,
    ) -> bool {
        let mut current = Some(id);
        while let Some(node_id) = current {
            let Some(node) = self.store.get(node_id) else {
                return false;
            };
            if node.kind == TreeNodeKind::Sequence && transitioned_sequences.contains(&node_id) {
                return true;
            }
            current = node.parent;
        }
        false
    }

    #[cfg(test)]
    fn folded_context_misses(&self) -> usize {
        self.folded_context_misses
    }
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
        return Some(PathSeg::Key(store.value_string_for(id).ok()?));
    }
    if let Some(key_id) = node.key() {
        return Some(PathSeg::Key(store.value_string_for(key_id).ok()?));
    }
    node.sequence_index()
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
    pass_cache: TopologyPassCache,
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
        self.pass_cache.begin_pass(store.len());
        #[cfg(test)]
        {
            self.metrics = TopologyMetrics::default();
        }
        self.dirty.clear();
        if self.root.is_none() {
            self.root = Some(root);
        }

        let mut planner = self.plan_graph_patches(store, root, patches);
        for &sequence_id in &planner.touched_sequences {
            self.refresh_sequence_state(store, sequence_id);
        }
        self.resolve_graph_patch_plan(store, config, &mut planner);
        self.execute_graph_patch_plan(store, config, planner);
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
        self.pass_cache.begin_pass(store.len());
        self.refresh_sequence_subtree(store, root);

        let mut visited = HashSet::new();
        let mut path = PathCursor::default();
        self.reconcile_full_with_path(store, root, config, &mut visited, &mut path);
        self.dirty.finish();
        self.dirty.clone()
    }

    fn plan_graph_patches(
        &mut self,
        store: &TreeStore,
        root: NodeId,
        patches: &[crate::stream::tree_patch::TreePatch],
    ) -> GraphPatchPlanner {
        let mut planner = GraphPatchPlanner::new(store.len(), patches.len());
        for patch in patches {
            self.collect_patch_plan(store, patch, root, &mut planner);
        }
        if planner.touched_nodes.is_empty() && self.handle_for(root).is_none() {
            planner.add_touched_node(root);
            planner.add_graph_frontier(root);
            planner.add_root_structural();
        }
        for node_id in planner.touched_nodes.clone() {
            self.add_touched_sequence_ancestors(store, node_id, &mut planner);
        }
        planner.finish();
        planner
    }

    fn collect_patch_plan(
        &mut self,
        store: &TreeStore,
        patch: &crate::stream::tree_patch::TreePatch,
        root: NodeId,
        planner: &mut GraphPatchPlanner,
    ) {
        use crate::stream::tree_patch::TreePatch;
        match patch {
            TreePatch::DocumentStarted { root } | TreePatch::DocumentEnded { root } => {
                planner.add_touched_node(*root);
                planner.add_root_structural();
            }
            TreePatch::NodeInserted {
                node_id,
                parent,
                key,
                sequence_index,
                ..
            } => {
                planner.mark_inserted(*node_id);
                planner.set_attachment(
                    *node_id,
                    PatchAttachment {
                        parent: *parent,
                        sequence_index: *sequence_index,
                    },
                );
                planner.add_touched_node(*node_id);
                let parent = (*parent).or_else(|| store.get(*node_id).and_then(|node| node.parent));
                if let Some(parent) = parent {
                    planner.add_touched_node(parent);
                }
                if let Some(key) = *key {
                    planner.add_touched_node(key);
                }
            }
            TreePatch::KeyInserted { key_id, parent, .. } => {
                planner.add_touched_node(*key_id);
                planner.add_touched_node(*parent);
            }
            TreePatch::NodeSealed { node_id } => {
                planner.mark_sealed(*node_id);
                planner.add_touched_node(*node_id);
                if let Some(parent) = store.get(*node_id).and_then(|node| node.parent) {
                    planner.add_touched_node(parent);
                }
            }
            TreePatch::DiagnosticAdded { .. } => {
                if self.root.is_none() {
                    planner.add_touched_node(root);
                    planner.add_graph_frontier(root);
                }
            }
        }
    }

    fn add_touched_sequence_ancestors(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        planner: &mut GraphPatchPlanner,
    ) {
        let mut current = Some(id);
        while let Some(start) = current {
            let Some(sequence_id) = self.nearest_sequence_ancestor(store, start) else {
                break;
            };
            planner.add_touched_sequence(&self.sequence_states, sequence_id);
            current = store.get(sequence_id).and_then(|node| node.parent);
        }
    }

    fn nearest_sequence_ancestor(&mut self, store: &TreeStore, id: NodeId) -> Option<NodeId> {
        if let Some(cached) = self.pass_cache.nearest_sequence_ancestor_by_node.get(id) {
            return cached;
        }

        let mut trail = Vec::new();
        let mut current = id;
        let result = loop {
            if let Some(cached) = self
                .pass_cache
                .nearest_sequence_ancestor_by_node
                .get(current)
            {
                break cached;
            }
            trail.push(current);
            let Some(node) = store.get(current) else {
                break None;
            };
            if node.kind == TreeNodeKind::Sequence {
                break Some(current);
            }
            let Some(parent) = node.parent else {
                break None;
            };
            current = parent;
        };

        for visited in trail {
            self.pass_cache
                .nearest_sequence_ancestor_by_node
                .set(visited, result);
        }
        result
    }

    fn resolve_graph_patch_plan(
        &mut self,
        store: &TreeStore,
        config: &BuilderConfig,
        planner: &mut GraphPatchPlanner,
    ) {
        let touched_sequences = planner.touched_sequences.clone();
        let touched_nodes = planner.touched_nodes.clone();
        let mut planned_owner_seen = planner.seen_workset.clone();
        let mut planned_owners = Vec::new();
        let mut table_row_index_hints = HashMap::new();
        let mut dirty_table_rows = HashSet::new();
        let mut transitioned_sequences = HashSet::new();
        let mut resolver = PlannerResolver::new(
            store,
            self.root,
            config,
            &planner.sequence_previous,
            &self.sequence_states,
            &planner.inserted_attachments,
        );
        for sequence_id in touched_sequences {
            let previous = resolver.sequence_state(sequence_id, PlannerEpoch::Previous);
            let current = resolver.sequence_state(sequence_id, PlannerEpoch::Current);
            let previous_expandable = previous
                .as_ref()
                .zip(store.get(sequence_id))
                .is_some_and(|(state, node)| table_children_can_expand(node, state, config));
            let current_expandable = current
                .as_ref()
                .zip(store.get(sequence_id))
                .is_some_and(|(state, node)| table_children_can_expand(node, state, config));
            if sequence_state_transition_requires_structural_reconcile(
                previous.as_ref(),
                current.as_ref(),
                previous_expandable,
                current_expandable,
            ) {
                if Some(sequence_id) == self.root {
                    queue_planned_owner(
                        PlannedGraphOwner::new(GraphWorksetOwner::RootStructural),
                        &mut planned_owner_seen,
                        &mut planned_owners,
                        &mut table_row_index_hints,
                        &mut dirty_table_rows,
                    );
                } else {
                    queue_planned_owner(
                        PlannedGraphOwner::new(GraphWorksetOwner::StructuralSequence {
                            sequence_id,
                        }),
                        &mut planned_owner_seen,
                        &mut planned_owners,
                        &mut table_row_index_hints,
                        &mut dirty_table_rows,
                    );
                }
                transitioned_sequences.insert(sequence_id);
            }
        }

        for id in touched_nodes {
            if !config.expand_header_table_rows
                && let Some((table_node_id, row_node_id)) =
                    resolver.folded_context(id, PlannerEpoch::Current)
                && dirty_table_rows.contains(&PlannedTableRow {
                    table_node_id,
                    row_node_id,
                })
            {
                continue;
            }

            if let Some(fast_plan) = resolver.owner_from_attachment_fast(id) {
                if fast_plan.needs_previous_role {
                    let previous_role = resolver.role_for(id, PlannerEpoch::Previous);
                    if role_transition_requires_structural_reconcile(
                        previous_role,
                        fast_plan.current_role,
                    ) {
                        let current_owner =
                            resolver.transition_owner_for_role(id, fast_plan.current_role);
                        let previous_owner = resolver.transition_owner_for_role(id, previous_role);
                        queue_planned_owner(
                            PlannedGraphOwner::new(current_owner),
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                        queue_planned_owner(
                            PlannedGraphOwner::new(previous_owner),
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                    }
                }
                if let Some(owner) = fast_plan.owner {
                    queue_planned_owner(
                        owner,
                        &mut planned_owner_seen,
                        &mut planned_owners,
                        &mut table_row_index_hints,
                        &mut dirty_table_rows,
                    );
                }
                continue;
            }

            let current_role = resolver.role_for(id, PlannerEpoch::Current);

            if planner.is_inserted_or_sealed(id)
                || matches!(current_role, GraphRole::Pending)
                || resolver.has_transitioned_sequence_ancestor(id, &transitioned_sequences)
            {
                let previous_role = resolver.role_for(id, PlannerEpoch::Previous);
                if role_transition_requires_structural_reconcile(previous_role, current_role)
                    && (planner.is_inserted_or_sealed(id)
                        || matches!(previous_role, GraphRole::Pending)
                        || matches!(current_role, GraphRole::Pending))
                {
                    let current_owner = resolver.transition_owner_for_role(id, current_role);
                    let previous_owner = resolver.transition_owner_for_role(id, previous_role);
                    queue_planned_owner(
                        PlannedGraphOwner::new(current_owner),
                        &mut planned_owner_seen,
                        &mut planned_owners,
                        &mut table_row_index_hints,
                        &mut dirty_table_rows,
                    );
                    queue_planned_owner(
                        PlannedGraphOwner::new(previous_owner),
                        &mut planned_owner_seen,
                        &mut planned_owners,
                        &mut table_row_index_hints,
                        &mut dirty_table_rows,
                    );
                }
            }

            match current_role {
                GraphRole::Pending => {}
                GraphRole::InlineValue => {
                    if let Some(owner) = resolver
                        .owner_from_attachment(id, current_role)
                        .or_else(|| resolver.inline_owner(id).map(PlannedGraphOwner::new))
                    {
                        queue_planned_owner(
                            owner,
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                    }
                }
                GraphRole::FoldedTableRow {
                    table_node_id,
                    row_node_id,
                    row_index,
                } => {
                    let row_owner = PlannedGraphOwner::dirty_table_row(
                        table_node_id,
                        row_node_id,
                        Some(row_index),
                    );
                    if planned_owner_seen.contains(&row_owner.owner) {
                        queue_planned_owner(
                            row_owner,
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                    } else if let Some(owner) = resolver.owner_from_attachment(id, current_role) {
                        queue_planned_owner(
                            owner,
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                    } else {
                        queue_planned_owner(
                            row_owner,
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                    }
                }
                GraphRole::GraphNode => {
                    let frontier_owner =
                        PlannedGraphOwner::new(GraphWorksetOwner::GraphFrontier { node_id: id });
                    if planned_owner_seen.contains(&frontier_owner.owner) {
                        continue;
                    } else if let Some(owner) = resolver.owner_from_attachment(id, current_role) {
                        queue_planned_owner(
                            owner,
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                    } else {
                        queue_planned_owner(
                            frontier_owner,
                            &mut planned_owner_seen,
                            &mut planned_owners,
                            &mut table_row_index_hints,
                            &mut dirty_table_rows,
                        );
                    }
                }
            }
        }
        #[cfg(test)]
        let folded_context_misses = resolver.folded_context_misses();
        drop(resolver);
        planner.table_row_index_hints.extend(table_row_index_hints);
        for owner in planned_owners {
            planner.add_owner(owner);
        }
        #[cfg(test)]
        {
            self.metrics.folded_context_misses += folded_context_misses;
        }
        planner.sort_workset();
    }

    fn execute_graph_patch_plan(
        &mut self,
        store: &TreeStore,
        config: &BuilderConfig,
        planner: GraphPatchPlanner,
    ) {
        let mut visited = HashSet::new();
        let GraphPatchPlanner {
            workset,
            table_row_index_hints,
            ..
        } = planner;
        for owner in workset {
            match owner {
                GraphWorksetOwner::DirtyTableRow(row) => {
                    self.mark_planned_table_row_dirty(
                        store,
                        row,
                        table_row_index_hints.get(&row).copied(),
                        &mut visited,
                        config,
                    );
                }
                GraphWorksetOwner::DirtyInlineOwner { graph_node_id } => {
                    self.mark_graph_node_owner_dirty(store, graph_node_id, &mut visited, config);
                }
                GraphWorksetOwner::GraphFrontier { node_id } => {
                    self.reconcile_one(store, node_id, &mut visited, config);
                }
                GraphWorksetOwner::StructuralSequence { sequence_id } => {
                    self.reconcile_one(store, sequence_id, &mut visited, config);
                }
                GraphWorksetOwner::RootStructural => {
                    if let Some(root) = self.root {
                        self.reconcile_one(store, root, &mut visited, config);
                    }
                }
            }
        }
    }

    fn mark_graph_node_owner_dirty(
        &mut self,
        store: &TreeStore,
        graph_node_id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
    ) {
        if let Some(handle) = self.handle_for(graph_node_id) {
            self.dirty.mark_shape(handle);
            self.dirty.mark_layout(handle);
            return;
        }
        self.reconcile_one(store, graph_node_id, visited, config);
    }

    fn mark_planned_table_row_dirty(
        &mut self,
        store: &TreeStore,
        row: PlannedTableRow,
        row_index_hint: Option<usize>,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
    ) {
        let Some(table_handle) = self
            .handle_for(row.table_node_id)
            .or_else(|| self.reconcile_one(store, row.table_node_id, visited, config))
        else {
            return;
        };
        let row_index = row_index_hint.unwrap_or_else(|| {
            self.sequence_child_row_index(store, row.table_node_id, row.row_node_id, None)
        });
        let row_ref = TableRowRef {
            table_handle,
            table_node_id: row.table_node_id,
            row_node_id: row.row_node_id,
            row_index,
        };
        self.folded_rows.insert(row.row_node_id, row_ref);
        self.dirty.mark_table_row(row_ref);
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

    fn refresh_sequence_state(&mut self, store: &TreeStore, id: NodeId) -> Option<SequenceState> {
        let state = sequence_state_from_store(store, id)?;
        self.sequence_states.insert(id, state);
        Some(state)
    }

    fn sequence_state(&mut self, store: &TreeStore, id: NodeId) -> Option<SequenceState> {
        if let Some(state) = self.sequence_states.get(&id) {
            return Some(*state);
        }
        self.refresh_sequence_state(store, id)
    }

    fn reconcile_full_with_path(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        config: &BuilderConfig,
        visited: &mut HashSet<NodeId>,
        path: &mut PathCursor,
    ) {
        self.reconcile_one_with_path(
            store,
            id,
            visited,
            config,
            Some(path.as_slice()),
            ReconcileIntent::Dirty,
        );
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
        self.reconcile_one_with_path(store, id, visited, config, None, ReconcileIntent::Dirty)
    }

    fn reconcile_one_with_path(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
        path_override: Option<&[PathSeg]>,
        intent: ReconcileIntent,
    ) -> Option<GraphHandle> {
        if intent == ReconcileIntent::Dirty
            && path_override.is_none()
            && let Some(result) = self.pass_cache.reconcile_results.get(&id).copied()
        {
            return result;
        }
        if !visited.insert(id) {
            return self.handle_for(id);
        }
        let role = self.role_for(store, id, config);
        let result = match role {
            GraphRole::Pending => None,
            GraphRole::InlineValue => {
                if intent.marks_existing() {
                    self.mark_inline_owner_dirty(store, id);
                }
                None
            }
            GraphRole::FoldedTableRow {
                table_node_id,
                row_node_id,
                row_index,
            } => {
                if intent.marks_existing() {
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
                }
                None
            }
            GraphRole::GraphNode => {
                self.ensure_graph_node_with_path(store, id, visited, config, path_override, intent)
            }
        };
        if intent == ReconcileIntent::Dirty && path_override.is_none() {
            self.pass_cache.reconcile_results.insert(id, result);
        }
        result
    }

    fn role_for(&mut self, store: &TreeStore, id: NodeId, config: &BuilderConfig) -> GraphRole {
        let root = self.root;
        let mut current_lookup = |sequence_id: NodeId| self.sequence_state(store, sequence_id);
        planner_role_for(store, root, id, config, &mut current_lookup)
    }

    fn ensure_graph_node_with_path(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
        path_override: Option<&[PathSeg]>,
        intent: ReconcileIntent,
    ) -> Option<GraphHandle> {
        if let Some(handle) = self.handle_for(id) {
            if intent.marks_existing() {
                self.dirty.mark_shape(handle);
                self.dirty.mark_layout(handle);
                self.reconcile_expandable_sequence_children(store, id, visited, config);
            }
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
        let row_in_parent = parent.map(|(_, row)| row).unwrap_or(0);
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

        if let Some((parent_handle, _)) = parent {
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
                insert_child_edge(&mut parent_slot.children, child_edge);
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
                    let row_index = self.sequence_child_row_index(store, parent_id, id, None);
                    self.dirty.mark_table_row(TableRowRef {
                        table_handle,
                        table_node_id: parent_id,
                        row_node_id: id,
                        row_index,
                    });
                }
            }
        }
        self.reconcile_new_graph_node_children(store, id, visited, config);
        Some(handle)
    }

    fn reconcile_new_graph_node_children(
        &mut self,
        store: &TreeStore,
        id: NodeId,
        visited: &mut HashSet<NodeId>,
        config: &BuilderConfig,
    ) {
        let Some(node) = store.get(id) else {
            return;
        };
        match node.kind {
            TreeNodeKind::Mapping => {
                for child in node.content.iter().skip(1).step_by(2).copied() {
                    self.reconcile_one(store, child, visited, config);
                }
            }
            TreeNodeKind::Sequence => {
                self.reconcile_expandable_sequence_children(store, id, visited, config);
            }
            TreeNodeKind::Scalar | TreeNodeKind::Alias | TreeNodeKind::Unknown => {}
        }
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
        if node.kind != TreeNodeKind::Sequence
            || !self.table_children_can_expand_timed(store, id, config)
        {
            return;
        }
        if !self.pass_cache.expanded_sequences.insert(id) {
            return;
        }
        #[cfg(test)]
        {
            self.metrics.expandable_sequence_scans += 1;
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
    ) -> Option<(GraphHandle, i32)> {
        // Edge rows belong to the visible parent. Advancing this child across folded
        // ancestors prevents a nested mapping row from leaking into a sequence table edge.
        let mut child_in_parent = id;
        let mut current = store.get(id).and_then(|node| node.parent);
        while let Some(parent_id) = current {
            if let Some(handle) = self.handle_for(parent_id) {
                let row = self.parent_row_index_cached(store, parent_id, child_in_parent);
                return Some((handle, row));
            }
            if matches!(
                self.role_for(store, parent_id, config),
                GraphRole::GraphNode
            ) {
                if let Some(handle) = self.reconcile_one_with_path(
                    store,
                    parent_id,
                    visited,
                    config,
                    None,
                    ReconcileIntent::ParentLookup,
                ) {
                    let row = self.parent_row_index_cached(store, parent_id, child_in_parent);
                    return Some((handle, row));
                }
                // A graph-node ancestor is the canonical owner of this edge. Do not
                // fall through to an older visible ancestor while that owner is pending.
                return None;
            }
            if store.get(parent_id).is_some_and(value_node_builds_child) {
                // A structural container owns all descendant graph edges. Until its
                // presentation materializes a graph node, its descendants stay deferred.
                return None;
            }
            if Some(parent_id) == self.root {
                break;
            }
            child_in_parent = parent_id;
            current = store.get(parent_id).and_then(|node| node.parent);
        }
        None
    }

    fn path_from_root(&mut self, store: &TreeStore, id: NodeId) -> Vec<PathSeg> {
        if let Some(path) = self.pass_cache.paths.get(&id) {
            return path.clone();
        }
        #[cfg(test)]
        {
            self.metrics.path_rebuilds += 1;
        }
        let mut path = if Some(id) == self.root {
            Vec::new()
        } else {
            store
                .get(id)
                .and_then(|node| node.parent)
                .map(|parent_id| self.path_from_root(store, parent_id))
                .unwrap_or_default()
        };
        if Some(id) != self.root
            && let Some(segment) = path_segment_for_node(store, id)
        {
            path.push(segment);
        }
        self.pass_cache.paths.insert(id, path.clone());
        path
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
                        let row_index = self.sequence_child_row_index(store, parent_id, id, None);
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

    fn parent_row_index_cached(
        &mut self,
        store: &TreeStore,
        parent_id: NodeId,
        child: NodeId,
    ) -> i32 {
        let Some(parent) = store.get(parent_id) else {
            return 0;
        };
        match parent.kind {
            TreeNodeKind::Sequence => {
                self.sequence_child_row_index(store, parent_id, child, None) as i32
            }
            _ => parent_row_index(store, parent_id, child),
        }
    }

    fn sequence_child_row_index(
        &mut self,
        store: &TreeStore,
        parent_id: NodeId,
        child: NodeId,
        index_hint: Option<usize>,
    ) -> usize {
        if let Some(index) = index_hint.or_else(|| sequence_row_index(store, child)) {
            self.pass_cache.row_index_by_node.set(child, index);
            return index;
        }
        if let Some(index) = self.pass_cache.row_index_by_node.get(child) {
            return index;
        }
        let Some(parent) = store.get(parent_id) else {
            return 0;
        };
        for (index, &candidate) in parent.content.iter().enumerate() {
            self.pass_cache.row_index_by_node.set(candidate, index);
        }
        self.pass_cache
            .row_index_by_node
            .get(child)
            .unwrap_or(parent.content.len())
    }

    fn table_children_can_expand_timed(
        &mut self,
        store: &TreeStore,
        sequence_id: NodeId,
        config: &BuilderConfig,
    ) -> bool {
        if let Some(result) = self
            .pass_cache
            .table_expandability
            .get(&sequence_id)
            .copied()
        {
            return result;
        }
        let result = store
            .get(sequence_id)
            .zip(self.sequence_state(store, sequence_id))
            .is_some_and(|(node, state)| table_children_can_expand(node, &state, config));
        self.pass_cache
            .table_expandability
            .insert(sequence_id, result);
        #[cfg(test)]
        {
            self.metrics.table_expand_misses += 1;
        }
        result
    }
}

fn insert_child_edge(children: &mut Vec<GraphChildEdge>, edge: GraphChildEdge) -> bool {
    match children.binary_search_by_key(&(edge.from_row, edge.to_row, edge.child), |candidate| {
        (candidate.from_row, candidate.to_row, candidate.child)
    }) {
        Ok(_) => false,
        Err(index) => {
            children.insert(index, edge);
            true
        }
    }
}

fn planner_role_for(
    store: &TreeStore,
    root: Option<NodeId>,
    id: NodeId,
    config: &BuilderConfig,
    sequence_state_lookup: &mut SequenceStateLookup<'_>,
) -> GraphRole {
    let Some(node) = store.get(id) else {
        return GraphRole::Pending;
    };
    if Some(id) == root {
        return if node.kind == TreeNodeKind::Sequence
            && matches!(
                sequence_state_lookup(id).map(|state| state.presentation),
                Some(
                    SequencePresentationState::EmptyOpen
                        | SequencePresentationState::PendingHeaderSchema
                )
            ) {
            GraphRole::Pending
        } else {
            GraphRole::GraphNode
        };
    }

    if !config.expand_header_table_rows
        && let Some((table_node_id, row_node_id)) =
            folded_table_row_context_with_sequence_lookup(store, id, root, sequence_state_lookup)
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

    match node.parent {
        None => GraphRole::GraphNode,
        Some(parent_id) => match store.get(parent_id) {
            None => GraphRole::Pending,
            Some(parent) => match parent.kind {
                TreeNodeKind::Mapping => {
                    if node.is_map_key {
                        GraphRole::InlineValue
                    } else if value_node_builds_child(node) {
                        GraphRole::GraphNode
                    } else {
                        GraphRole::InlineValue
                    }
                }
                TreeNodeKind::Sequence => match sequence_state_lookup(parent_id) {
                    Some(SequenceState {
                        presentation:
                            SequencePresentationState::EmptyOpen
                            | SequencePresentationState::PendingHeaderSchema,
                        ..
                    }) => GraphRole::Pending,
                    Some(
                        state @ SequenceState {
                            presentation: SequencePresentationState::HeaderlessTable,
                            ..
                        },
                    ) => {
                        if table_children_can_expand(parent, &state, config)
                            && value_node_builds_child(node)
                        {
                            GraphRole::GraphNode
                        } else {
                            GraphRole::InlineValue
                        }
                    }
                    Some(
                        state @ SequenceState {
                            presentation: SequencePresentationState::HeaderTable,
                            ..
                        },
                    ) => {
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
                        } else if table_children_can_expand(parent, &state, config)
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
            },
        },
    }
}

fn sequence_state_transition_requires_structural_reconcile(
    previous: Option<&SequenceState>,
    current: Option<&SequenceState>,
    previous_expandable: bool,
    current_expandable: bool,
) -> bool {
    match (previous, current) {
        (Some(previous), Some(current)) => {
            previous.presentation != current.presentation
                || previous.first_child_kind != current.first_child_kind
                || previous.header_key_count != current.header_key_count
                || previous.closed != current.closed
                || previous_expandable != current_expandable
        }
        (None, Some(current)) => {
            !matches!(
                current.presentation,
                SequencePresentationState::EmptyOpen
                    | SequencePresentationState::PendingHeaderSchema
            ) || current_expandable
        }
        (Some(_), None) => true,
        (None, None) => false,
    }
}

fn role_transition_requires_structural_reconcile(previous: GraphRole, current: GraphRole) -> bool {
    previous != current
}

fn folded_table_row_context_with_sequence_lookup(
    store: &TreeStore,
    mut id: NodeId,
    boundary_root: Option<NodeId>,
    sequence_state_lookup: &mut SequenceStateLookup<'_>,
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
            && matches!(
                sequence_state_lookup(parent_id).map(|state| state.presentation),
                Some(SequencePresentationState::HeaderTable)
            )
        {
            return Some((parent_id, id));
        }
        id = parent_id;
    }
}

fn structural_escalation_target_for_role(
    store: &TreeStore,
    node_id: NodeId,
    role: GraphRole,
) -> NodeId {
    match role {
        GraphRole::FoldedTableRow { table_node_id, .. } => table_node_id,
        _ => store
            .get(node_id)
            .and_then(|node| node.parent)
            .filter(|parent_id| {
                store
                    .get(*parent_id)
                    .is_some_and(|parent| parent.kind == TreeNodeKind::Sequence)
            })
            .unwrap_or(node_id),
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
    let first_mapping_has_object_or_array = matches!(first_child_kind, Some(TreeNodeKind::Mapping))
        && first_child
            .and_then(|child| store.get(child))
            .is_some_and(|mapping| mapping_contains_object_or_array(store, mapping));
    let header_key_count =
        if matches!(first_child_kind, Some(TreeNodeKind::Mapping)) && !node.sequence_closed() {
            header_key_count(store, id)
        } else {
            0
        };
    let presentation = match (
        first_child_kind,
        node.sequence_closed(),
        header_key_count,
        first_mapping_has_object_or_array,
    ) {
        (None, true, _, _) => SequencePresentationState::EmptyClosed,
        (None, false, _, _) => SequencePresentationState::EmptyOpen,
        (Some(TreeNodeKind::Mapping), _, _, true) => SequencePresentationState::HeaderlessTable,
        (Some(TreeNodeKind::Mapping), true, _, false) => SequencePresentationState::HeaderTable,
        (Some(TreeNodeKind::Mapping), false, 0, false) => {
            SequencePresentationState::PendingHeaderSchema
        }
        (Some(TreeNodeKind::Mapping), false, _, false) => SequencePresentationState::HeaderTable,
        (Some(_), _, _, _) => SequencePresentationState::HeaderlessTable,
    };

    Some(SequenceState {
        presentation,
        first_child,
        first_child_kind,
        header_key_count,
        closed: node.sequence_closed(),
    })
}

pub(crate) fn is_header_table_sequence(store: &TreeStore, id: NodeId) -> bool {
    matches!(
        sequence_presentation_state(store, id),
        Some(SequencePresentationState::HeaderTable)
    )
}

fn table_children_can_expand(
    node: &crate::tree::tree_node::TreeNode,
    state: &SequenceState,
    config: &BuilderConfig,
) -> bool {
    if node.kind != TreeNodeKind::Sequence || !state.closed {
        return false;
    }
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

fn mapping_contains_object_or_array(
    store: &TreeStore,
    mapping: &crate::tree::tree_node::TreeNode,
) -> bool {
    debug_assert_eq!(mapping.kind, TreeNodeKind::Mapping);
    mapping
        .content
        .iter()
        .skip(1)
        .step_by(2)
        .filter_map(|value_id| store.get(*value_id))
        .any(|value| matches!(value.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence))
}

fn value_node_builds_child(node: &crate::tree::tree_node::TreeNode) -> bool {
    matches!(node.kind, TreeNodeKind::Mapping | TreeNodeKind::Sequence) && !node.content.is_empty()
}

fn sequence_row_index(store: &TreeStore, child: NodeId) -> Option<usize> {
    store
        .get(child)?
        .sequence_index()
        .and_then(|value| usize::try_from(value).ok())
}

fn row_index(parent: &crate::tree::tree_node::TreeNode, child: NodeId) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{Decode, JsonDecoder};
    use crate::stream::tree_patch::TreePatch;

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

    fn node_inserted_patch(store: &TreeStore, node_id: NodeId) -> TreePatch {
        let node = store.get(node_id).expect("node should exist");
        TreePatch::NodeInserted {
            node_id,
            parent: node.parent,
            key: node.key(),
            sequence_index: node
                .sequence_index()
                .and_then(|index| u32::try_from(index).ok()),
            kind: node.kind as i32,
            sem_type: node.sem_type.map(|sem_type| sem_type as i32).unwrap_or(-1),
            tag: node.tag.to_string_value(),
            value: store.value_string_for(node_id).unwrap_or_default(),
        }
    }

    #[test]
    fn build_full_keeps_deep_paths_correct_without_recursive_path_rebuilds() {
        let source = nested_json_object(64);
        let (store, root) = store_from_json(&source);
        let mut topology = GraphTopology::new();
        let cfg = crate::graph::graph_builder::default_config();

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
        // Parent-ownership reconciliation may resolve a deep graph parent lazily;
        // those lookups intentionally rebuild the cached path once per ancestor.
        assert_eq!(metrics.path_rebuilds, 65);
        assert!(
            metrics.path_segment_pushes <= store.len(),
            "full build should push each TreeStore path segment at most once per DFS visit"
        );
    }

    #[test]
    fn graph_topology_apply_folds_header_table_rows_and_tracks_dirty_handles() {
        let (store, root) = store_from_json(r#"{"rows":[{"a":1,"b":2},{"a":3}]}"#);
        let mut topology = GraphTopology::new();

        let cfg = crate::graph::graph_builder::default_config();
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
        assert!(topology.handle_for(first_row).is_none());
        assert!(topology.folded_row(first_row).is_some());
    }

    #[test]
    fn sequence_with_object_or_array_in_first_mapping_row_is_headerless() {
        for source in [
            r#"{"rows":[{"nested":{}},{"nested":{}}]}"#,
            r#"{"rows":[{"nested":[]},{"nested":[]}]}"#,
        ] {
            let (store, root) = store_from_json(source);
            let rows = store
                .get(root)
                .and_then(|node| node.content.get(1))
                .copied()
                .expect("rows sequence exists");

            assert_eq!(
                sequence_presentation_state(&store, rows),
                Some(SequencePresentationState::HeaderlessTable)
            );
        }
    }

    #[test]
    fn small_headerless_table_expands_structured_rows() {
        let source =
            r#"{"rows":[{"name":"a","meta":{"x":1}},{"name":"b","meta":{"x":2}}]}"#;
        let (store, root) = store_from_json(source);
        let mut topology = GraphTopology::new();
        let cfg = crate::graph::graph_builder::default_config();

        topology.build_full(&store, root, &cfg);

        let table_handle = topology
            .slots()
            .iter()
            .position(|slot| slot.path == [PathSeg::Key("rows".to_owned())])
            .expect("rows should be a graph table") as GraphHandle;
        let children = &topology.slots()[table_handle as usize].children;

        assert_eq!(children.len(), 2);
        assert_eq!(
            topology.slots()[children[0].child as usize].path,
            [PathSeg::Key("rows".to_owned()), PathSeg::Index(0)]
        );
        assert_eq!(
            topology.slots()[children[1].child as usize].path,
            [PathSeg::Key("rows".to_owned()), PathSeg::Index(1)]
        );
    }

    #[test]
    fn insert_child_edge_keeps_edges_sorted_and_unique() {
        let mut children = vec![
            GraphChildEdge {
                child: 4,
                from_row: 4,
                to_row: 0,
            },
            GraphChildEdge {
                child: 8,
                from_row: 8,
                to_row: 0,
            },
        ];

        assert!(insert_child_edge(
            &mut children,
            GraphChildEdge {
                child: 6,
                from_row: 6,
                to_row: 0,
            }
        ));
        assert!(!insert_child_edge(
            &mut children,
            GraphChildEdge {
                child: 6,
                from_row: 6,
                to_row: 0,
            }
        ));
        assert!(insert_child_edge(
            &mut children,
            GraphChildEdge {
                child: 2,
                from_row: 2,
                to_row: 0,
            }
        ));

        assert_eq!(
            children,
            vec![
                GraphChildEdge {
                    child: 2,
                    from_row: 2,
                    to_row: 0,
                },
                GraphChildEdge {
                    child: 4,
                    from_row: 4,
                    to_row: 0,
                },
                GraphChildEdge {
                    child: 6,
                    from_row: 6,
                    to_row: 0,
                },
                GraphChildEdge {
                    child: 8,
                    from_row: 8,
                    to_row: 0,
                },
            ]
        );
    }

    #[test]
    fn complex_fixture_keeps_large_headerless_table_rows_folded() {
        let source = include_str!("../../../../test/fixtures/json/complex.1.json");
        let (store, root) = store_from_json(source);
        let mut topology = GraphTopology::new();
        let cfg = crate::graph::graph_builder::default_config();
        topology.build_full(&store, root, &cfg);

        let key = "we___are___such___stuff___as___dreams___are___made___on___and___our___little___life___is___rounded___with___sleep";
        let table_handle = topology
            .slots()
            .iter()
            .position(|slot| slot.path == [PathSeg::Key(key.to_owned())])
            .expect("complex sequence table should exist")
            as GraphHandle;
        let children = &topology.slots()[table_handle as usize].children;

        assert!(children.is_empty());
    }

    #[test]
    fn graph_topology_apply_caches_folded_context_within_pass() {
        let (store, root) = store_from_json(r#"{"rows":[{"value":1},{"value":2}]}"#);
        let mut topology = GraphTopology::new();
        let cfg = crate::graph::graph_builder::default_config();
        topology.build_full(&store, root, &cfg);

        let rows = store
            .get(root)
            .and_then(|node| node.content.get(1))
            .copied()
            .expect("rows sequence exists");
        let first_row = store
            .get(rows)
            .and_then(|node| node.content.first())
            .copied()
            .expect("first row exists");
        let row_value = store
            .get(first_row)
            .and_then(|row| row.content.get(1))
            .copied()
            .expect("row value exists");

        let _ = topology.apply(
            &store,
            root,
            &[
                crate::stream::tree_patch::TreePatch::NodeSealed { node_id: row_value },
                crate::stream::tree_patch::TreePatch::NodeSealed { node_id: row_value },
            ],
            &cfg,
        );

        let metrics = topology.metrics();
        assert_eq!(metrics.folded_context_misses, 2);
    }

    #[test]
    fn graph_patch_planner_coalesces_insert_and_seal_into_one_table_row() {
        let (store, root) = store_from_json(r#"{"rows":[{"a":1,"b":2}]}"#);
        let mut topology = GraphTopology::new();
        let cfg = crate::graph::graph_builder::default_config();
        topology.build_full(&store, root, &cfg);

        let rows = store
            .get(root)
            .and_then(|node| node.content.get(1))
            .copied()
            .expect("rows sequence exists");
        let first_row = store
            .get(rows)
            .and_then(|node| node.content.first())
            .copied()
            .expect("first row exists");
        let value_a = store
            .get(first_row)
            .and_then(|row| row.content.get(1))
            .copied()
            .expect("a value exists");
        let value_b = store
            .get(first_row)
            .and_then(|row| row.content.get(3))
            .copied()
            .expect("b value exists");

        let dirty = topology.apply(
            &store,
            root,
            &[
                node_inserted_patch(&store, value_a),
                TreePatch::NodeSealed { node_id: value_a },
                node_inserted_patch(&store, value_b),
                TreePatch::NodeSealed { node_id: value_b },
            ],
            &cfg,
        );

        assert_eq!(dirty.table_rows().len(), 1);
        assert_eq!(dirty.table_rows()[0].row_node_id, first_row);
    }

    #[test]
    fn planner_role_for_explicitly_captures_pending_header_table_publish_transition() {
        let (store, root) = store_from_json(r#"{"rows":[{"a":1}]}"#);
        let cfg = crate::graph::graph_builder::default_config();
        let rows = store
            .get(root)
            .and_then(|node| node.content.get(1))
            .copied()
            .expect("rows sequence exists");
        let row = store
            .get(rows)
            .and_then(|node| node.content.first())
            .copied()
            .expect("row exists");

        let current_state =
            sequence_state_from_store(&store, rows).expect("current sequence state");
        let mut previous_lookup = |sequence_id: NodeId| {
            if sequence_id == rows {
                Some(SequenceState {
                    presentation: SequencePresentationState::PendingHeaderSchema,
                    first_child: current_state.first_child,
                    first_child_kind: current_state.first_child_kind,
                    header_key_count: 0,
                    closed: false,
                })
            } else {
                sequence_state_from_store(&store, sequence_id)
            }
        };
        let mut current_lookup =
            |sequence_id: NodeId| sequence_state_from_store(&store, sequence_id);

        let previous_role = planner_role_for(&store, Some(root), row, &cfg, &mut previous_lookup);
        let current_role = planner_role_for(&store, Some(root), row, &cfg, &mut current_lookup);

        assert_eq!(previous_role, GraphRole::Pending);
        assert!(matches!(
            current_role,
            GraphRole::FoldedTableRow {
                table_node_id,
                row_node_id,
                ..
            } if table_node_id == rows && row_node_id == row
        ));
        assert!(role_transition_requires_structural_reconcile(
            previous_role,
            current_role
        ));
        assert_eq!(
            structural_escalation_target_for_role(&store, row, current_role),
            rows
        );
    }

    #[test]
    fn planner_role_for_explicitly_captures_headerless_expand_transition() {
        let (store, root) = store_from_json(r#"[[1],[2],[3]]"#);
        let cfg = crate::graph::graph_builder::default_config();
        let row = store
            .get(root)
            .and_then(|node| node.content.first())
            .copied()
            .expect("first row exists");

        let current_state =
            sequence_state_from_store(&store, root).expect("current sequence state");
        let mut previous_lookup = |sequence_id: NodeId| {
            if sequence_id == root {
                Some(SequenceState {
                    closed: false,
                    ..current_state.clone()
                })
            } else {
                sequence_state_from_store(&store, sequence_id)
            }
        };
        let mut current_lookup =
            |sequence_id: NodeId| sequence_state_from_store(&store, sequence_id);

        let previous_role = planner_role_for(&store, Some(root), row, &cfg, &mut previous_lookup);
        let current_role = planner_role_for(&store, Some(root), row, &cfg, &mut current_lookup);

        assert_eq!(previous_role, GraphRole::InlineValue);
        assert_eq!(current_role, GraphRole::GraphNode);
        assert!(role_transition_requires_structural_reconcile(
            previous_role,
            current_role
        ));
        assert_eq!(
            structural_escalation_target_for_role(&store, row, current_role),
            root
        );
    }

    #[test]
    fn parent_lookup_intent_does_not_mark_existing_graph_node_dirty() {
        let (store, root) = store_from_json(r#"{"a":{"b":1}}"#);
        let mut topology = GraphTopology::new();
        let cfg = crate::graph::graph_builder::default_config();
        topology.build_full(&store, root, &cfg);
        topology.dirty.clear();

        let mut visited = HashSet::new();
        let handle = topology.reconcile_one_with_path(
            &store,
            root,
            &mut visited,
            &cfg,
            None,
            ReconcileIntent::ParentLookup,
        );

        assert_eq!(handle, topology.handle_for(root));
        assert!(topology.dirty.is_empty());
    }

    #[test]
    fn graph_topology_apply_scans_expandable_sequences_once_per_pass() {
        let (store, root) = store_from_json(r#"[[1],[2],[3]]"#);
        let mut topology = GraphTopology::new();
        let cfg = crate::graph::graph_builder::default_config();
        topology.build_full(&store, root, &cfg);

        let patches = store
            .get(root)
            .map(|node| {
                node.content
                    .iter()
                    .copied()
                    .map(|node_id| crate::stream::tree_patch::TreePatch::NodeSealed { node_id })
                    .collect::<Vec<_>>()
            })
            .expect("root sequence exists");

        let _ = topology.apply(&store, root, &patches, &cfg);

        let metrics = topology.metrics();
        assert_eq!(metrics.table_expand_misses, 4);
        assert_eq!(metrics.expandable_sequence_scans, 4);
    }
}
