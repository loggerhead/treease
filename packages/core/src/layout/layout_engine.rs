use std::collections::{BTreeMap, HashSet};

use crate::graph::graph_builder::{
    BoxArgs, BuilderConfig, CellBounds, GraphBuilder, GraphCell, GraphKind, GraphLanguage,
    GraphModel, GraphNode,
};
use crate::graph::graph_topology::{DirtyEdge, GraphHandle, GraphTopology};

const CHECKPOINT_INTERVAL: usize = 64;

#[derive(Debug, Clone)]
pub struct LayoutEngine {
    config: BuilderConfig,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayoutMetrics {
    pub y_events_replayed: usize,
    pub x_width_touches: usize,
    pub edge_indexes_refreshed: usize,
    pub children_index_sorts: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct LayoutChangeSet {
    node_handles: Vec<u32>,
    edge_indexes: Vec<usize>,
}

impl LayoutChangeSet {
    pub(crate) fn node_handles(&self) -> &[u32] {
        &self.node_handles
    }

    pub(crate) fn edge_indexes(&self) -> &[usize] {
        &self.edge_indexes
    }

    fn mark(&mut self, handle: u32) {
        self.node_handles.push(handle);
    }

    fn finish(&mut self) {
        self.node_handles.sort_unstable();
        self.node_handles.dedup();
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct LayoutState {
    depth_widths: Vec<DepthWidthStats>,
    col_x: Vec<i32>,
    handles_by_depth: Vec<Vec<GraphHandle>>,
    active_handles: Vec<bool>,
    positions: Vec<NodePosition>,
    events: Vec<LayoutEvent>,
    enter_index: Vec<Option<usize>>,
    exit_index: Vec<Option<usize>>,
    checkpoints: Vec<LayoutCheckpoint>,
    initialized: bool,
    metrics: LayoutMetrics,
}

impl LayoutState {
    #[cfg(test)]
    pub(crate) fn metrics(&self) -> LayoutMetrics {
        self.metrics
    }

    fn reset(&mut self) {
        self.depth_widths.clear();
        self.col_x.clear();
        self.handles_by_depth.clear();
        self.active_handles.clear();
        self.positions.clear();
        self.events.clear();
        self.enter_index.clear();
        self.exit_index.clear();
        self.checkpoints.clear();
        self.initialized = false;
        self.metrics = LayoutMetrics::default();
    }

    fn begin_incremental_pass(&mut self) {
        self.metrics = LayoutMetrics::default();
    }

    fn ensure_node_capacity(&mut self, len: usize) {
        self.active_handles.resize(len, false);
        self.positions.resize(len, NodePosition::default());
        self.enter_index.resize(len, None);
        self.exit_index.resize(len, None);
    }

    fn ensure_depth_capacity(&mut self, depth: usize) {
        if self.depth_widths.len() <= depth {
            self.depth_widths
                .resize_with(depth + 1, DepthWidthStats::default);
        }
        if self.col_x.len() <= depth {
            self.col_x.resize(depth + 1, 0);
        }
        if self.handles_by_depth.len() <= depth {
            self.handles_by_depth.resize_with(depth + 1, Vec::new);
        }
    }

    fn register_active_handle(&mut self, handle: GraphHandle, depth: usize) {
        self.ensure_depth_capacity(depth);
        let index = handle as usize;
        if index >= self.active_handles.len() || self.active_handles[index] {
            return;
        }
        self.active_handles[index] = true;
        self.handles_by_depth[depth].push(handle);
    }

    fn rebuild_indexes_from(&mut self, start: usize) {
        for event_index in start..self.events.len() {
            match self.events[event_index] {
                LayoutEvent::Enter(handle) => {
                    if let Some(slot) = self.enter_index.get_mut(handle as usize) {
                        *slot = Some(event_index);
                    }
                }
                LayoutEvent::Exit(handle) => {
                    if let Some(slot) = self.exit_index.get_mut(handle as usize) {
                        *slot = Some(event_index);
                    }
                }
            }
        }
    }

    fn update_dimensions_and_x(
        &mut self,
        model: &mut GraphModel,
        root_render_handle: GraphHandle,
        seed_handles: &[GraphHandle],
        changed: &mut LayoutChangeSet,
        h_gap: i32,
    ) -> Option<usize> {
        self.ensure_node_capacity(model.nodes.len());
        let mut min_recompute_from = None::<usize>;
        let mut earliest_height_event = None::<usize>;

        for &handle in seed_handles {
            let index = handle as usize;
            let Some(node) = model.nodes.get(index) else {
                continue;
            };
            self.metrics.x_width_touches += 1;
            let depth = node.depth as usize;
            let old_col_len = self.col_x.len();
            self.ensure_depth_capacity(depth);
            if depth >= old_col_len {
                let start = old_col_len.max(1);
                min_recompute_from = Some(min_recompute_from.map_or(start, |min| min.min(start)));
            }
            let is_new = !self.active_handles.get(index).copied().unwrap_or(false);
            self.register_active_handle(handle, depth);

            if is_new {
                if self.depth_widths[depth].insert(node.width) {
                    min_recompute_from =
                        Some(min_recompute_from.map_or(depth + 1, |min| min.min(depth + 1)));
                }
                self.positions[index].width = node.width;
                self.positions[index].height = node.height;
                changed.mark(handle);
                continue;
            }

            let previous = self.positions[index];
            if previous.width != node.width {
                let removed_changed = self.depth_widths[depth].remove(previous.width);
                let inserted_changed = self.depth_widths[depth].insert(node.width);
                if removed_changed || inserted_changed {
                    min_recompute_from =
                        Some(min_recompute_from.map_or(depth + 1, |min| min.min(depth + 1)));
                }
                self.positions[index].width = node.width;
                changed.mark(handle);
            }
            if previous.height != node.height {
                self.positions[index].height = node.height;
                changed.mark(handle);
                if handle != root_render_handle {
                    if let Some(Some(enter)) = self.enter_index.get(index) {
                        earliest_height_event =
                            Some(earliest_height_event.map_or(*enter, |min| min.min(*enter)));
                    }
                }
            }
        }

        if self.col_x.is_empty() && !self.depth_widths.is_empty() {
            self.col_x.resize(self.depth_widths.len(), 0);
            min_recompute_from = Some(min_recompute_from.unwrap_or(1));
        }

        if let Some(start_depth) = min_recompute_from {
            let max_depth = self.depth_widths.len().saturating_sub(1);
            if self.col_x.len() <= max_depth {
                self.col_x.resize(max_depth + 1, 0);
            }
            for depth in start_depth..=max_depth {
                let previous_x = self.col_x[depth];
                self.col_x[depth] =
                    self.col_x[depth - 1] + self.depth_widths[depth - 1].max_width + h_gap;
                if previous_x != self.col_x[depth] {
                    for &handle in &self.handles_by_depth[depth] {
                        let index = handle as usize;
                        if let Some(node) = model.nodes.get_mut(index) {
                            node.x = self.col_x[depth];
                        }
                        if let Some(position) = self.positions.get_mut(index) {
                            position.x = self.col_x[depth];
                        }
                        changed.mark(handle);
                    }
                }
            }
        }

        for &handle in seed_handles {
            let index = handle as usize;
            let Some(node) = model.nodes.get_mut(index) else {
                continue;
            };
            let depth = node.depth as usize;
            self.ensure_depth_capacity(depth);
            let x = self.col_x[depth];
            if node.x != x || self.positions[index].x != x {
                node.x = x;
                self.positions[index].x = x;
                changed.mark(handle);
            }
        }

        earliest_height_event
    }

    fn insert_events_for_added_edges(
        &mut self,
        topology: &GraphTopology,
        added_edges: &[DirtyEdge],
    ) -> Option<usize> {
        if added_edges.is_empty() {
            return None;
        }

        let added_children: HashSet<GraphHandle> = added_edges.iter().map(|edge| edge.to).collect();
        let mut roots: Vec<(GraphHandle, GraphHandle, i32)> = added_edges
            .iter()
            .filter(|edge| !added_children.contains(&edge.from))
            .map(|edge| (edge.from, edge.to, edge.from_row))
            .collect();
        roots.sort_by_key(|(parent, _child, row)| (*parent, *row));

        let mut earliest = None::<usize>;
        for (parent, child, _) in roots {
            if self
                .enter_index
                .get(child as usize)
                .and_then(|entry| *entry)
                .is_some()
            {
                continue;
            }
            let Some(insert_at) = self.insertion_index_for_child(topology, parent, child) else {
                continue;
            };
            let mut subtree = Vec::new();
            push_subtree_events(topology, child, &mut subtree);
            if subtree.is_empty() {
                continue;
            }
            self.events.splice(insert_at..insert_at, subtree);
            self.rebuild_indexes_from(insert_at);
            earliest = Some(earliest.map_or(insert_at, |min| min.min(insert_at)));
        }
        earliest
    }

    fn insertion_index_for_child(
        &self,
        topology: &GraphTopology,
        parent: GraphHandle,
        child: GraphHandle,
    ) -> Option<usize> {
        let children = topology.child_edges(parent);
        let child_position = children.iter().position(|edge| edge.child == child)?;
        for sibling in children.iter().skip(child_position + 1) {
            if let Some(Some(index)) = self.enter_index.get(sibling.child as usize) {
                return Some(*index);
            }
        }
        self.exit_index
            .get(parent as usize)
            .and_then(|entry| *entry)
    }

    fn replay_y_from(
        &mut self,
        model: &mut GraphModel,
        start_offset: usize,
        changed: &mut LayoutChangeSet,
        v_gap: i32,
    ) {
        let checkpoint = self.checkpoint_before(start_offset);
        self.checkpoints
            .retain(|candidate| candidate.event_offset < checkpoint.event_offset);
        let mut level_meta = checkpoint.level_meta;
        let mut open_stack = checkpoint.open_stack;

        for event_offset in checkpoint.event_offset..self.events.len() {
            if event_offset % CHECKPOINT_INTERVAL == 0 {
                self.checkpoints.push(LayoutCheckpoint {
                    event_offset,
                    level_meta: level_meta.clone(),
                    open_stack: open_stack.clone(),
                });
            }
            self.metrics.y_events_replayed += 1;
            match self.events[event_offset] {
                LayoutEvent::Enter(handle) => {
                    let index = handle as usize;
                    let Some(node) = model.nodes.get_mut(index) else {
                        continue;
                    };
                    let level = node.depth as usize;
                    while level >= level_meta.len() {
                        level_meta.push(LevelMeta::default());
                    }
                    let parent_y = open_stack
                        .last()
                        .and_then(|open| self.positions.get(open.handle as usize))
                        .map(|position| position.y)
                        .unwrap_or(0);
                    let y = if !level_meta[level].seen {
                        parent_y
                    } else {
                        parent_y.max(level_meta[level].bottom + v_gap)
                    };
                    level_meta[level].seen = true;
                    if node.y != y || self.positions[index].y != y {
                        node.y = y;
                        self.positions[index].y = y;
                        changed.mark(handle);
                    }
                    let height = self.positions[index].height;
                    open_stack.push(OpenSubtree {
                        handle,
                        current_bottom: y + height,
                    });
                }
                LayoutEvent::Exit(handle) => {
                    let Some(open) = open_stack.pop() else {
                        continue;
                    };
                    debug_assert_eq!(open.handle, handle);
                    let depth = model
                        .nodes
                        .get(handle as usize)
                        .map(|node| node.depth as usize)
                        .unwrap_or(0);
                    while depth >= level_meta.len() {
                        level_meta.push(LevelMeta::default());
                    }
                    level_meta[depth].bottom = open.current_bottom;
                    if let Some(parent) = open_stack.last_mut() {
                        parent.current_bottom = parent.current_bottom.max(open.current_bottom);
                    }
                }
            }
        }
    }

    fn checkpoint_before(&self, offset: usize) -> LayoutCheckpoint {
        self.checkpoints
            .iter()
            .rev()
            .find(|checkpoint| checkpoint.event_offset <= offset)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Debug, Default, Clone)]
struct DepthWidthStats {
    counts: BTreeMap<i32, usize>,
    max_width: i32,
}

impl DepthWidthStats {
    fn insert(&mut self, width: i32) -> bool {
        let old = self.max_width;
        *self.counts.entry(width).or_insert(0) += 1;
        self.max_width = self.counts.keys().next_back().copied().unwrap_or(0);
        old != self.max_width
    }

    fn remove(&mut self, width: i32) -> bool {
        let old = self.max_width;
        if let Some(count) = self.counts.get_mut(&width) {
            *count -= 1;
            if *count == 0 {
                self.counts.remove(&width);
            }
        }
        self.max_width = self.counts.keys().next_back().copied().unwrap_or(0);
        old != self.max_width
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct NodePosition {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutEvent {
    Enter(GraphHandle),
    Exit(GraphHandle),
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct LevelMeta {
    bottom: i32,
    seen: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct OpenSubtree {
    handle: GraphHandle,
    current_bottom: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LayoutCheckpoint {
    event_offset: usize,
    level_meta: Vec<LevelMeta>,
    open_stack: Vec<OpenSubtree>,
}

#[derive(Debug)]
struct ChildrenIndex {
    offsets: Vec<usize>,
    children: Vec<u32>,
}

impl LayoutEngine {
    pub fn new(config: BuilderConfig) -> Self {
        Self { config }
    }

    pub fn layout_full(&self, model: &mut GraphModel, root_render_handle: u32) {
        self.layout_positions(model, root_render_handle);
        self.refresh_bounds_and_edges(model);
    }

    pub(crate) fn layout_full_with_topology(
        &self,
        state: &mut LayoutState,
        topology: &GraphTopology,
        model: &mut GraphModel,
        root_render_handle: u32,
    ) -> LayoutChangeSet {
        state.reset();
        if model.nodes.is_empty() || root_render_handle as usize >= model.nodes.len() {
            return LayoutChangeSet::default();
        }
        state.ensure_node_capacity(model.nodes.len());
        push_subtree_events(topology, root_render_handle, &mut state.events);
        state.rebuild_indexes_from(0);

        let mut changed = LayoutChangeSet::default();
        for node in &model.nodes {
            let handle = node.render_handle;
            let depth = node.depth as usize;
            state.ensure_depth_capacity(depth);
            state.register_active_handle(handle, depth);
            state.depth_widths[depth].insert(node.width);
            state.positions[handle as usize] = NodePosition {
                x: 0,
                y: node.y,
                width: node.width,
                height: node.height,
            };
            changed.mark(handle);
        }
        state.col_x.resize(state.depth_widths.len(), 0);
        for depth in 1..state.depth_widths.len() {
            state.col_x[depth] = state.col_x[depth - 1]
                + state.depth_widths[depth - 1].max_width
                + self.config.h_gap;
        }
        for node in &mut model.nodes {
            let x = state.col_x[node.depth as usize];
            node.x = x;
            state.positions[node.render_handle as usize].x = x;
        }
        state.replay_y_from(model, 0, &mut changed, self.config.v_gap);
        changed.finish();
        self.refresh_bounds_and_edges(model);
        changed.edge_indexes = (0..model.edges.len()).collect();
        state.metrics.edge_indexes_refreshed = changed.edge_indexes.len();
        state.initialized = true;
        changed
    }

    pub(crate) fn layout_changed_region(
        &self,
        state: &mut LayoutState,
        topology: &GraphTopology,
        model: &mut GraphModel,
        root_render_handle: u32,
        seed_handles: &[u32],
        added_edges: &[DirtyEdge],
        added_edge_indexes: &[usize],
    ) -> LayoutChangeSet {
        if model.nodes.is_empty() || root_render_handle as usize >= model.nodes.len() {
            return LayoutChangeSet::default();
        }
        state.begin_incremental_pass();
        if !state.initialized || state.events.is_empty() {
            return self.layout_full_with_topology(state, topology, model, root_render_handle);
        }
        state.ensure_node_capacity(model.nodes.len());

        let inserted_at = state.insert_events_for_added_edges(topology, added_edges);
        let mut changed = LayoutChangeSet::default();
        let height_event = state.update_dimensions_and_x(
            model,
            root_render_handle,
            seed_handles,
            &mut changed,
            self.config.h_gap,
        );
        let replay_from = match (inserted_at, height_event) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(offset), None) | (None, Some(offset)) => Some(offset),
            (None, None) => None,
        };
        if let Some(offset) = replay_from {
            state.replay_y_from(model, offset, &mut changed, self.config.v_gap);
        }
        changed.finish();
        changed.edge_indexes = self.refresh_changed_bounds_and_edges_indexed(
            model,
            changed.node_handles(),
            seed_handles,
            added_edge_indexes,
        );
        state.metrics.edge_indexes_refreshed = changed.edge_indexes.len();
        changed
    }

    pub fn layout_positions(&self, model: &mut GraphModel, root_render_handle: u32) {
        if model.nodes.is_empty() {
            return;
        }
        if root_render_handle as usize >= model.nodes.len() {
            return;
        }

        let children_index = build_children_index(model);
        compute_x(model, &self.config);
        let mut level_meta = Vec::new();
        compute_y(
            model,
            &self.config,
            root_render_handle,
            0,
            &mut level_meta,
            &children_index,
        );
    }

    pub fn refresh_bounds_and_edges(&self, model: &mut GraphModel) {
        let helper = GraphBuilder::new(self.config.clone(), GraphLanguage::None);
        for node in &mut model.nodes {
            helper.apply_node_bounds_to(node);
        }
        let nodes = &model.nodes;
        for edge in &mut model.edges {
            helper.apply_edge_bezier_args_to(nodes, edge);
        }
    }

    fn refresh_changed_bounds_and_edges_indexed(
        &self,
        model: &mut GraphModel,
        bounds_handles: &[u32],
        edge_seed_handles: &[u32],
        extra_edge_indexes: &[usize],
    ) -> Vec<usize> {
        let helper = GraphBuilder::new(self.config.clone(), GraphLanguage::None);
        for &handle in bounds_handles {
            let index = handle as usize;
            if let Some(node) = model.nodes.get_mut(index) {
                if node.kind == GraphKind::Table {
                    apply_table_node_frame_bounds(&self.config, node);
                } else {
                    helper.apply_node_bounds_to(node);
                }
            }
        }

        let mut edge_refresh_handles = bounds_handles.to_vec();
        edge_refresh_handles.extend_from_slice(edge_seed_handles);
        edge_refresh_handles.sort_unstable();
        edge_refresh_handles.dedup();

        let mut refreshed_edges = model.edge_indexes_incident_to_handles(&edge_refresh_handles);
        refreshed_edges.extend_from_slice(extra_edge_indexes);
        refreshed_edges.sort_unstable();
        refreshed_edges.dedup();

        let nodes = &model.nodes;
        for &edge_index in &refreshed_edges {
            if let Some(edge) = model.edges.get_mut(edge_index) {
                helper.apply_edge_bezier_args_to(nodes, edge);
            }
        }
        refreshed_edges
    }
}

fn push_subtree_events(
    topology: &GraphTopology,
    handle: GraphHandle,
    events: &mut Vec<LayoutEvent>,
) {
    if topology.slot(handle).is_none() {
        return;
    }
    events.push(LayoutEvent::Enter(handle));
    for child in topology.child_edges(handle) {
        push_subtree_events(topology, child.child, events);
    }
    events.push(LayoutEvent::Exit(handle));
}

fn apply_table_node_frame_bounds(config: &BuilderConfig, node: &mut GraphNode) {
    let border_width = config.node_border_width.max(0);
    node.box_args = BoxArgs {
        x: node.x,
        y: node.y,
        width: node.width,
        height: node.height,
        corner_radius: 0,
    };

    let meta_height = config.row_height;
    let inner_width = node.table.as_ref().map(|table| table.width).unwrap_or(0);
    set_cell_bounds(
        &mut node.meta,
        node.x + border_width,
        node.y - meta_height,
        inner_width,
        meta_height,
    );
    set_text_bounds(
        &mut node.meta,
        node.x + border_width + config.row_padding_x,
        node.y - meta_height,
        inner_width - config.row_padding_x * 2,
        meta_height,
    );
}

fn set_cell_bounds(cell: &mut GraphCell, x: i32, y: i32, width: i32, height: i32) {
    set_bounds(&mut cell.bounds, x, y, width, height);
    cell.box_args = BoxArgs {
        x: cell.bounds.x,
        y: cell.bounds.y,
        width: cell.bounds.width,
        height: cell.bounds.height,
        corner_radius: 0,
    };
}

fn set_text_bounds(cell: &mut GraphCell, x: i32, y: i32, width: i32, height: i32) {
    set_bounds(&mut cell.text_bounds, x, y, width, height);
}

fn set_bounds(bounds: &mut CellBounds, x: i32, y: i32, width: i32, height: i32) {
    *bounds = CellBounds {
        x,
        y,
        width: width.max(0),
        height: height.max(0),
    };
}

fn build_children_index(model: &GraphModel) -> ChildrenIndex {
    // Legacy path for graph models that do not carry topology adjacency.
    // Primary DocumentJob graph paths use layout_full_with_topology().
    let node_len = model.nodes.len();
    let mut counts = vec![0usize; node_len];
    for edge in &model.edges {
        let index = edge.from_render_handle as usize;
        if index < node_len {
            counts[index] += 1;
        }
    }

    let mut offsets = vec![0usize; node_len + 1];
    for index in 0..node_len {
        offsets[index + 1] = offsets[index] + counts[index];
    }

    let mut children = vec![0u32; offsets[node_len]];
    let mut cursors = vec![0usize; node_len];
    let mut sorted_edges: Vec<_> = model
        .edges
        .iter()
        .filter(|edge| {
            (edge.from_render_handle as usize) < node_len
                && (edge.to_render_handle as usize) < node_len
        })
        .collect();
    sorted_edges.sort_by_key(|edge| {
        (
            edge.from_render_handle,
            edge.from_row,
            edge.to_render_handle,
        )
    });

    for edge in sorted_edges {
        let parent = edge.from_render_handle as usize;
        let index = offsets[parent] + cursors[parent];
        children[index] = edge.to_render_handle;
        cursors[parent] += 1;
    }

    ChildrenIndex { offsets, children }
}

fn compute_x(model: &mut GraphModel, config: &BuilderConfig) {
    let max_depth = model
        .nodes
        .iter()
        .map(|node| node.depth as usize)
        .max()
        .unwrap_or(0);
    let mut max_width_by_depth = vec![0; max_depth + 1];
    for node in &model.nodes {
        let depth = node.depth as usize;
        max_width_by_depth[depth] = max_width_by_depth[depth].max(node.width);
    }

    let mut col_x = vec![0; max_depth + 1];
    for depth in 1..=max_depth {
        col_x[depth] = col_x[depth - 1] + max_width_by_depth[depth - 1] + config.h_gap;
    }
    for node in &mut model.nodes {
        node.x = col_x[node.depth as usize];
    }
}

fn compute_y(
    model: &mut GraphModel,
    config: &BuilderConfig,
    render_handle: u32,
    parent_y: i32,
    level_meta: &mut Vec<LevelMeta>,
    children_index: &ChildrenIndex,
) -> i32 {
    let node_index = render_handle as usize;
    if node_index >= model.nodes.len() {
        return parent_y;
    }

    let level = model.nodes[node_index].depth as usize;
    while level >= level_meta.len() {
        level_meta.push(LevelMeta::default());
    }

    if !level_meta[level].seen {
        model.nodes[node_index].y = parent_y;
    } else {
        model.nodes[node_index].y = parent_y.max(level_meta[level].bottom + config.v_gap);
    }
    level_meta[level].seen = true;

    let child_parent_y = model.nodes[node_index].y;
    let mut subtree_bottom = model.nodes[node_index].y + model.nodes[node_index].height;
    let start = children_index.offsets[node_index];
    let end = children_index.offsets[node_index + 1];
    for child_index in start..end {
        let child_bottom = compute_y(
            model,
            config,
            children_index.children[child_index],
            child_parent_y,
            level_meta,
            children_index,
        );
        subtree_bottom = subtree_bottom.max(child_bottom);
    }

    level_meta[level].bottom = subtree_bottom;
    subtree_bottom
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::{Decode, JsonDecoder};
    use crate::graph::graph_builder::PathSeg;
    use crate::graph::graph_materialize::materialize_into_current_model;
    use crate::graph::graph_projection_service;
    use crate::graph::graph_shape::NodeShapeBuilder;
    use crate::graph::graph_topology::GraphTopology;

    fn model_from_json(source: &str) -> (GraphTopology, GraphModel, u32) {
        let decoded = JsonDecoder
            .decode_str(source)
            .expect("json fixture should decode");
        let cfg = graph_projection_service::projection_builder_config().to_graph_builder_config();
        let mut topology = GraphTopology::new();
        let dirty = topology.build_full(&decoded.store, decoded.root, &cfg);
        let mut model = GraphModel::default();
        let shape_builder = NodeShapeBuilder::new(&cfg, GraphLanguage::Json);
        materialize_into_current_model(
            &mut topology,
            &mut model,
            &decoded.store,
            &dirty,
            &shape_builder,
            &cfg,
        );
        let root = topology.root_handle().expect("root handle should exist");
        (topology, model, root)
    }

    #[test]
    fn full_layout_with_topology_does_not_build_sorted_children_index() {
        let (topology, mut model, root) =
            model_from_json(r#"{"a":{"x":1},"b":{"y":2},"c":{"z":3}}"#);
        let cfg = graph_projection_service::projection_builder_config().to_graph_builder_config();
        let mut state = LayoutState::default();

        let changed = LayoutEngine::new(cfg)
            .layout_full_with_topology(&mut state, &topology, &mut model, root);

        assert!(!changed.node_handles().is_empty());
        assert_eq!(state.metrics().children_index_sorts, 0);
        assert_eq!(state.metrics().edge_indexes_refreshed, model.edges.len());
    }

    #[test]
    fn changed_region_refreshes_edges_for_siblings_repositioned_by_seed_growth() {
        let (topology, mut model, root) =
            model_from_json(r#"{"left":{"v":1},"right":{"v":2},"tail":{"v":3}}"#);
        let cfg = graph_projection_service::projection_builder_config().to_graph_builder_config();
        let engine = LayoutEngine::new(cfg);
        let mut state = LayoutState::default();
        engine.layout_full_with_topology(&mut state, &topology, &mut model, root);

        let right_handle = model
            .nodes
            .iter()
            .find(|node| {
                node.path.len() == 1 && matches!(&node.path[0], PathSeg::Key(key) if key == "right")
            })
            .map(|node| node.render_handle)
            .expect("right node should exist");
        let tail_handle = model
            .nodes
            .iter()
            .find(|node| {
                node.path.len() == 1 && matches!(&node.path[0], PathSeg::Key(key) if key == "tail")
            })
            .map(|node| node.render_handle)
            .expect("tail node should exist");
        let right_edge_before = model
            .edges
            .iter()
            .find(|edge| edge.to_render_handle == right_handle)
            .map(|edge| edge.bezier_args)
            .expect("root -> right edge should exist");
        let tail_edge_before = model
            .edges
            .iter()
            .find(|edge| edge.to_render_handle == tail_handle)
            .map(|edge| edge.bezier_args)
            .expect("root -> tail edge should exist");

        let seed = 1u32;
        model.nodes[seed as usize].height += 7;
        let changed = engine.layout_changed_region(
            &mut state,
            &topology,
            &mut model,
            root,
            &[seed],
            &[],
            &[],
        );

        assert!(
            changed.edge_indexes().iter().any(|&edge_index| {
                model
                    .edges
                    .get(edge_index)
                    .is_some_and(|edge| edge.to_render_handle == right_handle)
            }),
            "right sibling edge must refresh when left seed growth replays later y layout"
        );
        assert!(
            changed.edge_indexes().iter().any(|&edge_index| {
                model
                    .edges
                    .get(edge_index)
                    .is_some_and(|edge| edge.to_render_handle == tail_handle)
            }),
            "tail sibling edge must refresh when left seed growth replays later y layout"
        );
        let right_edge_after = model
            .edges
            .iter()
            .find(|edge| edge.to_render_handle == right_handle)
            .map(|edge| edge.bezier_args)
            .expect("root -> right edge should still exist");
        let tail_edge_after = model
            .edges
            .iter()
            .find(|edge| edge.to_render_handle == tail_handle)
            .map(|edge| edge.bezier_args)
            .expect("root -> tail edge should still exist");
        assert!(
            right_edge_after != right_edge_before,
            "right sibling edge geometry must update after sibling reposition"
        );
        assert!(
            tail_edge_after != tail_edge_before,
            "tail sibling edge geometry must update after sibling reposition"
        );
    }
}
